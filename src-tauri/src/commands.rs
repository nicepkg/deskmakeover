//! Tauri commands — the bridge slice Rust owns: settings get/set, a diagnostics
//! ping, and the THIN wallpaper verbs (M6-WIRE A6, owner ruling D1 — screen info +
//! get/set wallpaper + snapshot restore; looks/reconcile/state assembly stay in the
//! web store). Each command is `#[specta::specta]` so its signature flows into the
//! generated TS bindings.

use dm_contracts::{
    DiagnosticsPing, SettingsDto, SettingsPatch, WallpaperResultDto, WallpaperScreensDto,
    WallpaperSourceDto,
};
use tauri::State;

use crate::AppState;

#[tauri::command]
#[specta::specta]
pub fn settings_get(state: State<'_, AppState>) -> Result<SettingsDto, String> {
    state.settings.get().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn settings_set(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<SettingsDto, String> {
    state.settings.set(&patch).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn wallpaper_get_screens(state: State<'_, AppState>) -> Result<WallpaperScreensDto, String> {
    state.wallpaper.screens()
}

#[tauri::command]
#[specta::specta]
pub fn wallpaper_get_source(state: State<'_, AppState>) -> Result<WallpaperSourceDto, String> {
    state.wallpaper.primary_source()
}

#[tauri::command]
#[specta::specta]
pub fn wallpaper_apply_baked(
    state: State<'_, AppState>,
    monitor_id: String,
    png_base64: String,
) -> Result<WallpaperResultDto, String> {
    state.wallpaper.apply_baked(&monitor_id, &png_base64)
}

#[tauri::command]
#[specta::specta]
pub fn wallpaper_restore(
    state: State<'_, AppState>,
    monitor_id: String,
) -> Result<WallpaperResultDto, String> {
    state.wallpaper.restore(&monitor_id)
}

#[tauri::command]
#[specta::specta]
pub fn diagnostics_ping(message: String) -> DiagnosticsPing {
    DiagnosticsPing {
        ok: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // settings_get/set require Tauri-managed `State`, so their happy/error paths are covered by
    // the `SettingsStore` tests in `dm-operations`; only the state-free `diagnostics_ping` is a
    // pure unit here.
    #[test]
    fn diagnostics_ping_echoes_the_message_and_reports_healthy() {
        let ping = diagnostics_ping("hello".to_string());
        assert!(ping.ok);
        assert_eq!(ping.message, "hello");
        assert_eq!(ping.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn diagnostics_ping_preserves_empty_and_unicode_messages() {
        assert_eq!(diagnostics_ping(String::new()).message, "");
        assert_eq!(diagnostics_ping("日本語 🌸".to_string()).message, "日本語 🌸");
    }
}
