use crate::{Capability, SettingId, VerificationManifest, WindowsEnvironment};

/// Product-owned default selection. IDs are UI/domain identifiers only; registry keys remain in
/// the production catalog. Advanced and manual-only capabilities are filtered again at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultSet {
    included: Vec<SettingId>,
}

impl DefaultSet {
    pub fn recommended() -> Self {
        Self {
            included: [
                "searchHighlights",
                "startPromotedRecommendations",
                "notificationSuggestions",
                "notificationWelcome",
                "notificationFinishDeviceSetup",
                "settingsSuggestedContent",
                "explorerSyncProviderNotifications",
                "taskbarSearch",
                "taskbarTaskView",
                "advertisingPersonalization",
            ]
            .into_iter()
            .map(SettingId::new)
            .collect(),
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.included
            .iter()
            .any(|candidate| candidate.as_str() == id)
    }

    /// Defense in depth: even an accidentally included advanced/manual ID is not selected.
    pub fn writable_for(
        &self,
        manifest: &VerificationManifest,
        environment: &WindowsEnvironment,
    ) -> Vec<SettingId> {
        self.included
            .iter()
            .filter(|id| manifest.evaluate(id, environment) == Capability::Available)
            .cloned()
            .collect()
    }
}
