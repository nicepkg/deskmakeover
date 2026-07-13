//! `WindowsSystemProfileProbe` — the live environment fingerprint gatherer. `[WINDOWS-VERIFY]`.
//!
//! Reads the certification fingerprint almost entirely from the registry (through the same faithful
//! [`WinregBackend`]), plus one `GetProductInfo` call for the SKU, then hands the raw fields to the
//! host-tested [`assemble_environment`] for ALL canonicalization. Version comes from the registry
//! `CurrentVersion` keys — the unshimmed build number, unlike `GetVersionEx` — so no `Wdk` /
//! `RtlGetVersion` dependency is pulled; a Wave 3 hardening may cross-check `RtlGetVersion`.

use dm_domain::system_tweaks::{
    RegistryAddress, RegistryBackend, RegistryHive, RegistrySnapshot, RegistryView,
    SystemProfileError, SystemProfileProbe, WindowsEnvironment,
};
use windows::Win32::System::SystemInformation::{GetProductInfo, OS_PRODUCT_TYPE};

use super::backend::WinregBackend;
use super::profile_facts::{
    assemble_environment, map_processor_architecture, snapshot_dword, snapshot_string,
    RawProfileFacts,
};

const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const GEO: &str = r"Control Panel\International\Geo";
const ENVIRONMENT: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

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

/// Reads the live Windows environment via the registry + `GetProductInfo`. Stateless.
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
        let read = |address: RegistryAddress| -> Result<RegistrySnapshot, SystemProfileError> {
            reg.read(&address)
                .map_err(|error| SystemProfileError(error.to_string()))
        };
        let required =
            |field: &str| SystemProfileError(format!("environment field {field} is unreadable"));

        // Version: the registry build number is authoritative and unshimmed. Major/minor are DWORDs
        // on Windows 10+/11; the build is a decimal string; UBR is a DWORD that is absent on some
        // early builds (defaults to 0 — never a hard failure).
        let major = snapshot_dword(&read(hklm(CURRENT_VERSION, "CurrentMajorVersionNumber"))?)
            .ok_or_else(|| required("CurrentMajorVersionNumber"))?;
        let minor = snapshot_dword(&read(hklm(CURRENT_VERSION, "CurrentMinorVersionNumber"))?)
            .ok_or_else(|| required("CurrentMinorVersionNumber"))?;
        let build = snapshot_string(&read(hklm(CURRENT_VERSION, "CurrentBuildNumber"))?)
            .and_then(|raw| raw.trim().parse::<u32>().ok())
            .ok_or_else(|| required("CurrentBuildNumber"))?;
        let ubr = snapshot_dword(&read(hklm(CURRENT_VERSION, "UBR"))?).unwrap_or(0);

        // Identity: EditionID + the GetProductInfo SKU must later agree for certification. A soft
        // field that is absent stays empty and fails `is_certifiable` rather than aborting the probe.
        let display_version =
            snapshot_string(&read(hklm(CURRENT_VERSION, "DisplayVersion"))?).unwrap_or_default();
        let edition_id = snapshot_string(&read(hklm(CURRENT_VERSION, "EditionID"))?)
            .ok_or_else(|| required("EditionID"))?;
        let installation_type =
            snapshot_string(&read(hklm(CURRENT_VERSION, "InstallationType"))?).unwrap_or_default();
        let region = snapshot_string(&read(hkcu(GEO, "Name"))?).unwrap_or_default();
        let native_architecture =
            snapshot_string(&read(hklm(ENVIRONMENT, "PROCESSOR_ARCHITECTURE"))?)
                .map(|raw| map_processor_architecture(&raw))
                .unwrap_or_default();

        let product_type = product_sku(major, minor)?;

        Ok(assemble_environment(RawProfileFacts {
            major,
            minor,
            build,
            ubr,
            display_version,
            edition_id,
            // A `Client` installation is the workstation case the calm module certifies. The domain
            // keeps `is_workstation` a separate field and `is_certifiable` requires BOTH, so
            // deriving it from the same InstallationType is the one honest signal available without
            // an `RtlGetVersion`/`OSVERSIONINFOEXW` read (a Wave 3 hardening may split them).
            is_workstation: installation_type.trim().eq_ignore_ascii_case("Client"),
            installation_type,
            product_type,
            region,
            native_architecture,
            process_architecture: PROCESS_ARCH.to_string(),
            // DeskMakeover ships as an unpackaged desktop binary; a packaged (MSIX) build would
            // re-derive this true and re-certify under package registry virtualization.
            packaged: false,
        }))
    }
}

/// The `GetProductInfo` SKU for the running OS version (service pack 0,0).
fn product_sku(major: u32, minor: u32) -> Result<u32, SystemProfileError> {
    let mut sku = OS_PRODUCT_TYPE(0);
    // SAFETY: all inputs are plain integers and `sku` is a valid out-param; the call reads no
    // caller-owned memory and writes only through `sku`.
    let ok = unsafe { GetProductInfo(major, minor, 0, 0, &mut sku) };
    if ok.as_bool() {
        Ok(sku.0)
    } else {
        Err(SystemProfileError(
            "GetProductInfo returned false for the running OS version".into(),
        ))
    }
}
