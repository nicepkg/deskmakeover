# Spec 07 — Background Resident Auto-Format (v1)

Living spec. Normative behaviour for the resident tray process that auto-formats new
desktop icons per the user's saved style. Decision record: ADR-0020 (v1 inclusion +
incremental-ledger restore), ADR-0019 (native Rust renderer, no WebView). Supersedes
spec 06 §7.

## Scope / Non-scope / Assumptions / Dependencies

- **Scope**: one per-user resident process; watch user + public desktop; detect
  new/changed items; auto-format user-desktop items per saved style, `kindPolicy`,
  `typeOverrides`, per-icon keeps; incremental ledger versions; conflict handling;
  tray surface; catch-up reconciliation.
- **Non-scope (v1)**: background wallpaper changes (wallpaper is edit-time only,
  spec 04); machine-level/public-desktop silent writes (queued instead); any
  elevation from the background; Explorer-module surfaces (drive/folder trees beyond
  the desktop); ML-based anything.
- **Assumptions**: the manual apply/restore path (same crates) has passed its own
  gates; the saved style is the single style truth (no separate "background style").
- **Dependencies**: `dm-resident` (reconciler/jobs/queue), `dm-windows` (watcher,
  scan, writers, STA actor), `dm-icon-core` native (headless render), `dm-operations`
  (durable ledger), tray/single-instance/autostart plugins (ADR-0019).

## 1. Process model

- One long-lived, non-elevated Rust process per user session. `--background` startup
  creates NO WebView; the tray icon is the only surface until the user opens the
  window. Closing the window destroys the WebView (verified child-process exit) and
  returns to windowless residency.
- Single instance per Windows user/session. A separate machine-scope lock guards
  global-overlay/public-desktop operations.
- Autostart registers only after the user enables resident automation; disabling
  automation unregisters it.
- Tray menu (fixed order): 打开 · 暂停/继续自动美化 · 立即核对桌面 · 待处理特权项
  (N) · 还原… · 退出. Every label states what it does; no mystery verbs.

## 2. Consent ladder (trust contract, carried from spec 06 §7)

1. Feature default OFF. The offer appears in the post-apply DoneCard.
2. First detection run is a **proposal**: 「有 N 个新图标，要美化吗？」 — nothing is
   written before the user says yes once.
3. Silent mode only after that consent; a toggle chip stays always visible in the
   icons module; 「新增图标」 markers show what changed since last visit.
4. Every automatic change lands in version history as an undoable entry.
5. Turning automation off never retro-reverts, and the UI says so.
6. The background NEVER pops UAC. Public-desktop/machine items queue as visible
   pending work; one batched UAC completes them when the user opens the window.

## 3. Change detection (events are hints; reconciliation is truth)

- Sources: `ReadDirectoryChangesW`/`notify` on the user desktop AND public desktop
  (paths via `SHGetKnownFolderPath` — never hardcoded; re-resolved after resume,
  policy change, OneDrive Known Folder Move); Shell change notifications for virtual
  items; full reconcile on: startup, wake-from-sleep, watcher overflow/error,
  Explorer restart, app update, abnormal-exit recovery.
- Debounce: user-configurable 2–10s (default 4s) coalescing installer bursts.
- Stability probe before processing a `.lnk`: file opens, size+mtime stable across
  two probes, `IShellLink` parses, target + IconLocation populated.
- Handle Created/Changed/Renamed/Deleted; installers commonly write-temp → rename.

## 4. Identity and self-write suppression

- Source fingerprint covers MORE than the `.lnk` bytes: link fields, target path +
  target file version/mtime, IconLocation + its file state, AUMID/package family +
  version + chosen resource variant. A target app update with an unchanged `.lnk`
  IS a change.
- File identity (volume + file id) distinguishes rename from replace.
- Self-write guard: operation id + before-hash + expected-after-hash + time window;
  our own applies never re-enter the queue (no format loops).

## 5. Apply and the incremental ledger (ADR-0020 §2)

- Render: native `dm-icon-core` via the same `RenderSession` model as the foreground;
  profile cache keyed `source_hash + analysis_schema_version` is persisted on disk, so
  a background single-icon format skips re-analysis.
- Hue: new icons allocate against **pinned existing seeds**; existing icons never
  reflow. Global rebalance only via explicit foreground re-apply.
- Generated ICOs are content-addressed (`<source-hash>-<style-hash>.ico`); write new
  file first, then swap IconLocation; GC only ledger-unreferenced files.
- Ledger entry per item: original fingerprint + restore anchor, last-applied
  fingerprint, owned fields, generated asset, transaction state
  (prepared → asset-written → applied → verified → committed). Persist + flush before
  every external mutation; a corrupted ledger NEVER reads as "nothing applied".
- Each background run appends incremental version entries to the SAME history the
  manual flow uses — one undo surface.
- Restore/re-apply is per-item compare-and-swap: current state ≠ our last-applied
  fingerprint → visible conflict (user/installer wins); no silent overwrite; no
  restore-the-whole-desktop-first behaviour.
- Ordinary-file wrapping (structural: companion `.lnk` + hidden original) defaults to
  the proposal queue even in silent mode; a setting may promote it.

## 6. Exclusions (never touched by automation)

Per-icon 「保留原样」 · `styleable:false` items · buckets with `kindPolicy=false` ·
items with a user/installer conflict flag until the user resolves it · anything on the
public desktop or requiring elevation (queued instead).

## 7. Verification (release gates for the resident path)

- Burst test: temp-write→rename installer storms; exactly one format per final item.
- Overflow test: forced watcher overflow → full rescan converges, nothing missed.
- Self-write test: N applies produce zero re-queued events.
- Kill-point battery: process death injected around every ledger transition; restart
  recovers to a consistent state; restore stays exact.
- Conflict test: externally modify a styled item; automation flags, does not touch.
- Environment matrix: OneDrive-redirected desktop, sleep/resume, Explorer restart,
  standard user, second user on the same machine (public desktop untouched).
- Idle budget: resident process ≈0% CPU idle, no WebView children, bounded RSS;
  warm single-icon background format under 250ms (Codex budget).
