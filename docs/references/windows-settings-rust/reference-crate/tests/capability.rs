mod common;

use common::*;
use deskmakeover_windows_settings_reference::*;

#[test]
fn advanced_feature_requires_the_complete_certification_fingerprint() {
    let feature = id("searchLocalOnly");
    let allowed = environment();
    let manifest =
        VerificationManifest::new([(feature.clone(), VerificationRule::Advanced(vec![allowed]))]);

    assert_eq!(
        manifest.evaluate(&feature, &environment()),
        Capability::Advanced
    );
    for changed in [
        WindowsEnvironment {
            ubr: 4_201,
            ..environment()
        },
        WindowsEnvironment {
            edition: WindowsEdition::Home,
            ..environment()
        },
        WindowsEnvironment {
            region: "US".into(),
            ..environment()
        },
        WindowsEnvironment {
            build: 26_101,
            ..environment()
        },
        WindowsEnvironment {
            native_architecture: "arm64".into(),
            ..environment()
        },
        WindowsEnvironment {
            process_architecture: "x86".into(),
            ..environment()
        },
        WindowsEnvironment {
            packaged: true,
            ..environment()
        },
    ] {
        assert_eq!(
            manifest.evaluate(&feature, &changed),
            Capability::Unavailable(UnavailableReason::AdvancedEnvironmentNotAllowlisted)
        );
    }
}

#[test]
fn unknown_future_and_unlisted_insider_builds_fail_closed() {
    let feature = id("searchHighlights");
    let manifest = VerificationManifest::new([(feature.clone(), standard_rule())]);

    let future = WindowsEnvironment {
        build: 30_000,
        ubr: 1,
        ..environment()
    };
    assert_eq!(
        manifest.evaluate(&feature, &future),
        Capability::Unavailable(UnavailableReason::FutureBuildNotVerified)
    );

    let numeric_gap = WindowsEnvironment {
        build: 25_000,
        ubr: 1,
        ..environment()
    };
    assert_eq!(
        manifest.evaluate(&feature, &numeric_gap),
        Capability::Unavailable(UnavailableReason::BuildNotVerified)
    );
}

#[test]
fn build_22000_requires_the_verified_minimum_ubr() {
    let feature = id("searchHighlights");
    let manifest = VerificationManifest::new([(feature.clone(), standard_rule())]);
    let too_old = WindowsEnvironment {
        build: 22_000,
        ubr: 1_760,
        ..environment()
    };
    let verified = WindowsEnvironment {
        build: 22_000,
        ubr: 1_761,
        ..environment()
    };

    assert_eq!(
        manifest.evaluate(&feature, &too_old),
        Capability::Unavailable(UnavailableReason::BuildNotVerified)
    );
    assert_eq!(
        manifest.evaluate(&feature, &verified),
        Capability::Available
    );
}

#[test]
fn standard_rules_require_bounded_ubr_and_explicit_runtime_profiles() {
    let feature = id("searchHighlights");
    let base = StandardVerification {
        families: vec![VerifiedBuildFamily {
            build: 26_100,
            min_ubr: 4_000,
            max_ubr: Some(5_000),
        }],
        profiles: vec![environment()],
    };
    for unsafe_rule in [
        StandardVerification {
            families: vec![VerifiedBuildFamily {
                max_ubr: None,
                ..base.families[0].clone()
            }],
            ..base.clone()
        },
        StandardVerification {
            profiles: Vec::new(),
            ..base.clone()
        },
        StandardVerification {
            profiles: vec![WindowsEnvironment {
                process_architecture: "arm64".into(),
                ..environment()
            }],
            ..base.clone()
        },
        StandardVerification {
            profiles: vec![WindowsEnvironment {
                native_architecture: "arm64".into(),
                ..environment()
            }],
            ..base.clone()
        },
        StandardVerification {
            profiles: vec![WindowsEnvironment {
                packaged: true,
                ..environment()
            }],
            ..base.clone()
        },
        StandardVerification {
            profiles: vec![WindowsEnvironment {
                native_architecture: String::new(),
                ..environment()
            }],
            ..base.clone()
        },
    ] {
        let manifest =
            VerificationManifest::new([(feature.clone(), VerificationRule::Standard(unsafe_rule))]);
        assert!(!manifest.evaluate(&feature, &environment()).permits_write());
    }
}

#[test]
fn recommended_defaults_preserve_recent_and_exclude_advanced_manual_and_device_usage() {
    let defaults = DefaultSet::recommended();

    assert!(defaults.contains("startPromotedRecommendations"));
    assert!(
        !defaults.contains("startRecent"),
        "Recent stays enabled by default"
    );
    assert!(
        !defaults.contains("searchLocalOnly"),
        "advanced is opt-in only"
    );
    assert!(
        !defaults.contains("lockScreenTips"),
        "Spotlight-coupled tips stay advanced"
    );
    assert!(!defaults.contains("deviceUsageRecommendations"));
    for manual in [
        "widgetsNews",
        "widgetsHover",
        "widgetsBadges",
        "widgetsAnnouncements",
        "lockScreenStatus",
        "taskbarWidgets",
    ] {
        assert!(
            !defaults.contains(manual),
            "manual-only item leaked into defaults: {manual}"
        );
    }
}

#[test]
fn default_runtime_filter_accepts_only_standard_available_items() {
    let defaults = DefaultSet::recommended();
    let standard = id("searchHighlights");
    let advanced = id("searchLocalOnly");
    let manual = id("widgetsNews");
    let manifest = VerificationManifest::new([
        (standard.clone(), standard_rule()),
        (advanced, VerificationRule::Advanced(vec![environment()])),
        (manual, VerificationRule::ManualOnly),
    ]);

    assert_eq!(
        defaults.writable_for(&manifest, &environment()),
        vec![standard]
    );
}
