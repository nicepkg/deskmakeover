use std::error::Error;
use std::fmt;

use crate::{
    EffectVerifier, JournalStore, MemoryRegistry, ReceiptSnapshot, RegistryAddress,
    RegistryBackend, RegistryError, RegistryKey, RegistrySnapshot, RuntimeProbe, SettingId,
    TransactionIntent, TransactionValue, VerificationBudget, VerificationPlan, VerificationReceipt,
};

use super::receipt::{device_usage_priority_addresses, validate_receipt};
use super::SettingsEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationPhase {
    ApplyDesired,
    ApplyRollback,
    RestoreOriginal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationExecutionMode {
    Foreground,
    UnattendedRecovery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationPreparationContext {
    pub feature: SettingId,
    pub intent: TransactionIntent,
    pub plan: VerificationPlan,
    pub values: Vec<TransactionValue>,
    pub execution_mode: VerificationExecutionMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationContext {
    pub transaction: u64,
    pub feature: SettingId,
    pub phase: VerificationPhase,
    pub plan: VerificationPlan,
    pub receipt: VerificationReceipt,
    pub execution_mode: VerificationExecutionMode,
    pub budget: VerificationBudget,
    pub expected: Vec<(RegistryAddress, RegistrySnapshot)>,
}

#[derive(Clone, Copy)]
pub(super) struct VerificationRun<'a> {
    pub transaction: u64,
    pub feature: &'a SettingId,
    pub phase: VerificationPhase,
    pub plan: &'a VerificationPlan,
    pub receipt: &'a VerificationReceipt,
    pub execution_mode: VerificationExecutionMode,
    pub values: &'a [TransactionValue],
    /// Exact key locations intentionally retained because ownership is shared or unconfirmed.
    pub retained_keys: &'a [RegistryKey],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    Receipt(String),
    Settle(String),
    Effect(String),
    Registry(RegistryError),
    DelayedRegistryMismatch {
        address: RegistryAddress,
        expected: Box<RegistrySnapshot>,
        actual: Box<RegistrySnapshot>,
    },
}

impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Receipt(message) => write!(f, "verification receipt invalid: {message}"),
            Self::Settle(message) => write!(f, "verification settle failed: {message}"),
            Self::Effect(message) => write!(f, "effect verification failed: {message}"),
            Self::Registry(error) => error.fmt(f),
            Self::DelayedRegistryMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "delayed registry verification failed at {address}: expected {expected:?}, got {actual:?}"
            ),
        }
    }
}

impl Error for VerificationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Registry(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RegistryError> for VerificationError {
    fn from(value: RegistryError) -> Self {
        Self::Registry(value)
    }
}

/// Platform verifier explicitly injected into `SettingsEngine`.
///
/// `prepare_receipt` captures typed evidence before the journal WAL and before registry writes.
/// All hooks receive a finite budget. `UnattendedRecovery` implementations must never display UI,
/// wait for user confirmation, or retry beyond that budget.
pub trait VerificationBackend<B: RegistryBackend> {
    fn prepare_receipt(
        &mut self,
        registry: &B,
        context: &VerificationPreparationContext,
    ) -> Result<VerificationReceipt, VerificationError>;

    fn settle(
        &mut self,
        registry: &mut B,
        context: &VerificationContext,
    ) -> Result<(), VerificationError>;

    fn verify_effect(
        &mut self,
        registry: &B,
        context: &VerificationContext,
    ) -> Result<(), VerificationError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationInvocation {
    pub transaction: u64,
    pub feature: SettingId,
    pub phase: VerificationPhase,
    pub plan: VerificationPlan,
    pub receipt: VerificationReceipt,
    pub execution_mode: VerificationExecutionMode,
    pub budget: VerificationBudget,
}

impl From<&VerificationContext> for VerificationInvocation {
    fn from(context: &VerificationContext) -> Self {
        Self {
            transaction: context.transaction,
            feature: context.feature.clone(),
            phase: context.phase,
            plan: context.plan.clone(),
            receipt: context.receipt.clone(),
            execution_mode: context.execution_mode,
            budget: context.budget,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationPreparationInvocation {
    pub feature: SettingId,
    pub intent: TransactionIntent,
    pub plan: VerificationPlan,
    pub execution_mode: VerificationExecutionMode,
}

/// Deterministic verifier used by tests. It records every mandatory hook and can emulate a
/// component reverting a registry value while the engine is waiting for settings to settle.
#[derive(Debug)]
pub struct MemoryVerificationBackend {
    preparation_invocations: Vec<VerificationPreparationInvocation>,
    settle_invocations: Vec<VerificationInvocation>,
    effect_invocations: Vec<VerificationInvocation>,
    next_settle_replacement: Option<(RegistryAddress, RegistrySnapshot)>,
    next_settle_failure: Option<String>,
    next_effect_failure: Option<String>,
    start_known_recent_marker: String,
}

impl MemoryVerificationBackend {
    /// Intentionally explicit: production code must never obtain a verifier through a silent
    /// default, and tests should show the fake at the engine composition root.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            preparation_invocations: Vec::new(),
            settle_invocations: Vec::new(),
            effect_invocations: Vec::new(),
            next_settle_replacement: None,
            next_settle_failure: None,
            next_effect_failure: None,
            start_known_recent_marker: "memory-known-recent-marker".into(),
        }
    }

    pub fn replace_registry_on_next_settle(
        &mut self,
        address: RegistryAddress,
        snapshot: RegistrySnapshot,
    ) {
        self.next_settle_replacement = Some((address, snapshot));
    }

    pub fn fail_next_settle(&mut self, message: impl Into<String>) {
        self.next_settle_failure = Some(message.into());
    }

    pub fn fail_next_effect(&mut self, message: impl Into<String>) {
        self.next_effect_failure = Some(message.into());
    }

    pub fn set_start_known_recent_marker(&mut self, marker: impl Into<String>) {
        self.start_known_recent_marker = marker.into();
    }

    pub fn preparation_invocations(&self) -> &[VerificationPreparationInvocation] {
        &self.preparation_invocations
    }

    pub fn settle_invocations(&self) -> &[VerificationInvocation] {
        &self.settle_invocations
    }

    pub fn effect_invocations(&self) -> &[VerificationInvocation] {
        &self.effect_invocations
    }
}

impl VerificationBackend<MemoryRegistry> for MemoryVerificationBackend {
    fn prepare_receipt(
        &mut self,
        registry: &MemoryRegistry,
        context: &VerificationPreparationContext,
    ) -> Result<VerificationReceipt, VerificationError> {
        self.preparation_invocations
            .push(VerificationPreparationInvocation {
                feature: context.feature.clone(),
                intent: context.intent,
                plan: context.plan.clone(),
                execution_mode: context.execution_mode,
            });
        match context.plan.effect {
            EffectVerifier::StartPromotionsAbsentAndKnownRecentPreserved => {
                Ok(VerificationReceipt::StartKnownRecent {
                    marker: self.start_known_recent_marker.clone(),
                })
            }
            EffectVerifier::DeviceUsageAllOffAndPrioritiesPreserved => {
                let priorities = device_usage_priority_addresses(&context.values, context.intent)
                    .map_err(VerificationError::Receipt)?
                    .into_iter()
                    .map(|address| {
                        Ok(ReceiptSnapshot {
                            snapshot: registry.read(&address)?,
                            address,
                        })
                    })
                    .collect::<Result<Vec<_>, VerificationError>>()?;
                Ok(VerificationReceipt::DeviceUsagePriorities { priorities })
            }
            EffectVerifier::DelayedReadBackAndSettingsUi
            | EffectVerifier::SearchLocalNonceHasNoWebAffordance
            | EffectVerifier::AdvertisingIdIsEmpty => Ok(VerificationReceipt::NoBaseline),
        }
    }

    fn settle(
        &mut self,
        registry: &mut MemoryRegistry,
        context: &VerificationContext,
    ) -> Result<(), VerificationError> {
        self.settle_invocations.push(context.into());
        if let Some(message) = self.next_settle_failure.take() {
            return Err(VerificationError::Settle(message));
        }
        if let Some((address, snapshot)) = self.next_settle_replacement.take() {
            registry.set_snapshot(address, snapshot);
        }
        Ok(())
    }

    fn verify_effect(
        &mut self,
        registry: &MemoryRegistry,
        context: &VerificationContext,
    ) -> Result<(), VerificationError> {
        self.effect_invocations.push(context.into());
        if let Some(message) = self.next_effect_failure.take() {
            return Err(VerificationError::Effect(message));
        }
        match &context.receipt {
            VerificationReceipt::StartKnownRecent { marker } => {
                if marker != &self.start_known_recent_marker {
                    return Err(VerificationError::Effect(
                        "known Recent marker changed".into(),
                    ));
                }
            }
            VerificationReceipt::DeviceUsagePriorities { priorities } => {
                for priority in priorities {
                    if registry.read(&priority.address)? != priority.snapshot {
                        return Err(VerificationError::Effect(format!(
                            "device-usage Priority changed at {}",
                            priority.address
                        )));
                    }
                }
            }
            VerificationReceipt::NoBaseline => {}
        }
        Ok(())
    }
}

pub(super) fn context_for(run: &VerificationRun<'_>) -> VerificationContext {
    let expected = run
        .values
        .iter()
        .map(|value| {
            let snapshot = match run.phase {
                VerificationPhase::ApplyDesired => value.desired.clone(),
                VerificationPhase::ApplyRollback | VerificationPhase::RestoreOriginal => {
                    if value.original == RegistrySnapshot::KeyMissing
                        && run
                            .retained_keys
                            .iter()
                            .any(|key| same_key(key, &value.address.key_location()))
                    {
                        RegistrySnapshot::ValueMissing
                    } else {
                        value.original.clone()
                    }
                }
            };
            (value.address.clone(), snapshot)
        })
        .collect();
    VerificationContext {
        transaction: run.transaction,
        feature: run.feature.clone(),
        phase: run.phase,
        plan: run.plan.clone(),
        receipt: run.receipt.clone(),
        execution_mode: run.execution_mode,
        budget: run.plan.budget,
        expected,
    }
}

impl<B, J, V, R> SettingsEngine<B, J, V, R>
where
    B: RegistryBackend,
    J: JournalStore,
    V: VerificationBackend<B>,
    R: RuntimeProbe,
{
    pub(super) fn verify_terminal_state(
        &mut self,
        run: &VerificationRun<'_>,
    ) -> Result<(), VerificationError> {
        let intent = match run.phase {
            VerificationPhase::ApplyDesired | VerificationPhase::ApplyRollback => {
                TransactionIntent::Apply
            }
            VerificationPhase::RestoreOriginal => TransactionIntent::Restore,
        };
        validate_receipt(run.plan, intent, run.values, run.receipt)
            .map_err(VerificationError::Receipt)?;
        let context = context_for(run);
        self.verifier.settle(&mut self.backend, &context)?;
        for (address, expected) in &context.expected {
            let actual = self.backend.read(address)?;
            if &actual != expected {
                return Err(VerificationError::DelayedRegistryMismatch {
                    address: address.clone(),
                    expected: Box::new(expected.clone()),
                    actual: Box::new(actual),
                });
            }
        }
        self.verifier.verify_effect(&self.backend, &context)
    }

    pub(super) fn restore_originals_and_verify(
        &mut self,
        run: &VerificationRun<'_>,
        cleanup_keys: &[RegistryKey],
        retained_by_other_owners: &[RegistryKey],
        unconfirmed_candidates: &[RegistryKey],
    ) -> Result<(), String> {
        // Validate persisted evidence before recovery can perform its first registry write.
        let intent = match run.phase {
            VerificationPhase::ApplyDesired | VerificationPhase::ApplyRollback => {
                TransactionIntent::Apply
            }
            VerificationPhase::RestoreOriginal => TransactionIntent::Restore,
        };
        validate_receipt(run.plan, intent, run.values, run.receipt)?;
        self.rollback_to_original(run.values)
            .map_err(|error| error.to_string())?;
        // Other managed features can own a key inherited by this failed apply even though this
        // transaction did not create it. Include those keys in terminal expectations without ever
        // considering them eligible for cleanup.
        let mut retained = retained_by_other_owners.to_vec();
        retained.extend(
            self.cleanup_owned_keys(cleanup_keys, retained_by_other_owners)
                .map_err(|error| error.to_string())?,
        );
        retained.extend(
            self.existing_keys(unconfirmed_candidates)
                .map_err(|error| error.to_string())?,
        );
        retained.sort_by_key(|key| (key.depth(), key.path.to_ascii_lowercase()));
        retained.dedup_by(|left, right| same_key(left, right));
        let terminal = VerificationRun {
            retained_keys: &retained,
            ..*run
        };
        self.verify_terminal_state(&terminal)
            .map_err(|error| error.to_string())
    }
}

fn same_key(left: &RegistryKey, right: &RegistryKey) -> bool {
    left.hive == right.hive
        && left.view == right.view
        && left.path.eq_ignore_ascii_case(&right.path)
}
