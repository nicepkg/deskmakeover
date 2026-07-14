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
   is now blind-written and layered ON TOP of judge 2 — see the **T2 judge-1** recipe below. Judge 2
   remains the fallback when the hook can't install.

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
- **Autostart, tray bitmaps, SHGetKnownFolderPath** — items 4, 14, 10 above. (Judge-1 hook: now
  blind-written — see the T2 judge-1 recipe below; still `[WV]` for live confirmation.)

## T2 judge-1 — `SetWinEventHook` drag/capture precision layer (`crates/dm-windows/src/activity.rs`)

Layered ON TOP of the existing judge-2 coarse poll (do not replace it): `is_desktop_busy` = (judge-1
hook window open) OR (judge-2). Blind-written, `cargo check -p dm-windows --target
x86_64-pc-windows-msvc` clean; the pure decision logic is `[MAC]`-unit-tested (`activity::tests` —
`only_drag_capture_movesize_events_arm_busy`, `arm_deadline_is_now_plus_hold_and_saturates`,
`hook_busy_is_open_only_before_the_deadline`, `a_drag_reads_busy_instantly_and_holds_at_least_1_5s_past_release`,
`a_later_event_extends_but_never_shortens_a_live_window`, `a_non_arming_event_leaves_the_window_untouched`).
The live `SetWinEventHook`/callback/message-pump wiring is what these items confirm.

Each item: **action → expected → file:line.**

1. **The hook installs against the real desktop listview.**
   Launch a build that constructs `WindowsActivityMonitor::new()` (the resident's activity port). → a
   background thread `dm-desktop-activity-hook` exists (Process Explorer / a log), and it resolved the
   `Progman → SHELLDLL_DefView → SysListView32` chain (add a temporary `dbg!` in `desktop_listview`).
   → `activity.rs` `win::HookGuard::install` + `desktop_listview` + `hook_thread_main`
   (`SetWinEventHook(EVENT_SYSTEM_CAPTURESTART..=EVENT_SYSTEM_DRAGDROPEND, …, WINEVENT_OUTOFCONTEXT)`).
   On a box where the walk fails (session 0), `install()` returns `None` and judge 2 carries — verify
   nothing panics and `is_desktop_busy` still answers.

2. **A real drag reads busy INSTANTLY (spec 07 §11 judge 1).**
   With the hook installed, start dragging an icon (or rubber-band marquee) on the desktop and, mid-
   drag, poll `is_desktop_busy()`. → returns `true` from the first `EVENT_SYSTEM_DRAGDROPSTART` /
   `EVENT_SYSTEM_CAPTURESTART` — faster than judge 2's <2s input-recency window. → callback
   `win::win_event_proc` (`event_arms_busy` → `BUSY_UNTIL_MS.fetch_max`), poll `win::is_desktop_busy`.

3. **Busy holds ≥1.5s PAST release (spec 07 §7).**
   Release the drag and keep polling. → `is_desktop_busy()` stays `true` until ~1.5s after the
   `…END` event, then flips to judge-2's answer. Measure the interval ≥ `DRAG_HOLD_MS` (1500ms). →
   `arm_deadline(now, DRAG_HOLD_MS)` on the END event; `hook_busy(now, busy_until)` in the poll.
   (This is the runtime twin of the `a_drag_reads_busy…1_5s_past_release` host test.)

4. **Teardown leaves no thread / hook.**
   Drop the monitor (or exit). → `HookGuard::drop` posts `WM_QUIT`, the pump exits, `UnhookWinEvent`
   runs, and the `dm-desktop-activity-hook` thread joins cleanly (no leaked hook in Spy++'s hook list,
   no hung thread). → `impl Drop for HookGuard` + the `GetMessageW` loop exit + `UnhookWinEvent`.

## System item — CLSID `DefaultIcon` read/apply/restore (`state_reader.rs` + `apply/mod.rs` + `apply/system.rs`)

Already landed (commits `2663f76`/`ffb202e`/`336cc37`) and wired into `state_reader.rs`
(`read_fingerprint`/`capture_anchor` `ItemKind::System` arms) + `apply/mod.rs`
(`apply`/`restore` `ItemKind::System` arms + the CLSID-match restore guard). PER-USER (HKCU) only —
the machine (HKLM/HKCR) scope is the §14 red line the resident refuses. Pure `parse_clsid`
(CLSID→key-path) + the anchor round-trip are `[MAC]`-tested (`apply::system::tests`,
`restore::tests::system_icon_anchor_round_trips_*`). The live registry read/write is what these items
confirm.

> **Key-location note (`[WV]`).** `apply/system.rs` writes the per-user override at
> `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\CLSID\{clsid}\DefaultIcon` — the SAME
> location the Recycle-Bin sibling (`apply/recyclebin.rs`) and the Windows "Desktop Icon Settings"
> applet use, chosen for sibling-consistency and because that path is the proven desktop-icon
> personalization key. The generic COM per-user override `HKCU\Software\Classes\CLSID\{clsid}\DefaultIcon`
> is an alternative; if box testing shows Explorer does NOT honor the `Explorer\CLSID` path for a
> given namespace icon, switch `user_key()` to `Software\Classes\CLSID\…`. Both are HKCU (§14 safe).

Each item: **action → expected → file:line.**

1. **This-PC icon styles via the per-user CLSID override.**
   Apply a style to the This-PC item (CLSID `{20D04FE0-3AEA-1069-A2D8-08002B30309D}`). → the desktop
   "This PC" icon changes; the value lands ONLY under `HKCU\…\Explorer\CLSID\{20D04FE0-…}\DefaultIcon`
   (`reg query`), nothing under HKLM/HKCR. Explorer repaints after the central `SHChangeNotify`
   (`refresh.rs`). → `apply/mod.rs` `ItemKind::System => system::apply(&parse_clsid(path)?, &icon)`,
   `apply/system.rs` `apply` (`create_subkey(user_key)` + `set_value`).

2. **Restore reverts cleanly (no residue) — key created vs pre-existing.**
   Restore. → if no per-user key existed before apply, the value we wrote is deleted (the now-empty key
   may remain, never `delete_subkey_all`); if a per-user value pre-existed, its exact raw bytes + kind
   (`REG_SZ`/`REG_EXPAND_SZ`, unexpanded `%SystemRoot%`) are rewritten. The icon returns to the
   original. → `apply/system.rs` `restore` (`key_existed` branch) driven by `state_reader.rs`
   `capture_anchor` `ItemKind::System` → `RestoreAnchor::SystemIcon`.

3. **Machine scope is REFUSED (spec 07 §14 red line).**
   Confirm no code path writes HKLM/HKCR for a System item: `apply`/`restore` only ever open
   `HKEY_CURRENT_USER`; `machine_value()` reads HKCR but never writes it. → `apply/system.rs` (grep:
   the only `HKEY_CLASSES_ROOT` use is the read-only `machine_value`).

4. **A malformed / mismatched CLSID is rejected, not mis-written.**
   Feed a non-`::{GUID}` target or a restore whose anchor CLSID ≠ the target's. → `parse_clsid` returns
   `Unsupported` for a non-GUID string (no key interpolation), and `apply/mod.rs` restore returns the
   "CLSID mismatch" error rather than mutating the wrong key. → `apply/system.rs` `parse_clsid` /
   `is_registry_guid`; `apply/mod.rs` restore CLSID re-derive + equality guard.

   **Per-CLSID `Unsupported` kept:** NONE. All five real desktop-namespace icons (This PC, Network,
   Recycle Bin, Control Panel, User's Files) accept a per-user `DefaultIcon` override, so no valid
   desktop CLSID is genuinely unstyleable. The only `Unsupported` path is a target string that is not a
   canonical `::{GUID}` (rejected by `parse_clsid`), which is correct — not a blanket stub.
