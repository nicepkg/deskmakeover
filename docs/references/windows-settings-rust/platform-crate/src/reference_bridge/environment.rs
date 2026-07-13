use std::{error::Error, fmt};

use deskmakeover_windows_settings_reference::{WindowsEdition, WindowsEnvironment};

use crate::{CpuArchitecture, PackageIdentity, SystemProfile, WindowsVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentBridgeError {
    MissingRevision,
    MissingProductType,
    MissingRegion,
    InvalidField(&'static str),
    NotWindows11(WindowsVersion),
    NonClientInstallation(String),
    NotWorkstation,
    UnsupportedNativeArchitecture(CpuArchitecture),
    UnsupportedProcessArchitecture(CpuArchitecture),
    ImpossibleArchitecturePair {
        native: CpuArchitecture,
        process: CpuArchitecture,
    },
    EditionProductMismatch {
        edition_id: String,
        product_type: u32,
    },
    EmptyPackageIdentity,
    UncertifiableEnvironment,
}

impl fmt::Display for EnvironmentBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for EnvironmentBridgeError {}

impl TryFrom<SystemProfile> for WindowsEnvironment {
    type Error = EnvironmentBridgeError;

    fn try_from(profile: SystemProfile) -> Result<Self, Self::Error> {
        let ubr = profile
            .version
            .revision
            .ok_or(EnvironmentBridgeError::MissingRevision)?;
        if profile.version.major != 10
            || profile.version.minor != 0
            || profile.version.build < 22_000
        {
            return Err(EnvironmentBridgeError::NotWindows11(profile.version));
        }

        let display_version = canonical_nonempty(&profile.display_version, "DisplayVersion")?;
        let edition_id = nonempty(&profile.edition_id, "EditionID")?;
        let installation_type = nonempty(&profile.installation_type, "InstallationType")?;
        if !installation_type.eq_ignore_ascii_case("Client") {
            return Err(EnvironmentBridgeError::NonClientInstallation(
                profile.installation_type,
            ));
        }
        if !profile.is_workstation {
            return Err(EnvironmentBridgeError::NotWorkstation);
        }

        let product_type = profile
            .product_type
            .ok_or(EnvironmentBridgeError::MissingProductType)?;
        let Some((edition_id, edition)) =
            WindowsEdition::canonical_raw_identity(edition_id, product_type)
        else {
            return Err(EnvironmentBridgeError::EditionProductMismatch {
                edition_id: edition_id.to_owned(),
                product_type,
            });
        };
        let region = profile
            .region
            .as_deref()
            .ok_or(EnvironmentBridgeError::MissingRegion)
            .and_then(|value| {
                WindowsEnvironment::canonical_region(value)
                    .ok_or(EnvironmentBridgeError::InvalidField("region"))
            })?;
        let native_architecture = native_architecture(profile.native_architecture)?;
        let process_architecture = process_architecture(profile.process_architecture)?;
        validate_architecture_pair(profile.native_architecture, profile.process_architecture)?;
        let packaged = match profile.package_identity {
            PackageIdentity::Unpackaged => false,
            PackageIdentity::Packaged { full_name } if full_name.trim().is_empty() => {
                return Err(EnvironmentBridgeError::EmptyPackageIdentity);
            }
            PackageIdentity::Packaged { .. } => true,
        };

        let environment = WindowsEnvironment {
            major: profile.version.major,
            minor: profile.version.minor,
            build: profile.version.build,
            ubr,
            display_version,
            edition_id,
            edition,
            installation_type: "Client".to_owned(),
            product_type,
            is_workstation: true,
            region,
            native_architecture: native_architecture.to_owned(),
            process_architecture: process_architecture.to_owned(),
            packaged,
        };
        environment
            .is_certifiable()
            .then_some(environment)
            .ok_or(EnvironmentBridgeError::UncertifiableEnvironment)
    }
}

fn nonempty<'a>(value: &'a str, field: &'static str) -> Result<&'a str, EnvironmentBridgeError> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("unknown") {
        Err(EnvironmentBridgeError::InvalidField(field))
    } else {
        Ok(value)
    }
}

fn canonical_nonempty(value: &str, field: &'static str) -> Result<String, EnvironmentBridgeError> {
    Ok(nonempty(value, field)?.to_ascii_uppercase())
}

fn native_architecture(value: CpuArchitecture) -> Result<&'static str, EnvironmentBridgeError> {
    match value {
        CpuArchitecture::X64 => Ok("x64"),
        CpuArchitecture::Arm64 => Ok("arm64"),
        value => Err(EnvironmentBridgeError::UnsupportedNativeArchitecture(value)),
    }
}

fn process_architecture(value: CpuArchitecture) -> Result<&'static str, EnvironmentBridgeError> {
    match value {
        CpuArchitecture::X86 => Ok("x86"),
        CpuArchitecture::X64 => Ok("x64"),
        CpuArchitecture::Arm64 => Ok("arm64"),
        value => Err(EnvironmentBridgeError::UnsupportedProcessArchitecture(
            value,
        )),
    }
}

fn validate_architecture_pair(
    native: CpuArchitecture,
    process: CpuArchitecture,
) -> Result<(), EnvironmentBridgeError> {
    let possible = matches!(
        (native, process),
        (
            CpuArchitecture::X64,
            CpuArchitecture::X86 | CpuArchitecture::X64
        ) | (
            CpuArchitecture::Arm64,
            CpuArchitecture::X86 | CpuArchitecture::X64 | CpuArchitecture::Arm64
        )
    );
    possible
        .then_some(())
        .ok_or(EnvironmentBridgeError::ImpossibleArchitecturePair { native, process })
}
