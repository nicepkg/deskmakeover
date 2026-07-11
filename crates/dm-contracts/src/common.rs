//! Cross-module bridge DTOs shared by more than one command family.

use serde::{Deserialize, Serialize};
use specta::Type;

/// A localized toast the host asks the shell to show: `key` is an i18n key and
/// `arg` an optional interpolation argument. Mirrors the TS `ToastDto`. Shared:
/// the wallpaper module (Wave A) and the icons module (Wave B) both return it
/// from mutating ops.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToastDto {
    pub key: String,
    pub arg: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_round_trips_and_uses_ts_keys() {
        let toast = ToastDto { key: "wallpaper.applied".into(), arg: None };
        let json = serde_json::to_string(&toast).unwrap();
        assert_eq!(json, r#"{"key":"wallpaper.applied","arg":null}"#);
        let back: ToastDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back, toast);
    }
}
