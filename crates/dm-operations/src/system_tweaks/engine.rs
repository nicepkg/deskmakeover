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

    /// Drive a RESTORE forward to each leaf's original (`desired` for restore values), writing ONLY
    /// a leaf still at the `before` value we last applied. A leaf already at the original is a
    /// no-op; a leaf at any other value is an external edit and is never overwritten (never-clobber).
    pub(super) fn advance_restore(&mut self, values: &[TransactionValue]) -> Result<(), String> {
        let mut conflicts: Vec<String> = Vec::new();
        for value in values {
            let live = match self.backend.read(&value.address) {
                Ok(live) => live,
                Err(error) => {
                    conflicts.push(format!("read {}: {error}", value.address));
                    continue;
                }
            };
            if live == value.desired {
                continue; // already at the original
            }
            if live != value.before {
                conflicts.push(format!("external edit at {}", value.address));
                continue;
            }
            if let Err(error) = self.backend.compare_exchange(
                RegistryWriteIntent::Undo,
                &value.address,
                &live,
                &value.desired,
            ) {
                conflicts.push(format!("restore {}: {error}", value.address));
            }
        }
        if conflicts.is_empty() {
            Ok(())
        } else {
            Err(conflicts.join("; "))
        }
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
