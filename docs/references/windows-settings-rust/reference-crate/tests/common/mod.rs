#![allow(dead_code)]

use deskmakeover_windows_settings_reference::*;

pub type TestEngine =
    SettingsEngine<MemoryRegistry, MemoryJournal, MemoryVerificationBackend, MemoryRuntimeProbe>;

#[derive(Debug, Clone)]
pub struct TestRecipe {
    pub id: SettingId,
    pub recipe_version: u32,
    pub mutations: Vec<SettingMutation>,
    pub verification: VerificationPlan,
}

pub fn id(value: &str) -> SettingId {
    SettingId::new(value)
}

pub fn address(name: &str) -> RegistryAddress {
    address_in(r"Software\DeskMakeoverReference\Tests", name)
}

pub fn address_in(path: &str, name: &str) -> RegistryAddress {
    RegistryAddress::new(
        RegistryHive::CurrentUser,
        RegistryView::Registry64,
        path,
        name,
    )
}

pub fn key(path: &str) -> RegistryKey {
    RegistryKey::new(RegistryHive::CurrentUser, RegistryView::Registry64, path)
}

pub fn dword(value: u32) -> RegistrySnapshot {
    RegistrySnapshot::Present(RawRegistryValue::dword(value))
}

pub fn string_raw(bytes: &[u8]) -> RegistrySnapshot {
    RegistrySnapshot::Present(RawRegistryValue::new(RegistryValueKind::String, bytes))
}

pub fn environment() -> WindowsEnvironment {
    WindowsEnvironment {
        major: 10,
        minor: 0,
        build: 26_100,
        ubr: 4_200,
        display_version: "24H2".into(),
        edition_id: "Professional".into(),
        edition: WindowsEdition::Pro,
        installation_type: "Client".into(),
        product_type: 48,
        is_workstation: true,
        region: "CN".into(),
        native_architecture: "x64".into(),
        process_architecture: "x64".into(),
        packaged: false,
    }
}

pub fn runtime_facts() -> RuntimeFacts {
    RuntimeFacts {
        environment: environment(),
        lock_screen_background: LockScreenBackground::Unknown,
    }
}

pub fn standard_rule() -> VerificationRule {
    VerificationRule::Standard(StandardVerification {
        families: vec![
            VerifiedBuildFamily {
                build: 22_000,
                min_ubr: 1_761,
                max_ubr: Some(9_999),
            },
            VerifiedBuildFamily {
                build: 22_631,
                min_ubr: 0,
                max_ubr: Some(9_999),
            },
            VerifiedBuildFamily {
                build: 22_621,
                min_ubr: 0,
                max_ubr: Some(9_999),
            },
            VerifiedBuildFamily {
                build: 26_100,
                min_ubr: 0,
                max_ubr: Some(8_737),
            },
            VerifiedBuildFamily {
                build: 26_200,
                min_ubr: 0,
                max_ubr: Some(8_737),
            },
        ],
        profiles: vec![
            WindowsEnvironment {
                build: 22_000,
                ubr: 1_761,
                ..environment()
            },
            WindowsEnvironment {
                build: 22_631,
                ubr: 4_200,
                ..environment()
            },
            WindowsEnvironment {
                build: 22_621,
                ubr: 4_200,
                ..environment()
            },
            environment(),
            WindowsEnvironment {
                build: 26_200,
                ubr: 4_200,
                ..environment()
            },
        ],
    })
}

pub fn definition(feature: &str, values: Vec<(&str, RegistrySnapshot)>) -> TestRecipe {
    definition_at(
        feature,
        values
            .into_iter()
            .map(|(name, desired)| (address(name), desired))
            .collect(),
    )
}

pub fn definition_at(
    feature: &str,
    values: Vec<(RegistryAddress, RegistrySnapshot)>,
) -> TestRecipe {
    TestRecipe {
        id: id(feature),
        recipe_version: 1,
        mutations: values
            .into_iter()
            .map(|(address, desired)| SettingMutation {
                address,
                desired,
                accepted_existing_kinds: vec![RegistryValueKind::Dword],
                missing_policy: MissingPolicy::CreateAllowed,
            })
            .collect(),
        verification: VerificationPlan::new(EffectVerifier::DelayedReadBackAndSettingsUi),
    }
}

pub fn engine(
    definition: TestRecipe,
    rule: VerificationRule,
    backend: MemoryRegistry,
) -> TestEngine {
    let resolved = resolve(definition, rule, &backend);
    SettingsEngine::new(
        resolved,
        backend,
        MemoryJournal::default(),
        MemoryVerificationBackend::new(),
        MemoryRuntimeProbe::new(runtime_facts()),
    )
}

pub fn resolve(
    definition: TestRecipe,
    rule: VerificationRule,
    backend: &MemoryRegistry,
) -> ResolvedRecipe {
    let feature = definition.id.clone();
    let (tier, evidence) = match &rule {
        VerificationRule::Standard(_) => (
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
        ),
        VerificationRule::Advanced(_) => {
            (FirstBatchTier::Advanced, EvidenceLevel::CommunityObserved)
        }
        VerificationRule::ManualOnly => panic!("manual-only rules cannot produce writable plans"),
    };
    let descriptor = FirstBatchSetting {
        id: feature.clone(),
        recipe_version: definition.recipe_version,
        tier,
        evidence,
        applicability: Applicability::AnyCertifiedEnvironment,
        mutations: definition.mutations,
        auxiliary_mutations: Vec::new(),
        policy_guards: Vec::new(),
        forbidden_mutations: Vec::new(),
        manual_fallback: None,
        effect_verifier: Some(definition.verification.effect),
        notes: Vec::new(),
    };
    FirstBatchCatalog::try_new([descriptor])
        .unwrap()
        .resolve(
            &feature,
            &VerificationManifest::new([(feature.clone(), rule)]),
            &runtime_facts(),
            backend,
        )
        .unwrap()
}

pub fn apply_request(engine: &TestEngine, feature: &str) -> ApplyRequest {
    let inspection = engine.inspect(&id(feature)).expect("inspection");
    assert_eq!(inspection.feature, id(feature));
    inspection.apply_request()
}

pub fn restore_request(engine: &TestEngine, feature: &str) -> RestoreRequest {
    let inspection = engine
        .inspect_restore(&id(feature))
        .expect("restore inspection");
    RestoreRequest {
        feature: id(feature),
        expected: inspection.expected_values(),
    }
}
