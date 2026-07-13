//! Registry value model for the 清爽 (calm-Windows) settings decision core.
//!
//! Deliberately NOT `dm_domain::restore::RegistryValue` (that type is `{raw: String, kind:
//! u32}`, narrow and string-shaped for the Recycle Bin icon anchors). General settings must
//! retain the exact Win32 kind AND raw bytes so a restore is byte-identical, and an unknown
//! extension type must fail closed rather than round-trip lossily. This module is that type.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A compile-time catalog setting id. The frontend names a `SettingId`; it never names a
/// registry path (the transaction contract: a path from the webview is always rejected).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SettingId(String);

impl SettingId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SettingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Which registry hive a value lives in. Only these two are ever touched; the calm module
/// writes HKCU per-user values and reads HKLM only to detect policy guards (never writes it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RegistryHive {
    CurrentUser,
    LocalMachine,
}

/// The 32/64-bit registry view. Recipes use `Registry64` unless a Windows lab proves a value
/// lives in the 32-bit view; the view is part of a key's identity so a mismatch never aliases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RegistryView {
    Native,
    Registry32,
    Registry64,
}

/// A fully-qualified registry value location: hive + view + key path + value name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RegistryAddress {
    pub hive: RegistryHive,
    pub view: RegistryView,
    pub key: String,
    pub value: String,
}

impl RegistryAddress {
    pub fn new(
        hive: RegistryHive,
        view: RegistryView,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            hive,
            view,
            key: key.into(),
            value: value.into(),
        }
    }

    /// The key this value lives under, without the value name.
    pub fn key_location(&self) -> RegistryKey {
        RegistryKey::new(self.hive, self.view, self.key.clone())
    }
}

impl fmt::Display for RegistryAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}\\{}\\{}", self.hive, self.key, self.value)
    }
}

/// A registry key location (no value name). Used for key-existence checks and reverse-order
/// key cleanup during restore.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RegistryKey {
    pub hive: RegistryHive,
    pub view: RegistryView,
    pub path: String,
}

impl RegistryKey {
    pub fn new(hive: RegistryHive, view: RegistryView, path: impl Into<String>) -> Self {
        Self {
            hive,
            view,
            path: path.into(),
        }
    }

    /// Every concrete subkey prefix of this key, shallowest first, never the hive root.
    /// Used to know which parent keys an apply may have created and must tear down on restore.
    pub fn prefixes(&self) -> Vec<Self> {
        let components: Vec<&str> = self
            .path
            .split('\\')
            .filter(|component| !component.is_empty())
            .collect();
        (1..=components.len())
            .map(|length| Self::new(self.hive, self.view, components[..length].join("\\")))
            .collect()
    }

    /// The number of non-empty path components (0 = the hive root itself).
    pub fn depth(&self) -> usize {
        self.path
            .split('\\')
            .filter(|component| !component.is_empty())
            .count()
    }

    pub fn is_hive_root(&self) -> bool {
        self.depth() == 0
    }
}

impl fmt::Display for RegistryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}\\{}", self.hive, self.path)
    }
}

/// The exact Win32 registry value type. Standard kinds `0..=11` are named; anything else is
/// `Other(raw)` and can be READ and RESTORED byte-for-byte but is NEVER written as a desired
/// value (a recipe only ever establishes a named kind), so an unknown extension type can never
/// be lossily normalized.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RegistryValueKind {
    None,
    String,
    ExpandString,
    Binary,
    Dword,
    DwordBigEndian,
    Link,
    MultiString,
    ResourceList,
    FullResourceDescriptor,
    ResourceRequirementsList,
    Qword,
    /// Any raw Win32 type number outside the standard `0..=11` range.
    Other(u32),
}

impl RegistryValueKind {
    /// The raw Win32 `REG_*` type number this kind maps to.
    pub fn raw(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::String => 1,
            Self::ExpandString => 2,
            Self::Binary => 3,
            Self::Dword => 4,
            Self::DwordBigEndian => 5,
            Self::Link => 6,
            Self::MultiString => 7,
            Self::ResourceList => 8,
            Self::FullResourceDescriptor => 9,
            Self::ResourceRequirementsList => 10,
            Self::Qword => 11,
            Self::Other(raw) => *raw,
        }
    }

    /// Reconstruct a kind from a raw Win32 type number. `0..=11` map to named kinds; any other
    /// number becomes `Other` (fail-closed: the caller must treat it as unwritable).
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            0 => Self::None,
            1 => Self::String,
            2 => Self::ExpandString,
            3 => Self::Binary,
            4 => Self::Dword,
            5 => Self::DwordBigEndian,
            6 => Self::Link,
            7 => Self::MultiString,
            8 => Self::ResourceList,
            9 => Self::FullResourceDescriptor,
            10 => Self::ResourceRequirementsList,
            11 => Self::Qword,
            other => Self::Other(other),
        }
    }

    /// True for the standard `0..=11` types. `Other` is not standard and is unwritable.
    pub fn is_standard(&self) -> bool {
        !matches!(self, Self::Other(_))
    }
}

/// A raw registry value: its exact kind plus its exact bytes, preserved verbatim so a restore
/// is byte-identical (a `REG_EXPAND_SZ` with an unexpanded `%SystemRoot%` is never expanded).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRegistryValue {
    pub kind: RegistryValueKind,
    #[serde(with = "crate::system_tweaks::bytes_base64")]
    pub bytes: Vec<u8>,
}

impl RawRegistryValue {
    pub fn new(kind: RegistryValueKind, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            kind,
            bytes: bytes.into(),
        }
    }

    /// A `REG_DWORD` holding the little-endian encoding of `value` (every calm recipe writes a
    /// DWORD, so this is the one constructor recipes use for a desired value).
    pub fn dword(value: u32) -> Self {
        Self::new(RegistryValueKind::Dword, value.to_le_bytes().to_vec())
    }

    /// The `u32` a well-formed 4-byte DWORD holds, else `None` (a malformed width never reads
    /// as a silent zero).
    pub fn as_dword(&self) -> Option<u32> {
        if self.kind == RegistryValueKind::Dword && self.bytes.len() == 4 {
            Some(u32::from_le_bytes([
                self.bytes[0],
                self.bytes[1],
                self.bytes[2],
                self.bytes[3],
            ]))
        } else {
            None
        }
    }
}

/// What a probe found at a registry address: the whole key was missing, the key existed but
/// the value was absent, or the value was present with its exact bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistrySnapshot {
    /// The complete key path did not exist.
    KeyMissing,
    /// The key existed but this value did not.
    ValueMissing,
    /// The value was present with this exact kind and bytes.
    Present(RawRegistryValue),
}

impl RegistrySnapshot {
    pub fn key_existed(&self) -> bool {
        !matches!(self, Self::KeyMissing)
    }

    pub fn value(&self) -> Option<&RawRegistryValue> {
        match self {
            Self::Present(value) => Some(value),
            Self::KeyMissing | Self::ValueMissing => None,
        }
    }

    pub fn kind(&self) -> Option<&RegistryValueKind> {
        self.value().map(|value| &value.kind)
    }
}

/// Whether a recipe leaf may be created when its current VALUE is missing. A missing KEY is a
/// separate, stricter matter: W1 creates no registry key, so a `KeyMissing` snapshot is always
/// fail-closed regardless of this policy (see [`SettingMutation::accepts`]). Key creation +
/// reverse-order key cleanup is a documented later slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissingPolicy {
    /// A documented primary preference may create a missing VALUE when its key already exists.
    CreateAllowed,
    /// An existing-only companion or advanced leaf. A missing value is inapplicable.
    MustAlreadyExist,
}

/// One registry mutation a setting establishes: the target address, the value to write, the
/// existing kinds the current value is allowed to have, and the missing-value policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingMutation {
    pub address: RegistryAddress,
    pub desired: RegistrySnapshot,
    pub accepted_existing_kinds: Vec<RegistryValueKind>,
    pub missing_policy: MissingPolicy,
}

impl SettingMutation {
    /// Whether the current `snapshot` is an acceptable base for this mutation:
    /// - a missing KEY is NEVER acceptable in W1 (no key creation — fail closed);
    /// - a missing VALUE (key present) is acceptable only when creation is allowed;
    /// - a present value must carry an accepted kind (and a DWORD must be exactly 4 bytes wide).
    pub fn accepts(&self, snapshot: &RegistrySnapshot) -> bool {
        match snapshot {
            RegistrySnapshot::KeyMissing => false,
            RegistrySnapshot::ValueMissing => self.missing_policy == MissingPolicy::CreateAllowed,
            RegistrySnapshot::Present(value) => {
                self.accepted_existing_kinds.contains(&value.kind)
                    && (value.kind != RegistryValueKind::Dword || value.bytes.len() == 4)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_round_trips_every_standard_number() {
        for raw in 0u32..=11 {
            let kind = RegistryValueKind::from_raw(raw);
            assert!(kind.is_standard());
            assert_eq!(kind.raw(), raw);
        }
    }

    #[test]
    fn an_unknown_type_number_fails_closed_as_other() {
        let kind = RegistryValueKind::from_raw(12);
        assert_eq!(kind, RegistryValueKind::Other(12));
        assert!(!kind.is_standard());
        assert_eq!(kind.raw(), 12); // preserved verbatim for a byte-identical restore
    }

    #[test]
    fn dword_encodes_little_endian_and_reads_back() {
        let value = RawRegistryValue::dword(0);
        assert_eq!(value.bytes, vec![0, 0, 0, 0]);
        assert_eq!(value.as_dword(), Some(0));
        assert_eq!(RawRegistryValue::dword(1).as_dword(), Some(1));
    }

    #[test]
    fn a_malformed_dword_width_never_reads_as_zero() {
        let value = RawRegistryValue::new(RegistryValueKind::Dword, vec![0, 0]);
        assert_eq!(value.as_dword(), None);
    }

    #[test]
    fn prefixes_are_shallowest_first_and_never_the_root() {
        let key = RegistryKey::new(
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            r"Software\Vendor\App",
        );
        let prefixes = key.prefixes();
        assert_eq!(prefixes.len(), 3);
        assert_eq!(prefixes[0].path, "Software");
        assert_eq!(prefixes[2].path, r"Software\Vendor\App");
        assert!(prefixes.iter().all(|prefix| !prefix.is_hive_root()));
    }

    #[test]
    fn accepts_gates_creation_and_kind() {
        let mutation = SettingMutation {
            address: RegistryAddress::new(
                RegistryHive::CurrentUser,
                RegistryView::Registry64,
                r"Software\Test",
                "Value",
            ),
            desired: RegistrySnapshot::Present(RawRegistryValue::dword(0)),
            accepted_existing_kinds: vec![RegistryValueKind::Dword],
            missing_policy: MissingPolicy::MustAlreadyExist,
        };
        // Missing value, but creation is not allowed → rejected.
        assert!(!mutation.accepts(&RegistrySnapshot::ValueMissing));
        // Present with the accepted kind and a 4-byte width → accepted.
        assert!(mutation.accepts(&RegistrySnapshot::Present(RawRegistryValue::dword(1))));
        // Present with a wrong kind → rejected.
        assert!(!mutation.accepts(&RegistrySnapshot::Present(RawRegistryValue::new(
            RegistryValueKind::String,
            b"x".to_vec()
        ))));
        // A missing KEY is never acceptable in W1, even with CreateAllowed.
        let create_allowed = SettingMutation {
            missing_policy: MissingPolicy::CreateAllowed,
            ..mutation.clone()
        };
        assert!(!create_allowed.accepts(&RegistrySnapshot::KeyMissing));
        assert!(create_allowed.accepts(&RegistrySnapshot::ValueMissing));
    }
}
