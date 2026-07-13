//! Typed handoff catalog for DeskMakeover's first Windows 11 calm-settings batch.
//!
//! Recipes are implementation candidates, not shipping certification. The initial manifest at the
//! bottom deliberately grants no B/C-level direct write until an exact Windows VM run is recorded.

use crate::{
    ExactEnvironment, MissingPolicy, RawRegistryValue, RegistryAddress, RegistryHive,
    RegistrySnapshot, RegistryValueKind, RegistryView, SettingId, SettingMutation,
    VerificationManifest, VerificationRule,
};

pub use crate::model::EffectVerifier;

mod applicability;
mod builders;
mod planner;
mod protection;

use builders::*;

pub use applicability::ApplicabilityFailure;
pub use planner::{FirstBatchCatalog, FirstBatchPlanError};
pub use protection::ProtectedReason;
pub type ResolvedRecipe = planner::ResolvedRecipe;

pub mod ids {
    pub const SEARCH_HIGHLIGHTS: &str = "searchHighlights";
    pub const SEARCH_LOCAL_ONLY: &str = "searchLocalOnly";
    pub const WIDGETS_NEWS: &str = "widgetsNews";
    pub const WIDGETS_HOVER: &str = "widgetsHover";
    pub const WIDGETS_BADGES: &str = "widgetsBadges";
    pub const WIDGETS_ANNOUNCEMENTS: &str = "widgetsAnnouncements";
    pub const START_PROMOTIONS: &str = "startPromotedRecommendations";
    pub const START_RECENT: &str = "startRecent";
    pub const LOCK_SCREEN_STATUS: &str = "lockScreenStatus";
    pub const LOCK_SCREEN_TIPS: &str = "lockScreenTips";
    pub const NOTIFICATION_SUGGESTIONS: &str = "notificationSuggestions";
    pub const NOTIFICATION_WELCOME: &str = "notificationWelcome";
    pub const FINISH_DEVICE_SETUP: &str = "notificationFinishDeviceSetup";
    pub const SETTINGS_SUGGESTED_CONTENT: &str = "settingsSuggestedContent";
    pub const DEVICE_USAGE: &str = "deviceUsageRecommendations";
    pub const EXPLORER_SYNC_NOTIFICATIONS: &str = "explorerSyncProviderNotifications";
    pub const TASKBAR_SEARCH: &str = "taskbarSearch";
    pub const TASKBAR_WIDGETS: &str = "taskbarWidgets";
    pub const TASKBAR_TASK_VIEW: &str = "taskbarTaskView";
    pub const SYSTEM_TRAY: &str = "systemTrayEntries";
    pub const ADVERTISING_PERSONALIZATION: &str = "advertisingPersonalization";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstBatchTier {
    AutomaticCandidate,
    Advanced,
    Guided,
    Invariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceLevel {
    MicrosoftContract,
    MicrosoftImplementation,
    CommunityObserved,
    NoStableSetter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    AnyCertifiedEnvironment,
    NonEea,
    LockScreenPictureOrSlideshow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuxiliaryCondition {
    /// Never create blindly. Require the value to exist and the exact environment to be certified.
    IfPresentAndExactEnvironmentVerified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuxiliaryMutation {
    pub mutation: SettingMutation,
    pub condition: AuxiliaryCondition,
    /// Empty fails closed. Each entry certifies this companion value only for one exact Windows
    /// build + UBR + edition + region tuple.
    pub exact_environment_allowlist: Vec<ExactEnvironment>,
    pub note: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyGuard {
    pub address: RegistryAddress,
    pub note: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForbiddenMutation {
    pub address: RegistryAddress,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualFallback {
    SettingsPage(&'static str),
    WidgetsBoardSettings,
    EeaSearchProviderSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstBatchSetting {
    pub id: SettingId,
    pub recipe_version: u32,
    pub tier: FirstBatchTier,
    pub evidence: EvidenceLevel,
    pub applicability: Applicability,
    pub mutations: Vec<SettingMutation>,
    pub auxiliary_mutations: Vec<AuxiliaryMutation>,
    pub policy_guards: Vec<PolicyGuard>,
    pub forbidden_mutations: Vec<ForbiddenMutation>,
    pub manual_fallback: Option<ManualFallback>,
    pub effect_verifier: Option<EffectVerifier>,
    pub notes: Vec<&'static str>,
}

const SEARCH_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\SearchSettings";
const SEARCH: &str = r"Software\Microsoft\Windows\CurrentVersion\Search";
const EXPLORER_ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const CONTENT_DELIVERY: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
const USER_PROFILE_ENGAGEMENT: &str =
    r"Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement";
const CLOUD_CONTENT_POLICY: &str = r"Software\Policies\Microsoft\Windows\CloudContent";
const EXPLORER_POLICY: &str = r"Software\Policies\Microsoft\Windows\Explorer";

pub fn first_batch_settings() -> Vec<FirstBatchSetting> {
    use ids::*;

    let start_track_docs = forbidden(
        EXPLORER_ADVANCED,
        "Start_TrackDocs",
        "Would also disable Explorer Recent and Jump Lists",
    );
    let mut settings = vec![
        recipe(
            SEARCH_HIGHLIGHTS,
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
            vec![dword(SEARCH_SETTINGS, "IsDynamicSearchBoxEnabled", 0)],
        ),
        recipe(
            SEARCH_LOCAL_ONLY,
            FirstBatchTier::Advanced,
            EvidenceLevel::CommunityObserved,
            vec![dword(
                r"Software\Policies\Microsoft\Windows\Explorer",
                "DisableSearchBoxSuggestions",
                1,
            )],
        ),
        recipe(
            START_PROMOTIONS,
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
            vec![dword(EXPLORER_ADVANCED, "Start_IrisRecommendations", 0)],
        ),
        invariant(START_RECENT, EvidenceLevel::MicrosoftContract),
        recipe(
            LOCK_SCREEN_TIPS,
            FirstBatchTier::Advanced,
            EvidenceLevel::CommunityObserved,
            vec![
                existing_dword(CONTENT_DELIVERY, "SubscribedContent-338387Enabled", 0),
                existing_dword(CONTENT_DELIVERY, "RotatingLockScreenOverlayEnabled", 0),
            ],
        ),
        recipe(
            NOTIFICATION_SUGGESTIONS,
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
            vec![dword(
                CONTENT_DELIVERY,
                "SubscribedContent-338389Enabled",
                0,
            )],
        ),
        recipe(
            NOTIFICATION_WELCOME,
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
            vec![dword(
                CONTENT_DELIVERY,
                "SubscribedContent-310093Enabled",
                0,
            )],
        ),
        recipe(
            FINISH_DEVICE_SETUP,
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
            vec![dword(
                USER_PROFILE_ENGAGEMENT,
                "ScoobeSystemSettingEnabled",
                0,
            )],
        ),
        recipe(
            SETTINGS_SUGGESTED_CONTENT,
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
            vec![dword(
                CONTENT_DELIVERY,
                "SubscribedContent-338393Enabled",
                0,
            )],
        ),
        recipe(
            DEVICE_USAGE,
            FirstBatchTier::Advanced,
            EvidenceLevel::CommunityObserved,
            device_usage_mutations(),
        ),
        recipe(
            EXPLORER_SYNC_NOTIFICATIONS,
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
            vec![dword(EXPLORER_ADVANCED, "ShowSyncProviderNotifications", 0)],
        ),
        recipe(
            TASKBAR_SEARCH,
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
            vec![dword(SEARCH, "SearchboxTaskbarMode", 0)],
        ),
        recipe(
            TASKBAR_TASK_VIEW,
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
            vec![dword(EXPLORER_ADVANCED, "ShowTaskViewButton", 0)],
        ),
        recipe(
            ADVERTISING_PERSONALIZATION,
            FirstBatchTier::AutomaticCandidate,
            EvidenceLevel::MicrosoftImplementation,
            vec![dword(
                r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo",
                "Enabled",
                0,
            )],
        ),
    ];

    settings.extend([
        guided(WIDGETS_NEWS, ManualFallback::WidgetsBoardSettings),
        guided(WIDGETS_HOVER, ManualFallback::WidgetsBoardSettings),
        guided(WIDGETS_BADGES, ManualFallback::WidgetsBoardSettings),
        guided(WIDGETS_ANNOUNCEMENTS, ManualFallback::WidgetsBoardSettings),
        guided(
            LOCK_SCREEN_STATUS,
            ManualFallback::SettingsPage("ms-settings:lockscreen"),
        ),
        guided(
            TASKBAR_WIDGETS,
            ManualFallback::SettingsPage("ms-settings:taskbar"),
        ),
        guided(
            SYSTEM_TRAY,
            ManualFallback::SettingsPage("ms-settings:taskbar"),
        ),
    ]);

    setting_mut(&mut settings, SEARCH_HIGHLIGHTS).policy_guards = vec![guard(
        RegistryHive::LocalMachine,
        r"SOFTWARE\Policies\Microsoft\Windows\Windows Search",
        "EnableDynamicContentInWSB",
        "Pro+ management policy; never overwrite or delete it",
    )];
    setting_mut(&mut settings, SEARCH_LOCAL_ONLY).manual_fallback =
        Some(ManualFallback::EeaSearchProviderSettings);
    setting_mut(&mut settings, SEARCH_LOCAL_ONLY).applicability = Applicability::NonEea;
    setting_mut(&mut settings, SEARCH_LOCAL_ONLY).effect_verifier =
        Some(EffectVerifier::SearchLocalNonceHasNoWebAffordance);
    setting_mut(&mut settings, SEARCH_LOCAL_ONLY).policy_guards = vec![guard(
        RegistryHive::LocalMachine,
        r"SOFTWARE\Policies\Microsoft\Windows\Windows Search",
        "ConnectedSearchUseWeb",
        "Different enterprise policy contract; observe only",
    )];
    setting_mut(&mut settings, START_PROMOTIONS).forbidden_mutations =
        vec![start_track_docs.clone()];
    setting_mut(&mut settings, START_PROMOTIONS).effect_verifier =
        Some(EffectVerifier::StartPromotionsAbsentAndKnownRecentPreserved);
    setting_mut(&mut settings, START_RECENT).forbidden_mutations = vec![start_track_docs];
    setting_mut(&mut settings, LOCK_SCREEN_TIPS).manual_fallback =
        Some(ManualFallback::SettingsPage("ms-settings:lockscreen"));
    setting_mut(&mut settings, LOCK_SCREEN_TIPS).applicability =
        Applicability::LockScreenPictureOrSlideshow;
    setting_mut(&mut settings, LOCK_SCREEN_TIPS).policy_guards = vec![
        guard(
            RegistryHive::CurrentUser,
            CLOUD_CONTENT_POLICY,
            "ConfigureWindowsSpotlight",
            "Spotlight configuration policy; observe only",
        ),
        guard(
            RegistryHive::CurrentUser,
            CLOUD_CONTENT_POLICY,
            "DisableWindowsSpotlightFeatures",
            "Spotlight feature policy; observe only",
        ),
    ];
    setting_mut(&mut settings, LOCK_SCREEN_TIPS).notes = vec![
        "Picture/Slideshow only after lab proof",
        "Never claim Spotlight image/content separation",
    ];
    setting_mut(&mut settings, NOTIFICATION_SUGGESTIONS).auxiliary_mutations = vec![auxiliary(
        CONTENT_DELIVERY,
        "SoftLandingEnabled",
        "Observed companion value; existing-only and exact-build certified",
    )];
    setting_mut(&mut settings, NOTIFICATION_SUGGESTIONS).policy_guards = vec![guard(
        RegistryHive::LocalMachine,
        CLOUD_CONTENT_POLICY,
        "DisableSoftLanding",
        "Soft landing policy; observe only",
    )];
    setting_mut(&mut settings, NOTIFICATION_WELCOME).policy_guards = vec![guard(
        RegistryHive::CurrentUser,
        CLOUD_CONTENT_POLICY,
        "DisableWindowsSpotlightWindowsWelcomeExperience",
        "Windows welcome policy; observe only",
    )];
    setting_mut(&mut settings, SETTINGS_SUGGESTED_CONTENT).auxiliary_mutations = vec![
        auxiliary(
            CONTENT_DELIVERY,
            "SubscribedContent-353694Enabled",
            "Observed companion value; existing-only and exact-build certified",
        ),
        auxiliary(
            CONTENT_DELIVERY,
            "SubscribedContent-353696Enabled",
            "Observed companion value; existing-only and exact-build certified",
        ),
        auxiliary(
            CONTENT_DELIVERY,
            "SubscribedContent-353698Enabled",
            "Observed companion value; existing-only and exact-build certified",
        ),
    ];
    setting_mut(&mut settings, SETTINGS_SUGGESTED_CONTENT).policy_guards = vec![guard(
        RegistryHive::CurrentUser,
        CLOUD_CONTENT_POLICY,
        "DisableWindowsSpotlightOnSettings",
        "Settings suggested-content policy; observe only",
    )];
    setting_mut(&mut settings, DEVICE_USAGE).notes = vec![
        "Composite all-off operation only",
        "Preserve every Priority value",
        "Reduces category recommendations; does not disable telemetry or all ads",
    ];
    setting_mut(&mut settings, DEVICE_USAGE).effect_verifier =
        Some(EffectVerifier::DeviceUsageAllOffAndPrioritiesPreserved);
    setting_mut(&mut settings, EXPLORER_SYNC_NOTIFICATIONS).notes =
        vec!["May also suppress legitimate sync-provider education/status notices"];
    setting_mut(&mut settings, TASKBAR_SEARCH).notes =
        vec!["Hides the entry, not Win+S or Search itself"];
    setting_mut(&mut settings, TASKBAR_SEARCH).policy_guards = vec![guard(
        RegistryHive::LocalMachine,
        r"Software\Policies\Microsoft\Windows\Windows Search",
        "SearchOnTaskbarMode",
        "Taskbar Search policy; observe only",
    )];
    setting_mut(&mut settings, TASKBAR_TASK_VIEW).policy_guards = vec![
        guard(
            RegistryHive::CurrentUser,
            EXPLORER_POLICY,
            "HideTaskViewButton",
            "Per-user Task View policy; observe only",
        ),
        guard(
            RegistryHive::LocalMachine,
            EXPLORER_POLICY,
            "HideTaskViewButton",
            "Machine Task View policy; observe only",
        ),
    ];
    setting_mut(&mut settings, ADVERTISING_PERSONALIZATION).policy_guards = vec![guard(
        RegistryHive::LocalMachine,
        r"Software\Policies\Microsoft\Windows\AdvertisingInfo",
        "DisabledByGroupPolicy",
        "Managed by organization; never delete it",
    )];
    setting_mut(&mut settings, ADVERTISING_PERSONALIZATION).manual_fallback =
        Some(ManualFallback::SettingsPage("ms-settings:privacy-general"));
    setting_mut(&mut settings, ADVERTISING_PERSONALIZATION).notes = vec![
        "Copy must say reduce personalized tracking, never reduce ad count",
        "Re-enabling creates a new Advertising ID; the prior ID is not restored",
    ];
    setting_mut(&mut settings, ADVERTISING_PERSONALIZATION).effect_verifier =
        Some(EffectVerifier::AdvertisingIdIsEmpty);
    setting_mut(&mut settings, TASKBAR_WIDGETS).notes = vec![
        "TaskbarDa is observed but UCPD may reject or revert third-party writes",
        "Do not bypass UCPD",
    ];
    settings
}

/// Production entry point. Unlike a `StaticCatalog` projection, this retains the complete typed
/// descriptors and refuses duplicate IDs or registry-resource collisions before it can resolve a
/// writable recipe.
pub fn first_batch_catalog() -> Result<FirstBatchCatalog, FirstBatchPlanError> {
    FirstBatchCatalog::try_new(first_batch_settings())
}

/// Empty by design. Automatic B-level candidates have no rule; Advanced C-level rules have an
/// empty exact-environment allowlist; guided items are ManualOnly; invariants have no rule.
pub fn initial_verification_manifest() -> VerificationManifest {
    VerificationManifest::new(first_batch_settings().into_iter().filter_map(|setting| {
        let rule = match setting.tier {
            FirstBatchTier::Advanced => VerificationRule::Advanced(Vec::new()),
            FirstBatchTier::Guided => VerificationRule::ManualOnly,
            FirstBatchTier::AutomaticCandidate | FirstBatchTier::Invariant => return None,
        };
        Some((setting.id, rule))
    }))
}
