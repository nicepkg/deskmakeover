use super::*;

pub(super) fn recipe(
    id: &'static str,
    tier: FirstBatchTier,
    evidence: EvidenceLevel,
    mutations: Vec<SettingMutation>,
) -> FirstBatchSetting {
    FirstBatchSetting {
        id: SettingId::new(id),
        recipe_version: 1,
        tier,
        evidence,
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

pub(super) fn guided(id: &'static str, fallback: ManualFallback) -> FirstBatchSetting {
    let mut setting = recipe(
        id,
        FirstBatchTier::Guided,
        EvidenceLevel::NoStableSetter,
        Vec::new(),
    );
    setting.manual_fallback = Some(fallback);
    // Guided entries never resolve to a writable recipe, so they deliberately have no verifier.
    setting.effect_verifier = None;
    setting
}

pub(super) fn invariant(id: &'static str, evidence: EvidenceLevel) -> FirstBatchSetting {
    let mut setting = recipe(id, FirstBatchTier::Invariant, evidence, Vec::new());
    setting.effect_verifier = None;
    setting
}

pub(super) fn dword(key: &str, value: &str, desired: u32) -> SettingMutation {
    dword_with_policy(key, value, desired, MissingPolicy::CreateAllowed)
}

pub(super) fn existing_dword(key: &str, value: &str, desired: u32) -> SettingMutation {
    dword_with_policy(key, value, desired, MissingPolicy::MustAlreadyExist)
}

fn dword_with_policy(
    key: &str,
    value: &str,
    desired: u32,
    missing_policy: MissingPolicy,
) -> SettingMutation {
    SettingMutation {
        address: RegistryAddress::new(
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            key,
            value,
        ),
        desired: RegistrySnapshot::Present(RawRegistryValue::dword(desired)),
        accepted_existing_kinds: vec![RegistryValueKind::Dword],
        missing_policy,
    }
}

pub(super) fn auxiliary(key: &str, value: &'static str, note: &'static str) -> AuxiliaryMutation {
    AuxiliaryMutation {
        mutation: existing_dword(key, value, 0),
        condition: AuxiliaryCondition::IfPresentAndExactEnvironmentVerified,
        exact_environment_allowlist: Vec::new(),
        note,
    }
}

pub(super) fn guard(hive: RegistryHive, key: &str, value: &str, note: &'static str) -> PolicyGuard {
    PolicyGuard {
        address: RegistryAddress::new(hive, RegistryView::Registry64, key, value),
        note,
    }
}

pub(super) fn forbidden(key: &str, value: &str, reason: &'static str) -> ForbiddenMutation {
    ForbiddenMutation {
        address: RegistryAddress::new(
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            key,
            value,
        ),
        reason,
    }
}

pub(super) fn device_usage_mutations() -> Vec<SettingMutation> {
    let root = r"Software\Microsoft\Windows\CurrentVersion\CloudExperienceHost\Intent";
    let mut mutations = [
        "creative",
        "business",
        "developer",
        "entertainment",
        "family",
        "gaming",
        "schoolwork",
    ]
    .into_iter()
    .map(|name| existing_dword(&format!(r"{root}\{name}"), "Intent", 0))
    .collect::<Vec<_>>();
    mutations.push(existing_dword(
        &format!(r"{root}\OffDeviceConsent"),
        "accepted",
        0,
    ));
    mutations
}

pub(super) fn setting_mut<'a>(
    settings: &'a mut [FirstBatchSetting],
    id: &str,
) -> &'a mut FirstBatchSetting {
    settings
        .iter_mut()
        .find(|setting| setting.id.as_str() == id)
        .expect("first-batch setting exists")
}
