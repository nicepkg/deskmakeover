//! DeskMakeover desktop shell — a thin Tauri 2 composition root (ADR-0019).
//! It hosts the existing React app, owns settings persistence (rusqlite, via
//! `dm-operations`), and exposes the M2 command slice. No pixel or platform
//! logic lives here; those belong to the `dm-*` crates.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dm_operations::{RustImageDecoder, SettingsStore};
use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

mod commands;
#[cfg(not(windows))]
mod devhost;
mod wallpaper_host;

use wallpaper_host::WallpaperHost;

/// App-wide state managed by Tauri and read by commands.
pub struct AppState {
    pub settings: SettingsStore,
    pub wallpaper: WallpaperHost,
}

/// The single command surface — used both to wire `invoke` at runtime and to
/// generate the TypeScript bindings, so the two can never drift.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::settings_get,
        commands::settings_set,
        commands::wallpaper_get_screens,
        commands::wallpaper_get_source,
        commands::wallpaper_apply_baked,
        commands::wallpaper_restore,
        commands::diagnostics_ping,
    ])
}

/// Composition root for the wallpaper stack: real COM adapters on Windows, the
/// dev-host fakes elsewhere; the pure-Rust decoder is the SAME on both platforms.
fn build_wallpaper_host(data_dir: &Path) -> Result<WallpaperHost, String> {
    #[cfg(windows)]
    {
        let exec = Arc::new(dm_windows::StaExecutor::spawn().map_err(|e| e.to_string())?);
        Ok(WallpaperHost::new(
            Arc::new(dm_windows::WindowsMonitorTopology::new(exec.clone())),
            Arc::new(dm_windows::WindowsWallpaper::new(exec)),
            Arc::new(RustImageDecoder),
            data_dir,
        ))
    }
    #[cfg(not(windows))]
    {
        let desk = devhost::DevDesktop::new();
        Ok(WallpaperHost::new(
            Arc::new(devhost::DevMonitorTopology(desk.clone())),
            Arc::new(devhost::DevWallpaperApplier(desk)),
            Arc::new(RustImageDecoder),
            data_dir,
        ))
    }
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
        // Decoded wallpaper sources ride this protocol as image/png, so pixel
        // buffers never cross the JSON bridge (M6-WIRE A6). URLs are revisioned
        // (`?rev=N`) and rev changes with the path, so responses are immutable.
        // The compositor reads them with `fetch()`, which is cross-origin from the
        // webview (dev: http://localhost:5173; prod: the app origin) — so the
        // response MUST carry `Access-Control-Allow-Origin` or the browser blocks
        // the read with a CORS error (compositor init "Load failed"). The bytes are
        // non-secret decoded wallpaper served only to our own window; `*` is safe.
        .register_uri_scheme_protocol("dmwallpaper", |ctx, request| {
            let key = request.uri().path().trim_start_matches('/');
            let png = ctx
                .app_handle()
                .try_state::<AppState>()
                .and_then(|s| s.wallpaper.png_for(key));
            match png {
                Some(bytes) => tauri::http::Response::builder()
                    .header("Content-Type", "image/png")
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Cache-Control", "public, max-age=31536000, immutable")
                    .body(bytes)
                    .expect("static headers cannot fail"),
                None => tauri::http::Response::builder()
                    .status(404)
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Vec::new())
                    .expect("static 404 cannot fail"),
            }
        })
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_window_state::Builder::default().build())?;

            specta.mount_events(app);

            // Crash recovery MUST run before any mutation command is reachable (ADR-0019): a
            // transaction interrupted by a previous crash is driven to a consistent terminal state
            // first. This happens during setup, before the settings store is managed and before the
            // window is interactive; a recovery error aborts startup (fail closed).
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            run_startup_recovery(&data_dir)?;

            let db_path = data_dir.join(SETTINGS_DB_FILE);
            let store = SettingsStore::open(&db_path)?;
            let wallpaper = build_wallpaper_host(&data_dir)?;
            app.manage(AppState { settings: store, wallpaper });
            log::info!("settings store ready at {}", db_path.display());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running DeskMakeover");
}

/// The settings database file name under the OS app-data dir.
const SETTINGS_DB_FILE: &str = "settings.sqlite3";

/// The crash-recovery inputs under the app-data dir: the write-ahead journal and the active
/// ledger the transaction machinery persists.
fn recovery_paths(data_dir: &Path) -> (PathBuf, PathBuf) {
    (data_dir.join("txn.log"), data_dir.join("ledger.json"))
}

/// Drive crash recovery to a consistent terminal state before any mutation command is reachable
/// (ADR-0019). On Windows it wires the real platform adapters and replays the journal; on a dev
/// host there is no desktop to recover, so it is a logged no-op. Fail-closed: any error propagates
/// out of `setup` and aborts startup rather than exposing a half-recovered desktop.
///
/// [WINDOWS-VERIFY] the Windows branch cannot be msvc-cross-checked from the host (this crate pulls
/// tauri + rusqlite C deps), so the adapter wiring is verified on the owner's Windows box.
fn run_startup_recovery(data_dir: &Path) -> Result<(), String> {
    let (journal_path, ledger_path) = recovery_paths(data_dir);
    #[cfg(windows)]
    {
        use std::sync::Arc;

        use dm_operations::{recover_from_journal, FileJournal, JsonLedgerStore};
        use dm_windows::{StaExecutor, WindowsIconApplier, WindowsStateReader};

        // A one-shot STA executor for the recovery pass; the resident apply/scan stack owns its own.
        // The reader and applier share it so ALL shell COM (.lnk reads AND writes) runs on the one
        // STA apartment thread.
        let exec = Arc::new(StaExecutor::spawn().map_err(|e| e.to_string())?);
        let applier = WindowsIconApplier::new(exec.clone());
        let reader = WindowsStateReader::new(exec);
        let mut journal = FileJournal::new(&journal_path);
        let mut ledger = JsonLedgerStore::new(&ledger_path);
        let outcome =
            recover_from_journal(&mut journal, &reader, &applier, &mut ledger).map_err(|e| e.to_string())?;
        log::info!(
            "startup recovery: {} aborted, {} reconciled, {} clean txns",
            outcome.aborted.len(),
            outcome.reconciled.len(),
            outcome.clean_txns
        );
    }
    #[cfg(not(windows))]
    {
        let _ = (&journal_path, &ledger_path);
        log::info!("startup recovery skipped: no desktop platform adapters on this host");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_path_points_at_the_committed_ts_file() {
        let p = bindings_path();
        assert!(p.ends_with("generated.ts"), "got {}", p.display());
        assert!(p.to_string_lossy().contains("bridge"));
    }

    #[test]
    fn recovery_reads_the_journal_and_ledger_from_the_app_data_dir() {
        let (journal, ledger) = recovery_paths(Path::new("/data/DeskMakeover"));
        assert!(journal.ends_with("txn.log"));
        assert!(ledger.ends_with("ledger.json"));
        // Both must sit under the passed app-data dir, never a caller-relative path.
        assert!(journal.starts_with("/data/DeskMakeover"));
        assert!(ledger.starts_with("/data/DeskMakeover"));
    }

    #[test]
    fn startup_recovery_is_a_clean_noop_on_the_dev_host() {
        // On non-Windows there is no desktop to recover; the seam must succeed so startup proceeds.
        let dir = std::env::temp_dir();
        assert!(run_startup_recovery(&dir).is_ok());
    }
}
