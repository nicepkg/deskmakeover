//! Tauri commands — the bridge slice Rust owns: settings get/set and the THIN
//! wallpaper verbs (M6-WIRE A6, owner ruling D1 — screen info + get/set wallpaper +
//! snapshot restore; looks/reconcile/state assembly stay in the web store). Each
//! command is `#[specta::specta]` so its signature flows into the generated TS bindings.

use dm_contracts::{SettingsDto, SettingsPatch, WallpaperResultDto, WallpaperScreensDto};
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
