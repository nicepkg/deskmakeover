//! The exact Windows environment fingerprint that gates every write.
//!
//! Certification is per-tuple, not per-build: a recipe is writable only on an environment that
//! byte-matches a certified profile across build, UBR, edition, region, and architecture. Every
//! field is fail-closed — an unknown edition/SKU pair, a non-canonical region, or a mismatched
//! `EditionID`/`GetProductInfo` identity is `Unverified`, never "probably supported".

use serde::{Deserialize, Serialize};

/// The normalized Windows edition family. `Other` is only reachable when BOTH the `EditionID`
/// string and the `GetProductInfo` SKU are simultaneously unknown — a value known on only one
/// side is rejected outright so a malformed profile can never masquerade as a certified unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowsEdition {
    Home,
    Pro,
    Enterprise,
    Education,
    Other(String),
}

impl WindowsEdition {
    /// Normalize the registry `EditionID` and the `GetProductInfo` SKU as ONE identity, or
    /// `None` when they disagree or either half is empty/unknown.
    pub fn from_raw_identity(edition_id: &str, product_type: u32) -> Option<Self> {
        Self::canonical_raw_identity(edition_id, product_type).map(|(_, edition)| edition)
    }

    /// Like [`from_raw_identity`](Self::from_raw_identity) but also returns the canonical
    /// upper/mixed-case `EditionID` string the certified profile must store.
    pub fn canonical_raw_identity(edition_id: &str, product_type: u32) -> Option<(String, Self)> {
        let trimmed = edition_id.trim();
        if product_type == 0 || trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
            return None;
        }
        let by_id = known_edition_id(trimmed);
        let by_product = known_product_type(product_type);
        match (by_id, by_product) {
            // Both sides known AND agree on the same product number and edition family.
            (Some((canonical_id, id_product, edition)), Some(product_edition))
                if id_product == product_type && edition == product_edition =>
            {
                Some((canonical_id.to_owned(), edition))
            }
            // Both sides unknown → a genuinely unrecognized edition, kept as `Other` but still
            // requiring its own certified profile row before any write.
            (None, None) => {
                let canonical_id = trimmed.to_ascii_uppercase();
                Some((canonical_id.clone(), Self::Other(canonical_id)))
            }
            // Exactly one side known (or the two disagree) → reject: a masquerade risk.
            _ => None,
        }
    }
}

fn known_edition_id(edition_id: &str) -> Option<(&'static str, u32, WindowsEdition)> {
    const KNOWN: &[(&str, u32, fn() -> WindowsEdition)] = &[
        ("Core", 101, || WindowsEdition::Home),
        ("CoreN", 98, || WindowsEdition::Home),
        ("Professional", 48, || WindowsEdition::Pro),
        ("ProfessionalN", 49, || WindowsEdition::Pro),
        ("Enterprise", 4, || WindowsEdition::Enterprise),
        ("EnterpriseN", 27, || WindowsEdition::Enterprise),
        ("Education", 121, || WindowsEdition::Education),
        ("EducationN", 122, || WindowsEdition::Education),
    ];
    KNOWN
        .iter()
        .find(|(id, _, _)| edition_id.eq_ignore_ascii_case(id))
        .map(|(id, product, edition)| (*id, *product, edition()))
}

fn known_product_type(product_type: u32) -> Option<WindowsEdition> {
    match product_type {
        101 | 98 => Some(WindowsEdition::Home),
        48 | 49 => Some(WindowsEdition::Pro),
        4 | 27 => Some(WindowsEdition::Enterprise),
        121 | 122 => Some(WindowsEdition::Education),
        _ => None,
    }
}

/// The complete certification fingerprint of the Windows install in front of DeskMakeover.
/// A platform probe must canonicalize every field before constructing this; the decision core
/// never re-derives or recombines a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowsEnvironment {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
    pub ubr: u32,
    /// Canonical uppercase `CurrentVersion\DisplayVersion` (e.g. `24H2`).
    pub display_version: String,
    /// Canonical registry `EditionID`, kept alongside its normalized family.
    pub edition_id: String,
    pub edition: WindowsEdition,
    /// Canonical installation identity; a writable certification requires exactly `Client`.
    pub installation_type: String,
    /// Raw SKU from `GetProductInfo` (historically "product type" in the Win32 APIs).
    pub product_type: u32,
    /// `OSVERSIONINFOEXW.wProductType == VER_NT_WORKSTATION`.
    pub is_workstation: bool,
    /// Canonical ISO 3166 alpha-2 or three-digit UN M.49 region code.
    pub region: String,
    /// Native architecture: only canonical `x64` or `arm64` can be certified.
    pub native_architecture: String,
    /// Process architecture (`x86`/`x64`/`arm64`); kept separate so emulation is tested apart.
    pub process_architecture: String,
    /// Package identity affects registry virtualization and API availability.
    pub packaged: bool,
}

impl WindowsEnvironment {
    pub fn is_windows_11(&self) -> bool {
        self.major == 10 && self.minor == 0 && self.build >= 22_000
    }

    /// Exact certification comparison: both sides must be independently certifiable AND equal.
    pub fn matches_certification(&self, other: &Self) -> bool {
        self.is_certifiable() && other.is_certifiable() && self == other
    }

    /// Canonicalize a raw region string to ISO alpha-2 or a three-digit UN M.49 code, or `None`
    /// when it is neither (`ZZ`, empty, and `unknown` all fail closed here).
    pub fn canonical_region(value: &str) -> Option<String> {
        let normalized = value.trim().to_ascii_uppercase();
        let alpha2 = ISO_3166_ALPHA2
            .split_ascii_whitespace()
            .any(|known| known == normalized);
        let m49 = normalized.len() == 3 && normalized.bytes().all(|byte| byte.is_ascii_digit());
        (alpha2 || m49).then_some(normalized)
    }

    /// Whether this fingerprint is well-formed enough to be a certification target at all. A
    /// value that fails this can never match a certified profile, so it is always `Unverified`.
    pub fn is_certifiable(&self) -> bool {
        let known_process_arch = |value: &str| ["x86", "x64", "arm64"].contains(&value);
        let known_native_arch = |value: &str| ["x64", "arm64"].contains(&value);
        let nonempty_known =
            |value: &str| !value.trim().is_empty() && !value.trim().eq_ignore_ascii_case("unknown");
        WindowsEdition::canonical_raw_identity(&self.edition_id, self.product_type)
            .is_some_and(|identity| identity == (self.edition_id.clone(), self.edition.clone()))
            && nonempty_known(&self.display_version)
            && self.display_version == self.display_version.trim().to_ascii_uppercase()
            && self.installation_type == "Client"
            && self.is_workstation
            && Self::canonical_region(&self.region).as_deref() == Some(self.region.as_str())
            && known_native_arch(&self.native_architecture)
            && known_process_arch(&self.process_architecture)
    }
}

/// The complete ISO 3166-1 alpha-2 allowlist, space-separated. A region outside this set (and
/// not a three-digit M.49 code) fails closed.
const ISO_3166_ALPHA2: &str = "AD AE AF AG AI AL AM AO AQ AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ BR BS BT BV BW BY BZ CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ DE DJ DK DM DO DZ EC EE EG EH ER ES ET FI FJ FK FM FO FR GA GB GD GE GF GG GH GI GL GM GN GP GQ GR GS GT GU GW GY HK HM HN HR HT HU ID IE IL IM IN IO IQ IR IS IT JE JM JO JP KE KG KH KI KM KN KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA MC MD ME MF MG MH MK ML MM MN MO MP MQ MR MS MT MU MV MW MX MY MZ NA NC NE NF NG NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM PN PR PS PT PW PY QA RE RO RS RU RW SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ TC TD TF TG TH TJ TK TL TM TN TO TR TT TV TW TZ UA UG UM US UY UZ VA VC VE VG VI VN VU WF WS YE YT ZA ZM ZW";

#[cfg(test)]
pub(crate) fn certified_pro_24h2(build: u32, ubr: u32) -> WindowsEnvironment {
    WindowsEnvironment {
        major: 10,
        minor: 0,
        build,
        ubr,
        display_version: "24H2".into(),
        edition_id: "Professional".into(),
        edition: WindowsEdition::Pro,
        installation_type: "Client".into(),
        product_type: 48,
        is_workstation: true,
        region: "US".into(),
        native_architecture: "x64".into(),
        process_architecture: "x64".into(),
        packaged: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_matching_edition_id_and_sku_normalize_together() {
        assert_eq!(
            WindowsEdition::from_raw_identity("Professional", 48),
            Some(WindowsEdition::Pro)
        );
    }

    #[test]
    fn a_one_sided_known_identity_is_rejected() {
        // Known EditionID string but a mismatched SKU number → masquerade risk → None.
        assert_eq!(WindowsEdition::from_raw_identity("Professional", 4), None);
        // Known SKU but unknown string → None.
        assert_eq!(WindowsEdition::from_raw_identity("Frobozz", 48), None);
    }

    #[test]
    fn a_fully_unknown_pair_is_other_but_still_needs_its_own_profile() {
        let edition = WindowsEdition::from_raw_identity("ServerCore", 500);
        assert_eq!(edition, Some(WindowsEdition::Other("SERVERCORE".into())));
    }

    #[test]
    fn empty_or_unknown_edition_fails_closed() {
        assert_eq!(WindowsEdition::from_raw_identity("", 48), None);
        assert_eq!(WindowsEdition::from_raw_identity("unknown", 48), None);
        assert_eq!(WindowsEdition::from_raw_identity("Professional", 0), None);
    }

    #[test]
    fn region_accepts_alpha2_and_m49_but_not_zz() {
        assert_eq!(WindowsEnvironment::canonical_region("us"), Some("US".into()));
        assert_eq!(WindowsEnvironment::canonical_region("276"), Some("276".into()));
        assert_eq!(WindowsEnvironment::canonical_region("ZZ"), None);
        assert_eq!(WindowsEnvironment::canonical_region(""), None);
    }

    #[test]
    fn a_well_formed_profile_is_certifiable_and_self_matches() {
        let env = certified_pro_24h2(26_100, 8_737);
        assert!(env.is_windows_11());
        assert!(env.is_certifiable());
        assert!(env.matches_certification(&env));
    }

    #[test]
    fn a_non_client_install_is_not_certifiable() {
        let mut env = certified_pro_24h2(26_100, 8_737);
        env.installation_type = "Server".into();
        assert!(!env.is_certifiable());
        assert!(!env.matches_certification(&env));
    }

    #[test]
    fn an_x86_native_arch_is_not_certifiable() {
        let mut env = certified_pro_24h2(26_100, 8_737);
        env.native_architecture = "x86".into();
        assert!(!env.is_certifiable());
    }
}
