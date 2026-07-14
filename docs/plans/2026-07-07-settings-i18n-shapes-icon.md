# Plan — v1.1 Interaction Cleanup: Settings Page, i18n, Icon, Marks, Shapes

> **Status:** ✅ EXECUTED — historical build record (ADR-0010 interaction cleanup). See `docs/journal/2026-07.md`.

**Goal:** Execute ADR-0010: remove dead rail affordances, turn settings into a
normal rail page, default theme/language to system, replace the app icon with a
hand-authored SVG, remove the selectable glass arrow, and expand shape masks.

**Architecture:** Keep the existing WPF shell and rendering pipeline. Settings
becomes a module in `MainViewModel`; the renderer's single `IconShapeGeometry`
service grows new shapes so preview and bake stay identical. Localization remains
resx-backed with runtime culture set before WPF loads localized resources.

**Tech stack:** .NET/WPF, existing resx localization, existing raster renderer,
SVG source asset plus generated PNG/ICO derivatives.

**Global constraints:**
- No real icon bake or wallpaper apply during automated verification.
- All user-facing strings must exist in both neutral English and zh-Hans resx.
- No blue/violet accent; keep coral `#FF6F5E`.
- Files heading over 500 lines must be split instead of stretched.
- Preview and baked icon shape math must share `IconShapeGeometry`.

---

## File Structure

- Modify `src/DeskMakeover.Core/StyleConfig.cs`: append shape enum values and
  retire `MarkStyle.Glass` from selectable UI.
- Modify `src/DeskMakeover.IconRendering/IconShapeGeometry.cs`: add deterministic
  shape outlines and tests.
- Modify `src/DeskMakeover.App/Controls/SquircleGeometry.cs`: consume expanded
  outlines.
- Modify `src/DeskMakeover.App/ViewModels/MainViewModel.cs`, `AppModule.cs`,
  `ShellViewModel.cs`: settings module, language/theme state.
- Replace `SettingsDrawerView` with a settings page view, reusing handlers from
  its code-behind where possible.
- Modify `MainWindow.xaml/.cs` and `ModuleRailView.xaml`: remove future slot and
  drawer overlay; settings is selected module; Ctrl+3.
- Modify `UiText.cs`, `AppSettings.cs`, `ThemeManager.cs`, `App.xaml.cs`: system
  defaults and language preference.
- Modify `Resources/*.resx` through the existing upsert script.
- Create `Assets/app-icon.svg`; regenerate `app-icon.png` and `app.ico` from the
  same hand-authored vector spec.
- Update tests under `tests/DeskMakeover.*.Tests`.

## Tasks

### Task 1: Settings rail module and page

- [x] Add `AppModule.Settings` and Ctrl+3.
- [x] Remove `Rail_FutureSlot` UI.
- [x] Make rail tiles icon-only inside the 40px tile, with localized label below.
- [x] Replace drawer overlay host with settings-page content.
- [x] Ensure Esc no longer has a settings drawer branch; About/changelog remain
      inline or as a deliberate settings-page action.
- [x] Tests: module switching includes settings and preserves wallpaper VM state.

### Task 2: Theme + language defaults

- [x] Change default theme from `Dark` to `System`.
- [x] Add language preference `{System, zh-Hans, en}` to settings.
- [x] Apply culture before resource lookup; unsupported system cultures fall back
      to English.
- [x] Add settings page controls for language and theme, both defaulting to
      System on a clean settings file.
- [x] Tests: clean defaults and resource preference parsing.

### Task 3: Settings visual rebuild

- [x] Recompose settings into grouped cards: 外观, 自动化, 本地数据, 关于.
- [x] Move about/changelog/update/feedback into the page; no modal for normal
      settings navigation.
- [x] Reuse existing export/save/update/feedback handlers.
- [x] Visual check dark/light/system themes.

### Task 4: Hand-authored app icon

- [x] Create `Assets/app-icon.svg` from the hand-authored vector spec.
- [x] Generate `app-icon.png` and `app.ico` from SVG using local tooling.
- [x] Update WPF window icon / about image references to the derived assets.
- [x] Verify the SVG source remains committed and readable.

### Task 5: Shortcut mark cleanup

- [x] Remove `Glass` from selectable mark chips and labels.
- [x] Keep classic arrow as the recommended shortcut distinction option.
- [x] Ensure persisted `Glass` configs degrade to `Arc` or `Keep` without crash.
- [x] Tests: mark chips exclude glass; serialization/degrade path covered.

### Task 6: Shape expansion

- [x] Append Google, Brave, Bookmark, Lemon, Squircle, Tile, Teardrop, Blob,
      Rectellipse to `IconShape`.
- [x] Implement outlines/contains for each in `IconShapeGeometry`.
- [x] Surface all shapes in the shape accordion with localized labels.
- [x] Tests: symmetry/area sanity, no empty mask, WPF/raster agreement.

### Task 7: Verification and review

- [x] Run `dotnet build DeskMakeover.slnx`.
- [x] Run `dotnet test DeskMakeover.slnx`.
- [x] Run grep gate for banned blue/violet literals.
- [x] Launch app and capture screenshots for icons/wallpaper/settings modules.
- [x] Run adversarial spec-compliance and code-quality review; fix findings.
- [x] Update `docs/STATE.md` with final status and evidence.
