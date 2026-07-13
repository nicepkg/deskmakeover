use std::collections::BTreeSet;

use crate::model::SettingDefinition;
use crate::{EngineError, ExpectedValue, ManagedSetting, ObservedValue};

pub(crate) fn validate_definition(definition: &SettingDefinition) -> Result<(), EngineError> {
    if definition.mutations.is_empty() {
        return Err(EngineError::InvalidDefinition(
            "recipe has no values".into(),
        ));
    }
    let unique = definition
        .mutations
        .iter()
        .map(|mutation| mutation.address.clone())
        .collect::<BTreeSet<_>>();
    if unique.len() != definition.mutations.len() {
        return Err(EngineError::InvalidDefinition(
            "recipe owns the same registry value twice".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_expected(
    observed: &[ObservedValue],
    expected: &[ExpectedValue],
) -> Result<(), EngineError> {
    if observed.len() != expected.len() {
        return Err(EngineError::InvalidDefinition(
            "request does not cover every recipe value exactly once".into(),
        ));
    }
    let unique = expected
        .iter()
        .map(|value| value.address.clone())
        .collect::<BTreeSet<_>>();
    if unique.len() != expected.len() {
        return Err(EngineError::InvalidDefinition(
            "request repeats a registry value".into(),
        ));
    }
    for value in observed {
        let Some(expected_value) = expected.iter().find(|item| item.address == value.address)
        else {
            return Err(EngineError::StaleObservation(value.address.clone()));
        };
        if expected_value.snapshot != value.snapshot {
            return Err(EngineError::StaleObservation(value.address.clone()));
        }
    }
    Ok(())
}

pub(crate) fn managed_matches(setting: &ManagedSetting, definition: &SettingDefinition) -> bool {
    let managed = setting
        .values
        .iter()
        .map(|value| &value.address)
        .collect::<BTreeSet<_>>();
    let recipe = definition
        .mutations
        .iter()
        .map(|value| &value.address)
        .collect::<BTreeSet<_>>();
    managed == recipe
}
