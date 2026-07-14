# Plan — v1.1: Module Rail + 美化桌面壁纸 1.0

> **Status:** ✅ EXECUTED — historical build record (the rail + wallpaper 1.0; the wallpaper module
> was later rebuilt per ADR-0014). See `docs/journal/2026-07.md`.

Executes ADR-0009 / spec 03 / spec 04. Phases are gated; each ends with fresh
verification evidence (build 0/0, suite green, live screenshots via
`scripts/dev/capture-window.ps1` + UIA where interaction is claimed). Prototype
stays the visual contract for the rail; the reference shots in
`C:\Users\yangxiaomingwin\Pictures\壁纸分区` are the zone look benchmark.

## R — Rail + settings consolidation (spec 03)  ~0.5-1d

- R1. `MainViewModel`: `ActiveModule` (Icons/Paper) + `SetModuleCommand` +
  Ctrl+1/2 bindings; module switch preserves both modules' VM state.
- R2. `MainWindow.xaml`: insert 66px rail column (new `Views/ModuleRailView.xaml`,
  prototype L61-69 metrics: 40px glyph tile radius 13 / 9.5px label / accent 16%
  wash / dashed future slot / bottom 设置 entry); body panel+canvas shift right.
- R3. TitleBar: delete ⚙/⋯ buttons + handlers; delete `OverflowMenuView.*`;
  MainWindow overlay host loses the overflow popup wiring.
- R4. SettingsDrawer: add 检查更新/联系反馈 rows to the 关于 group (reuse
  handlers); resx moves per spec 03 §4.
- R5. Panel headers: icons panel gains title 「美化图标」 (spec 03 full-name rule);
  compact summary toolbar hidden when Paper active.
- Gate: spec-03 acceptance list — screenshots dark+light, Esc order, Ctrl+1/2,
  all four drawer actions live; suite green.

## W1 — Wallpaper read/write + snapshot foundation  ~1-2d

- W1a. `DesktopWallpaperInterop` (Shell): IDesktopWallpaper CoClass wrapper —
  Get/SetWallpaper per monitor, position, background colour, slideshow
  get/set/status. Vtable-audited like FolderViewInterop (MSDN order test).
- W1b. `WallpaperSnapshotService` (Operations): capture full state + byte-copy of
  primary source → `wallpaper\backup\`; `RestoreAsync` with keep-anchor-on-failure
  semantics; corruption-tolerant like LookHistoryStore. Fixture tests (fake
  interop) for round-trip, partial-failure, double-apply idempotence.
- W1c. `WallpaperConfig` (Core): `ZoneRect` (cells), `ZoneStyleKind`,
  `ClarityLevel`, `EnvironmentFingerprint` records + JSON store; fingerprint
  compute from ScreenMetrics + grid metrics + file hash. Unit tests.
- Gate: fixture suite green; no UI yet.

## W2 — WallpaperBakeRenderer (pure, preview==bake)  ~2-3d

- W2a. Source load + linear-light area resample to native px (reuse
  SrgbLinear/IconResampler); gradient dim / vignette / label-halo layers.
- W2b. Zone painting: region gaussian blur (σ=cell/6), 磨砂白/壁纸色 fills per
  spec 04 §3, radius/stroke, handwritten-font title rasterisation (bundle font
  asset + licence; GlyphTypeface render into the Rgba32 buffer).
- W2c. Pale-wallpaper detector: luminance P50 under real label rows
  (DesktopLayoutReader positions) > 0.72 → recommend 柔和.
- Tests: golden-ish assertions (blur region stats, fill alpha, title pixels,
  clarity gradient monotonicity, detector thresholds on synthetic wallpapers).
- Gate: harness PNGs eyeballed vs reference shots; suite green.

## W3 — Zone editor on the mirror canvas  ~2-3d

- W3a. `WallpaperViewModel` (App): zones collection, selection, clarity state,
  dirty/CTA 5-state machine, zone list for the panel, recommended-layout +
  add-zone commands, coach-mark once-flag.
- W3b. Canvas overlay: zone adorner layer in DesktopCanvasView (Paper mode only)
  — rubber-band create, 8 snap handles, coral dashed alignment lines, 80ms snap
  pulse (reduced-motion aware), inline rename, Del/arrow keys; preview composes
  through the SAME renderer output downscaled (mirror keeps real icons on top).
- W3c. Empty state + [用推荐布局] (3 zones per reference layout).
- Gate: UIA-driven interaction screenshots (create/resize/rename/delete);
  preview==bake sample diff test; suite green.

## W4 — Panel view + apply flow + fingerprint guard  ~1-2d

- W4a. `WallpaperPanelView.xaml`: spec 04 §2 layout (status/hero/CTA/清晰度
  segmented + 高级 fold/分区 list/footer), styles reuse existing tokens/chips.
- W4b. Apply: bake native → snapshot (first time) → SetWallpaper+Fill → toast
  「壁纸已应用 · 原壁纸已备份」; 换回我的壁纸; slideshow pause/restore honesty.
- W4c. Fingerprint mismatch banner + 重新合成; coach mark overlay.
- Gate: spec-04 acceptance except live-desktop items; suite green, 0 warnings.

## W5 — Verify + review + ship gate  ~1d

- Full suite + grep gates; adversarial review (spec-compliance then quality) via
  independent reviewer; live supervised apply/restore on the owner's desktop
  (owner-gated, never auto); STATE/CHANGELOG/roadmap updates.

Standing rules: files ≤500 lines; every service behind an interface for fixture
tests; no new colours outside tokens; all user-facing copy zh-Hans first.
