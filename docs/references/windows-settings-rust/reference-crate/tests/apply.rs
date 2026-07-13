mod common;

use common::*;
use deskmakeover_windows_settings_reference::*;

#[test]
fn missing_key_and_every_recorded_prefix_are_deleted_on_exact_restore() {
    let feature = "searchHighlights";
    let target = address("SearchHighlights");
    let mut engine = engine(
        definition(feature, vec![("SearchHighlights", dword(0))]),
        standard_rule(),
        MemoryRegistry::default(),
    );

    engine.apply(apply_request(&engine, feature)).unwrap();
    assert_eq!(engine.fake_registry().snapshot(&target), dword(0));
    let managed = engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .unwrap();
    assert_eq!(
        managed.verification,
        VerificationPlan::new(EffectVerifier::DelayedReadBackAndSettingsUi)
    );
    let created_keys = managed.cleanup_owned_keys.clone();
    assert!(!created_keys.is_empty());

    engine.restore(restore_request(&engine, feature)).unwrap();
    assert_eq!(
        engine.fake_registry().snapshot(&target),
        RegistrySnapshot::KeyMissing
    );
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_none());
    assert!(created_keys
        .iter()
        .all(|key| !engine.fake_registry().key_is_present(key)));
    assert_eq!(
        engine
            .fake_verifier()
            .effect_invocations()
            .iter()
            .map(|invocation| invocation.phase)
            .collect::<Vec<_>>(),
        vec![
            VerificationPhase::ApplyDesired,
            VerificationPhase::RestoreOriginal
        ]
    );
}

#[test]
fn effect_failure_prevents_commit_and_fully_rolls_back() {
    let feature = "advertisingPersonalization";
    let first = address("AdvertisingId");
    let second = address("Enabled");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(first.clone(), dword(9));
    backend.set_snapshot(second.clone(), dword(1));
    let mut recipe = definition(
        feature,
        vec![("AdvertisingId", dword(0)), ("Enabled", dword(0))],
    );
    recipe.verification = VerificationPlan::new(EffectVerifier::AdvertisingIdIsEmpty);
    let plan = recipe.verification.clone();
    let mut engine = engine(recipe, standard_rule(), backend);
    engine
        .fake_verifier_mut()
        .fail_next_effect("AdvertisingManager still returned an ID");

    let error = engine.apply(apply_request(&engine, feature)).unwrap_err();
    assert!(matches!(
        error,
        EngineError::ApplyFailed {
            rollback_complete: true,
            ..
        }
    ));
    assert_eq!(engine.fake_registry().snapshot(&first), dword(9));
    assert_eq!(engine.fake_registry().snapshot(&second), dword(1));
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_none());
    let entry = engine.fake_journal().entry(1).unwrap();
    assert_eq!(entry.state, TransactionState::RolledBack);
    assert_eq!(entry.verification, plan);
    assert_eq!(
        engine
            .fake_verifier()
            .effect_invocations()
            .iter()
            .map(|invocation| invocation.phase)
            .collect::<Vec<_>>(),
        vec![
            VerificationPhase::ApplyDesired,
            VerificationPhase::ApplyRollback
        ]
    );
}

#[test]
fn delayed_registry_replacement_is_detected_before_effect_and_commit() {
    let feature = "taskbarWidgets";
    let target = address("TaskbarDa");
    let original = dword(1);
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), original.clone());
    let mut engine = engine(
        definition(feature, vec![("TaskbarDa", dword(0))]),
        standard_rule(),
        backend,
    );
    engine
        .fake_verifier_mut()
        .replace_registry_on_next_settle(target.clone(), original.clone());

    let error = engine.apply(apply_request(&engine, feature)).unwrap_err();
    assert!(matches!(
        error,
        EngineError::ApplyFailed {
            rollback_complete: true,
            ref cause,
            ..
        } if cause.contains("delayed registry verification failed")
    ));
    assert_eq!(engine.fake_registry().snapshot(&target), original);
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_none());
    assert_eq!(
        engine.fake_journal().entry(1).unwrap().state,
        TransactionState::RolledBack
    );
    assert_eq!(
        engine
            .fake_verifier()
            .effect_invocations()
            .iter()
            .map(|invocation| invocation.phase)
            .collect::<Vec<_>>(),
        vec![VerificationPhase::ApplyRollback]
    );
}

#[test]
fn present_value_restores_exact_raw_kind_and_bytes() {
    let feature = "lockScreenTips";
    let target = address("TipsPayload");
    let original = RegistrySnapshot::Present(RawRegistryValue::new(
        RegistryValueKind::Binary,
        [0, 255, 17, 42, 0, 9],
    ));
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), original.clone());
    let mut recipe = definition(feature, vec![("TipsPayload", dword(0))]);
    recipe.mutations[0]
        .accepted_existing_kinds
        .push(RegistryValueKind::Binary);
    let mut engine = engine(recipe, standard_rule(), backend);

    engine.apply(apply_request(&engine, feature)).unwrap();
    engine.restore(restore_request(&engine, feature)).unwrap();

    assert_eq!(engine.fake_registry().snapshot(&target), original);
}

#[test]
fn unexpected_raw_type_fails_closed_without_a_write() {
    let feature = "widgetsNews";
    let target = address("WidgetsNews");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), string_raw(&[65, 0, 0, 0]));
    let mut engine = engine(
        definition(feature, vec![("WidgetsNews", dword(0))]),
        standard_rule(),
        backend,
    );

    let inspection = engine.inspect(&id(feature)).unwrap();
    assert!(matches!(
        inspection.capability,
        Capability::Unavailable(UnavailableReason::UnexpectedRegistryType { .. })
    ));
    let request = inspection.apply_request();
    assert!(matches!(
        engine.apply(request),
        Err(EngineError::Unavailable(
            UnavailableReason::UnexpectedRegistryType { .. }
        ))
    ));
    assert_eq!(engine.fake_registry().successful_write_count(), 0);
    assert_eq!(
        engine.fake_registry().snapshot(&target),
        string_raw(&[65, 0, 0, 0])
    );
}

#[test]
fn policy_managed_value_is_inspectable_but_never_written() {
    let feature = "notificationSuggestions";
    let target = address("Suggestions");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    backend.mark_policy_managed(target.clone());
    let error = FirstBatchCatalog::try_new([FirstBatchSetting {
        id: id(feature),
        recipe_version: 1,
        tier: FirstBatchTier::AutomaticCandidate,
        evidence: EvidenceLevel::MicrosoftImplementation,
        applicability: Applicability::AnyCertifiedEnvironment,
        mutations: definition(feature, vec![("Suggestions", dword(0))]).mutations,
        auxiliary_mutations: Vec::new(),
        policy_guards: Vec::new(),
        forbidden_mutations: Vec::new(),
        manual_fallback: None,
        effect_verifier: Some(EffectVerifier::DelayedReadBackAndSettingsUi),
        notes: Vec::new(),
    }])
    .unwrap()
    .resolve(
        &id(feature),
        &VerificationManifest::new([(id(feature), standard_rule())]),
        &runtime_facts(),
        &backend,
    )
    .unwrap_err();

    assert!(matches!(error, FirstBatchPlanError::Managed { .. }));
    assert_eq!(backend.successful_write_count(), 0);
    assert_eq!(backend.snapshot(&target), dword(1));
}

#[test]
fn multi_key_apply_failure_rolls_every_written_value_back() {
    let feature = "widgetsSurface";
    let first = address("News");
    let second = address("Badges");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(first.clone(), dword(1));
    backend.set_snapshot(second.clone(), dword(1));
    backend.fail_compare_exchange_at(2);
    let mut engine = engine(
        definition(feature, vec![("News", dword(0)), ("Badges", dword(0))]),
        standard_rule(),
        backend,
    );

    let error = engine.apply(apply_request(&engine, feature)).unwrap_err();
    assert!(matches!(
        error,
        EngineError::ApplyFailed {
            rollback_complete: true,
            ..
        }
    ));
    assert_eq!(engine.fake_registry().snapshot(&first), dword(1));
    assert_eq!(engine.fake_registry().snapshot(&second), dword(1));
    assert_eq!(engine.fake_journal().prepared_count(), 0);
}

#[test]
fn per_value_cas_never_overwrites_a_concurrent_external_change() {
    let feature = "explorerNotifications";
    let target = address("ShowSyncProviderNotifications");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    let mut engine = engine(
        definition(feature, vec![("ShowSyncProviderNotifications", dword(0))]),
        standard_rule(),
        backend,
    );
    let request = apply_request(&engine, feature);
    engine
        .fake_registry_mut()
        .replace_before_compare_exchange_at(1, target.clone(), dword(7));

    let error = engine.apply(request).unwrap_err();
    assert!(matches!(
        error,
        EngineError::ApplyFailed {
            rollback_complete: false,
            ..
        }
    ));
    assert_eq!(engine.fake_registry().snapshot(&target), dword(7));
    assert_eq!(engine.fake_journal().prepared_count(), 1);
}

#[test]
fn manual_only_feature_cannot_reach_the_registry() {
    let feature = "manualSpotlightCleanup";
    let engine = SettingsEngine::restore_only(
        MemoryRegistry::default(),
        MemoryJournal::default(),
        MemoryVerificationBackend::new(),
        MemoryRuntimeProbe::new(runtime_facts()),
    );

    assert!(matches!(
        engine.inspect(&id(feature)),
        Err(EngineError::MissingDefinition(_))
    ));
    // No inspection means no constructible ApplyRequest for this manual-only feature.
    assert_eq!(engine.fake_registry().successful_write_count(), 0);
    assert_eq!(engine.fake_journal().prepared_count(), 0);
}

#[test]
fn applying_the_same_managed_recipe_is_a_no_op() {
    let feature = "taskbarSearch";
    let target = address("SearchboxTaskbarMode");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target, dword(1));
    let mut engine = engine(
        definition(feature, vec![("SearchboxTaskbarMode", dword(0))]),
        standard_rule(),
        backend,
    );

    let first = engine.apply(apply_request(&engine, feature)).unwrap();
    let writes = engine.fake_registry().successful_write_count();
    let second = engine.apply(apply_request(&engine, feature)).unwrap();

    assert_eq!(second, first);
    assert_eq!(engine.fake_registry().successful_write_count(), writes);
    assert_eq!(engine.fake_journal().prepared_count(), 0);
}

#[test]
fn recipe_version_mismatch_blocks_reapply_but_not_exact_restore() {
    let feature = "notificationSuggestions";
    let target = address("Suggestions");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    let mut first = engine(
        definition(feature, vec![("Suggestions", dword(0))]),
        standard_rule(),
        backend,
    );
    first.apply(apply_request(&first, feature)).unwrap();
    let (backend, _, journal, verifier, runtime) = first.into_fake_parts();

    let mut changed = definition(feature, vec![("Suggestions", dword(0))]);
    changed.recipe_version = 2;
    let resolved = resolve(changed, standard_rule(), &backend);
    let mut second = SettingsEngine::new(resolved, backend, journal, verifier, runtime);
    assert!(matches!(
        second.inspect(&id(feature)).unwrap().capability,
        Capability::Unavailable(UnavailableReason::RecipeVersionMismatch {
            managed: 1,
            selected: 2
        })
    ));
    let restore = second.inspect_restore(&id(feature)).unwrap();
    second
        .restore(RestoreRequest {
            feature: id(feature),
            expected: restore.expected_values(),
        })
        .unwrap();
    assert_eq!(second.fake_registry().snapshot(&target), dword(1));
}

#[test]
fn effect_plan_change_without_recipe_version_bump_blocks_reapply() {
    let feature = "notificationSuggestions";
    let mut first = engine(
        definition(feature, vec![("Suggestions", dword(0))]),
        standard_rule(),
        MemoryRegistry::default(),
    );
    first.apply(apply_request(&first, feature)).unwrap();
    let (backend, _, journal, verifier, runtime) = first.into_fake_parts();

    let mut changed = definition(feature, vec![("Suggestions", dword(0))]);
    changed.verification = VerificationPlan::new(EffectVerifier::AdvertisingIdIsEmpty);
    let resolved = resolve(changed, standard_rule(), &backend);
    let second = SettingsEngine::new(resolved, backend, journal, verifier, runtime);

    assert_eq!(
        second.inspect(&id(feature)).unwrap().capability,
        Capability::Unavailable(UnavailableReason::RecipeChangedWithoutVersionBump)
    );
}

#[test]
fn manual_only_after_upgrade_still_exposes_restore_values() {
    let feature = "taskbarWidgets";
    let target = address("TaskbarDa");
    let mut first = engine(
        definition(feature, vec![("TaskbarDa", dword(0))]),
        standard_rule(),
        MemoryRegistry::default(),
    );
    first.apply(apply_request(&first, feature)).unwrap();
    let (backend, _, journal, verifier, runtime) = first.into_fake_parts();
    let mut second = SettingsEngine::restore_only(backend, journal, verifier, runtime);

    assert!(matches!(
        second.inspect(&id(feature)),
        Err(EngineError::MissingDefinition(_))
    ));
    let restore = second.inspect_restore(&id(feature)).unwrap();
    assert_eq!(restore.values.len(), 1);
    second
        .restore(RestoreRequest {
            feature: id(feature),
            expected: restore.expected_values(),
        })
        .unwrap();
    assert_eq!(
        second.fake_registry().snapshot(&target),
        RegistrySnapshot::KeyMissing
    );
}
