//! Tauri commands — the bridge slice Rust owns: settings get/set and the THIN
//! wallpaper verbs (M6-WIRE A6, owner ruling D1 — screen info + get/set wallpaper +
//! snapshot restore; looks/reconcile/state assembly stay in the web store). Each
//! command is `#[specta::specta]` so its signature flows into the generated TS bindings.

use dm_contracts::{
    CalmApplyRowDto, CalmGuidedProbeDto, CalmProbeRowDto, CalmRestoreRowDto, IconChunkItemDto,
    IconOpResultDto, IconPersistedDto, IconScanDto, PresetEntryDto, PresetPackageReadDto,
    PresetSaveDto, SettingsDto, SettingsPatch, SystemInfoDto, WallpaperResultDto,
    WallpaperScreensDto,
};
use tauri::State;

use crate::AppState;

#[tauri::command]
#[specta::specta]
pub fn diagnostics_get_info() -> Result<SystemInfoDto, String> {
    // Audit #7: return real host facts, replacing the browser `(mock)` stub that fell through even
    // on the real Tauri app (so a Windows diagnostics report showed `osVersion: "Win32 (mock)"`).
    // `webview_version()` is Tauri's cross-platform query — the Edge WebView2 runtime version on
    // Windows ([WINDOWS-VERIFY]), the WebKit version on macOS. `arch` is real; `os_version` is the
    // coarse target-OS name (the detailed Windows build number is a [WINDOWS-VERIFY] enrichment that
    // would need a Win32/os_info source not pulled in for this marginal path). `host_log_tail` is
    // empty until F8's host error-log buffer exists (the DTO documents the same).
    Ok(SystemInfoDto {
        os_version: std::env::consts::OS.to_string(),
        webview2_version: tauri::webview_version().unwrap_or_else(|_| "unavailable".to_string()),
        arch: std::env::consts::ARCH.to_string(),
        host_log_tail: Vec::new(),
    })
}

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
) -> Result<String, String> {
    state.icons.apply_baked_begin(revision, count)
}

#[tauri::command]
#[specta::specta]
pub fn icons_apply_baked_chunk(
    state: State<'_, AppState>,
    session_id: String,
    items: Vec<IconChunkItemDto>,
) -> Result<(), String> {
    state.icons.apply_baked_chunk(&session_id, items)
}

#[tauri::command]
#[specta::specta]
pub fn icons_apply_baked_commit(
    state: State<'_, AppState>,
    session_id: String,
    style_json: String,
    restore_ids: Vec<String>,
    label: Option<String>,
) -> Result<IconOpResultDto, String> {
    state.icons.apply_baked_commit(&session_id, style_json, restore_ids, label)
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
pub fn icons_switch_version(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<IconOpResultDto, String> {
    state.icons.switch_version(&version_id)
}

#[tauri::command]
#[specta::specta]
pub fn icons_export_compare(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    png_base64: String,
) -> Result<IconOpResultDto, String> {
    // The webview composed the sheet (it owns the fonts + both image states); Rust saves it to
    // the user's Pictures folder, falling back to the host's own export dir when the platform
    // has no Pictures known-folder.
    use tauri::Manager;
    let pictures = app.path().picture_dir().ok();
    state.icons.export_compare(&png_base64, pictures)
}

// ---- Preset packages + the user preset library (spec 09, bridge schema 9). Rust owns
// structure/security (bounded unzip, caps, atomic writes); payload semantics stay with the ONE
// TS validator — so read is PURE (nothing written) and save is the only library writer.

#[tauri::command]
#[specta::specta]
pub fn presets_read_package(
    state: State<'_, AppState>,
    path: String,
) -> Result<PresetPackageReadDto, String> {
    Ok(state.presets.read_package(&path))
}

#[tauri::command]
#[specta::specta]
pub fn presets_list(state: State<'_, AppState>) -> Result<Vec<PresetEntryDto>, String> {
    Ok(state.presets.list())
}

#[tauri::command]
#[specta::specta]
pub fn presets_save(
    state: State<'_, AppState>,
    entry: PresetSaveDto,
    overwrite: bool,
) -> Result<PresetEntryDto, String> {
    state.presets.save(entry, overwrite)
}

#[tauri::command]
#[specta::specta]
pub fn presets_delete(state: State<'_, AppState>, entry_id: String) -> Result<(), String> {
    state.presets.delete(&entry_id)
}

#[tauri::command]
#[specta::specta]
pub fn presets_rename(
    state: State<'_, AppState>,
    entry_id: String,
    name: String,
) -> Result<PresetEntryDto, String> {
    state.presets.rename(&entry_id, &name)
}

#[tauri::command]
#[specta::specta]
pub fn presets_export(
    state: State<'_, AppState>,
    dest_path: String,
    entries: Vec<PresetSaveDto>,
) -> Result<String, String> {
    use crate::icon_host::export::{iso_stamp, now_secs};
    state.presets.export(&dest_path, entries, iso_stamp(now_secs()))
}

// ---- 清爽 (calm-Windows) settings — the THIN calm verbs (bridge schema 8). The frontend owns
// grouping / hero phase / schematic; Rust reports each row's honest probe + apply/restore outcome.

#[tauri::command]
#[specta::specta]
pub fn tweaks_probe(state: State<'_, AppState>) -> Result<Vec<CalmProbeRowDto>, String> {
    state.tweaks.probe()
}

#[tauri::command]
#[specta::specta]
pub fn tweaks_apply(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<Vec<CalmApplyRowDto>, String> {
    state.tweaks.apply(ids)
}

#[tauri::command]
#[specta::specta]
pub fn tweaks_restore(state: State<'_, AppState>) -> Result<Vec<CalmRestoreRowDto>, String> {
    state.tweaks.restore()
}

#[tauri::command]
#[specta::specta]
pub fn tweaks_restore_one(
    state: State<'_, AppState>,
    id: String,
) -> Result<CalmRestoreRowDto, String> {
    state.tweaks.restore_one(id)
}

#[tauri::command]
#[specta::specta]
pub fn tweaks_open_route(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.tweaks.open_route(id)
}

#[tauri::command]
#[specta::specta]
pub fn tweaks_re_probe_guided(
    state: State<'_, AppState>,
    id: String,
) -> Result<CalmGuidedProbeDto, String> {
    state.tweaks.re_probe_guided(id)
}
