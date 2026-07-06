# Plan: v1.0 Prototype-Parity Rebuild

**Date:** 2026-07-06 · **Governing ADR:** [0008](../decisions/0008-prototype-v2-ui-contract.md)
**Audience:** the executing AI (a fresh session with no prior context). Read this
plan top-to-bottom, then work the phases in order. Do not skim.

---

## 0. Mission and non-negotiables

Rebuild the DeskMakeover UI so that **v1.0 completely replicates the owner's
interactive prototype**, while keeping the tested foundation (scanning, snapshots,
journaled ops, elevated helper, ico pipeline) underneath.

**The contract:**

- `docs/references/prototype/桌面美颜 v2.dc.html` (+ `support.js`) — **the single
  source of truth**. It is a working React-like simulation: every layout metric,
  copy string, colour, interaction, and mark algorithm you need is executable in
  it. **Open it in a browser and interact with it before writing any code.**
  When this plan, the specs, and the prototype disagree → the prototype wins.
- `docs/specs/01-product-architecture.md` — product scope, IA, copy tables.
- `docs/specs/02-visual-language.md` — tokens, metrics, colour/mark math, motion.

**Non-negotiables (owner rules — violating any of these is a rejected build):**

1. **Never blue/violet** anywhere. Accent = warm coral `#FF6F5E` only.
2. **v1.0 = the prototype's 今日形态 (today form) only.** Do NOT build: the 演示控制
   strip (lines 30–39), the module rail (60–71), the 已帮你做的事 checklist
   (122–135), the discover card (137–146), the 净化/文管/壁纸 views (280–378,
   431–509). Those are v1.1+ IA.
3. **Preview == desktop parity law:** the preview tile bitmap and the baked
   desktop `.ico` must come from the same rendering functions.
4. Engineering standards: extreme DRY · files ≤500 lines (split before crossing) ·
   domain enums never bound to XAML · all visible chrome custom (no default WPF
   control faces) · user-facing strings from the resource table only.
5. **Evidence before claims.** Never report a phase done without running its
   verify commands in that turn and (for UI phases) capturing a screenshot and
   comparing it side-by-side with the prototype in the same state.
6. Windows required: build, tests, and all visual verification run on win-x64.
   `dotnet build DeskMakeover.slnx` must stay at **0 warnings / 0 errors**, and
   the full test suite green, at every phase boundary.

**In-app version strings**: v1.0 (title-bar chip 「v1.0」, About 「v1.0.0（2026.07）」).
The prototype's "v0.9 预览" texts are the only strings you intentionally change.

---

## 1. How to read the prototype (line map)

The file has two halves: an HTML template using `{{ bindings }}` +
`sc-if`/`sc-for`, and a `Component` class computing every binding in
`renderVals()`. Key coordinates:

| What | Lines |
|---|---|
| CSS variables (both themes) + keyframes | 12–24 |
| Title bar | 44–56 |
| Control panel: hero + CTA + link chips | 76–96 |
| Version-history card | 98–119 |
| 风格 preset cards | 148–169 |
| 自定义 accordion (外形/配色/标识/大小) | 171–277 |
| Compact summary toolbar | 385–394 |
| Tile grid template | 399–429 |
| Updating cue / compare pill / taskbar | 511–555 |
| Settings drawer | 564–622 |
| 调色盘 popup | 624–658 |
| Overflow menu / About + changelog / tile context menu / toast | 660–761 |
| **Default state (`initState`)** | 778–794 |
| Colour helpers (`lum`, `hsl`, `grayOf`, HSV↔hex) | 846–883 |
| Picker logic + eyedropper | 885–928 |
| Mark chip previews (`markPreview`) | 930–962 |
| Demo icon set (incl. 纯黑/纯白 edge tiles) | 964–989 |
| **Shape masks (`clipFor`) — authoritative** | 991–1020 |
| **Colour treatments (`styledFor`) — authoritative** | 1040–1053 |
| Presets (`presetsDef`, `activePreset`) | 1055–1072 |
| Dirty check (`isDirty`) | 1074–1078 |
| History label grammar (`labelFor`) | 1081–1087 |
| Apply / restore / re-apply version | 1092–1123 |
| Hero/CTA state machine | 1196–1224 |
| Axis chip/swatch definitions | 1247–1289 |
| **Tile composition + the 7 mark algorithms** | 1386–1507 |
| Context menu / drawer / picker / overflow / about bindings | 1514–1716 |

Default state on launch (line 781): `shape:'apple', colorMode:'orig',
tint:'#FF6F5E', dist:'mark', markStyle:'glass', markColor:null (=自动),
sizeMode:'mid', theme:'dark', keepUp:true` → the 苹果极简 preset reads active.

---

## 2. Current codebase map (what stays / changes / dies)

| Area | Verdict |
|---|---|
| `DeskMakeover.Core` (DesktopItem, IconSource, IconStylePlan, StylePreset, OperationPlan, DesktopSnapshot) | **Stays**; StylePreset gains axes (P2), new LookVersion type (P9) |
| `DeskMakeover.Shell` (scanner, COM shortcut IO, `.url`, ExplorerRefresh, RestoreMetadataCollector) | **Stays**; add DesktopIconSize adapter (P10) |
| `DeskMakeover.Operations` (JournaledOperationRunner, OperationPlanner, SnapshotFactory/Store) | **Stays**; add history ledger (P9), snapshot gains icon-size + overrides (P9/P10) |
| `DeskMakeover.ElevatedHelper` | **Stays** as-is |
| `DeskMakeover.IconRendering` (masks, classifier, styler, resampler, IcoWriter, GeneratedIconStore, OverlayBadgeIconFactory) | **Core stays**; shape engine extended (P2), colour engine aligned (P3), mark factory rewritten to the 7 styles (P4) |
| `DeskMakeover.App` MainWindow.xaml/.cs, MainViewModel, ComboCardViewModel | **Rebuilt** to the prototype layout (P5–P8); salvage services (MakeoverService, OverlayBadgeService, ThemeManager, WallpaperColorService, ComparisonImageExporter, ColorPicker control, presentation mappers) |
| Tests (~105) | All stay green; every phase adds its own |

Keep `MainWindow.xaml` under control by splitting into UserControls (each ≤500
lines): `TitleBarView`, `ControlPanelView`, `PresetGridView`, `CustomizeAccordionView`,
`DesktopCanvasView`, `SettingsDrawerView`, `AboutDialogView`, `TileContextMenuView`,
plus the existing `ColorPickerControl`.

---

## 3. Phases

Work strictly in order; each phase ends with: build 0-warnings, tests green,
its acceptance checks, and (UI phases) a screenshot compared against the
prototype. Commit per phase with a conventional message.

### P0 · Baseline

- Run: `dotnet build DeskMakeover.slnx && dotnet test DeskMakeover.slnx` — record
  the green baseline. Run the app, screenshot the current UI once (before shot).
- Open the prototype in a browser; click through every state (apply, dirty,
  restore, history, compact via the demo strip, drawer, picker, right-click
  menu, about). You must be able to describe each from memory before P5.

### P1 · Design tokens & theme plumbing

**Files:** `src/DeskMakeover.App/Theming/` (ThemeManager + new `DesignTokens.xaml`
resource dictionaries — dark + light), App.xaml merges.

- Transcribe the token tables from spec-02 §Colour Tokens verbatim (prototype
  lines 13–14). Three theme modes: 深色 (default) / 浅色 / 跟随系统; live switch
  without restart (prototype `themeClass` behaviour, line 1168–1170).
- `accentInk` rule: dark theme = accent; light theme = accent mixed 70% toward
  `#40140C` (compute once in code; WPF has no color-mix).
- Selected-chip wash = accent at 17% over chip base; preset-selected = 15% mix;
  precompute these brushes per theme.
- **Accept:** a token test page or harness renders both palettes; no `#5E5CE6`
  or any blue/violet literal remains anywhere in `src/` (grep gate:
  `grep -rniE "5E5CE6|7C6AF2|4CC2FF" src/` → empty).

### P2 · Shape engine parity (StylePreset axes + masks)

**Files:** `src/DeskMakeover.Core/StylePreset.cs`,
`src/DeskMakeover.IconRendering/ContinuousCornerMask.cs` (+ new `OneUiMaskPath.cs`),
`src/DeskMakeover.App/Controls/SquircleGeometry.cs`, tests.

- Extend the preset value object to the v1.0 axes (spec-01 §System Architecture):
  `Shape {Apple, Circle, Samsung}` · `ColorMode {Original, BlackWhite, Mono}` +
  `Tint` · `Distinction {Mark, Keep, None}` · `MarkStyle {Glass, Card, Echo,
  Satin, Arc, Fold, Ring}` · `MarkColor int?` (null=auto) · `IconSize {Small,
  Mid, Big}`. Pure data; serializable (history needs it).
- Masks per prototype `clipFor` (991–1020): Apple = quintic superellipse
  (existing engine, n=5, 96-pt polygon fidelity); Circle = exact circle with the
  `IsRoundish` keep-untouched rule intact; **Samsung = the official One UI path**
  `M50,0 C10,0 0,10 0,50 C0,90 10,100 50,100 C90,100 100,90 100,50 C100,10 90,0 50,0`
  scaled to size — implement as cubic-Bézier rasterization (raster side) and a
  `PathGeometry` (XAML side) from the same constant table. Delete/retire the old
  superellipse(r=.40,n=4) Samsung approximation.
- **Accept:** golden tests: mask row-span symmetry, Apple apparent-corner ≈22.37%
  ±0.5%, Samsung matches sampled points of the Bézier path (tolerance 1px @256),
  circle exactness; XAML geometry and raster mask agree on edge coordinates
  (sample 8 angles).

### P3 · Colour engine parity (`styledFor` math)

**Files:** `src/DeskMakeover.IconRendering/IconStyler.cs`,
`BackgroundClassifier.cs` (if plate detection needs alignment), tests.

Implement spec-02 §Colour Treatments exactly (prototype 846–864 + 1040–1053):

- luminance `l = (0.299R+0.587G+0.114B)/255` of the tile's dominant colour;
- 原彩: keep colour; ink dark when `l>0.66`;
- 黑白: `v = 255·clamp(0.5+(l−0.5)·1.4, 0.08, 0.94)`; ink `#2A2A2E` when `v>168`;
- 单色: H,S from tint (prototype `hsl()` 848–859); `L = 26+46·l` %; fill
  `hsl(H, S·0.85, L)`; ink dark when `L>56`;
- document-kind plates (原彩 `#F7F7F4` / 黑白 `#EFEFED`+`#3B3B3F` / 单色
  `hsl(H,S·0.5,90%)` + glyph `hsl(H,S·0.9,30%)`) — map to the real pipeline's
  white-tile strategy for document-like icons.
- **Accept:** unit tests over a fixed swatch set **including pure black
  `#000000` and pure white `#FFFFFF` tiles** (the prototype ships both as test
  icons, lines 986–987): every treatment keeps glyph/plate contrast ≥ WCAG 3:1;
  黑白 never outputs v<20 or v>240; 单色 hue equals the tint hue.

### P4 · The seven shortcut marks (renderer rewrite)

**Files:** `src/DeskMakeover.IconRendering/OverlayBadgeIconFactory.cs` → refactor
into `Marks/` (one file per style + shared helpers, each ≤300 lines), tests
(extend `OverlayBadgeIconFactoryTests`, `BadgeStyleRenderHarness`).

Implement all seven styles as raster algorithms per spec-02 §Shortcut Marks —
the executable reference is prototype lines **1405–1487** (canvas tiles) and
930–962 (chip previews). Shared laws:

- Anchor on the icon's **own alpha** (ADR-0006): multiply every mark element by
  icon alpha; never spill outside.
- Adaptive tone from tile luminance `l` (threshold 0.58 — note: different from
  the ink threshold 0.66 of P3; keep both as named constants).
- `MarkColor` null = auto (neutral adaptive); user colour is **mixed** per style
  (the color-mix percentages are in spec-02's table — implement `Mix(c1,c2,pct)`
  once).
- 经典箭头 (Keep): classic plate `#F4F4F1` radius 4, dark ↗ `#2E3238`,
  bottom-left, size `max(14, 0.28S)` — used for kept-original items in preview
  AND as the 保留原样 desktop treatment (unchanged real arrow there).
- WPF/backdrop note: "frosted/backdrop-blur" in the prototype = blur the
  underlying icon pixels in the raster pipeline (box/stack blur), then overlay
  the translucent seat — the tiles are our own bitmaps, so this is exact.
- Fold (卷角) geometry: corner factor per shape `{apple:.26, samsung:.28,
  circle:.30}`; 315° linear mask cuts the SE corner at `0.707c`; the fold
  triangle mirrors across the cut with the 4-stop warm-paper gradient
  (prototype 1468–1479).
- **Bake parity:** `MakeoverService.ApplyAsync`/`CatchUpAsync` bake the selected
  mark into each per-icon `.ico` (all ladder sizes; hint small sizes: at 16–24px
  simplify strokes to ≥1.5px, drop shadows) and keep the registry overlay
  transparent (ADR-0006 facts — already wired; re-verify).
- **Accept:** per style × 3 shapes × {dark tile, light tile, 纯黑, 纯白} harness
  renders (offscreen PNG grid) — no spill outside alpha, adaptive flip at the
  threshold, user-colour honoured with ring/mix, 16px legibility; the 3-second
  misread gate set (owner reviews the harness sheet — flag for owner, don't
  self-approve). Existing adaptive-ink tests keep passing for Glass.

### P5 · Window shell & layout skeleton

**Files:** `MainWindow.xaml` (slimmed to regions), new `Views/TitleBarView.xaml`,
`Views/DesktopCanvasView.xaml` (stub), `Views/ControlPanelView.xaml` (stub),
`ShellViewModel`.

- Custom title bar (prototype 44–56): 24px coral apple-squircle logo (real
  app.ico asset, not ✦ text), 「桌面美颜」 13/600, 「v1.0」 chip, spacer, ⚙, ⋯,
  ─ ▢ ✕ caption buttons (36×30, hover `--raisedHov`); window drag on empty area;
  dark titlebar attribute; **no Mica**.
- Body grid: control panel column (300px) + canvas column (fills), paddings per
  spec-02 metrics. Canvas region: radius-14 clipped, inset hairline ring.
- **Compact mode**: when window width < 1100px → panel column collapses; panel
  becomes an overlay (300px, slides from left 0.22s, scrim `rgba(0,0,0,.35)`,
  close on scrim click/Esc) and the summary toolbar (P6.6) appears above the
  canvas. Prototype geometry: lines 1189–1194, 385–394, 560–562.
- Esc handling per prototype (line 800): closes menu → drawer → panel → overflow
  → about → picker.
- **Accept:** screenshot at 1340×840 and at 1024×700 vs prototype 常规窗口 /
  紧凑窗口 (use the demo strip to switch the prototype); regions align within
  ~2px at 100% DPI.

### P6 · Control panel (hero, presets, accordion)

**Files:** `Views/ControlPanelView.xaml` + `MainViewModel` (split: keep VM ≤500
lines — extract `PresetSectionViewModel`, `CustomizeSectionViewModel`).

1. **Hero + CTA state machine** — implement the exact 5-state table from spec-01
   (prototype 1196–1224). N = real scanned styleable count. CTA is the ONLY
   solid-coral surface on the screen.
2. **Link chips** 还原 / 上一版 / 历史 N / 对比图 with the prototype's visibility
   rules (1585–1590): show row once applied or history non-empty; 还原 only when
   applied; 上一版 needs history[1] when applied else history[0]; tooltips as in
   lines 84–93.
3. **版本历史 card** (98–119): expandable via 历史 chip; rows + 当前 pill +
   回到此版 + fixed footer 「最初 · Windows 原生桌面 · 回到最初」; `rise`
   entrance. Wire to the P9 ledger.
4. **风格 presets** (148–169, 1227–1244): 2×2 cards; each renders TWO 18px live
   mini previews using the real pipeline (sample icons: the user's 3rd and 8th
   scanned items — prototype uses demo indexes 2 and 7; pick two visually
   distinct real icons deterministically); selected wash = accent 15% mix;
   「自定义中」 indicator per `activePreset()` semantics (1064–1072): match on
   shape+colour+dist(+tint), markStyle/markColor/size do NOT deactivate.
5. **自定义 accordion** (171–277): 4 rows (42px) with right-aligned summary value
   + rotating chevron; ＋/− expand-all toggle (1609–1614); chips per spec-01:
   - 外形 chips with 14px live clip swatches;
   - 配色 chips (原彩 conic swatch / 黑白 split swatch / 单色 tint dot) + 单色
     swatch row (7 swatches incl. wallpaper primary/secondary — from
     `WallpaperColorService`) + 调色盘 chip;
   - 快捷方式标识: 3 state chips → when 美化: 7 mark chips **with live 22px mark
     previews rendered by the P4 code** (markPreview parity, 930–962) + 标识配色
     row (自动 chip default-selected + 5 swatches + 调色盘);
   - 图标大小: 小/中/大.
6. **Compact summary toolbar** (385–394): 4 preset chips + 「自定义 ▸」 (opens
   panel overlay) + compact CTA (34px), horizontal scroll if needed.
7. Every axis change routes through ONE `RestyleRequested` funnel: ~420ms
   debounce → in-place tile image swap + 「正在更新预览…」 cue + tiles dim to 45%
   (511–513, 1489) — reuse/port the existing `RunRestyleAsync` funnel; never
   Clear()+rebuild.
- **Accept:** screenshot parity for: default state, each accordion row open,
  单色 selected (swatch row visible), 美化 with mark chips visible, expand-all;
  interaction video/gif optional but state screenshots mandatory. Copy strings
  byte-identical to the prototype (see §5 copy table sources).

### P7 · 调色盘 (shared colour-picker popup)

**Files:** existing `Controls/ColorPickerControl` (+ `ColorPickerViewModel`) —
align to the prototype (624–658, 885–928).

- 244px popup anchored near the invoking chip (clamp inside window, 885–895);
  contents: SV field (122px, crosshair cursor ring) → hue bar (14px) → preview
  square + hex input (mono font, accepts `#RRGGBB`/`RRGGBB`) + eyedropper ⌖ →
  「从壁纸自动提取」 4 wallpaper-palette swatches → 「快捷选择」 6 swatches
  (白/黑/珊瑚/湖水/琥珀/砖红 `#E4574D`).
- Two consumers (ADR-0005 D3): 图标单色 (sets tint + colorMode=mono live) and
  标识配色 (sets markColor live); title switches accordingly (1672).
- Eyedropper: existing screen-pixel capture; on failure show toast, never crash.
- **Accept:** open from both consumers; drag SV/hue live-updates the preview
  grid through the P6 funnel; hex round-trips; screenshot vs prototype picker.

### P8 · Desktop-mirror canvas (tiles, compare, taskbar, context menu)

**Files:** `Views/DesktopCanvasView.xaml`, `PreviewItemViewModel`,
`Views/TileContextMenuView.xaml`, tile-rendering glue.

1. **Background** = the user's actual wallpaper (stretched, from
   `WallpaperColorService`/system API), NOT a flat surface. Canvas bottom hosts
   the **decorative taskbar** (519–555): translucent strip, start grid glyph
   (`#5AA7E8` squares — the ONE permitted blue, it depicts Windows itself),
   search glyph, 5 generic app chips, live clock HH:mm + yyyy/M/d. Non-interactive.
2. **Tile grid**: column-major wrap (top→bottom then next column — real desktop
   order), tile cell = box+18 wide, icon box 1.08S×1.10S, label 11px with text
   shadow, single-line ellipsis, hover cell wash. Sizes S per `IconSize`.
3. **Tile composition** mirrors prototype 1386–1507 exactly: shimmer (shape-
   clipped, staggered) while scanning → styled card (P3 colour + P2 clip) +
   mark layers (P4) — kept-original items and peek/compare show the original
   icon with the classic arrow for shortcuts.
4. **Press-to-peek** per tile (pointer down/up/leave, 1504–1505).
   **Compare pill** 「⇄ 按住对比原样」/held 「原来的样子」 (515–517, 1509–1512,
   1648–1650): while held the whole canvas shows originals; also active-styled
   during `settle`.
5. **Right-click context menu** (745–757, 1514–1526): header = item label;
   保留原样 with ✓ when active (toggles override, toast 「已为…」/「…恢复跟随全局」
   texts at 1658–1663); 跟随全局样式 (clears override); 单独配色 6 swatches
   (sets `{tint}` override, toast 「已为「X」单独配色」). Overrides:
   preview-instant; if globally applied → immediately restyle that one icon on
   the real desktop (journaled single-item op).
6. Bloom wave on apply (per-tile stagger 42ms), settle on restore (24ms), all
   reduced-motion aware (P12 centralizes timings).
- **Accept:** screenshots: scanning shimmer, default applied grid, compare held,
  a peeked tile, context menu open, each of small/mid/big grid density; parity
  vs prototype states. All 20+ real icons visible without scroll at 1340×840
  with mid size.

### P9 · Apply / dirty / restore / version history (orchestration)

**Files:** `Orchestration/MakeoverService.cs`, `Operations/` (new
`LookHistoryStore.cs`), `Core` (new `LookVersion.cs`), `MainViewModel`.

- **Dirty semantics** (1074–1078): applied && any of (shape, colorMode, tint,
  dist, markStyle, markColor, sizeMode) differs from the applied snapshot config
  → CTA 更新桌面. Per-icon overrides do NOT dirty the global CTA (they apply
  immediately, P8.5).
- **doApply pipeline** (1103–1118) mapped to real ops: guard (ready && !working
  && (not applied || dirty)) → snapshot (first apply) → journaled restyle (only
  changed items on update — diff by item+effective config) → bake marks + icon
  size → success: store applied config, push history entry `{HH:mm, label,
  config}` (cap 10), toast 「美化完成 · 已保存还原快照」, bloom.
- **labelFor grammar** (1081–1087): 外形名 · 配色名 · (经典箭头|无标识|mark-style
  name). Localized via the resource table.
- **回到此版 / 上一版** (1092–1101): re-apply that config through the same
  pipeline; push a NEW history entry; toast 「已回到：<label>」.
- **回到最初 / 还原** (1119–1123): full journaled restore (icons, ico store,
  overlay registry, layout best-effort, icon size) → applied=false, keep
  history, toast 「已还原 · 桌面回到原来的样子」, settle animation.
- **LookHistoryStore**: JSON under `%LocalAppData%\DeskMakeover\history.json`,
  newest-first, cap 10, corruption-tolerant (bad file → empty + log).
- **Accept:** unit tests — dirty matrix (each axis), history cap/ordering/
  re-apply-pushes-new, restore-preserves-history, config serialization
  roundtrip; integration test with temp `.url` fixtures: apply → tweak → update
  (only changed items rewritten) → 上一版 → 回到最初 → zero residue (extend
  existing MakeoverService roundtrip tests).

### P10 · 图标大小 (real desktop icon size)

**Files:** `src/DeskMakeover.Shell/DesktopIconSize.cs` (new), snapshot fields,
`MakeoverService` wiring, tests.

- Read/write the desktop icon size via `IFolderView2` on the desktop folder view
  (`SHGetDesktopFolderView` route): `GetViewModeAndIconSize` to capture the
  original into the snapshot; `SetViewModeAndIconSize(FVM_ICON, px)` to apply.
  Mapping: 小=32 · 中=48 (Windows default) · 大=96. No registry hacks; no
  Explorer restart (the COM route applies live).
- Restore returns the captured original size. If the COM route fails (rare
  shells), degrade: leave size untouched, mark the axis unavailable in UI
  (summary shows 系统默认), never error the apply.
- **Accept:** manual verify on the Windows box: switch 小/中/大 live, restore
  returns original; snapshot JSON contains the captured size; unit tests for the
  mapping + snapshot field; graceful-degrade path covered by a fake.

### P11 · Drawer, overflow, About, changelog

**Files:** `Views/SettingsDrawerView.xaml`, `Views/AboutDialogView.xaml`,
overflow menu in `TitleBarView`, `AppSettings`.

- **Drawer** (564–622): exact rows/copy per spec-01. 新图标自动美化 = the
  existing keep-up (logon task + catch-up) master toggle, default ON. 还原快照
  「导出」 = copy the current snapshot JSON to a user-chosen folder. 前后对比图
  「保存」 = existing ComparisonImageExporter. Theme segmented control live-
  switches P1 themes.
- **Overflow** (660–666, 1699–1704): 检查更新 → open
  `https://github.com/nicepkg/deskmakeover/releases` in default browser;
  帮助与反馈 → `…/issues`; 更新日志 / 关于 → About dialog tabs.
- **About** (668–743): everything per spec-01 §About (logo asset, v1.0.0, five
  chips, GitHub card, author card with the 5 real links, buttons, footer).
  Links open the browser — the app itself stays offline. **Changelog tab**
  (723–741): content from a local `changelog.json` resource; seed with the real
  v1.0.0 notes (write honest bullets at release time; prototype's 1713–1716 are
  placeholders).
- **Accept:** screenshots vs prototype (drawer, overflow, about, changelog);
  every link verified to open the right URL; toggle states persist via
  AppSettings across app restarts.

### P12 · Motion & reduced motion

**Files:** `Theming/MotionTokens.cs` (or resource dictionary) + usages.

- Centralize the spec-02 motion table: bloom (.6s, cubic-bezier(.34,1.4,.4,1),
  42ms stagger), settle (.8s, 24ms), shimmer (1.3s), rise, drawer (.22s), pop
  (.12–.18s), 0.15s hover transitions, CTA press scale .98.
- System reduced-motion (`SystemParameters.ClientAreaAnimation` / animation
  preference): everything degrades to plain crossfades — no scale/stagger/sweep.
- **Accept:** manual check of each animation; reduced-motion mode screenshot-
  verified (no motion, still functional); no animation runs during in-place
  restyle refresh (only the dim+cue).

### P13 · Real-desktop regression (foundation re-verify)

No new features — re-verify the untouched foundation still holds behind the new
UI, on the Windows box:

- Apply → check real desktop: crisp multi-size icons, correct mark baked, no
  double arrow (overlay transparent), kept items untouched, icon size applied.
- New shortcut appears → catch-up styles it per current config incl. mark/size.
- UAC denial mid-flow → non-privileged styling applied, privileged step
  skippable/retryable, honest message.
- Restore → zero residue (icons, registry overlay, ico store, layout
  best-effort, icon size) — verify with `RestoreMetadataCollector` evidence +
  manual inspection.
- **Accept:** the existing integration tests green + a written evidence log of
  the manual pass (commands/screenshots), stored in the PR/commit message.

### P14 · Parity audit, docs, release prep

1. **Side-by-side audit** — for each checklist row (§4), screenshot the app and
   the prototype in the same state; fix every visible divergence (spacing,
   colour, copy, radius, order). The audit sheet (markdown table + image pairs)
   is the release evidence; store under `docs/plans/evidence/2026-07-parity/`.
2. Update `docs/STATE.md` (done/next), write `CHANGELOG.md` v1.0.0, bump
   assembly/product versions to 1.0.0, regenerate app.ico if the logo asset
   changed, run `node scripts/publish-win.mjs`, fresh-run smoke of the published
   exe.
3. Remaining release gates tracked in STATE (owner): signing cert, public repo,
   fresh-VM smoke, supervised live run.

---

## 4. Parity checklist (the audit contract)

Every row must pass the side-by-side comparison (今日形态, unless noted):

**Shell**: title bar composition + hover states · window at regular/compact ·
panel overlay + scrim (compact) · Esc close ordering.
**Hero**: 5 CTA states with exact copy · link-chip visibility rules · history
card (rows, 当前 pill, 回到此版, 回到最初 footer).
**风格**: 4 preset cards with live minis · selected wash · 「自定义中」 rules
(markStyle/size don't break preset; dist/shape/colour/tint do).
**自定义**: accordion summaries + chevrons + expand-all · 外形 3 chips with clip
swatches · 配色 3 chips + 7 单色 swatches + 调色盘 · 标识 3 states + 7 mark chips
with live previews + 自动/swatches/调色盘 · 大小 3 chips.
**Marks (per style × 3 shapes × light/dark/纯黑/纯白 tiles)**: 玻璃箭头 seat+
arrow adaptivity · 双层卡片 offsets+tones · 幽灵叠影 translucency · 缎光角
gradient · 珐琅光弧 glow · 卷角 dog-ear+fold gradient · 细描边 ring · 经典箭头
plate · 无标识.
**Canvas**: real wallpaper bg · column-major order · shimmer · label styling ·
hover wash · press-to-peek · compare pill idle/held · 「正在更新预览…」+dim ·
taskbar strip + live clock · bloom/settle waves.
**Context menu**: contents, ✓ state, swatch ring, toasts.
**Picker**: full anatomy, both consumers, wallpaper palette, eyedropper.
**Drawer / Overflow / About / Changelog**: exact contents, copy, links.
**Colour treatments**: 原彩/黑白/单色 across the icon set incl. documents and
纯黑/纯白 · wallpaper primary/secondary swatches present.
**Sizes**: 小/中/大 preview density AND real desktop change+restore.
**Flows**: apply → done · tweak → dirty → 更新桌面 (in-place, no flash) · 还原 →
settle + history preserved · 回到此版/上一版 · per-icon override immediate
restyle · keep-up styles a new shortcut · UAC denial path.
**Toasts** (exact strings): 美化完成 · 已保存还原快照 / 已还原 · 桌面回到原来的样子 /
已回到：… / 已为「X」单独配色 / 「X」将保留原样 / 「X」恢复跟随全局 /
「X」已跟随全局样式 / 对比图已保存… / 快照已导出… / 此浏览器暂不支持屏幕取色→
(app equivalent: 取色失败提示).
**Global**: no blue/violet anywhere (except the taskbar start glyph) · dark AND
light theme for every screen · reduced-motion pass · zh-CN strings byte-match
the prototype (except v1.0 version strings).

---

## 5. Where to find every copy string

Do not invent copy. Sources, in order: (1) the prototype HTML text nodes and
`renderVals()` string literals; (2) spec-01 tables (hero/CTA, drawer, about);
(3) for genuinely new strings (error paths the prototype lacks), follow spec-01
§UI Language Rules and the ADR-0003 tone rule, and flag them for owner review.
All strings live in the localization resource table (`Resources/UiText`);
zh-CN is release-gating.

## 6. Reporting protocol (for the executing AI)

- Per phase: report DONE / DONE_WITH_CONCERNS / BLOCKED with the verify output
  and screenshots. Never claim done without fresh evidence from this turn.
- Any prototype ambiguity: open the prototype, reproduce the state, and match
  behaviour; if still ambiguous, choose the interpretation that matches the
  prototype's *code* (not its prose comments) and log the call in STATE.md.
- Bug found in the untouched foundation: fix with a regression test (red→green)
  — a bug fix always ships its regression test.
- Keep `docs/STATE.md` updated at every phase boundary (it is the continuity
  file for the next session).
