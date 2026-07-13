//! Crash recovery: finish every transaction a crash left prepared, honestly.
//!
//! Both an interrupted apply and an interrupted restore recover FORWARD to the true originals
//! (a restore's desired IS the original), re-proving the terminal state with the SAME persisted
//! receipt under an unattended, bounded mode. A leaf the user edited externally is never
//! overwritten — that entry stays prepared and is reported as a conflict for the owner to resolve.

use dm_domain::system_tweaks::{RegistryBackend, SettingId, SystemProfileProbe};

use super::driver::{DriverError, TweakDriver};
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
            let phase = match entry.intent {
                TransactionIntent::Apply => VerificationPhase::ApplyRollback,
                TransactionIntent::Restore => VerificationPhase::RestoreOriginal,
            };
            // Roll each leaf back to its original where safe; an external edit blocks recovery.
            if let Err(cause) = self.rollback_to_original(&entry.values) {
                report.conflicts.push(RecoveryConflict {
                    transaction: entry.id,
                    feature: entry.feature,
                    cause,
                });
                continue;
            }
            // Re-prove the terminal state with the persisted receipt, unattended.
            if let Err(cause) = self.verify_terminal_mode(
                phase,
                entry.verification,
                &entry.receipt,
                &entry.values,
                super::verify::ExecutionMode::UnattendedRecovery,
            ) {
                report.conflicts.push(RecoveryConflict {
                    transaction: entry.id,
                    feature: entry.feature,
                    cause,
                });
                continue;
            }
            let result = match entry.intent {
                TransactionIntent::Apply => {
                    self.journal
                        .mark_apply_rolled_back(&lease, entry.id, &entry.feature)
                }
                TransactionIntent::Restore => {
                    self.journal.commit_restore(&lease, entry.id, &entry.feature)
                }
            };
            result.map_err(|error| DriverError::Journal(error.to_string()))?;
            report.recovered.push(entry.id);
        }
        Ok(report)
    }
}
