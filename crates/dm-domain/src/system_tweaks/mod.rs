//! 清爽 (calm-Windows) settings decision-core domain layer.
//!
//! Setting ids, the exact registry value model, the Windows environment fingerprint, capability
//! gating, probe/apply/restore outcome states, and the platform ports. Every type here is
//! platform-agnostic: the real registry/profile backends live in `dm-windows`, the transaction
//! driver in `dm-operations`. This layer is copied by BOUNDARY from the research reference
//! crate, never as a runtime dependency (spec 08 §12).
//!
//! It deliberately does NOT reuse [`crate::restore::RegistryValue`] — that type is string-shaped
//! for the icon anchors, while general settings must retain the exact kind and raw bytes.

pub mod environment;
pub mod model;
pub mod ports;
pub mod state;

/// Raw bytes serialize as base64 so the JSON journal/ledger stays compact (the same convention
/// the icon restore anchors use).
pub(crate) mod bytes_base64 {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(deserializer)?;
        STANDARD
            .decode(text.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}

pub use environment::{WindowsEdition, WindowsEnvironment};
pub use model::{
    MissingPolicy, RawRegistryValue, RegistryAddress, RegistryHive, RegistryKey, RegistrySnapshot,
    RegistryValueKind, RegistryView, SettingId, SettingMutation,
};
pub use ports::{
    DeleteKeyOutcome, RegistryBackend, RegistryError, RegistryWriteIntent, RegistryWriteOutcome,
    SystemProfileError, SystemProfileProbe,
};
pub use state::{
    ApplyOutcome, Capability, ProbeContext, ProbeOutcome, ProbeReport, RestoreOutcome, SkipReason,
    UnavailableReason,
};
