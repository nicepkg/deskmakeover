use crate::{
    EffectVerifier, RawRegistryValue, ReceiptSnapshot, RegistryAddress, RegistryHive,
    RegistrySnapshot, RegistryValueKind, RegistryView, TransactionIntent, TransactionValue,
    VerificationPlan, VerificationReceipt,
};

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

pub(super) fn validate_receipt(
    plan: &VerificationPlan,
    intent: TransactionIntent,
    values: &[TransactionValue],
    receipt: &VerificationReceipt,
) -> Result<(), String> {
    if !plan.budget.is_bounded() {
        return Err("verification budget must be finite and non-zero".into());
    }
    match (&plan.effect, receipt) {
        (
            EffectVerifier::StartPromotionsAbsentAndKnownRecentPreserved,
            VerificationReceipt::StartKnownRecent { marker },
        ) if !marker.trim().is_empty() => validate_start_values(values, intent),
        (
            EffectVerifier::DeviceUsageAllOffAndPrioritiesPreserved,
            VerificationReceipt::DeviceUsagePriorities { priorities },
        ) => validate_device_usage_receipt(values, intent, priorities),
        (
            EffectVerifier::DelayedReadBackAndSettingsUi
            | EffectVerifier::SearchLocalNonceHasNoWebAffordance
            | EffectVerifier::AdvertisingIdIsEmpty,
            VerificationReceipt::NoBaseline,
        ) => Ok(()),
        _ => Err("verification receipt does not match the selected effect verifier".into()),
    }
}

pub(super) fn device_usage_priority_addresses(
    values: &[TransactionValue],
    intent: TransactionIntent,
) -> Result<Vec<RegistryAddress>, String> {
    if values.len() != DEVICE_USAGE_CATEGORIES.len() + 1 {
        return Err("device-usage transaction must contain seven intents and consent".into());
    }
    let mut priorities = Vec::with_capacity(DEVICE_USAGE_CATEGORIES.len());
    for category in DEVICE_USAGE_CATEGORIES {
        let key = format!(r"{DEVICE_USAGE_ROOT}\{category}");
        let value = values
            .iter()
            .find(|value| address_is(&value.address, &key, "Intent"))
            .ok_or_else(|| format!("device-usage transaction is missing {category} Intent"))?;
        require_managed_dword_zero(value, intent)?;
        priorities.push(RegistryAddress::new(
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            key,
            "Priority",
        ));
    }
    let consent_key = format!(r"{DEVICE_USAGE_ROOT}\OffDeviceConsent");
    let consent = values
        .iter()
        .find(|value| address_is(&value.address, &consent_key, "accepted"))
        .ok_or_else(|| "device-usage transaction is missing OffDeviceConsent".to_owned())?;
    require_managed_dword_zero(consent, intent)?;
    Ok(priorities)
}

fn validate_start_values(
    values: &[TransactionValue],
    intent: TransactionIntent,
) -> Result<(), String> {
    if values.len() != 1
        || !address_is(
            &values[0].address,
            EXPLORER_ADVANCED,
            "Start_IrisRecommendations",
        )
    {
        return Err(
            "Start verifier requires only Explorer\\Advanced Start_IrisRecommendations".into(),
        );
    }
    require_managed_dword_zero(&values[0], intent)
}

fn validate_device_usage_receipt(
    values: &[TransactionValue],
    intent: TransactionIntent,
    priorities: &[ReceiptSnapshot],
) -> Result<(), String> {
    let expected = device_usage_priority_addresses(values, intent)?;
    if priorities.len() != expected.len()
        || priorities
            .iter()
            .zip(expected)
            .any(|(recorded, expected)| recorded.address != expected)
    {
        return Err(
            "device-usage receipt must contain the seven Priority snapshots in order".into(),
        );
    }
    Ok(())
}

fn address_is(address: &RegistryAddress, key: &str, value: &str) -> bool {
    address.hive == RegistryHive::CurrentUser
        && address.view == RegistryView::Registry64
        && address.key.eq_ignore_ascii_case(key)
        && address.value.eq_ignore_ascii_case(value)
}

fn require_managed_dword_zero(
    value: &TransactionValue,
    intent: TransactionIntent,
) -> Result<(), String> {
    let expected = RegistrySnapshot::Present(RawRegistryValue::new(
        RegistryValueKind::Dword,
        0_u32.to_le_bytes(),
    ));
    let managed_side = match intent {
        TransactionIntent::Apply => &value.desired,
        TransactionIntent::Restore => &value.before,
    };
    if *managed_side == expected {
        Ok(())
    } else {
        Err(format!(
            "{} must have the managed-side DWORD 0 for this verifier",
            value.address,
        ))
    }
}
