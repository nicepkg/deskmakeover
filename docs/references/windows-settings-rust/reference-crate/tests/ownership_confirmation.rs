mod common;

use common::*;
use deskmakeover_windows_settings_reference::*;

const OWNED_PARENT: &str = r"Software\DeskMakeoverReference";
const OWNED_PATH: &str = r"Software\DeskMakeoverReference\ConfirmedOwnership";

#[test]
fn externally_race_created_empty_key_is_never_confirmed_or_deleted() {
    let feature = "raceCreatedKey";
    let target = address_in(OWNED_PATH, "Enabled");
    let owned_key = key(OWNED_PATH);
    let mut backend = MemoryRegistry::default();
    backend.ensure_key(key(OWNED_PARENT));
    let mut engine = engine(
        definition_at(feature, vec![(target.clone(), dword(0))]),
        standard_rule(),
        backend,
    );
    let request = apply_request(&engine, feature);
    engine
        .fake_registry_mut()
        .replace_before_compare_exchange_at(1, target.clone(), RegistrySnapshot::ValueMissing);

    assert!(matches!(
        engine.apply(request),
        Err(EngineError::ApplyFailed {
            rollback_complete: true,
            ..
        })
    ));
    let entry = engine.fake_journal().entry(1).unwrap();
    assert_eq!(entry.state, TransactionState::RolledBack);
    assert!(entry.confirmed_created_keys.is_empty());
    assert!(entry.candidate_keys.contains(&owned_key));
    assert!(engine.fake_registry().key_is_present(&owned_key));
    assert_eq!(
        engine.fake_registry().snapshot(&target),
        RegistrySnapshot::ValueMissing
    );
}

#[test]
fn write_to_confirmation_failure_recovers_leaf_and_leaves_unowned_empty_key() {
    let feature = "confirmationCrash";
    let target = address_in(OWNED_PATH, "Enabled");
    let owned_key = key(OWNED_PATH);
    let mut backend = MemoryRegistry::default();
    backend.ensure_key(key(OWNED_PARENT));
    let mut engine = engine(
        definition_at(feature, vec![(target.clone(), dword(0))]),
        standard_rule(),
        backend,
    );
    engine
        .fake_journal_mut()
        .fail_next_created_key_confirmation();

    assert!(matches!(
        engine.apply(apply_request(&engine, feature)),
        Err(EngineError::Journal(_))
    ));
    let entry = engine.fake_journal().entry(1).unwrap();
    assert_eq!(entry.state, TransactionState::Prepared);
    assert!(entry.confirmed_created_keys.is_empty());
    assert!(engine.fake_registry().key_is_present(&owned_key));
    assert_eq!(engine.fake_registry().snapshot(&target), dword(0));

    let report = engine.recover().unwrap();
    assert_eq!(report.recovered, vec![1]);
    assert!(report.conflicts.is_empty());
    assert!(engine.fake_registry().key_is_present(&owned_key));
    assert_eq!(
        engine.fake_registry().snapshot(&target),
        RegistrySnapshot::ValueMissing
    );
}

#[test]
fn confirmed_first_write_allows_recovery_to_remove_created_key_after_later_crash() {
    let feature = "confirmedThenCrash";
    let first = address_in(OWNED_PATH, "First");
    let second = address_in(OWNED_PATH, "Second");
    let owned_key = key(OWNED_PATH);
    let mut backend = MemoryRegistry::default();
    backend.ensure_key(key(OWNED_PARENT));
    backend.interrupt_after_additional_writes(2);
    let mut engine = engine(
        definition_at(
            feature,
            vec![(first.clone(), dword(0)), (second.clone(), dword(0))],
        ),
        standard_rule(),
        backend,
    );

    assert!(matches!(
        engine.apply(apply_request(&engine, feature)),
        Err(EngineError::Interrupted { .. })
    ));
    let entry = engine.fake_journal().entry(1).unwrap();
    assert!(entry.confirmed_created_keys.contains(&owned_key));
    engine.fake_registry_mut().clear_faults();

    assert_eq!(engine.recover().unwrap().recovered, vec![1]);
    assert!(!engine.fake_registry().key_is_present(&owned_key));
    assert_eq!(
        engine.fake_registry().snapshot(&first),
        RegistrySnapshot::KeyMissing
    );
    assert_eq!(
        engine.fake_registry().snapshot(&second),
        RegistrySnapshot::KeyMissing
    );
}

#[test]
fn policy_added_after_apply_cannot_block_exact_restore() {
    let feature = "undoManagedValue";
    let target = address("ManagedAfterApply");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    let mut engine = engine(
        definition_at(feature, vec![(target.clone(), dword(0))]),
        standard_rule(),
        backend,
    );
    engine.apply(apply_request(&engine, feature)).unwrap();
    engine
        .fake_registry_mut()
        .mark_policy_managed(target.clone());

    engine.restore(restore_request(&engine, feature)).unwrap();
    assert_eq!(engine.fake_registry().snapshot(&target), dword(1));
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_none());
}

#[test]
fn policy_added_after_interruption_cannot_block_recovery_undo() {
    let feature = "undoManagedRecovery";
    let target = address("ManagedRecovery");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    backend.interrupt_after_additional_writes(1);
    let mut engine = engine(
        definition_at(feature, vec![(target.clone(), dword(0))]),
        standard_rule(),
        backend,
    );

    assert!(matches!(
        engine.apply(apply_request(&engine, feature)),
        Err(EngineError::Interrupted { .. })
    ));
    engine
        .fake_registry_mut()
        .mark_policy_managed(target.clone());
    assert_eq!(engine.recover().unwrap().recovered, vec![1]);
    assert_eq!(engine.fake_registry().snapshot(&target), dword(1));
}

#[test]
fn undo_io_failure_keeps_restore_prepared_until_recovery_can_retry() {
    let feature = "undoAclFailure";
    let target = address("AclFailure");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    let mut engine = engine(
        definition_at(feature, vec![(target.clone(), dword(0))]),
        standard_rule(),
        backend,
    );
    engine.apply(apply_request(&engine, feature)).unwrap();
    engine.fake_registry_mut().fail_compare_exchange_at(2);

    assert!(matches!(
        engine.restore(restore_request(&engine, feature)),
        Err(EngineError::RestorePending { .. })
    ));
    assert_eq!(engine.fake_journal().prepared_count(), 1);
    engine.fake_registry_mut().clear_faults();
    assert_eq!(engine.recover().unwrap().recovered.len(), 1);
    assert_eq!(engine.fake_registry().snapshot(&target), dword(1));
}
