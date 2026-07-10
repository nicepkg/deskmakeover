//! DeskMakeover desktop shell — a thin Tauri 2 composition root (ADR-0019).
//! It hosts the existing React app, owns settings persistence (rusqlite, via
//! `dm-operations`), and exposes the M2 command slice. No pixel or platform
//! logic lives here; those belong to the `dm-*` crates.

use std::path::{Path, PathBuf};

use dm_operations::SettingsStore;
use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

mod commands;

/// App-wide state managed by Tauri and read by commands.
pub struct AppState {
    pub settings: SettingsStore,
}

/// The single command surface — used both to wire `invoke` at runtime and to
/// generate the TypeScript bindings, so the two can never drift.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::settings_get,
        commands::settings_set,
        commands::diagnostics_ping,
    ])
}

/// Absolute path to the committed TS bindings, resolved from the crate dir so
/// codegen works regardless of the caller's cwd.
pub fn bindings_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bridge/generated.ts")
}

/// Regenerate the TS bindings at `path` from the live command surface.
pub fn export_bindings(path: &Path) -> Result<(), String> {
    specta_builder()
        .export(Typescript::default(), path)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta = specta_builder();

    let mut builder = tauri::Builder::default();

    // Single-instance MUST be registered first (ADR-0019): a second launch
    // focuses the existing window instead of spawning a duplicate.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                let _ = window.unminimize();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_log::Builder::new().build())
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_window_state::Builder::default().build())?;

            specta.mount_events(app);

            let db_path = settings_db_path(app)?;
            let store = SettingsStore::open(&db_path)?;
            app.manage(AppState { settings: store });
            log::info!("settings store ready at {}", db_path.display());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running DeskMakeover");
}

/// Resolve (and create) the settings database path under the OS app-data dir.
fn settings_db_path<R: tauri::Runtime>(
    app: &tauri::App<R>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("settings.sqlite3"))
}
