//! The calm settings driver failure type, coarse by kind so the host branches on the KIND of
//! failure (unavailable? pending recovery? migration?), never a Win32 code or a deep cause chain.

use dm_domain::system_tweaks::{SettingId, UnavailableReason};

/// A driver failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverError {
    UnknownFeature(SettingId),
    /// The feature is guided — the app opens the route, it is never written.
    Guided(SettingId),
    /// Not writable on this environment (fail-closed reason).
    Unavailable(UnavailableReason),
    /// A prepared transaction from a prior crash must be recovered before a new write.
    RecoveryRequired(Vec<u64>),
    /// The feature is not currently owned by DeskMakeover (restore has nothing to do).
    NotManaged(SettingId),
    /// The owned recipe changed (version or leaf set) — restore-before-reapply is required so an
    /// old leaf is never orphaned without a restore record.
    MigrationRequired(SettingId),
    /// A write was interrupted; the transaction is prepared and awaits recovery.
    Interrupted(u64),
    /// A rollback/restore could not cleanly finish; the transaction stays prepared for recovery.
    Pending { transaction: u64, cause: String },
    Registry(String),
    Journal(String),
    Verification(String),
    Profile(String),
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFeature(id) => write!(f, "unknown feature: {id}"),
            Self::Guided(id) => write!(f, "feature is guided, never written: {id}"),
            Self::Unavailable(reason) => write!(f, "unavailable: {reason:?}"),
            Self::RecoveryRequired(ids) => write!(f, "recovery required for {ids:?}"),
            Self::NotManaged(id) => write!(f, "not managed: {id}"),
            Self::MigrationRequired(id) => write!(f, "recipe changed, restore before re-apply: {id}"),
            Self::Interrupted(txn) => write!(f, "interrupted, transaction {txn} awaits recovery"),
            Self::Pending { transaction, cause } => {
                write!(f, "transaction {transaction} pending recovery: {cause}")
            }
            Self::Registry(m) => write!(f, "registry: {m}"),
            Self::Journal(m) => write!(f, "journal: {m}"),
            Self::Verification(m) => write!(f, "verification: {m}"),
            Self::Profile(m) => write!(f, "profile probe: {m}"),
        }
    }
}

impl std::error::Error for DriverError {}
