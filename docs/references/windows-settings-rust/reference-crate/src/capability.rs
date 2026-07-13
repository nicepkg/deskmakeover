use std::collections::BTreeMap;

use crate::{RegistryAddress, RegistryValueKind, SettingId, WindowsEnvironment};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnavailableReason {
    NotWindows11,
    FeatureNotVerified,
    BuildNotVerified,
    FutureBuildNotVerified,
    EditionNotVerified,
    RegionNotVerified,
    RuntimeProfileNotVerified,
    AdvancedEnvironmentNotAllowlisted,
    RecipeVersionMismatch {
        managed: u32,
        selected: u32,
    },
    RecipeChangedWithoutVersionBump,
    PolicyManaged(RegistryAddress),
    UnexpectedRegistryType {
        address: RegistryAddress,
        actual: RegistryValueKind,
    },
    ExternalModification(RegistryAddress),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capability {
    Available,
    Advanced,
    ManualOnly,
    Unavailable(UnavailableReason),
}

impl Capability {
    pub fn permits_write(&self) -> bool {
        matches!(self, Self::Available | Self::Advanced)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBuildFamily {
    pub build: u32,
    pub min_ubr: u32,
    /// Production verification must set an upper bound. `None` deliberately fails closed so a
    /// copied recipe cannot silently accept a future servicing revision.
    pub max_ubr: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardVerification {
    /// Deliberately discrete. A numeric gap or Insider build is not implicitly accepted.
    pub families: Vec<VerifiedBuildFamily>,
    /// Exact runtime certification fingerprints. Empty fails closed. Each profile includes
    /// build/UBR/edition/region, native+process architecture, and package context so tested
    /// dimensions cannot accidentally form an untested Cartesian product.
    pub profiles: Vec<WindowsEnvironment>,
}

/// Advanced and standard verification share one complete certification fingerprint shape.
pub type ExactEnvironment = WindowsEnvironment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationRule {
    Standard(StandardVerification),
    /// Advanced features require an exact match across every certification fingerprint field.
    Advanced(Vec<ExactEnvironment>),
    /// The app may explain the manual route but must never write this feature.
    ManualOnly,
}

#[derive(Debug, Clone, Default)]
pub struct VerificationManifest {
    rules: BTreeMap<SettingId, VerificationRule>,
}

impl VerificationManifest {
    pub fn new(rules: impl IntoIterator<Item = (SettingId, VerificationRule)>) -> Self {
        Self {
            rules: rules.into_iter().collect(),
        }
    }

    pub fn evaluate(&self, feature: &SettingId, environment: &WindowsEnvironment) -> Capability {
        if !environment.is_windows_11() {
            return Capability::Unavailable(UnavailableReason::NotWindows11);
        }
        let Some(rule) = self.rules.get(feature) else {
            return Capability::Unavailable(UnavailableReason::FeatureNotVerified);
        };
        match rule {
            VerificationRule::ManualOnly => Capability::ManualOnly,
            VerificationRule::Advanced(allowlist) => {
                if allowlist
                    .iter()
                    .any(|allowed| allowed.matches_certification(environment))
                {
                    Capability::Advanced
                } else {
                    Capability::Unavailable(UnavailableReason::AdvancedEnvironmentNotAllowlisted)
                }
            }
            VerificationRule::Standard(range) => evaluate_standard(range, environment),
        }
    }
}

fn evaluate_standard(
    verification: &StandardVerification,
    environment: &WindowsEnvironment,
) -> Capability {
    let max_verified_build = verification
        .families
        .iter()
        .map(|family| family.build)
        .max()
        .unwrap_or(0);
    if environment.build > max_verified_build {
        return Capability::Unavailable(UnavailableReason::FutureBuildNotVerified);
    }
    let Some(family) = verification
        .families
        .iter()
        .find(|family| family.build == environment.build)
    else {
        return Capability::Unavailable(UnavailableReason::BuildNotVerified);
    };
    let Some(maximum_ubr) = family.max_ubr else {
        return Capability::Unavailable(UnavailableReason::BuildNotVerified);
    };
    if environment.ubr < family.min_ubr || environment.ubr > maximum_ubr {
        return Capability::Unavailable(UnavailableReason::BuildNotVerified);
    }
    if verification.profiles.is_empty()
        || !verification
            .profiles
            .iter()
            .any(|profile| profile.matches_certification(environment))
    {
        return Capability::Unavailable(UnavailableReason::RuntimeProfileNotVerified);
    }
    Capability::Available
}
