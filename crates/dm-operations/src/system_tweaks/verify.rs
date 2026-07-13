//! Terminal verification: a registry write is proven only by delayed raw read-back PLUS a
//! feature-specific effect check. A value matching in the registry is never sufficient on its
//! own — the surface (Start / Search / Settings) must have reloaded the change.

use dm_domain::system_tweaks::{RegistryBackend, RegistrySnapshot};

use super::catalog::EffectVerifier;

/// A bounded settle budget. `UnattendedRecovery` verifiers must never exceed it, never open UI,
/// and never wait for confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationBudget {
    pub max_settle_millis: u32,
    pub max_attempts: u8,
}

impl VerificationBudget {
    pub const DEFAULT: Self = Self {
        max_settle_millis: 5_000,
        max_attempts: 3,
    };

    pub fn is_bounded(self) -> bool {
        self.max_settle_millis > 0 && self.max_attempts > 0
    }
}

/// The durable, typed terminal-state requirement persisted in the journal before any write, so
/// recovery re-runs exactly the proof apply used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationPlan {
    pub effect: EffectVerifier,
    pub budget: VerificationBudget,
}

impl VerificationPlan {
    pub fn new(effect: EffectVerifier) -> Self {
        Self {
            effect,
            budget: VerificationBudget::DEFAULT,
        }
    }
}

/// Durable pre-write evidence captured BEFORE the first registry write and persisted in the
/// journal, so both terminal verification and crash recovery prove the SAME contract (research
/// reference contract 5). `StartKnownRecent` records a known Recent item that must survive the
/// write — a Start recipe must never claim promotions gone if it also cleared the user's Recent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationReceipt {
    /// No pre-write baseline is needed (delayed read-back + a stateless effect check suffice).
    NoBaseline,
    /// The known Recent marker Start must still show after the write.
    StartKnownRecent { marker: String },
}

impl VerificationReceipt {
    /// Whether this receipt shape satisfies the given effect verifier's evidence requirement.
    /// Recovery cannot substitute a weaker receipt than the plan demands.
    pub fn satisfies(&self, effect: EffectVerifier) -> bool {
        match effect {
            EffectVerifier::StartPromotionsAbsentAndKnownRecentPreserved => {
                matches!(self, Self::StartKnownRecent { .. })
            }
            EffectVerifier::DelayedReadBackAndSettingsUi
            | EffectVerifier::AdvertisingIdIsEmpty => matches!(self, Self::NoBaseline),
        }
    }
}

/// Which terminal state a verification run is proving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationPhase {
    /// Apply established the desired values.
    ApplyDesired,
    /// Apply failed and rolled back to originals.
    ApplyRollback,
    /// Restore returned to originals.
    RestoreOriginal,
}

/// Whether the verifier runs in the foreground (a user-driven apply/restore) or during
/// unattended crash recovery (finite budget, no UI, no waiting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Foreground,
    UnattendedRecovery,
}

/// A verification failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    Settle(String),
    Effect(String),
    /// The delayed read-back at `address` did not match the expected terminal value.
    DelayedMismatch {
        address: String,
        expected: Box<RegistrySnapshot>,
        actual: Box<RegistrySnapshot>,
    },
    Registry(String),
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settle(m) => write!(f, "settle failed: {m}"),
            Self::Effect(m) => write!(f, "effect verification failed: {m}"),
            Self::DelayedMismatch { address, .. } => {
                write!(f, "delayed read-back mismatch at {address}")
            }
            Self::Registry(m) => write!(f, "registry read during verify: {m}"),
        }
    }
}

impl std::error::Error for VerificationError {}

/// The context handed to a verifier: what terminal state to expect, under what plan, receipt, and
/// mode.
#[derive(Debug, Clone)]
pub struct VerificationContext {
    pub phase: VerificationPhase,
    pub plan: VerificationPlan,
    pub receipt: VerificationReceipt,
    pub execution_mode: ExecutionMode,
    /// The (address, expected-terminal-value) pairs delayed read-back must confirm.
    pub expected: Vec<(RegistryAddress, RegistrySnapshot)>,
}

use dm_domain::system_tweaks::RegistryAddress;

/// The platform effect verifier, injected into the driver. Real implementations query the live
/// surface (Search reload, Start reload, Advertising ID state); the Mac fake records invocations
/// and can be told to fail.
pub trait VerificationBackend<B: RegistryBackend> {
    /// Capture the typed pre-write receipt BEFORE the first registry write (contract 5). The plan
    /// selects which evidence is captured.
    fn prepare_receipt(
        &mut self,
        registry: &B,
        plan: VerificationPlan,
    ) -> Result<VerificationReceipt, VerificationError>;

    /// Wait for the change to settle (a real backend polls within the budget).
    fn settle(&mut self, registry: &mut B, context: &VerificationContext)
        -> Result<(), VerificationError>;

    /// The feature-specific effect proof, run AFTER delayed read-back passes, against the receipt.
    fn verify_effect(
        &mut self,
        registry: &B,
        context: &VerificationContext,
    ) -> Result<(), VerificationError>;
}

/// Deterministic verifier used by tests and the Mac devhost. Records every hook, tracks a known
/// Start Recent marker (which the effect check confirms survived), and can emulate a component
/// reverting a value while the engine waits for settle, or an effect check failing.
#[derive(Debug)]
pub struct MemoryVerifier {
    pub settle_calls: usize,
    pub effect_calls: usize,
    pub prepare_calls: usize,
    /// The known Recent item the Start receipt captures and the effect check re-confirms.
    start_recent_marker: String,
    next_settle_replacement: Option<(RegistryAddress, RegistrySnapshot)>,
    next_settle_failure: Option<String>,
    next_effect_failure: Option<String>,
}

impl Default for MemoryVerifier {
    fn default() -> Self {
        Self {
            settle_calls: 0,
            effect_calls: 0,
            prepare_calls: 0,
            start_recent_marker: "memory-known-recent".into(),
            next_settle_replacement: None,
            next_settle_failure: None,
            next_effect_failure: None,
        }
    }
}

impl MemoryVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// The next settle will overwrite `address` with `snapshot` (emulating a surface reverting).
    pub fn replace_on_next_settle(&mut self, address: RegistryAddress, snapshot: RegistrySnapshot) {
        self.next_settle_replacement = Some((address, snapshot));
    }

    pub fn fail_next_settle(&mut self, message: impl Into<String>) {
        self.next_settle_failure = Some(message.into());
    }

    pub fn fail_next_effect(&mut self, message: impl Into<String>) {
        self.next_effect_failure = Some(message.into());
    }

    /// Emulate the user's known Recent item changing between the receipt and the effect check
    /// (the Start effect proof must then fail — a promotions write must not clear Recent).
    pub fn set_start_recent_marker(&mut self, marker: impl Into<String>) {
        self.start_recent_marker = marker.into();
    }
}

impl<B: RegistryBackend> VerificationBackend<B> for MemoryVerifier {
    fn prepare_receipt(
        &mut self,
        _registry: &B,
        plan: VerificationPlan,
    ) -> Result<VerificationReceipt, VerificationError> {
        self.prepare_calls += 1;
        Ok(match plan.effect {
            EffectVerifier::StartPromotionsAbsentAndKnownRecentPreserved => {
                VerificationReceipt::StartKnownRecent {
                    marker: self.start_recent_marker.clone(),
                }
            }
            EffectVerifier::DelayedReadBackAndSettingsUi | EffectVerifier::AdvertisingIdIsEmpty => {
                VerificationReceipt::NoBaseline
            }
        })
    }

    fn settle(
        &mut self,
        registry: &mut B,
        _context: &VerificationContext,
    ) -> Result<(), VerificationError> {
        self.settle_calls += 1;
        if let Some(message) = self.next_settle_failure.take() {
            return Err(VerificationError::Settle(message));
        }
        if let Some((address, snapshot)) = self.next_settle_replacement.take() {
            // A test-only hook: the memory registry exposes compare_exchange, so emulate an
            // external replacement by writing through a fresh CAS against whatever is live.
            let live = registry
                .read(&address)
                .map_err(|error| VerificationError::Registry(error.to_string()))?;
            registry
                .compare_exchange(
                    dm_domain::system_tweaks::RegistryWriteIntent::Undo,
                    &address,
                    &live,
                    &snapshot,
                )
                .map_err(|error| VerificationError::Registry(error.to_string()))?;
        }
        Ok(())
    }

    fn verify_effect(
        &mut self,
        _registry: &B,
        context: &VerificationContext,
    ) -> Result<(), VerificationError> {
        self.effect_calls += 1;
        if let Some(message) = self.next_effect_failure.take() {
            return Err(VerificationError::Effect(message));
        }
        // Prove the receipt's contract, not just that the callback ran.
        if let VerificationReceipt::StartKnownRecent { marker } = &context.receipt {
            if marker != &self.start_recent_marker {
                return Err(VerificationError::Effect(
                    "known Recent item changed during a Start promotions write".into(),
                ));
            }
        }
        Ok(())
    }
}
