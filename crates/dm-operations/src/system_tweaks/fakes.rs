//! Deterministic in-memory port fakes for the Mac devhost loop and the driver tests.
//!
//! `MemoryRegistry` implements the `RegistryBackend` logical CAS with test hooks for injecting a
//! CAS failure, a process interruption at the Nth write, and an external replacement before a CAS
//! (the same fault surface the icon `txn::fakes` expose). W1 scope is value-level: the calm first
//! batch writes only pre-existing Windows keys, so this fake pre-seeds keys present and never
//! materializes one.

use std::collections::{BTreeMap, BTreeSet};

use dm_domain::system_tweaks::{
    DeleteKeyOutcome, RawRegistryValue, RegistryAddress, RegistryBackend, RegistryError,
    RegistryKey, RegistrySnapshot, RegistryWriteIntent, RegistryWriteOutcome, SystemProfileError,
    SystemProfileProbe, WindowsEnvironment,
};

#[derive(Debug, Clone)]
struct Replacement {
    call: usize,
    address: RegistryAddress,
    snapshot: RegistrySnapshot,
}

/// A deterministic registry. Keys must be pre-seeded present; the calm recipes never create one.
#[derive(Debug, Default)]
pub struct MemoryRegistry {
    keys: BTreeSet<RegistryKey>,
    values: BTreeMap<RegistryAddress, RawRegistryValue>,
    managed: BTreeSet<RegistryAddress>,
    cas_calls: usize,
    successful_writes: usize,
    fail_on_cas: Option<usize>,
    interrupt_on_write: Option<usize>,
    replace_before_cas: Option<Replacement>,
}

impl MemoryRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed a key as present (a standard Windows key the recipes target).
    pub fn ensure_key(&mut self, key: RegistryKey) {
        self.keys.insert(key);
    }

    /// Seed a value present, materializing its key.
    pub fn set_value(&mut self, address: RegistryAddress, value: RawRegistryValue) {
        self.keys.insert(address.key_location());
        self.values.insert(address, value);
    }

    /// Mark a policy guard address present.
    pub fn mark_policy_managed(&mut self, address: RegistryAddress) {
        self.keys.insert(address.key_location());
        self.managed.insert(address);
    }

    pub fn fail_compare_exchange_at(&mut self, call: usize) {
        self.fail_on_cas = Some(call);
    }

    pub fn interrupt_after_writes(&mut self, additional: usize) {
        self.interrupt_on_write = Some(self.successful_writes + additional);
    }

    pub fn replace_before_compare_exchange_at(
        &mut self,
        call: usize,
        address: RegistryAddress,
        snapshot: RegistrySnapshot,
    ) {
        self.replace_before_cas = Some(Replacement {
            call,
            address,
            snapshot,
        });
    }

    pub fn successful_writes(&self) -> usize {
        self.successful_writes
    }

    fn snapshot(&self, address: &RegistryAddress) -> RegistrySnapshot {
        if !self.keys.contains(&address.key_location()) {
            return RegistrySnapshot::KeyMissing;
        }
        self.values
            .get(address)
            .cloned()
            .map(RegistrySnapshot::Present)
            .unwrap_or(RegistrySnapshot::ValueMissing)
    }

    fn write(&mut self, address: &RegistryAddress, desired: &RegistrySnapshot) {
        match desired {
            RegistrySnapshot::Present(value) => {
                self.keys.insert(address.key_location());
                self.values.insert(address.clone(), value.clone());
            }
            RegistrySnapshot::ValueMissing => {
                self.values.remove(address);
            }
            RegistrySnapshot::KeyMissing => {
                self.values.remove(address);
            }
        }
    }
}

impl RegistryBackend for MemoryRegistry {
    fn read(&self, address: &RegistryAddress) -> Result<RegistrySnapshot, RegistryError> {
        Ok(self.snapshot(address))
    }

    fn key_exists(&self, key: &RegistryKey) -> Result<bool, RegistryError> {
        Ok(key.is_hive_root() || self.keys.contains(key))
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
        if let Some(replacement) = self
            .replace_before_cas
            .as_ref()
            .filter(|replacement| replacement.call == self.cas_calls)
            .cloned()
        {
            self.replace_before_cas = None;
            self.write(&replacement.address, &replacement.snapshot);
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
            return Err(RegistryError::Io("injected compare-exchange failure".into()));
        }
        // The calm recipes only write pre-existing keys, so a write never creates a key.
        self.write(address, desired);
        self.successful_writes += 1;
        if self.interrupt_on_write == Some(self.successful_writes) {
            self.interrupt_on_write = None;
            return Err(RegistryError::Interrupted);
        }
        Ok(RegistryWriteOutcome::default())
    }

    fn delete_key_if_empty(
        &mut self,
        _key: &RegistryKey,
    ) -> Result<DeleteKeyOutcome, RegistryError> {
        // W1 recipes never create a key, so cleanup never has one to remove.
        Ok(DeleteKeyOutcome::AlreadyMissing)
    }
}

/// A profile probe returning a fixed environment, with a one-shot failure hook.
#[derive(Debug)]
pub struct MemoryProfileProbe {
    environment: std::cell::RefCell<WindowsEnvironment>,
    next_failure: std::cell::RefCell<Option<SystemProfileError>>,
}

impl MemoryProfileProbe {
    pub fn new(environment: WindowsEnvironment) -> Self {
        Self {
            environment: std::cell::RefCell::new(environment),
            next_failure: std::cell::RefCell::new(None),
        }
    }

    pub fn set_environment(&self, environment: WindowsEnvironment) {
        *self.environment.borrow_mut() = environment;
    }

    pub fn fail_next(&self, error: impl Into<SystemProfileError>) {
        *self.next_failure.borrow_mut() = Some(error.into());
    }
}

impl SystemProfileProbe for MemoryProfileProbe {
    fn probe(&self) -> Result<WindowsEnvironment, SystemProfileError> {
        if let Some(error) = self.next_failure.borrow_mut().take() {
            return Err(error);
        }
        Ok(self.environment.borrow().clone())
    }
}
