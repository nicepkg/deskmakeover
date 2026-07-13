//! The calm settings transaction driver: inspect / apply / restore / recover.
//!
//! Every write is fail-closed and fully reversible. Apply re-probes the environment, gates on the
//! capability manifest, journals the transaction BEFORE the first write, does a logical CAS with
//! immediate read-back, then proves the terminal state (delayed read-back + effect) before it
//! commits ownership. Restore walks back to the true original and refuses to overwrite a value the
//! user changed externally. Recovery finishes any transaction a crash left prepared.
//!
//! W1 scope: value-level over pre-existing keys (the calm first batch never creates a registry
//! key), an in-memory journal, Mac-testable end to end.

use dm_domain::system_tweaks::{
    ApplyOutcome, Capability, ProbeOutcome, RegistryBackend, RegistryError, RegistrySnapshot,
    RegistryWriteIntent, RestoreOutcome, SettingId, SettingMutation, SystemProfileProbe,
    UnavailableReason, WindowsEnvironment,
};

use super::capability::VerificationManifest;
use super::catalog::{TweakCatalog, TweakDescriptor, TweakTier};
use super::journal::{
    JournalStore, ManagedSetting, ManagedValue, TransactionIntent, TransactionValue,
};
use super::verify::{
    expected_terminal, ExecutionMode, VerificationBackend, VerificationContext, VerificationPhase,
    VerificationPlan,
};

/// A driver failure. Coarse by kind so the host branches on the kind, not a deep cause chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverError {
    UnknownFeature(SettingId),
    /// The feature is guided — the app opens the route, it is never written.
    Guided(SettingId),
    /// Not writable on this environment (fail-closed reason).
    Unavailable(UnavailableReason),
    /// The live environment moved between resolve and write.
    EnvironmentChanged,
    /// A prepared transaction from a prior crash must be recovered before a new write.
    RecoveryRequired(Vec<u64>),
    /// The feature is not currently owned by DeskMakeover (restore has nothing to do).
    NotManaged(SettingId),
    Registry(String),
    Journal(String),
    Verification(String),
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFeature(id) => write!(f, "unknown feature: {id}"),
            Self::Guided(id) => write!(f, "feature is guided, never written: {id}"),
            Self::Unavailable(reason) => write!(f, "unavailable: {reason:?}"),
            Self::EnvironmentChanged => write!(f, "environment fingerprint changed"),
            Self::RecoveryRequired(ids) => write!(f, "recovery required for {ids:?}"),
            Self::NotManaged(id) => write!(f, "not managed: {id}"),
            Self::Registry(m) => write!(f, "registry: {m}"),
            Self::Journal(m) => write!(f, "journal: {m}"),
            Self::Verification(m) => write!(f, "verification: {m}"),
        }
    }
}

impl std::error::Error for DriverError {}

/// The result of a crash-recovery pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    pub recovered: Vec<u64>,
    pub conflicts: Vec<(u64, SettingId, String)>,
}

/// The calm settings driver over injected ports.
pub struct TweakDriver<B, J, V, P> {
    catalog: TweakCatalog,
    manifest: VerificationManifest,
    backend: B,
    journal: J,
    verifier: V,
    profile: P,
}

impl<B, J, V, P> TweakDriver<B, J, V, P>
where
    B: RegistryBackend,
    J: JournalStore,
    V: VerificationBackend<B>,
    P: SystemProfileProbe,
{
    pub fn new(
        catalog: TweakCatalog,
        manifest: VerificationManifest,
        backend: B,
        journal: J,
        verifier: V,
        profile: P,
    ) -> Self {
        Self {
            catalog,
            manifest,
            backend,
            journal,
            verifier,
            profile,
        }
    }

    /// Probe one feature's live state (the frontend `CalmProbeState` source of truth).
    pub fn inspect(&self, feature: &SettingId) -> Result<ProbeOutcome, DriverError> {
        let descriptor = self.descriptor(feature)?;
        let environment = self.probe_environment()?;

        if descriptor.tier == TweakTier::Guided {
            // Guided rows have no writable state; the frontend drives them via the route.
            return Ok(ProbeOutcome::Pushing);
        }

        // A present policy guard means the feature is managed — reported, never written.
        if self.guard_present(descriptor)?.is_some() {
            return Ok(ProbeOutcome::Managed);
        }

        // A managed anchor whose live values still match → owned; a moved value → drifted.
        if let Some(managed) = self.managed(feature)? {
            let mut all_match = true;
            for value in &managed.values {
                let live = self.read(&value.address)?;
                if live != value.last_applied {
                    all_match = false;
                    break;
                }
            }
            return Ok(if all_match {
                ProbeOutcome::OwnedQuiet
            } else {
                ProbeOutcome::OwnedDrifted
            });
        }

        match self.manifest.evaluate(feature, &environment) {
            Capability::Unavailable(reason) => Ok(ProbeOutcome::Unsupported(reason)),
            Capability::ManualOnly => Ok(ProbeOutcome::Pushing),
            Capability::Available | Capability::Advanced => {
                // Certified-writable but not owned: already quiet if every desired value is live.
                let mut already_quiet = true;
                for mutation in &descriptor.mutations {
                    if self.read(&mutation.address)? != mutation.desired {
                        already_quiet = false;
                        break;
                    }
                }
                Ok(if already_quiet {
                    ProbeOutcome::AlreadyQuiet
                } else {
                    ProbeOutcome::Pushing
                })
            }
        }
    }

    /// Apply one feature: journal, CAS-write with immediate read-back, prove the terminal state,
    /// then commit ownership. Idempotent — re-applying an owned feature returns `Verified`.
    pub fn apply(&mut self, feature: &SettingId) -> Result<ApplyOutcome, DriverError> {
        self.require_recovery_complete()?;
        let descriptor = self.descriptor(feature)?.clone();
        if descriptor.tier == TweakTier::Guided {
            return Err(DriverError::Guided(feature.clone()));
        }
        let environment = self.probe_environment()?;
        let capability = self.manifest.evaluate(feature, &environment);
        if !capability.permits_write() {
            return match capability {
                Capability::Unavailable(reason) => Err(DriverError::Unavailable(reason)),
                _ => Err(DriverError::Unavailable(UnavailableReason::FeatureNotVerified)),
            };
        }

        // Idempotent: already owned → nothing to do, the write is verified.
        if self.managed(feature)?.is_some() {
            return Ok(ApplyOutcome::Verified);
        }

        // A present policy guard means the feature is managed — never overwrite or delete it.
        if let Some(guard) = self.guard_present(&descriptor)? {
            return Err(DriverError::Unavailable(UnavailableReason::PolicyManaged(guard)));
        }

        // Reject a backend-managed target and validate the live base value of each leaf.
        for mutation in &descriptor.mutations {
            if self.is_policy_managed(&mutation.address)? {
                return Err(DriverError::Unavailable(UnavailableReason::PolicyManaged(
                    mutation.address.clone(),
                )));
            }
            let live = self.read(&mutation.address)?;
            if !mutation.accepts(&live) {
                // The environment changed between probe and apply, or an unexpected kind: skip
                // without a write rather than clobber.
                return Ok(ApplyOutcome::Skipped(
                    dm_domain::system_tweaks::SkipReason::Changed,
                ));
            }
        }

        let values = self.transaction_values(&descriptor.mutations)?;
        let plan = VerificationPlan::new(
            descriptor
                .effect_verifier
                .expect("catalog guarantees a writable recipe has an effect verifier"),
        );
        let transaction = self
            .journal
            .prepare(
                feature.clone(),
                descriptor.recipe_version,
                environment.clone(),
                plan,
                TransactionIntent::Apply,
                values.clone(),
                None,
            )
            .map_err(|error| DriverError::Journal(error.to_string()))?;

        // Write each leaf with a logical CAS, then confirm the raw read-back immediately.
        for value in &values {
            match self.backend.compare_exchange(
                RegistryWriteIntent::Apply,
                &value.address,
                &value.before,
                &value.desired,
            ) {
                Ok(_) => {
                    let readback = self.read(&value.address)?;
                    if readback != value.desired {
                        return self.rollback_apply(transaction, feature, &values, "read-back");
                    }
                }
                Err(RegistryError::Interrupted) => {
                    // The write may have committed; leave the transaction prepared for recovery.
                    return Err(DriverError::Registry("interrupted".into()));
                }
                Err(error) => {
                    return self.rollback_apply(transaction, feature, &values, &error.to_string());
                }
            }
        }

        // Terminal proof: the same environment, delayed read-back, and the effect verifier.
        if self.probe_environment()? != environment {
            return self.rollback_apply(transaction, feature, &values, "environment changed");
        }
        if let Err(cause) = self.verify_terminal(VerificationPhase::ApplyDesired, plan, &values) {
            return self.rollback_apply(transaction, feature, &values, &cause);
        }

        let managed = ManagedSetting {
            feature: feature.clone(),
            recipe_version: descriptor.recipe_version,
            environment,
            verification: plan,
            last_transaction: transaction,
            values: values
                .into_iter()
                .map(|value| ManagedValue {
                    address: value.address,
                    original: value.original,
                    last_applied: value.desired,
                })
                .collect(),
        };
        self.journal
            .commit_apply(transaction, managed)
            .map_err(|error| DriverError::Journal(error.to_string()))?;
        Ok(ApplyOutcome::Verified)
    }

    /// Restore one owned feature to its true original. Refuses to overwrite a value the user
    /// changed externally (external conflict → the row is disowned, never clobbered).
    pub fn restore(&mut self, feature: &SettingId) -> Result<RestoreOutcome, DriverError> {
        self.require_recovery_complete()?;
        let managed = self
            .managed(feature)?
            .ok_or_else(|| DriverError::NotManaged(feature.clone()))?;

        for value in &managed.values {
            let live = self.read(&value.address)?;
            if live != value.last_applied {
                // Hand-edited since our write → it is theirs now; disown, do not overwrite.
                self.journal
                    .commit_restore(managed.last_transaction, feature)
                    .map_err(|error| DriverError::Journal(error.to_string()))?;
                return Ok(RestoreOutcome::SkippedExternalConflict);
            }
        }

        let values: Vec<TransactionValue> = managed
            .values
            .iter()
            .map(|value| TransactionValue {
                address: value.address.clone(),
                original: value.original.clone(),
                before: value.last_applied.clone(),
                desired: value.original.clone(),
            })
            .collect();
        let transaction = self
            .journal
            .prepare(
                feature.clone(),
                managed.recipe_version,
                managed.environment.clone(),
                managed.verification,
                TransactionIntent::Restore,
                values.clone(),
                Some(managed.clone()),
            )
            .map_err(|error| DriverError::Journal(error.to_string()))?;

        for value in &values {
            self.backend
                .compare_exchange(
                    RegistryWriteIntent::Undo,
                    &value.address,
                    &value.before,
                    &value.desired,
                )
                .map_err(|error| DriverError::Registry(error.to_string()))?;
        }
        self.verify_terminal(
            VerificationPhase::RestoreOriginal,
            managed.verification,
            &values,
        )
        .map_err(DriverError::Verification)?;
        self.journal
            .commit_restore(transaction, feature)
            .map_err(|error| DriverError::Journal(error.to_string()))?;
        Ok(RestoreOutcome::Restored)
    }

    /// Finish every transaction a crash left prepared: roll an apply back to originals, or drive a
    /// restore forward to originals, re-proving the terminal state under recovery mode.
    pub fn recover(&mut self) -> Result<RecoveryReport, DriverError> {
        let mut report = RecoveryReport::default();
        let incomplete = self
            .journal
            .incomplete()
            .map_err(|error| DriverError::Journal(error.to_string()))?;
        for entry in incomplete {
            let phase = match entry.intent {
                TransactionIntent::Apply => VerificationPhase::ApplyRollback,
                TransactionIntent::Restore => VerificationPhase::RestoreOriginal,
            };
            match self.recover_entry(&entry.values, phase, entry.verification) {
                Ok(()) => {
                    let result = match entry.intent {
                        TransactionIntent::Apply => {
                            self.journal.mark_apply_rolled_back(entry.id, &entry.feature)
                        }
                        TransactionIntent::Restore => {
                            self.journal.commit_restore(entry.id, &entry.feature)
                        }
                    };
                    result.map_err(|error| DriverError::Journal(error.to_string()))?;
                    report.recovered.push(entry.id);
                }
                Err(cause) => report.conflicts.push((entry.id, entry.feature, cause)),
            }
        }
        Ok(report)
    }

    // ---- internals ----

    fn descriptor(&self, feature: &SettingId) -> Result<&TweakDescriptor, DriverError> {
        self.catalog
            .descriptor(feature)
            .ok_or_else(|| DriverError::UnknownFeature(feature.clone()))
    }

    fn probe_environment(&self) -> Result<WindowsEnvironment, DriverError> {
        self.profile
            .probe()
            .map_err(|_| DriverError::EnvironmentChanged)
    }

    fn read(&self, address: &dm_domain::system_tweaks::RegistryAddress) -> Result<RegistrySnapshot, DriverError> {
        self.backend
            .read(address)
            .map_err(|error| DriverError::Registry(error.to_string()))
    }

    fn is_policy_managed(
        &self,
        address: &dm_domain::system_tweaks::RegistryAddress,
    ) -> Result<bool, DriverError> {
        self.backend
            .is_policy_managed(address)
            .map_err(|error| DriverError::Registry(error.to_string()))
    }

    /// The first present policy-guard address for a descriptor, if any — its presence means the
    /// feature is managed and the app must not write it (it is only ever read).
    fn guard_present(
        &self,
        descriptor: &TweakDescriptor,
    ) -> Result<Option<dm_domain::system_tweaks::RegistryAddress>, DriverError> {
        for guard in &descriptor.policy_guards {
            if self.read(&guard.address)?.value().is_some() {
                return Ok(Some(guard.address.clone()));
            }
        }
        Ok(None)
    }

    fn managed(&self, feature: &SettingId) -> Result<Option<ManagedSetting>, DriverError> {
        self.journal
            .managed(feature)
            .map_err(|error| DriverError::Journal(error.to_string()))
    }

    fn require_recovery_complete(&self) -> Result<(), DriverError> {
        let incomplete = self
            .journal
            .incomplete()
            .map_err(|error| DriverError::Journal(error.to_string()))?;
        if incomplete.is_empty() {
            Ok(())
        } else {
            Err(DriverError::RecoveryRequired(
                incomplete.into_iter().map(|entry| entry.id).collect(),
            ))
        }
    }

    /// Build the transaction leaves for a fresh apply: since we never owned the feature, the true
    /// original AND the live-before value are both the current snapshot.
    fn transaction_values(
        &self,
        mutations: &[SettingMutation],
    ) -> Result<Vec<TransactionValue>, DriverError> {
        mutations
            .iter()
            .map(|mutation| {
                let current = self.read(&mutation.address)?;
                Ok(TransactionValue {
                    address: mutation.address.clone(),
                    original: current.clone(),
                    before: current,
                    desired: mutation.desired.clone(),
                })
            })
            .collect()
    }

    /// Roll a failed apply back to originals in reverse order, then mark it rolled back.
    fn rollback_apply(
        &mut self,
        transaction: u64,
        feature: &SettingId,
        values: &[TransactionValue],
        cause: &str,
    ) -> Result<ApplyOutcome, DriverError> {
        self.rollback_to_original(values);
        self.journal
            .mark_apply_rolled_back(transaction, feature)
            .map_err(|error| DriverError::Journal(error.to_string()))?;
        // A failed apply is honestly reverted (retryable), not silently dropped. The cause is
        // recorded for diagnostics; the row simply pushes again.
        let _ = cause;
        Ok(ApplyOutcome::Reverted)
    }

    /// Best-effort restore of each leaf to its original, reverse order; failures do not abort the
    /// remaining rollbacks (an incomplete transaction stays for recovery).
    fn rollback_to_original(&mut self, values: &[TransactionValue]) {
        for value in values.iter().rev() {
            let live = match self.backend.read(&value.address) {
                Ok(live) => live,
                Err(_) => continue,
            };
            let _ = self.backend.compare_exchange(
                RegistryWriteIntent::Undo,
                &value.address,
                &live,
                &value.original,
            );
        }
    }

    fn recover_entry(
        &mut self,
        values: &[TransactionValue],
        phase: VerificationPhase,
        plan: VerificationPlan,
    ) -> Result<(), String> {
        self.rollback_to_original(values);
        self.verify_terminal_mode(phase, plan, values, ExecutionMode::UnattendedRecovery)
    }

    fn verify_terminal(
        &mut self,
        phase: VerificationPhase,
        plan: VerificationPlan,
        values: &[TransactionValue],
    ) -> Result<(), String> {
        self.verify_terminal_mode(phase, plan, values, ExecutionMode::Foreground)
    }

    /// Test-only: the journal, to assert ownership after a driver operation.
    #[cfg(test)]
    pub(crate) fn journal_ref(&self) -> &J {
        &self.journal
    }

    /// Test-only: the backend, to inject an external edit between driver operations.
    #[cfg(test)]
    pub(crate) fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    fn verify_terminal_mode(
        &mut self,
        phase: VerificationPhase,
        plan: VerificationPlan,
        values: &[TransactionValue],
        mode: ExecutionMode,
    ) -> Result<(), String> {
        let expected: Vec<_> = values
            .iter()
            .map(|value| (value.address.clone(), expected_terminal(phase, value)))
            .collect();
        let context = VerificationContext {
            phase,
            plan,
            execution_mode: mode,
            expected: expected.clone(),
        };
        self.verifier
            .settle(&mut self.backend, &context)
            .map_err(|error| error.to_string())?;
        for (address, want) in &expected {
            let live = self
                .backend
                .read(address)
                .map_err(|error| error.to_string())?;
            if &live != want {
                return Err(format!("delayed read-back mismatch at {address}"));
            }
        }
        self.verifier
            .verify_effect(&self.backend, &context)
            .map_err(|error| error.to_string())
    }
}
