//! Tauri commands — the M2 bridge slice. Only settings get/set + a diagnostics
//! ping route through Rust for now; every other bridge verb still hits the web
//! mock (see `frontend/src/bridge/tauri.ts`). Each command is `#[specta::specta]`
//! so its signature flows into the generated TS bindings.

use dm_contracts::{DiagnosticsPing, SettingsDto, SettingsPatch};
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
pub fn diagnostics_ping(message: String) -> DiagnosticsPing {
    DiagnosticsPing {
        ok: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
        message,
    }
}
