//! Crash recovery: finish every transaction a crash left prepared, honestly.
//!
//! Both an interrupted apply and an interrupted restore recover FORWARD to the true originals
//! (a restore's desired IS the original), re-proving the terminal state with the SAME persisted
//! receipt under an unattended, bounded mode. A leaf the user edited externally is never
//! overwritten — that entry stays prepared and is reported as a conflict for the owner to resolve.

use dm_domain::system_tweaks::{RegistryBackend, SettingId, SystemProfileProbe};

use super::driver::TweakDriver;
use super::error::DriverError;
use super::journal::{JournalStore, TransactionIntent};
use super::verify::{VerificationBackend, VerificationPhase};

/// The result of a crash-recovery pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    /// Transactions driven to a clean terminal state.
    pub recovered: Vec<u64>,
    /// Transactions that could not be finished (an external edit blocks the rollback); each stays
    /// prepared for the owner to resolve.
    pub conflicts: Vec<RecoveryConflict>,
}

/// One transaction recovery could not finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryConflict {
    pub transaction: u64,
    pub feature: SettingId,
    pub cause: String,
}

impl<B, J, V, P> TweakDriver<B, J, V, P>
where
    B: RegistryBackend,
    J: JournalStore,
    V: VerificationBackend<B>,
    P: SystemProfileProbe,
{
    /// Finish every crash-left-prepared transaction. A clean recovery marks the transaction
    /// terminal; a blocked one is reported and left prepared (never a silent clobber).
    pub fn recover(&mut self) -> Result<RecoveryReport, DriverError> {
        let lease = self.lease()?;
        let mut report = RecoveryReport::default();
        let incomplete = self
            .journal
            .incomplete(&lease)
            .map_err(|error| DriverError::Journal(error.to_string()))?;

        for entry in incomplete {
            let conflict = |cause: String| RecoveryConflict {
                transaction: entry.id,
                feature: entry.feature.clone(),
                cause,
            };
            // An interrupted apply undoes back to `before`; an interrupted restore advances forward
            // to the original. Both refuse to overwrite an external edit or a policy takeover.
            match entry.intent {
                TransactionIntent::Apply => {
                    if let Err(cause) = self.undo_apply(&entry.values) {
                        report.conflicts.push(conflict(cause));
                        continue;
                    }
                    if let Err(cause) = self.verify_recovery(&entry, VerificationPhase::ApplyRollback)
                    {
                        report.conflicts.push(conflict(cause));
                        continue;
                    }
                    self.journal
                        .mark_apply_rolled_back(&lease, entry.id, &entry.feature)
                        .map_err(|error| DriverError::Journal(error.to_string()))?;
                }
                // A prepared restore has already ENTERED settle, so recover_restore treats a leaf at
                // the original as OUR progress (not an external takeover), disowns only a genuine
                // third value, and — critically — a Clean recovery MUST re-prove the terminal state
                // with the persisted receipt before commit (codex W1 R6: a failed effect proof must
                // never be laundered into a no-proof disown).
                TransactionIntent::Restore => match self.recover_restore(&entry.values) {
                    Ok(super::engine::RestoreSettle::Disown) => {
                        self.journal
                            .commit_restore(&lease, entry.id, &entry.feature)
                            .map_err(|error| DriverError::Journal(error.to_string()))?;
                    }
                    Ok(super::engine::RestoreSettle::Clean) => {
                        if let Err(cause) =
                            self.verify_recovery(&entry, VerificationPhase::RestoreOriginal)
                        {
                            report.conflicts.push(conflict(cause));
                            continue;
                        }
                        self.journal
                            .commit_restore(&lease, entry.id, &entry.feature)
                            .map_err(|error| DriverError::Journal(error.to_string()))?;
                    }
                    Err(cause) => {
                        report.conflicts.push(conflict(cause));
                        continue;
                    }
                },
            }
            report.recovered.push(entry.id);
        }
        Ok(report)
    }

    /// Re-prove a recovered transaction's terminal state with its PERSISTED receipt, unattended.
    fn verify_recovery(
        &mut self,
        entry: &super::journal::JournalEntry,
        phase: VerificationPhase,
    ) -> Result<(), String> {
        self.verify_terminal_mode(
            phase,
            entry.verification,
            &entry.receipt,
            &entry.values,
            super::verify::ExecutionMode::UnattendedRecovery,
        )
    }
}
