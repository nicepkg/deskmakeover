use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SettingId(String);

impl SettingId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SettingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegistryHive {
    CurrentUser,
    LocalMachine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegistryView {
    Native,
    Registry32,
    Registry64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistryAddress {
    pub hive: RegistryHive,
    pub view: RegistryView,
    pub key: String,
    pub value: String,
}

impl RegistryAddress {
    pub fn new(
        hive: RegistryHive,
        view: RegistryView,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            hive,
            view,
            key: key.into(),
            value: value.into(),
        }
    }

    pub fn key_location(&self) -> RegistryKey {
        RegistryKey::new(self.hive, self.view, self.key.clone())
    }
}

impl fmt::Display for RegistryAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}\\{}\\{}", self.hive, self.key, self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistryKey {
    pub hive: RegistryHive,
    pub view: RegistryView,
    pub path: String,
}

impl RegistryKey {
    pub fn new(hive: RegistryHive, view: RegistryView, path: impl Into<String>) -> Self {
        Self {
            hive,
            view,
            path: path.into(),
        }
    }

    /// Enumerates concrete subkey prefixes without ever returning the hive root.
    pub fn prefixes(&self) -> Vec<Self> {
        let components = self
            .path
            .split('\\')
            .filter(|component| !component.is_empty())
            .collect::<Vec<_>>();
        (1..=components.len())
            .map(|length| Self::new(self.hive, self.view, components[..length].join("\\")))
            .collect()
    }

    pub fn depth(&self) -> usize {
        self.path
            .split('\\')
            .filter(|component| !component.is_empty())
            .count()
    }

    pub fn is_hive_root(&self) -> bool {
        self.depth() == 0
    }
}

impl fmt::Display for RegistryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}\\{}", self.hive, self.path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegistryValueKind {
    None,
    String,
    ExpandString,
    Binary,
    Dword,
    MultiString,
    Qword,
    Other(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRegistryValue {
    pub kind: RegistryValueKind,
    pub bytes: Vec<u8>,
}

impl RawRegistryValue {
    pub fn new(kind: RegistryValueKind, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            kind,
            bytes: bytes.into(),
        }
    }

    pub fn dword(value: u32) -> Self {
        Self::new(RegistryValueKind::Dword, value.to_le_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrySnapshot {
    /// The complete key path did not exist.
    KeyMissing,
    /// The key existed but this value did not.
    ValueMissing,
    Present(RawRegistryValue),
}

impl RegistrySnapshot {
    pub fn key_existed(&self) -> bool {
        !matches!(self, Self::KeyMissing)
    }

    pub fn value(&self) -> Option<&RawRegistryValue> {
        match self {
            Self::Present(value) => Some(value),
            Self::KeyMissing | Self::ValueMissing => None,
        }
    }

    pub fn kind(&self) -> Option<&RegistryValueKind> {
        self.value().map(|value| &value.kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingMutation {
    pub address: RegistryAddress,
    pub desired: RegistrySnapshot,
    pub accepted_existing_kinds: Vec<RegistryValueKind>,
    pub missing_policy: MissingPolicy,
}

impl SettingMutation {
    pub fn accepts(&self, snapshot: &RegistrySnapshot) -> bool {
        match snapshot {
            RegistrySnapshot::KeyMissing | RegistrySnapshot::ValueMissing => {
                self.missing_policy == MissingPolicy::CreateAllowed
            }
            RegistrySnapshot::Present(value) => {
                self.accepted_existing_kinds.contains(&value.kind)
                    && (value.kind != RegistryValueKind::Dword || value.bytes.len() == 4)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPolicy {
    /// This documented primary preference may create a missing value (and, when journaled, key).
    CreateAllowed,
    /// The value is an existing-only companion or an advanced recipe leaf. Missing is inapplicable.
    MustAlreadyExist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectVerifier {
    DelayedReadBackAndSettingsUi,
    SearchLocalNonceHasNoWebAffordance,
    StartPromotionsAbsentAndKnownRecentPreserved,
    DeviceUsageAllOffAndPrioritiesPreserved,
    AdvertisingIdIsEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationBudget {
    max_settle_millis: u32,
    max_attempts: u8,
}

impl VerificationBudget {
    pub const DEFAULT: Self = Self {
        max_settle_millis: 5_000,
        max_attempts: 3,
    };

    pub fn max_settle_millis(self) -> u32 {
        self.max_settle_millis
    }

    pub fn max_attempts(self) -> u8 {
        self.max_attempts
    }

    pub fn is_bounded(self) -> bool {
        self.max_settle_millis > 0 && self.max_attempts > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptSnapshot {
    pub address: RegistryAddress,
    pub snapshot: RegistrySnapshot,
}

/// Durable pre-write evidence used by both terminal verification and crash recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationReceipt {
    NoBaseline,
    StartKnownRecent { marker: String },
    DeviceUsagePriorities { priorities: Vec<ReceiptSnapshot> },
}

/// Durable, typed requirement for proving a transaction's terminal state.
///
/// Raw delayed read-back is always enforced by the engine. `effect` selects the additional
/// feature-level proof that the injected verification backend must execute before journal commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationPlan {
    pub effect: EffectVerifier,
    pub budget: VerificationBudget,
}

impl VerificationPlan {
    pub fn new(effect: EffectVerifier) -> Self {
        Self {
            effect,
            budget: VerificationBudget::DEFAULT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingDefinition {
    pub(crate) id: SettingId,
    pub(crate) recipe_version: u32,
    pub(crate) mutations: Vec<SettingMutation>,
    pub(crate) verification: VerificationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedValue {
    pub address: RegistryAddress,
    pub snapshot: RegistrySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyRequest {
    pub(crate) feature: SettingId,
    pub(crate) expected: Vec<ExpectedValue>,
    pub(crate) environment_fingerprint: WindowsEnvironment,
}

impl ApplyRequest {
    pub fn feature(&self) -> &SettingId {
        &self.feature
    }

    pub fn expected(&self) -> &[ExpectedValue] {
        &self.expected
    }

    pub fn environment_fingerprint(&self) -> &WindowsEnvironment {
        &self.environment_fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreRequest {
    pub feature: SettingId,
    pub expected: Vec<ExpectedValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsEdition {
    Home,
    Pro,
    Enterprise,
    Education,
    Other(String),
}

impl WindowsEdition {
    /// Normalizes the registry `EditionID` and `GetProductInfo` SKU as one identity.
    ///
    /// A known value on only one side is rejected: accepting it as `Other` would let a malformed
    /// profile masquerade as a separately certified unknown edition.
    pub fn from_raw_identity(edition_id: &str, product_type: u32) -> Option<Self> {
        Self::canonical_raw_identity(edition_id, product_type).map(|(_, edition)| edition)
    }

    pub fn canonical_raw_identity(edition_id: &str, product_type: u32) -> Option<(String, Self)> {
        let trimmed = edition_id.trim();
        if product_type == 0 || trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
            return None;
        }

        let known_by_id = known_edition_id(trimmed);
        let known_by_product = known_product_type(product_type);
        match (known_by_id, known_by_product) {
            (Some((canonical_id, id_product, edition)), Some(product_edition))
                if id_product == product_type && edition == product_edition =>
            {
                Some((canonical_id.to_owned(), edition))
            }
            (None, None) => {
                let canonical_id = trimmed.to_ascii_uppercase();
                Some((canonical_id.clone(), Self::Other(canonical_id)))
            }
            _ => None,
        }
    }
}

fn known_edition_id(edition_id: &str) -> Option<(&'static str, u32, WindowsEdition)> {
    if edition_id.eq_ignore_ascii_case("Core") {
        Some(("Core", 101, WindowsEdition::Home))
    } else if edition_id.eq_ignore_ascii_case("CoreN") {
        Some(("CoreN", 98, WindowsEdition::Home))
    } else if edition_id.eq_ignore_ascii_case("Professional") {
        Some(("Professional", 48, WindowsEdition::Pro))
    } else if edition_id.eq_ignore_ascii_case("ProfessionalN") {
        Some(("ProfessionalN", 49, WindowsEdition::Pro))
    } else if edition_id.eq_ignore_ascii_case("Enterprise") {
        Some(("Enterprise", 4, WindowsEdition::Enterprise))
    } else if edition_id.eq_ignore_ascii_case("EnterpriseN") {
        Some(("EnterpriseN", 27, WindowsEdition::Enterprise))
    } else if edition_id.eq_ignore_ascii_case("Education") {
        Some(("Education", 121, WindowsEdition::Education))
    } else if edition_id.eq_ignore_ascii_case("EducationN") {
        Some(("EducationN", 122, WindowsEdition::Education))
    } else {
        None
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsEnvironment {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
    pub ubr: u32,
    /// Canonical uppercase identity derived from `CurrentVersion\\DisplayVersion`.
    pub display_version: String,
    /// Canonical registry identity derived from `EditionID`, retained with the normalized family.
    pub edition_id: String,
    pub edition: WindowsEdition,
    /// Canonical installation identity; writable certification requires exactly `Client`.
    pub installation_type: String,
    /// Raw SKU returned by `GetProductInfo` (historically named product type in Win32 APIs).
    pub product_type: u32,
    /// `OSVERSIONINFOEXW.wProductType == VER_NT_WORKSTATION`.
    pub is_workstation: bool,
    pub region: String,
    /// Windows 11 native architecture: only canonical `x64` or `arm64` can be certified.
    pub native_architecture: String,
    /// Canonical `x86`, `x64`, or `arm64`; kept separate so emulation is independently tested.
    pub process_architecture: String,
    /// Package identity affects registry virtualization and API availability.
    pub packaged: bool,
}

impl WindowsEnvironment {
    pub fn is_windows_11(&self) -> bool {
        self.major == 10 && self.minor == 0 && self.build >= 22_000
    }

    /// Exact certification fingerprint comparison. Platform bridges must canonicalize fields
    /// before constructing this value; no field is discarded or independently recombined.
    pub fn matches_certification(&self, other: &Self) -> bool {
        self.is_certifiable() && other.is_certifiable() && self == other
    }

    pub fn canonical_region(value: &str) -> Option<String> {
        let normalized = value.trim().to_ascii_uppercase();
        let alpha2 = ISO_3166_ALPHA2
            .split_ascii_whitespace()
            .any(|known| known == normalized);
        let m49 = normalized.len() == 3 && normalized.bytes().all(|byte| byte.is_ascii_digit());
        (alpha2 || m49).then_some(normalized)
    }

    pub fn is_certifiable(&self) -> bool {
        let known_process_architecture = |value: &str| ["x86", "x64", "arm64"].contains(&value);
        let known_native_architecture = |value: &str| ["x64", "arm64"].contains(&value);
        let nonempty_known =
            |value: &str| !value.trim().is_empty() && !value.trim().eq_ignore_ascii_case("unknown");
        WindowsEdition::canonical_raw_identity(&self.edition_id, self.product_type)
            .is_some_and(|identity| identity == (self.edition_id.clone(), self.edition.clone()))
            && nonempty_known(&self.display_version)
            && self.display_version == self.display_version.trim().to_ascii_uppercase()
            && self.installation_type == "Client"
            && self.is_workstation
            && Self::canonical_region(&self.region).as_deref() == Some(self.region.as_str())
            && known_native_architecture(&self.native_architecture)
            && known_process_architecture(&self.process_architecture)
    }
}

const ISO_3166_ALPHA2: &str = "AD AE AF AG AI AL AM AO AQ AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ BR BS BT BV BW BY BZ CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ DE DJ DK DM DO DZ EC EE EG EH ER ES ET FI FJ FK FM FO FR GA GB GD GE GF GG GH GI GL GM GN GP GQ GR GS GT GU GW GY HK HM HN HR HT HU ID IE IL IM IN IO IQ IR IS IT JE JM JO JP KE KG KH KI KM KN KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA MC MD ME MF MG MH MK ML MM MN MO MP MQ MR MS MT MU MV MW MX MY MZ NA NC NE NF NG NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM PN PR PS PT PW PY QA RE RO RS RU RW SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ TC TD TF TG TH TJ TK TL TM TN TO TR TT TV TW TZ UA UG UM US UY UZ VA VC VE VG VI VN VU WF WS YE YT ZA ZM ZW";
