//! Platform-neutral settings model. No Win32 types cross this boundary.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryHive {
    CurrentUser,
    LocalMachine,
    Users,
    CurrentConfig,
    /// Reads are a merged view. Prefer explicit `Software\\Classes` locations for writes.
    ClassesRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryView {
    Native,
    View32,
    View64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegistryLocation {
    pub hive: RegistryHive,
    pub path: String,
    /// Empty string means the unnamed/default value.
    pub value_name: String,
    pub view: RegistryView,
}

impl RegistryLocation {
    pub fn new(
        hive: RegistryHive,
        path: impl Into<String>,
        value_name: impl Into<String>,
        view: RegistryView,
    ) -> Self {
        Self {
            hive,
            path: path.into(),
            value_name: value_name.into(),
            view,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRegistryValue {
    /// Native `REG_*` numeric kind. Bytes are stored exactly as returned by the registry.
    pub kind: u32,
    pub bytes: Vec<u8>,
}

impl RawRegistryValue {
    pub const REG_SZ: u32 = 1;
    pub const REG_DWORD: u32 = 4;

    pub fn dword(value: u32) -> Self {
        Self {
            kind: Self::REG_DWORD,
            bytes: value.to_le_bytes().to_vec(),
        }
    }

    pub fn as_dword(&self) -> Option<u32> {
        (self.kind == Self::REG_DWORD && self.bytes.len() == 4)
            .then(|| u32::from_le_bytes(self.bytes.as_slice().try_into().expect("length checked")))
    }

    /// Decodes an exact `REG_SZ` without accepting `REG_EXPAND_SZ` as an equivalent type.
    pub fn as_reg_sz(&self) -> Option<String> {
        if self.kind != Self::REG_SZ || !self.bytes.len().is_multiple_of(2) {
            return None;
        }
        let units: Vec<u16> = self
            .bytes
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        let end = units
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(units.len());
        String::from_utf16(&units[..end]).ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrySnapshot {
    pub key_existed: bool,
    pub value: Option<RawRegistryValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDisposition {
    OpenedExisting,
    Created,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    AlreadyMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteKeyOutcome {
    Deleted,
    AlreadyMissing,
    NotEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryErrorKind {
    NotFound,
    AccessDenied,
    InvalidData,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryError {
    pub kind: RegistryErrorKind,
    pub win32_code: Option<i32>,
    pub operation: &'static str,
    pub location: Option<RegistryLocation>,
    pub message: String,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.operation, self.message)
    }
}

impl std::error::Error for RegistryError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsVersion {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
    pub revision: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuArchitecture {
    X86,
    X64,
    Arm64,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageIdentity {
    Unpackaged,
    Packaged { full_name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Unpackaged,
    Packaged,
}

impl PackageIdentity {
    pub fn kind(&self) -> PackageKind {
        match self {
            Self::Unpackaged => PackageKind::Unpackaged,
            Self::Packaged { .. } => PackageKind::Packaged,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProfile {
    pub version: WindowsVersion,
    /// Marketing version from 64-bit `CurrentVersion\\DisplayVersion` (for example, `24H2`).
    pub display_version: String,
    /// Registry edition identifier from 64-bit `CurrentVersion\\EditionID`.
    pub edition_id: String,
    /// Installation role from 64-bit `CurrentVersion\\InstallationType`.
    pub installation_type: String,
    /// Raw `OS_PRODUCT_TYPE` from `GetProductInfo`; unknown remains `None`.
    pub product_type: Option<u32>,
    /// `OSVERSIONINFOEXW.wProductType == VER_NT_WORKSTATION`.
    pub is_workstation: bool,
    /// ISO 3166-1 alpha-2 or UN M.49, exactly as reported by Windows.
    pub region: Option<String>,
    /// Physical/native OS architecture from `GetNativeSystemInfo`.
    pub native_architecture: CpuArchitecture,
    /// Architecture this executable was compiled for; it may differ under emulation.
    pub process_architecture: CpuArchitecture,
    pub package_identity: PackageIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    Version(i32),
    BuildMismatch { rtl_build: u32, registry_build: u32 },
    MissingRegistryValue { name: &'static str },
    InvalidRegistryValue { name: &'static str },
    Package(u32),
    Registry(RegistryError),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Version(code) => write!(f, "RtlGetVersion failed with NTSTATUS {code:#x}"),
            Self::BuildMismatch {
                rtl_build,
                registry_build,
            } => {
                write!(
                    f,
                    "build mismatch: RtlGetVersion={rtl_build}, registry={registry_build}"
                )
            }
            Self::MissingRegistryValue { name } => {
                write!(f, "required registry value {name} is missing")
            }
            Self::InvalidRegistryValue { name } => write!(f, "invalid registry value {name}"),
            Self::Package(code) => write!(f, "package identity probe failed with {code}"),
            Self::Registry(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProfileError {}

impl From<RegistryError> for ProfileError {
    fn from(value: RegistryError) -> Self {
        Self::Registry(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshError {
    BroadcastFailed { last_error: Option<u32> },
    SettingsLaunchFailed { legacy_code: isize },
    InvalidSettingsUri,
}

impl fmt::Display for RefreshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for RefreshError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_dword_is_exact_little_endian() {
        let value = RawRegistryValue::dword(0x1234_5678);
        assert_eq!(value.bytes, [0x78, 0x56, 0x34, 0x12]);
        assert_eq!(value.as_dword(), Some(0x1234_5678));
    }

    #[test]
    fn reg_sz_decoder_rejects_wrong_type_odd_bytes_and_invalid_utf16() {
        let valid = RawRegistryValue {
            kind: RawRegistryValue::REG_SZ,
            bytes: "24H2"
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect(),
        };
        assert_eq!(valid.as_reg_sz().as_deref(), Some("24H2"));

        assert_eq!(
            RawRegistryValue {
                kind: 2,
                bytes: valid.bytes.clone(),
            }
            .as_reg_sz(),
            None
        );
        assert_eq!(
            RawRegistryValue {
                kind: RawRegistryValue::REG_SZ,
                bytes: vec![b'A'],
            }
            .as_reg_sz(),
            None
        );
        assert_eq!(
            RawRegistryValue {
                kind: RawRegistryValue::REG_SZ,
                bytes: 0xD800_u16.to_le_bytes().to_vec(),
            }
            .as_reg_sz(),
            None
        );
    }
}
