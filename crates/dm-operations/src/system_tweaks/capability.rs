//! The verification manifest: the ONLY authority on whether a recipe is writable on the exact
//! environment in front of DeskMakeover. Fail-closed by construction — the initial manifest
//! grants NO direct write, so a copied recipe cannot silently accept an uncertified build.

use std::collections::BTreeMap;

use dm_domain::system_tweaks::{
    Capability, SettingId, UnavailableReason, WindowsEnvironment,
};

use super::catalog::{TweakDescriptor, TweakTier};

/// A discrete verified build family with lower AND upper UBR bounds. `max_ubr = None` fails
/// closed: a family with no upper bound can never accept a future servicing revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBuildFamily {
    pub build: u32,
    pub min_ubr: u32,
    pub max_ubr: Option<u32>,
}

/// Standard verification: discrete build families PLUS exact runtime profiles. Both must match;
/// an empty profile list fails closed so a family window alone never authorizes a write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardVerification {
    pub families: Vec<VerifiedBuildFamily>,
    pub profiles: Vec<WindowsEnvironment>,
}

/// How a feature is verified for one environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationRule {
    Standard(StandardVerification),
    /// An advanced feature: the exact environment must be on this allowlist.
    Advanced(Vec<WindowsEnvironment>),
    /// Never writable; the app may only explain the manual route.
    ManualOnly,
}

/// Maps each feature to its verification rule. A feature absent from the map is
/// `FeatureNotVerified` (fail closed).
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

    /// The initial manifest for a catalog: guided settings are `ManualOnly`; every writable
    /// recipe gets an EMPTY rule (Advanced allowlist empty, Standard profiles empty) so nothing
    /// is writable until a Windows VM certification run populates it. Automatic candidates are
    /// modelled as `Standard` with empty families+profiles (fail closed, same as an empty
    /// advanced allowlist), which keeps the "automatic vs advanced" distinction in the catalog
    /// tier while granting zero writes today.
    pub fn initial(catalog: &[TweakDescriptor]) -> Self {
        Self::new(catalog.iter().map(|descriptor| {
            let rule = match descriptor.tier {
                TweakTier::AutomaticCandidate => VerificationRule::Standard(StandardVerification {
                    families: Vec::new(),
                    profiles: Vec::new(),
                }),
                TweakTier::Advanced => VerificationRule::Advanced(Vec::new()),
                TweakTier::Guided => VerificationRule::ManualOnly,
            };
            (descriptor.id.clone(), rule)
        }))
    }

    /// Evaluate a feature against an environment.
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
    let Some(max_ubr) = family.max_ubr else {
        return Capability::Unavailable(UnavailableReason::BuildNotVerified);
    };
    if environment.ubr < family.min_ubr || environment.ubr > max_ubr {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_tweaks::catalog::first_batch;

    fn env(build: u32, ubr: u32) -> WindowsEnvironment {
        WindowsEnvironment {
            major: 10,
            minor: 0,
            build,
            ubr,
            display_version: "24H2".into(),
            edition_id: "Professional".into(),
            edition: dm_domain::system_tweaks::WindowsEdition::Pro,
            installation_type: "Client".into(),
            product_type: 48,
            is_workstation: true,
            region: "US".into(),
            native_architecture: "x64".into(),
            process_architecture: "x64".into(),
            packaged: false,
        }
    }

    #[test]
    fn the_initial_manifest_grants_no_write() {
        let manifest = VerificationManifest::initial(&first_batch());
        let environment = env(26_100, 8_737);
        for id in ["start.recommendations", "taskbar.search", "taskbar.taskview"] {
            let capability = manifest.evaluate(&SettingId::new(id), &environment);
            assert!(!capability.permits_write(), "{id} must be fail-closed");
        }
    }

    #[test]
    fn guided_features_evaluate_to_manual_only() {
        let manifest = VerificationManifest::initial(&first_batch());
        let capability =
            manifest.evaluate(&SettingId::new("widgets.feed"), &env(26_100, 8_737));
        assert_eq!(capability, Capability::ManualOnly);
    }

    #[test]
    fn a_non_windows_11_environment_is_unavailable() {
        let manifest = VerificationManifest::initial(&first_batch());
        let mut old = env(19_045, 1); // Windows 10
        old.build = 19_045;
        let capability = manifest.evaluate(&SettingId::new("taskbar.search"), &old);
        assert_eq!(
            capability,
            Capability::Unavailable(UnavailableReason::NotWindows11)
        );
    }

    #[test]
    fn a_certified_profile_makes_a_standard_recipe_available() {
        let environment = env(26_100, 8_737);
        let manifest = VerificationManifest::new([(
            SettingId::new("taskbar.search"),
            VerificationRule::Standard(StandardVerification {
                families: vec![VerifiedBuildFamily {
                    build: 26_100,
                    min_ubr: 8_000,
                    max_ubr: Some(9_000),
                }],
                profiles: vec![environment.clone()],
            }),
        )]);
        assert_eq!(
            manifest.evaluate(&SettingId::new("taskbar.search"), &environment),
            Capability::Available
        );
    }

    #[test]
    fn a_future_build_over_the_ceiling_fails_closed() {
        let manifest = VerificationManifest::new([(
            SettingId::new("taskbar.search"),
            VerificationRule::Standard(StandardVerification {
                families: vec![VerifiedBuildFamily {
                    build: 26_100,
                    min_ubr: 8_000,
                    max_ubr: Some(9_000),
                }],
                profiles: vec![env(26_100, 8_737)],
            }),
        )]);
        assert_eq!(
            manifest.evaluate(&SettingId::new("taskbar.search"), &env(26_200, 1)),
            Capability::Unavailable(UnavailableReason::FutureBuildNotVerified)
        );
    }
}
