//! Concrete Windows adapters. This module is compiled only for a Windows target.

use std::io;
use std::mem::size_of;
use std::time::Duration;

use windows::core::{PCWSTR, PWSTR};
use windows::Wdk::System::SystemServices::RtlGetVersion;
use windows::Win32::Foundation::{
    GetLastError, SetLastError, APPMODEL_ERROR_NO_PACKAGE, ERROR_INSUFFICIENT_BUFFER,
    ERROR_SUCCESS, LPARAM, WPARAM,
};
use windows::Win32::Globalization::GetUserDefaultGeoName;
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::Win32::System::SystemInformation::{
    GetNativeSystemInfo, GetProductInfo, OSVERSIONINFOEXW, OSVERSIONINFOW, OS_PRODUCT_TYPE,
    PROCESSOR_ARCHITECTURE_AMD64, PROCESSOR_ARCHITECTURE_ARM64, PROCESSOR_ARCHITECTURE_INTEL,
    SYSTEM_INFO,
};
use windows::Win32::UI::Shell::{SHChangeNotify, ShellExecuteW, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, SW_SHOWNORMAL, WM_SETTINGCHANGE,
};
use winreg::enums::{
    RegDisposition, RegType, HKEY_CLASSES_ROOT, HKEY_CURRENT_CONFIG, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, HKEY_USERS, KEY_QUERY_VALUE, KEY_READ, KEY_SET_VALUE, KEY_WOW64_32KEY,
    KEY_WOW64_64KEY, REG_BINARY, REG_CREATED_NEW_KEY, REG_DWORD, REG_DWORD_BIG_ENDIAN,
    REG_EXPAND_SZ, REG_FULL_RESOURCE_DESCRIPTOR, REG_LINK, REG_MULTI_SZ, REG_NONE,
    REG_OPENED_EXISTING_KEY, REG_QWORD, REG_RESOURCE_LIST, REG_RESOURCE_REQUIREMENTS_LIST, REG_SZ,
};
use winreg::{RegKey, RegValue};

use crate::{
    CpuArchitecture, DeleteKeyOutcome, DeleteOutcome, KeyDisposition, PackageIdentity,
    ProfileError, RawRegistryValue, RefreshBackend, RefreshError, RegistryBackend, RegistryError,
    RegistryErrorKind, RegistryHive, RegistryLocation, RegistrySnapshot, RegistryView,
    SystemProfile, SystemProfileProbe, WindowsVersion,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct WinRegistryBackend;

impl RegistryBackend for WinRegistryBackend {
    fn read_value(&self, location: &RegistryLocation) -> Result<RegistrySnapshot, RegistryError> {
        let root = root_key(location.hive);
        let key = match root
            .open_subkey_with_flags(&location.path, KEY_QUERY_VALUE | view_flag(location.view))
        {
            Ok(key) => key,
            Err(error) if is_not_found(&error) => {
                return Ok(RegistrySnapshot {
                    key_existed: false,
                    value: None,
                });
            }
            Err(error) => return Err(registry_error("open key for read", location, error)),
        };
        match key.get_raw_value(&location.value_name) {
            Ok(value) => Ok(RegistrySnapshot {
                key_existed: true,
                value: Some(RawRegistryValue {
                    kind: value.vtype.clone() as u32,
                    bytes: value.bytes,
                }),
            }),
            Err(error) if is_not_found(&error) => Ok(RegistrySnapshot {
                key_existed: true,
                value: None,
            }),
            Err(error) => Err(registry_error("read raw value", location, error)),
        }
    }

    fn key_exists(&self, location: &RegistryLocation) -> Result<bool, RegistryError> {
        match root_key(location.hive)
            .open_subkey_with_flags(&location.path, KEY_QUERY_VALUE | view_flag(location.view))
        {
            Ok(_) => Ok(true),
            Err(error) if is_not_found(&error) => Ok(false),
            Err(error) => Err(registry_error("probe key existence", location, error)),
        }
    }

    fn write_value(
        &self,
        location: &RegistryLocation,
        value: &RawRegistryValue,
    ) -> Result<(), RegistryError> {
        let access = KEY_QUERY_VALUE | KEY_SET_VALUE | view_flag(location.view);
        let key = root_key(location.hive)
            .open_subkey_with_flags(&location.path, access)
            .map_err(|error| registry_error("open existing key for write", location, error))?;
        let vtype = reg_type(value.kind).ok_or_else(|| RegistryError {
            kind: RegistryErrorKind::InvalidData,
            win32_code: None,
            operation: "convert registry type",
            location: Some(location.clone()),
            message: format!("unsupported REG_* kind {}", value.kind),
        })?;
        key.set_raw_value(
            &location.value_name,
            &RegValue {
                vtype,
                bytes: value.bytes.clone(),
            },
        )
        .map_err(|error| registry_error("write raw value", location, error))
    }

    fn create_key(&self, location: &RegistryLocation) -> Result<KeyDisposition, RegistryError> {
        let access = KEY_QUERY_VALUE | KEY_SET_VALUE | view_flag(location.view);
        let (_, disposition) = root_key(location.hive)
            .create_subkey_with_flags(&location.path, access)
            .map_err(|error| registry_error("create/open key prefix", location, error))?;
        Ok(disposition_from(disposition))
    }

    fn delete_value(&self, location: &RegistryLocation) -> Result<DeleteOutcome, RegistryError> {
        let key = match root_key(location.hive).open_subkey_with_flags(
            &location.path,
            KEY_QUERY_VALUE | KEY_SET_VALUE | view_flag(location.view),
        ) {
            Ok(key) => key,
            Err(error) if is_not_found(&error) => return Ok(DeleteOutcome::AlreadyMissing),
            Err(error) => return Err(registry_error("open key for value delete", location, error)),
        };
        match key.delete_value(&location.value_name) {
            Ok(()) => Ok(DeleteOutcome::Deleted),
            Err(error) if is_not_found(&error) => Ok(DeleteOutcome::AlreadyMissing),
            Err(error) => Err(registry_error("delete value", location, error)),
        }
    }

    fn delete_key_if_empty(
        &self,
        location: &RegistryLocation,
    ) -> Result<DeleteKeyOutcome, RegistryError> {
        let root = root_key(location.hive);
        let key = match root
            .open_subkey_with_flags(&location.path, KEY_READ | view_flag(location.view))
        {
            Ok(key) => key,
            Err(error) if is_not_found(&error) => return Ok(DeleteKeyOutcome::AlreadyMissing),
            Err(error) => return Err(registry_error("open key for cleanup", location, error)),
        };
        let info = key
            .query_info()
            .map_err(|error| registry_error("query key before cleanup", location, error))?;
        if info.sub_keys != 0 || info.values != 0 {
            return Ok(DeleteKeyOutcome::NotEmpty);
        }
        drop(key);
        match root.delete_subkey_with_flags(&location.path, view_flag(location.view)) {
            Ok(()) => Ok(DeleteKeyOutcome::Deleted),
            Err(error) if is_not_found(&error) => Ok(DeleteKeyOutcome::AlreadyMissing),
            Err(error) => Err(registry_error("delete empty key", location, error)),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsSystemProfileProbe {
    registry: WinRegistryBackend,
}

impl SystemProfileProbe for WindowsSystemProfileProbe {
    fn probe(&self) -> Result<SystemProfile, ProfileError> {
        let info = rtl_version()?;
        let revision = self.read_required_dword("UBR")?;
        let registry_build_text = self.read_required_string("CurrentBuildNumber")?;
        let registry_build =
            registry_build_text
                .parse::<u32>()
                .map_err(|_| ProfileError::InvalidRegistryValue {
                    name: "CurrentBuildNumber",
                })?;
        if registry_build != info.dwBuildNumber {
            return Err(ProfileError::BuildMismatch {
                rtl_build: info.dwBuildNumber,
                registry_build,
            });
        }
        let display_version = self.read_required_string("DisplayVersion")?;
        let edition_id = self.read_required_string("EditionID")?;
        let installation_type = self.read_required_string("InstallationType")?;
        let mut product = OS_PRODUCT_TYPE::default();
        let product_type = unsafe {
            GetProductInfo(
                info.dwMajorVersion,
                info.dwMinorVersion,
                info.wServicePackMajor as u32,
                info.wServicePackMinor as u32,
                &mut product,
            )
        }
        .as_bool()
        .then_some(product.0);

        Ok(SystemProfile {
            version: WindowsVersion {
                major: info.dwMajorVersion,
                minor: info.dwMinorVersion,
                build: info.dwBuildNumber,
                revision: Some(revision),
            },
            display_version,
            edition_id,
            installation_type,
            product_type,
            // VER_NT_WORKSTATION from winnt.h. Kept platform-neutral outside this adapter.
            is_workstation: info.wProductType == 1,
            region: user_region(),
            native_architecture: native_architecture(),
            process_architecture: process_architecture(),
            package_identity: package_identity()?,
        })
    }
}

impl WindowsSystemProfileProbe {
    fn current_version_location(&self, value_name: &str) -> RegistryLocation {
        RegistryLocation::new(
            RegistryHive::LocalMachine,
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            value_name,
            RegistryView::View64,
        )
    }

    fn read_required_dword(&self, name: &'static str) -> Result<u32, ProfileError> {
        let snapshot = self
            .registry
            .read_value(&self.current_version_location(name))?;
        snapshot
            .value
            .ok_or(ProfileError::MissingRegistryValue { name })?
            .as_dword()
            .ok_or(ProfileError::InvalidRegistryValue { name })
    }

    fn read_required_string(&self, name: &'static str) -> Result<String, ProfileError> {
        let snapshot = self
            .registry
            .read_value(&self.current_version_location(name))?;
        let value = snapshot
            .value
            .ok_or(ProfileError::MissingRegistryValue { name })?;
        let text = value
            .as_reg_sz()
            .ok_or(ProfileError::InvalidRegistryValue { name })?;
        if text.trim().is_empty() {
            return Err(ProfileError::InvalidRegistryValue { name });
        }
        Ok(text)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsRefreshBackend;

impl RefreshBackend for WindowsRefreshBackend {
    fn broadcast_setting_change(
        &self,
        section: &str,
        per_recipient_timeout: Duration,
    ) -> Result<(), RefreshError> {
        let section = wide_nul(section);
        let timeout_ms = per_recipient_timeout.as_millis().min(u32::MAX as u128) as u32;
        let mut recipient_result = 0usize;
        unsafe { SetLastError(ERROR_SUCCESS) };
        let result = unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(section.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                timeout_ms,
                Some(&mut recipient_result),
            )
        };
        if result.0 != 0 {
            return Ok(());
        }
        let code = unsafe { GetLastError() };
        Err(RefreshError::BroadcastFailed {
            last_error: (code != ERROR_SUCCESS).then_some(code.0),
        })
    }

    fn notify_shell_associations_changed(&self) {
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    }

    fn open_settings_page(&self, uri: &str) -> Result<(), RefreshError> {
        if !uri.starts_with("ms-settings:") {
            return Err(RefreshError::InvalidSettingsUri);
        }
        let operation = wide_nul("open");
        let uri = wide_nul(uri);
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(operation.as_ptr()),
                PCWSTR(uri.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        let code = result.0 as isize;
        if code > 32 {
            Ok(())
        } else {
            Err(RefreshError::SettingsLaunchFailed { legacy_code: code })
        }
    }
}

fn root_key(hive: RegistryHive) -> RegKey {
    RegKey::predef(match hive {
        RegistryHive::CurrentUser => HKEY_CURRENT_USER,
        RegistryHive::LocalMachine => HKEY_LOCAL_MACHINE,
        RegistryHive::Users => HKEY_USERS,
        RegistryHive::CurrentConfig => HKEY_CURRENT_CONFIG,
        RegistryHive::ClassesRoot => HKEY_CLASSES_ROOT,
    })
}

fn view_flag(view: RegistryView) -> u32 {
    match view {
        RegistryView::Native => 0,
        RegistryView::View32 => KEY_WOW64_32KEY,
        RegistryView::View64 => KEY_WOW64_64KEY,
    }
}

fn disposition_from(value: RegDisposition) -> KeyDisposition {
    match value {
        REG_CREATED_NEW_KEY => KeyDisposition::Created,
        REG_OPENED_EXISTING_KEY => KeyDisposition::OpenedExisting,
    }
}

fn reg_type(kind: u32) -> Option<RegType> {
    Some(match kind {
        0 => REG_NONE,
        1 => REG_SZ,
        2 => REG_EXPAND_SZ,
        3 => REG_BINARY,
        4 => REG_DWORD,
        5 => REG_DWORD_BIG_ENDIAN,
        6 => REG_LINK,
        7 => REG_MULTI_SZ,
        8 => REG_RESOURCE_LIST,
        9 => REG_FULL_RESOURCE_DESCRIPTOR,
        10 => REG_RESOURCE_REQUIREMENTS_LIST,
        11 => REG_QWORD,
        _ => return None,
    })
}

fn is_not_found(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(2 | 3))
}

fn registry_error(
    operation: &'static str,
    location: &RegistryLocation,
    error: io::Error,
) -> RegistryError {
    let win32_code = error.raw_os_error();
    let kind = match win32_code {
        Some(2 | 3) => RegistryErrorKind::NotFound,
        Some(5) => RegistryErrorKind::AccessDenied,
        _ => RegistryErrorKind::Native,
    };
    RegistryError {
        kind,
        win32_code,
        operation,
        location: Some(location.clone()),
        message: error.to_string(),
    }
}

fn rtl_version() -> Result<OSVERSIONINFOEXW, ProfileError> {
    let mut info = OSVERSIONINFOEXW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOEXW>() as u32,
        ..Default::default()
    };
    let status =
        unsafe { RtlGetVersion((&mut info as *mut OSVERSIONINFOEXW).cast::<OSVERSIONINFOW>()) };
    if status.0 < 0 {
        Err(ProfileError::Version(status.0))
    } else {
        Ok(info)
    }
}

fn user_region() -> Option<String> {
    // GEO_NAME_LENGTH from winnls.h is 85 characters.
    let mut buffer = [0u16; 85];
    let written = unsafe { GetUserDefaultGeoName(&mut buffer) };
    if written <= 0 {
        return None;
    }
    let end = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(written as usize);
    Some(String::from_utf16_lossy(&buffer[..end]))
}

fn native_architecture() -> CpuArchitecture {
    let mut info = SYSTEM_INFO::default();
    unsafe { GetNativeSystemInfo(&mut info) };
    let architecture = unsafe { info.Anonymous.Anonymous.wProcessorArchitecture };
    if architecture == PROCESSOR_ARCHITECTURE_INTEL {
        CpuArchitecture::X86
    } else if architecture == PROCESSOR_ARCHITECTURE_AMD64 {
        CpuArchitecture::X64
    } else if architecture == PROCESSOR_ARCHITECTURE_ARM64 {
        CpuArchitecture::Arm64
    } else {
        CpuArchitecture::Unknown
    }
}

fn process_architecture() -> CpuArchitecture {
    if cfg!(target_arch = "x86") {
        CpuArchitecture::X86
    } else if cfg!(target_arch = "x86_64") {
        CpuArchitecture::X64
    } else if cfg!(target_arch = "aarch64") {
        CpuArchitecture::Arm64
    } else {
        CpuArchitecture::Unknown
    }
}

fn package_identity() -> Result<PackageIdentity, ProfileError> {
    let mut length = 0u32;
    let first = unsafe { GetCurrentPackageFullName(&mut length, None) };
    if first == APPMODEL_ERROR_NO_PACKAGE {
        return Ok(PackageIdentity::Unpackaged);
    }
    if first != ERROR_INSUFFICIENT_BUFFER {
        return Err(ProfileError::Package(first.0));
    }
    let mut buffer = vec![0u16; length as usize];
    let second =
        unsafe { GetCurrentPackageFullName(&mut length, Some(PWSTR(buffer.as_mut_ptr()))) };
    if second != ERROR_SUCCESS {
        return Err(ProfileError::Package(second.0));
    }
    let end = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    Ok(PackageIdentity::Packaged {
        full_name: String::from_utf16_lossy(&buffer[..end]),
    })
}

fn wide_nul(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
