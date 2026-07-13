use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use deskmakeover_windows_settings_reference::{
    RawRegistryValue as ReferenceRawValue, RegistryAddress, RegistryBackend as ReferenceBackend,
    RegistryError as ReferenceError, RegistryHive as ReferenceHive, RegistryKey as ReferenceKey,
    RegistrySnapshot as ReferenceSnapshot, RegistryValueKind, RegistryView as ReferenceView,
    RegistryWriteIntent,
};
use dm_windows_settings_platform::{
    DeleteKeyOutcome, DeleteOutcome, KeyDisposition, PolicyStateProbe,
    RawRegistryValue as PlatformRawValue, ReferenceRegistryBackend,
    RegistryBackend as PlatformBackend, RegistryError as PlatformError,
    RegistryHive as PlatformHive, RegistryLocation, RegistrySnapshot as PlatformSnapshot,
    RegistryView as PlatformView,
};

#[derive(Debug)]
struct RawState {
    snapshot: PlatformSnapshot,
    reads: Vec<RegistryLocation>,
    creates: Vec<RegistryLocation>,
    create_dispositions: VecDeque<KeyDisposition>,
    writes: Vec<(RegistryLocation, PlatformRawValue)>,
    deletes: Vec<RegistryLocation>,
    key_delete_outcome: DeleteKeyOutcome,
}

#[derive(Debug)]
struct FakeRegistry(Rc<RefCell<RawState>>);

impl FakeRegistry {
    fn new(snapshot: PlatformSnapshot) -> Self {
        Self(Rc::new(RefCell::new(RawState {
            snapshot,
            reads: Vec::new(),
            creates: Vec::new(),
            create_dispositions: VecDeque::new(),
            writes: Vec::new(),
            deletes: Vec::new(),
            key_delete_outcome: DeleteKeyOutcome::Deleted,
        })))
    }

    fn state(&self) -> Rc<RefCell<RawState>> {
        Rc::clone(&self.0)
    }
}

impl PlatformBackend for FakeRegistry {
    fn read_value(&self, location: &RegistryLocation) -> Result<PlatformSnapshot, PlatformError> {
        let mut state = self.0.borrow_mut();
        state.reads.push(location.clone());
        Ok(state.snapshot.clone())
    }

    fn key_exists(&self, _location: &RegistryLocation) -> Result<bool, PlatformError> {
        Ok(self.0.borrow().snapshot.key_existed)
    }

    fn create_key(&self, location: &RegistryLocation) -> Result<KeyDisposition, PlatformError> {
        let mut state = self.0.borrow_mut();
        state.creates.push(location.clone());
        Ok(state
            .create_dispositions
            .pop_front()
            .unwrap_or(KeyDisposition::Created))
    }

    fn write_value(
        &self,
        location: &RegistryLocation,
        value: &PlatformRawValue,
    ) -> Result<(), PlatformError> {
        let mut state = self.0.borrow_mut();
        state.writes.push((location.clone(), value.clone()));
        state.snapshot = PlatformSnapshot {
            key_existed: true,
            value: Some(value.clone()),
        };
        Ok(())
    }

    fn delete_value(&self, location: &RegistryLocation) -> Result<DeleteOutcome, PlatformError> {
        let mut state = self.0.borrow_mut();
        state.deletes.push(location.clone());
        let existed = state.snapshot.value.take().is_some();
        Ok(if existed {
            DeleteOutcome::Deleted
        } else {
            DeleteOutcome::AlreadyMissing
        })
    }

    fn delete_key_if_empty(
        &self,
        _location: &RegistryLocation,
    ) -> Result<DeleteKeyOutcome, PlatformError> {
        Ok(self.0.borrow().key_delete_outcome)
    }
}

#[derive(Debug)]
struct ExplicitPolicy {
    managed: bool,
    calls: Rc<Cell<usize>>,
}

impl PolicyStateProbe for ExplicitPolicy {
    fn is_policy_managed(&self, _location: &RegistryLocation) -> Result<bool, PlatformError> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.managed)
    }
}

fn address() -> RegistryAddress {
    RegistryAddress::new(
        ReferenceHive::CurrentUser,
        ReferenceView::Registry64,
        r"Software\DeskMakeover\Bridge",
        "Enabled",
    )
}

fn make_policy(managed: bool) -> (ExplicitPolicy, Rc<Cell<usize>>) {
    let calls = Rc::new(Cell::new(0));
    (
        ExplicitPolicy {
            managed,
            calls: calls.clone(),
        },
        calls,
    )
}

#[test]
fn read_preserves_key_missing_value_missing_and_raw_value() {
    let (policy, _) = make_policy(false);
    let backend = ReferenceRegistryBackend::new(
        FakeRegistry::new(PlatformSnapshot {
            key_existed: false,
            value: None,
        }),
        policy,
    );
    assert_eq!(
        backend.read(&address()).unwrap(),
        ReferenceSnapshot::KeyMissing
    );

    let (policy, _) = make_policy(false);
    let backend = ReferenceRegistryBackend::new(
        FakeRegistry::new(PlatformSnapshot {
            key_existed: true,
            value: None,
        }),
        policy,
    );
    assert_eq!(
        backend.read(&address()).unwrap(),
        ReferenceSnapshot::ValueMissing
    );

    let (policy, _) = make_policy(false);
    let backend = ReferenceRegistryBackend::new(
        FakeRegistry::new(PlatformSnapshot {
            key_existed: true,
            value: Some(PlatformRawValue {
                kind: 37,
                bytes: vec![1, 2, 3],
            }),
        }),
        policy,
    );
    assert_eq!(
        backend.read(&address()).unwrap(),
        ReferenceSnapshot::Present(ReferenceRawValue::new(
            RegistryValueKind::Other(37),
            [1, 2, 3]
        ))
    );
}

#[test]
fn logical_compare_exchange_checks_policy_and_writes_exact_dword() {
    let (policy, policy_calls) = make_policy(false);
    let registry = FakeRegistry::new(PlatformSnapshot {
        key_existed: false,
        value: None,
    });
    let state = registry.state();
    let mut backend = ReferenceRegistryBackend::new(registry, policy);
    let outcome = backend
        .compare_exchange(
            RegistryWriteIntent::Apply,
            &address(),
            &ReferenceSnapshot::KeyMissing,
            &ReferenceSnapshot::Present(ReferenceRawValue::dword(7)),
        )
        .unwrap();
    assert_eq!(outcome.created_keys, address().key_location().prefixes());

    assert_eq!(policy_calls.get(), 1);
    let state = state.borrow();
    assert_eq!(state.reads.len(), 1);
    assert_eq!(state.writes.len(), 1);
    let (location, value) = &state.writes[0];
    assert_eq!(location.hive, PlatformHive::CurrentUser);
    assert_eq!(location.view, PlatformView::View64);
    assert_eq!(location.path, r"Software\DeskMakeover\Bridge");
    assert_eq!(location.value_name, "Enabled");
    assert_eq!(value.kind, 4);
    assert_eq!(value.bytes, 7_u32.to_le_bytes());
    assert_eq!(
        state.creates.len(),
        address().key_location().prefixes().len()
    );
}

#[test]
fn each_prefix_disposition_independently_controls_confirmed_creation() {
    let prefixes = address().key_location().prefixes();
    let registry = FakeRegistry::new(PlatformSnapshot {
        key_existed: false,
        value: None,
    });
    let state = registry.state();
    registry.0.borrow_mut().create_dispositions = prefixes
        .iter()
        .enumerate()
        .map(|(index, _)| {
            if index % 2 == 0 {
                KeyDisposition::OpenedExisting
            } else {
                KeyDisposition::Created
            }
        })
        .collect();
    let (policy, _) = make_policy(false);
    let mut backend = ReferenceRegistryBackend::new(registry, policy);

    let outcome = backend
        .compare_exchange(
            RegistryWriteIntent::Apply,
            &address(),
            &ReferenceSnapshot::KeyMissing,
            &ReferenceSnapshot::Present(ReferenceRawValue::dword(1)),
        )
        .unwrap();
    let expected_created = prefixes
        .iter()
        .enumerate()
        .filter(|(index, _)| index % 2 == 1)
        .map(|(_, key)| key.clone())
        .collect::<Vec<_>>();
    assert_eq!(outcome.created_keys, expected_created);
    assert_eq!(
        state
            .borrow()
            .creates
            .iter()
            .map(|location| location.path.as_str())
            .collect::<Vec<_>>(),
        prefixes
            .iter()
            .map(|key| key.path.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn managed_and_conflicting_values_never_write() {
    let (policy, _) = make_policy(true);
    let registry = FakeRegistry::new(PlatformSnapshot {
        key_existed: true,
        value: None,
    });
    let managed_state = registry.state();
    let mut managed = ReferenceRegistryBackend::new(registry, policy);
    assert!(matches!(
        managed.compare_exchange(
            RegistryWriteIntent::Apply,
            &address(),
            &ReferenceSnapshot::ValueMissing,
            &ReferenceSnapshot::Present(ReferenceRawValue::dword(1))
        ),
        Err(ReferenceError::ManagedByPolicy(_))
    ));
    assert!(managed_state.borrow().writes.is_empty());

    let (policy, _) = make_policy(false);
    let registry = FakeRegistry::new(PlatformSnapshot {
        key_existed: true,
        value: Some(PlatformRawValue::dword(2)),
    });
    let conflict_state = registry.state();
    let mut conflict = ReferenceRegistryBackend::new(registry, policy);
    assert!(matches!(
        conflict.compare_exchange(
            RegistryWriteIntent::Apply,
            &address(),
            &ReferenceSnapshot::Present(ReferenceRawValue::dword(1)),
            &ReferenceSnapshot::Present(ReferenceRawValue::dword(0))
        ),
        Err(ReferenceError::Conflict { .. })
    ));
    assert!(conflict_state.borrow().writes.is_empty());
}

#[test]
fn undo_intent_bypasses_policy_probe_but_apply_remains_blocked() {
    let (policy, undo_policy_calls) = make_policy(true);
    let registry = FakeRegistry::new(PlatformSnapshot {
        key_existed: true,
        value: Some(PlatformRawValue::dword(0)),
    });
    let undo_state = registry.state();
    let mut undo = ReferenceRegistryBackend::new(registry, policy);
    undo.compare_exchange(
        RegistryWriteIntent::Undo,
        &address(),
        &ReferenceSnapshot::Present(ReferenceRawValue::dword(0)),
        &ReferenceSnapshot::ValueMissing,
    )
    .unwrap();
    assert_eq!(undo_policy_calls.get(), 0);
    assert_eq!(undo_state.borrow().deletes.len(), 1);

    let (policy, apply_policy_calls) = make_policy(true);
    let registry = FakeRegistry::new(PlatformSnapshot {
        key_existed: true,
        value: None,
    });
    let apply_state = registry.state();
    let mut apply = ReferenceRegistryBackend::new(registry, policy);
    assert!(matches!(
        apply.compare_exchange(
            RegistryWriteIntent::Apply,
            &address(),
            &ReferenceSnapshot::ValueMissing,
            &ReferenceSnapshot::Present(ReferenceRawValue::dword(1)),
        ),
        Err(ReferenceError::ManagedByPolicy(_))
    ));
    assert_eq!(apply_policy_calls.get(), 1);
    assert!(apply_state.borrow().writes.is_empty());
}

#[test]
fn every_standard_win32_reg_kind_zero_through_eleven_round_trips() {
    for numeric_kind in 0..=11 {
        let bytes = vec![numeric_kind as u8, 0xa5, 0x00];
        let (policy, _) = make_policy(false);
        let reader = ReferenceRegistryBackend::new(
            FakeRegistry::new(PlatformSnapshot {
                key_existed: true,
                value: Some(PlatformRawValue {
                    kind: numeric_kind,
                    bytes: bytes.clone(),
                }),
            }),
            policy,
        );
        let reference_value = match reader.read(&address()).unwrap() {
            ReferenceSnapshot::Present(value) => value,
            snapshot => panic!("kind {numeric_kind} became {snapshot:?}"),
        };

        let (policy, _) = make_policy(false);
        let registry = FakeRegistry::new(PlatformSnapshot {
            key_existed: true,
            value: None,
        });
        let state = registry.state();
        let mut writer = ReferenceRegistryBackend::new(registry, policy);
        writer
            .compare_exchange(
                RegistryWriteIntent::Apply,
                &address(),
                &ReferenceSnapshot::ValueMissing,
                &ReferenceSnapshot::Present(reference_value),
            )
            .unwrap();
        let state = state.borrow();
        assert_eq!(state.writes[0].1.kind, numeric_kind);
        assert_eq!(state.writes[0].1.bytes, bytes);
    }
}

#[test]
fn unknown_raw_kind_restore_fails_closed_and_key_outcomes_are_exact() {
    let (policy, _) = make_policy(false);
    let mut backend = ReferenceRegistryBackend::new(
        FakeRegistry::new(PlatformSnapshot {
            key_existed: true,
            value: None,
        }),
        policy,
    );
    let result = backend.compare_exchange(
        RegistryWriteIntent::Apply,
        &address(),
        &ReferenceSnapshot::ValueMissing,
        &ReferenceSnapshot::Present(ReferenceRawValue::new(RegistryValueKind::Other(37), [1, 2])),
    );
    assert!(matches!(result, Err(ReferenceError::Io(message)) if message.contains("REG_*")));

    let key = ReferenceKey::new(
        ReferenceHive::CurrentUser,
        ReferenceView::Registry64,
        r"Software\DeskMakeover\Bridge",
    );
    assert_eq!(
        backend.delete_key_if_empty(&key).unwrap(),
        deskmakeover_windows_settings_reference::DeleteKeyOutcome::Deleted
    );
}
