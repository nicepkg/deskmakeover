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

/// The Windows-case-insensitive identity of a registry address (hive, view, lowercased key,
/// lowercased value). Two recipes that differ only by case target the SAME registry value, so a
/// collision check must fold case.
fn address_identity(address: &RegistryAddress) -> (RegistryHive, RegistryView, String, String) {
    (
        address.hive,
        address.view,
        address.key.to_ascii_lowercase(),
        address.value.to_ascii_lowercase(),
    )
}

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

/// A single documented `ms-settings:` route. Carried by guided rows (no stable setter) AND by
/// fail-closed automatic rows whose official settings page is known — until the W3 lab certifies
/// a write, an automatic row still walks the user to the switch instead of dead-ending (ADR-0023
/// D2 group 2 「带你去系统里关的」). URIs are `&'static` literals from THIS catalog only — the
/// frontend never supplies one, so `open_route` has no injection surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualRoute {
    SettingsPage(&'static str),
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
    /// Guided rows only: whether the app can re-probe a readable off/on state after the walk
    /// (mirrors the frontend catalog's `readableState`). `None` for writable rows.
    pub readable_state: Option<bool>,
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
        readable_state: None,
    }
}

fn guided(id: &str, route: ManualRoute, readable_state: bool) -> TweakDescriptor {
    TweakDescriptor {
        id: SettingId::new(id),
        recipe_version: 1,
        tier: TweakTier::Guided,
        mutations: Vec::new(),
        policy_guards: Vec::new(),
        forbidden_mutations: Vec::new(),
        manual_route: Some(route),
        effect_verifier: None,
        readable_state: Some(readable_state),
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
    // Settings › Personalization › Start › Show recommendations.
    start.manual_route = Some(ManualRoute::SettingsPage("ms-settings:personalization-start"));

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
    // Settings › Personalization › Taskbar › Search.
    taskbar_search.manual_route = Some(ManualRoute::SettingsPage("ms-settings:taskbar"));

    let mut taskbar_taskview = automatic(
        "taskbar.taskview",
        hkcu_dword(EXPLORER_ADVANCED, "ShowTaskViewButton", 0),
        DelayedReadBackAndSettingsUi,
    );
    // Settings › Personalization › Taskbar › Task view (same page as taskbar Search).
    taskbar_taskview.manual_route = Some(ManualRoute::SettingsPage("ms-settings:taskbar"));

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
    // Settings › Privacy & security › Search permissions › Show search highlights.
    search_highlights.manual_route = Some(ManualRoute::SettingsPage("ms-settings:search-permissions"));

    let mut notif_suggestions = automatic(
        "notifications.suggestions",
        hkcu_dword(CONTENT_DELIVERY, "SubscribedContent-338389Enabled", 0),
        DelayedReadBackAndSettingsUi,
    );
    notif_suggestions.manual_route = Some(ManualRoute::SettingsPage("ms-settings:notifications"));
    let mut notif_welcome = automatic(
        "notifications.welcome",
        hkcu_dword(CONTENT_DELIVERY, "SubscribedContent-310093Enabled", 0),
        DelayedReadBackAndSettingsUi,
    );
    notif_welcome.manual_route = Some(ManualRoute::SettingsPage("ms-settings:notifications"));
    let mut finish_setup = automatic(
        "notifications.finishSetup",
        hkcu_dword(USER_PROFILE_ENGAGEMENT, "ScoobeSystemSettingEnabled", 0),
        DelayedReadBackAndSettingsUi,
    );
    finish_setup.manual_route = Some(ManualRoute::SettingsPage("ms-settings:notifications"));
    let mut settings_suggestions = automatic(
        "settings.suggestions",
        hkcu_dword(CONTENT_DELIVERY, "SubscribedContent-338393Enabled", 0),
        DelayedReadBackAndSettingsUi,
    );
    // Settings › Privacy & security › General › Show me suggested content in the Settings app.
    settings_suggestions.manual_route = Some(ManualRoute::SettingsPage("ms-settings:privacy-general"));
    // explorer.syncNotifications has NO ms-settings page (File Explorer Options › View), so it
    // carries no route and stays honestly held until the W3 lab certifies a direct write.
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
        // Guided rows — no stable setter; the app opens the route and never writes. Only the
        // taskbar Widgets button exposes a readable off/on state (mirrors the frontend catalog).
        // The widgets board itself has no ms-settings deep link, so the feed row routes to the
        // taskbar page (owner 2026-07-16). It opens a real window (the old inert Win+W caption did
        // not); the row copy offers the taskbar Widgets entry and points to Win+W for the feed
        // itself, and NEVER claims the taskbar toggle alone removes the feed (the board can persist).
        guided("widgets.feed", ManualRoute::SettingsPage("ms-settings:taskbar"), false),
        guided("taskbar.widgetsButton", ManualRoute::SettingsPage("ms-settings:taskbar"), true),
        guided("lockscreen.status", ManualRoute::SettingsPage("ms-settings:lockscreen"), false),
        guided("tray.entries", ManualRoute::SettingsPage("ms-settings:taskbar"), false),
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
    /// Two settings write the same registry address (Windows-case-insensitive) — one apply would
    /// clobber the other's leaf.
    ResourceCollision(RegistryAddress),
    /// An automatic/advanced recipe with no effect verifier could never prove it took effect.
    MissingEffectVerifier(SettingId),
    /// A writable recipe with no primary mutation.
    NoMutations(SettingId),
    /// A guided descriptor carries a mutation (a guided row is never written).
    GuidedWithMutation(SettingId),
    /// A mutation writes a value the same descriptor lists as forbidden.
    MutatesForbidden(RegistryAddress),
    /// A writable desired value is not a concrete standard-kind write (a deletion, or `Other(raw)`
    /// extension type, which a recipe must never establish).
    IllegalDesired(RegistryAddress),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate setting id: {id}"),
            Self::ResourceCollision(address) => write!(f, "registry resource collision: {address}"),
            Self::MissingEffectVerifier(id) => write!(f, "missing effect verifier: {id}"),
            Self::NoMutations(id) => write!(f, "writable recipe has no mutation: {id}"),
            Self::GuidedWithMutation(id) => write!(f, "guided descriptor carries a mutation: {id}"),
            Self::MutatesForbidden(address) => write!(f, "recipe mutates a forbidden value: {address}"),
            Self::IllegalDesired(address) => write!(f, "illegal desired value: {address}"),
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
            if descriptor.tier == TweakTier::Guided && !descriptor.mutations.is_empty() {
                return Err(CatalogError::GuidedWithMutation(descriptor.id.clone()));
            }
            if writable {
                if descriptor.mutations.is_empty() {
                    return Err(CatalogError::NoMutations(descriptor.id.clone()));
                }
                if descriptor.effect_verifier.is_none() {
                    return Err(CatalogError::MissingEffectVerifier(descriptor.id.clone()));
                }
            }
            let forbidden: std::collections::BTreeSet<_> = descriptor
                .forbidden_mutations
                .iter()
                .map(|forbidden| address_identity(&forbidden.address))
                .collect();
            for mutation in &descriptor.mutations {
                if forbidden.contains(&address_identity(&mutation.address)) {
                    return Err(CatalogError::MutatesForbidden(mutation.address.clone()));
                }
                if !is_legal_desired(&mutation.desired) {
                    return Err(CatalogError::IllegalDesired(mutation.address.clone()));
                }
                // W1 is DWORD-only: every accepted existing kind must be `REG_DWORD`. Anything
                // else (String/Binary/Qword/Other) would let a non-DWORD live value pass
                // `accepts()` into a write path.
                if mutation
                    .accepted_existing_kinds
                    .iter()
                    .any(|kind| *kind != RegistryValueKind::Dword)
                {
                    return Err(CatalogError::IllegalDesired(mutation.address.clone()));
                }
                if !seen_addresses.insert(address_identity(&mutation.address)) {
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

/// W1 recipes establish EXACTLY a well-formed 4-byte `REG_DWORD`. Any other kind (String, Binary,
/// Qword, Other), a malformed DWORD width, or a deletion is never a legitimate desired write —
/// this is the honest W1 DWORD-only boundary, enforced not just documented (codex W1 R3 #6).
fn is_legal_desired(desired: &RegistrySnapshot) -> bool {
    match desired {
        RegistrySnapshot::Present(value) => {
            value.kind == RegistryValueKind::Dword && value.bytes.len() == 4
        }
        RegistrySnapshot::ValueMissing | RegistrySnapshot::KeyMissing => false,
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
    fn every_fail_closed_automatic_row_with_an_official_page_carries_a_walk_route() {
        // ADR-0023 D2 group 2 (owner 2026-07-16): an uncertified automatic row must WALK the user
        // to its official settings page, never dead-end at 「本版本不支持」. Only the pageless
        // sync-notifications row keeps no route. A route never turns a row writable — the
        // capability manifest still gates every write independently.
        let catalog = TweakCatalog::first_batch().unwrap();
        for id in [
            "start.recommendations",
            "taskbar.search",
            "taskbar.taskview",
            "search.highlights",
            "notifications.suggestions",
            "notifications.welcome",
            "notifications.finishSetup",
            "settings.suggestions",
        ] {
            let descriptor = catalog.descriptor(&SettingId::new(id)).unwrap();
            assert_eq!(descriptor.tier, TweakTier::AutomaticCandidate, "{id}");
            assert!(descriptor.manual_route.is_some(), "{id} must carry a walk route");
        }
        // No ms-settings page exists for File Explorer sync notices, so it stays routeless (held).
        let sync = catalog
            .descriptor(&SettingId::new("explorer.syncNotifications"))
            .unwrap();
        assert!(sync.manual_route.is_none());
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
