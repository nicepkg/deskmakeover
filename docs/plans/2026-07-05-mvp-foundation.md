# DeskMakeover MVP Foundation Implementation Plan

> **Status:** ✅ EXECUTED — historical build record (the early .NET/WPF foundation; the whole stack
> was later replatformed to Tauri 2 + Rust, ADR-0019). See `docs/journal/2026-07.md`.

**Goal:** Build the first runnable DeskMakeover foundation: local git repo, .NET solution, tested domain and rendering core, read-only desktop scanning, WPF preview shell, and snapshot/restore scaffolding.
**Architecture:** Keep Windows Shell side effects behind adapters and keep the WPF UI non-admin. Implement core logic first with tests, then wire a read-only preview UI before privileged apply behavior.
**Tech stack:** .NET 10 LTS, WPF, xUnit, System.Drawing/Common-compatible image rendering, Windows Shell adapters.

---

## File Map

- `DeskMakeover.slnx` or `DeskMakeover.sln`: solution file.
- `src/DeskMakeover.App/`: WPF app, localization resources, view models, preview UI.
- `src/DeskMakeover.Core/`: pure domain models, operation plans, snapshots, journal contracts.
- `src/DeskMakeover.IconRendering/`: icon image processing, background classification, continuous-corner rendering, `.ico` writer.
- `src/DeskMakeover.Shell/`: Windows desktop scanning and Shell adapters.
- `src/DeskMakeover.Operations/`: apply/restore orchestration, dry-run plans, snapshot persistence.
- `src/DeskMakeover.ElevatedHelper/`: privileged helper shell, initially command-whitelisted but side-effect-light.
- `tests/DeskMakeover.Core.Tests/`: domain and snapshot tests.
- `tests/DeskMakeover.IconRendering.Tests/`: rendering and `.ico` tests.
- `tests/DeskMakeover.Operations.Tests/`: journal and rollback behavior tests.
- `docs/STATE.md`: checkpoint updates.
- `.gitignore`, `README.md`, `global.json`, `Directory.Build.props`: repo setup.

## Tasks

### Task 1: Repository And Toolchain Bootstrap

**Files:**
- Create: `.gitignore`
- Create: `README.md`
- Create: `global.json`
- Create: `Directory.Build.props`

- [ ] Initialize local git repository.
- [ ] Configure ignores for build artifacts, local SDK, user settings, logs, generated icons, and test outputs.
- [ ] Install or configure a local .NET 10 SDK without requiring global machine changes.
- [ ] Commit approved docs and repository bootstrap.
- [ ] Verify with `git status --short`.

### Task 2: Solution Skeleton

**Files:**
- Create: solution file
- Create: project files under `src/` and `tests/`

- [ ] Create class library projects for Core, IconRendering, Shell, and Operations.
- [ ] Create WPF app project for App.
- [ ] Create console/helper project for ElevatedHelper.
- [ ] Create xUnit test projects.
- [ ] Add project references following the architecture boundaries.
- [ ] Build the solution.

### Task 3: Domain And Snapshot Core

**Files:**
- Create: `src/DeskMakeover.Core/DesktopItem.cs`
- Create: `src/DeskMakeover.Core/IconSource.cs`
- Create: `src/DeskMakeover.Core/IconStylePlan.cs`
- Create: `src/DeskMakeover.Core/Operations/*.cs`
- Create: `src/DeskMakeover.Core/Snapshots/*.cs`
- Create: `tests/DeskMakeover.Core.Tests/*.cs`

- [ ] Add immutable domain models for desktop items, icon sources, style plans, operation plans, snapshots, and undo records.
- [ ] Write tests for snapshot serialization round-trip and no-snapshot/no-apply rule.
- [ ] Run tests and build.

### Task 4: Icon Rendering Core

**Files:**
- Create: `src/DeskMakeover.IconRendering/*.cs`
- Create: `tests/DeskMakeover.IconRendering.Tests/*.cs`

- [ ] Implement background classification for transparent, solid-edge, border-like, and opaque icons.
- [ ] Implement continuous-corner mask rendering.
- [ ] Implement PNG preview output and multi-size ICO writer.
- [ ] Test classifier and ICO structure.
- [ ] Run tests and build.

### Task 5: Read-Only Desktop Scanner

**Files:**
- Create: `src/DeskMakeover.Shell/DesktopScanner.cs`
- Create: `src/DeskMakeover.Shell/DesktopPaths.cs`
- Create: `src/DeskMakeover.Shell/ShortcutIconReader.cs`
- Create: `src/DeskMakeover.Shell/UrlShortcutReader.cs`

- [ ] Discover user Desktop and Public Desktop.
- [ ] Return desktop items without mutating the filesystem.
- [ ] Recognize `.lnk`, `.url`, folders, and regular files.
- [ ] Add safe fallback statuses for unsupported or inaccessible items.
- [ ] Build and add focused tests where shell behavior can be isolated.

### Task 6: WPF Preview Shell

**Files:**
- Create: `src/DeskMakeover.App/MainWindow.xaml`
- Create: `src/DeskMakeover.App/MainWindow.xaml.cs`
- Create: `src/DeskMakeover.App/ViewModels/*.cs`
- Create: `src/DeskMakeover.App/Resources/Strings*.resx`

- [ ] Build a simple first-screen shell: scan, preview grid, status summary, apply disabled until snapshot support is wired.
- [ ] Add English and Simplified Chinese resource files.
- [ ] Show skipped/unsupported items in plain language.
- [ ] Build the WPF app.

### Task 7: Operations And Snapshot Persistence

**Files:**
- Create: `src/DeskMakeover.Operations/*.cs`
- Create: `tests/DeskMakeover.Operations.Tests/*.cs`

- [ ] Implement dry-run operation plan.
- [ ] Persist versioned snapshots under the user's local app data.
- [ ] Implement journal contract and rollback model without destructive shell writes.
- [ ] Test partial failure rollback semantics with fake operations.

### Task 8: Checkpoint Verification

**Files:**
- Modify: `docs/STATE.md`

- [ ] Run full build and test commands in a fresh terminal context.
- [ ] Update `docs/STATE.md` with completed work and next step.
- [ ] Commit all verified changes locally.

