//! The calm settings transaction driver: inspect / apply / restore. Recovery lives in
//! [`super::recovery`]; the shared rollback + terminal-verify engine internals live in
//! [`super::engine`].
//!
//! Every write is fail-closed and fully reversible. Apply captures a typed pre-write receipt,
//! journals the transaction under a writer lease BEFORE the first write, does a logical CAS with
//! immediate read-back, then proves the terminal state (delayed read-back + effect) before it
//! commits ownership. A failed apply rolls back to originals ONLY where the live value is one we
//! could have produced — never overwriting an external edit — and stays prepared for recovery when
//! it cannot cleanly finish. Restore walks back to the true original and disowns (never clobbers)
//! a value the user changed externally.
//!
//! W1 scope: value-level over pre-existing keys (the first batch creates no key), an in-memory
//! journal, Mac-testable end to end.

use dm_domain::system_tweaks::{
    ApplyOutcome, Capability, ProbeOutcome, RegistryAddress, RegistryBackend, RegistrySnapshot,
    RegistryWriteIntent, RestoreOutcome, SettingId, SettingMutation, SkipReason,
    SystemProfileProbe, UnavailableReason, WindowsEnvironment,
};

use super::capability::VerificationManifest;
use super::catalog::{TweakCatalog, TweakDescriptor, TweakTier};
use super::journal::{
    JournalStore, ManagedSetting, ManagedValue, PrepareRequest, TransactionIntent,
    TransactionValue,
};
use super::error::DriverError;
use super::verify::{VerificationBackend, VerificationPhase, VerificationPlan};

/// The calm settings driver over injected ports.
pub struct TweakDriver<B, J, V, P> {
    pub(super) catalog: TweakCatalog,
    pub(super) manifest: VerificationManifest,
    pub(super) backend: B,
    pub(super) journal: J,
    pub(super) verifier: V,
    pub(super) profile: P,
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
        let lease = self.lease()?;

        if descriptor.tier == TweakTier::Guided {
            return Ok(ProbeOutcome::Pushing);
        }
        if self.guard_present(descriptor)?.is_some() {
            return Ok(ProbeOutcome::Managed);
        }

        // An owned anchor: a crossed certification boundary outranks ownership; else intact →
        // owned-quiet, moved → drifted.
        if let Some(managed) = self.journal_managed(&lease, feature)? {
            if environment != managed.environment {
                return Ok(ProbeOutcome::NeedsReconfirm);
            }
            let intact = self.all_leaves_match(&managed)?;
            return Ok(if intact {
                ProbeOutcome::OwnedQuiet
            } else {
                ProbeOutcome::OwnedDrifted
            });
        }

        match self.manifest.evaluate(feature, &environment) {
            Capability::Unavailable(reason) => Ok(ProbeOutcome::Unsupported(reason)),
            Capability::ManualOnly => Ok(ProbeOutcome::Pushing),
            Capability::Available | Capability::Advanced => {
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

    /// Apply one feature: capture the receipt, journal, CAS-write with immediate read-back, prove
    /// the terminal state, then commit ownership. A previously-owned feature is re-established
    /// (its TRUE original is preserved), so a drifted row re-closes correctly.
    pub fn apply(&mut self, feature: &SettingId) -> Result<ApplyOutcome, DriverError> {
        let lease = self.lease()?;
        self.require_recovery_complete(&lease)?;
        let descriptor = self.descriptor(feature)?.clone();
        if descriptor.tier == TweakTier::Guided {
            return Err(DriverError::Guided(feature.clone()));
        }
        let environment = self.probe_environment()?;
        if let Some(guard) = self.guard_present(&descriptor)? {
            return Err(DriverError::Unavailable(UnavailableReason::PolicyManaged(guard)));
        }
        let capability = self.manifest.evaluate(feature, &environment);
        if !capability.permits_write() {
            return match capability {
                Capability::Unavailable(reason) => Err(DriverError::Unavailable(reason)),
                _ => Err(DriverError::Unavailable(UnavailableReason::FeatureNotVerified)),
            };
        }
        // The capability variant MUST match the descriptor's tier — a manifest that pairs an
        // Advanced recipe with a Standard rule (or vice versa) is a certification mismatch, never
        // a licence to write through the wrong gate (codex W1 #6).
        let expected = match descriptor.tier {
            TweakTier::AutomaticCandidate => Capability::Available,
            TweakTier::Advanced => Capability::Advanced,
            TweakTier::Guided => unreachable!("guided returned above"),
        };
        if capability != expected {
            return Err(DriverError::Unavailable(UnavailableReason::FeatureNotVerified));
        }

        let managed_before = self.journal_managed(&lease, feature)?;
        // A re-apply must target the SAME recipe (version + leaf addresses) the anchor was written
        // under; a recipe migration would otherwise orphan a leaf we still own with no restore
        // record (README 381-382). Require an explicit restore-before-reapply instead.
        if let Some(managed) = &managed_before {
            if managed.recipe_version != descriptor.recipe_version
                || !self.same_address_set(managed, &descriptor.mutations)
            {
                return Err(DriverError::MigrationRequired(feature.clone()));
            }
        }

        // Read + validate each leaf EXACTLY ONCE, and build the transaction from those SAME
        // snapshots — a "validate A then write B" second read could let an external kind change
        // slip past the shape check into the CAS expected (codex W1 R2 #8).
        let mut values = Vec::with_capacity(descriptor.mutations.len());
        for mutation in &descriptor.mutations {
            if self.is_backend_managed(&mutation.address)? {
                return Err(DriverError::Unavailable(UnavailableReason::PolicyManaged(
                    mutation.address.clone(),
                )));
            }
            let current = self.read(&mutation.address)?;
            if !mutation.accepts(&current) {
                return Ok(ApplyOutcome::Skipped(SkipReason::Changed));
            }
            // The TRUE original is preserved from the anchor on a re-apply, else it is the value we
            // just observed.
            let original = managed_before
                .as_ref()
                .and_then(|managed| {
                    managed
                        .values
                        .iter()
                        .find(|value| value.address == mutation.address)
                        .map(|value| value.original.clone())
                })
                .unwrap_or_else(|| current.clone());
            values.push(TransactionValue {
                address: mutation.address.clone(),
                original,
                before: current,
                desired: mutation.desired.clone(),
            });
        }

        let plan = VerificationPlan::new(
            descriptor
                .effect_verifier
                .expect("catalog guarantees a writable recipe has an effect verifier"),
        );
        // Receipt captured BEFORE the first write (contract 5).
        let receipt = self
            .verifier
            .prepare_receipt(&self.backend, plan)
            .map_err(|error| DriverError::Verification(error.to_string()))?;

        let transaction = self
            .journal
            .prepare(
                &lease,
                PrepareRequest {
                    feature: feature.clone(),
                    recipe_version: descriptor.recipe_version,
                    environment: environment.clone(),
                    verification: plan,
                    receipt: receipt.clone(),
                    intent: TransactionIntent::Apply,
                    values: values.clone(),
                    managed_before: managed_before.clone(),
                },
            )
            .map_err(|error| DriverError::Journal(error.to_string()))?;

        // Track the leaves we actually wrote, so a failure undoes ONLY those (a leaf a CAS conflict
        // never wrote is left untouched — codex W1 R2 #8/#2).
        let mut written: Vec<TransactionValue> = Vec::new();
        for value in &values {
            // Re-authenticate IMMEDIATELY before each write: the environment fingerprint, the
            // descriptor policy guards, and the leaf's backend-managed state must still hold — a
            // feature update or a policy that landed after prepare must NOT be written into (codex
            // W1 R3 NEW; README contract 3, the reference's per-leaf same-environment guard).
            if let Err(cause) = self.reauth_before_write(&descriptor, &environment, &value.address) {
                return self.fail_apply(&lease, transaction, feature, &values, &written, plan, &receipt, &cause);
            }
            match self.backend.compare_exchange(
                RegistryWriteIntent::Apply,
                &value.address,
                &value.before,
                &value.desired,
            ) {
                Ok(_) => {
                    written.push(value.clone());
                    if self.read(&value.address)? != value.desired {
                        return self.fail_apply(&lease, transaction, feature, &values, &written, plan, &receipt, "read-back");
                    }
                }
                Err(dm_domain::system_tweaks::RegistryError::Interrupted) => {
                    // The write may have committed; leave prepared for recovery.
                    return Err(DriverError::Interrupted(transaction));
                }
                Err(error) => {
                    return self.fail_apply(&lease, transaction, feature, &values, &written, plan, &receipt, &error.to_string());
                }
            }
        }

        if let Err(cause) =
            self.verify_terminal(VerificationPhase::ApplyDesired, plan, &receipt, &values)
        {
            return self.fail_apply(&lease, transaction, feature, &values, &written, plan, &receipt, &cause);
        }
        // A final re-authentication AFTER the terminal proof, BEFORE committing ownership: a feature
        // update during settle must not let us commit a Verified anchor on an uncertified env.
        if let Err(cause) = self.reauth(&descriptor, &environment) {
            return self.fail_apply(&lease, transaction, feature, &values, &written, plan, &receipt, &cause);
        }

        let managed = ManagedSetting {
            feature: feature.clone(),
            recipe_version: descriptor.recipe_version,
            environment,
            verification: plan,
            apply_receipt: receipt,
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
            .commit_apply(&lease, transaction, managed)
            .map_err(|error| DriverError::Journal(error.to_string()))?;
        Ok(ApplyOutcome::Verified)
    }

    /// Restore one owned feature to its true original. A value the user changed externally is
    /// disowned through a generation-guarded restore transaction — never overwritten.
    pub fn restore(&mut self, feature: &SettingId) -> Result<RestoreOutcome, DriverError> {
        let lease = self.lease()?;
        self.require_recovery_complete(&lease)?;
        let managed = self
            .journal_managed(&lease, feature)?
            .ok_or_else(|| DriverError::NotManaged(feature.clone()))?;

        let external_conflict = !self.all_leaves_match(&managed)?;
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
        // Capture a FRESH receipt for THIS restore before its first write, so the effect proof
        // uses a current baseline (the apply's receipt may be days stale) and recovery re-uses this
        // one (codex W1 R2 #4).
        let receipt = self
            .verifier
            .prepare_receipt(&self.backend, managed.verification)
            .map_err(|error| DriverError::Verification(error.to_string()))?;
        let transaction = self
            .journal
            .prepare(
                &lease,
                PrepareRequest {
                    feature: feature.clone(),
                    recipe_version: managed.recipe_version,
                    environment: managed.environment.clone(),
                    verification: managed.verification,
                    receipt: receipt.clone(),
                    intent: TransactionIntent::Restore,
                    values: values.clone(),
                    managed_before: Some(managed.clone()),
                },
            )
            .map_err(|error| DriverError::Journal(error.to_string()))?;

        if external_conflict {
            // Disown without writing — the user's values stand.
            self.journal
                .commit_restore(&lease, transaction, feature)
                .map_err(|error| DriverError::Journal(error.to_string()))?;
            return Ok(RestoreOutcome::SkippedExternalConflict);
        }

        // Clean restore: each live value is the last_applied we own → CAS it back to original.
        for value in &values {
            if let Err(error) = self.backend.compare_exchange(
                RegistryWriteIntent::Undo,
                &value.address,
                &value.before,
                &value.desired,
            ) {
                return Err(DriverError::Pending {
                    transaction,
                    cause: error.to_string(),
                });
            }
        }
        if let Err(cause) = self.verify_terminal(
            VerificationPhase::RestoreOriginal,
            managed.verification,
            &receipt,
            &values,
        ) {
            return Err(DriverError::Pending { transaction, cause });
        }
        self.journal
            .commit_restore(&lease, transaction, feature)
            .map_err(|error| DriverError::Journal(error.to_string()))?;
        Ok(RestoreOutcome::Restored)
    }

    // ---- internals shared with recovery / engine ----

    pub(super) fn descriptor(&self, feature: &SettingId) -> Result<&TweakDescriptor, DriverError> {
        self.catalog
            .descriptor(feature)
            .ok_or_else(|| DriverError::UnknownFeature(feature.clone()))
    }

    pub(super) fn lease(&self) -> Result<J::Lease, DriverError> {
        self.journal
            .acquire_writer_lease()
            .map_err(|error| DriverError::Journal(error.to_string()))
    }

    pub(super) fn probe_environment(&self) -> Result<WindowsEnvironment, DriverError> {
        self.profile
            .probe()
            .map_err(|error| DriverError::Profile(error.to_string()))
    }

    pub(super) fn read(&self, address: &RegistryAddress) -> Result<RegistrySnapshot, DriverError> {
        self.backend
            .read(address)
            .map_err(|error| DriverError::Registry(error.to_string()))
    }

    pub(super) fn is_backend_managed(&self, address: &RegistryAddress) -> Result<bool, DriverError> {
        self.backend
            .is_policy_managed(address)
            .map_err(|error| DriverError::Registry(error.to_string()))
    }

    pub(super) fn guard_present(
        &self,
        descriptor: &TweakDescriptor,
    ) -> Result<Option<RegistryAddress>, DriverError> {
        for guard in &descriptor.policy_guards {
            if self.read(&guard.address)?.value().is_some() {
                return Ok(Some(guard.address.clone()));
            }
        }
        Ok(None)
    }

    pub(super) fn journal_managed(
        &self,
        lease: &J::Lease,
        feature: &SettingId,
    ) -> Result<Option<ManagedSetting>, DriverError> {
        self.journal
            .managed(lease, feature)
            .map_err(|error| DriverError::Journal(error.to_string()))
    }

    /// Whether every owned leaf's live value still equals what we last applied.
    fn all_leaves_match(&self, managed: &ManagedSetting) -> Result<bool, DriverError> {
        for value in &managed.values {
            if self.read(&value.address)? != value.last_applied {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn require_recovery_complete(&self, lease: &J::Lease) -> Result<(), DriverError> {
        let incomplete = self
            .journal
            .incomplete(lease)
            .map_err(|error| DriverError::Journal(error.to_string()))?;
        if incomplete.is_empty() {
            Ok(())
        } else {
            Err(DriverError::RecoveryRequired(
                incomplete.into_iter().map(|entry| entry.id).collect(),
            ))
        }
    }

    /// Whether the anchor's leaf addresses are exactly the descriptor's current mutation addresses
    /// (a recipe migration that changed the leaf set must not silently re-apply — codex R2 #9).
    fn same_address_set(
        &self,
        managed: &ManagedSetting,
        mutations: &[SettingMutation],
    ) -> bool {
        if managed.values.len() != mutations.len() {
            return false;
        }
        mutations.iter().all(|mutation| {
            managed
                .values
                .iter()
                .any(|value| value.address == mutation.address)
        })
    }
}
