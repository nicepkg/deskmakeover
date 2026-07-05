# DeskMakeover MVP Product Architecture

## Product Identity

DeskMakeover is the English product name for "桌面整容大师".

Chinese slogan:

> 一键整容美化 Windows 桌面，随时完整还原。

English slogan:

> Give your Windows desktop a one-click makeover. Restore everything anytime.

The product must feel like a trustworthy desktop makeover tool, not a registry tweak wrapper, script launcher, system cleaner, or macOS imitation.

## Target Users

The MVP targets non-technical Windows users who want a cleaner desktop but do not know or trust PowerShell, registry edits, or manual icon replacement. The product should also work for design-sensitive users with many desktop shortcuts, folders, Store apps, web shortcuts, and ordinary desktop files.

The first release supports Windows 10 and Windows 11. Windows 7 is outside the mainline MVP.

## MVP Scope

The MVP must complete one reliable loop:

1. Scan the current desktop.
2. Show a visual preview of icon changes.
3. Create a reversible snapshot.
4. Apply selected icon styling.
5. Refresh desktop presentation safely.
6. Restore the previous state completely when requested.

The MVP includes:

- Desktop item discovery for user Desktop and Public Desktop.
- Support for `.lnk` shortcuts, `.url` shortcuts, AppX/UWP shortcuts, Recycle Bin, folders, and regular files.
- Regular files are previewed by default. Wrapping a regular file as a styled shortcut requires explicit user confirmation.
- Continuous-corner icon rendering with automatic background handling.
- Shortcut arrow overlay control as an explicit option with clear permission messaging.
- Desktop icon layout snapshot and best-effort restore.
- Versioned restore snapshots.
- English and Simplified Chinese localization.
- Local-only operation with no account, upload, telemetry, or cloud dependency.

The MVP excludes:

- AI icon generation.
- Icon marketplace or community style sharing.
- Wallpaper, taskbar, cursor, widget, or complete theme management.
- Enterprise policy management.
- Windows 7 support.
- Any irreversible system makeover.

## User Experience

The default flow is a guided one-click path:

1. The app starts in normal user mode.
2. It scans the desktop and reports how many items can be styled, skipped, or require confirmation.
3. It shows before-and-after icon previews in a stable grid.
4. It explains any operation that needs administrator permission before showing UAC.
5. It creates a snapshot before applying changes.
6. It applies changes with progress stages: backup, render, apply, refresh, verify.
7. It keeps restore actions visible after completion.

Advanced controls are available but not required for the default path. The default style should be good enough for a non-technical user to accept without tuning.

## Information Architecture

The initial navigation should support these areas:

- Home: desktop status, primary action, last snapshot, quick restore.
- Icons: preview grid, style presets, item filters, per-item status, batch selection.
- Layout: layout snapshot and restore status.
- Restore Center: versioned snapshots, whole-desktop restore, single-item restore, emergency restore notes.
- Settings: language, theme, backup location, diagnostics, and helper permissions.

Future modules such as color filters, style packs, AI generation, wallpaper, or widgets must follow the same safety loop: preview, snapshot, apply, restore.

## System Architecture

### App.UI

The WPF UI runs without administrator permission. It owns navigation, preview presentation, localization, user consent, progress display, and error messaging.

### Core.Domain

Pure domain models:

- `DesktopItem`
- `IconSource`
- `IconStylePlan`
- `OperationPlan`
- `Snapshot`
- `UndoRecord`
- `OperationResult`

This layer must not depend on Win32, COM, registry APIs, WPF, or file-system side effects.

### Shell.Adapters

Windows integration lives behind adapters:

- shortcut read/write
- `.url` read/write
- AppX/UWP icon resolution
- Recycle Bin icon state
- folder `desktop.ini`
- regular-file wrapper shortcuts
- Desktop and Public Desktop discovery
- OneDrive desktop detection
- Explorer refresh
- privileged registry and file-system operations via helper

### Icon.Rendering

Icon rendering owns:

- source image extraction and normalization
- continuous-corner mask generation
- automatic background classification
- white tile, preserved background, and clipped modes
- multi-size `.ico` output
- preview PNG output
- render cache
- future `IIconGenerator` extension for AI or style-pack generation

### Layout.Engine

Layout handling owns desktop icon coordinate snapshots and restore attempts. It must treat layout restore as best-effort because Explorer settings, DPI, monitor topology, auto-arrange mode, and grid alignment can change outside the app.

### Operations.Engine

Operations are transaction-like:

1. Build a dry-run plan.
2. Validate prerequisites.
3. Create a snapshot.
4. Execute steps.
5. Write undo records as each step succeeds.
6. Verify changed items where possible.
7. Commit or rollback completed steps on failure.

No mutating operation may run without a snapshot unless it is itself a restore operation.

### Elevated.Helper

Privileged work runs in a separate helper process. The helper must not accept arbitrary commands or run scripts. It should expose only fixed, whitelisted operations, such as applying or restoring shortcut overlay state, refreshing Explorer icon cache when elevated access is required, and modifying protected desktop locations.

The helper should be invoked only when required and should exit after the requested operation.

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

- whole-desktop restore
- single-item restore where the snapshot contains enough information
- emergency restore path if the main UI cannot complete a normal restore

## Safety Rules

- No snapshot, no apply.
- No silent wrapping of regular files.
- No destructive file deletion during apply.
- No hidden network access in MVP.
- No default Explorer kill. Use light refresh first and ask before disruptive refresh.
- If restore state is uncertain, stop and show a recovery path instead of guessing.
- Skipped items must be visible to the user with plain-language reasons.

## Localization

MVP requires English and Simplified Chinese. UI strings, status text, error messages, logs intended for users, and installer-facing text must be resource-backed. Layout must allow text expansion for future languages.

Engineering docs and code identifiers use English.

## Error Handling

Errors must be presented with:

1. what happened
2. what changed or did not change
3. what the user can do next

Technical details and error codes belong behind a details expander or diagnostics export.

Examples:

- Backup failure stops the apply operation before any desktop change.
- Permission denial leaves the app in preview-only mode or applies only non-privileged steps after user confirmation.
- Partial apply failure rolls back completed steps where possible and surfaces the snapshot used for recovery.
- Unsupported items are skipped without changing their source files.

## Verification Strategy

Core logic must be tested before broad UI work:

- icon background classification
- continuous-corner mask generation
- `.ico` multi-size output
- operation plan generation
- journaled apply and rollback semantics
- snapshot serialization and restore mapping
- localization resource coverage

Shell behavior needs manual and automated matrix testing on clean VMs:

- Windows 10 and Windows 11
- single monitor and multi-monitor
- mixed DPI
- OneDrive Desktop enabled and disabled
- auto-arrange and align-to-grid modes
- Desktop and Public Desktop items
- `.lnk`, `.url`, folders, AppX/UWP, Recycle Bin, and regular files
- apply and restore loops
- interrupted apply recovery

## Initial Build Order

Implementation should proceed in this order after the plan is approved:

1. Solution skeleton and project boundaries.
2. Domain models and snapshot schema.
3. Icon rendering and tests.
4. Desktop scanning adapters with safe read-only behavior.
5. Preview UI.
6. Snapshot and operation journal.
7. Non-privileged apply and restore for user desktop items.
8. Elevated helper for privileged operations.
9. Layout engine.
10. Installer and emergency restore entry point.

