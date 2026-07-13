use deskmakeover_windows_settings_reference::{WindowsEdition, WindowsEnvironment};
use dm_windows_settings_platform::{
    CpuArchitecture, EnvironmentBridgeError, PackageIdentity, SystemProfile, WindowsVersion,
};

fn profile() -> SystemProfile {
    SystemProfile {
        version: WindowsVersion {
            major: 10,
            minor: 0,
            build: 26_100,
            revision: Some(8_737),
        },
        display_version: "24H2".into(),
        edition_id: "Professional".into(),
        installation_type: "Client".into(),
        product_type: Some(48),
        is_workstation: true,
        region: Some("CN".into()),
        native_architecture: CpuArchitecture::X64,
        process_architecture: CpuArchitecture::X64,
        package_identity: PackageIdentity::Unpackaged,
    }
}

#[test]
fn conversion_is_complete_and_canonical() {
    let mut source = profile();
    source.display_version = " 24h2 ".into();
    source.edition_id = " professional ".into();
    source.installation_type = " client ".into();
    source.region = Some(" cn ".into());

    let environment = WindowsEnvironment::try_from(source).unwrap();
    assert_eq!(
        environment,
        WindowsEnvironment {
            major: 10,
            minor: 0,
            build: 26_100,
            ubr: 8_737,
            display_version: "24H2".into(),
            edition_id: "Professional".into(),
            edition: WindowsEdition::Pro,
            installation_type: "Client".into(),
            product_type: 48,
            is_workstation: true,
            region: "CN".into(),
            native_architecture: "x64".into(),
            process_architecture: "x64".into(),
            packaged: false,
        }
    );
    assert!(environment.is_certifiable());
}

#[test]
fn all_documented_edition_sku_pairs_normalize_consistently() {
    for (raw_id, sku, canonical_id, edition) in [
        ("Core", 101, "Core", WindowsEdition::Home),
        ("CoreN", 98, "CoreN", WindowsEdition::Home),
        ("Professional", 48, "Professional", WindowsEdition::Pro),
        ("ProfessionalN", 49, "ProfessionalN", WindowsEdition::Pro),
        ("Enterprise", 4, "Enterprise", WindowsEdition::Enterprise),
        ("EnterpriseN", 27, "EnterpriseN", WindowsEdition::Enterprise),
        ("Education", 121, "Education", WindowsEdition::Education),
        ("EducationN", 122, "EducationN", WindowsEdition::Education),
    ] {
        let mut source = profile();
        source.edition_id = raw_id.to_ascii_lowercase();
        source.product_type = Some(sku);
        let environment = WindowsEnvironment::try_from(source).unwrap();
        assert_eq!(environment.edition_id, canonical_id);
        assert_eq!(environment.edition, edition);
    }
}

#[test]
fn genuinely_unknown_nonzero_identity_requires_its_own_other_profile() {
    let mut source = profile();
    source.edition_id = " futureEdition ".into();
    source.product_type = Some(999);
    source.region = Some("156".into());

    let environment = WindowsEnvironment::try_from(source).unwrap();
    assert_eq!(environment.edition_id, "FUTUREEDITION");
    assert_eq!(
        environment.edition,
        WindowsEdition::Other("FUTUREEDITION".into())
    );
    assert_eq!(environment.region, "156");
}

#[test]
fn missing_and_invalid_profile_dimensions_fail_closed() {
    let mut cases = Vec::new();

    let mut value = profile();
    value.version.revision = None;
    cases.push((value, EnvironmentBridgeError::MissingRevision));

    let mut value = profile();
    value.version.major = 11;
    cases.push((
        value.clone(),
        EnvironmentBridgeError::NotWindows11(value.version),
    ));

    for (field, mutate) in [("DisplayVersion", 0_u8), ("EditionID", 1), ("region", 2)] {
        let mut value = profile();
        match mutate {
            0 => value.display_version = "unknown".into(),
            1 => value.edition_id.clear(),
            _ => value.region = Some("ZZ".into()),
        }
        cases.push((value, EnvironmentBridgeError::InvalidField(field)));
    }

    let mut value = profile();
    value.installation_type = "Server".into();
    cases.push((
        value,
        EnvironmentBridgeError::NonClientInstallation("Server".into()),
    ));

    let mut value = profile();
    value.is_workstation = false;
    cases.push((value, EnvironmentBridgeError::NotWorkstation));

    let mut value = profile();
    value.product_type = None;
    cases.push((value, EnvironmentBridgeError::MissingProductType));

    let mut value = profile();
    value.edition_id = "ProfessionalN".into();
    cases.push((
        value,
        EnvironmentBridgeError::EditionProductMismatch {
            edition_id: "ProfessionalN".into(),
            product_type: 48,
        },
    ));

    let mut value = profile();
    value.region = None;
    cases.push((value, EnvironmentBridgeError::MissingRegion));

    let mut value = profile();
    value.native_architecture = CpuArchitecture::X86;
    cases.push((
        value,
        EnvironmentBridgeError::UnsupportedNativeArchitecture(CpuArchitecture::X86),
    ));

    let mut value = profile();
    value.process_architecture = CpuArchitecture::Unknown;
    cases.push((
        value,
        EnvironmentBridgeError::UnsupportedProcessArchitecture(CpuArchitecture::Unknown),
    ));

    let mut value = profile();
    value.process_architecture = CpuArchitecture::Arm64;
    cases.push((
        value,
        EnvironmentBridgeError::ImpossibleArchitecturePair {
            native: CpuArchitecture::X64,
            process: CpuArchitecture::Arm64,
        },
    ));

    let mut value = profile();
    value.package_identity = PackageIdentity::Packaged {
        full_name: " ".into(),
    };
    cases.push((value, EnvironmentBridgeError::EmptyPackageIdentity));

    for (source, expected) in cases {
        assert_eq!(WindowsEnvironment::try_from(source), Err(expected));
    }
}
