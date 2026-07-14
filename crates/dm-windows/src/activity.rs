//! Desktop activity detection ([WINDOWS-VERIFY]) — the `ActivityMonitor` port (spec 07 §11).
//!
//! Data safety is already the driver's CAS job; this is a UX-layer signal only: don't let an icon
//! visibly change under the user's cursor mid-drag. The reconciler POLLS `is_desktop_busy()`
//! synchronously between every icon in a batch.
//!
//! Two layered judges (spec 07 §11):
//!   * **Judge 2** (coarse, synchronous poll — `is_busy_judge2`): the foreground window is a
//!     desktop/Explorer class AND the user gave input within the recency window. Answerable from a
//!     plain synchronous read, so it needs no hook thread.
//!   * **Judge 1** (precise — the `SetWinEventHook` layer): a hook on the desktop `SysListView32`'s
//!     drag / marquee-capture / move-size WinEvents maintains a shared "busy-until" deadline, so
//!     `is_desktop_busy` returns true the INSTANT a real drag/marquee starts and stays busy ≥1.5s
//!     past release (spec 07 §7). Judge 1 is layered ON TOP of judge 2 — busy = (hook window open)
//!     OR (judge 2) — never a replacement.
//!
//! Judge 1's install is BEST-EFFORT: WinEvent callbacks arrive on the hook-owning thread, so it
//! needs a dedicated thread with a message pump maintaining an atomic deadline the poll reads. If the
//! hook can't install (no desktop listview, session 0, `SetWinEventHook` returns null) we keep judge
//! 2 as the conservative fallback — it already fails toward "busy" on any read failure — so a missed
//! drag only risks a cosmetic mid-drag repaint, never data (the driver CAS is the real safety net;
//! §11 just wants conservative suppression).
//!
//! The judge-1 handle-resolution chain (`Progman → SHELLDLL_DefView`/`WorkerW → SysListView32`) is
//! the Technique B walk documented in `shell/layout.rs`; it is re-derived here with `FindWindowExW`
//! because layout.rs leaves Technique B UNWRITTEN (its live geometry uses Technique A / IFolderView2),
//! so there is no shared function to call — only the documented walk to reproduce.
//!
//! The PURE decision logic (event kind + timestamps → busy?) is the `event_arms_busy` /
//! `arm_deadline` / `hook_busy` primitives + the `HookBusyState` reducer, all `[MAC]`-unit-tested on
//! the host below; only the live `SetWinEventHook`/callback/message-pump wiring is `[WINDOWS-VERIFY]`.

// ─── Judge 1: pure, host-testable decision logic ──────────────────────────────────────────────────

/// The busy-hold window (ms) past a drag/capture event (spec 07 §7: hold ≥1.5s past release). The
/// hold applies to the END events too, so releasing a drag keeps the desktop "busy" for this long.
#[cfg(any(windows, test))]
const DRAG_HOLD_MS: u64 = 1_500;

/// The raw `EVENT_SYSTEM_*` numeric values (`WindowsAndMessaging`) that signal desktop drag / marquee
/// selection / window move-size. Kept as bare `u32` so the classifier stays pure and host-testable
/// without pulling the Windows crate into the Mac build.
#[cfg(any(windows, test))]
mod ev {
    pub const CAPTURESTART: u32 = 8; // marquee / mouse-capture begins (rubber-band select)
    pub const CAPTUREEND: u32 = 9;
    pub const MOVESIZESTART: u32 = 10; // an icon/window move-size loop begins
    pub const MOVESIZEEND: u32 = 11;
    pub const DRAGDROPSTART: u32 = 14; // OLE drag/drop begins
    pub const DRAGDROPEND: u32 = 15;
}

/// Pure: does this raw WinEvent constant arm the desktop busy window? Both START and END events arm
/// it — the END events matter because the hold must persist ≥1.5s PAST the release, not stop at it.
/// `EVENT_SYSTEM_CONTEXTHELP*` (12/13), which also falls inside the hooked range, is intentionally
/// NOT an arming event.
#[cfg(any(windows, test))]
fn event_arms_busy(event: u32) -> bool {
    matches!(
        event,
        ev::CAPTURESTART
            | ev::CAPTUREEND
            | ev::MOVESIZESTART
            | ev::MOVESIZEEND
            | ev::DRAGDROPSTART
            | ev::DRAGDROPEND
    )
}

/// Pure: the busy-until deadline after an arming event observed at `now_ms` with a `hold_ms` window.
/// Saturating so a near-`u64::MAX` clock can never wrap the deadline back to "already expired".
#[cfg(any(windows, test))]
fn arm_deadline(now_ms: u64, hold_ms: u64) -> u64 {
    now_ms.saturating_add(hold_ms)
}

/// Pure: is the hook-tracked busy window still open at `now_ms`? `busy_until_ms == 0` is the
/// never-armed sentinel and always reads idle.
#[cfg(any(windows, test))]
fn hook_busy(now_ms: u64, busy_until_ms: u64) -> bool {
    busy_until_ms != 0 && now_ms < busy_until_ms
}

/// The pure state the live hook maintains: the current busy-until deadline. `on_event` mirrors the
/// WinEvent callback (arm/extend on a drag event, monotonic so an END never shortens a live window)
/// and `is_busy` mirrors the poll. The LIVE code and these host tests share the SAME
/// `event_arms_busy` / `arm_deadline` / `hook_busy` primitives, so the tests exercise the real
/// decision rather than a parallel reimplementation.
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
struct HookBusyState {
    busy_until_ms: u64,
}

#[cfg(test)]
impl HookBusyState {
    fn on_event(&mut self, event: u32, now_ms: u64, hold_ms: u64) {
        if event_arms_busy(event) {
            self.busy_until_ms = self.busy_until_ms.max(arm_deadline(now_ms, hold_ms));
        }
    }
    fn is_busy(&self, now_ms: u64) -> bool {
        hook_busy(now_ms, self.busy_until_ms)
    }
}

// ─── Judge 2: coarse foreground-class + input-recency poll ────────────────────────────────────────

/// The recency window (ms) within which recent input, under a desktop-class foreground window,
/// reads as "busy" (spec 07 §11 judge 2 uses < 2s).
#[cfg(windows)]
const RECENT_INPUT_MS: u32 = 2_000;

/// The `ActivityMonitor` implementation for a real Windows desktop.
#[cfg(windows)]
pub struct WindowsActivityMonitor {
    /// Judge-1 hook lifecycle, RAII (best-effort). `None` ⇒ the hook could not install and judge 2
    /// carries alone. Held only to keep the hook thread alive and tear it down on drop.
    _hook: Option<win::HookGuard>,
}

#[cfg(windows)]
impl WindowsActivityMonitor {
    /// Installs the judge-1 `SetWinEventHook` precision layer best-effort, then always answers via
    /// judge 1 (if installed) OR judge 2. A failed install is NOT an error — judge 2 is the
    /// conservative fallback (fail toward "busy").
    pub fn new() -> Self {
        Self { _hook: win::HookGuard::install() }
    }
}

#[cfg(windows)]
impl Default for WindowsActivityMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl dm_domain::ActivityMonitor for WindowsActivityMonitor {
    fn is_desktop_busy(&self) -> dm_domain::PortResult<bool> {
        Ok(win::is_desktop_busy())
    }
}

/// Whether a foreground window class name is a desktop/Explorer shell class (judge 2). Pure — the
/// class list is exercised on the Mac host; the live `GetForegroundWindow`/`GetClassNameW` reads
/// are `[WINDOWS-VERIFY]`.
#[cfg(any(windows, test))]
fn is_desktop_class(class: &str) -> bool {
    // Progman/WorkerW host the desktop; SHELLDLL_DefView/SysListView32 are the icon view;
    // CabinetWClass is an Explorer window (the user could be dragging from a folder onto the
    // desktop). Case-insensitive to be defensive about class-name casing.
    const DESKTOP_CLASSES: &[&str] = &[
        "Progman",
        "WorkerW",
        "SHELLDLL_DefView",
        "SysListView32",
        "CabinetWClass",
    ];
    DESKTOP_CLASSES.iter().any(|c| c.eq_ignore_ascii_case(class))
}

#[cfg(windows)]
mod win {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::thread::JoinHandle;

    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::System::SystemInformation::{GetTickCount, GetTickCount64};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, FindWindowExW, GetClassNameW, GetForegroundWindow, GetMessageW,
        GetWindowThreadProcessId, PeekMessageW, PostThreadMessageW, TranslateMessage,
        EVENT_SYSTEM_CAPTURESTART, EVENT_SYSTEM_DRAGDROPEND, MSG, PM_NOREMOVE, WINEVENT_OUTOFCONTEXT,
        WM_QUIT,
    };

    use super::{
        arm_deadline, event_arms_busy, hook_busy, is_desktop_class, DRAG_HOLD_MS, RECENT_INPUT_MS,
    };

    /// The judge-1 shared "busy-until" deadline in `GetTickCount64` milliseconds (`0` = never armed).
    /// A process-global because the `WINEVENTPROC` callback carries no user-data slot, and exactly one
    /// desktop `ActivityMonitor` exists in the resident. The callback (hook thread) stores via
    /// `fetch_max`; the poll (reconciler thread) loads — `Relaxed` is sufficient for one independent
    /// scalar with no companion invariants.
    static BUSY_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

    /// Judge 1 (the hook's live busy window) OR judge 2 (the coarse synchronous fallback). Judge 1
    /// makes a real drag read busy INSTANTLY and hold ≥1.5s past release; judge 2 covers the case
    /// where the hook never installed. [WINDOWS-VERIFY] runtime.
    pub(super) fn is_desktop_busy() -> bool {
        // SAFETY: a plain kernel32 monotonic-tick read.
        let now = unsafe { GetTickCount64() };
        if hook_busy(now, BUSY_UNTIL_MS.load(Ordering::Relaxed)) {
            return true;
        }
        is_busy_judge2()
    }

    /// Judge 2: the foreground window is a desktop/Explorer class AND input landed within the
    /// recency window. Any read failure reads as BUSY (err on the quiet side — a missed suppress
    /// only risks a cosmetic repaint, never data).
    fn is_busy_judge2() -> bool {
        // SAFETY: plain user32 reads, no resources to release.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return true; // no foreground window resolved → conservative
            }
            let mut buf = [0u16; 256];
            let len = GetClassNameW(hwnd, &mut buf);
            if len <= 0 {
                return true;
            }
            let class = String::from_utf16_lossy(&buf[..len as usize]);
            if !is_desktop_class(&class) {
                return false; // the user is in another app — the desktop is not busy
            }
            let mut info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if GetLastInputInfo(&mut info).as_bool() {
                let idle_ms = GetTickCount().wrapping_sub(info.dwTime);
                idle_ms < RECENT_INPUT_MS
            } else {
                true // cannot read input recency → conservative
            }
        }
    }

    /// The `WINEVENTPROC` — arrives on the hook thread's message pump (OUTOFCONTEXT). Arms/extends the
    /// shared busy window on a desktop drag / marquee / move-size event. Uses the SAME pure
    /// `event_arms_busy`/`arm_deadline` the host tests cover; only the tick read + atomic store are
    /// live. `fetch_max` keeps the window monotonic (an END event during the hold extends, never
    /// shortens, it). [WINDOWS-VERIFY] runtime.
    unsafe extern "system" fn win_event_proc(
        _hook: HWINEVENTHOOK,
        event: u32,
        _hwnd: HWND,
        _id_object: i32,
        _id_child: i32,
        _id_event_thread: u32,
        _dwms_event_time: u32,
    ) {
        if event_arms_busy(event) {
            let deadline = arm_deadline(GetTickCount64(), DRAG_HOLD_MS);
            BUSY_UNTIL_MS.fetch_max(deadline, Ordering::Relaxed);
        }
    }

    /// The RAII handle for the judge-1 hook thread. Drop signals the pump to quit and joins it (the
    /// thread unhooks itself before returning). `thread_id` is the pump thread's Win32 id, needed to
    /// `PostThreadMessageW(WM_QUIT)` at teardown.
    pub(super) struct HookGuard {
        thread_id: u32,
        join: Option<JoinHandle<()>>,
    }

    impl HookGuard {
        /// Best-effort install: resolve the desktop `SysListView32`, hook its owning thread's
        /// drag/capture/move-size events, and pump them on a dedicated thread. Returns `None` on ANY
        /// failure so the caller keeps judge 2. The install BLOCKS until the pump thread reports the
        /// hook's fate (so a `Some` guard always owns a live, quittable pump). [WINDOWS-VERIFY].
        pub(super) fn install() -> Option<HookGuard> {
            // Resolve the target thread on THIS thread first, so a missing desktop needs no spawn.
            let listview = unsafe { desktop_listview() }?;
            let mut pid = 0u32;
            let owner_thread = unsafe { GetWindowThreadProcessId(listview, Some(&mut pid)) };
            if owner_thread == 0 {
                return None;
            }
            // A one-shot handshake: the pump thread reports `Some(its_thread_id)` once the hook is
            // installed and its message queue exists, or `None` if the hook failed to install.
            let (tx, rx) = mpsc::channel::<Option<u32>>();
            let join = std::thread::Builder::new()
                .name("dm-desktop-activity-hook".into())
                .spawn(move || hook_thread_main(pid, owner_thread, tx))
                .ok()?;
            match rx.recv() {
                Ok(Some(thread_id)) => Some(HookGuard { thread_id, join: Some(join) }),
                // Hook failed to install (or the sender dropped): the thread has already returned.
                _ => {
                    let _ = join.join();
                    None
                }
            }
        }
    }

    impl Drop for HookGuard {
        fn drop(&mut self) {
            // Break the message pump; the thread then unhooks and returns. The queue is guaranteed to
            // exist (the thread pumped a `PeekMessageW` before reporting), so this post is not lost.
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }

    /// The judge-1 hook thread body: force the queue into existence, install the hook, report our
    /// thread id (or failure), pump until `WM_QUIT`, then unhook. Runs on its OWN thread so the
    /// OUTOFCONTEXT WinEvent callbacks have a message queue to dispatch through without ever touching
    /// the reconciler thread. [WINDOWS-VERIFY] runtime.
    fn hook_thread_main(pid: u32, owner_thread: u32, tx: mpsc::Sender<Option<u32>>) {
        // SAFETY: a standard user32 WinEvent hook + GetMessage pump; the hook is unhooked before
        // return and no resource outlives the thread.
        unsafe {
            // Force this thread's message queue to exist BEFORE anyone can post WM_QUIT to it (Drop)
            // and before the hook starts delivering, so no teardown post and no early event is lost.
            let mut probe = MSG::default();
            let _ = PeekMessageW(&mut probe, None, 0, 0, PM_NOREMOVE);

            let hook = SetWinEventHook(
                EVENT_SYSTEM_CAPTURESTART, // 0x0008 — low end of the drag/capture/move-size range
                EVENT_SYSTEM_DRAGDROPEND,  // 0x000F — high end; the callback filters within it
                None,
                Some(win_event_proc),
                pid,          // scope to Explorer's process …
                owner_thread, // … and the exact thread owning the desktop SysListView32
                WINEVENT_OUTOFCONTEXT,
            );
            if hook.is_invalid() {
                let _ = tx.send(None);
                return;
            }
            // Report the pump thread id so Drop can PostThreadMessage(WM_QUIT) precisely here.
            let _ = tx.send(Some(GetCurrentThreadId()));

            let mut msg = MSG::default();
            // GetMessageW returns 0 on WM_QUIT (our teardown) and -1 on error — exit the pump either
            // way; the WinEvent callbacks fire as the system dispatches through this loop.
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWinEvent(hook);
        }
    }

    /// The desktop icon `SysListView32` HWND via the Technique B chain documented in
    /// `shell/layout.rs`: `Progman → SHELLDLL_DefView → SysListView32`, with the `WorkerW` fallback
    /// for when a wallpaper host has reparented `SHELLDLL_DefView` under a top-level `WorkerW` sibling
    /// of `Progman`. `None` if any hop is missing (session 0, the shell not running). [WINDOWS-VERIFY].
    unsafe fn desktop_listview() -> Option<HWND> {
        let progman = find_child(None, None, "Progman")?;
        let defview = match find_child(Some(progman), None, "SHELLDLL_DefView") {
            Some(h) => h,
            None => defview_under_workerw()?,
        };
        find_child(Some(defview), None, "SysListView32")
    }

    /// The `SHELLDLL_DefView` hosted under a top-level `WorkerW` (active when a wallpaper slideshow /
    /// DWM has reparented the icon view out of `Progman`). Walks every `WorkerW` sibling.
    unsafe fn defview_under_workerw() -> Option<HWND> {
        let mut worker = find_child(None, None, "WorkerW")?;
        loop {
            if let Some(defview) = find_child(Some(worker), None, "SHELLDLL_DefView") {
                return Some(defview);
            }
            worker = find_child(None, Some(worker), "WorkerW")?;
        }
    }

    /// `FindWindowExW` by class name (no window-name filter), following the codebase's
    /// `HSTRING → PCWSTR` convention (overlay.rs) rather than the `w!` literal macro.
    unsafe fn find_child(parent: Option<HWND>, after: Option<HWND>, class: &str) -> Option<HWND> {
        let class_w = HSTRING::from(class);
        FindWindowExW(parent, after, PCWSTR(class_w.as_ptr()), PCWSTR(std::ptr::null())).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_and_explorer_classes_are_recognized_case_insensitively() {
        for c in ["Progman", "WorkerW", "SHELLDLL_DefView", "SysListView32", "CabinetWClass"] {
            assert!(is_desktop_class(c), "{c} is a desktop/Explorer class");
        }
        // Case-insensitive.
        assert!(is_desktop_class("progman"));
        assert!(is_desktop_class("syslistview32"));
        // A normal application window is NOT the desktop.
        assert!(!is_desktop_class("Chrome_WidgetWin_1"));
        assert!(!is_desktop_class("Notepad"));
        assert!(!is_desktop_class(""));
    }

    // ─── Judge 1 pure decision logic ([MAC]) ─────────────────────────────────────────────────────

    #[test]
    fn only_drag_capture_movesize_events_arm_busy() {
        // The six arming events (start AND end of each gesture).
        for e in [
            ev::CAPTURESTART,
            ev::CAPTUREEND,
            ev::MOVESIZESTART,
            ev::MOVESIZEEND,
            ev::DRAGDROPSTART,
            ev::DRAGDROPEND,
        ] {
            assert!(event_arms_busy(e), "{e} should arm the busy window");
        }
        // CONTEXTHELP (12/13) sits inside the hooked range but must NOT arm.
        assert!(!event_arms_busy(12));
        assert!(!event_arms_busy(13));
        // Out-of-range / unrelated events never arm (e.g. EVENT_OBJECT_CREATE = 0x8000).
        for e in [0u32, 1, 7, 16, 0x8000, 0x800B] {
            assert!(!event_arms_busy(e), "{e} must not arm");
        }
    }

    #[test]
    fn arm_deadline_is_now_plus_hold_and_saturates() {
        assert_eq!(arm_deadline(1_000, DRAG_HOLD_MS), 2_500);
        assert_eq!(arm_deadline(0, DRAG_HOLD_MS), 1_500);
        // Never wraps past the top of the clock.
        assert_eq!(arm_deadline(u64::MAX, DRAG_HOLD_MS), u64::MAX);
    }

    #[test]
    fn hook_busy_is_open_only_before_the_deadline() {
        // Never-armed sentinel.
        assert!(!hook_busy(0, 0));
        assert!(!hook_busy(1_000_000, 0));
        // Open strictly before the deadline, closed at and after it.
        assert!(hook_busy(2_499, 2_500));
        assert!(!hook_busy(2_500, 2_500));
        assert!(!hook_busy(3_000, 2_500));
    }

    #[test]
    fn a_drag_reads_busy_instantly_and_holds_at_least_1_5s_past_release() {
        let mut s = HookBusyState::default();
        // Drag starts at t=1000 → busy instantly and through the hold window.
        s.on_event(ev::DRAGDROPSTART, 1_000, DRAG_HOLD_MS);
        assert!(s.is_busy(1_000), "busy the instant the drag starts");
        assert!(s.is_busy(2_400), "still busy mid-drag");

        // Release (DRAGDROPEND) at t=2_000 must extend the window to ≥ release + 1.5s.
        s.on_event(ev::DRAGDROPEND, 2_000, DRAG_HOLD_MS);
        let release = 2_000u64;
        assert!(s.is_busy(release + 1_499), "held <1.5s past release — too short");
        assert!(!s.is_busy(release + 1_500), "released exactly at the 1.5s boundary");
        assert!(!s.is_busy(release + 5_000), "long-idle after release reads idle");

        // The hold spans at least 1.5s (spec 07 §7).
        assert!(arm_deadline(release, DRAG_HOLD_MS) - release >= 1_500);
    }

    #[test]
    fn a_later_event_extends_but_never_shortens_a_live_window() {
        let mut s = HookBusyState::default();
        s.on_event(ev::CAPTURESTART, 1_000, DRAG_HOLD_MS); // busy_until = 2_500
        // An out-of-order earlier event (deadline 2_400) must NOT shrink the live window.
        s.on_event(ev::CAPTUREEND, 900, DRAG_HOLD_MS);
        assert!(s.is_busy(2_499), "the live window was not shortened");
        // A genuinely later event extends it.
        s.on_event(ev::MOVESIZESTART, 3_000, DRAG_HOLD_MS); // busy_until = 4_500
        assert!(s.is_busy(4_499));
        assert!(!s.is_busy(4_500));
    }

    #[test]
    fn a_non_arming_event_leaves_the_window_untouched() {
        let mut s = HookBusyState::default();
        s.on_event(0x8000, 1_000, DRAG_HOLD_MS); // EVENT_OBJECT_CREATE — ignored
        assert!(!s.is_busy(1_000), "a non-drag event never marks busy");
        s.on_event(12, 1_000, DRAG_HOLD_MS); // CONTEXTHELP — ignored
        assert!(!s.is_busy(1_000));
    }
}
