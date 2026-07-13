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
#[cfg(not(windows))]
mod devhost_icons;
mod icon_host;
mod wallpaper_host;

use dm_operations::icons::scope::ScopeRoots;
use icon_host::{IconHost, IconHostPorts};
use wallpaper_host::WallpaperHost;

/// App-wide state managed by Tauri and read by commands.
pub struct AppState {
    /// Shared with the icon host (both persist against the one settings DB — store ② lives here).
    pub settings: Arc<SettingsStore>,
    pub wallpaper: WallpaperHost,
    pub icons: IconHost,
}

/// The single command surface — used both to wire `invoke` at runtime and to
/// generate the TypeScript bindings, so the two can never drift.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::settings_get,
        commands::settings_set,
        commands::diagnostics_get_info,
        commands::wallpaper_get_screens,
        commands::wallpaper_apply_baked,
        commands::wallpaper_restore,
        commands::icons_scan,
        commands::icons_get_persisted,
        commands::icons_apply_baked_begin,
        commands::icons_apply_baked_chunk,
        commands::icons_apply_baked_commit,
        commands::icons_restore,
        commands::icons_restore_overlay,
        commands::icons_switch_version,
        commands::icons_export_compare,
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

/// Composition root for the icon stack: real dm-windows shell adapters on Windows, the dev-host
/// fakes elsewhere; the FsAssetStore + ②③ stores live inside the host (data_dir). The settings
/// store is SHARED with `AppState` so store ② has one writer.
fn build_icon_host(data_dir: &Path, settings: Arc<SettingsStore>) -> Result<IconHost, String> {
    #[cfg(windows)]
    {
        // [WINDOWS-VERIFY] the whole icon composition is blind-wired: the shell adapters exist +
        // msvc-check, but source extraction is deferred (WindowsIconSourceExtractor returns [WV]),
        // the overlay helper path is a placeholder, and none of it is runtime-verified on Mac.
        // Swapping to live = fill the extractor body + resolve the real helper path on the box.
        let exec = Arc::new(dm_windows::StaExecutor::spawn().map_err(|e| e.to_string())?);
        let helper = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("dm-elevated.exe")))
            .unwrap_or_else(|| data_dir.join("dm-elevated.exe"));
        let ports = IconHostPorts {
            scanner: Arc::new(dm_windows::WindowsScanner::new(exec.clone())),
            extractor: Arc::new(dm_windows::WindowsIconSourceExtractor::new(exec.clone())),
            reader: Arc::new(dm_windows::WindowsStateReader::new(exec.clone())),
            applier: Arc::new(dm_windows::WindowsIconApplier::new(exec.clone())),
            overlay: Arc::new(dm_windows::WindowsOverlayControl::new(helper)),
            refresher: Arc::new(dm_windows::WindowsExplorerRefresher),
            geometry: Arc::new(dm_windows::WindowsDesktopGeometry::new(exec)),
        };
        // Real active-profile count is a [WINDOWS-VERIFY] ProfileList enum; default single-user.
        // [WINDOWS-VERIFY] the §14 scope is UNRESOLVED until SHGetKnownFolderPath resolves the real
        // Public Desktop / ProgramData folders — until then version-switch / auto-format fail CLOSED
        // (style nothing), never open. Swapping to live = `ScopeRoots::resolved(public, programdata)?`.
        Ok(IconHost::new(ports, settings, data_dir, 1, ScopeRoots::Unresolved))
    }
    #[cfg(not(windows))]
    {
        let desk = devhost_icons::DevIconDesktop::new();
        let ports = IconHostPorts {
            scanner: Arc::new(devhost_icons::DevDesktopScanner),
            extractor: Arc::new(devhost_icons::DevIconSourceExtractor(desk.clone())),
            reader: Arc::new(devhost_icons::DevIconReader(desk.clone())),
            applier: Arc::new(devhost_icons::DevIconApplier(desk)),
            overlay: Arc::new(devhost_icons::DevOverlayControl),
            refresher: Arc::new(devhost_icons::DevExplorerRefresher),
            geometry: Arc::new(devhost_icons::DevDesktopGeometry),
        };
        // The dev host has no shared/privileged desktop scope — nothing is scope-excluded.
        Ok(IconHost::new(ports, settings, data_dir, 1, ScopeRoots::Unprivileged))
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
        // Opens About/support links in the default browser + the data folder in the
        // file manager (shell.openExternal / shell.openDataFolder).
        builder = builder.plugin(tauri_plugin_opener::init());
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
        // Extracted 256px icon sources ride this protocol as image/png (mirrors dmwallpaper://):
        // the key is "<itemId>/<slot>", the URL revisioned (`?rev=N`) so each scan cache-busts.
        .register_uri_scheme_protocol("dmicon", |ctx, request| {
            let key = request.uri().path().trim_start_matches('/');
            let png = ctx
                .app_handle()
                .try_state::<AppState>()
                .and_then(|s| s.icons.png_for(key));
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
            let settings = Arc::new(SettingsStore::open(&db_path)?);
            let wallpaper = build_wallpaper_host(&data_dir)?;
            let icons = build_icon_host(&data_dir, settings.clone())?;
            app.manage(AppState { settings, wallpaper, icons });
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
        // A DEGRADED recovery (codex R5-#6): a restore/ledger op faulted while replaying a prior crash's
        // journal, so the desktop is only PARTIALLY recovered. `recover_from_journal` deliberately did
        // NOT checkpoint — the unreconciled records stay so the next (idempotent) recovery finishes the
        // job (the user's first apply/reset re-runs it, and `get_persisted` reports `applied: true` off
        // the retained in-flight journal, keeping the restore affordance reachable). We do NOT abort
        // startup (that would strand the user with no way to reach the very reset that heals it) — but
        // we must NOT swallow it either: log loudly so the fault is diagnosable.
        if !outcome.degraded.is_empty() {
            log::error!(
                "startup recovery DEGRADED — a prior crash could not be fully recovered ({} fault(s)); \
                 the journal was retained for the next pass and the restore affordance stays reachable: {}",
                outcome.degraded.len(),
                outcome.degraded.join("; ")
            );
        }
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
