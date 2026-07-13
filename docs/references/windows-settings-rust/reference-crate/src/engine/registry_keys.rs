use std::collections::BTreeMap;

use crate::{
    DeleteKeyOutcome, JournalStore, ManagedSetting, RegistryBackend, RegistryError, RegistryHive,
    RegistryKey, RegistrySnapshot, RegistryView, RuntimeProbe, SettingId, TransactionValue,
};

use super::{SettingsEngine, VerificationBackend};

impl<B, J, V, R> SettingsEngine<B, J, V, R>
where
    B: RegistryBackend,
    J: JournalStore,
    V: VerificationBackend<B>,
    R: RuntimeProbe,
{
    /// Pre-WAL candidates only. Cleanup authority requires a later native `Created` confirmation.
    pub(super) fn missing_key_prefixes(
        &self,
        values: &[TransactionValue],
    ) -> Result<Vec<RegistryKey>, RegistryError> {
        let mut missing = BTreeMap::<CanonicalKey, RegistryKey>::new();
        for value in values {
            for prefix in value.address.key_location().prefixes() {
                if !self.backend.key_exists(&prefix)? {
                    missing.entry(CanonicalKey::from(&prefix)).or_insert(prefix);
                }
            }
        }
        let mut keys = missing.into_values().collect::<Vec<_>>();
        keys.sort_by_key(|key| (key.depth(), CanonicalKey::from(key)));
        Ok(keys)
    }

    pub(super) fn cleanup_owned_keys(
        &mut self,
        cleanup_keys: &[RegistryKey],
        retained_by_other_owners: &[RegistryKey],
    ) -> Result<Vec<RegistryKey>, RegistryError> {
        let mut keys = cleanup_keys.to_vec();
        keys.sort_by_key(|key| (std::cmp::Reverse(key.depth()), CanonicalKey::from(key)));
        let mut retained = Vec::new();
        for key in keys {
            if contains_key(retained_by_other_owners, &key) {
                retained.push(key);
                continue;
            }
            match self.backend.delete_key_if_empty(&key)? {
                DeleteKeyOutcome::Deleted | DeleteKeyOutcome::AlreadyMissing => {}
                DeleteKeyOutcome::NotEmpty => {
                    return Err(RegistryError::KeyCleanupBlocked(key));
                }
            }
        }
        Ok(retained)
    }

    pub(super) fn existing_keys(
        &self,
        candidates: &[RegistryKey],
    ) -> Result<Vec<RegistryKey>, RegistryError> {
        candidates
            .iter()
            .filter_map(|key| match self.backend.key_exists(key) {
                Ok(true) => Some(Ok(key.clone())),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }
}

pub(super) fn inherited_cleanup_keys(
    values: &[TransactionValue],
    managed: &[ManagedSetting],
) -> Vec<RegistryKey> {
    let mutation_keys = values
        .iter()
        .map(|value| value.address.key_location())
        .collect::<Vec<_>>();
    normalized_keys(managed.iter().flat_map(|setting| {
        setting
            .cleanup_owned_keys
            .iter()
            .filter(|owned| {
                mutation_keys
                    .iter()
                    .any(|mutation| is_same_or_parent(owned, mutation))
            })
            .cloned()
    }))
}

pub(super) fn owned_by_other_features(
    feature: &SettingId,
    managed: &[ManagedSetting],
) -> Vec<RegistryKey> {
    normalized_keys(
        managed
            .iter()
            .filter(|setting| setting.feature != *feature)
            .flat_map(|setting| setting.cleanup_owned_keys.iter().cloned()),
    )
}

pub(super) fn unconfirmed_candidates(
    candidates: &[RegistryKey],
    confirmed: &[RegistryKey],
) -> Vec<RegistryKey> {
    candidates
        .iter()
        .filter(|candidate| !contains_key(confirmed, candidate))
        .cloned()
        .collect()
}

pub(super) fn contains_key(keys: &[RegistryKey], expected: &RegistryKey) -> bool {
    keys.iter().any(|key| same_key(key, expected))
}

pub(super) fn merge_cleanup_keys(target: &mut Vec<RegistryKey>, keys: &[RegistryKey]) {
    target.extend(keys.iter().cloned());
    *target = normalized_keys(std::mem::take(target));
}

pub(super) fn leaf_restore_target(snapshot: &RegistrySnapshot) -> RegistrySnapshot {
    match snapshot {
        RegistrySnapshot::KeyMissing => RegistrySnapshot::ValueMissing,
        RegistrySnapshot::ValueMissing | RegistrySnapshot::Present(_) => snapshot.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalKey {
    hive: RegistryHive,
    view: RegistryView,
    path: String,
}

fn normalized_keys(keys: impl IntoIterator<Item = RegistryKey>) -> Vec<RegistryKey> {
    let mut normalized = BTreeMap::<CanonicalKey, RegistryKey>::new();
    for key in keys {
        normalized.entry(CanonicalKey::from(&key)).or_insert(key);
    }
    let mut values = normalized.into_values().collect::<Vec<_>>();
    values.sort_by_key(|key| (key.depth(), CanonicalKey::from(key)));
    values
}

fn same_key(left: &RegistryKey, right: &RegistryKey) -> bool {
    CanonicalKey::from(left) == CanonicalKey::from(right)
}

fn is_same_or_parent(candidate: &RegistryKey, child: &RegistryKey) -> bool {
    if candidate.hive != child.hive || candidate.view != child.view {
        return false;
    }
    let candidate = candidate.path.to_ascii_lowercase();
    let child = child.path.to_ascii_lowercase();
    child == candidate || child.starts_with(&format!(r"{candidate}\"))
}

impl From<&RegistryKey> for CanonicalKey {
    fn from(key: &RegistryKey) -> Self {
        Self {
            hive: key.hive,
            view: key.view,
            path: key.path.to_ascii_lowercase(),
        }
    }
}
