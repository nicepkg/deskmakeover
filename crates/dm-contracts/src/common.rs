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

/// Environment snapshot for the diagnostics report (mirrors the TS `SystemInfoDto`). Returned by
/// `diagnostics.getInfo` — the audit #7 fix that replaces the browser `(mock)` stub (which shipped
/// even on the real Tauri app, so a Windows diagnostics report read `osVersion: "Win32 (mock)"`)
/// with real host facts. `hostLogTail` stays empty until the host error-log buffer is wired (F8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfoDto {
    pub os_version: String,
    pub webview2_version: String,
    pub arch: String,
    pub host_log_tail: Vec<String>,
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

    #[test]
    fn system_info_uses_the_ts_camelcase_keys() {
        let info = SystemInfoDto {
            os_version: "macos".into(),
            webview2_version: "618.1".into(),
            arch: "aarch64".into(),
            host_log_tail: vec![],
        };
        let json = serde_json::to_string(&info).unwrap();
        assert_eq!(
            json,
            r#"{"osVersion":"macos","webview2Version":"618.1","arch":"aarch64","hostLogTail":[]}"#
        );
        assert_eq!(serde_json::from_str::<SystemInfoDto>(&json).unwrap(), info);
    }
}
