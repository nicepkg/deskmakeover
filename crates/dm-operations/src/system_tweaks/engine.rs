//! Shared transaction-engine internals used by both apply-failure rollback and crash recovery:
//! the never-clobber rollback and the receipt-gated terminal verification.

use dm_domain::system_tweaks::{
    ApplyOutcome, RegistryAddress, RegistryBackend, RegistrySnapshot, RegistryWriteIntent,
    SettingId, SystemProfileProbe, WindowsEnvironment,
};

use super::catalog::TweakDescriptor;
use super::driver::TweakDriver;
use super::error::DriverError;
use super::journal::{JournalStore, TransactionValue};
use super::verify::{
    ExecutionMode, VerificationBackend, VerificationContext, VerificationPhase, VerificationPlan,
    VerificationReceipt,
};

impl<B, J, V, P> TweakDriver<B, J, V, P>
where
    B: RegistryBackend,
    J: JournalStore,
    V: VerificationBackend<B>,
    P: SystemProfileProbe,
{
    /// Re-confirm the environment fingerprint is unchanged and no policy guard has appeared. A
    /// mismatch means we must not write on the (now uncertified) environment (codex W1 R3 NEW).
    pub(super) fn reauth(
        &self,
        descriptor: &TweakDescriptor,
        environment: &WindowsEnvironment,
    ) -> Result<(), String> {
        let current = self.probe_environment().map_err(|error| error.to_string())?;
        if &current != environment {
            return Err("environment fingerprint changed".to_string());
        }
        if let Some(guard) = self.guard_present(descriptor).map_err(|error| error.to_string())? {
            return Err(format!("policy guard appeared at {guard}"));
        }
        Ok(())
    }

    /// [`reauth`](Self::reauth) plus a per-leaf backend-managed check immediately before the write.
    pub(super) fn reauth_before_write(
        &self,
        descriptor: &TweakDescriptor,
        environment: &WindowsEnvironment,
        address: &RegistryAddress,
    ) -> Result<(), String> {
        self.reauth(descriptor, environment)?;
        if self
            .is_backend_managed(address)
            .map_err(|error| error.to_string())?
        {
            return Err(format!("target became policy-managed: {address}"));
        }
        Ok(())
    }

    /// Undo a failed/interrupted APPLY back to each leaf's `before` (the value we OBSERVED when the
    /// transaction started — NOT the true original), reverse order, writing ONLY a leaf still at
    /// the `desired` value we wrote. A leaf already at `before` was never written (no-op); a leaf
    /// at any OTHER value is an external edit and is never overwritten — the whole undo returns
    /// `Err` so the caller keeps the transaction prepared for recovery (contract 9/10). Undoing to
    /// `before` (not `original`) is what makes a failed RE-apply preserve the user's drift instead
    /// of forcing the true original (codex W1 R2 #2).
    pub(super) fn undo_apply(&mut self, values: &[TransactionValue]) -> Result<(), String> {
        let mut conflicts: Vec<String> = Vec::new();
        for value in values.iter().rev() {
            let live = match self.backend.read(&value.address) {
                Ok(live) => live,
                Err(error) => {
                    conflicts.push(format!("read {}: {error}", value.address));
                    continue;
                }
            };
            if live == value.before {
                continue; // never written, or already undone
            }
            if live != value.desired {
                conflicts.push(format!("external edit at {}", value.address));
                continue;
            }
            // A policy that took over this leaf since our write is NEVER overwritten, even to undo
            // (codex W1 R4 #1) — record a conflict and leave it prepared.
            match self.backend.is_policy_managed(&value.address) {
                Ok(true) => {
                    conflicts.push(format!("policy-managed, not undone: {}", value.address));
                    continue;
                }
                Ok(false) => {}
                Err(error) => {
                    conflicts.push(format!("managed check {}: {error}", value.address));
                    continue;
                }
            }
            if let Err(error) = self.backend.compare_exchange(
                RegistryWriteIntent::Undo,
                &value.address,
                &live,
                &value.before,
            ) {
                conflicts.push(format!("undo {}: {error}", value.address));
            }
        }
        if conflicts.is_empty() {
            Ok(())
        } else {
            Err(conflicts.join("; "))
        }
    }

    /// Drive a RESTORE forward to each leaf's original (`desired` for restore values). CLASSIFY all
    /// leaves read-only first, then write — so an external edit never leaves a partial write, and
    /// the three outcomes are separated (codex W1 R5 #1): a pure external edit is a `Disown` the
    /// caller commits as `SkippedExternalConflict` (never a permanent Pending); a policy takeover or
    /// I/O failure is a hard `Err` that keeps the transaction prepared for recovery.
    pub(super) fn advance_restore(
        &mut self,
        values: &[TransactionValue],
    ) -> Result<RestoreSettle, String> {
        // Phase 1 — classify (read-only). We OWN a leaf only while its live value still equals the
        // `before` (last_applied) we recorded; a live value that has moved AT ALL means the user
        // took it back (even to the original) → disown, never overwrite. A policy takeover or an
        // I/O failure is a hard block.
        let mut external = false;
        for value in values {
            if self
                .backend
                .is_policy_managed(&value.address)
                .map_err(|error| format!("managed check {}: {error}", value.address))?
            {
                return Err(format!("policy-managed, not restored: {}", value.address));
            }
            let live = self
                .backend
                .read(&value.address)
                .map_err(|error| format!("read {}: {error}", value.address))?;
            if live != value.before {
                external = true;
            }
        }
        if external {
            return Ok(RestoreSettle::Disown);
        }
        // Phase 2 — write each leaf back to the original with the CAS expecting `before`, so a race
        // that landed after classification is a CAS conflict (→ block) and never a clobber.
        for value in values {
            if value.desired == value.before {
                continue; // nothing to change
            }
            self.backend
                .compare_exchange(
                    RegistryWriteIntent::Undo,
                    &value.address,
                    &value.before,
                    &value.desired,
                )
                .map_err(|error| format!("restore {}: {error}", value.address))?;
        }
        Ok(RestoreSettle::Clean)
    }

    /// Handle a failed apply: undo ONLY the leaves this transaction actually wrote (`written`),
    /// back to their `before`, and mark the transaction rolled back only if the undo finished
    /// cleanly and the rollback terminal state verifies. A leaf we never wrote is left untouched
    /// (a pure CAS conflict that wrote nothing aborts cleanly). Otherwise the transaction stays
    /// prepared for recovery.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fail_apply(
        &mut self,
        lease: &J::Lease,
        transaction: u64,
        feature: &SettingId,
        values: &[TransactionValue],
        written: &[TransactionValue],
        plan: VerificationPlan,
        receipt: &VerificationReceipt,
        cause: &str,
    ) -> Result<ApplyOutcome, DriverError> {
        let _ = values; // the full value set is kept in the journal entry; the undo acts on `written`
        // Nothing was written (a CAS conflict on the first leaf, say) → the desktop is untouched by
        // us, so the transaction aborts cleanly with no rollback to prove.
        if written.is_empty() {
            self.journal
                .mark_apply_rolled_back(lease, transaction, feature)
                .map_err(|error| DriverError::Journal(error.to_string()))?;
            return Ok(ApplyOutcome::Reverted);
        }
        match self.undo_apply(written) {
            Ok(()) => {
                // Verify only the leaves we wrote are back to their `before`; unwritten leaves were
                // never touched and are not this transaction's to prove.
                if let Err(verify_cause) =
                    self.verify_terminal(VerificationPhase::ApplyRollback, plan, receipt, written)
                {
                    return Err(DriverError::Pending {
                        transaction,
                        cause: format!("{cause}; rollback verify: {verify_cause}"),
                    });
                }
                self.journal
                    .mark_apply_rolled_back(lease, transaction, feature)
                    .map_err(|error| DriverError::Journal(error.to_string()))?;
                Ok(ApplyOutcome::Reverted)
            }
            Err(conflict) => Err(DriverError::Pending {
                transaction,
                cause: format!("{cause}; rollback blocked: {conflict}"),
            }),
        }
    }

    /// Prove a terminal state: validate the receipt satisfies the plan, settle, confirm every
    /// leaf's delayed read-back, then run the effect verifier against the receipt.
    pub(super) fn verify_terminal(
        &mut self,
        phase: VerificationPhase,
        plan: VerificationPlan,
        receipt: &VerificationReceipt,
        values: &[TransactionValue],
    ) -> Result<(), String> {
        self.verify_terminal_mode(phase, plan, receipt, values, ExecutionMode::Foreground)
    }

    /// Terminal proof under an explicit execution mode (recovery runs `UnattendedRecovery`).
    pub(super) fn verify_terminal_mode(
        &mut self,
        phase: VerificationPhase,
        plan: VerificationPlan,
        receipt: &VerificationReceipt,
        values: &[TransactionValue],
        mode: ExecutionMode,
    ) -> Result<(), String> {
        // Recovery cannot substitute a weaker receipt than the plan demands (contract 5).
        if !receipt.satisfies(plan.effect) {
            return Err(format!(
                "receipt does not satisfy the {:?} verifier",
                plan.effect
            ));
        }
        let expected: Vec<_> = values
            .iter()
            .map(|value| (value.address.clone(), expected_terminal(phase, value)))
            .collect();
        let context = VerificationContext {
            phase,
            plan,
            receipt: receipt.clone(),
            execution_mode: mode,
            expected: expected.clone(),
        };
        self.verifier
            .settle(&mut self.backend, &context)
            .map_err(|error| error.to_string())?;
        for (address, want) in &expected {
            let live = self
                .backend
                .read(address)
                .map_err(|error| error.to_string())?;
            if &live != want {
                return Err(format!("delayed read-back mismatch at {address}"));
            }
        }
        // The feature effect proof runs in EVERY direction (contract 8/§376-380: a matching raw
        // value is never proof the surface reloaded). The verifier is PHASE-AWARE — it proves the
        // apply effect (promotions absent, Recent preserved) on ApplyDesired and proves the surface
        // reloaded the original/before on a rollback/restore. Running the wrong-direction predicate
        // was the earlier bug; the fix is a direction-aware predicate, not a skipped proof.
        self.verifier
            .verify_effect(&self.backend, &context)
            .map_err(|error| error.to_string())
    }
}

/// The outcome of an `advance_restore` classification: a clean restore that wrote the originals,
/// or an external edit that means the feature is no longer ours (disown, never overwrite).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RestoreSettle {
    Clean,
    Disown,
}

/// The expected terminal value for one leaf under a given phase:
/// - `ApplyDesired` → the desired value we established;
/// - `ApplyRollback` → the `before` we OBSERVED (undoing a failed apply restores the pre-apply
///   state, which for a re-apply is the user's drift, NOT the true original — codex W1 R2 #2);
/// - `RestoreOriginal` → the original (`desired` for a restore's values).
pub(super) fn expected_terminal(
    phase: VerificationPhase,
    value: &TransactionValue,
) -> RegistrySnapshot {
    match phase {
        VerificationPhase::ApplyDesired => value.desired.clone(),
        VerificationPhase::ApplyRollback => value.before.clone(),
        VerificationPhase::RestoreOriginal => value.original.clone(),
    }
}
