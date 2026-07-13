mod common;

use common::*;
use deskmakeover_windows_settings_reference::*;

const SHARED_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const SHARED_PARENT: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer";

fn shared_recipe(feature: &str, value: &str) -> TestRecipe {
    definition_at(feature, vec![(address_in(SHARED_PATH, value), dword(0))])
}

fn switch_recipe(engine: TestEngine, recipe: TestRecipe) -> TestEngine {
    let (backend, _, journal, verifier, runtime) = engine.into_fake_parts();
    let resolved = resolve(recipe, standard_rule(), &backend);
    SettingsEngine::new(resolved, backend, journal, verifier, runtime)
}

fn apply_shared(features: &[(&str, &str)]) -> TestEngine {
    let mut backend = MemoryRegistry::default();
    backend.ensure_key(key(SHARED_PARENT));
    let (first_feature, first_value) = features[0];
    let mut engine = engine(
        shared_recipe(first_feature, first_value),
        standard_rule(),
        backend,
    );
    engine.apply(apply_request(&engine, first_feature)).unwrap();
    for &(feature, value) in &features[1..] {
        engine = switch_recipe(engine, shared_recipe(feature, value));
        engine.apply(apply_request(&engine, feature)).unwrap();
    }
    engine
}

fn assert_restores_in_order(features: &[(&str, &str)], order: &[usize]) {
    let mut engine = apply_shared(features);
    let shared_key = key(SHARED_PATH);
    for &(feature, _) in features {
        let managed = engine
            .fake_journal()
            .managed(&id(feature))
            .unwrap()
            .unwrap();
        assert!(managed
            .cleanup_owned_keys
            .iter()
            .any(|owned| owned == &shared_key));
        assert_eq!(managed.values[0].original, RegistrySnapshot::KeyMissing);
    }

    for (position, index) in order.iter().copied().enumerate() {
        let (feature, value_name) = features[index];
        engine.restore(restore_request(&engine, feature)).unwrap();
        assert!(engine
            .fake_journal()
            .managed(&id(feature))
            .unwrap()
            .is_none());
        assert_eq!(engine.fake_journal().prepared_count(), 0);
        if position + 1 == order.len() {
            assert!(!engine.fake_registry().key_is_present(&shared_key));
        } else {
            assert!(engine.fake_registry().key_is_present(&shared_key));
            assert_eq!(
                engine
                    .fake_registry()
                    .snapshot(&address_in(SHARED_PATH, value_name)),
                RegistrySnapshot::ValueMissing
            );
        }
    }
}

#[test]
fn two_owners_restore_in_either_order_and_only_the_last_deletes_the_key() {
    let features = [
        ("startPromotionsShared", "Start_IrisRecommendations"),
        ("taskViewShared", "ShowTaskViewButton"),
    ];
    assert_restores_in_order(&features, &[0, 1]);
    assert_restores_in_order(&features, &[1, 0]);
}

#[test]
fn three_owners_support_non_creation_order_restore() {
    let features = [
        ("startPromotionsShared", "Start_IrisRecommendations"),
        ("taskViewShared", "ShowTaskViewButton"),
        ("startAccountShared", "Start_AccountNotifications"),
    ];
    assert_restores_in_order(&features, &[1, 0, 2]);
}

#[test]
fn interrupted_restore_recovers_while_another_owner_retains_the_key() {
    let features = [
        ("startPromotionsShared", "Start_IrisRecommendations"),
        ("taskViewShared", "ShowTaskViewButton"),
    ];
    let mut engine = apply_shared(&features);
    let shared_key = key(SHARED_PATH);
    engine
        .fake_registry_mut()
        .interrupt_after_additional_writes(1);

    assert!(matches!(
        engine.restore(restore_request(&engine, features[0].0)),
        Err(EngineError::Interrupted { .. })
    ));
    let recovery = engine.recover().unwrap();
    assert_eq!(recovery.recovered.len(), 1);
    assert!(recovery.conflicts.is_empty());
    assert!(engine.fake_registry().key_is_present(&shared_key));
    assert!(engine
        .fake_journal()
        .managed(&id(features[0].0))
        .unwrap()
        .is_none());
    assert!(engine
        .fake_journal()
        .managed(&id(features[1].0))
        .unwrap()
        .is_some());

    engine
        .restore(restore_request(&engine, features[1].0))
        .unwrap();
    assert!(!engine.fake_registry().key_is_present(&shared_key));
}

#[test]
fn failed_apply_rolls_back_under_a_key_retained_by_another_owner() {
    let first = ("startPromotionsShared", "Start_IrisRecommendations");
    let second = ("taskViewShared", "ShowTaskViewButton");
    let mut engine = apply_shared(&[first]);
    engine = switch_recipe(engine, shared_recipe(second.0, second.1));
    engine
        .fake_verifier_mut()
        .fail_next_effect("injected shared-key apply failure");

    assert!(matches!(
        engine.apply(apply_request(&engine, second.0)),
        Err(EngineError::ApplyFailed {
            rollback_complete: true,
            ..
        })
    ));
    assert_eq!(
        engine.fake_journal().entry(2).unwrap().state,
        TransactionState::RolledBack
    );
    assert!(engine
        .fake_journal()
        .managed(&id(first.0))
        .unwrap()
        .is_some());
    assert!(engine
        .fake_journal()
        .managed(&id(second.0))
        .unwrap()
        .is_none());
    assert!(engine.fake_registry().key_is_present(&key(SHARED_PATH)));
    assert_eq!(
        engine
            .fake_registry()
            .snapshot(&address_in(SHARED_PATH, second.1)),
        RegistrySnapshot::ValueMissing
    );
}

#[test]
fn interrupted_shared_key_apply_recovers_without_deleting_the_other_owner() {
    let first = ("startPromotionsShared", "Start_IrisRecommendations");
    let second = ("taskViewShared", "ShowTaskViewButton");
    let mut engine = apply_shared(&[first]);
    engine = switch_recipe(engine, shared_recipe(second.0, second.1));
    engine
        .fake_registry_mut()
        .interrupt_after_additional_writes(1);

    assert!(matches!(
        engine.apply(apply_request(&engine, second.0)),
        Err(EngineError::Interrupted { transaction: 2 })
    ));
    let recovery = engine.recover().unwrap();
    assert_eq!(recovery.recovered, vec![2]);
    assert!(recovery.conflicts.is_empty());
    assert!(engine
        .fake_journal()
        .managed(&id(first.0))
        .unwrap()
        .is_some());
    assert!(engine
        .fake_journal()
        .managed(&id(second.0))
        .unwrap()
        .is_none());
    assert!(engine.fake_registry().key_is_present(&key(SHARED_PATH)));
    assert_eq!(
        engine
            .fake_registry()
            .snapshot(&address_in(SHARED_PATH, second.1)),
        RegistrySnapshot::ValueMissing
    );
}

#[test]
fn shared_key_apply_commit_failure_recovers_to_the_retained_owner() {
    let first = ("startPromotionsShared", "Start_IrisRecommendations");
    let second = ("taskViewShared", "ShowTaskViewButton");
    let mut engine = apply_shared(&[first]);
    engine = switch_recipe(engine, shared_recipe(second.0, second.1));
    engine.fake_journal_mut().fail_next_atomic_commit();

    assert!(matches!(
        engine.apply(apply_request(&engine, second.0)),
        Err(EngineError::Journal(_))
    ));
    assert_eq!(engine.fake_journal().prepared_count(), 1);
    let recovery = engine.recover().unwrap();
    assert_eq!(recovery.recovered, vec![2]);
    assert!(recovery.conflicts.is_empty());
    assert_eq!(engine.fake_journal().prepared_count(), 0);
    assert!(engine
        .fake_journal()
        .managed(&id(first.0))
        .unwrap()
        .is_some());
    assert!(engine
        .fake_journal()
        .managed(&id(second.0))
        .unwrap()
        .is_none());
    assert!(engine.fake_registry().key_is_present(&key(SHARED_PATH)));
    assert_eq!(
        engine
            .fake_registry()
            .snapshot(&address_in(SHARED_PATH, second.1)),
        RegistrySnapshot::ValueMissing
    );
}
