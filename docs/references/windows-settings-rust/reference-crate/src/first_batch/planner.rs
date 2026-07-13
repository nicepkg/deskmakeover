use std::collections::BTreeMap;

mod error;

pub use error::FirstBatchPlanError;

use crate::model::SettingDefinition;
use crate::{
    Capability, ExactEnvironment, RegistryAddress, RegistryBackend, RegistryError,
    RegistrySnapshot, RuntimeFacts, SettingId, SettingMutation, VerificationManifest,
    VerificationPlan, WindowsEnvironment,
};

use super::applicability;
use super::protection::ProtectedProfile;
use super::{
    Applicability, ApplicabilityFailure, AuxiliaryCondition, EvidenceLevel, FirstBatchSetting,
    FirstBatchTier, PolicyGuard,
};

/// A validated first-batch descriptor collection. Construction is the only supported path to a
/// writable resolution, so a `BTreeMap` can never silently overwrite a duplicate setting ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstBatchCatalog {
    descriptors: Vec<FirstBatchSetting>,
}

impl FirstBatchCatalog {
    pub fn try_new(
        descriptors: impl IntoIterator<Item = FirstBatchSetting>,
    ) -> Result<Self, FirstBatchPlanError> {
        let descriptors = descriptors.into_iter().collect::<Vec<_>>();
        validate_descriptors(&descriptors)?;
        Ok(Self { descriptors })
    }

    pub fn descriptors(&self) -> &[FirstBatchSetting] {
        &self.descriptors
    }

    pub fn descriptor(&self, id: &SettingId) -> Option<&FirstBatchSetting> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.id == *id)
    }

    /// Resolve a descriptor into the exact mutations an upper layer may hand to the transaction
    /// engine. This method never accepts registry paths or desired values from the caller.
    pub fn resolve<B: RegistryBackend>(
        &self,
        id: &SettingId,
        manifest: &VerificationManifest,
        runtime: &RuntimeFacts,
        registry: &B,
    ) -> Result<super::ResolvedRecipe, FirstBatchPlanError> {
        // Keep validation at the trust boundary even if a future constructor or deserializer is
        // added. The built-in catalog is small, so the defense-in-depth cost is negligible.
        validate_descriptors(&self.descriptors)?;

        let descriptor = self
            .descriptor(id)
            .ok_or_else(|| FirstBatchPlanError::UnknownSetting(id.clone()))?;
        validate_tier_and_evidence(descriptor)?;

        if matches!(
            descriptor.tier,
            FirstBatchTier::Guided | FirstBatchTier::Invariant
        ) {
            return Err(FirstBatchPlanError::NonWritableTier {
                feature: descriptor.id.clone(),
                tier: descriptor.tier,
            });
        }

        applicability::check(descriptor.applicability, runtime, runtime).map_err(|reason| {
            FirstBatchPlanError::Inapplicable {
                feature: descriptor.id.clone(),
                reason,
            }
        })?;

        let capability = manifest.evaluate(&descriptor.id, &runtime.environment);
        if !capability.permits_write() {
            return Err(FirstBatchPlanError::CapabilityDenied {
                feature: descriptor.id.clone(),
                capability,
            });
        }
        let expected_capability = match descriptor.tier {
            FirstBatchTier::AutomaticCandidate => Capability::Available,
            FirstBatchTier::Advanced => Capability::Advanced,
            FirstBatchTier::Guided | FirstBatchTier::Invariant => unreachable!("checked above"),
        };
        if capability != expected_capability {
            return Err(FirstBatchPlanError::CertificationMismatch {
                feature: descriptor.id.clone(),
                tier: descriptor.tier,
                capability,
            });
        }

        let effect_verifier = descriptor
            .effect_verifier
            .ok_or_else(|| FirstBatchPlanError::MissingEffectVerifier(descriptor.id.clone()))?;
        if descriptor.mutations.is_empty() {
            return Err(FirstBatchPlanError::NoPrimaryMutations(
                descriptor.id.clone(),
            ));
        }

        for guard in &descriptor.policy_guards {
            let snapshot = registry
                .read(&guard.address)
                .map_err(|source| registry_access_error(&guard.address, source))?;
            if matches!(snapshot, RegistrySnapshot::Present(_)) {
                return Err(FirstBatchPlanError::Managed {
                    feature: descriptor.id.clone(),
                    address: guard.address.clone(),
                });
            }
        }

        let mut selected = descriptor.mutations.clone();
        for auxiliary in &descriptor.auxiliary_mutations {
            match auxiliary.condition {
                AuxiliaryCondition::IfPresentAndExactEnvironmentVerified => {
                    if !auxiliary
                        .exact_environment_allowlist
                        .iter()
                        .any(|allowed| exact_environment_matches(allowed, &runtime.environment))
                    {
                        continue;
                    }
                    let snapshot =
                        registry
                            .read(&auxiliary.mutation.address)
                            .map_err(|source| {
                                registry_access_error(&auxiliary.mutation.address, source)
                            })?;
                    if matches!(snapshot, RegistrySnapshot::Present(_)) {
                        selected.push(auxiliary.mutation.clone());
                    }
                }
            }
        }

        reject_forbidden_mutations(&self.descriptors, descriptor, &selected)?;
        for mutation in &selected {
            if registry
                .is_policy_managed(&mutation.address)
                .map_err(|source| registry_access_error(&mutation.address, source))?
            {
                return Err(FirstBatchPlanError::Managed {
                    feature: descriptor.id.clone(),
                    address: mutation.address.clone(),
                });
            }
        }

        Ok(ResolvedRecipe {
            descriptor: descriptor.clone(),
            definition: SettingDefinition {
                id: descriptor.id.clone(),
                recipe_version: descriptor.recipe_version,
                mutations: selected,
                verification: VerificationPlan::new(effect_verifier),
            },
            capability,
            constraints: ExecutionConstraints {
                policy_guards: descriptor.policy_guards.clone(),
                applicability: descriptor.applicability,
                resolved_runtime: runtime.clone(),
                protection: ProtectedProfile::first_batch(),
            },
        })
    }
}

/// The complete descriptor is retained beside the selected engine definition, so policy guards,
/// forbidden addresses, fallbacks, evidence, and the post-apply effect verifier cannot be lost at
/// the production planning boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRecipe {
    descriptor: FirstBatchSetting,
    definition: SettingDefinition,
    capability: Capability,
    constraints: ExecutionConstraints,
}

impl ResolvedRecipe {
    pub fn id(&self) -> &SettingId {
        &self.definition.id
    }

    pub fn descriptor(&self) -> &FirstBatchSetting {
        &self.descriptor
    }

    pub fn selected_mutations(&self) -> &[SettingMutation] {
        &self.definition.mutations
    }

    pub fn selected_capability(&self) -> &Capability {
        &self.capability
    }

    pub fn effect_verifier(&self) -> crate::EffectVerifier {
        self.definition.verification.effect
    }

    pub(crate) fn definition(&self) -> &SettingDefinition {
        &self.definition
    }

    pub(crate) fn capability(&self) -> &Capability {
        &self.capability
    }

    pub(crate) fn environment_fingerprint(&self) -> &WindowsEnvironment {
        &self.constraints.resolved_runtime.environment
    }

    pub(crate) fn policy_guards(&self) -> &[PolicyGuard] {
        &self.constraints.policy_guards
    }

    pub(crate) fn ensure_runtime(
        &self,
        runtime: &RuntimeFacts,
    ) -> Result<(), ApplicabilityFailure> {
        applicability::check(
            self.constraints.applicability,
            &self.constraints.resolved_runtime,
            runtime,
        )
    }

    pub(crate) fn ensure_unprotected(&self) -> Result<(), FirstBatchPlanError> {
        for mutation in &self.definition.mutations {
            if let Some(reason) = self.constraints.protection.classify(&mutation.address) {
                return Err(FirstBatchPlanError::ProtectedMutation {
                    feature: self.definition.id.clone(),
                    address: mutation.address.clone(),
                    reason,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutionConstraints {
    policy_guards: Vec<PolicyGuard>,
    applicability: Applicability,
    resolved_runtime: RuntimeFacts,
    protection: ProtectedProfile,
}

fn validate_descriptors(descriptors: &[FirstBatchSetting]) -> Result<(), FirstBatchPlanError> {
    let mut ids = BTreeMap::<String, SettingId>::new();
    let mut owners = BTreeMap::<CanonicalAddress, SettingId>::new();
    for descriptor in descriptors {
        let canonical_id = descriptor.id.as_str().to_ascii_lowercase();
        if ids.insert(canonical_id, descriptor.id.clone()).is_some() {
            return Err(FirstBatchPlanError::DuplicateSettingId(
                descriptor.id.clone(),
            ));
        }

        let mut local = BTreeMap::<CanonicalAddress, RegistryAddress>::new();
        for mutation in all_candidate_mutations(descriptor) {
            if let RegistrySnapshot::Present(value) = &mutation.desired {
                if value.kind == crate::RegistryValueKind::Dword && value.bytes.len() != 4 {
                    return Err(FirstBatchPlanError::InvalidDwordBytes {
                        feature: descriptor.id.clone(),
                        address: mutation.address.clone(),
                        length: value.bytes.len(),
                    });
                }
            }
            if let Some(reason) = ProtectedProfile::first_batch().classify(&mutation.address) {
                return Err(FirstBatchPlanError::ProtectedMutation {
                    feature: descriptor.id.clone(),
                    address: mutation.address.clone(),
                    reason,
                });
            }
            let canonical = CanonicalAddress::from(&mutation.address);
            if local
                .insert(canonical.clone(), mutation.address.clone())
                .is_some()
            {
                return Err(FirstBatchPlanError::DuplicateMutationAddress {
                    feature: descriptor.id.clone(),
                    address: mutation.address.clone(),
                });
            }
            if let Some(first) = owners.insert(canonical, descriptor.id.clone()) {
                return Err(FirstBatchPlanError::ResourceCollision {
                    address: mutation.address.clone(),
                    first,
                    second: descriptor.id.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_tier_and_evidence(descriptor: &FirstBatchSetting) -> Result<(), FirstBatchPlanError> {
    let valid = matches!(
        (descriptor.tier, descriptor.evidence),
        (
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftContract | EvidenceLevel::MicrosoftImplementation
        ) | (FirstBatchTier::Advanced, EvidenceLevel::CommunityObserved)
            | (FirstBatchTier::Guided, EvidenceLevel::NoStableSetter)
            | (FirstBatchTier::Invariant, EvidenceLevel::MicrosoftContract)
    );
    if valid {
        Ok(())
    } else {
        Err(FirstBatchPlanError::InvalidTierEvidence {
            feature: descriptor.id.clone(),
            tier: descriptor.tier,
            evidence: descriptor.evidence,
        })
    }
}

fn reject_forbidden_mutations(
    descriptors: &[FirstBatchSetting],
    selected_descriptor: &FirstBatchSetting,
    selected: &[SettingMutation],
) -> Result<(), FirstBatchPlanError> {
    for mutation in selected {
        let canonical = CanonicalAddress::from(&mutation.address);
        if let Some(forbidden) = descriptors
            .iter()
            .flat_map(|descriptor| &descriptor.forbidden_mutations)
            .find(|forbidden| CanonicalAddress::from(&forbidden.address) == canonical)
        {
            return Err(FirstBatchPlanError::ForbiddenMutation {
                feature: selected_descriptor.id.clone(),
                address: mutation.address.clone(),
                reason: forbidden.reason,
            });
        }
    }
    Ok(())
}

fn all_candidate_mutations(
    descriptor: &FirstBatchSetting,
) -> impl Iterator<Item = &SettingMutation> {
    descriptor.mutations.iter().chain(
        descriptor
            .auxiliary_mutations
            .iter()
            .map(|auxiliary| &auxiliary.mutation),
    )
}

fn exact_environment_matches(allowed: &ExactEnvironment, environment: &WindowsEnvironment) -> bool {
    allowed.matches_certification(environment)
}

fn registry_access_error(address: &RegistryAddress, source: RegistryError) -> FirstBatchPlanError {
    FirstBatchPlanError::RegistryAccess {
        address: address.clone(),
        source: Box::new(source),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalAddress {
    hive: crate::RegistryHive,
    view: crate::RegistryView,
    key: String,
    value: String,
}

impl From<&RegistryAddress> for CanonicalAddress {
    fn from(address: &RegistryAddress) -> Self {
        Self {
            hive: address.hive,
            view: address.view,
            key: address.key.to_ascii_lowercase(),
            value: address.value.to_ascii_lowercase(),
        }
    }
}

#[cfg(test)]
mod tests;
