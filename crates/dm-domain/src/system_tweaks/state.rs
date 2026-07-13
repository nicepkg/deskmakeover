//! Capability gating and probe/apply outcome states for the calm settings core.
//!
//! These are the Rust truth the bridge projects onto the frontend's honest-state machine
//! (`src/lib/calm/states.ts`). A capability decides whether a write is even permitted on THIS
//! environment; an outcome reports what actually happened without ever over-claiming.

use serde::{Deserialize, Serialize};

use super::model::{RegistryAddress, RegistryValueKind, SettingId};
use super::environment::WindowsEnvironment;

/// Why a setting is not writable on the current environment. Every variant is a fail-closed
/// reason, never "probably fine".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnavailableReason {
    NotWindows11,
    /// No verification rule exists for this feature yet.
    FeatureNotVerified,
    BuildNotVerified,
    FutureBuildNotVerified,
    EditionNotVerified,
    RegionNotVerified,
    RuntimeProfileNotVerified,
    /// An advanced feature whose exact-environment allowlist does not contain this environment.
    AdvancedEnvironmentNotAllowlisted,
    /// A policy guard value is present → managed; never overwrite or delete it.
    PolicyManaged(RegistryAddress),
    /// The live value carries a registry kind the recipe does not accept.
    UnexpectedRegistryType {
        address: RegistryAddress,
        actual: RegistryValueKind,
    },
    /// The live value no longer matches DeskMakeover's recorded last-applied value.
    ExternalModification(RegistryAddress),
}

/// The result of evaluating a feature against the verification manifest for one environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    /// A standard automatic-candidate recipe certified on this exact environment.
    Available,
    /// An advanced recipe whose exact environment is on its allowlist.
    Advanced,
    /// The app may explain the manual route but must never write this feature.
    ManualOnly,
    /// Not writable; the reason is fail-closed.
    Unavailable(UnavailableReason),
}

impl Capability {
    /// Whether a write may proceed. Only certified `Available`/`Advanced` permit a write.
    pub fn permits_write(&self) -> bool {
        matches!(self, Self::Available | Self::Advanced)
    }
}

/// What a probe found for one setting on the live machine. This is the Rust source of the
/// frontend's `CalmProbeState`: the bridge never invents a state the probe did not report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeOutcome {
    /// Already off (matches the desired end state), and DeskMakeover did not write it.
    AlreadyQuiet,
    /// The surface still pushes content; the recipe could quiet it if certified.
    Pushing,
    /// Ledger-owned by DeskMakeover and the value still matches what it wrote.
    OwnedQuiet,
    /// Ledger-owned but the value moved since — drifted (HealthCheck re-propose).
    OwnedDrifted,
    /// Fail-closed: this environment is not certified for this feature.
    Unsupported(UnavailableReason),
    /// Policy/MDM managed — reported, never written.
    Managed,
    /// A certification boundary was crossed (feature update) — needs re-confirmation.
    NeedsReconfirm,
}

/// The result of an apply attempt for one setting. Mirrors the frontend apply outcomes; a write
/// that did not verifiably take effect is never reported as `Verified`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplyOutcome {
    /// Raw read-back + delayed read-back + the effect verifier all passed.
    Verified,
    /// Written, but the effect needs a sign-out / surface reopen before it is live.
    SetAwaiting,
    /// Apply failed and rolled back to the original; retryable.
    Reverted,
    /// No write happened because the environment changed between probe and apply.
    Skipped(SkipReason),
}

/// Why an apply skipped a setting without writing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    /// The live value changed between the probe and the apply (fail closed, no write).
    Changed,
}

/// The result of a restore attempt for one setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestoreOutcome {
    /// Restored to the true original.
    Restored,
    /// The live value differs from our recorded last-applied value → external conflict; the
    /// row is disowned rather than clobbered.
    SkippedExternalConflict,
}

/// A per-feature probe report the host returns for the whole catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeReport {
    pub feature: SettingId,
    pub outcome: ProbeOutcome,
}

/// The environment a probe report was captured under, carried so the frontend can show a
/// fail-closed reason without re-probing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeContext {
    pub environment: WindowsEnvironment,
    pub reports: Vec<ProbeReport>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_certified_capabilities_permit_a_write() {
        assert!(Capability::Available.permits_write());
        assert!(Capability::Advanced.permits_write());
        assert!(!Capability::ManualOnly.permits_write());
        assert!(!Capability::Unavailable(UnavailableReason::NotWindows11).permits_write());
    }

    #[test]
    fn outcomes_serialize_stably_for_the_bridge() {
        // The bridge maps these variants by name; a rename is a breaking schema change.
        let json = serde_json::to_string(&ProbeOutcome::OwnedDrifted).unwrap();
        assert_eq!(json, "\"OwnedDrifted\"");
        let skipped = serde_json::to_string(&ApplyOutcome::Skipped(SkipReason::Changed)).unwrap();
        assert_eq!(skipped, "{\"Skipped\":\"Changed\"}");
    }
}
