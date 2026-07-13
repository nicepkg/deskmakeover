mod common;

use common::*;
use deskmakeover_windows_settings_reference::*;

fn mutation(address: RegistryAddress, missing_policy: MissingPolicy) -> SettingMutation {
    SettingMutation {
        address,
        desired: dword(0),
        accepted_existing_kinds: vec![RegistryValueKind::Dword],
        missing_policy,
    }
}

fn descriptor(feature: &str, mutations: Vec<SettingMutation>) -> FirstBatchSetting {
    FirstBatchSetting {
        id: id(feature),
        recipe_version: 1,
        tier: FirstBatchTier::AutomaticCandidate,
        evidence: EvidenceLevel::MicrosoftImplementation,
        applicability: Applicability::AnyCertifiedEnvironment,
        mutations,
        auxiliary_mutations: Vec::new(),
        policy_guards: Vec::new(),
        forbidden_mutations: Vec::new(),
        manual_fallback: None,
        effect_verifier: Some(EffectVerifier::DelayedReadBackAndSettingsUi),
        notes: Vec::new(),
    }
}

fn resolved(
    descriptor: FirstBatchSetting,
    rule: VerificationRule,
    facts: &RuntimeFacts,
    backend: &MemoryRegistry,
) -> ResolvedRecipe {
    let feature = descriptor.id.clone();
    FirstBatchCatalog::try_new([descriptor])
        .unwrap()
        .resolve(
            &feature,
            &VerificationManifest::new([(feature.clone(), rule)]),
            facts,
            backend,
        )
        .unwrap()
}

fn writable_engine(
    plan: ResolvedRecipe,
    backend: MemoryRegistry,
    facts: RuntimeFacts,
) -> TestEngine {
    SettingsEngine::new(
        plan,
        backend,
        MemoryJournal::default(),
        MemoryVerificationBackend::new(),
        MemoryRuntimeProbe::new(facts),
    )
}

#[test]
fn resolved_recipe_runs_end_to_end_without_a_bare_catalog() {
    let feature = "typedEndToEnd";
    let target = address("Enabled");
    let backend = MemoryRegistry::default();
    let facts = runtime_facts();
    let plan = resolved(
        descriptor(
            feature,
            vec![mutation(target.clone(), MissingPolicy::CreateAllowed)],
        ),
        standard_rule(),
        &facts,
        &backend,
    );
    let mut engine = writable_engine(plan, backend, facts);

    engine.apply(apply_request(&engine, feature)).unwrap();

    assert_eq!(engine.fake_registry().snapshot(&target), dword(0));
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_some());
}

#[test]
fn policy_guard_appearing_after_resolve_blocks_before_any_write() {
    let feature = "policyBeforeApply";
    let target = address("Enabled");
    let guard = address_in(r"Software\Policies\DeskMakeover", "Managed");
    let mut item = descriptor(
        feature,
        vec![mutation(target.clone(), MissingPolicy::CreateAllowed)],
    );
    item.policy_guards.push(PolicyGuard {
        address: guard.clone(),
        note: "test guard",
    });
    let backend = MemoryRegistry::default();
    let facts = runtime_facts();
    let plan = resolved(item, standard_rule(), &facts, &backend);
    let mut engine = writable_engine(plan, backend, facts);
    engine.fake_registry_mut().set_snapshot(guard, dword(1));

    assert!(matches!(
        engine.inspect(&id(feature)),
        Err(EngineError::Unavailable(UnavailableReason::PolicyManaged(
            _
        )))
    ));
    assert_eq!(engine.fake_registry().successful_write_count(), 0);
    assert_eq!(
        engine.fake_registry().snapshot(&target),
        RegistrySnapshot::KeyMissing
    );
}

#[test]
fn policy_guard_appearing_during_a_write_forces_full_rollback() {
    let feature = "policyDuringApply";
    let first = address("First");
    let second = address("Second");
    let guard = address_in(r"Software\Policies\DeskMakeover", "Managed");
    let mut item = descriptor(
        feature,
        vec![
            mutation(first.clone(), MissingPolicy::CreateAllowed),
            mutation(second.clone(), MissingPolicy::CreateAllowed),
        ],
    );
    item.policy_guards.push(PolicyGuard {
        address: guard.clone(),
        note: "test guard",
    });
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(first.clone(), dword(1));
    backend.set_snapshot(second.clone(), dword(1));
    let facts = runtime_facts();
    let plan = resolved(item, standard_rule(), &facts, &backend);
    let mut engine = writable_engine(plan, backend, facts);
    let request = apply_request(&engine, feature);
    engine
        .fake_registry_mut()
        .replace_before_compare_exchange_at(1, guard, dword(1));

    assert!(matches!(
        engine.apply(request),
        Err(EngineError::ApplyFailed {
            rollback_complete: true,
            ..
        })
    ));
    assert_eq!(engine.fake_registry().snapshot(&first), dword(1));
    assert_eq!(engine.fake_registry().snapshot(&second), dword(1));
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_none());
}

fn auxiliary_plan(backend: &MemoryRegistry, facts: &RuntimeFacts) -> ResolvedRecipe {
    let feature = "existingOnlyAuxiliary";
    let mut item = descriptor(
        feature,
        vec![mutation(address("Primary"), MissingPolicy::CreateAllowed)],
    );
    item.auxiliary_mutations.push(AuxiliaryMutation {
        mutation: mutation(
            address_in(r"Software\DeskMakeoverAux", "Companion"),
            MissingPolicy::MustAlreadyExist,
        ),
        condition: AuxiliaryCondition::IfPresentAndExactEnvironmentVerified,
        exact_environment_allowlist: vec![facts.environment.clone()],
        note: "existing only",
    });
    resolved(item, standard_rule(), facts, backend)
}

#[test]
fn selected_auxiliary_disappearing_before_inspect_is_never_created() {
    for disappeared in [RegistrySnapshot::KeyMissing, RegistrySnapshot::ValueMissing] {
        let primary = address("Primary");
        let auxiliary = address_in(r"Software\DeskMakeoverAux", "Companion");
        let mut backend = MemoryRegistry::default();
        backend.set_snapshot(primary, dword(1));
        backend.set_snapshot(auxiliary.clone(), dword(1));
        let facts = runtime_facts();
        let plan = auxiliary_plan(&backend, &facts);
        backend.set_snapshot(auxiliary.clone(), disappeared.clone());
        let engine = writable_engine(plan, backend, facts);

        assert!(matches!(
            engine.inspect(&id("existingOnlyAuxiliary")),
            Err(EngineError::RequiredValueMissing(_))
        ));
        assert_eq!(engine.fake_registry().snapshot(&auxiliary), disappeared);
        assert_eq!(engine.fake_registry().successful_write_count(), 0);
    }
}

#[test]
fn selected_auxiliary_disappearing_at_cas_is_never_created() {
    for disappeared in [RegistrySnapshot::KeyMissing, RegistrySnapshot::ValueMissing] {
        let primary = address("Primary");
        let auxiliary = address_in(r"Software\DeskMakeoverAux", "Companion");
        let mut backend = MemoryRegistry::default();
        backend.set_snapshot(primary.clone(), dword(1));
        backend.set_snapshot(auxiliary.clone(), dword(1));
        let facts = runtime_facts();
        let plan = auxiliary_plan(&backend, &facts);
        let mut engine = writable_engine(plan, backend, facts);
        let request = apply_request(&engine, "existingOnlyAuxiliary");
        engine
            .fake_registry_mut()
            .replace_before_compare_exchange_at(2, auxiliary.clone(), disappeared.clone());

        assert!(matches!(
            engine.apply(request),
            Err(EngineError::ApplyFailed { .. })
        ));
        assert_eq!(engine.fake_registry().snapshot(&auxiliary), disappeared);
        assert_eq!(engine.fake_registry().snapshot(&primary), dword(1));
    }
}

#[test]
fn search_local_rejects_eea_and_unknown_regions_even_when_exactly_certified() {
    let catalog = first_batch_catalog().unwrap();
    let feature = id(first_batch_ids::SEARCH_LOCAL_ONLY);
    for region in ["DE", "ZZ"] {
        let facts = RuntimeFacts {
            environment: WindowsEnvironment {
                region: region.into(),
                ..environment()
            },
            lock_screen_background: LockScreenBackground::Unknown,
        };
        let manifest = VerificationManifest::new([(
            feature.clone(),
            VerificationRule::Advanced(vec![facts.environment.clone()]),
        )]);
        let error = catalog
            .resolve(&feature, &manifest, &facts, &MemoryRegistry::default())
            .unwrap_err();
        assert!(matches!(error, FirstBatchPlanError::Inapplicable { .. }));
    }
}

#[test]
fn lock_tips_requires_picture_or_slideshow_at_resolve_and_apply() {
    let catalog = first_batch_catalog().unwrap();
    let feature = id(first_batch_ids::LOCK_SCREEN_TIPS);
    let descriptor = catalog.descriptor(&feature).unwrap();
    let mut backend = MemoryRegistry::default();
    for mutation in &descriptor.mutations {
        backend.set_snapshot(mutation.address.clone(), dword(1));
    }
    for background in [
        LockScreenBackground::Unknown,
        LockScreenBackground::Spotlight,
    ] {
        let facts = RuntimeFacts {
            environment: environment(),
            lock_screen_background: background,
        };
        let manifest = VerificationManifest::new([(
            feature.clone(),
            VerificationRule::Advanced(vec![facts.environment.clone()]),
        )]);
        assert!(matches!(
            catalog.resolve(&feature, &manifest, &facts, &backend),
            Err(FirstBatchPlanError::Inapplicable { .. })
        ));
    }

    let facts = RuntimeFacts {
        environment: environment(),
        lock_screen_background: LockScreenBackground::Picture,
    };
    let manifest = VerificationManifest::new([(
        feature.clone(),
        VerificationRule::Advanced(vec![facts.environment.clone()]),
    )]);
    let plan = catalog
        .resolve(&feature, &manifest, &facts, &backend)
        .unwrap();
    let engine = writable_engine(plan, backend, facts.clone());
    engine.fake_runtime().set_facts(RuntimeFacts {
        lock_screen_background: LockScreenBackground::Spotlight,
        ..facts
    });

    assert!(matches!(
        engine.inspect(&feature),
        Err(EngineError::Inapplicable(
            ApplicabilityFailure::LockScreenBackground(LockScreenBackground::Spotlight)
        ))
    ));
    assert_eq!(engine.fake_registry().successful_write_count(), 0);
}
