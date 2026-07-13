//! 清爽 (calm-Windows) settings decision core.
//!
//! Rides the operations layer on top of the `dm-domain::system_tweaks` kernel: a validated
//! recipe catalog, a fail-closed capability manifest, and (in later slices) the WAL-journaled
//! apply/restore/recovery driver. Everything here is platform-agnostic — the real registry and
//! profile backends live in `dm-windows`, the Mac devhost supplies deterministic fakes.

pub mod capability;
pub mod catalog;

pub use capability::{
    StandardVerification, VerificationManifest, VerificationRule, VerifiedBuildFamily,
};
pub use catalog::{
    first_batch, CatalogError, EffectVerifier, ForbiddenMutation, ManualRoute, PolicyGuard,
    TweakCatalog, TweakDescriptor, TweakTier,
};
