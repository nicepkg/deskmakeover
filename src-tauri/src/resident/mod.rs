//! T8 — the resident tray + windowless-residency wiring (spec 07 §1/§12/§14, plan T8).
//!
//! This is the composition root for the background auto-format engine: it registers the tray icon +
//! menu (spec 07 §12), keeps the process resident when the window closes (spec 07 §1), and spawns
//! the reconcile-loop driver ([`dm_resident::ResidentDriver`]) behind the SAME cfg-selected-adapter
//! pattern the icon/wallpaper/tweaks hosts use — the real `dm-windows` watcher + COM engine on
//! Windows (`[WINDOWS-VERIFY]`), a devhost fake engine over the icon dev fakes elsewhere so the app
//! still boots + is Mac-E2E-testable (a synthetic watcher event reaches a proposal).
//!
//! The driver runs on ONE background thread; the tray menu handlers (main thread) talk to it through
//! [`ResidentShared`] (atomics + a command queue). The loop is single-threaded, so the confirm /
//! 2h-timeout → `apply_batch` path (spec 07 §2 item 4) interleaves cleanly after each `tick`.
//!
//! §8.1 terminology law: every user-facing string is 外观/外观方案/应用/恢复系统原始外观 — NEVER
//! 版本/快照/回退. The destructive reset ("恢复系统原始外观") is NOT a direct tray action (spec §12/§13
//! level 4): the tray item routes to Settings › Advanced behind its confirmation, never reverts inline.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use dm_operations::SettingsStore;
use dm_resident::{
    DriverConfig, MonotonicClock, Proposal, ResidentDriver, ResidentHost, TrayState, UndoTarget,
    VettedCandidate, WatchEvent, PROPOSAL_TIMEOUT_SECS,
};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

mod engine;
#[cfg(all(test, not(windows)))]
mod tests;

/// The tray icon's stable id (spec 07 §12).
const TRAY_ID: &str = "dm-resident";

/// Main-thread → loop-thread commands (the tray menu drives these).
enum ResidentCmd {
    /// Toggle automation on/off (spec 07 §2/§12.1). The §2 precondition is enforced on the settings
    /// write BEFORE this is queued, so `true` here always has a valid ② saved-style.
    SetEnabled(bool),
    /// The user acknowledged the ERROR state → retry (spec 07 §12).
    AcknowledgeError,
    /// 撤销最近一次整理 (spec 07 §13 level 2): restore the last applied batch off its ledger
    /// anchors — CAS-gated, never clobbers a hand-edit.
    UndoLast,
}

/// Shared state between the main thread (tray handlers, window-close) and the driver loop thread.
pub struct ResidentShared {
    /// Whether automation is enabled — read by the window-close handler (spec 07 §1: stay resident).
    enabled: AtomicBool,
    /// The pending-privileged depth (spec 07 §14) — mirrors the driver so the main thread can read it.
    pending_privileged: AtomicUsize,
    /// A tray "立即整理桌面" click: apply the pending batch now (skip the 2h wait) + force a rescan.
    apply_now: AtomicBool,
    /// Force the next tick to reconcile even without watcher hints (the "立即整理桌面" rescan).
    force_reconcile: AtomicBool,
    /// The loop should stop (app exit).
    shutdown: AtomicBool,
    cmds: Mutex<VecDeque<ResidentCmd>>,
    /// Synthetic watcher hints — fed by the Windows `watch_desktops` callback in prod, or by
    /// [`ResidentHandle::inject_event`] for the Mac E2E.
    events: Mutex<VecDeque<WatchEvent>>,
}

impl ResidentShared {
    fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            pending_privileged: AtomicUsize::new(0),
            apply_now: AtomicBool::new(false),
            force_reconcile: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            cmds: Mutex::new(VecDeque::new()),
            events: Mutex::new(VecDeque::new()),
        }
    }

    fn push_cmd(&self, cmd: ResidentCmd) {
        self.cmds.lock().unwrap().push_back(cmd);
    }

    fn take_cmds(&self) -> Vec<ResidentCmd> {
        self.cmds.lock().unwrap().drain(..).collect()
    }

    fn take_events(&self) -> Vec<WatchEvent> {
        self.events.lock().unwrap().drain(..).collect()
    }

    fn push_event(&self, event: WatchEvent) {
        self.events.lock().unwrap().push_back(event);
    }
}

/// The handle stored in `AppState` + captured by the window-close / exit guards. Cheap to clone
/// (an `Arc`).
#[derive(Clone)]
pub struct ResidentHandle(Arc<ResidentShared>);

impl ResidentHandle {
    /// Whether automation is enabled — the window-close / exit guards keep the process resident only
    /// when this is `true` (spec 07 §1).
    pub fn is_enabled(&self) -> bool {
        self.0.enabled.load(Ordering::Relaxed)
    }

    /// Feeds a synthetic watcher hint into the loop (the Mac E2E seam — prod uses the real watcher).
    pub fn inject_event(&self, event: WatchEvent) {
        self.0.push_event(event);
    }

    /// Requests loop shutdown (app exit).
    pub fn shutdown(&self) {
        self.0.shutdown.store(true, Ordering::Relaxed);
    }
}

// ---- tray rendering (loop thread) ------------------------------------------------------------

/// The tray glyph tooltip for each state (spec 07 §12). The double-coded 16px BITMAP pairs are T11
/// (`[WINDOWS-VERIFY]`); until then the default window icon carries the glyph and the tooltip carries
/// the state.
fn tooltip_for(state: &TrayState) -> &'static str {
    match state {
        TrayState::Off => "自动整理已关闭",
        TrayState::Watching => "正在为你保持桌面风格",
        TrayState::Paused => "桌面使用中，已暂停",
        TrayState::Working { .. } => "正在整理新图标",
        TrayState::Error { .. } => "遇到问题，点击查看",
    }
}

/// The disabled status-line text (spec 07 §12, first menu row) — the state in words.
fn status_text_for(state: &TrayState) -> String {
    match state {
        TrayState::Off => "自动整理：已关闭".to_string(),
        TrayState::Watching => "自动整理：守护中".to_string(),
        TrayState::Paused => "自动整理：桌面使用中，已暂停".to_string(),
        TrayState::Working { count } => format!("自动整理：正在整理 {count} 个新图标"),
        TrayState::Error { .. } => "自动整理：遇到问题".to_string(),
    }
}

/// A proposal awaiting confirm / 2h-timeout (spec 07 §2 item 4). `deadline` is armed by the loop on
/// the driver's clock the tick AFTER it is surfaced (so the two clocks never diverge).
struct Pending {
    candidates: Vec<VettedCandidate>,
    deadline: Option<u64>,
}

/// The [`ResidentHost`] the driver renders to: updates the real tray + holds the pending proposal.
struct TrayHost {
    app: AppHandle<Wry>,
    status: MenuItem<Wry>,
    toggle: CheckMenuItem<Wry>,
    pending_item: MenuItem<Wry>,
    /// 撤销最近一次整理 — enabled only while an undoable batch exists (spec 07 §12.1 menu law).
    undo: MenuItem<Wry>,
    pending: Option<Pending>,
}

impl TrayHost {
    fn new(
        app: AppHandle<Wry>,
        status: MenuItem<Wry>,
        toggle: CheckMenuItem<Wry>,
        pending_item: MenuItem<Wry>,
        undo: MenuItem<Wry>,
    ) -> Self {
        Self { app, status, toggle, pending_item, undo, pending: None }
    }

    /// Arms the surfaced proposal's 2h-timeout deadline on the driver's clock (spec 07 §2 item 4).
    fn arm_pending(&mut self, now_ms: u64) {
        if let Some(p) = self.pending.as_mut() {
            if p.deadline.is_none() {
                p.deadline = Some(now_ms.saturating_add(PROPOSAL_TIMEOUT_SECS as u64 * 1000));
            }
        }
    }

    /// Returns the batch to apply if the user confirmed (`forced`) or the 2h-timeout elapsed.
    fn take_due_batch(&mut self, now_ms: u64, forced: bool) -> Option<Vec<VettedCandidate>> {
        let due = match &self.pending {
            Some(p) => forced || p.deadline.map(|d| now_ms >= d).unwrap_or(false),
            None => false,
        };
        due.then(|| self.pending.take().map(|p| p.candidates)).flatten()
    }
}

impl ResidentHost for TrayHost {
    fn render_tray(&mut self, state: &TrayState, pending_privileged: usize) {
        let _ = self.status.set_text(status_text_for(state));
        let _ = self.pending_item.set_text(format!("待处理特权项({pending_privileged})"));
        // The toggle check reflects enablement; OFF is the only disabled state (a Failure while
        // enabled stays checked so the user can still uncheck it — spec 07 §12.1).
        let _ = self.toggle.set_checked(!matches!(state, TrayState::Off));
        if let Some(tray) = self.app.tray_by_id(TRAY_ID) {
            let _ = tray.set_tooltip(Some(tooltip_for(state)));
        }
    }

    fn surface_proposal(&mut self, proposal: Proposal) {
        // v1 batched proposal (spec 07 §2 item 4): the OS-native toast (tauri-plugin-notification /
        // WinRT) is a documented follow-up ([WV]); for now emit an app event the window can render +
        // arm the in-memory 2h-timeout. The trust-tier toast decision rides `anomaly` (spec §2 item 7).
        log::info!(
            "resident: proposing {} new icon(s) (anomaly={})",
            proposal.candidates.len(),
            proposal.anomaly
        );
        let _ = self.app.emit("resident://proposal", proposal.candidates.len());
        self.pending = Some(Pending { candidates: proposal.candidates, deadline: None });
    }
}

// ---- the loop --------------------------------------------------------------------------------

/// The single-threaded reconcile loop (spec 07 §3). Owns the driver + engine + tray host; drains
/// main-thread commands + watcher hints each pass, ticks, then interleaves the confirm/2h-timeout →
/// `apply_batch` path (spec 07 §2 item 4).
fn run_loop<E: engine::ResidentEngine>(
    mut driver: ResidentDriver<MonotonicClock>,
    mut engine: E,
    mut host: TrayHost,
    shared: Arc<ResidentShared>,
    settings: Arc<SettingsStore>,
    initial_enabled: bool,
) {
    // Reflect the persisted toggle on boot (spec 07 §2 — enablement survives restarts).
    if initial_enabled {
        driver.set_enabled(true, &mut host);
    } else {
        host.render_tray(&TrayState::Off, 0);
    }
    // The last successfully applied batch WITH its snapshot fingerprints — the tray
    // 「撤销最近一次整理」 target (spec 07 §13 level 2). Cleared only when an undo pass finishes
    // with no per-item errors; a busy/recovery-deferred or degraded undo keeps it for a retry.
    let mut last_batch: Vec<UndoTarget> = Vec::new();
    while !shared.shutdown.load(Ordering::Relaxed) {
        // Converge on the PERSISTED toggle every pass (one indexed sqlite read per 500ms
        // heartbeat): the tray writes it, but so do the Settings page toggle and the §10 reset
        // coupling (`reset_style_and_autoformat`) — without this the loop would keep a stale
        // enablement until restart after any non-tray writer flipped it. The engine already
        // re-reads ② every cycle for the same never-drift reason.
        if let Ok(s) = settings.get() {
            if s.keep_new_icons_styled != shared.enabled.load(Ordering::Relaxed) {
                shared.enabled.store(s.keep_new_icons_styled, Ordering::Relaxed);
                driver.set_enabled(s.keep_new_icons_styled, &mut host);
            }
        }
        for cmd in shared.take_cmds() {
            match cmd {
                ResidentCmd::SetEnabled(on) => {
                    shared.enabled.store(on, Ordering::Relaxed);
                    driver.set_enabled(on, &mut host);
                }
                ResidentCmd::AcknowledgeError => driver.on_error_acknowledged(&mut host),
                ResidentCmd::UndoLast => {
                    if last_batch.is_empty() {
                        continue; // stale click — the menu item disables right after an undo
                    }
                    match engine.restore_batch(&last_batch) {
                        Ok(out) if out.deferred_busy || out.deferred_recovery => {
                            // Nothing restored — keep the batch so the user can retry once idle.
                            log::info!("resident: undo deferred (busy/recovery) — batch kept");
                        }
                        Ok(out) => {
                            log::info!(
                                "resident: undo restored {} item(s) ({} conflict(s) kept, {} skipped)",
                                out.restored.len(),
                                out.conflicts.len(),
                                out.skipped.len()
                            );
                            if !out.restored.is_empty() {
                                let _ = host.app.emit("resident://undone", out.restored.len());
                            }
                            if out.errors.is_empty() {
                                // Fully settled: every item restored / superseded / already
                                // original — nothing undoable remains.
                                last_batch.clear();
                                let _ = host.undo.set_enabled(false);
                            } else {
                                // A per-item read/restore fault — KEEP the batch armed so the
                                // user can retry (already-restored items re-skip idempotently;
                                // clearing here would strand the failed items, codex P2).
                                for e in &out.errors {
                                    log::warn!("resident: undo degraded — {e}");
                                }
                            }
                        }
                        Err(reason) => driver.on_failure(format!("undo: {reason}"), &mut host),
                    }
                }
            }
        }
        if shared.force_reconcile.swap(false, Ordering::Relaxed) {
            driver.request_reconcile();
        }
        let events = shared.take_events();
        driver.note_events(&events);
        driver.tick(&mut engine, &mut host);
        shared.pending_privileged.store(driver.pending_privileged(), Ordering::Relaxed);

        // Confirm / 2h-timeout → apply_batch (spec 07 §2 item 4). Arm a freshly-surfaced proposal's
        // deadline on the driver's clock, then apply if due (a "立即整理桌面" click sets `apply_now`).
        let now = driver.now_ms();
        host.arm_pending(now);
        let forced = shared.apply_now.swap(false, Ordering::Relaxed);
        if let Some(batch) = host.take_due_batch(now, forced) {
            let n = batch.len() as u32;
            driver.on_batch_start(n, &mut host);
            match engine.apply_batch(batch) {
                Ok(out) => {
                    shared.pending_privileged.store(out.pending_privileged, Ordering::Relaxed);
                    if !out.applied_snapshot.is_empty() {
                        // Arm 撤销最近一次整理 with THIS batch's id+fingerprint snapshots
                        // (level 2 — narrow, one batch; the snapshot is the undo CAS anchor).
                        last_batch = out.applied_snapshot.clone();
                        let _ = host.undo.set_enabled(true);
                        let _ = host.app.emit("resident://applied", out.applied_snapshot.len());
                    }
                    driver.on_batch_done(&mut host);
                }
                Err(reason) => driver.on_failure(reason, &mut host),
            }
        }
        std::thread::sleep(DriverConfig::default().heartbeat);
    }
}

// ---- setup + spawn ---------------------------------------------------------------------------

/// Registers the tray icon + menu and spawns the reconcile-loop driver. Called ONCE from
/// `lib.rs::run`'s setup. Returns the [`ResidentHandle`] for `AppState` + the residency guards.
pub fn setup(
    app: &AppHandle<Wry>,
    settings: Arc<SettingsStore>,
    data_dir: PathBuf,
) -> tauri::Result<ResidentHandle> {
    // The toggle persists (spec 07 §2); a fresh/empty settings row reads OFF.
    let enabled0 = settings.get().map(|s| s.keep_new_icons_styled).unwrap_or(false);
    let shared = Arc::new(ResidentShared::new(enabled0));
    let initial = if enabled0 { TrayState::Watching } else { TrayState::Off };

    // Menu — spec 07 §12 fixed order, §8.1 terminology (NEVER 版本/快照/回退).
    let status = MenuItem::with_id(app, "dm_status", status_text_for(&initial), false, None::<&str>)?;
    let toggle = CheckMenuItem::with_id(app, "dm_toggle", "自动整理新图标", true, enabled0, None::<&str>)?;
    let apply_now = MenuItem::with_id(app, "dm_apply_now", "立即整理桌面", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "dm_history", "查看最近整理记录", true, None::<&str>)?;
    let pending_item = MenuItem::with_id(app, "dm_pending", "待处理特权项(0)", false, None::<&str>)?;
    let undo = MenuItem::with_id(app, "dm_undo", "撤销最近一次整理", false, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let open = MenuItem::with_id(app, "dm_open", "打开 DeskMakeover", true, None::<&str>)?;
    // "恢复系统原始外观" ROUTES to Settings › Advanced (spec §13 level 4) — never reverts from the
    // tray. The ellipsis signals the confirmation surface; it is NOT the forbidden inline reset (§12).
    let reset = MenuItem::with_id(app, "dm_reset", "恢复系统原始外观…", true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "dm_settings", "设置", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "dm_quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&status, &toggle, &apply_now, &history, &pending_item, &undo, &sep, &open, &reset, &settings_i, &quit],
    )?;

    let shared_evt = shared.clone();
    let settings_evt = settings.clone();
    let toggle_evt = toggle.clone();
    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(tooltip_for(&initial))
        .icon(app.default_window_icon().expect("bundled window icon").clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            on_menu_event(app, event.id.as_ref(), &shared_evt, &settings_evt, &toggle_evt)
        })
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    spawn_loop(app.clone(), shared.clone(), settings, data_dir, (status, toggle, pending_item, undo), enabled0);
    Ok(ResidentHandle(shared))
}

/// Handles a tray menu click (main thread). Enable/disable go through the settings-patch layer FIRST
/// (spec 07 §2 precondition — a `true` while ② is empty is rejected there), then queue the driver.
/// History / Settings / Reset show the window AND emit a `resident://navigate` deep-link the web
/// shell routes ("恢复系统原始外观" lands on Settings › Advanced there, never reverts inline — §13).
fn on_menu_event(
    app: &AppHandle<Wry>,
    id: &str,
    shared: &Arc<ResidentShared>,
    settings: &Arc<SettingsStore>,
    toggle: &CheckMenuItem<Wry>,
) {
    match id {
        "dm_quit" => {
            shared.shutdown.store(true, Ordering::Relaxed);
            app.exit(0);
        }
        "dm_toggle" => {
            let desired = !shared.enabled.load(Ordering::Relaxed);
            // Persist through the precondition-enforcing patch; only flip the driver if it took.
            let patch = dm_contracts::SettingsPatch { keep_new_icons_styled: Some(desired), ..Default::default() };
            match settings.set(&patch) {
                Ok(_) => {
                    shared.enabled.store(desired, Ordering::Relaxed);
                    shared.push_cmd(ResidentCmd::SetEnabled(desired));
                }
                Err(e) => {
                    // The OS menu auto-flips the checkmark on click — put it back to the REAL
                    // state, then tell the user WHY in-app (a silent dead toggle was the exact
                    // "menu items don't respond" trap, owner 2026-07-16).
                    let _ = toggle.set_checked(shared.enabled.load(Ordering::Relaxed));
                    log::warn!("resident: toggle rejected (spec 07 §2 precondition) — {e}");
                    show_main_window(app);
                    let _ = app.emit("resident://toggle-rejected", ());
                }
            }
        }
        // "立即整理桌面": rescan now + confirm the pending batch (skip the 2h wait, spec 07 §12/§2).
        // This is also the explicit RETRY, so it acknowledges any ERROR state first (spec 07 §12:
        // ERROR→WATCHING on the user acknowledging/retrying) — a no-op when not in error.
        "dm_apply_now" => {
            shared.push_cmd(ResidentCmd::AcknowledgeError);
            shared.force_reconcile.store(true, Ordering::Relaxed);
            shared.apply_now.store(true, Ordering::Relaxed);
        }
        "dm_undo" => shared.push_cmd(ResidentCmd::UndoLast),
        // Deep-link emits ride the tauri event bus; the web listener registers during webview
        // module init, so a click landing in the first moments of a COLD boot can lose the
        // navigate payload (the window still opens — a second click navigates). Known narrow
        // window, accepted (codex 2026-07-16 P2-3): the tray exists before the webview only
        // during startup, and hide-on-close keeps listeners alive for the resident's lifetime.
        "dm_history" => {
            show_main_window(app);
            let _ = app.emit("resident://navigate", "history");
        }
        "dm_settings" => {
            show_main_window(app);
            let _ = app.emit("resident://navigate", "settings");
        }
        "dm_reset" => {
            show_main_window(app);
            let _ = app.emit("resident://navigate", "reset");
        }
        // dm_open (and anything unrecognized) just shows the window.
        _ => show_main_window(app),
    }
}

/// Shows + focuses the main window, re-creating the WebView if the close handler hid it (spec 07 §1).
fn show_main_window(app: &AppHandle<Wry>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// The windowless-residency close guard (spec 07 §1): when automation is enabled, closing the window
/// keeps the process resident instead of exiting. Called from `lib.rs`'s `on_window_event`.
/// [WINDOWS-VERIFY] `hide()` keeps the WebView in memory; swapping to `destroy()` (freeing the WebView2
/// child process, verified re-creatable on reopen) is the Windows refinement.
pub fn on_close_requested(handle: &ResidentHandle, hide: impl FnOnce()) -> bool {
    if handle.is_enabled() {
        hide();
        true // prevented — stay resident
    } else {
        false
    }
}

#[cfg(not(windows))]
fn spawn_loop(
    app: AppHandle<Wry>,
    shared: Arc<ResidentShared>,
    settings: Arc<SettingsStore>,
    data_dir: PathBuf,
    items: (MenuItem<Wry>, CheckMenuItem<Wry>, MenuItem<Wry>, MenuItem<Wry>),
    enabled0: bool,
) {
    let engine = engine::DevhostResidentEngine::new(settings.clone(), &data_dir);
    std::thread::spawn(move || {
        let host = TrayHost::new(app, items.0, items.1, items.2, items.3);
        let driver = ResidentDriver::new(MonotonicClock::new(), DriverConfig::default());
        run_loop(driver, engine, host, shared, settings, enabled0);
    });
}

#[cfg(windows)]
fn spawn_loop(
    app: AppHandle<Wry>,
    shared: Arc<ResidentShared>,
    settings: Arc<SettingsStore>,
    data_dir: PathBuf,
    items: (MenuItem<Wry>, CheckMenuItem<Wry>, MenuItem<Wry>, MenuItem<Wry>),
    enabled0: bool,
) {
    // [WINDOWS-VERIFY] the real engine + watcher. Cannot be msvc-cross-checked here (src-tauri pulls
    // rusqlite's C build) — see docs/references/windows-wiring-handoff/m7-resident.md.
    let engine = match engine::WindowsResidentEngine::new(settings.clone(), &data_dir) {
        Ok(e) => e,
        Err(e) => {
            // The loop never starts: say so IN the menu instead of leaving live-looking rows that
            // silently do nothing (the owner's "没有反应" trap, 2026-07-16).
            log::error!("resident: failed to build the Windows engine — {e}");
            let _ = items.0.set_text("自动整理：初始化失败，重启应用后重试");
            let _ = items.1.set_enabled(false);
            return;
        }
    };
    // The real desktop watcher (B10) feeds the loop's hint queue. Roots come from
    // SHGetKnownFolderPath (spec 07 §3, never hardcoded) — [WINDOWS-VERIFY] runtime.
    let watch = start_desktop_watch(shared.clone());
    std::thread::spawn(move || {
        let _watch = watch; // hold the DesktopWatch alive for the loop's lifetime
        let host = TrayHost::new(app, items.0, items.1, items.2, items.3);
        let driver = ResidentDriver::new(MonotonicClock::new(), DriverConfig::default());
        run_loop(driver, engine, host, shared, settings, enabled0);
    });
}

/// [WINDOWS-VERIFY] Arms the `notify` desktop watcher (B10) and forwards each hint into the loop's
/// queue. Returns the `DesktopWatch` guard the caller must hold. Roots resolve via
/// `SHGetKnownFolderPath` (user + public desktop, spec 07 §3 — never hardcoded); OneDrive-KFM
/// re-resolution stays a runtime verification point.
#[cfg(windows)]
fn start_desktop_watch(shared: Arc<ResidentShared>) -> Option<dm_windows::watcher::DesktopWatch> {
    let roots: Vec<PathBuf> = match dm_windows::shell::known_folders::desktop_roots() {
        Ok(r) => r,
        Err(e) => {
            log::error!("resident: desktop-root resolution failed — the watcher is not armed: {e}");
            return None;
        }
    };
    if roots.is_empty() {
        log::error!("resident: no desktop roots resolved — the watcher is not armed");
        return None;
    }
    match dm_windows::watcher::watch_desktops(roots, move |event| shared.push_event(event)) {
        Ok(watch) => Some(watch),
        Err(e) => {
            log::error!("resident: watch_desktops failed — {e}");
            None
        }
    }
}
