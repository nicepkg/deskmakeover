use deskmakeover_windows_settings_reference::*;

fn certifiable_environment() -> WindowsEnvironment {
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
}

#[test]
fn native_x86_is_not_a_certifiable_windows_11_profile() {
    let allowed = certifiable_environment();
    let candidate = WindowsEnvironment {
        native_architecture: "x86".into(),
        process_architecture: "x86".into(),
        ..allowed.clone()
    };
    assert!(!allowed.matches_certification(&candidate));
    assert!(!candidate.matches_certification(&candidate));
}

#[test]
fn every_environment_field_participates_in_certification() {
    let allowed = certifiable_environment();
    let candidates = [
        WindowsEnvironment {
            major: 11,
            ..allowed.clone()
        },
        WindowsEnvironment {
            minor: 1,
            ..allowed.clone()
        },
        WindowsEnvironment {
            build: 26_200,
            ..allowed.clone()
        },
        WindowsEnvironment {
            ubr: 8_738,
            ..allowed.clone()
        },
        WindowsEnvironment {
            display_version: "25H2".into(),
            ..allowed.clone()
        },
        WindowsEnvironment {
            edition_id: "ProfessionalN".into(),
            ..allowed.clone()
        },
        WindowsEnvironment {
            edition: WindowsEdition::Enterprise,
            ..allowed.clone()
        },
        WindowsEnvironment {
            installation_type: "Server".into(),
            ..allowed.clone()
        },
        WindowsEnvironment {
            product_type: 49,
            ..allowed.clone()
        },
        WindowsEnvironment {
            is_workstation: false,
            ..allowed.clone()
        },
        WindowsEnvironment {
            region: "US".into(),
            ..allowed.clone()
        },
        WindowsEnvironment {
            native_architecture: "arm64".into(),
            ..allowed.clone()
        },
        WindowsEnvironment {
            process_architecture: "x86".into(),
            ..allowed.clone()
        },
        WindowsEnvironment {
            packaged: true,
            ..allowed.clone()
        },
    ];
    for candidate in candidates {
        assert!(!allowed.matches_certification(&candidate), "{candidate:?}");
    }
}

#[test]
fn edition_identity_rejects_known_id_or_sku_mismatches() {
    assert_eq!(
        WindowsEdition::from_raw_identity("Professional", 48),
        Some(WindowsEdition::Pro)
    );
    assert_eq!(WindowsEdition::from_raw_identity("Professional", 49), None);
    assert_eq!(WindowsEdition::from_raw_identity("FutureEdition", 48), None);
    assert_eq!(
        WindowsEdition::from_raw_identity("FutureEdition", 999),
        Some(WindowsEdition::Other("FUTUREEDITION".into()))
    );
}

#[test]
fn geography_is_canonical_alpha2_or_three_digit_m49() {
    assert_eq!(
        WindowsEnvironment::canonical_region(" cn "),
        Some("CN".into())
    );
    assert_eq!(
        WindowsEnvironment::canonical_region("156"),
        Some("156".into())
    );
    for invalid in ["", "unknown", "ZZ", "15", "15A"] {
        assert_eq!(WindowsEnvironment::canonical_region(invalid), None);
    }
    let mut noncanonical = certifiable_environment();
    noncanonical.region = "cn".into();
    assert!(!noncanonical.is_certifiable());
}

#[test]
fn empty_key_path_never_enumerates_the_hive_root_as_a_created_prefix() {
    let key = RegistryKey::new(RegistryHive::CurrentUser, RegistryView::Registry64, "");
    assert!(key.is_hive_root());
    assert!(key.prefixes().is_empty());
}
