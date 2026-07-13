//! The WAL-shaped transaction journal for calm settings.
//!
//! A separate contract from the icon `txn::JournalSink` (that spine is keyed on `ItemId` +
//! `Fingerprint` + `RestoreAnchor` — icon concepts; a registry tweak is keyed on
//! `RegistryAddress` + `RegistrySnapshot`). This mirrors the research reference's `JournalStore`:
//! every transaction is durably PREPARED before the first registry write and only COMMITTED after
//! terminal verification, so a crash at any point is recoverable.
//!
//! W1 scope: an in-memory implementation, Mac-testable. A durable SQLite/WAL adapter with a
//! cross-process writer lease is a later slice (the reference explicitly ships the memory journal
//! and documents the SQLite requirement).

use dm_domain::system_tweaks::{RegistryAddress, RegistrySnapshot, SettingId, WindowsEnvironment};

use super::verify::VerificationPlan;

/// Whether a journalled transaction is applying a recipe or restoring originals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionIntent {
    Apply,
    Restore,
}

/// The lifecycle state of a journalled transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    Prepared,
    Committed,
    RolledBack,
}

/// One registry leaf inside a transaction: its true original (captured before DeskMakeover ever
/// owned it), the value live immediately before this transaction, and the value to establish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionValue {
    pub address: RegistryAddress,
    pub original: RegistrySnapshot,
    pub before: RegistrySnapshot,
    pub desired: RegistrySnapshot,
}

/// The durable pre-write record of one transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    pub id: u64,
    pub feature: SettingId,
    pub recipe_version: u32,
    pub environment: WindowsEnvironment,
    /// Persisted before any write so recovery cannot pick a weaker verifier than apply used.
    pub verification: VerificationPlan,
    pub intent: TransactionIntent,
    pub state: TransactionState,
    pub values: Vec<TransactionValue>,
    /// The anchor visible before prepare; a commit is conditional on this generation so a stale
    /// writer can never overwrite a newer managed record.
    pub managed_before: Option<ManagedSetting>,
}

/// The committed ownership anchor for a feature: what DeskMakeover wrote and what it must restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSetting {
    pub feature: SettingId,
    pub recipe_version: u32,
    pub environment: WindowsEnvironment,
    pub verification: VerificationPlan,
    pub last_transaction: u64,
    pub values: Vec<ManagedValue>,
}

/// One committed leaf: its true original and the value DeskMakeover last applied to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedValue {
    pub address: RegistryAddress,
    pub original: RegistrySnapshot,
    pub last_applied: RegistrySnapshot,
}

/// A journal failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalError(pub String);

impl std::fmt::Display for JournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for JournalError {}

/// The durable transaction record. A production adapter combines a SQLite WAL transaction with an
/// OS file lock; this contract already forbids an inspect-to-prepare gap by threading the same
/// store through the whole apply/restore call.
pub trait JournalStore {
    /// Durably record a prepared transaction before any registry write; returns its id.
    #[allow(clippy::too_many_arguments)]
    fn prepare(
        &mut self,
        feature: SettingId,
        recipe_version: u32,
        environment: WindowsEnvironment,
        verification: VerificationPlan,
        intent: TransactionIntent,
        values: Vec<TransactionValue>,
        managed_before: Option<ManagedSetting>,
    ) -> Result<u64, JournalError>;

    /// Transactions that were prepared but never reached a terminal state (crash recovery input).
    fn incomplete(&self) -> Result<Vec<JournalEntry>, JournalError>;

    /// The committed anchor for one feature, if DeskMakeover currently owns it.
    fn managed(&self, feature: &SettingId) -> Result<Option<ManagedSetting>, JournalError>;

    /// Every committed anchor.
    fn managed_all(&self) -> Result<Vec<ManagedSetting>, JournalError>;

    /// Commit an apply: install the managed anchor and mark the transaction committed.
    fn commit_apply(
        &mut self,
        transaction: u64,
        setting: ManagedSetting,
    ) -> Result<(), JournalError>;

    /// Commit a restore: drop the managed anchor and mark the transaction committed.
    fn commit_restore(
        &mut self,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError>;

    /// Mark a prepared apply as rolled back (its originals were restored).
    fn mark_apply_rolled_back(
        &mut self,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError>;
}

/// Deterministic in-memory journal used by tests and the Mac devhost loop.
#[derive(Debug, Default)]
pub struct MemoryJournal {
    next_id: u64,
    entries: Vec<JournalEntry>,
    managed: Vec<ManagedSetting>,
}

impl MemoryJournal {
    pub fn new() -> Self {
        Self::default()
    }

    fn entry_mut(&mut self, transaction: u64) -> Result<&mut JournalEntry, JournalError> {
        self.entries
            .iter_mut()
            .find(|entry| entry.id == transaction)
            .ok_or_else(|| JournalError(format!("unknown transaction {transaction}")))
    }
}

impl JournalStore for MemoryJournal {
    fn prepare(
        &mut self,
        feature: SettingId,
        recipe_version: u32,
        environment: WindowsEnvironment,
        verification: VerificationPlan,
        intent: TransactionIntent,
        values: Vec<TransactionValue>,
        managed_before: Option<ManagedSetting>,
    ) -> Result<u64, JournalError> {
        self.next_id += 1;
        let id = self.next_id;
        self.entries.push(JournalEntry {
            id,
            feature,
            recipe_version,
            environment,
            verification,
            intent,
            state: TransactionState::Prepared,
            values,
            managed_before,
        });
        Ok(id)
    }

    fn incomplete(&self) -> Result<Vec<JournalEntry>, JournalError> {
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.state == TransactionState::Prepared)
            .cloned()
            .collect())
    }

    fn managed(&self, feature: &SettingId) -> Result<Option<ManagedSetting>, JournalError> {
        Ok(self
            .managed
            .iter()
            .find(|setting| &setting.feature == feature)
            .cloned())
    }

    fn managed_all(&self) -> Result<Vec<ManagedSetting>, JournalError> {
        Ok(self.managed.clone())
    }

    fn commit_apply(
        &mut self,
        transaction: u64,
        setting: ManagedSetting,
    ) -> Result<(), JournalError> {
        // Conditional on the generation seen at prepare: a stale writer cannot clobber a newer
        // managed record (the reference's atomic entry+anchor rule).
        let expected = self.entry_mut(transaction)?.managed_before.clone();
        let current = self
            .managed
            .iter()
            .find(|s| s.feature == setting.feature)
            .cloned();
        if current != expected {
            return Err(JournalError(format!(
                "managed generation moved under transaction {transaction}"
            )));
        }
        self.entry_mut(transaction)?.state = TransactionState::Committed;
        self.managed.retain(|s| s.feature != setting.feature);
        self.managed.push(setting);
        Ok(())
    }

    fn commit_restore(
        &mut self,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError> {
        self.entry_mut(transaction)?.state = TransactionState::Committed;
        self.managed.retain(|s| &s.feature != feature);
        Ok(())
    }

    fn mark_apply_rolled_back(
        &mut self,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError> {
        self.entry_mut(transaction)?.state = TransactionState::RolledBack;
        // A rolled-back apply never owned the feature; ensure no anchor lingers.
        self.managed.retain(|s| &s.feature != feature);
        Ok(())
    }
}
