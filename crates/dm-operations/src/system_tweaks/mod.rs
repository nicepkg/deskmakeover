//! 清爽 (calm-Windows) settings decision core.
//!
//! Rides the operations layer on top of the `dm-domain::system_tweaks` kernel: a validated
//! recipe catalog, a fail-closed capability manifest, and (in later slices) the WAL-journaled
//! apply/restore/recovery driver. Everything here is platform-agnostic — the real registry and
//! profile backends live in `dm-windows`, the Mac devhost supplies deterministic fakes.

pub mod capability;
pub mod catalog;
pub mod driver;
pub mod engine;
pub mod fakes;
pub mod journal;
pub mod recovery;
pub mod verify;

#[cfg(test)]
mod tests;

pub use capability::{
    StandardVerification, VerificationManifest, VerificationRule, VerifiedBuildFamily,
};
pub use catalog::{
    first_batch, CatalogError, EffectVerifier, ForbiddenMutation, ManualRoute, PolicyGuard,
    TweakCatalog, TweakDescriptor, TweakTier,
};
pub use driver::{DriverError, TweakDriver};
pub use fakes::{MemoryProfileProbe, MemoryRegistry};
pub use journal::{
    JournalEntry, JournalError, JournalStore, ManagedSetting, ManagedValue, MemoryJournal,
    MemoryLease, PrepareRequest, TransactionIntent, TransactionState, TransactionValue,
    WriterLease,
};
pub use recovery::{RecoveryConflict, RecoveryReport};
pub use verify::{
    ExecutionMode, MemoryVerifier, VerificationBackend, VerificationBudget, VerificationContext,
    VerificationError, VerificationPhase, VerificationPlan, VerificationReceipt,
};
