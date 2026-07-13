//! Platform seams for the calm settings core. `dm-windows` supplies the real `winreg`/
//! `windows-rs` implementations; the Mac devhost supplies deterministic fakes. The decision
//! core in `dm-operations` depends ONLY on these traits, never on a Windows crate.

use super::environment::WindowsEnvironment;
use super::model::{RegistryAddress, RegistryKey, RegistrySnapshot};

/// Whether a compare-exchange is establishing a desired value (apply) or walking one back (undo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryWriteIntent {
    Apply,
    Undo,
}

/// The prefixes a write opened with a native `Created` disposition — the ONLY keys an apply
/// rollback may later delete (a key that already existed is never torn down).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistryWriteOutcome {
    pub created_keys: Vec<RegistryKey>,
}

/// The result of a delete-if-empty attempt on an app-created key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteKeyOutcome {
    Deleted,
    AlreadyMissing,
    /// The key still holds a value or subkey → retained, never force-deleted.
    NotEmpty,
}

/// A registry failure, coarse by kind (the operations layer branches on the kind, not on a
/// Win32 error code).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    Io(String),
    /// A policy guard rejected the write.
    ManagedByPolicy(RegistryAddress),
    /// The logical compare-and-set found a value other than the expected base.
    Conflict {
        address: RegistryAddress,
        expected: Box<RegistrySnapshot>,
        actual: Box<RegistrySnapshot>,
    },
    /// A recorded app-created key was unexpectedly non-empty during cleanup.
    KeyCleanupBlocked(RegistryKey),
    /// Test-only process-death signal. Production backends never return this.
    Interrupted,
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message) => write!(f, "registry i/o: {message}"),
            Self::ManagedByPolicy(address) => write!(f, "managed by policy: {address}"),
            Self::Conflict { address, .. } => write!(f, "registry cas conflict: {address}"),
            Self::KeyCleanupBlocked(key) => write!(f, "app-created key not empty: {key}"),
            Self::Interrupted => write!(f, "simulated process interruption"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Raw registry access. A Windows implementation uses explicit 32/64-bit views and raw
/// `winreg::RegValue` bytes; it must NEVER accept a path chosen by the frontend.
pub trait RegistryBackend {
    fn read(&self, address: &RegistryAddress) -> Result<RegistrySnapshot, RegistryError>;

    fn key_exists(&self, key: &RegistryKey) -> Result<bool, RegistryError>;

    /// A conservative "do not write this leaf" signal: `true` means the leaf must NOT be
    /// overwritten because it appears managed or write-protected. Implementations answer with the
    /// best AUTHORITATIVE signal they have — a real policy-guard read, or (the `winreg` adapter) a
    /// side-effect-free `KEY_SET_VALUE` write-access probe reporting ACL write-protection. It is
    /// deliberately CONSERVATIVE and may have FALSE NEGATIVES: a Group Policy PREFERENCE or a
    /// direct-registry management that leaves the value user-writable is NOT detectable here, and a
    /// custom ACL denial that is not policy at all still reads `true` (a conservative "managed"
    /// classification the UI may surface). The authoritative per-recipe administrative-template
    /// detection is the catalog `policy_guards` (read separately by the engine); the write slice
    /// stays fail-closed (empty capability manifest) until the certification lab enumerates every
    /// recipe's guard set. An implementation must NOT treat this as a complete policy-provenance
    /// oracle.
    fn is_policy_managed(&self, address: &RegistryAddress) -> Result<bool, RegistryError>;

    /// Process-serialized logical CAS: re-read, compare against `expected`, then write `desired`.
    /// The Windows registry has no cross-process atomic CAS, so an implementation documents the
    /// residual compare-to-write TOCTOU window and relies on post-write double verification.
    fn compare_exchange(
        &mut self,
        intent: RegistryWriteIntent,
        address: &RegistryAddress,
        expected: &RegistrySnapshot,
        desired: &RegistrySnapshot,
    ) -> Result<RegistryWriteOutcome, RegistryError>;

    /// Delete a recorded app-created key ONLY when it is still empty; otherwise retain it and
    /// return `NotEmpty`. Never `delete_subkey_all`.
    fn delete_key_if_empty(
        &mut self,
        key: &RegistryKey,
    ) -> Result<DeleteKeyOutcome, RegistryError>;
}

/// A failure to read the live Windows environment fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProfileError(pub String);

impl std::fmt::Display for SystemProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SystemProfileError {}

impl From<&str> for SystemProfileError {
    fn from(message: &str) -> Self {
        Self(message.to_owned())
    }
}

impl From<String> for SystemProfileError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

/// Reads and canonicalizes the complete Windows environment fingerprint. Re-probed immediately
/// before every mutation so a feature update cannot let a stale certification auto-replay.
pub trait SystemProfileProbe {
    fn probe(&self) -> Result<WindowsEnvironment, SystemProfileError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_tweaks::model::{RegistryHive, RegistryView};

    #[test]
    fn registry_error_reports_its_kind() {
        let address = RegistryAddress::new(
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            r"Software\Test",
            "Value",
        );
        let error = RegistryError::ManagedByPolicy(address);
        assert!(error.to_string().contains("managed by policy"));
    }
}
