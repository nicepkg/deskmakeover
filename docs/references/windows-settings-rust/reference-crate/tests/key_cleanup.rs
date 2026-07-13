mod common;

use common::*;
use deskmakeover_windows_settings_reference::*;

#[test]
fn existing_key_with_missing_default_value_is_preserved() {
    let feature = "existingKeyMissingValue";
    let path = r"Software\DeskMakeoverReference\Existing";
    let target = address_in(path, "");
    let target_key = key(path);
    let mut backend = MemoryRegistry::default();
    backend.ensure_key(target_key.clone());
    assert_eq!(backend.snapshot(&target), RegistrySnapshot::ValueMissing);

    let mut engine = engine(
        definition_at(feature, vec![(target.clone(), dword(0))]),
        standard_rule(),
        backend,
    );
    engine.apply(apply_request(&engine, feature)).unwrap();
    let managed = engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .unwrap();
    assert!(managed.cleanup_owned_keys.is_empty());

    engine.restore(restore_request(&engine, feature)).unwrap();
    assert!(engine.fake_registry().key_is_present(&target_key));
    assert_eq!(
        engine.fake_registry().snapshot(&target),
        RegistrySnapshot::ValueMissing
    );
}

#[test]
fn multilevel_shared_prefixes_are_walled_once_and_removed_deepest_first() {
    let feature = "sharedCreatedPrefixes";
    let root_path = r"Software\DeskMakeoverReference\ExistingRoot";
    let shared_path = format!(r"{root_path}\Owned");
    let first_path = format!(r"{shared_path}\First");
    let second_path = format!(r"{shared_path}\Second");
    let first = address_in(&first_path, "Enabled");
    let second = address_in(&second_path, "Enabled");
    let root = key(root_path);
    let shared = key(&shared_path);
    let first_key = key(&first_path);
    let second_key = key(&second_path);
    let mut backend = MemoryRegistry::default();
    backend.ensure_key(root.clone());

    let mut engine = engine(
        definition_at(feature, vec![(first, dword(0)), (second, dword(0))]),
        standard_rule(),
        backend,
    );
    engine.apply(apply_request(&engine, feature)).unwrap();
    let managed = engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .unwrap();
    assert_eq!(
        managed.cleanup_owned_keys,
        vec![shared.clone(), first_key.clone(), second_key.clone()]
    );
    assert_eq!(
        engine
            .fake_journal()
            .entry(1)
            .unwrap()
            .confirmed_created_keys,
        managed.cleanup_owned_keys
    );

    engine.restore(restore_request(&engine, feature)).unwrap();
    assert!(engine.fake_registry().key_is_present(&root));
    for created in [shared, first_key, second_key] {
        assert!(!engine.fake_registry().key_is_present(&created));
    }
}

#[test]
fn external_value_and_subkey_block_cleanup_without_recursive_deletion() {
    let feature = "externalCleanupConflict";
    let root_path = r"Software\DeskMakeoverReference\ExistingRoot";
    let owned_path = format!(r"{root_path}\Owned");
    let child_path = format!(r"{owned_path}\ExternalChild");
    let target = address_in(&owned_path, "DeskMakeoverValue");
    let external_value = address_in(&owned_path, "ExternalValue");
    let child_value = address_in(&child_path, "ExternalChildValue");
    let owned_key = key(&owned_path);
    let child_key = key(&child_path);
    let mut backend = MemoryRegistry::default();
    backend.ensure_key(key(root_path));
    let mut engine = engine(
        definition_at(feature, vec![(target.clone(), dword(0))]),
        standard_rule(),
        backend,
    );
    engine.apply(apply_request(&engine, feature)).unwrap();
    engine
        .fake_registry_mut()
        .set_snapshot(external_value.clone(), dword(7));
    engine
        .fake_registry_mut()
        .set_snapshot(child_value.clone(), dword(8));

    let error = engine
        .restore(restore_request(&engine, feature))
        .unwrap_err();
    assert!(matches!(
        error,
        EngineError::RestorePending { ref cause, .. }
            if cause.contains("recorded app-created registry key is not empty")
    ));
    assert_eq!(
        engine.fake_registry().snapshot(&target),
        RegistrySnapshot::ValueMissing
    );
    assert_eq!(engine.fake_registry().snapshot(&external_value), dword(7));
    assert_eq!(engine.fake_registry().snapshot(&child_value), dword(8));
    assert!(engine.fake_registry().key_is_present(&owned_key));
    assert!(engine.fake_registry().key_is_present(&child_key));
    assert_eq!(engine.fake_journal().prepared_count(), 1);
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_some());

    let recovery = engine.recover().unwrap();
    assert!(recovery.recovered.is_empty());
    assert_eq!(recovery.conflicts.len(), 1);
    assert_eq!(engine.fake_registry().snapshot(&external_value), dword(7));
    assert_eq!(engine.fake_registry().snapshot(&child_value), dword(8));
    assert_eq!(engine.fake_journal().prepared_count(), 1);
}
