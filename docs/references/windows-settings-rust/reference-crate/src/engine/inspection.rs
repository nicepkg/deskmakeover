use crate::validation::{managed_matches, validate_definition};
use crate::{
    Capability, EngineError, JournalStore, MissingPolicy, RegistryBackend, RegistryError,
    RegistrySnapshot, RuntimeFacts, RuntimeProbe, SettingId, UnavailableReason,
    VerificationBackend,
};

use super::{Inspection, ObservedValue, SettingsEngine};

impl<B, J, V, R> SettingsEngine<B, J, V, R>
where
    B: RegistryBackend,
    J: JournalStore,
    V: VerificationBackend<B>,
    R: RuntimeProbe,
{
    /// Restore inspection uses the managed fingerprint and never applies current certification
    /// gates. Future builds must not hide an exact undo path.
    pub fn inspect_restore(&self, feature: &SettingId) -> Result<Inspection, EngineError> {
        let lease = self.journal.acquire_writer_lease()?;
        self.inspect_restore_locked(feature, &lease)
    }

    pub(super) fn inspect_restore_locked(
        &self,
        feature: &SettingId,
        lease: &J::Lease,
    ) -> Result<Inspection, EngineError> {
        let managed = self
            .journal
            .managed(lease, feature)?
            .ok_or_else(|| EngineError::NotManaged(feature.clone()))?;
        let values = managed
            .values
            .iter()
            .map(|value| {
                Ok(ObservedValue {
                    address: value.address.clone(),
                    snapshot: self.backend.read(&value.address)?,
                })
            })
            .collect::<Result<Vec<_>, RegistryError>>()?;
        let conflict = managed.values.iter().find(|managed_value| {
            values
                .iter()
                .find(|observed| observed.address == managed_value.address)
                .is_some_and(|observed| observed.snapshot != managed_value.last_applied)
        });
        let capability = conflict.map_or_else(
            || {
                self.recipe
                    .as_ref()
                    .filter(|recipe| recipe.id() == feature)
                    .map(|recipe| recipe.capability().clone())
                    .unwrap_or(Capability::Unavailable(
                        UnavailableReason::FeatureNotVerified,
                    ))
            },
            |value| {
                Capability::Unavailable(UnavailableReason::ExternalModification(
                    value.address.clone(),
                ))
            },
        );
        Ok(Inspection::new(
            feature.clone(),
            capability,
            true,
            values,
            managed.environment,
        ))
    }

    pub fn inspect(&self, feature: &SettingId) -> Result<Inspection, EngineError> {
        let lease = self.journal.acquire_writer_lease()?;
        self.inspect_locked(feature, &lease, None)
    }

    pub(super) fn inspect_locked(
        &self,
        feature: &SettingId,
        lease: &J::Lease,
        runtime: Option<&RuntimeFacts>,
    ) -> Result<Inspection, EngineError> {
        let base = self
            .recipe
            .as_ref()
            .filter(|recipe| recipe.id() == feature)
            .map(|recipe| recipe.capability().clone())
            .ok_or_else(|| EngineError::MissingDefinition(feature.clone()))?;
        let runtime = match runtime {
            Some(runtime) => runtime.clone(),
            None => self.enforce_apply_constraints()?,
        };
        if !base.permits_write() {
            return Ok(Inspection::new(
                feature.clone(),
                base,
                self.journal.managed(lease, feature)?.is_some(),
                Vec::new(),
                runtime.environment,
            ));
        }

        let definition = self.definition(feature)?;
        validate_definition(definition)?;
        let mut values = Vec::with_capacity(definition.mutations.len());
        for mutation in &definition.mutations {
            if self.backend.is_policy_managed(&mutation.address)? {
                return Ok(Inspection::new(
                    feature.clone(),
                    Capability::Unavailable(UnavailableReason::PolicyManaged(
                        mutation.address.clone(),
                    )),
                    self.journal.managed(lease, feature)?.is_some(),
                    values,
                    runtime.environment,
                ));
            }
            let snapshot = self.backend.read(&mutation.address)?;
            if !mutation.accepts(&snapshot) {
                if mutation.missing_policy == MissingPolicy::MustAlreadyExist
                    && matches!(
                        snapshot,
                        RegistrySnapshot::KeyMissing | RegistrySnapshot::ValueMissing
                    )
                {
                    return Err(EngineError::RequiredValueMissing(mutation.address.clone()));
                }
                let actual = snapshot
                    .kind()
                    .expect("only present values can mismatch")
                    .clone();
                return Ok(Inspection::new(
                    feature.clone(),
                    Capability::Unavailable(UnavailableReason::UnexpectedRegistryType {
                        address: mutation.address.clone(),
                        actual,
                    }),
                    self.journal.managed(lease, feature)?.is_some(),
                    values,
                    runtime.environment,
                ));
            }
            values.push(ObservedValue {
                address: mutation.address.clone(),
                snapshot,
            });
        }

        let managed = self.journal.managed(lease, feature)?;
        if let Some(setting) = &managed {
            let unavailable = if setting.recipe_version != definition.recipe_version {
                Some(UnavailableReason::RecipeVersionMismatch {
                    managed: setting.recipe_version,
                    selected: definition.recipe_version,
                })
            } else if setting.verification != definition.verification
                || !managed_matches(setting, definition)
            {
                Some(UnavailableReason::RecipeChangedWithoutVersionBump)
            } else {
                setting.values.iter().find_map(|value| {
                    let observed = values
                        .iter()
                        .find(|item| item.address == value.address)
                        .expect("address sets checked");
                    let selected = definition
                        .mutations
                        .iter()
                        .find(|mutation| mutation.address == value.address)
                        .expect("address sets checked");
                    if observed.snapshot != value.last_applied {
                        Some(UnavailableReason::ExternalModification(
                            value.address.clone(),
                        ))
                    } else if selected.desired != value.last_applied {
                        Some(UnavailableReason::RecipeChangedWithoutVersionBump)
                    } else {
                        None
                    }
                })
            };
            if let Some(reason) = unavailable {
                return Ok(Inspection::new(
                    feature.clone(),
                    Capability::Unavailable(reason),
                    true,
                    values,
                    runtime.environment,
                ));
            }
        }

        Ok(Inspection::new(
            feature.clone(),
            base,
            managed.is_some(),
            values,
            runtime.environment,
        ))
    }
}
