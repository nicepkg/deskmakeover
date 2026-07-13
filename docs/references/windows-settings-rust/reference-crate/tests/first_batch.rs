mod common;

use common::*;
use deskmakeover_windows_settings_reference::*;

fn setting(id: &str) -> FirstBatchSetting {
    first_batch_settings()
        .into_iter()
        .find(|setting| setting.id.as_str() == id)
        .unwrap_or_else(|| panic!("missing first-batch setting {id}"))
}

#[test]
fn all_guided_and_invariant_items_have_no_mutations() {
    let settings = first_batch_settings();
    assert_eq!(settings.len(), 21);
    for item in settings.iter().filter(|item| {
        matches!(
            item.tier,
            FirstBatchTier::Guided | FirstBatchTier::Invariant
        )
    }) {
        assert!(item.mutations.is_empty(), "{} can write", item.id);
        assert!(
            item.auxiliary_mutations.is_empty(),
            "{} has auxiliary writes",
            item.id
        );
        assert!(
            item.effect_verifier.is_none(),
            "{} could carry a verifier into a writable plan",
            item.id
        );
    }

    let widgets = setting(first_batch_ids::TASKBAR_WIDGETS);
    assert_eq!(widgets.tier, FirstBatchTier::Guided);
    assert_eq!(
        widgets.manual_fallback,
        Some(ManualFallback::SettingsPage("ms-settings:taskbar"))
    );
    assert!(widgets
        .notes
        .iter()
        .any(|note| note.contains("Do not bypass UCPD")));
}

#[test]
fn start_recent_is_an_invariant_and_start_track_docs_is_forbidden() {
    let recent = setting(first_batch_ids::START_RECENT);
    assert_eq!(recent.tier, FirstBatchTier::Invariant);
    assert!(recent.mutations.is_empty());

    let promotions = setting(first_batch_ids::START_PROMOTIONS);
    assert_eq!(promotions.mutations.len(), 1);
    assert_eq!(
        promotions.mutations[0].address.value,
        "Start_IrisRecommendations"
    );
    assert!(promotions
        .forbidden_mutations
        .iter()
        .any(|item| item.address.value == "Start_TrackDocs"));
    assert!(first_batch_settings().iter().all(|item| item
        .mutations
        .iter()
        .all(|mutation| mutation.address.value != "Start_TrackDocs")));
}

#[test]
fn auxiliary_values_are_if_present_metadata_not_primary_recipe_leaves() {
    let notifications = setting(first_batch_ids::NOTIFICATION_SUGGESTIONS);
    let suggested = setting(first_batch_ids::SETTINGS_SUGGESTED_CONTENT);
    let auxiliaries = notifications
        .auxiliary_mutations
        .iter()
        .chain(&suggested.auxiliary_mutations)
        .collect::<Vec<_>>();
    assert_eq!(auxiliaries.len(), 4);
    assert!(auxiliaries.iter().all(|item| {
        item.condition == AuxiliaryCondition::IfPresentAndExactEnvironmentVerified
            && item.exact_environment_allowlist.is_empty()
    }));

    let auxiliary_names = auxiliaries
        .iter()
        .map(|item| item.mutation.address.value.as_str())
        .collect::<Vec<_>>();
    for name in [
        "SoftLandingEnabled",
        "SubscribedContent-353694Enabled",
        "SubscribedContent-353696Enabled",
        "SubscribedContent-353698Enabled",
    ] {
        assert!(auxiliary_names.contains(&name));
        assert!(first_batch_settings().iter().all(|item| item
            .mutations
            .iter()
            .all(|mutation| mutation.address.value != name)));
    }
}

#[test]
fn initial_manifest_fails_closed_for_every_direct_write_candidate() {
    let manifest = initial_verification_manifest();
    for item in first_batch_settings()
        .into_iter()
        .filter(|item| !item.mutations.is_empty())
    {
        let capability = manifest.evaluate(&item.id, &environment());
        match item.tier {
            FirstBatchTier::AutomaticCandidate => assert_eq!(
                capability,
                Capability::Unavailable(UnavailableReason::FeatureNotVerified),
                "{} was accidentally certified",
                item.id
            ),
            FirstBatchTier::Advanced => assert_eq!(
                capability,
                Capability::Unavailable(UnavailableReason::AdvancedEnvironmentNotAllowlisted),
                "{} advanced allowlist was not empty",
                item.id
            ),
            FirstBatchTier::Guided | FirstBatchTier::Invariant => unreachable!(),
        }
    }
}

#[test]
fn guided_items_are_manual_only_and_invariants_are_not_manifest_rules() {
    let manifest = initial_verification_manifest();
    for item in first_batch_settings() {
        match item.tier {
            FirstBatchTier::Guided => {
                assert_eq!(
                    manifest.evaluate(&item.id, &environment()),
                    Capability::ManualOnly
                )
            }
            FirstBatchTier::Invariant => assert_eq!(
                manifest.evaluate(&item.id, &environment()),
                Capability::Unavailable(UnavailableReason::FeatureNotVerified)
            ),
            _ => {}
        }
    }
}

#[test]
fn production_catalog_retains_complete_descriptors() {
    let expected = first_batch_settings();
    let catalog = first_batch_catalog().expect("valid built-in first-batch catalog");
    assert_eq!(catalog.descriptors(), expected);
    for item in catalog.descriptors() {
        assert_eq!(catalog.descriptor(&item.id), Some(item));
        for mutation in &item.mutations {
            assert_eq!(mutation.address.hive, RegistryHive::CurrentUser);
            assert_eq!(mutation.address.view, RegistryView::Registry64);
            assert_eq!(mutation.desired.kind(), Some(&RegistryValueKind::Dword));
        }
    }
}

#[test]
fn planner_requires_a_write_permitting_capability() {
    let catalog = first_batch_catalog().unwrap();
    let id = id(first_batch_ids::SEARCH_HIGHLIGHTS);
    let error = catalog
        .resolve(
            &id,
            &initial_verification_manifest(),
            &runtime_facts(),
            &MemoryRegistry::default(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("is not writable"));
}

#[test]
fn planner_rejects_an_existing_policy_guard() {
    let catalog = first_batch_catalog().unwrap();
    let id = id(first_batch_ids::SEARCH_HIGHLIGHTS);
    let descriptor = catalog.descriptor(&id).unwrap();
    let guard = descriptor.policy_guards[0].address.clone();
    let mut registry = MemoryRegistry::default();
    registry.set_snapshot(guard.clone(), dword(1));
    let manifest = VerificationManifest::new([(id.clone(), standard_rule())]);

    let error = catalog
        .resolve(&id, &manifest, &runtime_facts(), &registry)
        .unwrap_err();
    assert!(error.to_string().contains("is managed"));
    assert!(error.to_string().contains(&guard.value));
}

#[test]
fn planner_rejects_a_selected_address_marked_as_policy_managed() {
    let catalog = first_batch_catalog().unwrap();
    let id = id(first_batch_ids::TASKBAR_SEARCH);
    let selected_address = catalog.descriptor(&id).unwrap().mutations[0]
        .address
        .clone();
    let mut registry = MemoryRegistry::default();
    registry.mark_policy_managed(selected_address.clone());
    let manifest = VerificationManifest::new([(id.clone(), standard_rule())]);

    let error = catalog
        .resolve(&id, &manifest, &runtime_facts(), &registry)
        .unwrap_err();
    assert!(error.to_string().contains("is managed"));
    assert!(error.to_string().contains(&selected_address.value));
}

#[test]
fn empty_auxiliary_allowlist_omits_even_an_existing_value() {
    let catalog = first_batch_catalog().unwrap();
    let id = id(first_batch_ids::NOTIFICATION_SUGGESTIONS);
    let descriptor = catalog.descriptor(&id).unwrap();
    let auxiliary = &descriptor.auxiliary_mutations[0];
    assert!(auxiliary.exact_environment_allowlist.is_empty());
    let mut registry = MemoryRegistry::default();
    registry.set_snapshot(auxiliary.mutation.address.clone(), dword(1));
    let manifest = VerificationManifest::new([(id.clone(), standard_rule())]);

    let resolved = catalog
        .resolve(&id, &manifest, &runtime_facts(), &registry)
        .unwrap();
    assert_eq!(resolved.selected_mutations(), descriptor.mutations);
    assert_eq!(resolved.descriptor(), descriptor);
    assert_eq!(
        resolved.effect_verifier(),
        EffectVerifier::DelayedReadBackAndSettingsUi
    );
    assert_eq!(resolved.selected_capability(), &Capability::Available);
}

#[test]
fn typed_verifiers_and_policy_guards_capture_special_boundaries() {
    assert_eq!(
        setting(first_batch_ids::SEARCH_LOCAL_ONLY).effect_verifier,
        Some(EffectVerifier::SearchLocalNonceHasNoWebAffordance)
    );
    assert_eq!(
        setting(first_batch_ids::START_PROMOTIONS).effect_verifier,
        Some(EffectVerifier::StartPromotionsAbsentAndKnownRecentPreserved)
    );
    assert_eq!(
        setting(first_batch_ids::ADVERTISING_PERSONALIZATION).effect_verifier,
        Some(EffectVerifier::AdvertisingIdIsEmpty)
    );
    for id in [
        first_batch_ids::SEARCH_HIGHLIGHTS,
        first_batch_ids::SEARCH_LOCAL_ONLY,
        first_batch_ids::ADVERTISING_PERSONALIZATION,
    ] {
        assert!(!setting(id).policy_guards.is_empty(), "{id}");
    }
}

#[test]
fn recommended_defaults_still_exclude_advanced_guided_and_invariant_items() {
    let defaults = DefaultSet::recommended();
    for item in first_batch_settings() {
        match item.tier {
            FirstBatchTier::Advanced | FirstBatchTier::Guided | FirstBatchTier::Invariant => {
                assert!(!defaults.contains(item.id.as_str()), "{}", item.id)
            }
            FirstBatchTier::AutomaticCandidate => {
                assert!(defaults.contains(item.id.as_str()), "{}", item.id)
            }
        }
    }
    assert!(defaults
        .writable_for(&initial_verification_manifest(), &environment())
        .is_empty());
}

#[test]
fn missing_policies_and_dword_shapes_are_explicit() {
    for item in first_batch_settings() {
        let expected = if matches!(
            item.id.as_str(),
            first_batch_ids::LOCK_SCREEN_TIPS | first_batch_ids::DEVICE_USAGE
        ) {
            MissingPolicy::MustAlreadyExist
        } else {
            MissingPolicy::CreateAllowed
        };
        for mutation in &item.mutations {
            assert_eq!(mutation.missing_policy, expected, "{}", item.id);
            let RegistrySnapshot::Present(value) = &mutation.desired else {
                panic!("{} has a non-present desired value", item.id);
            };
            assert_eq!(value.kind, RegistryValueKind::Dword, "{}", item.id);
            assert_eq!(value.bytes.len(), 4, "{}", item.id);
        }
        for auxiliary in &item.auxiliary_mutations {
            assert_eq!(
                auxiliary.mutation.missing_policy,
                MissingPolicy::MustAlreadyExist,
                "{}",
                item.id
            );
        }
    }
}

#[test]
fn first_batch_descriptors_include_every_independent_policy_guard() {
    let expected = [
        (
            first_batch_ids::SEARCH_HIGHLIGHTS,
            RegistryHive::LocalMachine,
            RegistryView::Registry64,
            r"SOFTWARE\Policies\Microsoft\Windows\Windows Search",
            "EnableDynamicContentInWSB",
        ),
        (
            first_batch_ids::SEARCH_LOCAL_ONLY,
            RegistryHive::LocalMachine,
            RegistryView::Registry64,
            r"SOFTWARE\Policies\Microsoft\Windows\Windows Search",
            "ConnectedSearchUseWeb",
        ),
        (
            first_batch_ids::LOCK_SCREEN_TIPS,
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            r"Software\Policies\Microsoft\Windows\CloudContent",
            "ConfigureWindowsSpotlight",
        ),
        (
            first_batch_ids::LOCK_SCREEN_TIPS,
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            r"Software\Policies\Microsoft\Windows\CloudContent",
            "DisableWindowsSpotlightFeatures",
        ),
        (
            first_batch_ids::NOTIFICATION_SUGGESTIONS,
            RegistryHive::LocalMachine,
            RegistryView::Registry64,
            r"Software\Policies\Microsoft\Windows\CloudContent",
            "DisableSoftLanding",
        ),
        (
            first_batch_ids::NOTIFICATION_WELCOME,
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            r"Software\Policies\Microsoft\Windows\CloudContent",
            "DisableWindowsSpotlightWindowsWelcomeExperience",
        ),
        (
            first_batch_ids::SETTINGS_SUGGESTED_CONTENT,
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            r"Software\Policies\Microsoft\Windows\CloudContent",
            "DisableWindowsSpotlightOnSettings",
        ),
        (
            first_batch_ids::TASKBAR_SEARCH,
            RegistryHive::LocalMachine,
            RegistryView::Registry64,
            r"Software\Policies\Microsoft\Windows\Windows Search",
            "SearchOnTaskbarMode",
        ),
        (
            first_batch_ids::TASKBAR_TASK_VIEW,
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            r"Software\Policies\Microsoft\Windows\Explorer",
            "HideTaskViewButton",
        ),
        (
            first_batch_ids::TASKBAR_TASK_VIEW,
            RegistryHive::LocalMachine,
            RegistryView::Registry64,
            r"Software\Policies\Microsoft\Windows\Explorer",
            "HideTaskViewButton",
        ),
        (
            first_batch_ids::ADVERTISING_PERSONALIZATION,
            RegistryHive::LocalMachine,
            RegistryView::Registry64,
            r"Software\Policies\Microsoft\Windows\AdvertisingInfo",
            "DisabledByGroupPolicy",
        ),
    ];
    for (feature, hive, view, key, value) in expected {
        assert!(
            setting(feature).policy_guards.iter().any(|guard| {
                guard.address.hive == hive
                    && guard.address.view == view
                    && guard.address.key == key
                    && guard.address.value == value
            }),
            "missing exact guard {feature}: {hive:?}/{view:?}/{key}/{value}"
        );
    }
    assert_eq!(
        first_batch_settings()
            .iter()
            .map(|setting| setting.policy_guards.len())
            .sum::<usize>(),
        expected.len(),
        "every policy guard must have an exact tuple assertion"
    );
}
