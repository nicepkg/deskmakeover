mod constraints;
mod error;
mod inspection;
mod receipt;
mod registry_keys;
mod registry_writes;
mod transaction_values;
mod transactions;
mod verification;

pub use error::EngineError;
pub use verification::{
    MemoryVerificationBackend, VerificationBackend, VerificationContext, VerificationError,
    VerificationExecutionMode, VerificationInvocation, VerificationPhase,
    VerificationPreparationContext, VerificationPreparationInvocation,
};

use crate::model::SettingDefinition;
use crate::{
    Capability, ExpectedValue, JournalStore, RegistryAddress, RegistryBackend, RegistrySnapshot,
    ResolvedRecipe, RuntimeProbe, SettingId, WindowsEnvironment,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedValue {
    pub address: RegistryAddress,
    pub snapshot: RegistrySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    pub feature: SettingId,
    pub capability: Capability,
    pub managed: bool,
    pub values: Vec<ObservedValue>,
    /// Full runtime fingerprint observed while the consistency lease was held.
    pub environment_fingerprint: WindowsEnvironment,
    proof: InspectionProof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectionProof;

impl Inspection {
    pub fn expected_values(&self) -> Vec<ExpectedValue> {
        self.values
            .iter()
            .map(|value| ExpectedValue {
                address: value.address.clone(),
                snapshot: value.snapshot.clone(),
            })
            .collect()
    }

    /// The only public constructor for an apply request. It binds CAS values and runtime profile
    /// to one prior leased inspection.
    pub fn apply_request(&self) -> crate::ApplyRequest {
        crate::ApplyRequest {
            feature: self.feature.clone(),
            expected: self.expected_values(),
            environment_fingerprint: self.environment_fingerprint.clone(),
        }
    }

    pub(super) fn new(
        feature: SettingId,
        capability: Capability,
        managed: bool,
        values: Vec<ObservedValue>,
        environment_fingerprint: WindowsEnvironment,
    ) -> Self {
        Self {
            feature,
            capability,
            managed,
            values,
            environment_fingerprint,
            proof: InspectionProof,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryConflict {
    pub transaction: u64,
    pub feature: SettingId,
    pub cause: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    pub recovered: Vec<u64>,
    pub conflicts: Vec<RecoveryConflict>,
}

/// Pure orchestration core. Every public observation or transaction obtains one journal lease.
pub struct SettingsEngine<B, J, V, R> {
    recipe: Option<ResolvedRecipe>,
    backend: B,
    journal: J,
    pub(super) verifier: V,
    pub(super) runtime: R,
}

impl<B, J, V, R> SettingsEngine<B, J, V, R>
where
    B: RegistryBackend,
    J: JournalStore,
    V: VerificationBackend<B>,
    R: RuntimeProbe,
{
    pub fn new(recipe: ResolvedRecipe, backend: B, journal: J, verifier: V, runtime: R) -> Self {
        Self {
            recipe: Some(recipe),
            backend,
            journal,
            verifier,
            runtime,
        }
    }

    /// Recovery and exact restore remain available without a currently writable recipe.
    pub fn restore_only(backend: B, journal: J, verifier: V, runtime: R) -> Self {
        Self {
            recipe: None,
            backend,
            journal,
            verifier,
            runtime,
        }
    }

    fn definition(&self, feature: &SettingId) -> Result<&SettingDefinition, EngineError> {
        self.recipe
            .as_ref()
            .filter(|recipe| recipe.id() == feature)
            .map(ResolvedRecipe::definition)
            .ok_or_else(|| EngineError::MissingDefinition(feature.clone()))
    }

    fn recipe_environment(&self) -> Result<&WindowsEnvironment, EngineError> {
        self.recipe
            .as_ref()
            .map(ResolvedRecipe::environment_fingerprint)
            .ok_or_else(|| EngineError::InvalidDefinition("engine is restore-only".into()))
    }
}

/// Explicit fake-only seams. Production backends cannot be extracted or mutated through engine.
impl<R> SettingsEngine<crate::MemoryRegistry, crate::MemoryJournal, MemoryVerificationBackend, R>
where
    R: RuntimeProbe,
{
    pub fn fake_registry(&self) -> &crate::MemoryRegistry {
        &self.backend
    }

    pub fn fake_registry_mut(&mut self) -> &mut crate::MemoryRegistry {
        &mut self.backend
    }

    pub fn fake_journal(&self) -> &crate::MemoryJournal {
        &self.journal
    }

    pub fn fake_journal_mut(&mut self) -> &mut crate::MemoryJournal {
        &mut self.journal
    }

    pub fn fake_verifier(&self) -> &MemoryVerificationBackend {
        &self.verifier
    }

    pub fn fake_verifier_mut(&mut self) -> &mut MemoryVerificationBackend {
        &mut self.verifier
    }

    pub fn fake_runtime(&self) -> &R {
        &self.runtime
    }

    pub fn into_fake_parts(
        self,
    ) -> (
        crate::MemoryRegistry,
        Option<ResolvedRecipe>,
        crate::MemoryJournal,
        MemoryVerificationBackend,
        R,
    ) {
        (
            self.backend,
            self.recipe,
            self.journal,
            self.verifier,
            self.runtime,
        )
    }
}
