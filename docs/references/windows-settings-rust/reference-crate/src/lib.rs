//! Compile-tested reference core for reversible Windows settings.
//!
//! [`first_batch_settings`] contains typed, fail-closed recipe candidates. Production code still
//! supplies the real [`RegistryBackend`], durable journal, effect verifiers, and a VM-certified
//! manifest before any recipe becomes writable.

mod backend;
mod capability;
mod catalog;
mod engine;
mod first_batch;
mod journal;
mod model;
mod runtime;
mod validation;

pub use backend::{
    DeleteKeyOutcome, MemoryRegistry, RegistryBackend, RegistryError, RegistryWriteIntent,
    RegistryWriteOutcome,
};
pub use capability::{
    Capability, ExactEnvironment, StandardVerification, UnavailableReason, VerificationManifest,
    VerificationRule, VerifiedBuildFamily,
};
pub use catalog::DefaultSet;
pub use engine::{
    EngineError, Inspection, MemoryVerificationBackend, ObservedValue, RecoveryConflict,
    RecoveryReport, SettingsEngine, VerificationBackend, VerificationContext, VerificationError,
    VerificationExecutionMode, VerificationInvocation, VerificationPhase,
    VerificationPreparationContext, VerificationPreparationInvocation,
};
pub use first_batch::ids as first_batch_ids;
pub use first_batch::{
    first_batch_catalog, first_batch_settings, initial_verification_manifest, Applicability,
    ApplicabilityFailure, AuxiliaryCondition, AuxiliaryMutation, EffectVerifier, EvidenceLevel,
    FirstBatchCatalog, FirstBatchPlanError, FirstBatchSetting, FirstBatchTier, ForbiddenMutation,
    ManualFallback, PolicyGuard, ProtectedReason, ResolvedRecipe,
};
pub use journal::{
    JournalEntry, JournalError, JournalStore, ManagedSetting, ManagedValue, MemoryJournal,
    MemoryWriterLease, TransactionIntent, TransactionState, TransactionValue, WriterLease,
};
pub use model::{
    ApplyRequest, ExpectedValue, MissingPolicy, RawRegistryValue, ReceiptSnapshot, RegistryAddress,
    RegistryHive, RegistryKey, RegistrySnapshot, RegistryValueKind, RegistryView, RestoreRequest,
    SettingId, SettingMutation, VerificationBudget, VerificationPlan, VerificationReceipt,
    WindowsEdition, WindowsEnvironment,
};
pub use runtime::{
    LockScreenBackground, MemoryRuntimeProbe, RuntimeFacts, RuntimeProbe, RuntimeProbeError,
};
