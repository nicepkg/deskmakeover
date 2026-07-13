//! The WAL-shaped transaction journal for calm settings.
//!
//! A separate contract from the icon `txn::JournalSink` (that spine is keyed on `ItemId` +
//! `Fingerprint` + `RestoreAnchor` — icon concepts; a registry tweak is keyed on
//! `RegistryAddress` + `RegistrySnapshot`). Every transaction is durably PREPARED before the first
//! registry write and only COMMITTED after terminal verification, so a crash at any point is
//! recoverable.
//!
//! Two guarantees the trait bakes in so a durable SQLite/WAL adapter can be dropped in later
//! WITHOUT changing the protocol (codex W1 review #3):
//!   1. an unforgeable [`WriterLease`] token threads `acquire → prepare → writes → commit`, so an
//!      inspect-to-prepare lock gap is unrepresentable at the call boundary;
//!   2. every terminal method is GENERATION-GUARDED — a commit is conditional on the managed
//!      generation seen at prepare, and prepare refuses to run while any transaction is still
//!      incomplete, so a stale writer can never clobber a newer managed record.
//!
//! W1 scope: an in-memory implementation. A durable cross-process adapter is a later slice.

use dm_domain::system_tweaks::{RegistryAddress, RegistrySnapshot, SettingId, WindowsEnvironment};

use super::verify::{VerificationPlan, VerificationReceipt};

/// An unforgeable proof that the holder owns the cross-process writer lease for the whole
/// transaction. `JournalStore` methods require the associated lease type, so a caller cannot
/// prepare or commit without first acquiring it — closing the inspect-to-prepare gap.
pub trait WriterLease {}

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
    /// Persisted pre-write evidence reused verbatim by terminal verification and recovery.
    pub receipt: VerificationReceipt,
    pub intent: TransactionIntent,
    pub state: TransactionState,
    pub values: Vec<TransactionValue>,
    /// The recipe's policy-guard addresses, captured at prepare so recovery checks THIS
    /// transaction's guards — not whatever the catalog holds now — before any reverse write. A guard
    /// that appeared since apply blocks the rollback/restore even if the leaf stays user-writable
    /// (the ACL probe alone would miss it; codex W2 R3).
    pub policy_guards: Vec<RegistryAddress>,
    /// The managed anchor visible before prepare; a commit is conditional on this generation so a
    /// stale writer can never overwrite a newer managed record, and a rollback restores it.
    pub managed_before: Option<ManagedSetting>,
}

/// The committed ownership anchor for a feature: what DeskMakeover wrote and what it must restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSetting {
    pub feature: SettingId,
    pub recipe_version: u32,
    pub environment: WindowsEnvironment,
    pub verification: VerificationPlan,
    /// The receipt proved at the apply that installed this anchor (audit trail; a restore writes a
    /// fresh journal receipt of its own).
    pub apply_receipt: VerificationReceipt,
    /// The recipe's policy-guard addresses at the apply that installed this anchor, so a later
    /// restore checks the transaction's own guards regardless of catalog drift (codex W2 R3).
    pub policy_guards: Vec<RegistryAddress>,
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

/// The arguments to [`JournalStore::prepare`], grouped so the call is not a wall of positional
/// parameters.
pub struct PrepareRequest {
    pub feature: SettingId,
    pub recipe_version: u32,
    pub environment: WindowsEnvironment,
    pub verification: VerificationPlan,
    pub receipt: VerificationReceipt,
    pub intent: TransactionIntent,
    pub values: Vec<TransactionValue>,
    /// The recipe's policy-guard addresses, persisted with the transaction for the reverse paths.
    pub policy_guards: Vec<RegistryAddress>,
    /// The managed generation the caller observed under the lease; prepare rejects a stale value.
    pub managed_before: Option<ManagedSetting>,
}

/// The durable transaction record. A production adapter combines a SQLite WAL transaction with an
/// OS file lock behind the same lease; the generation guards here are the protocol a durable
/// adapter must honour.
pub trait JournalStore {
    type Lease: WriterLease;

    /// Acquire the cross-process writer lease held for the whole transaction.
    fn acquire_writer_lease(&self) -> Result<Self::Lease, JournalError>;

    /// Durably record a prepared transaction before any registry write; returns its id. Rejects
    /// if any transaction is still incomplete, or if `managed_before` no longer matches the live
    /// managed generation for the feature (a stale caller observation).
    fn prepare(&mut self, lease: &Self::Lease, request: PrepareRequest)
        -> Result<u64, JournalError>;

    /// Transactions prepared but never terminal (crash recovery input).
    fn incomplete(&self, lease: &Self::Lease) -> Result<Vec<JournalEntry>, JournalError>;

    /// The committed anchor for one feature, if DeskMakeover currently owns it.
    fn managed(
        &self,
        lease: &Self::Lease,
        feature: &SettingId,
    ) -> Result<Option<ManagedSetting>, JournalError>;

    /// Every committed anchor.
    fn managed_all(&self, lease: &Self::Lease) -> Result<Vec<ManagedSetting>, JournalError>;

    /// Commit an apply: install the managed anchor and mark the transaction committed. Validates
    /// the entry is `Prepared` + `Apply` for this feature and the managed generation still matches.
    fn commit_apply(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        setting: ManagedSetting,
    ) -> Result<(), JournalError>;

    /// Commit a restore: drop the managed anchor and mark the transaction committed. Validates the
    /// entry is `Prepared` + `Restore` for this feature and the managed generation still matches.
    fn commit_restore(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError>;

    /// Mark a prepared apply rolled back: RESTORE its `managed_before` anchor (never blindly
    /// delete a possibly-newer one) and mark the transaction rolled back.
    fn mark_apply_rolled_back(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError>;
}

/// The in-memory lease — a zero-sized unforgeable token (only the store constructs it).
#[derive(Debug)]
pub struct MemoryLease(());

impl WriterLease for MemoryLease {}

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

    fn managed_for(&self, feature: &SettingId) -> Option<ManagedSetting> {
        self.managed
            .iter()
            .find(|setting| &setting.feature == feature)
            .cloned()
    }

    /// The prepared (still-incomplete) entry for `transaction`, validated to match `intent` and
    /// `feature` and to still agree with the live managed generation it was prepared against.
    fn guarded_entry(
        &self,
        transaction: u64,
        intent: TransactionIntent,
        feature: &SettingId,
    ) -> Result<&JournalEntry, JournalError> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == transaction)
            .ok_or_else(|| JournalError(format!("unknown transaction {transaction}")))?;
        if entry.state != TransactionState::Prepared {
            return Err(JournalError(format!(
                "transaction {transaction} is not prepared ({:?})",
                entry.state
            )));
        }
        if entry.intent != intent {
            return Err(JournalError(format!(
                "transaction {transaction} intent mismatch"
            )));
        }
        if &entry.feature != feature {
            return Err(JournalError(format!(
                "transaction {transaction} feature mismatch"
            )));
        }
        if self.managed_for(feature) != entry.managed_before {
            return Err(JournalError(format!(
                "managed generation moved under transaction {transaction}"
            )));
        }
        Ok(entry)
    }

    fn set_state(&mut self, transaction: u64, state: TransactionState) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == transaction) {
            entry.state = state;
        }
    }

    fn install_anchor(&mut self, setting: ManagedSetting) {
        self.managed.retain(|s| s.feature != setting.feature);
        self.managed.push(setting);
    }

    fn set_anchor(&mut self, feature: &SettingId, anchor: Option<ManagedSetting>) {
        self.managed.retain(|s| &s.feature != feature);
        if let Some(anchor) = anchor {
            self.managed.push(anchor);
        }
    }

    /// Test-only: bump a committed anchor's recorded recipe version to simulate a recipe migration.
    #[cfg(test)]
    pub(crate) fn bump_managed_version_for_test(&mut self, feature: &SettingId) {
        if let Some(setting) = self.managed.iter_mut().find(|s| &s.feature == feature) {
            setting.recipe_version += 1;
        }
    }
}

impl JournalStore for MemoryJournal {
    type Lease = MemoryLease;

    fn acquire_writer_lease(&self) -> Result<Self::Lease, JournalError> {
        Ok(MemoryLease(()))
    }

    fn prepare(
        &mut self,
        _lease: &Self::Lease,
        request: PrepareRequest,
    ) -> Result<u64, JournalError> {
        // A new transaction may not begin while another is incomplete (atomic gate).
        if self
            .entries
            .iter()
            .any(|entry| entry.state == TransactionState::Prepared)
        {
            return Err(JournalError("a prior transaction is still incomplete".into()));
        }
        // The caller's observed generation must still be live.
        if self.managed_for(&request.feature) != request.managed_before {
            return Err(JournalError(format!(
                "managed generation moved before prepare of {}",
                request.feature
            )));
        }
        self.next_id += 1;
        let id = self.next_id;
        self.entries.push(JournalEntry {
            id,
            feature: request.feature,
            recipe_version: request.recipe_version,
            environment: request.environment,
            verification: request.verification,
            receipt: request.receipt,
            intent: request.intent,
            state: TransactionState::Prepared,
            values: request.values,
            policy_guards: request.policy_guards,
            managed_before: request.managed_before,
        });
        Ok(id)
    }

    fn incomplete(&self, _lease: &Self::Lease) -> Result<Vec<JournalEntry>, JournalError> {
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.state == TransactionState::Prepared)
            .cloned()
            .collect())
    }

    fn managed(
        &self,
        _lease: &Self::Lease,
        feature: &SettingId,
    ) -> Result<Option<ManagedSetting>, JournalError> {
        Ok(self.managed_for(feature))
    }

    fn managed_all(&self, _lease: &Self::Lease) -> Result<Vec<ManagedSetting>, JournalError> {
        Ok(self.managed.clone())
    }

    fn commit_apply(
        &mut self,
        _lease: &Self::Lease,
        transaction: u64,
        setting: ManagedSetting,
    ) -> Result<(), JournalError> {
        self.guarded_entry(transaction, TransactionIntent::Apply, &setting.feature)?;
        self.set_state(transaction, TransactionState::Committed);
        self.install_anchor(setting);
        Ok(())
    }

    fn commit_restore(
        &mut self,
        _lease: &Self::Lease,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError> {
        self.guarded_entry(transaction, TransactionIntent::Restore, feature)?;
        self.set_state(transaction, TransactionState::Committed);
        self.set_anchor(feature, None);
        Ok(())
    }

    fn mark_apply_rolled_back(
        &mut self,
        _lease: &Self::Lease,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError> {
        let entry = self.guarded_entry(transaction, TransactionIntent::Apply, feature)?;
        // Restore the anchor that existed before this apply (None for a fresh apply, or the prior
        // managed record for a re-apply). Never blindly delete a possibly-newer anchor.
        let restore_to = entry.managed_before.clone();
        self.set_state(transaction, TransactionState::RolledBack);
        self.set_anchor(feature, restore_to);
        Ok(())
    }
}
