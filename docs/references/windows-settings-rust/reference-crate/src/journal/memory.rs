use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::{RegistryKey, SettingId, VerificationPlan, VerificationReceipt, WindowsEnvironment};

use super::lease::{self, MemoryLeaseState};
use super::{
    JournalEntry, JournalError, JournalStore, ManagedSetting, MemoryWriterLease, TransactionIntent,
    TransactionState, TransactionValue,
};

#[derive(Debug)]
pub struct MemoryJournal {
    owner_id: u64,
    lease_state: Arc<Mutex<MemoryLeaseState>>,
    next_id: u64,
    entries: BTreeMap<u64, JournalEntry>,
    managed: BTreeMap<SettingId, ManagedSetting>,
    fail_next_atomic_commit: bool,
    fail_next_created_key_confirmation: bool,
}

impl Default for MemoryJournal {
    fn default() -> Self {
        let (owner_id, lease_state) = lease::new_owner();
        Self {
            owner_id,
            lease_state,
            next_id: 0,
            entries: BTreeMap::new(),
            managed: BTreeMap::new(),
            fail_next_atomic_commit: false,
            fail_next_created_key_confirmation: false,
        }
    }
}

impl MemoryJournal {
    /// Non-transactional diagnostic snapshot for deterministic tests only.
    pub fn entry(&self, id: u64) -> Option<JournalEntry> {
        self.entries.get(&id).cloned()
    }

    /// Non-transactional diagnostic count for deterministic tests only.
    pub fn prepared_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| entry.state == TransactionState::Prepared)
            .count()
    }

    /// Non-transactional diagnostic anchor snapshot for deterministic tests only.
    pub fn managed(&self, feature: &SettingId) -> Result<Option<ManagedSetting>, JournalError> {
        Ok(self.managed.get(feature).cloned())
    }

    /// Injects a failure before the next atomic commit changes either journal state or anchor.
    pub fn fail_next_atomic_commit(&mut self) {
        self.fail_next_atomic_commit = true;
    }

    /// Simulates losing durability after a native create/write but before ownership promotion.
    pub fn fail_next_created_key_confirmation(&mut self) {
        self.fail_next_created_key_confirmation = true;
    }

    fn validate(&self, lease: &MemoryWriterLease) -> Result<(), JournalError> {
        lease::validate(self.owner_id, &self.lease_state, lease)
    }

    fn entry_mut(&mut self, id: u64) -> Result<&mut JournalEntry, JournalError> {
        self.entries
            .get_mut(&id)
            .ok_or_else(|| JournalError(format!("transaction {id} not found")))
    }

    fn fail_before_atomic_commit(&mut self) -> Result<(), JournalError> {
        if self.fail_next_atomic_commit {
            self.fail_next_atomic_commit = false;
            return Err(JournalError("injected atomic journal failure".into()));
        }
        Ok(())
    }

    fn prepared_entry(
        &self,
        id: u64,
        intent: TransactionIntent,
    ) -> Result<&JournalEntry, JournalError> {
        let entry = self
            .entries
            .get(&id)
            .ok_or_else(|| JournalError(format!("transaction {id} not found")))?;
        if entry.state != TransactionState::Prepared || entry.intent != intent {
            return Err(JournalError(format!(
                "transaction {id} is not a matching Prepared entry"
            )));
        }
        Ok(entry)
    }
}

impl JournalStore for MemoryJournal {
    type Lease = MemoryWriterLease;

    fn acquire_writer_lease(&self) -> Result<Self::Lease, JournalError> {
        lease::acquire(self.owner_id, &self.lease_state)
    }

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
    ) -> Result<u64, JournalError> {
        self.validate(lease)?;
        if self
            .entries
            .values()
            .any(|entry| entry.state == TransactionState::Prepared)
        {
            return Err(JournalError(
                "recovery required before preparing another transaction".into(),
            ));
        }
        if self.managed.get(&feature) != managed_before.as_ref() {
            return Err(JournalError(
                "managed anchor generation changed before prepare".into(),
            ));
        }
        self.next_id += 1;
        let id = self.next_id;
        self.entries.insert(
            id,
            JournalEntry {
                id,
                feature,
                recipe_version,
                environment,
                verification,
                receipt,
                intent,
                state: TransactionState::Prepared,
                values,
                candidate_keys,
                confirmed_created_keys: Vec::new(),
                cleanup_owned_keys,
                managed_before,
            },
        );
        Ok(id)
    }

    fn confirm_created_keys(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        keys: &[RegistryKey],
    ) -> Result<(), JournalError> {
        self.validate(lease)?;
        self.prepared_entry(transaction, TransactionIntent::Apply)?;
        if self.fail_next_created_key_confirmation {
            self.fail_next_created_key_confirmation = false;
            return Err(JournalError(
                "injected created-key confirmation failure".into(),
            ));
        }
        let entry = self.entry_mut(transaction)?;
        for key in keys {
            if !entry
                .candidate_keys
                .iter()
                .any(|candidate| same_key(candidate, key))
            {
                return Err(JournalError(format!(
                    "native-created key was not a pre-WAL candidate: {key}"
                )));
            }
        }
        merge_keys(&mut entry.confirmed_created_keys, keys);
        merge_keys(&mut entry.cleanup_owned_keys, keys);
        Ok(())
    }

    fn incomplete(&self, lease: &Self::Lease) -> Result<Vec<JournalEntry>, JournalError> {
        self.validate(lease)?;
        Ok(self
            .entries
            .values()
            .filter(|entry| entry.state == TransactionState::Prepared)
            .cloned()
            .collect())
    }

    fn managed(
        &self,
        lease: &Self::Lease,
        feature: &SettingId,
    ) -> Result<Option<ManagedSetting>, JournalError> {
        self.validate(lease)?;
        Ok(self.managed.get(feature).cloned())
    }

    fn managed_all(&self, lease: &Self::Lease) -> Result<Vec<ManagedSetting>, JournalError> {
        self.validate(lease)?;
        Ok(self.managed.values().cloned().collect())
    }

    fn commit_apply(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        setting: ManagedSetting,
    ) -> Result<(), JournalError> {
        self.validate(lease)?;
        self.prepared_entry(transaction, TransactionIntent::Apply)?;
        if setting.last_transaction != transaction {
            return Err(JournalError(
                "managed anchor transaction does not match journal entry".into(),
            ));
        }
        let entry = self.prepared_entry(transaction, TransactionIntent::Apply)?;
        if setting.feature != entry.feature
            || setting.recipe_version != entry.recipe_version
            || setting.environment != entry.environment
            || setting.verification != entry.verification
            || setting.apply_receipt != entry.receipt
            || setting.cleanup_owned_keys != entry.cleanup_owned_keys
        {
            return Err(JournalError(
                "managed anchor does not match prepared verification evidence".into(),
            ));
        }
        self.fail_before_atomic_commit()?;
        self.managed.insert(setting.feature.clone(), setting);
        self.entry_mut(transaction)?.state = TransactionState::Committed;
        Ok(())
    }

    fn commit_restore(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError> {
        self.validate(lease)?;
        let entry = self
            .prepared_entry(transaction, TransactionIntent::Restore)?
            .clone();
        if entry.feature != *feature || self.managed.get(feature) != entry.managed_before.as_ref() {
            return Err(JournalError(
                "managed anchor changed since restore prepare".into(),
            ));
        }
        self.fail_before_atomic_commit()?;
        self.managed.remove(feature);
        self.entry_mut(transaction)?.state = TransactionState::Committed;
        Ok(())
    }

    fn mark_apply_rolled_back(
        &mut self,
        lease: &Self::Lease,
        transaction: u64,
        feature: &SettingId,
    ) -> Result<(), JournalError> {
        self.validate(lease)?;
        let entry = self
            .prepared_entry(transaction, TransactionIntent::Apply)?
            .clone();
        if entry.feature != *feature {
            return Err(JournalError(
                "feature does not match apply transaction".into(),
            ));
        }
        self.fail_before_atomic_commit()?;
        match entry.managed_before {
            Some(setting) => {
                self.managed.insert(feature.clone(), setting);
            }
            None => {
                self.managed.remove(feature);
            }
        }
        self.entry_mut(transaction)?.state = TransactionState::RolledBack;
        Ok(())
    }
}

fn same_key(left: &RegistryKey, right: &RegistryKey) -> bool {
    left.hive == right.hive
        && left.view == right.view
        && left.path.eq_ignore_ascii_case(&right.path)
}

fn merge_keys(target: &mut Vec<RegistryKey>, keys: &[RegistryKey]) {
    for key in keys {
        if !target.iter().any(|existing| same_key(existing, key)) {
            target.push(key.clone());
        }
    }
    target.sort_by_key(|key| {
        (
            key.depth(),
            key.hive,
            key.view,
            key.path.to_ascii_lowercase(),
        )
    });
}
