use std::{error::Error, fmt};

use crate::{
    ApplicabilityFailure, JournalError, RegistryAddress, RegistryError, RuntimeProbeError,
    SettingId, UnavailableReason,
};

#[derive(Debug)]
pub enum EngineError {
    Unavailable(UnavailableReason),
    ManualOnly,
    MissingDefinition(SettingId),
    InvalidDefinition(String),
    StaleObservation(RegistryAddress),
    RequiredValueMissing(RegistryAddress),
    Inapplicable(ApplicabilityFailure),
    RuntimeProbe(RuntimeProbeError),
    EnvironmentFingerprintChanged,
    InvalidVerificationReceipt(String),
    NotManaged(SettingId),
    RecoveryRequired(Vec<u64>),
    Interrupted {
        transaction: u64,
    },
    ApplyFailed {
        transaction: u64,
        cause: String,
        rollback_complete: bool,
    },
    RestorePending {
        transaction: u64,
        cause: String,
    },
    Backend(RegistryError),
    Journal(JournalError),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(f, "feature unavailable: {reason:?}"),
            Self::ManualOnly => write!(f, "feature is manual-only"),
            Self::MissingDefinition(id) => write!(f, "missing setting definition: {id}"),
            Self::InvalidDefinition(message) => write!(f, "invalid setting definition: {message}"),
            Self::StaleObservation(address) => write!(f, "stale observation: {address}"),
            Self::RequiredValueMissing(address) => {
                write!(f, "required existing registry value is missing: {address}")
            }
            Self::Inapplicable(reason) => write!(f, "runtime applicability failed: {reason:?}"),
            Self::RuntimeProbe(error) => write!(f, "runtime probe failed: {error}"),
            Self::EnvironmentFingerprintChanged => {
                write!(
                    f,
                    "Windows environment fingerprint changed since inspection"
                )
            }
            Self::InvalidVerificationReceipt(message) => {
                write!(f, "invalid pre-write verification receipt: {message}")
            }
            Self::NotManaged(id) => write!(f, "setting is not managed: {id}"),
            Self::RecoveryRequired(transactions) => {
                write!(f, "recovery required for transactions {transactions:?}")
            }
            Self::Interrupted { transaction } => write!(f, "transaction {transaction} interrupted"),
            Self::ApplyFailed {
                transaction, cause, ..
            } => write!(f, "apply transaction {transaction} failed: {cause}"),
            Self::RestorePending { transaction, cause } => write!(
                f,
                "restore transaction {transaction} remains pending: {cause}"
            ),
            Self::Backend(error) => error.fmt(f),
            Self::Journal(error) => error.fmt(f),
        }
    }
}

impl Error for EngineError {}

impl From<RegistryError> for EngineError {
    fn from(value: RegistryError) -> Self {
        Self::Backend(value)
    }
}

impl From<JournalError> for EngineError {
    fn from(value: JournalError) -> Self {
        Self::Journal(value)
    }
}

impl From<RuntimeProbeError> for EngineError {
    fn from(value: RuntimeProbeError) -> Self {
        Self::RuntimeProbe(value)
    }
}
