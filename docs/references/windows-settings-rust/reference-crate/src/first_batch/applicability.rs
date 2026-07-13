use crate::{LockScreenBackground, RuntimeFacts, WindowsEnvironment};

use super::Applicability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicabilityFailure {
    EnvironmentChanged,
    EeaRegion(String),
    UnknownRegion(String),
    LockScreenBackground(LockScreenBackground),
}

pub(super) fn check(
    applicability: Applicability,
    resolved: &RuntimeFacts,
    current: &RuntimeFacts,
) -> Result<(), ApplicabilityFailure> {
    if !resolved
        .environment
        .matches_certification(&current.environment)
    {
        return Err(ApplicabilityFailure::EnvironmentChanged);
    }
    match applicability {
        Applicability::AnyCertifiedEnvironment => Ok(()),
        Applicability::NonEea => check_non_eea(&current.environment.region),
        Applicability::LockScreenPictureOrSlideshow => match current.lock_screen_background {
            LockScreenBackground::Picture | LockScreenBackground::Slideshow => Ok(()),
            background => Err(ApplicabilityFailure::LockScreenBackground(background)),
        },
    }
}

fn check_non_eea(region: &str) -> Result<(), ApplicabilityFailure> {
    let Some(normalized) = WindowsEnvironment::canonical_region(region) else {
        return Err(ApplicabilityFailure::UnknownRegion(region.into()));
    };
    if normalized.len() != 2 {
        return Err(ApplicabilityFailure::UnknownRegion(region.into()));
    }
    if EEA
        .split_ascii_whitespace()
        .any(|value| value == normalized)
    {
        return Err(ApplicabilityFailure::EeaRegion(region.into()));
    }
    Ok(())
}

const EEA: &str =
    "AT BE BG HR CY CZ DK EE FI FR DE GR HU IE IT LV LT LU MT NL PL PT RO SK SI ES SE IS LI NO";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WindowsEdition, WindowsEnvironment};

    fn facts(region: &str, background: LockScreenBackground) -> RuntimeFacts {
        RuntimeFacts {
            environment: WindowsEnvironment {
                major: 10,
                minor: 0,
                build: 26_100,
                ubr: 1,
                display_version: "24H2".into(),
                edition_id: "Professional".into(),
                edition: WindowsEdition::Pro,
                installation_type: "Client".into(),
                product_type: 48,
                is_workstation: true,
                region: region.into(),
                native_architecture: "x64".into(),
                process_architecture: "x64".into(),
                packaged: false,
            },
            lock_screen_background: background,
        }
    }

    #[test]
    fn non_eea_rejects_eea_and_unknown_regions() {
        let eea = facts("DE", LockScreenBackground::Unknown);
        assert!(matches!(
            check(Applicability::NonEea, &eea, &eea),
            Err(ApplicabilityFailure::EeaRegion(_))
        ));
        for region in ["276", "156", "001"] {
            let runtime = facts(region, LockScreenBackground::Unknown);
            assert!(matches!(
                check(Applicability::NonEea, &runtime, &runtime),
                Err(ApplicabilityFailure::UnknownRegion(_))
            ));
        }
        for region in ["de", "ZZ", ""] {
            let runtime = facts(region, LockScreenBackground::Unknown);
            assert!(check(Applicability::NonEea, &runtime, &runtime).is_err());
        }
        let runtime = facts("CN", LockScreenBackground::Unknown);
        assert_eq!(check(Applicability::NonEea, &runtime, &runtime), Ok(()));
    }

    #[test]
    fn lock_screen_requires_picture_or_slideshow() {
        for background in [
            LockScreenBackground::Unknown,
            LockScreenBackground::Spotlight,
        ] {
            let runtime = facts("US", background);
            assert!(check(
                Applicability::LockScreenPictureOrSlideshow,
                &runtime,
                &runtime
            )
            .is_err());
        }
    }
}
