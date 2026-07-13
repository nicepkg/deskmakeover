use std::{error::Error, fmt};

use crate::{Capability, RegistryAddress, RegistryError, SettingId};

use super::super::{ApplicabilityFailure, EvidenceLevel, FirstBatchTier, ProtectedReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirstBatchPlanError {
    DuplicateSettingId(SettingId),
    DuplicateMutationAddress {
        feature: SettingId,
        address: RegistryAddress,
    },
    ResourceCollision {
        address: RegistryAddress,
        first: SettingId,
        second: SettingId,
    },
    UnknownSetting(SettingId),
    InvalidTierEvidence {
        feature: SettingId,
        tier: FirstBatchTier,
        evidence: EvidenceLevel,
    },
    NonWritableTier {
        feature: SettingId,
        tier: FirstBatchTier,
    },
    CapabilityDenied {
        feature: SettingId,
        capability: Capability,
    },
    CertificationMismatch {
        feature: SettingId,
        tier: FirstBatchTier,
        capability: Capability,
    },
    Inapplicable {
        feature: SettingId,
        reason: ApplicabilityFailure,
    },
    MissingEffectVerifier(SettingId),
    NoPrimaryMutations(SettingId),
    Managed {
        feature: SettingId,
        address: RegistryAddress,
    },
    ForbiddenMutation {
        feature: SettingId,
        address: RegistryAddress,
        reason: &'static str,
    },
    ProtectedMutation {
        feature: SettingId,
        address: RegistryAddress,
        reason: ProtectedReason,
    },
    InvalidDwordBytes {
        feature: SettingId,
        address: RegistryAddress,
        length: usize,
    },
    RegistryAccess {
        address: RegistryAddress,
        source: Box<RegistryError>,
    },
}

impl fmt::Display for FirstBatchPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSettingId(id) => write!(f, "duplicate first-batch setting ID: {id}"),
            Self::DuplicateMutationAddress { feature, address } => {
                write!(f, "duplicate mutation address in {feature}: {address}")
            }
            Self::ResourceCollision {
                address,
                first,
                second,
            } => write!(
                f,
                "first-batch registry resource collision at {address}: {first} and {second}"
            ),
            Self::UnknownSetting(id) => write!(f, "unknown first-batch setting: {id}"),
            Self::InvalidTierEvidence {
                feature,
                tier,
                evidence,
            } => write!(
                f,
                "invalid first-batch tier/evidence pair for {feature}: {tier:?}/{evidence:?}"
            ),
            Self::NonWritableTier { feature, tier } => {
                write!(
                    f,
                    "first-batch setting {feature} has non-writable tier {tier:?}"
                )
            }
            Self::CapabilityDenied {
                feature,
                capability,
            } => {
                write!(
                    f,
                    "first-batch setting {feature} is not writable: {capability:?}"
                )
            }
            Self::CertificationMismatch {
                feature,
                tier,
                capability,
            } => write!(
                f,
                "certification result {capability:?} does not match {feature} tier {tier:?}"
            ),
            Self::Inapplicable { feature, reason } => {
                write!(
                    f,
                    "first-batch setting {feature} is inapplicable: {reason:?}"
                )
            }
            Self::MissingEffectVerifier(id) => write!(f, "{id} has no effect verifier"),
            Self::NoPrimaryMutations(id) => write!(f, "{id} has no primary mutations"),
            Self::Managed { feature, address } => write!(f, "{feature} is managed at {address}"),
            Self::ForbiddenMutation {
                feature,
                address,
                reason,
            } => {
                write!(
                    f,
                    "{feature} selected forbidden mutation {address}: {reason}"
                )
            }
            Self::ProtectedMutation {
                feature,
                address,
                reason,
            } => {
                write!(
                    f,
                    "{feature} selected protected mutation {address}: {reason:?}"
                )
            }
            Self::InvalidDwordBytes {
                feature,
                address,
                length,
            } => {
                write!(
                    f,
                    "{feature} has malformed DWORD at {address}: {length} bytes"
                )
            }
            Self::RegistryAccess { address, source } => {
                write!(f, "failed to inspect registry resource {address}: {source}")
            }
        }
    }
}

impl Error for FirstBatchPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RegistryAccess { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
