//! `WindowsSystemProfileProbe` — the live environment fingerprint gatherer. `[WINDOWS-VERIFY]`.
//!
//! Reads the certification fingerprint from AUTHORITATIVE sources, never derived or hardcoded
//! (codex W2 R1 Major-3): `RtlGetVersion` for the unshimmed OS version AND `wProductType`
//! (`is_workstation`), `GetNativeSystemInfo` for the native architecture, `GetCurrentPackageFullName`
//! for package identity, `GetProductInfo` for the SKU, and the registry `CurrentVersion` keys for
//! UBR / DisplayVersion / EditionID / InstallationType / region. Every registry field is read
//! strictly: absent may default, but PRESENT-BUT-MALFORMED fails the whole probe closed, so corrupt
//! or hostile data can never assemble a certifiable tuple. All canonicalization is the host-tested
//! [`assemble_environment`].

use dm_domain::system_tweaks::{
    RegistryAddress, RegistryBackend, RegistryHive, RegistryView, SystemProfileError,
    SystemProfileProbe, WindowsEnvironment,
};
use windows::Wdk::System::SystemServices::RtlGetVersion;
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::Win32::System::SystemInformation::{
    GetNativeSystemInfo, GetProductInfo, OSVERSIONINFOEXW, OS_PRODUCT_TYPE, SYSTEM_INFO,
};

use super::backend::WinregBackend;
use super::profile_facts::{
    assemble_environment, map_native_arch, snapshot_dword, snapshot_string, RawProfileFacts,
};

const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const GEO: &str = r"Control Panel\International\Geo";

/// `OSVERSIONINFOEXW.wProductType` for a workstation (client) install.
const VER_NT_WORKSTATION: u8 = 1;
/// `GetCurrentPackageFullName` error when the process has no package identity.
const APPMODEL_ERROR_NO_PACKAGE: u32 = 15_700;

/// This process's architecture at build time. On a native run it equals the machine arch; under
/// emulation it differs, which is exactly the separation [`WindowsEnvironment`] keeps between the
/// native and process architectures.
const PROCESS_ARCH: &str = if cfg!(target_arch = "x86_64") {
    "x64"
} else if cfg!(target_arch = "aarch64") {
    "arm64"
} else if cfg!(target_arch = "x86") {
    "x86"
} else {
    "unknown"
};

/// Reads the live Windows environment. Stateless.
#[derive(Debug, Default)]
pub struct WindowsSystemProfileProbe;

impl WindowsSystemProfileProbe {
    pub fn new() -> Self {
        Self
    }
}

fn hklm(key: &str, value: &str) -> RegistryAddress {
    RegistryAddress::new(RegistryHive::LocalMachine, RegistryView::Registry64, key, value)
}

fn hkcu(key: &str, value: &str) -> RegistryAddress {
    RegistryAddress::new(RegistryHive::CurrentUser, RegistryView::Registry64, key, value)
}

impl SystemProfileProbe for WindowsSystemProfileProbe {
    fn probe(&self) -> Result<WindowsEnvironment, SystemProfileError> {
        let reg = WinregBackend::new();
        let version = os_version()?;

        // Registry fields. UBR is absent on some early builds → 0; a PRESENT-but-malformed UBR
        // fails closed (codex W2 R1 Major-4). EditionID is required; the rest default to empty and
        // fail `is_certifiable` if absent, but a malformed string fails the probe (Major-5).
        let ubr = read_dword_or(&reg, hklm(CURRENT_VERSION, "UBR"), "UBR", 0)?;
        let display_version = read_string(&reg, hklm(CURRENT_VERSION, "DisplayVersion"), false)?;
        let edition_id = read_string(&reg, hklm(CURRENT_VERSION, "EditionID"), true)?;
        let installation_type =
            read_string(&reg, hklm(CURRENT_VERSION, "InstallationType"), false)?;
        let region = read_string(&reg, hkcu(GEO, "Name"), false)?;

        let product_type = product_sku(version.major, version.minor)?;

        Ok(assemble_environment(RawProfileFacts {
            major: version.major,
            minor: version.minor,
            build: version.build,
            ubr,
            display_version,
            edition_id,
            installation_type,
            product_type,
            is_workstation: version.is_workstation,
            region,
            native_architecture: native_arch(),
            process_architecture: PROCESS_ARCH.to_string(),
            packaged: has_package_identity(),
        }))
    }
}

/// A required-or-optional registry string read: absent → an error (required) or empty (optional);
/// PRESENT-but-malformed → always an error (never silently coerced).
fn read_string(
    reg: &WinregBackend,
    address: RegistryAddress,
    required: bool,
) -> Result<String, SystemProfileError> {
    let label = format!("{}\\{}", address.key, address.value);
    let snapshot = reg
        .read(&address)
        .map_err(|error| SystemProfileError(format!("{label}: {error}")))?;
    match snapshot_string(&snapshot)
        .map_err(|malformed| SystemProfileError(format!("{label}: {}", malformed.0)))?
    {
        Some(text) => Ok(text),
        None if required => Err(SystemProfileError(format!("environment field {label} is unreadable"))),
        None => Ok(String::new()),
    }
}

/// A registry DWORD read that defaults when ABSENT but fails closed when PRESENT-but-malformed.
fn read_dword_or(
    reg: &WinregBackend,
    address: RegistryAddress,
    name: &str,
    default: u32,
) -> Result<u32, SystemProfileError> {
    let snapshot = reg
        .read(&address)
        .map_err(|error| SystemProfileError(format!("{name}: {error}")))?;
    Ok(snapshot_dword(&snapshot)
        .map_err(|malformed| SystemProfileError(format!("{name}: {}", malformed.0)))?
        .unwrap_or(default))
}

/// The unshimmed OS version + `is_workstation`, from `RtlGetVersion`.
struct OsVersion {
    major: u32,
    minor: u32,
    build: u32,
    is_workstation: bool,
}

fn os_version() -> Result<OsVersion, SystemProfileError> {
    let mut info = OSVERSIONINFOEXW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOEXW>() as u32,
        ..Default::default()
    };
    // SAFETY: `RtlGetVersion` fills the extended `OSVERSIONINFOEXW` fields (incl. `wProductType`)
    // when `dwOSVersionInfoSize` is the EXW size; the pointer is a valid, correctly-sized, mutable
    // `OSVERSIONINFOEXW` reinterpreted as the `OSVERSIONINFOW` the API takes.
    let status = unsafe { RtlGetVersion((&mut info as *mut OSVERSIONINFOEXW).cast()) };
    if status.0 != 0 {
        return Err(SystemProfileError(format!(
            "RtlGetVersion failed: NTSTATUS {}",
            status.0
        )));
    }
    Ok(OsVersion {
        major: info.dwMajorVersion,
        minor: info.dwMinorVersion,
        build: info.dwBuildNumber,
        is_workstation: info.wProductType == VER_NT_WORKSTATION,
    })
}

/// The native machine architecture, from `GetNativeSystemInfo` (unaffected by WOW64 emulation).
fn native_arch() -> String {
    let mut info = SYSTEM_INFO::default();
    // SAFETY: `info` is a valid out-param that `GetNativeSystemInfo` fully initializes.
    unsafe { GetNativeSystemInfo(&mut info) };
    // SAFETY: after the call the anonymous union's struct arm holds `wProcessorArchitecture`.
    let raw = unsafe { info.Anonymous.Anonymous.wProcessorArchitecture.0 };
    map_native_arch(raw)
}

/// Whether the process runs with package identity, from `GetCurrentPackageFullName`. A length-only
/// query returns `APPMODEL_ERROR_NO_PACKAGE` when unpackaged; any other status means packaged.
fn has_package_identity() -> bool {
    let mut length: u32 = 0;
    // SAFETY: a length-only query (null buffer); it writes only through `length` and returns a
    // status code, never touching caller memory.
    let status = unsafe { GetCurrentPackageFullName(&mut length, None) };
    status.0 != APPMODEL_ERROR_NO_PACKAGE
}

/// The `GetProductInfo` SKU for the running OS version (service pack 0,0).
fn product_sku(major: u32, minor: u32) -> Result<u32, SystemProfileError> {
    let mut sku = OS_PRODUCT_TYPE(0);
    // SAFETY: all inputs are plain integers and `sku` is a valid out-param; the call writes only
    // through `sku`.
    let ok = unsafe { GetProductInfo(major, minor, 0, 0, &mut sku) };
    if ok.as_bool() {
        Ok(sku.0)
    } else {
        Err(SystemProfileError(
            "GetProductInfo returned false for the running OS version".into(),
        ))
    }
}
