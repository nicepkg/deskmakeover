use crate::model::SettingDefinition;
use crate::validation::validate_expected;
use crate::{
    ApplyRequest, Capability, JournalStore, ManagedSetting, ManagedValue, RegistryBackend,
    RegistryError, RegistryKey, RegistryWriteIntent, RestoreRequest, RuntimeFacts, RuntimeProbe,
    SettingId, TransactionIntent, TransactionValue, UnavailableReason, VerificationBackend,
    VerificationReceipt, WindowsEnvironment,
};

use super::receipt::validate_receipt;
use super::registry_keys::{
    inherited_cleanup_keys, merge_cleanup_keys, owned_by_other_features, unconfirmed_candidates,
};
use super::transaction_values::{inherit_key_missing_originals, transaction_values};
use super::verification::VerificationRun;
use super::{
    EngineError, ObservedValue, RecoveryConflict, RecoveryReport, SettingsEngine,
    VerificationExecutionMode, VerificationPhase, VerificationPreparationContext,
};

impl<B, J, V, R> SettingsEngine<B, J, V, R>
where
    B: RegistryBackend,
    J: JournalStore,
    V: VerificationBackend<B>,
    R: RuntimeProbe,
{
    pub fn apply(&mut self, request: ApplyRequest) -> Result<u64, EngineError> {
        // This is deliberately the first observation: the same token spans every later step.
        let lease = self.journal.acquire_writer_lease()?;
        self.apply_locked(request, &lease)
    }

    fn apply_locked(
        &mut self,
        request: ApplyRequest,
        lease: &J::Lease,
    ) -> Result<u64, EngineError> {
        self.require_recovery_complete(lease)?;
        let runtime = self.runtime.probe()?;
        self.require_apply_fingerprint(&request.environment_fingerprint, &runtime)?;
        self.enforce_apply_constraints_for(&runtime)?;
        let inspection = self.inspect_locked(&request.feature, lease, Some(&runtime))?;
        match &inspection.capability {
            Capability::Available | Capability::Advanced => {}
            Capability::ManualOnly => return Err(EngineError::ManualOnly),
            Capability::Unavailable(reason) => {
                return Err(EngineError::Unavailable(reason.clone()));
            }
        }
        validate_expected(&inspection.values, &request.expected)?;

        let definition = self.definition(&request.feature)?.clone();
        let prior_managed = self.journal.managed(lease, &request.feature)?;
        if let Some(setting) = &prior_managed {
            return Ok(setting.last_transaction);
        }
        let all_managed = self.journal.managed_all(lease)?;
        let mut values = transaction_values(&definition, &inspection.values);
        let inherited_keys = inherited_cleanup_keys(&values, &all_managed);
        inherit_key_missing_originals(&mut values, &inherited_keys);
        let receipt = self.prepare_receipt(
            &request.feature,
            TransactionIntent::Apply,
            &definition,
            &values,
        )?;
        let candidate_keys = self.missing_key_prefixes(&values)?;
        let transaction = self.journal.prepare(
            lease,
            request.feature.clone(),
            definition.recipe_version,
            runtime.environment.clone(),
            definition.verification.clone(),
            receipt.clone(),
            TransactionIntent::Apply,
            values.clone(),
            candidate_keys.clone(),
            inherited_keys.clone(),
            prior_managed,
        )?;

        let mut confirmed_created_keys = Vec::new();
        let retained_by_other_owners = owned_by_other_features(&request.feature, &all_managed);

        for value in &values {
            if let Err(error) = self.enforce_same_environment(&request.environment_fingerprint) {
                return self.fail_prepared_apply(
                    lease,
                    transaction,
                    &request.feature,
                    &definition,
                    &receipt,
                    &values,
                    &candidate_keys,
                    &confirmed_created_keys,
                    &retained_by_other_owners,
                    error.to_string(),
                );
            }
            let expected = if value.before == crate::RegistrySnapshot::KeyMissing
                && confirmed_created_keys.iter().any(|key| {
                    key.hive == value.address.hive
                        && key.view == value.address.view
                        && key.path.eq_ignore_ascii_case(&value.address.key)
                }) {
                crate::RegistrySnapshot::ValueMissing
            } else {
                value.before.clone()
            };
            match self.begin_write(
                RegistryWriteIntent::Apply,
                &value.address,
                &expected,
                &value.desired,
            ) {
                Ok(None) => {}
                Ok(Some((outcome, leaf_target))) => {
                    self.journal
                        .confirm_created_keys(lease, transaction, &outcome.created_keys)?;
                    merge_cleanup_keys(&mut confirmed_created_keys, &outcome.created_keys);
                    if let Err(error) = self.verify_immediate_readback(&value.address, &leaf_target)
                    {
                        return self.fail_prepared_apply(
                            lease,
                            transaction,
                            &request.feature,
                            &definition,
                            &receipt,
                            &values,
                            &candidate_keys,
                            &confirmed_created_keys,
                            &retained_by_other_owners,
                            error.to_string(),
                        );
                    }
                }
                Err(RegistryError::Interrupted) => {
                    return Err(EngineError::Interrupted { transaction });
                }
                Err(error) => {
                    return self.fail_prepared_apply(
                        lease,
                        transaction,
                        &request.feature,
                        &definition,
                        &receipt,
                        &values,
                        &candidate_keys,
                        &confirmed_created_keys,
                        &retained_by_other_owners,
                        error.to_string(),
                    );
                }
            }
        }

        let terminal = VerificationRun {
            transaction,
            feature: &request.feature,
            phase: VerificationPhase::ApplyDesired,
            plan: &definition.verification,
            receipt: &receipt,
            execution_mode: VerificationExecutionMode::Foreground,
            values: &values,
            retained_keys: &[],
        };
        if let Err(error) = self.verify_terminal_state(&terminal) {
            return self.fail_prepared_apply(
                lease,
                transaction,
                &request.feature,
                &definition,
                &receipt,
                &values,
                &candidate_keys,
                &confirmed_created_keys,
                &retained_by_other_owners,
                error.to_string(),
            );
        }
        if let Err(error) = self.enforce_same_environment(&request.environment_fingerprint) {
            return self.fail_prepared_apply(
                lease,
                transaction,
                &request.feature,
                &definition,
                &receipt,
                &values,
                &candidate_keys,
                &confirmed_created_keys,
                &retained_by_other_owners,
                error.to_string(),
            );
        }

        let mut cleanup_owned_keys = inherited_keys;
        merge_cleanup_keys(&mut cleanup_owned_keys, &confirmed_created_keys);
        let managed = ManagedSetting {
            feature: request.feature,
            recipe_version: definition.recipe_version,
            environment: runtime.environment,
            verification: definition.verification,
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
            cleanup_owned_keys,
        };
        self.journal.commit_apply(lease, transaction, managed)?;
        Ok(transaction)
    }

    pub fn restore(&mut self, request: RestoreRequest) -> Result<u64, EngineError> {
        let lease = self.journal.acquire_writer_lease()?;
        self.require_recovery_complete(&lease)?;
        let managed = self
            .journal
            .managed(&lease, &request.feature)?
            .ok_or_else(|| EngineError::NotManaged(request.feature.clone()))?;
        let all_managed = self.journal.managed_all(&lease)?;
        let retained_by_other_owners = owned_by_other_features(&request.feature, &all_managed);
        let observed = managed
            .values
            .iter()
            .map(|value| {
                Ok(ObservedValue {
                    address: value.address.clone(),
                    snapshot: self.backend.read(&value.address)?,
                })
            })
            .collect::<Result<Vec<_>, RegistryError>>()?;
        validate_expected(&observed, &request.expected)?;
        for value in &managed.values {
            let current = observed
                .iter()
                .find(|item| item.address == value.address)
                .expect("managed and observed share addresses");
            if current.snapshot != value.last_applied {
                return Err(EngineError::Unavailable(
                    UnavailableReason::ExternalModification(value.address.clone()),
                ));
            }
        }

        let values = managed
            .values
            .iter()
            .map(|value| TransactionValue {
                address: value.address.clone(),
                original: value.original.clone(),
                before: value.last_applied.clone(),
                desired: value.original.clone(),
            })
            .collect::<Vec<_>>();
        let receipt = self.prepare_receipt_from_plan(
            &request.feature,
            TransactionIntent::Restore,
            &managed.verification,
            &values,
        )?;
        let transaction = self.journal.prepare(
            &lease,
            request.feature.clone(),
            managed.recipe_version,
            managed.environment.clone(),
            managed.verification.clone(),
            receipt.clone(),
            TransactionIntent::Restore,
            values.clone(),
            Vec::new(),
            managed.cleanup_owned_keys.clone(),
            Some(managed.clone()),
        )?;
        for value in &values {
            if let Err(error) = self.write_or_verify(
                RegistryWriteIntent::Undo,
                &value.address,
                &value.before,
                &value.original,
            ) {
                if error == RegistryError::Interrupted {
                    return Err(EngineError::Interrupted { transaction });
                }
                return Err(EngineError::RestorePending {
                    transaction,
                    cause: error.to_string(),
                });
            }
        }
        let retained_keys =
            match self.cleanup_owned_keys(&managed.cleanup_owned_keys, &retained_by_other_owners) {
                Ok(retained) => retained,
                Err(error) => {
                    return Err(EngineError::RestorePending {
                        transaction,
                        cause: error.to_string(),
                    });
                }
            };
        let terminal = VerificationRun {
            transaction,
            feature: &request.feature,
            phase: VerificationPhase::RestoreOriginal,
            plan: &managed.verification,
            receipt: &receipt,
            execution_mode: VerificationExecutionMode::Foreground,
            values: &values,
            retained_keys: &retained_keys,
        };
        if let Err(error) = self.verify_terminal_state(&terminal) {
            return Err(EngineError::RestorePending {
                transaction,
                cause: error.to_string(),
            });
        }
        self.journal
            .commit_restore(&lease, transaction, &request.feature)?;
        Ok(transaction)
    }

    pub fn recover(&mut self) -> Result<RecoveryReport, EngineError> {
        let lease = self.journal.acquire_writer_lease()?;
        let mut report = RecoveryReport::default();
        for entry in self.journal.incomplete(&lease)? {
            let all_managed = self.journal.managed_all(&lease)?;
            let retained_by_other_owners = owned_by_other_features(&entry.feature, &all_managed);
            let phase = match entry.intent {
                TransactionIntent::Apply => VerificationPhase::ApplyRollback,
                TransactionIntent::Restore => VerificationPhase::RestoreOriginal,
            };
            let recovery = VerificationRun {
                transaction: entry.id,
                feature: &entry.feature,
                phase,
                plan: &entry.verification,
                receipt: &entry.receipt,
                execution_mode: VerificationExecutionMode::UnattendedRecovery,
                values: &entry.values,
                retained_keys: &[],
            };
            let (cleanup_keys, unconfirmed) = match entry.intent {
                TransactionIntent::Apply => (
                    entry.confirmed_created_keys.clone(),
                    unconfirmed_candidates(&entry.candidate_keys, &entry.confirmed_created_keys),
                ),
                TransactionIntent::Restore => (entry.cleanup_owned_keys.clone(), Vec::new()),
            };
            match self.restore_originals_and_verify(
                &recovery,
                &cleanup_keys,
                &retained_by_other_owners,
                &unconfirmed,
            ) {
                Ok(()) => {
                    match entry.intent {
                        TransactionIntent::Apply => {
                            self.journal
                                .mark_apply_rolled_back(&lease, entry.id, &entry.feature)?
                        }
                        TransactionIntent::Restore => {
                            self.journal
                                .commit_restore(&lease, entry.id, &entry.feature)?
                        }
                    }
                    report.recovered.push(entry.id);
                }
                Err(error) => report.conflicts.push(RecoveryConflict {
                    transaction: entry.id,
                    feature: entry.feature,
                    cause: error,
                }),
            }
        }
        Ok(report)
    }

    fn prepare_receipt(
        &mut self,
        feature: &SettingId,
        intent: TransactionIntent,
        definition: &SettingDefinition,
        values: &[TransactionValue],
    ) -> Result<VerificationReceipt, EngineError> {
        self.prepare_receipt_from_plan(feature, intent, &definition.verification, values)
    }

    fn prepare_receipt_from_plan(
        &mut self,
        feature: &SettingId,
        intent: TransactionIntent,
        plan: &crate::VerificationPlan,
        values: &[TransactionValue],
    ) -> Result<VerificationReceipt, EngineError> {
        let context = VerificationPreparationContext {
            feature: feature.clone(),
            intent,
            plan: plan.clone(),
            values: values.to_vec(),
            execution_mode: VerificationExecutionMode::Foreground,
        };
        let receipt = self
            .verifier
            .prepare_receipt(&self.backend, &context)
            .map_err(|error| EngineError::InvalidVerificationReceipt(error.to_string()))?;
        validate_receipt(plan, intent, values, &receipt)
            .map_err(EngineError::InvalidVerificationReceipt)?;
        Ok(receipt)
    }

    #[allow(clippy::too_many_arguments)]
    fn fail_prepared_apply<T>(
        &mut self,
        lease: &J::Lease,
        transaction: u64,
        feature: &SettingId,
        definition: &SettingDefinition,
        receipt: &VerificationReceipt,
        values: &[TransactionValue],
        candidate_keys: &[RegistryKey],
        confirmed_created_keys: &[RegistryKey],
        retained_by_other_owners: &[RegistryKey],
        cause: String,
    ) -> Result<T, EngineError> {
        let rollback = VerificationRun {
            transaction,
            feature,
            phase: VerificationPhase::ApplyRollback,
            plan: &definition.verification,
            receipt,
            execution_mode: VerificationExecutionMode::Foreground,
            values,
            retained_keys: &[],
        };
        let unconfirmed = unconfirmed_candidates(candidate_keys, confirmed_created_keys);
        let rollback_complete = self
            .restore_originals_and_verify(
                &rollback,
                confirmed_created_keys,
                retained_by_other_owners,
                &unconfirmed,
            )
            .is_ok();
        if rollback_complete {
            self.journal
                .mark_apply_rolled_back(lease, transaction, feature)?;
        }
        Err(EngineError::ApplyFailed {
            transaction,
            cause,
            rollback_complete,
        })
    }

    fn require_recovery_complete(&self, lease: &J::Lease) -> Result<(), EngineError> {
        let incomplete = self.journal.incomplete(lease)?;
        if incomplete.is_empty() {
            Ok(())
        } else {
            Err(EngineError::RecoveryRequired(
                incomplete.into_iter().map(|entry| entry.id).collect(),
            ))
        }
    }

    fn require_apply_fingerprint(
        &self,
        inspected: &WindowsEnvironment,
        current: &RuntimeFacts,
    ) -> Result<(), EngineError> {
        if inspected != &current.environment || self.recipe_environment()? != &current.environment {
            return Err(EngineError::EnvironmentFingerprintChanged);
        }
        Ok(())
    }

    fn enforce_same_environment(&self, inspected: &WindowsEnvironment) -> Result<(), EngineError> {
        let current = self.runtime.probe()?;
        self.require_apply_fingerprint(inspected, &current)?;
        self.enforce_apply_constraints_for(&current)
    }
}
