# [WINDOWS-VERIFY] — M7 resident driver + tray/windowless-residency wiring

> **Status:** code EXECUTED (blind-written on Mac, native-`cargo check` clean, driver logic + Mac
> devhost loop unit-tested). Everything below is the RUNTIME gate a future on-Windows AI/owner runs.
> Companion to `docs/plans/2026-07-12-m7-resident.md` (T4/T8) and the watcher handoff in
> `docs/plans/2026-07-10-m34-windows-blind.md` §9 (B10). Normative behaviour: spec 07 §1/§3/§11/§12/§14.

## What landed (Mac-verified vs blind)

| Piece | File | Status |
|---|---|---|
| Reconcile-loop driver (coalesce · periodic backstop · tray SM · proposal surface) | `crates/dm-resident/src/driver/mod.rs` | **[MAC]** — 9 unit tests, incl. a real `Reconciler` over a `FakeActivityMonitor` |
| Host-side reconcile engine (ports + `reconcile`/`apply_batch`) | `src-tauri/src/resident/engine.rs` | Mac devhost engine **[MAC]** unit-tested; Windows engine **[WV]** (blind) |
| Tray + menu + windowless residency + loop spawn | `src-tauri/src/resident/mod.rs` | compiles + tray API is cross-platform; real notification-area behaviour **[WV]** |
| lib.rs registration (tray setup, close/exit guards) | `src-tauri/src/lib.rs` | native `cargo check` clean; residency under real Explorer **[WV]** |

**msvc note (pre-existing environmental blocker, NOT this task).** `cargo check -p dm-windows -p
dm-domain --target x86_64-pc-windows-msvc` is **green**. `dm-resident` / `deskmakeover-desktop`
CANNOT be msvc-cross-checked from Mac: they pull C-building crates (`blake3`, `rusqlite`) whose
cross-compile needs the Windows CRT/SDK (same blocker documented in `2026-07-10-m34-windows-blind.md`).
On the Windows box the native msvc toolchain compiles them, so the checklist below is where those
crates' Windows correctness is actually established.

## Prerequisites on the box

1. `cargo build -p deskmakeover-desktop` compiles (native msvc) — expected clean; if the `#[cfg(windows)]`
   Windows engine (`engine.rs:230-320`) has a blind typo it surfaces HERE first. Fix in place.
2. A completed global Apply exists so ② saved-style is non-empty (spec 07 §2 precondition) — the
   resident is dormant until then by design.

## Numbered runtime checklist

Each item: **action → expected → file:line verified.**

1. **Tray renders in the notification area.**
   Launch the app. → A tray glyph appears; right-click shows the menu in this exact order: a disabled
   status line, ☑「自动整理新图标」, 「立即整理桌面」, 「查看最近整理记录」, 「待处理特权项(N)」,
   「撤销最近一次整理」, ─, 「打开 DeskMakeover」, 「恢复系统原始外观…」, 「设置」, 「退出」. NO string
   contains 版本/快照/回退 (spec 07 §8.1). → `src-tauri/src/resident/mod.rs:281-296` (menu build),
   `:271` (`setup`).

2. **Status line + tooltip track the 5 states (spec 07 §12).**
   Enable automation. → tooltip = 「正在为你保持桌面风格」, status line = 「自动整理：守护中」. → tooltip
   map `resident/mod.rs:126-136` (`tooltip_for`), status `:139-149` (`status_text_for`),
   render `:200-210` (`TrayHost::render_tray`).

3. **Enable is gated by the ② precondition (spec 07 §2 item 2).**
   With ② empty, click ☑「自动整理新图标」. → the toggle does NOT enable; a warn log
   "toggle rejected (spec 07 §2 precondition?)" appears (the settings-patch layer rejected it). Apply a
   style, retry → it enables. → `resident/mod.rs:331-342` (`dm_toggle` arm → `SettingsStore::set`).

4. **Autostart registers ONLY on enable, unregisters on disable (spec 07 §1).**
   ⚠️ **NOT YET WIRED — implement + verify.** The current wiring persists the toggle but does not yet
   register `tauri-plugin-autostart` / a Task-Scheduler entry. Add the autostart plugin registration to
   the `dm_toggle` enable path (`resident/mod.rs:331`) and the disable path, then verify: enabling
   creates exactly one autostart entry; disabling leaves zero registry/Task-Scheduler residue
   (spec 07 §1 test). Until wired, the resident only runs while the app is open.

5. **Window-close stays resident under real Explorer (spec 07 §1).**
   With automation ON, close the main window (the X). → the WebView disappears but the process stays
   alive (tray still present, Task Manager still lists it); no exit. → `src-tauri/src/lib.rs:227-239`
   (`on_window_event` → `resident::on_close_requested`), `resident/mod.rs:367-374`.
   With automation OFF, closing exits normally. → same site (`is_enabled()` false → not prevented).
   **[WV] refinement:** the guard currently `hide()`s the WebView (kept in memory). To truly free the
   WebView2 child process per spec 07 §1 ("verified child-process exit"), swap `window.hide()` →
   `window.destroy()` at `lib.rs:234` and confirm reopening (step 6) re-creates it; keep the
   `RunEvent::ExitRequested` guard (`lib.rs:277-285`) so destroying the last window does not exit.

6. **Reopening works.**
   Left-click the tray glyph (or 「打开 DeskMakeover」). → the window shows + focuses (re-created if it
   was destroyed). → `resident/mod.rs:308-316` (tray-click → `show_main_window`), menu open arm `:344-349`.

7. **A real desktop file-create drives exactly one format (spec 07 §3/§7 burst test).**
   With automation ON, run an installer / `New > Shortcut` on the desktop. → within one debounce+settle
   cycle a proposal surfaces (log "resident: proposing N new icon(s)"; an OS event
   `resident://proposal`), and confirming (「立即整理桌面」) formats each NEW item EXACTLY once — a
   temp-write→rename storm yields one format per final item, not one per hint. → watcher feeds
   `resident/mod.rs:426-447` (`start_desktop_watch` → `shared.push_event`); coalescing
   `crates/dm-resident/src/driver/mod.rs:172-189` (`note_events`) + `:191-214` (`tick`); apply
   `resident/mod.rs:252-262`.

8. **The periodic full reconcile catches a burst-overflow (spec 07 §3, notify-8.2 silent-overflow).**
   Create MANY files at once (overflow the watch buffer) so hints are silently dropped on Windows. →
   nothing formats immediately from hints, BUT within the periodic horizon (default 300s) a full
   reconcile fires with NO hints and picks up every missed item. Temporarily lower
   `DriverConfig::full_reconcile` (`driver/mod.rs:94`) to a few seconds to verify quickly. → periodic
   gate `driver/mod.rs:196-197` (`periodic_due`); this is the ONLY reliable recovery — `WatchEvent::Overflow`
   is logged but NOT relied on alone (`driver/mod.rs:180-186`).

9. **Activity-suppression holds ≥1.5s past a real drag (spec 07 §7/§11).**
   Start a proposal, then drag an icon on the desktop while the batch would apply. → the tray shows
   PAUSED (「桌面使用中，已暂停」) and NO write lands during the drag; ≥1.5s after releasing, the
   deferred batch applies (nothing dropped). → `WindowsActivityMonitor` (`crates/dm-windows/src/activity.rs`,
   judge-2 v1) → `deferred_busy` → PAUSED mapping `driver/mod.rs:217-221` (`absorb`); the reconciler
   re-checks between every icon (`crates/dm-resident/src/reconciler/mod.rs:165,297,413`).
   **[WV] precision layer:** judge 1 (`SetWinEventHook` DRAGDROP/CAPTURE on the desktop `SysListView32`)
   is still a documented enhancement (`activity.rs` module doc) — verify judge 2 suffices first.

10. **Known-folder resolution (spec 07 §3 — never hardcode) + fail-closed scope (spec 07 §14).**
    ⚠️ **`start_desktop_watch` currently uses an env-based placeholder** (`resident/mod.rs:434-441`,
    `%USERPROFILE%\Desktop` + `%PUBLIC%\Desktop`). Replace with `SHGetKnownFolderPath` (user + public
    desktop; re-resolve after resume / policy change / OneDrive KFM). Separately, the Windows engine's
    scope is `ScopeRoots::Unresolved` (`engine.rs:258`) → automation FAILS CLOSED (styles nothing) until
    you resolve the real roots and swap to `ScopeRoots::resolved(public, programdata)?`. Verify: before
    resolution nothing is styled AND nothing floods the pending-privileged queue; after resolution, a
    Public-Desktop item is routed to 「待处理特权项(N)」 and NEVER formatted (spec 07 §14 red line).

11. **The background NEVER elevates (spec 07 §14 red line).**
    Place an item under `Public Desktop` / `ProgramData` while the resident runs. → it appears in the
    「待处理特权项(N)」 count, is never formatted, and NO UAC prompt ever appears from the background. The
    batched UAC only fires when the user opens the window (drain path). → structural (dm-resident has no
    `dm-elevated` dep) + tray count `resident/mod.rs:203-206`; queue depth from
    `driver.pending_privileged()` (`driver/mod.rs:243-246`).

12. **Self-write suppression (spec 07 §4/§7).**
    Confirm a format, then watch for a self-format loop. → the app's own `desktop.ini`/ICO writes do NOT
    re-enter as new-icon events (the reconciler's ledger CAS + op-epoch guard). → reconciler
    `crates/dm-resident/src/reconciler/mod.rs:193-236`.

13. **Crash recovery shares the foreground journal (spec 07 §1).**
    The Windows engine writes `ledger.json` / `txn.log` in the app-data dir (`engine.rs:260-261`), the
    SAME files the foreground + `run_startup_recovery` use (`lib.rs:171`). Kill the process mid-apply →
    restart → recovery drives the interrupted txn to a terminal state; restore stays exact.
    ⚠️ **Concurrency [WV]:** the foreground `IconHost` also holds these under its own mutex. Verify the
    resident's separate `JsonLedgerStore`/`FileJournal` handle does not interleave a write with a
    foreground apply (the intended fix is a shared lock or a single owning actor — currently each opens
    its own handle). The Mac devhost engine sidesteps this with in-memory stores (`engine.rs:141-143`),
    so this contention is Windows-only and unverified.

14. **Theme-aware tray bitmaps (spec 07 §16, T11).**
    ⚠️ **NOT WIRED** — the tray uses the default window icon (`resident/mod.rs:299`), not the double-coded
    16/20/24/32px light/dark glyph pairs. Ship the bitmaps + the `AppsUseLightTheme`/`WM_SETTINGCHANGE`
    watch (T11) and verify the glyph swaps on a light/dark toggle without a restart.

## Follow-ups explicitly deferred (documented, not silently dropped)

- **OS-native toast** (`tauri-plugin-notification` / WinRT, spec 07 §2 item 6) — the proposal currently
  logs + emits `resident://proposal` (`resident/mod.rs:212-219`); the inline-undo toast is a follow-up.
- **2h-timeout persistence** (spec 07 §2 item 4) — the timeout is in-memory only
  (`resident/mod.rs:167-173`); it does not survive a restart yet.
- **Deep-linked tray items** — 「查看最近整理记录」/「设置」/「恢复系统原始外观…」/「撤销最近一次整理」 all
  currently just open the window (`resident/mod.rs:344-349`); routing to the specific screens + the
  §13 reset-confirmation surface is frontend work.
- **Autostart, tray bitmaps, SHGetKnownFolderPath, judge-1 hook** — items 4, 14, 10, 9 above.
