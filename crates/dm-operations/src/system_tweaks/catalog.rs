//! The compile-time recipe catalog for the 清爽 (calm-Windows) first batch.
//!
//! Setting ids match the frontend catalog (`src/lib/calm/catalog.ts`) EXACTLY, so the bridge
//! maps a row to its recipe with no translation table. Registry recipes are the exact HKCU
//! DWORD writes from the research handoff (`docs/references/windows-settings-rust/README.md`
//! §Exact registry recipes). Descriptors are implementation candidates: writability is gated
//! separately by the capability manifest, which is initially empty for every direct write.

use dm_domain::system_tweaks::{
    MissingPolicy, RawRegistryValue, RegistryAddress, RegistryHive, RegistrySnapshot,
    RegistryValueKind, RegistryView, SettingId, SettingMutation,
};

/// The tier a setting occupies in the honest three-group grammar (ADR-0023 D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweakTier {
    /// A one-click automatic write, gated by the capability manifest.
    AutomaticCandidate,
    /// A stricter write requiring an exact-environment allowlist entry.
    Advanced,
    /// No stable programmable setter — the app walks the user there, never writes.
    Guided,
}

/// The feature-specific proof that a write actually took effect. A matching registry value is
/// NEVER sufficient on its own; the effect verifier confirms the surface reloaded the change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectVerifier {
    /// Delayed raw read-back plus the Settings UI reflecting the change.
    DelayedReadBackAndSettingsUi,
    /// Start promotions gone AND a known Recent item still present (never claim all of Start).
    StartPromotionsAbsentAndKnownRecentPreserved,
    /// The Advertising ID reads empty after the write.
    AdvertisingIdIsEmpty,
}

/// A policy guard: an HKLM/HKCU policy value whose presence means the feature is managed. It is
/// only ever read; the app never overwrites or deletes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyGuard {
    pub address: RegistryAddress,
    pub note: &'static str,
}

/// A registry value the recipe must NEVER touch even though it sits nearby (it has broad
/// collateral side effects). Recorded so the resolver can reject a recipe that names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForbiddenMutation {
    pub address: RegistryAddress,
    pub reason: &'static str,
}

/// A single documented `ms-settings:` / Widgets route for a guided setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualRoute {
    SettingsPage(&'static str),
    WidgetsBoardSettings,
}

/// One catalog entry: its id, tier, the mutations it establishes, guards, and effect verifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TweakDescriptor {
    pub id: SettingId,
    pub recipe_version: u32,
    pub tier: TweakTier,
    pub mutations: Vec<SettingMutation>,
    pub policy_guards: Vec<PolicyGuard>,
    pub forbidden_mutations: Vec<ForbiddenMutation>,
    pub manual_route: Option<ManualRoute>,
    pub effect_verifier: Option<EffectVerifier>,
}

const EXPLORER_ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const SEARCH: &str = r"Software\Microsoft\Windows\CurrentVersion\Search";
const SEARCH_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\SearchSettings";
const CONTENT_DELIVERY: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
const USER_PROFILE_ENGAGEMENT: &str =
    r"Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement";

/// A per-user (HKCU, 64-bit view) DWORD write mutation that may create a missing value.
fn hkcu_dword(key: &str, value: &str, desired: u32) -> SettingMutation {
    SettingMutation {
        address: RegistryAddress::new(RegistryHive::CurrentUser, RegistryView::Registry64, key, value),
        desired: RegistrySnapshot::Present(RawRegistryValue::dword(desired)),
        accepted_existing_kinds: vec![RegistryValueKind::Dword],
        missing_policy: MissingPolicy::CreateAllowed,
    }
}

/// An HKLM policy guard leaf (64-bit view), read-only.
fn hklm_guard(key: &str, value: &str, note: &'static str) -> PolicyGuard {
    PolicyGuard {
        address: RegistryAddress::new(RegistryHive::LocalMachine, RegistryView::Registry64, key, value),
        note,
    }
}

fn automatic(id: &str, mutation: SettingMutation, effect: EffectVerifier) -> TweakDescriptor {
    TweakDescriptor {
        id: SettingId::new(id),
        recipe_version: 1,
        tier: TweakTier::AutomaticCandidate,
        mutations: vec![mutation],
        policy_guards: Vec::new(),
        forbidden_mutations: Vec::new(),
        manual_route: None,
        effect_verifier: Some(effect),
    }
}

fn guided(id: &str, route: ManualRoute) -> TweakDescriptor {
    TweakDescriptor {
        id: SettingId::new(id),
        recipe_version: 1,
        tier: TweakTier::Guided,
        mutations: Vec::new(),
        policy_guards: Vec::new(),
        forbidden_mutations: Vec::new(),
        manual_route: Some(route),
        effect_verifier: None,
    }
}

/// The full first-batch catalog. Ids are the frontend's `CalmControlId` strings verbatim.
pub fn first_batch() -> Vec<TweakDescriptor> {
    use EffectVerifier::*;

    let mut start = automatic(
        "start.recommendations",
        hkcu_dword(EXPLORER_ADVANCED, "Start_IrisRecommendations", 0),
        StartPromotionsAbsentAndKnownRecentPreserved,
    );
    // Start_TrackDocs also controls Explorer Recent and Jump Lists — never touched here.
    start.forbidden_mutations = vec![ForbiddenMutation {
        address: RegistryAddress::new(
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            EXPLORER_ADVANCED,
            "Start_TrackDocs",
        ),
        reason: "would also disable Explorer Recent and Jump Lists",
    }];

    let mut taskbar_search = automatic(
        "taskbar.search",
        hkcu_dword(SEARCH, "SearchboxTaskbarMode", 0),
        DelayedReadBackAndSettingsUi,
    );
    // The Pro+ machine policy SearchOnTaskbarMode uses a DIFFERENT enum; observe only.
    taskbar_search.policy_guards = vec![hklm_guard(
        r"Software\Policies\Microsoft\Windows\Windows Search",
        "SearchOnTaskbarMode",
        "taskbar Search machine policy; observe only, different enum",
    )];

    let taskbar_taskview = automatic(
        "taskbar.taskview",
        hkcu_dword(EXPLORER_ADVANCED, "ShowTaskViewButton", 0),
        DelayedReadBackAndSettingsUi,
    );

    let mut search_highlights = automatic(
        "search.highlights",
        hkcu_dword(SEARCH_SETTINGS, "IsDynamicSearchBoxEnabled", 0),
        DelayedReadBackAndSettingsUi,
    );
    search_highlights.policy_guards = vec![hklm_guard(
        r"SOFTWARE\Policies\Microsoft\Windows\Windows Search",
        "EnableDynamicContentInWSB",
        "Pro+ management policy; never overwrite or delete it",
    )];

    let notif_suggestions = automatic(
        "notifications.suggestions",
        hkcu_dword(CONTENT_DELIVERY, "SubscribedContent-338389Enabled", 0),
        DelayedReadBackAndSettingsUi,
    );
    let notif_welcome = automatic(
        "notifications.welcome",
        hkcu_dword(CONTENT_DELIVERY, "SubscribedContent-310093Enabled", 0),
        DelayedReadBackAndSettingsUi,
    );
    let finish_setup = automatic(
        "notifications.finishSetup",
        hkcu_dword(USER_PROFILE_ENGAGEMENT, "ScoobeSystemSettingEnabled", 0),
        DelayedReadBackAndSettingsUi,
    );
    let settings_suggestions = automatic(
        "settings.suggestions",
        hkcu_dword(CONTENT_DELIVERY, "SubscribedContent-338393Enabled", 0),
        DelayedReadBackAndSettingsUi,
    );
    let explorer_sync = automatic(
        "explorer.syncNotifications",
        hkcu_dword(EXPLORER_ADVANCED, "ShowSyncProviderNotifications", 0),
        DelayedReadBackAndSettingsUi,
    );

    vec![
        start,
        taskbar_search,
        taskbar_taskview,
        search_highlights,
        notif_suggestions,
        notif_welcome,
        finish_setup,
        settings_suggestions,
        explorer_sync,
        // Guided rows — no stable setter; the app opens the route and never writes.
        guided("widgets.feed", ManualRoute::WidgetsBoardSettings),
        guided("taskbar.widgetsButton", ManualRoute::SettingsPage("ms-settings:taskbar")),
        guided("lockscreen.status", ManualRoute::SettingsPage("ms-settings:lockscreen")),
        guided("tray.entries", ManualRoute::SettingsPage("ms-settings:taskbar")),
    ]
}

/// A validated catalog: construction rejects duplicate ids and registry-resource collisions so a
/// lookup can never silently resolve the wrong recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TweakCatalog {
    descriptors: Vec<TweakDescriptor>,
}

/// A catalog construction failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    DuplicateId(SettingId),
    /// Two settings write the same registry address — one apply would clobber the other's leaf.
    ResourceCollision(RegistryAddress),
    /// An automatic/advanced recipe with no effect verifier could never prove it took effect.
    MissingEffectVerifier(SettingId),
    /// A writable recipe with no primary mutation.
    NoMutations(SettingId),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate setting id: {id}"),
            Self::ResourceCollision(address) => write!(f, "registry resource collision: {address}"),
            Self::MissingEffectVerifier(id) => write!(f, "missing effect verifier: {id}"),
            Self::NoMutations(id) => write!(f, "writable recipe has no mutation: {id}"),
        }
    }
}

impl std::error::Error for CatalogError {}

impl TweakCatalog {
    pub fn try_new(descriptors: Vec<TweakDescriptor>) -> Result<Self, CatalogError> {
        let mut seen_ids = std::collections::BTreeSet::new();
        let mut seen_addresses = std::collections::BTreeSet::new();
        for descriptor in &descriptors {
            if !seen_ids.insert(descriptor.id.clone()) {
                return Err(CatalogError::DuplicateId(descriptor.id.clone()));
            }
            let writable =
                matches!(descriptor.tier, TweakTier::AutomaticCandidate | TweakTier::Advanced);
            if writable {
                if descriptor.mutations.is_empty() {
                    return Err(CatalogError::NoMutations(descriptor.id.clone()));
                }
                if descriptor.effect_verifier.is_none() {
                    return Err(CatalogError::MissingEffectVerifier(descriptor.id.clone()));
                }
            }
            for mutation in &descriptor.mutations {
                if !seen_addresses.insert(mutation.address.clone()) {
                    return Err(CatalogError::ResourceCollision(mutation.address.clone()));
                }
            }
        }
        Ok(Self { descriptors })
    }

    /// The validated first-batch catalog.
    pub fn first_batch() -> Result<Self, CatalogError> {
        Self::try_new(first_batch())
    }

    pub fn descriptors(&self) -> &[TweakDescriptor] {
        &self.descriptors
    }

    pub fn descriptor(&self, id: &SettingId) -> Option<&TweakDescriptor> {
        self.descriptors.iter().find(|d| &d.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_batch_catalog_is_valid() {
        let catalog = TweakCatalog::first_batch().expect("first batch must validate");
        assert_eq!(catalog.descriptors().len(), 13);
    }

    #[test]
    fn ids_match_the_frontend_starter_slice() {
        let catalog = TweakCatalog::first_batch().unwrap();
        for id in ["start.recommendations", "taskbar.search", "taskbar.taskview"] {
            let descriptor = catalog.descriptor(&SettingId::new(id)).expect("id present");
            assert_eq!(descriptor.tier, TweakTier::AutomaticCandidate);
            assert!(descriptor.effect_verifier.is_some());
        }
    }

    #[test]
    fn guided_rows_carry_a_route_and_never_a_mutation() {
        let catalog = TweakCatalog::first_batch().unwrap();
        for id in [
            "widgets.feed",
            "taskbar.widgetsButton",
            "lockscreen.status",
            "tray.entries",
        ] {
            let descriptor = catalog.descriptor(&SettingId::new(id)).unwrap();
            assert_eq!(descriptor.tier, TweakTier::Guided);
            assert!(descriptor.manual_route.is_some());
            assert!(descriptor.mutations.is_empty());
        }
    }

    #[test]
    fn start_recommendations_forbids_track_docs() {
        let catalog = TweakCatalog::first_batch().unwrap();
        let start = catalog
            .descriptor(&SettingId::new("start.recommendations"))
            .unwrap();
        assert!(start
            .forbidden_mutations
            .iter()
            .any(|forbidden| forbidden.address.value == "Start_TrackDocs"));
    }

    #[test]
    fn a_duplicate_id_is_rejected() {
        let one = automatic(
            "dup",
            hkcu_dword(EXPLORER_ADVANCED, "A", 0),
            EffectVerifier::DelayedReadBackAndSettingsUi,
        );
        let two = automatic(
            "dup",
            hkcu_dword(EXPLORER_ADVANCED, "B", 0),
            EffectVerifier::DelayedReadBackAndSettingsUi,
        );
        assert_eq!(
            TweakCatalog::try_new(vec![one, two]),
            Err(CatalogError::DuplicateId(SettingId::new("dup")))
        );
    }

    #[test]
    fn two_settings_writing_the_same_address_collide() {
        let one = automatic(
            "one",
            hkcu_dword(EXPLORER_ADVANCED, "Same", 0),
            EffectVerifier::DelayedReadBackAndSettingsUi,
        );
        let two = automatic(
            "two",
            hkcu_dword(EXPLORER_ADVANCED, "Same", 0),
            EffectVerifier::DelayedReadBackAndSettingsUi,
        );
        assert!(matches!(
            TweakCatalog::try_new(vec![one, two]),
            Err(CatalogError::ResourceCollision(_))
        ));
    }
}

// TweakCatalog::try_new needs Ord on RegistryAddress for the BTreeSet dedup — provided by the
// derive on RegistryAddress in dm-domain.
