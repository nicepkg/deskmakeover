//! Tauri commands — the bridge slice Rust owns: settings get/set and the THIN
//! wallpaper verbs (M6-WIRE A6, owner ruling D1 — screen info + get/set wallpaper +
//! snapshot restore; looks/reconcile/state assembly stay in the web store). Each
//! command is `#[specta::specta]` so its signature flows into the generated TS bindings.

use dm_contracts::{
    IconChunkItemDto, IconOpResultDto, IconPersistedDto, IconScanDto, SettingsDto, SettingsPatch,
    WallpaperResultDto, WallpaperScreensDto,
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

// ---- Icons (M6-WIRE B4, D1-thin): scan / persisted state / chunked apply / restore ----

#[tauri::command]
#[specta::specta]
pub fn icons_scan(state: State<'_, AppState>) -> Result<IconScanDto, String> {
    state.icons.scan()
}

#[tauri::command]
#[specta::specta]
pub fn icons_get_persisted(state: State<'_, AppState>) -> Result<IconPersistedDto, String> {
    state.icons.get_persisted()
}

#[tauri::command]
#[specta::specta]
pub fn icons_apply_baked_begin(
    state: State<'_, AppState>,
    revision: u32,
    count: u32,
) -> Result<(), String> {
    state.icons.apply_baked_begin(revision, count)
}

#[tauri::command]
#[specta::specta]
pub fn icons_apply_baked_chunk(
    state: State<'_, AppState>,
    items: Vec<IconChunkItemDto>,
) -> Result<(), String> {
    state.icons.apply_baked_chunk(items)
}

#[tauri::command]
#[specta::specta]
pub fn icons_apply_baked_commit(
    state: State<'_, AppState>,
    style_json: String,
    label: Option<String>,
) -> Result<IconOpResultDto, String> {
    state.icons.apply_baked_commit(style_json, label)
}

#[tauri::command]
#[specta::specta]
pub fn icons_restore(state: State<'_, AppState>) -> Result<IconOpResultDto, String> {
    state.icons.restore()
}

#[tauri::command]
#[specta::specta]
pub fn icons_restore_overlay(state: State<'_, AppState>) -> Result<IconOpResultDto, String> {
    state.icons.restore_overlay()
}

#[tauri::command]
#[specta::specta]
pub fn icons_export_compare(state: State<'_, AppState>) -> Result<IconOpResultDto, String> {
    state.icons.export_compare()
}
