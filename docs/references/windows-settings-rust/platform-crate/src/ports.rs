//! Platform ports driven by the operations layer.

use std::time::Duration;

use crate::{
    DeleteKeyOutcome, DeleteOutcome, KeyDisposition, ProfileError, RawRegistryValue, RefreshError,
    RegistryError, RegistryLocation, RegistrySnapshot, SystemProfile,
};

pub trait RegistryBackend {
    fn read_value(&self, location: &RegistryLocation) -> Result<RegistrySnapshot, RegistryError>;

    fn key_exists(&self, location: &RegistryLocation) -> Result<bool, RegistryError>;

    /// Creates or opens exactly one prefix. Callers walk root-to-leaf so every native disposition
    /// can be promoted independently from pre-WAL candidate to confirmed cleanup ownership.
    fn create_key(&self, location: &RegistryLocation) -> Result<KeyDisposition, RegistryError>;

    /// Writes exact raw type+bytes to an already-openable key. The transaction layer must WAL the
    /// original and confirm all preceding `Created` dispositions before terminal verification.
    fn write_value(
        &self,
        location: &RegistryLocation,
        value: &RawRegistryValue,
    ) -> Result<(), RegistryError>;

    fn delete_value(&self, location: &RegistryLocation) -> Result<DeleteOutcome, RegistryError>;

    /// Best-effort cleanup for a key recorded as created by this app. Registry has no atomic
    /// delete-if-empty primitive, so the transaction layer must treat external changes as conflicts.
    fn delete_key_if_empty(
        &self,
        location: &RegistryLocation,
    ) -> Result<DeleteKeyOutcome, RegistryError>;
}

pub trait SystemProfileProbe {
    fn probe(&self) -> Result<SystemProfile, ProfileError>;
}

pub trait RefreshBackend {
    /// Sends a per-recipient-timeout hint. With `HWND_BROADCAST`, total wait can be the timeout
    /// multiplied by the number of top-level windows. This is never proof that a component reloaded.
    fn broadcast_setting_change(
        &self,
        section: &str,
        per_recipient_timeout: Duration,
    ) -> Result<(), RefreshError>;

    /// Shell namespace/icon-association hint only; not a generic settings refresh.
    fn notify_shell_associations_changed(&self);

    /// Opens a documented `ms-settings:` URI as a safe manual fallback.
    fn open_settings_page(&self, uri: &str) -> Result<(), RefreshError>;
}
