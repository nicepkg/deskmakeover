---
updated: 2026-07-07 (overnight)
version: v1.0.0 (parity rebuild complete; live-desktop run = owner-supervised gate)
branch: main
---

# State

## Active Work

### 2026-07-07 v1.1: rail + wallpaper module (owner directive; IN PROGRESS)

Owner ordered: unlock the module rail (图标/壁纸 + bottom 设置; title bar sheds
⚙/⋯, overflow merges into the drawer) and build 美化桌面壁纸 1.0 (clarity
enhancement + partition zones baked into wallpaper, visual editor on the mirror
canvas). A 4-expert panel (Norman/PM/designer/codex) reviewed; owner's calls:
2-char rail labels · NO icon auto-placement in v1.1 · NO watermark · bundled
handwritten title font (ZCOOL KuaiLe, OFL, in Assets/fonts). Governing docs:
**ADR-0009**, **spec 03**, **spec 04**, plan
**`docs/plans/2026-07-07-rail-and-wallpaper.md`** (phases R, W1-W5).
Roadmap renumbered: 净化 → v1.2, Explorer → v1.3.

**Progress (all live-verified via UIA captures):** R done (rail + drawer
consolidation, Ctrl+1/2, 284→ tests green). W1 done (`DesktopWallpaperInterop`
16-slot vtable + detached-monitor tolerance; `WallpaperSnapshotService` full-state
backup/apply/restore, unreadable-anchor never overwritten). W2 done (pure
`WallpaperBakeRenderer`: clarity gradient/vignette/label-halos + zone panels;
`WallpaperClarityAdvisor` pale detection on label rows). W3+W4 done (zone editor
on the mirror: rubber-band create / snap move / 8-handle resize / inline rename /
Del+arrows / grid guide / snap pulse; panel with 清晰度三档+高级 fold, zone list,
CTA state machine, fingerprint-mismatch banner, coach mark; compose preview ==
bake buffer via `WallpaperComposer`). Owner visual round (2026-07-07): title band
= own cell row + divider (icons never cover the title), ghost placeholder tiles
(editor-only), radius ≤16, FOUR styles (磨砂白/半透明黑/壁纸色/同色边框, all from
the wallpaper's palette), 壁纸 module defaults to fit-all view. Remaining: W5
(adversarial review + owner-supervised live apply/restore — the wallpaper apply
is NEVER auto-triggered).

### 2026-07-07 overnight wave (owner directive; autonomous)

Supersedes older notes where they disagree:

- **Compare/peek = LITERAL desktop pixels**: `DesktopLookSource` rips the BASE icon
  from the shell image list (`SHGetImageList` SHIL matched to the desktop icon size +
  `IImageList::GetIcon`, Recycle Bin via PIDL); the shortcut arrow is composited on
  top by `ShortcutMarkRenderer.DrawClassicArrow` at the ONE owner-approved ratio
  `StyledArrowScale = 0.65` (same constant for styled tiles, originals, and the
  Keep-mode custom overlay the bake installs via the elevated helper's `custom` style).
- **WYSIWYG pipeline**: every tile (styled AND original) is COMPOSED at 256 (the bake
  size) and downscaled to the exact display device-pixel size with the same
  linear-light area resampler that builds the .ico ladder (`IconResampler`); the WPF
  compositor never resamples our bitmaps (its transform-path scaling bypasses Fant).
  sRGB↔linear conversion lives in `SrgbLinear`.
- **Windows-faithful grid**: `IFolderView::GetSpacing` (FolderViewInterop slot 10)
  supplies the real cell; mirror cell = icon + constant padding; labels use the real
  icon-title font (`ScreenMetrics.IconTitleFontPx`, SPI) with a 2-line ellipsis clamp;
  positions map to grid indices at the desktop's current cell and re-lay-out at the
  selected size, overflow reflowing column-major (`AssignPositions`).
- ~~MacBook shell~~ — built, shipped, then REMOVED on owner order (2026-07-07 morning):
  the preview is back to the flat radius-18 dark card. Keep it that way unless the
  owner asks again (the parametric shell lives in git history, commit a5a0e21^).
- Mirror opens at the desktop's REAL icon size + grid; canvas toolbar is one frosted
  bar; Space = hold-to-compare unconditionally (text inputs excepted).

### 2026-07-07 wave (owner-feedback driven; all live-verified)

Current truth for reviewers — supersedes any older notes below that disagree:

- **Shapes** (`IconShapeGeometry`): Apple = TRUE iOS continuous-corner squircle
  (r=0.225·S, three cubic béziers per corner — NOT the earlier Lamé superellipse
  mentioned in the P2 note) · Circle = exact · Samsung = official One UI path.
- **Composition** (`TileRenderer.ComposeTile` + `IconSilhouetteClassifier`):
  1. silhouette already IS the target shape (IoU ≥ .985) → pass-through (no
     plate, no clip, normalized + marks only);
  2. uniform plate detected (`IconBackgroundAnalyzer`) → rebuild: target shape
     filled with the icon's own bg + its logo (`ForegroundBounds`) centred at
     original proportion;
  3. bare logo on transparency → white tile + 42/256 padded content
     (reference `StyleBitmap` parity);
  4. gradient/photo art → full-bleed clip on squircles, but 纯圆 inscribes on a
     white plate at `MaxScaleInside` (never crops content).
- **AA**: `RasterOps.ShapeMask` = corner-grid classification + 16×16 coverage on
  boundary pixels (257 alpha levels), cached per (shape,size,offset); carving
  marks clone before mutating. `TileRenderer.DrawScaled` = 2×2 supersampled
  bilinear. Per-artwork analysis memoized in `IconAnalysisCache` (weak table).
- **Restyle pipeline**: PLINQ parallel; 「正在更新预览…」 cue clears only when the
  winner restyle lands (no timer).
- **Styling coverage == bake coverage** (`DesktopBakeService.CanStyle`):
  shortcuts (.lnk IconLocation) · .url · folders (desktop.ini) · loose files
  (wrapper .lnk + hidden original, reference `-WrapFilesAsShortcuts` parity) ·
  Recycle Bin (per-user CLSID DefaultIcon, empty+full state icons). AppX only
  stays untouched. All journaled, zero-residue restore.
- **Canvas**: fit-height = 100% default, hidden scrollbars, drag-pan,
  Ctrl+wheel zoom at pointer, Ctrl+=/−/0, ⤢ whole-desktop, ↻ re-scan; Space =
  hold-to-compare; radius-18 flat card (no outer shadow); CTA carries the
  prototype dm-glow. Maximize respects the work area (WM_GETMINMAXINFO).

- **v1.0 prototype-parity rebuild** (ADR-0008): the owner rejected the iterated
  UI and produced a complete Claude Design prototype —
  `docs/references/prototype/桌面美颜 v2.dc.html` — which is now the **binding
  UI/UX contract**. All docs were rewritten against it (2026-07-06):
  specs 00/01/02, ADR-0008 (+ status amendments to 0005/0006/0007), READMEs.
- Execution plan for a separate AI:
  **`docs/plans/2026-07-06-v1-prototype-parity.md`** — phased, task-level,
  with prototype line references and per-phase acceptance gates. Start there.

### Rebuild progress (execution log)

- **P0 done (2026-07-06)**: baseline `dotnet build` = 0 warn / 0 err; full suite
  green (118 tests: Core 7 · Shell 26 · Operations 15 · IconRendering 28 · App 42).
  Whole prototype (1724 lines HTML + support.js runtime) read end-to-end — every
  metric/colour/algorithm/copy string extracted. Prototype runtime pulls
  React/Babel from unpkg CDN, so parity is **code-driven** (deterministic from
  source) + verified against WPF screenshots via the new
  `scripts/dev/capture-window.ps1` win-only capture helper. Before-shot of the
  rejected UI captured.
- **P1 done (2026-07-06)**: `Tokens.Dark/Light.xaml` rewritten to prototype-exact
  CSS variables (spec-02) + new tokens (Chip base, AccentInk, Cta.OnAccent, teal/
  amber bg, precomputed washes: chip-selected accent@.17, preset-selected 15% mix,
  seat 16%). accentInk light = mix(accent 70%,#40140C)=#C65445 precomputed. Dropped
  unused blue-ish AccentGlow gradient. ThemeManager already live-swaps (3 modes).
  New `DesignTokensTests` harness loads both compiled dicts via pack URI on STA and
  asserts every value. Build 0/0; App tests 42→46; grep gate empty.
- **P2 done (2026-07-06)**: canonical v1.0 axis model in Core — `StyleConfig`
  (7 axes, serializable) + enums (IconShape/ColorMode/Distinction/MarkStyle/
  IconSizeMode) + `NamedStyles` (4 presets, activePreset/dirty parity) + size maps
  (52/64/76 preview · 32/48/96 desktop). Exact `clipFor` shape engine
  `IconShapeGeometry` (Apple analytic quintic superellipse · Circle exact · Samsung
  official One UI cubic-bézier path) — single source shared by raster mask and the
  new `SquircleGeometry.CreateIconShapePath` (WPF preview). Retired
  ContinuousCornerMask + the Samsung superellipse(.40,4) approximation; IconStyler
  rewired. Golden tests: superellipse eq, corner≈22.1%, One UI bézier ±1px, circle
  exact, mirror symmetry, raster↔XAML 8-angle agreement. 118→152 tests, 0 warn.
  NOTE: old MaskShape/ColorTreatment/StyleCombo model still used by the not-yet-
  rebuilt App/pipeline; both coexist until the App rebuild (P5–P9) deletes the old.
- **P3 done**: `IconColorTreatment` pure module — exact prototype lum/hsl/grayOf/
  styledFor + per-pixel TransformPixel + tests (WCAG ≥3:1 adaptive modes incl
  纯黑/纯白, bw bounds, mono hue==tint).
- **P4 done (subagent-built, reviewed)**: unified `TileRenderer.Render(artwork,
  StyleConfig, TileKind, isShortcut, showOriginal, size)` — the ONE fn preview+bake
  share. `Marks/`: RasterOps + MarkContext + IMark + 7 styles + classic arrow.
  Harness PNG verified: all 7 marks distinct/adaptive/clipped, 16px legible.
- **P5 done**: custom-titlebar window shell + 300px panel + radius-14 canvas +
  compact overlay (<1100px) + Esc order + neutral DWM border (no system blue).
  New lean MainViewModel; TitleBar/ControlPanel(stub)/Canvas(stub) views.
- **P10 done (subagent-built, reviewed; LIVE-VERIFIED)**: IFolderView2 icon size
  adapter, graceful-degrade, vtable slots tallied vs MSDN, mapping tests. The subagent
  ran it on the real desktop: 48→96→48 round-trip, restored — behavioural proof the
  vtable slot count is exact.
- **P11 done**: settings drawer + overflow + About + changelog — verified in dark
  AND light with live theme switch. (导出/保存 open data folder; P9 wires exact export.)
- **P9(data) done**: LookVersion + LookHistoryStore (capped ledger, corruption-tolerant).
- Build 0/0; **203 tests** green (Core 26 · Ops 20 · Shell 51 · App 49 · IconRendering 57).
- Two subagents (marks-p4, iconsize-p10) built P4+P10 in parallel; both reviewed + green.
- **P6 done**: control panel — hero/CTA 5-state machine, 4 preset cards with live
  minis, 4-row accordion (shape swatches + 7 live mark chips + swatch rows), ~420ms
  restyle funnel. Verified default + expanded, dark.
- **P8 done**: desktop-mirror canvas — real wallpaper, real desktop icons (23) in
  column-major order, apple-squircle + glass marks, decorative taskbar (start/search/
  apps/live clock), compare pill, press-to-peek, right-click override menu. FIX:
  shortcuts without icon-location now extract their TARGET's clean icon (real app
  icons, not the generic monitor). Apply/dirty/restore/history state machine works
  (preview flow). 单色 recolours all tiles live via the restyle funnel.
- **P7 done**: shared 调色盘 picker wired to both consumers (图标单色 / 标识配色),
  routes through the restyle funnel; reuses the existing HSV ColorPickerControl.
- **The prototype-parity PREVIEW is complete + visually verified across all states**
  (the prototype is itself a preview simulation; this replicates it fully).
- **P12 done**: bloom (apply) / settle (restore) staggered tile waves + reduced-motion;
  drawer/panel/overflow/about/toast pops via MotionTokens.
- **P6.6 done**: compact summary toolbar (preset chips + 自定义 ▸ + compact CTA).
- **P9 done**: `DesktopBakeService` — real desktop bake reusing the tested foundation
  (snapshot → journaled per-icon .ico via TileRenderer → transparent overlay → live
  icon size) + zero-residue restore; wired to the CTA; fixture tests byte-identical.
- **P13 done**: foundation re-verified green via automated fixtures (evidence in
  `docs/plans/evidence/2026-07-parity/audit.md`). The live switch-on run stays the one
  owner-supervised gate (never auto-triggered) — unchanged from before the rebuild.
- **P14 done**: parity audit sheet + evidence screenshots (`.../evidence/2026-07-parity/`),
  version 1.0.0, CHANGELOG.md. **Codex adversarial review APPROVED after 6 rounds** — it
  Rejected 5 times, surfacing 20+ real reversibility / preview==desktop bugs I'd missed
  (dirty-reapply losing the true original, restore deleting the anchor without verifying,
  per-icon overrides not baked, non-atomic state, double-arrow, MTA icon-size, off-thread
  restyle, history-not-baking, rollback-failure anchor deletion, 3-way gate split, …). All
  fixed + regression-tested. Round 6: no block/major/blocking-minor. Build 0/0, 216 tests.
- **Canvas upgraded to an equal-scale desktop mirror** (Boss feedback): real wallpaper,
  true screen aspect ratio (Viewbox over real px), proportional icons, white Win11 taskbar,
  folders/files shown untouched, styleable count.

### Status: **all 14 rebuild phases complete.** 208 tests green, 0 warnings, grep gate
empty. The v1.0 prototype-parity app is built + visually verified across every state
(regular/compact, dark/light, all overlays, all interactions). Known follow-up: the old
MakeoverService/IconStyler/OverlayBadgeIconFactory/StylePreset(old) + their tests are
now dead-but-still-tested legacy the rebuilt app no longer uses — a safe deletion pass
(remove code + tests together) is queued for after the codex review.

### Owner gates (unchanged): OV/individual signing cert · public github repo · fresh-VM
smoke · supervised live switch-on run.

## Owner rules (durable)

- Accent is warm coral `#FF6F5E`; **never blue/violet** (reads as AI slop).
- Extreme DRY; files ≤500 lines; aesthetic calls go through VOC + panel + the
  3-second-misread gate, not engineer fiat.
- The prototype wins every conflict with prose docs; when unsure, open it in a
  browser and compare (`open "docs/references/prototype/桌面美颜 v2.dc.html"`).
- v1.0 adds **no functional territory beyond icon beautification** — the
  future-form modules (净化/文管/壁纸) are design-locked IA for v1.1+.

## What exists and works (keep, don't rebuild)

- Foundation: domain models, scanner, snapshots, journaled ops, elevated helper
  with single-UAC batching, keep-up (logon task + catch-up), layout best-effort
  save/restore. ~105 tests green; `dotnet build` 0 warnings.
- Rendering: superellipse mask engine, background classifier, `IsRoundish`,
  hi-res 256px extraction, multi-size ico ladder (16–256, premultiplied
  box-average resampler), per-icon mark bake + transparent global overlay
  (ADR-0006 facts), GeneratedIconStore.
- Wallpaper colour extraction, HSV ColorPicker component (SV+hue+hex+eyedropper),
  comparison-image exporter, theme manager (dark/light/system), toast/dialog
  chrome, reduced-motion awareness.
- The current UI shell/layout/controls are **superseded** by the prototype and
  will be rebuilt per the plan (keep the services/pipeline underneath).

## Next

1. Execute the plan phases in order (docs/plans/2026-07-06-v1-prototype-parity.md),
   each phase gated by its acceptance checks + screenshot evidence.
2. Prototype parity audit (plan P14) before any release claim.
3. Owner actions: purchase OV/individual signing cert; create public
   `github.com/nicepkg/deskmakeover` repo (About panel links to it).
4. Supervised live switch-on → UAC → switch-off run on the owner's machine —
   still the one path never exercised end-to-end.

## Blockers

- None for development. Release gates: signing cert (owner), public repo (owner).
- GitHub CLI not in PATH; no git remote configured yet.

## Open Questions

- Signing entity/name for the OV certificate (owner decision, purchase pending).
- v1.0 distribution channel details (direct download + pinned comment reply).

## History (compressed)

- 2026-07-05: MVP foundation (60→105 tests), product shell v1, ADR-0001..0004.
- 2026-07-06 rounds 1–6: composable shape×colour×distinction engine, ColorPicker,
  responsive grid, multi-size ico, real-desktop mark bake, codex adversarial
  review (10 findings fixed), three-band layout, distinction iterations
  (arc→arrow→stacked cards→gallery) — owner rejected each UI round.
- 2026-07-06 (this session): owner shipped the v2 prototype; ADR-0008; full doc
  realignment; v0.9→v1.0 renumbering; big rebuild plan written.
