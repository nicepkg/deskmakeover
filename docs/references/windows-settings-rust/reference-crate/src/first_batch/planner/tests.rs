use super::*;
use crate::first_batch::{AuxiliaryMutation, ForbiddenMutation};
use crate::{
    EffectVerifier, LockScreenBackground, MissingPolicy, RawRegistryValue, RegistryHive,
    RegistryValueKind, RegistryView, RuntimeFacts, VerificationRule, WindowsEdition,
};

fn mutation(key: &str, value: &str) -> SettingMutation {
    SettingMutation {
        address: RegistryAddress::new(
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            key,
            value,
        ),
        desired: RegistrySnapshot::Present(RawRegistryValue::dword(0)),
        accepted_existing_kinds: vec![RegistryValueKind::Dword],
        missing_policy: MissingPolicy::CreateAllowed,
    }
}

fn descriptor(id: &str, key: &str, value: &str) -> FirstBatchSetting {
    FirstBatchSetting {
        id: SettingId::new(id),
        recipe_version: 1,
        tier: FirstBatchTier::AutomaticCandidate,
        evidence: EvidenceLevel::MicrosoftImplementation,
        applicability: Applicability::AnyCertifiedEnvironment,
        mutations: vec![mutation(key, value)],
        auxiliary_mutations: Vec::new(),
        policy_guards: Vec::new(),
        forbidden_mutations: Vec::new(),
        manual_fallback: None,
        effect_verifier: Some(EffectVerifier::DelayedReadBackAndSettingsUi),
        notes: Vec::new(),
    }
}

fn environment() -> WindowsEnvironment {
    WindowsEnvironment {
        major: 10,
        minor: 0,
        build: 26_100,
        ubr: 8_737,
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

fn runtime() -> RuntimeFacts {
    RuntimeFacts {
        environment: environment(),
        lock_screen_background: LockScreenBackground::Unknown,
    }
}

fn standard_manifest(id: &SettingId) -> VerificationManifest {
    use crate::{StandardVerification, VerifiedBuildFamily};

    VerificationManifest::new([(
        id.clone(),
        VerificationRule::Standard(StandardVerification {
            families: vec![VerifiedBuildFamily {
                build: 26_100,
                min_ubr: 8_737,
                max_ubr: Some(8_737),
            }],
            profiles: vec![environment()],
        }),
    )])
}

#[test]
fn rejects_duplicate_ids_instead_of_overwriting() {
    let error = FirstBatchCatalog::try_new([
        descriptor("same", "Software\\One", "Enabled"),
        descriptor("SAME", "Software\\Two", "Enabled"),
    ])
    .unwrap_err();
    assert!(matches!(error, FirstBatchPlanError::DuplicateSettingId(_)));
}

#[test]
fn rejects_duplicate_addresses_within_recipe_case_insensitively() {
    let mut item = descriptor("one", "Software\\One", "Enabled");
    item.mutations.push(mutation("software\\one", "ENABLED"));
    let error = FirstBatchCatalog::try_new([item]).unwrap_err();
    assert!(matches!(
        error,
        FirstBatchPlanError::DuplicateMutationAddress { .. }
    ));
}

#[test]
fn rejects_cross_resource_collisions_case_insensitively() {
    let error = FirstBatchCatalog::try_new([
        descriptor("one", "Software\\Shared", "Enabled"),
        descriptor("two", "software\\shared", "ENABLED"),
    ])
    .unwrap_err();
    assert!(matches!(
        error,
        FirstBatchPlanError::ResourceCollision { .. }
    ));
}

#[test]
fn exact_environment_matching_requires_canonical_complete_fingerprints() {
    let allowed = environment();
    let environment = environment();
    assert!(exact_environment_matches(&allowed, &environment));

    let mut noncanonical_region = environment.clone();
    noncanonical_region.region = "cn".into();
    assert!(!exact_environment_matches(&allowed, &noncanonical_region));

    let mut wrong_ubr = environment;
    wrong_ubr.ubr += 1;
    assert!(!exact_environment_matches(&allowed, &wrong_ubr));
}

#[test]
fn guided_and_invariant_tiers_can_never_resolve() {
    for tier in [FirstBatchTier::Guided, FirstBatchTier::Invariant] {
        let mut item = descriptor("blocked", "Software\\Blocked", "Enabled");
        item.tier = tier;
        item.evidence = match tier {
            FirstBatchTier::Guided => EvidenceLevel::NoStableSetter,
            FirstBatchTier::Invariant => EvidenceLevel::MicrosoftContract,
            _ => unreachable!(),
        };
        let id = item.id.clone();
        let catalog = FirstBatchCatalog::try_new([item]).unwrap();
        let error = catalog
            .resolve(
                &id,
                &standard_manifest(&id),
                &runtime(),
                &crate::MemoryRegistry::default(),
            )
            .unwrap_err();
        assert!(matches!(error, FirstBatchPlanError::NonWritableTier { .. }));
    }
}

#[test]
fn advanced_rejects_an_injected_standard_certification() {
    let mut item = descriptor("advanced", "Software\\Advanced", "Enabled");
    item.tier = FirstBatchTier::Advanced;
    item.evidence = EvidenceLevel::CommunityObserved;
    let id = item.id.clone();
    let catalog = FirstBatchCatalog::try_new([item]).unwrap();
    let error = catalog
        .resolve(
            &id,
            &standard_manifest(&id),
            &runtime(),
            &crate::MemoryRegistry::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        FirstBatchPlanError::CertificationMismatch {
            capability: Capability::Available,
            ..
        }
    ));
}

#[test]
fn automatic_rejects_an_injected_advanced_certification() {
    let item = descriptor("automatic", "Software\\Automatic", "Enabled");
    let id = item.id.clone();
    let catalog = FirstBatchCatalog::try_new([item]).unwrap();
    let manifest =
        VerificationManifest::new([(id.clone(), VerificationRule::Advanced(vec![environment()]))]);
    let error = catalog
        .resolve(
            &id,
            &manifest,
            &runtime(),
            &crate::MemoryRegistry::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        FirstBatchPlanError::CertificationMismatch {
            capability: Capability::Advanced,
            ..
        }
    ));
}

#[test]
fn rejects_invalid_tier_evidence_and_missing_verifier() {
    let mut wrong_evidence = descriptor("wrong", "Software\\Wrong", "Enabled");
    wrong_evidence.evidence = EvidenceLevel::CommunityObserved;
    let id = wrong_evidence.id.clone();
    let catalog = FirstBatchCatalog::try_new([wrong_evidence]).unwrap();
    let error = catalog
        .resolve(
            &id,
            &standard_manifest(&id),
            &runtime(),
            &crate::MemoryRegistry::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        FirstBatchPlanError::InvalidTierEvidence { .. }
    ));

    let mut no_verifier = descriptor("unverified", "Software\\Unverified", "Enabled");
    no_verifier.effect_verifier = None;
    let id = no_verifier.id.clone();
    let catalog = FirstBatchCatalog::try_new([no_verifier]).unwrap();
    let error = catalog
        .resolve(
            &id,
            &standard_manifest(&id),
            &runtime(),
            &crate::MemoryRegistry::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        FirstBatchPlanError::MissingEffectVerifier(_)
    ));
}

#[test]
fn auxiliary_requires_both_exact_certification_and_current_presence() {
    let mut item = descriptor("with-aux", "Software\\Primary", "Enabled");
    let mut auxiliary = mutation("Software\\Companion", "Enabled");
    auxiliary.missing_policy = MissingPolicy::MustAlreadyExist;
    item.auxiliary_mutations.push(AuxiliaryMutation {
        mutation: auxiliary.clone(),
        condition: AuxiliaryCondition::IfPresentAndExactEnvironmentVerified,
        exact_environment_allowlist: vec![environment()],
        note: "test companion",
    });
    let id = item.id.clone();
    let catalog = FirstBatchCatalog::try_new([item]).unwrap();
    let manifest = standard_manifest(&id);

    let missing = catalog
        .resolve(
            &id,
            &manifest,
            &runtime(),
            &crate::MemoryRegistry::default(),
        )
        .unwrap();
    assert_eq!(missing.definition.mutations.len(), 1);

    let mut present_registry = crate::MemoryRegistry::default();
    present_registry.set_snapshot(
        auxiliary.address.clone(),
        RegistrySnapshot::Present(RawRegistryValue::dword(1)),
    );
    let present = catalog
        .resolve(&id, &manifest, &runtime(), &present_registry)
        .unwrap();
    assert_eq!(present.definition.mutations.len(), 2);
    assert_eq!(present.definition.mutations[1], auxiliary);
}

#[test]
fn any_globally_forbidden_address_is_rejected_if_selected() {
    let mut invariant = descriptor("safety-boundary", "Software\\Safe", "Enabled");
    let forbidden = mutation("Software\\Protected", "NeverTouch").address;
    invariant.forbidden_mutations.push(ForbiddenMutation {
        address: forbidden.clone(),
        reason: "must remain untouched",
    });
    let selected = descriptor("selected", "software\\protected", "NEVERTOUCH");
    let selected_id = selected.id.clone();
    let catalog = FirstBatchCatalog::try_new([invariant, selected]).unwrap();

    let error = catalog
        .resolve(
            &selected_id,
            &standard_manifest(&selected_id),
            &runtime(),
            &crate::MemoryRegistry::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        FirstBatchPlanError::ForbiddenMutation { .. }
    ));
}

#[test]
fn catalog_and_execution_boundary_reject_protected_patterns() {
    for (key, value) in [
        (
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            "Start_TrackDocs",
        ),
        (r"Software\Microsoft\Windows\CloudStore\Cache", "Enabled"),
        (r"Software\Packages\Example\LocalState", "Enabled"),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            "TaskbarDa",
        ),
    ] {
        assert!(matches!(
            FirstBatchCatalog::try_new([descriptor("protected", key, value)]),
            Err(FirstBatchPlanError::ProtectedMutation { .. })
        ));
    }

    let safe = descriptor("safe", r"Software\Safe", "Enabled");
    let id = safe.id.clone();
    let catalog = FirstBatchCatalog::try_new([safe]).unwrap();
    let mut resolved = catalog
        .resolve(
            &id,
            &standard_manifest(&id),
            &runtime(),
            &crate::MemoryRegistry::default(),
        )
        .unwrap();
    resolved.definition.mutations[0].address = RegistryAddress::new(
        RegistryHive::CurrentUser,
        RegistryView::Registry32,
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        "TASKBARDA",
    );
    assert!(matches!(
        resolved.ensure_unprotected(),
        Err(FirstBatchPlanError::ProtectedMutation { .. })
    ));
}

#[test]
fn malformed_dword_payload_is_rejected_at_catalog_boundary() {
    let mut item = descriptor("bad-dword", r"Software\Bad", "Enabled");
    item.mutations[0].desired =
        RegistrySnapshot::Present(RawRegistryValue::new(RegistryValueKind::Dword, [0, 1, 2]));
    assert!(matches!(
        FirstBatchCatalog::try_new([item]),
        Err(FirstBatchPlanError::InvalidDwordBytes { length: 3, .. })
    ));
}
