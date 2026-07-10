//! Settings DTOs — the Rust source for the TS `SettingsDto`/`SettingsPatch` in
//! `bridge/types.ts` (BRIDGE_SCHEMA_VERSION = 4). Field names and enum string
//! literals mirror the TS shape exactly so the generated bindings drop in place.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Appearance theme. Serialized as its variant name (`"System"` | `"Dark"` |
/// `"Light"`) to match the TS union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Theme {
    System,
    Dark,
    Light,
}

/// UI language. `System` follows the OS UI culture; the two concrete tags use
/// the BCP-47 spellings the TS dictionaries key on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Language {
    System,
    #[serde(rename = "zh-Hans")]
    ZhHans,
    #[serde(rename = "en")]
    En,
}

/// The full persisted settings row (spec 03 appearance + participation coach).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub theme: Theme,
    pub language: Language,
    pub keep_new_icons_styled: bool,
    pub wallpaper_coach_shown: bool,
}

impl Default for SettingsDto {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: Language::System,
            keep_new_icons_styled: false,
            wallpaper_coach_shown: false,
        }
    }
}

/// Sparse update over [`SettingsDto`] — every field absent (or `null`) leaves
/// the stored value untouched. Mirrors the TS `SettingsPatch = Partial<SettingsDto>`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPatch {
    pub theme: Option<Theme>,
    pub language: Option<Language>,
    pub keep_new_icons_styled: Option<bool>,
    pub wallpaper_coach_shown: Option<bool>,
}

impl SettingsDto {
    /// Apply a patch in place, ignoring absent fields.
    pub fn apply(&mut self, patch: &SettingsPatch) {
        if let Some(theme) = patch.theme {
            self.theme = theme;
        }
        if let Some(language) = patch.language {
            self.language = language;
        }
        if let Some(keep) = patch.keep_new_icons_styled {
            self.keep_new_icons_styled = keep;
        }
        if let Some(shown) = patch.wallpaper_coach_shown {
            self.wallpaper_coach_shown = shown;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_tags_match_ts_literals() {
        assert_eq!(serde_json::to_string(&Theme::System).unwrap(), "\"System\"");
        assert_eq!(
            serde_json::to_string(&Language::ZhHans).unwrap(),
            "\"zh-Hans\""
        );
        assert_eq!(serde_json::to_string(&Language::En).unwrap(), "\"en\"");
    }

    #[test]
    fn dto_uses_camel_case_keys() {
        let json = serde_json::to_string(&SettingsDto::default()).unwrap();
        assert!(json.contains("\"keepNewIconsStyled\""));
        assert!(json.contains("\"wallpaperCoachShown\""));
    }

    #[test]
    fn patch_leaves_absent_fields_untouched() {
        let mut dto = SettingsDto::default();
        let patch: SettingsPatch = serde_json::from_str("{\"theme\":\"Dark\"}").unwrap();
        dto.apply(&patch);
        assert_eq!(dto.theme, Theme::Dark);
        assert_eq!(dto.language, Language::System);
        assert!(!dto.keep_new_icons_styled);
    }
}
