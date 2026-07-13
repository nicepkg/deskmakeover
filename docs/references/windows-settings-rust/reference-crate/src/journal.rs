use std::error::Error;
use std::fmt;

use crate::{
    RegistryAddress, RegistryKey, RegistrySnapshot, SettingId, VerificationPlan,
    VerificationReceipt, WindowsEnvironment,
};

mod lease;
mod memory;

pub use lease::{MemoryWriterLease, WriterLease};
pub use memory::MemoryJournal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionIntent {
    Apply,
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    Prepared,
    Committed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionValue {
    pub address: RegistryAddress,
    /// The true value captured before DeskMakeover first owned this feature.
    pub original: RegistrySnapshot,
    /// The live value immediately before this transaction.
    pub before: RegistrySnapshot,
    /// The value this transaction intends to establish.
    pub desired: RegistrySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    pub id: u64,
    pub feature: SettingId,
    pub recipe_version: u32,
    pub environment: WindowsEnvironment,
    /// Persisted before any external write so recovery cannot select a weaker verifier.
    pub verification: VerificationPlan,
    /// Persisted pre-write evidence reused verbatim by terminal verification and recovery.
    pub receipt: VerificationReceipt,
    pub intent: TransactionIntent,
    pub state: TransactionState,
    pub values: Vec<TransactionValue>,
    /// Pre-WAL observations only. A candidate is never cleanup authority by itself.
    pub candidate_keys: Vec<RegistryKey>,
    /// Native `Created` dispositions durably confirmed after successful registry calls.
    pub confirmed_created_keys: Vec<RegistryKey>,
    /// Inherited ownership plus confirmed creations; apply rollback uses only confirmed creations.
    pub cleanup_owned_keys: Vec<RegistryKey>,
    /// Anchor visible before prepare. Durable stores conditionally commit against this generation.
    pub managed_before: Option<ManagedSetting>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedValue {
    pub address: RegistryAddress,
    pub original: RegistrySnapshot,
    pub last_applied: RegistrySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSetting {
    pub feature: SettingId,
    pub recipe_version: u32,
    pub environment: WindowsEnvironment,
    /// The exact terminal-state proof used before this anchor was installed.
    pub verification: VerificationPlan,
    /// Audit receipt from the successful apply transaction. Restore writes a new journal receipt.
    pub apply_receipt: VerificationReceipt,
    pub last_transaction: u64,
    pub values: Vec<ManagedValue>,
    /// Shared cleanup ownership inherited from existing anchors plus this apply's confirmations.
    pub cleanup_owned_keys: Vec<RegistryKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalError(pub String);

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for JournalError {}

/// Durable implementations serialize all reads and writes with one cross-process writer lease.
/// Every transaction method requires the associated unforgeable lease type, making an
/// inspect-to-prepare lock gap impossible at the engine composition boundary.
///
/// A production SQLite adapter should combine WAL transactions with an OS file lock. The lease
/// must cover `incomplete/managed -> runtime probe -> registry inspection -> prepare -> registry
/// writes -> terminal commit/rollback`. Atomic entry+anchor rules remain unchanged.
pub trait JournalStore {
    type Lease: WriterLease;

    fn acquire_writer_lease(&self) -> Result<Self::Lease, JournalError>;

    /// Every argument is part of the durable pre-write record.
    #[allow(clippy::too_many_arguments)]
    fn prepare(
        &mut self,
        lease: &Self::Lease,
        feature: SettingId,
        recipe_version: u32,
        environment: WindowsEnvironment,
        verification: VerificationPlan,
        receipt: VerificationReceipt,
        intent: TransactionIntent,
        values: Vec<TransactionValue>,
        candidate_keys: Vec<RegistryKey>,
        cleanup_owned_keys: Vec<RegistryKey>,
        managed_before: Option<ManagedSetting>,
    ) -> Result<u64, JournalError>;

    /// Durably promotes only native `Created` dispositions from candidate to cleanup ownership.
    fn confirm_created_keys(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        keys: &[RegistryKey],
    ) -> Result<(), JournalError>;

    fn incomplete(&self, lease: &Self::Lease) -> Result<Vec<JournalEntry>, JournalError>;

    fn managed(
        &self,
        lease: &Self::Lease,
        feature: &SettingId,
    ) -> Result<Option<ManagedSetting>, JournalError>;

    fn managed_all(&self, lease: &Self::Lease) -> Result<Vec<ManagedSetting>, JournalError>;

    fn commit_apply(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        setting: ManagedSetting,
    ) -> Result<(), JournalError>;

    fn commit_restore(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError>;

    fn mark_apply_rolled_back(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError>;
}
