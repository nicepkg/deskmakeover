mod common;

use common::*;
use deskmakeover_windows_settings_reference::*;

const EXPLORER_ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const DEVICE_USAGE_ROOT: &str =
    r"Software\Microsoft\Windows\CurrentVersion\CloudExperienceHost\Intent";
const DEVICE_USAGE_CATEGORIES: [&str; 7] = [
    "creative",
    "business",
    "developer",
    "entertainment",
    "family",
    "gaming",
    "schoolwork",
];

#[test]
fn memory_writer_lease_rejects_concurrency_and_foreign_owner_then_releases_on_drop() {
    let first = MemoryJournal::default();
    let second = MemoryJournal::default();
    let lease = first.acquire_writer_lease().unwrap();

    assert!(first.acquire_writer_lease().is_err());
    let foreign = second.incomplete(&lease).unwrap_err();
    assert!(foreign.0.contains("different journal"));

    drop(lease);
    let replacement = first.acquire_writer_lease().unwrap();
    assert!(first.incomplete(&replacement).unwrap().is_empty());
}

#[test]
fn engine_acquires_lease_before_probe_and_interrupted_apply_releases_it_to_recovery_barrier() {
    let feature = "taskbarSearch";
    let mut engine = engine(
        definition(feature, vec![("SearchboxTaskbarMode", dword(0))]),
        standard_rule(),
        MemoryRegistry::default(),
    );
    let request = apply_request(&engine, feature);
    let held = engine.fake_journal().acquire_writer_lease().unwrap();
    engine
        .fake_runtime()
        .fail_next("probe must happen after lease");

    assert!(matches!(
        engine.apply(request.clone()),
        Err(EngineError::Journal(_))
    ));
    drop(held);
    assert!(matches!(
        engine.apply(request.clone()),
        Err(EngineError::RuntimeProbe(_))
    ));

    engine
        .fake_registry_mut()
        .interrupt_after_additional_writes(1);
    assert!(matches!(
        engine.apply(request.clone()),
        Err(EngineError::Interrupted { .. })
    ));
    let released = engine.fake_journal().acquire_writer_lease().unwrap();
    drop(released);
    assert!(matches!(
        engine.apply(request),
        Err(EngineError::RecoveryRequired(_))
    ));
}

#[test]
fn public_inspections_require_their_own_consistency_lease() {
    let feature = "taskbarTaskView";
    let mut engine = engine(
        definition(feature, vec![("ShowTaskViewButton", dword(0))]),
        standard_rule(),
        MemoryRegistry::default(),
    );
    engine.apply(apply_request(&engine, feature)).unwrap();

    let held = engine.fake_journal().acquire_writer_lease().unwrap();
    assert!(matches!(
        engine.inspect(&id(feature)),
        Err(EngineError::Journal(_))
    ));
    assert!(matches!(
        engine.inspect_restore(&id(feature)),
        Err(EngineError::Journal(_))
    ));
    drop(held);
    assert!(engine.inspect(&id(feature)).is_ok());
    assert!(engine.inspect_restore(&id(feature)).is_ok());
}

#[test]
fn changed_inspection_fingerprint_fails_before_journal_or_registry_write() {
    let feature = "explorerNotifications";
    let mut engine = engine(
        definition(feature, vec![("ShowSyncProviderNotifications", dword(0))]),
        standard_rule(),
        MemoryRegistry::default(),
    );
    let request = apply_request(&engine, feature);
    let mut changed = runtime_facts();
    changed.environment.ubr += 1;
    engine.fake_runtime().set_facts(changed);

    assert!(matches!(
        engine.apply(request),
        Err(EngineError::EnvironmentFingerprintChanged)
    ));
    assert_eq!(engine.fake_registry().successful_write_count(), 0);
    assert_eq!(engine.fake_journal().prepared_count(), 0);
}

#[test]
fn apply_also_compares_request_with_the_resolved_recipe_fingerprint() {
    let feature = "taskbarSearch";
    let recipe = definition(feature, vec![("SearchboxTaskbarMode", dword(0))]);
    let first = engine(recipe.clone(), standard_rule(), MemoryRegistry::default());
    let request = apply_request(&first, feature);

    let mut resolved_runtime = runtime_facts();
    resolved_runtime.environment.build = 26_200;
    let descriptor = FirstBatchSetting {
        id: recipe.id.clone(),
        recipe_version: recipe.recipe_version,
        tier: FirstBatchTier::AutomaticCandidate,
        evidence: EvidenceLevel::MicrosoftImplementation,
        applicability: Applicability::AnyCertifiedEnvironment,
        mutations: recipe.mutations,
        auxiliary_mutations: Vec::new(),
        policy_guards: Vec::new(),
        forbidden_mutations: Vec::new(),
        manual_fallback: None,
        effect_verifier: Some(recipe.verification.effect),
        notes: Vec::new(),
    };
    let registry = MemoryRegistry::default();
    let resolved = FirstBatchCatalog::try_new([descriptor])
        .unwrap()
        .resolve(
            &id(feature),
            &VerificationManifest::new([(id(feature), standard_rule())]),
            &resolved_runtime,
            &registry,
        )
        .unwrap();
    let mut second = SettingsEngine::new(
        resolved,
        registry,
        MemoryJournal::default(),
        MemoryVerificationBackend::new(),
        MemoryRuntimeProbe::new(runtime_facts()),
    );

    assert!(matches!(
        second.apply(request),
        Err(EngineError::EnvironmentFingerprintChanged)
    ));
    assert_eq!(second.fake_registry().successful_write_count(), 0);
    assert_eq!(second.fake_journal().prepared_count(), 0);
}

#[test]
fn start_apply_and_restore_persist_distinct_valid_receipts() {
    let feature = "startPromotedRecommendations";
    let target = address_in(EXPLORER_ADVANCED, "Start_IrisRecommendations");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    let mut recipe = definition_at(feature, vec![(target.clone(), dword(0))]);
    recipe.verification =
        VerificationPlan::new(EffectVerifier::StartPromotionsAbsentAndKnownRecentPreserved);
    let mut engine = engine(recipe, standard_rule(), backend);
    engine
        .fake_verifier_mut()
        .set_start_known_recent_marker("recent-before-apply");

    engine.apply(apply_request(&engine, feature)).unwrap();
    assert_eq!(
        engine.fake_journal().entry(1).unwrap().receipt,
        VerificationReceipt::StartKnownRecent {
            marker: "recent-before-apply".into()
        }
    );
    assert_eq!(
        engine
            .fake_journal()
            .managed(&id(feature))
            .unwrap()
            .unwrap()
            .apply_receipt,
        VerificationReceipt::StartKnownRecent {
            marker: "recent-before-apply".into()
        }
    );

    engine
        .fake_verifier_mut()
        .set_start_known_recent_marker("recent-before-restore");
    engine.restore(restore_request(&engine, feature)).unwrap();
    assert_eq!(
        engine.fake_journal().entry(2).unwrap().receipt,
        VerificationReceipt::StartKnownRecent {
            marker: "recent-before-restore".into()
        }
    );
    assert_eq!(engine.fake_registry().snapshot(&target), dword(1));
}

#[test]
fn device_usage_receipt_preserves_all_seven_raw_priorities_across_restore() {
    let feature = "deviceUsageRecommendations";
    let mut backend = MemoryRegistry::default();
    let mut mutations = Vec::new();
    let mut expected_priorities = Vec::new();
    for (index, category) in DEVICE_USAGE_CATEGORIES.into_iter().enumerate() {
        let key = format!(r"{DEVICE_USAGE_ROOT}\{category}");
        let intent = address_in(&key, "Intent");
        backend.set_snapshot(intent.clone(), dword(1));
        mutations.push((intent, dword(0)));

        let priority = address_in(&key, "Priority");
        let snapshot = if index % 2 == 0 {
            RegistrySnapshot::ValueMissing
        } else {
            RegistrySnapshot::Present(RawRegistryValue::new(
                RegistryValueKind::Binary,
                [index as u8, 0xff, 0x00],
            ))
        };
        backend.set_snapshot(priority.clone(), snapshot.clone());
        expected_priorities.push(ReceiptSnapshot {
            address: priority,
            snapshot,
        });
    }
    let consent_key = format!(r"{DEVICE_USAGE_ROOT}\OffDeviceConsent");
    let consent = address_in(&consent_key, "accepted");
    backend.set_snapshot(consent.clone(), dword(1));
    mutations.push((consent, dword(0)));

    let mut recipe = definition_at(feature, mutations);
    recipe.verification =
        VerificationPlan::new(EffectVerifier::DeviceUsageAllOffAndPrioritiesPreserved);
    let mut engine = engine(recipe, standard_rule(), backend);

    engine.apply(apply_request(&engine, feature)).unwrap();
    let expected_receipt = VerificationReceipt::DeviceUsagePriorities {
        priorities: expected_priorities.clone(),
    };
    assert_eq!(
        engine.fake_journal().entry(1).unwrap().receipt,
        expected_receipt
    );

    engine.restore(restore_request(&engine, feature)).unwrap();
    assert_eq!(
        engine.fake_journal().entry(2).unwrap().receipt,
        expected_receipt
    );
    for priority in expected_priorities {
        assert_eq!(
            engine.fake_registry().snapshot(&priority.address),
            priority.snapshot
        );
    }
}

#[test]
fn recovery_reuses_wal_receipt_in_unattended_bounded_context() {
    let feature = "startPromotedRecommendations";
    let target = address_in(EXPLORER_ADVANCED, "Start_IrisRecommendations");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    let mut recipe = definition_at(feature, vec![(target.clone(), dword(0))]);
    recipe.verification =
        VerificationPlan::new(EffectVerifier::StartPromotionsAbsentAndKnownRecentPreserved);
    let mut engine = engine(recipe, standard_rule(), backend);
    engine
        .fake_verifier_mut()
        .set_start_known_recent_marker("durable-recent-marker");
    engine
        .fake_registry_mut()
        .interrupt_after_additional_writes(1);

    assert!(matches!(
        engine.apply(apply_request(&engine, feature)),
        Err(EngineError::Interrupted { transaction: 1 })
    ));
    let wal_receipt = engine.fake_journal().entry(1).unwrap().receipt;
    assert_eq!(engine.fake_verifier().preparation_invocations().len(), 1);

    let report = engine.recover().unwrap();
    assert_eq!(report.recovered, vec![1]);
    assert_eq!(engine.fake_verifier().preparation_invocations().len(), 1);
    let invocation = engine.fake_verifier().effect_invocations().last().unwrap();
    assert_eq!(
        invocation.execution_mode,
        VerificationExecutionMode::UnattendedRecovery
    );
    assert_eq!(invocation.receipt, wal_receipt);
    assert!(invocation.budget.is_bounded());
    assert_eq!(invocation.budget.max_settle_millis(), 5_000);
    assert_eq!(invocation.budget.max_attempts(), 3);
    assert_eq!(engine.fake_registry().snapshot(&target), dword(1));
}

#[test]
fn invalid_typed_receipt_fails_before_wal_and_registry_write() {
    let feature = "startPromotedRecommendations";
    let target = address_in(EXPLORER_ADVANCED, "Start_IrisRecommendations");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target, dword(1));
    let mut recipe = definition_at(
        feature,
        vec![(
            address_in(EXPLORER_ADVANCED, "Start_IrisRecommendations"),
            dword(0),
        )],
    );
    recipe.verification =
        VerificationPlan::new(EffectVerifier::StartPromotionsAbsentAndKnownRecentPreserved);
    let mut engine = engine(recipe, standard_rule(), backend);
    engine
        .fake_verifier_mut()
        .set_start_known_recent_marker("  ");

    assert!(matches!(
        engine.apply(apply_request(&engine, feature)),
        Err(EngineError::InvalidVerificationReceipt(_))
    ));
    assert_eq!(engine.fake_registry().successful_write_count(), 0);
    assert_eq!(engine.fake_journal().prepared_count(), 0);
}
