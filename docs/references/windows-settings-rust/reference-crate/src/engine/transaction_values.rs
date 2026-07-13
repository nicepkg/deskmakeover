use crate::model::SettingDefinition;
use crate::{RegistryKey, RegistrySnapshot, TransactionValue};

use super::ObservedValue;

pub(super) fn transaction_values(
    definition: &SettingDefinition,
    observed: &[ObservedValue],
) -> Vec<TransactionValue> {
    definition
        .mutations
        .iter()
        .map(|mutation| {
            let before = observed
                .iter()
                .find(|value| value.address == mutation.address)
                .expect("definition and inspection share addresses")
                .snapshot
                .clone();
            TransactionValue {
                address: mutation.address.clone(),
                original: before.clone(),
                before,
                desired: mutation.desired.clone(),
            }
        })
        .collect()
}

pub(super) fn inherit_key_missing_originals(
    values: &mut [TransactionValue],
    inherited: &[RegistryKey],
) {
    for value in values {
        if value.original == RegistrySnapshot::ValueMissing
            && inherited.iter().any(|key| {
                key.hive == value.address.hive
                    && key.view == value.address.view
                    && key.path.eq_ignore_ascii_case(&value.address.key)
            })
        {
            value.original = RegistrySnapshot::KeyMissing;
        }
    }
}
