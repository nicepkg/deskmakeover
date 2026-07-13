use crate::{
    EngineError, JournalStore, RegistryBackend, RegistrySnapshot, RuntimeFacts, RuntimeProbe,
    UnavailableReason, VerificationBackend,
};

use super::SettingsEngine;

impl<B, J, V, R> SettingsEngine<B, J, V, R>
where
    B: RegistryBackend,
    J: JournalStore,
    V: VerificationBackend<B>,
    R: RuntimeProbe,
{
    /// Rechecked before prepare, before every write, and after terminal verification. Rollback and
    /// recovery intentionally never call this method: exact undo cannot be blocked by new policy.
    pub(super) fn enforce_apply_constraints(&self) -> Result<RuntimeFacts, EngineError> {
        let runtime = self.runtime.probe()?;
        self.enforce_apply_constraints_for(&runtime)?;
        Ok(runtime)
    }

    pub(super) fn enforce_apply_constraints_for(
        &self,
        runtime: &RuntimeFacts,
    ) -> Result<(), EngineError> {
        let recipe = self
            .recipe
            .as_ref()
            .ok_or_else(|| EngineError::InvalidDefinition("engine is restore-only".into()))?;
        recipe
            .ensure_unprotected()
            .map_err(|error| EngineError::InvalidDefinition(error.to_string()))?;
        recipe
            .ensure_runtime(runtime)
            .map_err(EngineError::Inapplicable)?;
        for guard in recipe.policy_guards() {
            if matches!(
                self.backend.read(&guard.address)?,
                RegistrySnapshot::Present(_)
            ) {
                return Err(EngineError::Unavailable(UnavailableReason::PolicyManaged(
                    guard.address.clone(),
                )));
            }
        }
        Ok(())
    }
}
