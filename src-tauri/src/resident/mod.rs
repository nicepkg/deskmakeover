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
    DriverConfig, MonotonicClock, Proposal, ResidentDriver, ResidentHost, TrayState, VettedCandidate,
    WatchEvent, PROPOSAL_TIMEOUT_SECS,
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
    pending: Option<Pending>,
}

impl TrayHost {
    fn new(app: AppHandle<Wry>, status: MenuItem<Wry>, toggle: CheckMenuItem<Wry>, pending_item: MenuItem<Wry>) -> Self {
        Self { app, status, toggle, pending_item, pending: None }
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
    initial_enabled: bool,
) {
    // Reflect the persisted toggle on boot (spec 07 §2 — enablement survives restarts).
    if initial_enabled {
        driver.set_enabled(true, &mut host);
    } else {
        host.render_tray(&TrayState::Off, 0);
    }
    while !shared.shutdown.load(Ordering::Relaxed) {
        for cmd in shared.take_cmds() {
            match cmd {
                ResidentCmd::SetEnabled(on) => {
                    shared.enabled.store(on, Ordering::Relaxed);
                    driver.set_enabled(on, &mut host);
                }
                ResidentCmd::AcknowledgeError => driver.on_error_acknowledged(&mut host),
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
    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(tooltip_for(&initial))
        .icon(app.default_window_icon().expect("bundled window icon").clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| on_menu_event(app, event.id.as_ref(), &shared_evt, &settings_evt))
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    spawn_loop(app.clone(), shared.clone(), settings, data_dir, (status, toggle, pending_item), enabled0);
    Ok(ResidentHandle(shared))
}

/// Handles a tray menu click (main thread). Enable/disable go through the settings-patch layer FIRST
/// (spec 07 §2 precondition — a `true` while ② is empty is rejected there), then queue the driver.
fn on_menu_event(app: &AppHandle<Wry>, id: &str, shared: &Arc<ResidentShared>, settings: &Arc<SettingsStore>) {
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
                Err(e) => log::warn!("resident: toggle rejected (spec 07 §2 precondition?) — {e}"),
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
        // Every other item opens the window (deep-linking history/settings/reset/undo is a frontend
        // follow-up; "恢复系统原始外观" lands on Settings › Advanced there, never inline — spec §13).
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
    items: (MenuItem<Wry>, CheckMenuItem<Wry>, MenuItem<Wry>),
    enabled0: bool,
) {
    let engine = engine::DevhostResidentEngine::new(settings, &data_dir);
    std::thread::spawn(move || {
        let host = TrayHost::new(app, items.0, items.1, items.2);
        let driver = ResidentDriver::new(MonotonicClock::new(), DriverConfig::default());
        run_loop(driver, engine, host, shared, enabled0);
    });
}

#[cfg(windows)]
fn spawn_loop(
    app: AppHandle<Wry>,
    shared: Arc<ResidentShared>,
    settings: Arc<SettingsStore>,
    data_dir: PathBuf,
    items: (MenuItem<Wry>, CheckMenuItem<Wry>, MenuItem<Wry>),
    enabled0: bool,
) {
    // [WINDOWS-VERIFY] the real engine + watcher. Cannot be msvc-cross-checked here (src-tauri pulls
    // rusqlite's C build) — see docs/references/windows-wiring-handoff/m7-resident.md.
    let engine = match engine::WindowsResidentEngine::new(settings, &data_dir) {
        Ok(e) => e,
        Err(e) => {
            log::error!("resident: failed to build the Windows engine — {e}");
            return;
        }
    };
    // The real desktop watcher (B10) feeds the loop's hint queue. Roots MUST come from
    // SHGetKnownFolderPath (spec 07 §3, never hardcoded) — resolved here is a [WINDOWS-VERIFY] point.
    let watch = start_desktop_watch(shared.clone());
    std::thread::spawn(move || {
        let _watch = watch; // hold the DesktopWatch alive for the loop's lifetime
        let host = TrayHost::new(app, items.0, items.1, items.2);
        let driver = ResidentDriver::new(MonotonicClock::new(), DriverConfig::default());
        run_loop(driver, engine, host, shared, enabled0);
    });
}

/// [WINDOWS-VERIFY] Arms the `notify` desktop watcher (B10) and forwards each hint into the loop's
/// queue. Returns the `DesktopWatch` guard the caller must hold. Known-folder resolution +
/// OneDrive-KFM re-resolution are the runtime verification points.
#[cfg(windows)]
fn start_desktop_watch(shared: Arc<ResidentShared>) -> Option<dm_windows::watcher::DesktopWatch> {
    // [WINDOWS-VERIFY] resolve via SHGetKnownFolderPath (user + public desktop); this env-based
    // placeholder is NOT spec-compliant (§3 forbids hardcoding) and exists only so the blind wiring
    // is shaped — replace with the real known-folder resolver on the box.
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(user) = std::env::var("USERPROFILE") {
        roots.push(PathBuf::from(user).join("Desktop"));
    }
    if let Ok(public) = std::env::var("PUBLIC") {
        roots.push(PathBuf::from(public).join("Desktop"));
    }
    if roots.is_empty() {
        log::error!("resident: no desktop roots resolved — the watcher is not armed ([WINDOWS-VERIFY])");
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
