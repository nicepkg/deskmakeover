//! Shared transaction-engine internals used by both apply-failure rollback and crash recovery:
//! the never-clobber rollback and the receipt-gated terminal verification.

use dm_domain::system_tweaks::{
    ApplyOutcome, RegistryBackend, RegistrySnapshot, RegistryWriteIntent, SettingId,
    SystemProfileProbe,
};

use super::driver::{DriverError, TweakDriver};
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
    /// Roll each leaf back to its original, reverse order, writing ONLY where the live value is
    /// one this transaction could itself have produced (the original already, the desired we just
    /// wrote, or the before we captured). A live value that is none of those is an EXTERNAL EDIT:
    /// it is never overwritten — the whole rollback returns `Err` so the caller keeps the
    /// transaction prepared for recovery/manual resolution (contract 9/10, never-clobber).
    pub(super) fn rollback_to_original(
        &mut self,
        values: &[TransactionValue],
    ) -> Result<(), String> {
        let mut conflicts: Vec<String> = Vec::new();
        for value in values.iter().rev() {
            let live = match self.backend.read(&value.address) {
                Ok(live) => live,
                Err(error) => {
                    conflicts.push(format!("read {}: {error}", value.address));
                    continue;
                }
            };
            if live == value.original {
                continue; // already at the original — nothing to undo
            }
            let ours = live == value.desired || live == value.before;
            if !ours {
                // An external edit sits here — refuse to overwrite it.
                conflicts.push(format!("external edit at {}", value.address));
                continue;
            }
            if let Err(error) = self.backend.compare_exchange(
                RegistryWriteIntent::Undo,
                &value.address,
                &live,
                &value.original,
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

    /// Handle a failed apply: roll each leaf back to its original ONLY where safe, and mark the
    /// transaction rolled back only if the rollback finished cleanly and the rollback terminal
    /// state verifies. Otherwise leave it prepared for recovery.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fail_apply(
        &mut self,
        lease: &J::Lease,
        transaction: u64,
        feature: &SettingId,
        values: &[TransactionValue],
        plan: VerificationPlan,
        receipt: &VerificationReceipt,
        cause: &str,
    ) -> Result<ApplyOutcome, DriverError> {
        match self.rollback_to_original(values) {
            Ok(()) => {
                if let Err(verify_cause) =
                    self.verify_terminal(VerificationPhase::ApplyRollback, plan, receipt, values)
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
        self.verifier
            .verify_effect(&self.backend, &context)
            .map_err(|error| error.to_string())
    }
}

/// The expected terminal value for one leaf under a given phase: the desired value after an apply,
/// the true original after a rollback or a restore.
pub(super) fn expected_terminal(
    phase: VerificationPhase,
    value: &TransactionValue,
) -> RegistrySnapshot {
    match phase {
        VerificationPhase::ApplyDesired => value.desired.clone(),
        VerificationPhase::ApplyRollback | VerificationPhase::RestoreOriginal => {
            value.original.clone()
        }
    }
}
