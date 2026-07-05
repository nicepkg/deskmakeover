# DeskMakeover MVP Product Architecture

## Product Identity

DeskMakeover is the English product name. The Chinese product name is **桌面美颜**
(renamed from 桌面整容大师 by [ADR-0002](../decisions/0002-one-click-product-form.md)).

Chinese slogan:

> 一键美颜你的 Windows 桌面，随时完整还原。

English slogan:

> Give your Windows desktop a one-click makeover. Restore everything anytime.

The product must feel like a **beauty camera for the desktop**: light, instant,
obviously reversible. It must not feel like a system utility, registry tweak wrapper,
script launcher, or cleaner. The mental model the UI builds is "美颜相机", never
"电脑管家". Visual personality and rendering rules live in
[specs/02-visual-language.md](02-visual-language.md).

## Target Users

The MVP targets non-technical Chinese Windows users who want a cleaner desktop but do
not know or trust PowerShell, registry edits, or manual icon replacement. It should
also satisfy design-sensitive users through restraint and craft, not through options.
When the two audiences conflict, the non-technical user wins.

The first release supports Windows 10 and Windows 11. Windows 7 is outside the
mainline MVP. Windows 10 accepts graceful visual degradation (no Mica, plain
backdrop, Segoe UI instead of Segoe UI Variable).

## MVP Scope

The MVP completes one reliable loop:

1. Launch and scan the desktop automatically.
2. Show the before/after transformation immediately.
3. On the single primary action: snapshot invisibly, apply styling, refresh safely.
4. Restore the previous state completely from an ever-present entry point.

The MVP includes:

- Desktop item discovery for user Desktop and Public Desktop (OneDrive-redirected
  desktops detected via known-folder resolution).
- Support for `.lnk` shortcuts, `.url` shortcuts, AppX/UWP shortcuts, Recycle Bin,
  folders, and regular files.
- One default style only: continuous-corner (squircle) rendering with automatic
  background handling. No presets, no per-item selection, no filters.
- Shortcut arrow overlay removal inside the one-click flow (privileged; see Trust
  Flow below).
- Desktop icon layout snapshot and best-effort restore, running invisibly around
  every mutating operation.
- Versioned restore snapshots, created automatically — never by a user action.
- A completion screen with a "save before/after image" share hook.
- English and Simplified Chinese localization; Chinese is the primary voice.
- Local-only operation with no account, upload, telemetry, or cloud dependency.

The MVP excludes:

- AI icon generation, icon marketplace, style packs, color filters.
- Batch selection, style presets, per-item filters, a layout page, a restore center
  (superseded by ADR-0002).
- Wallpaper, taskbar, cursor, widget, or complete theme management.
- Enterprise policy management (but managed machines must degrade gracefully).
- Windows 7 support.
- Any irreversible system makeover.

Future modules must obey the ADR-0002 rule: fold into the default one-click result,
or live in settings as an advanced option. The primary flow never gains steps.

## User Experience

### The primary flow (the whole product)

1. **Launch.** The app scans the desktop in the background immediately. No scan
   button exists. While scanning, the grid shows shimmering squircle skeleton tiles.
2. **The mirror.** The main screen shows the transformation: a hero before/after
   region plus a grid of the user's real icons rendered in the default style.
   Long-press on any tile crossfades back to the original icon (press-to-peek).
3. **One button.** A single primary action: **一键美化**. Next to it, one plain
   promise: 「只美化图标外观，不动你的文件 · 随时一键还原」.
4. **Consent card (first run / when arrow removal will run).** Before UAC, a card
   explains in plain language: what will happen, what will not happen, and that one
   administrator approval will be requested. One confirm, one system UAC prompt.
5. **Apply.** A snapshot is created automatically first (no snapshot, no apply).
   Progress is shown as the transformation itself: tiles bloom into their styled
   form in a staggered wave across the real grid. A single quiet status line
   replaces stage jargon; the words 备份/快照/计划/扫描/dry-run never appear.
6. **Done.** 「✨ 好了，你的桌面焕然一新」 with three elements: a "去看看桌面"
   nudge, a persistent 「不满意？一键还原」 link, and 「保存对比图」 (share hook).
   The app expects the user to leave — completion is the goal, not retention.
7. **Restore.** The restore entry is visible on the main screen at all times, in
   every state. One click → one plain confirmation → full restore to the latest
   snapshot with a calm settle-back animation. Restore is a safety belt, not a
   center: no version list in the primary UI.

### Trust flow for privileged work (arrow removal)

- Explain before elevating; never elevate at launch.
- Batch all privileged steps into one helper invocation → exactly one UAC prompt.
- Denial (ERROR_CANCELLED) or policy block is not an error state: apply all
  non-privileged styling anyway, tell the user the arrow step was skipped, and keep
  a "再试一次" entry. Never dead-end.
- Explorer refresh is communicated before it happens ("桌面会闪一下，约 2 秒，
  打开的窗口和文件不会丢") with light refresh attempted before any disruptive one.
  No silent Explorer kill.

### UI language rules

- The UI speaks the user's Chinese, never the engineer's. Banned in any user-facing
  string: 快照、应用计划、扫描、dry-run、操作步骤、注册表、缓存、HKLM、journal,
  and raw enum identifiers.
- Domain enums (`DesktopItemKind`, `DesktopItemState`) are never bound directly to
  the UI; a presentation mapper translates them to localized plain language with a
  semantic status (styled / skipped / needs-attention).
- Skipped items are visible with human reasons ("这是系统回收站，Windows 不允许改")
  behind a low-key details entry, not as noise on the main stage.
- Errors follow three parts: what happened / what changed or did not change / what
  the user can do next. Technical details live behind a details expander.

## Information Architecture

One main screen. No navigation.

- **Main screen**: hero before/after, primary action, status line, icon tile grid,
  ever-present restore link, skipped-items details entry.
- **Settings flyout** (gear, top corner): theme (dark default / light / follow
  system), language, arrow-removal toggle, regular-file wrapping opt-in, backup
  location, diagnostics export.
- **Completion state**: rendered in place on the main screen, not a separate page.

## System Architecture

### App.UI

The WPF UI runs without administrator permission. It owns the single main screen,
theming, preview presentation, localization, consent, progress display, and error
messaging. It renders both original and styled images for every item
(press-to-peek requires both).

### Core.Domain

Pure domain models:

- `DesktopItem`
- `IconSource`
- `IconStylePlan`
- `OperationPlan`
- `Snapshot`
- `UndoRecord`
- `OperationResult`

This layer must not depend on Win32, COM, registry APIs, WPF, or file-system side
effects.

### Shell.Adapters

Windows integration lives behind adapters:

- shortcut read/write
- `.url` read/write
- AppX/UWP icon resolution
- Recycle Bin icon state
- folder `desktop.ini`
- regular-file wrapper shortcuts (opt-in only)
- Desktop and Public Desktop discovery, OneDrive desktop detection
- Explorer refresh (light `SHChangeNotify` first; disruptive refresh only with
  communication)
- privileged registry and file-system operations via helper

### Icon.Rendering

Icon rendering owns:

- source image extraction and normalization
- continuous-corner (superellipse) mask generation
- automatic background classification (white tile / preserved background / clipped)
- multi-size `.ico` output selected against real per-monitor DPI
- preview PNG output (original + styled pairs)
- render cache
- future `IIconGenerator` extension for AI or style-pack generation

### Layout.Engine

Layout handling owns desktop icon coordinate snapshots and restore attempts. It runs
invisibly around every mutating operation. Restore is best-effort: it validates that
monitor topology, DPI, and auto-arrange state still match the snapshot before
attempting position restore, and degrades honestly when they do not.

### Operations.Engine

Operations are transaction-like:

1. Build a dry-run plan.
2. Validate prerequisites.
3. Create a snapshot.
4. Execute steps.
5. Write undo records as each step succeeds.
6. Verify changed items where possible.
7. Commit or rollback completed steps on failure.

No mutating operation may run without a snapshot unless it is itself a restore
operation. Snapshots are always created automatically by the engine, never as a
user-facing action.

### Elevated.Helper

Privileged work runs in a separate helper process with a `requireAdministrator`
manifest. The helper must not accept arbitrary commands or run scripts. It exposes
only fixed, whitelisted operations: applying or restoring the shortcut overlay
state, refreshing the Explorer icon cache when elevation is required, and modifying
protected desktop locations. All privileged steps for one apply are batched into a
single invocation (one UAC prompt). The transparent overlay `.ico` is installed to
`%ProgramData%\DeskMakeover\` (a stable path that survives app moves), and the
original registry state is captured in the snapshot before any write. The helper
exits after the requested operation.

## Snapshot And Restore Requirements

Each apply creates an immutable snapshot containing:

- app version and style version
- OS version and build
- current user SID
- Desktop, Public Desktop, and detected OneDrive desktop paths
- display topology and DPI summary
- original shortcut icon locations
- original `.url` contents or icon fields
- original folder `desktop.ini` bytes and attributes
- original Recycle Bin icon registry state
- original shortcut overlay registry state
- regular-file wrapper mapping and original attributes
- layout coordinates where available
- operation journal with completed steps

Restore must support:

- whole-desktop restore (the one-click restore path)
- single-item restore where the snapshot contains enough information (data model
  capability; not exposed as MVP UI)
- emergency restore path if the main UI cannot complete a normal restore

## Safety Rules

- No snapshot, no apply.
- No silent wrapping of regular files; wrapping is opt-in via settings.
- No destructive file deletion during apply.
- No hidden network access in MVP.
- No default Explorer kill. Light refresh first; disruptive refresh only after
  telling the user what will happen.
- OneDrive-synced items: warn that changes sync to the cloud; offline placeholder
  files are skipped, never silently hydrated.
- If restore state is uncertain, stop and show a recovery path instead of guessing.
- Skipped items must be visible to the user with plain-language reasons.

## Localization

MVP requires English and Simplified Chinese; the UI follows the system language by
default. UI strings, status text, error messages, user-facing logs, and
installer-facing text must be resource-backed. Layout must allow text expansion.

Engineering docs and code identifiers use English.

## Error Handling

Errors must be presented with:

1. what happened
2. what changed or did not change
3. what the user can do next

Technical details and error codes belong behind a details expander or diagnostics
export.

Examples:

- Backup failure stops the apply operation before any desktop change.
- UAC denial leaves non-privileged styling applied, the arrow step skipped and
  retryable, and the message free of blame.
- Partial apply failure rolls back completed steps where possible and surfaces the
  snapshot used for recovery.
- Unsupported items are skipped without changing their source files.

## Distribution Trust (release-phase, tracked)

- Authenticode code signing (EV preferred) for the app and helper is required before
  public distribution; budget and signing entity are an open question in STATE.md.
- Antivirus false-positive mitigation: avoid rapid Explorer kill/restart loops,
  clear publisher metadata, pre-submission to Microsoft/360/火绒 whitelists, and an
  in-UI plain-language note when security software may prompt.

## Verification Strategy

Core logic must be tested before broad UI work:

- icon background classification
- continuous-corner mask generation
- `.ico` multi-size output
- operation plan generation
- journaled apply and rollback semantics
- snapshot serialization and restore mapping
- enum-to-presentation mapping coverage (no raw enum leaks)
- localization resource coverage

Shell behavior needs manual and automated matrix testing on clean VMs:

- Windows 10 and Windows 11
- single monitor and multi-monitor, mixed DPI
- OneDrive Desktop enabled and disabled
- auto-arrange and align-to-grid modes
- Desktop and Public Desktop items
- `.lnk`, `.url`, folders, AppX/UWP, Recycle Bin, and regular files
- apply and restore loops, including UAC denial mid-flow
- interrupted apply recovery

## Build Order (post-foundation)

The foundation (solution skeleton, domain, rendering primitives, read-only scanning,
snapshot persistence, dry-run planning, COM shortcut adapters, journaled
non-privileged operations) is complete. Remaining order:

1. Platform foundation: app.manifest (PerMonitorV2 DPI, supportedOS, long paths,
   UTF-8), Fluent theme base, dark-first theming.
2. Visual system: design tokens, squircle controls, typography
   (per specs/02-visual-language.md).
3. Presentation model: original+styled image pairs, humanized status mapping.
4. Main screen v2: mirror, one button, restore link, settings flyout.
5. One-click orchestration: auto-scan → invisible snapshot → non-privileged apply.
6. Elevated helper: arrow overlay state machine with single-UAC batching.
7. Motion: bloom wave, press-to-peek, restore settle, reduce-motion fallbacks.
8. Layout save/restore wiring around mutations.
9. Installer and emergency restore entry point.
