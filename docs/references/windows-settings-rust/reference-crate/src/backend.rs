use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{RawRegistryValue, RegistryAddress, RegistryKey, RegistrySnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteKeyOutcome {
    Deleted,
    AlreadyMissing,
    NotEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryWriteIntent {
    Apply,
    Undo,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistryWriteOutcome {
    /// Exact prefixes this call opened with a native `Created` disposition.
    pub created_keys: Vec<RegistryKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    Io(String),
    ManagedByPolicy(RegistryAddress),
    Conflict {
        address: RegistryAddress,
        expected: Box<RegistrySnapshot>,
        actual: Box<RegistrySnapshot>,
    },
    VerificationFailed {
        address: RegistryAddress,
        desired: Box<RegistrySnapshot>,
        first: Box<RegistrySnapshot>,
        second: Box<RegistrySnapshot>,
    },
    KeyCleanupBlocked(RegistryKey),
    /// Test-only process-death signal. Production backends never return this value.
    Interrupted,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(f, "registry I/O: {message}"),
            Self::ManagedByPolicy(address) => write!(f, "managed by policy: {address}"),
            Self::Conflict { address, .. } => write!(f, "registry CAS conflict: {address}"),
            Self::VerificationFailed { address, .. } => {
                write!(f, "registry read-back verification failed: {address}")
            }
            Self::KeyCleanupBlocked(key) => {
                write!(f, "recorded app-created registry key is not empty: {key}")
            }
            Self::Interrupted => write!(f, "simulated process interruption"),
        }
    }
}

impl Error for RegistryError {}

/// Platform seam. A Windows implementation should use raw `winreg::RegValue` bytes and explicit
/// registry views. It must never accept an arbitrary path from the frontend.
pub trait RegistryBackend {
    fn read(&self, address: &RegistryAddress) -> Result<RegistrySnapshot, RegistryError>;

    fn key_exists(&self, key: &RegistryKey) -> Result<bool, RegistryError>;

    fn is_policy_managed(&self, address: &RegistryAddress) -> Result<bool, RegistryError>;

    /// Process-serialized logical CAS: re-read and compare before writing `desired`.
    ///
    /// The Windows registry has no general cross-process atomic CAS primitive. A production
    /// adapter must document the remaining compare-to-write TOCTOU window, serialize this app's
    /// writers, and rely on post-write double verification to detect a race it cannot prevent.
    fn compare_exchange(
        &mut self,
        intent: RegistryWriteIntent,
        address: &RegistryAddress,
        expected: &RegistrySnapshot,
        desired: &RegistrySnapshot,
    ) -> Result<RegistryWriteOutcome, RegistryError>;

    /// Deletes only a recorded app-created key that is still empty. Windows has no atomic
    /// delete-if-empty primitive for both values and subkeys, so concrete adapters must retain the
    /// key and return `NotEmpty` or an error whenever ownership is uncertain.
    fn delete_key_if_empty(&mut self, key: &RegistryKey)
        -> Result<DeleteKeyOutcome, RegistryError>;
}

#[derive(Debug, Clone)]
struct Replacement {
    call: usize,
    address: RegistryAddress,
    snapshot: RegistrySnapshot,
}

/// Deterministic in-memory backend used by the reference tests and non-Windows development.
#[derive(Debug, Default)]
pub struct MemoryRegistry {
    keys: BTreeSet<RegistryKey>,
    values: BTreeMap<RegistryAddress, RawRegistryValue>,
    managed: BTreeSet<RegistryAddress>,
    cas_calls: usize,
    successful_writes: usize,
    fail_on_cas: Option<usize>,
    interrupt_on_successful_write: Option<usize>,
    replacement_before_cas: Option<Replacement>,
}

impl MemoryRegistry {
    pub fn set_snapshot(&mut self, address: RegistryAddress, snapshot: RegistrySnapshot) {
        self.put(address, snapshot);
    }

    pub fn snapshot(&self, address: &RegistryAddress) -> RegistrySnapshot {
        if !self.contains_key(&address.key_location()) {
            return RegistrySnapshot::KeyMissing;
        }
        self.values
            .get(address)
            .cloned()
            .map(RegistrySnapshot::Present)
            .unwrap_or(RegistrySnapshot::ValueMissing)
    }

    pub fn ensure_key(&mut self, key: RegistryKey) {
        self.materialize_key(&key);
    }

    pub fn key_is_present(&self, key: &RegistryKey) -> bool {
        self.contains_key(key)
    }

    pub fn mark_policy_managed(&mut self, address: RegistryAddress) {
        self.managed.insert(address);
    }

    pub fn fail_compare_exchange_at(&mut self, call: usize) {
        self.fail_on_cas = Some(call);
    }

    pub fn replace_before_compare_exchange_at(
        &mut self,
        call: usize,
        address: RegistryAddress,
        snapshot: RegistrySnapshot,
    ) {
        self.replacement_before_cas = Some(Replacement {
            call,
            address,
            snapshot,
        });
    }

    pub fn interrupt_after_additional_writes(&mut self, additional: usize) {
        self.interrupt_on_successful_write = Some(self.successful_writes + additional);
    }

    pub fn clear_faults(&mut self) {
        self.fail_on_cas = None;
        self.interrupt_on_successful_write = None;
        self.replacement_before_cas = None;
    }

    pub fn successful_write_count(&self) -> usize {
        self.successful_writes
    }

    fn put(&mut self, address: RegistryAddress, snapshot: RegistrySnapshot) {
        match snapshot {
            RegistrySnapshot::KeyMissing => {
                let key = address.key_location();
                self.values
                    .retain(|candidate, _| !is_same_or_child_key(&candidate.key_location(), &key));
                self.keys
                    .retain(|candidate| !is_same_or_child_key(candidate, &key));
            }
            RegistrySnapshot::ValueMissing => {
                self.ensure_key(address.key_location());
                self.values.remove(&address);
            }
            RegistrySnapshot::Present(value) => {
                self.ensure_key(address.key_location());
                self.values.insert(address, value);
            }
        }
    }

    fn contains_key(&self, key: &RegistryKey) -> bool {
        key.is_hive_root() || self.keys.iter().any(|candidate| same_key(candidate, key))
    }

    fn materialize_key(&mut self, key: &RegistryKey) -> Vec<RegistryKey> {
        let mut created = Vec::new();
        for prefix in key.prefixes() {
            if !self.contains_key(&prefix) {
                self.keys.insert(prefix.clone());
                created.push(prefix);
            }
        }
        created
    }
}

impl RegistryBackend for MemoryRegistry {
    fn read(&self, address: &RegistryAddress) -> Result<RegistrySnapshot, RegistryError> {
        Ok(self.snapshot(address))
    }

    fn key_exists(&self, key: &RegistryKey) -> Result<bool, RegistryError> {
        Ok(self.contains_key(key))
    }

    fn is_policy_managed(&self, address: &RegistryAddress) -> Result<bool, RegistryError> {
        Ok(self.managed.contains(address))
    }

    fn compare_exchange(
        &mut self,
        intent: RegistryWriteIntent,
        address: &RegistryAddress,
        expected: &RegistrySnapshot,
        desired: &RegistrySnapshot,
    ) -> Result<RegistryWriteOutcome, RegistryError> {
        self.cas_calls += 1;
        if intent == RegistryWriteIntent::Apply && self.managed.contains(address) {
            return Err(RegistryError::ManagedByPolicy(address.clone()));
        }
        if self
            .replacement_before_cas
            .as_ref()
            .is_some_and(|replacement| replacement.call == self.cas_calls)
        {
            let replacement = self.replacement_before_cas.take().expect("checked above");
            self.put(replacement.address, replacement.snapshot);
        }
        let actual = self.snapshot(address);
        if &actual != expected {
            return Err(RegistryError::Conflict {
                address: address.clone(),
                expected: Box::new(expected.clone()),
                actual: Box::new(actual),
            });
        }
        if self.fail_on_cas == Some(self.cas_calls) {
            self.fail_on_cas = None;
            return Err(RegistryError::Io(
                "injected compare-exchange failure".into(),
            ));
        }

        let created_keys = match desired {
            RegistrySnapshot::Present(value) => {
                let created = self.materialize_key(&address.key_location());
                self.values.insert(address.clone(), value.clone());
                created
            }
            RegistrySnapshot::ValueMissing => {
                let created = self.materialize_key(&address.key_location());
                self.values.remove(address);
                created
            }
            RegistrySnapshot::KeyMissing => {
                self.values.remove(address);
                Vec::new()
            }
        };
        self.successful_writes += 1;
        if self.interrupt_on_successful_write == Some(self.successful_writes) {
            self.interrupt_on_successful_write = None;
            return Err(RegistryError::Interrupted);
        }
        Ok(RegistryWriteOutcome { created_keys })
    }

    fn delete_key_if_empty(
        &mut self,
        key: &RegistryKey,
    ) -> Result<DeleteKeyOutcome, RegistryError> {
        if key.is_hive_root() {
            return Ok(DeleteKeyOutcome::NotEmpty);
        }
        let Some(stored) = self
            .keys
            .iter()
            .find(|candidate| same_key(candidate, key))
            .cloned()
        else {
            return Ok(DeleteKeyOutcome::AlreadyMissing);
        };
        let has_value = self
            .values
            .keys()
            .any(|address| same_key(&address.key_location(), &stored));
        let has_subkey = self
            .keys
            .iter()
            .any(|candidate| is_strict_child_key(candidate, &stored));
        if has_value || has_subkey {
            return Ok(DeleteKeyOutcome::NotEmpty);
        }
        self.keys.remove(&stored);
        Ok(DeleteKeyOutcome::Deleted)
    }
}

fn same_key(left: &RegistryKey, right: &RegistryKey) -> bool {
    left.hive == right.hive
        && left.view == right.view
        && left.path.eq_ignore_ascii_case(&right.path)
}

fn is_same_or_child_key(candidate: &RegistryKey, parent: &RegistryKey) -> bool {
    same_key(candidate, parent) || is_strict_child_key(candidate, parent)
}

fn is_strict_child_key(candidate: &RegistryKey, parent: &RegistryKey) -> bool {
    if candidate.hive != parent.hive || candidate.view != parent.view {
        return false;
    }
    let candidate = candidate.path.to_ascii_lowercase();
    let mut prefix = parent.path.to_ascii_lowercase();
    prefix.push('\\');
    candidate.starts_with(&prefix)
}
