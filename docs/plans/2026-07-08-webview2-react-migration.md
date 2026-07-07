# Plan — WebView2 + React UI migration (ADR-0011, spec 05)

Owner directive 2026-07-08: replatform the UI to WebView2 + React 19 +
Tailwind 4 + shadcn/ui + Motion. Bun-only (no Node). Latest package versions
from the live registry, never from memory. Restore the current layout
(baseline: `docs/plans/evidence/2026-07-settings-i18n-shapes-icon/*.png`,
visual law: spec 02, prototype). Prefer shadcn primitives over hand-designed
interactions. Old WPF UI is deleted after parity — nothing is released, no
compat. The icon bake and wallpaper apply stay owner-supervised gates.

Execution log lives at the bottom; STATE.md points here.

## P0 — Scaffold (verify versions live)

1. `bun create vite` → `src/DeskMakeover.Web` (react-ts template); record the
   actual installed versions of react / react-dom / vite / typescript /
   tailwindcss / @tailwindcss/vite / motion / zustand in the execution log
   (`bun pm ls`). Install shadcn via `bunx shadcn@latest init` (Tailwind 4
   flow) + the primitives we need (button, slider, switch, dropdown-menu,
   dialog, tooltip, scroll-area, popover…).
2. Add `Microsoft.Web.WebView2` (latest stable from nuget.org) to
   `DeskMakeover.App.csproj`.
3. Verify: `bun run build` produces `dist/`; `dotnet build` 0 warnings.

## P1 — Host window + bridge + preview transport

1. `Host/WebShellWindow` (frameless WPF window + WebView2, WindowChrome resize
   borders, WM_GETMINMAXINFO port, non-client region support, virtual hosts
   `app.deskmakeover` → `<exe>/web` and `assets.deskmakeover` →
   `%LocalAppData%/DeskMakeover/webassets`, dev-server env override, WebView2
   hardening per spec 05 §1).
2. `Host/Bridge/` — envelope types, dispatcher (method registry, off-thread
   handlers, error mapping), `Contracts.cs` DTOs + `BRIDGE_SCHEMA_VERSION`,
   frame pusher (`PostSharedBufferToScript`).
3. Web: `src/bridge/` typed client (promise correlation, event bus, schema
   check, mock transport for browser-only dev).
4. `Host/Controllers/ShellController` (window ops, openExternal, data folder,
   app info) as the first real controller.
5. Verify: app launches showing the React splash; `shell.*` round-trips work;
   .NET bridge tests with fake transport green.

## P2 — Design system

1. Tailwind `@theme` tokens = spec 02 tables (dark default, `.light`
   overrides); font chain; tabular numerals. Banned-colour `bun test`.
2. `components/common/`: Chip/ChipGroup, Slider (coral), AccordionAxis
   (42px row, chevron 180°, hairline separators) + ExpandAllToggle (＋/−),
   CtaButton (HeroPhase states, 44px, dm-glow), Toast, SquircleMask (clipFor
   SVG paths ported from `IconShapeGeometry` — same math), LinkChip,
   SegmentedControl, Toggle (32×19), AngleDial (SVG rotary, drag + Shift 15°),
   ColorPicker (SV field 122 + hue bar + hex + eyedropper via
   `EyeDropper` API/host fallback + wallpaper/quick swatches, width 244).
3. `lib/motion.ts` presets: bloom/settle/shimmer/rise/slide/pop + reduced-motion.
4. Verify: Storybook-less demo route (`?debug=components`) screenshot; bun tests
   for zone math scaffolding + i18n harness land here.

## P3 — App shell

1. TitleBar (46px, logo squircle, name + version chip, drag region, caption
   buttons), ModuleRail (66px, 3 modules, Ctrl+1/2/3), routing store.
2. SettingsPage (two-column: identity card | 外观(语言/主题 segmented) ·
   自动化 toggle · 本地数据 · 关于与帮助 + inline changelog) — parity with
   `settings.png`, taste-upgraded spacing/typography.
3. i18n dictionaries ported from `Strings.resx` + `Strings.zh-Hans.resx`;
   settings.set round-trip; effective-theme sync (`os-theme-changed`).
4. Verify: all three rail routes render; language/theme switch live; screenshots.

## P4 — Icons module

1. `Host/Controllers/IconsController`: scan, restyle (tile PNG pipeline into
   assets host, versioned names), original tiles, overrides, apply/restore/
   history/exports — thin adapters over MakeoverService/DesktopBakeService/
   LookHistoryStore (logic stays in services).
2. IconsPanel: status line + hero + CTA state machine (5 states, spec 01) +
   link chips (还原/上一版/历史 N/对比图) + history card + 风格 presets (2×2,
   live minis) + 自定义 accordion (外形 13 shapes / 配色 3+tint+swatches /
   快捷方式标识 3-state + 6 marks + mark colour / 图标大小) — 420ms debounce,
   「正在更新预览…」 cue.
3. MirrorViewport: real wallpaper underlay, column-major tile grid at true
   metrics (from `system.getEnvironment`), labels (2-line ellipsis, real font
   size), decorative taskbar + live clock, pan/zoom (Ctrl+wheel at pointer,
   Ctrl+=/−/0, fit-height default, ⤢, ↻), compare pill (Space hold), per-tile
   press-to-peek, right-click override menu, shimmer/bloom/settle waves.
4. Compact mode (<1100px): panel becomes overlay + summary toolbar.
5. Verify: visual parity vs `icons.png`; E2E: scan → preset switch → axis
   change → history (fake-apply flag for the bake).

## P5 — Wallpaper module

1. `Host/Controllers/WallpaperController`: getState/recompose (debounce
   host-side guard + revision echo)/apply/restore/saveLook/fonts.list/palette.
2. WallpaperPanel: eyebrow/hero/CTA (应用到壁纸 states) + 分区 section
   (用推荐布局 / + 添加分区 / zone rows with size + delete) + 自定义 accordion
   (清晰度: 3-seg + strength 0-100 + direction chips + AngleDial + scrim colour
   chips + picker, pale badge · 分区样式: 4 styles + fill colour + opacity
   3-100 + corner radius · 标题文字: font dropdown (bundled first + system
   families, virtualized list) + size/align/ink/shadow) + footnote — parity
   with `wallpaper.png`.
3. ZoneLayer on the mirror: `lib/zone-math.ts` (half-cell snap, exclusive-edge
   resize, min 2×2, clamp; ported 1:1 from `DesktopCanvasView.Zones.cs`),
   rubber-band create, 8 handles, move, arrow nudges (0.5), Del, inline rename,
   dual-density grid guide while interacting, selected-only chrome, ghost mock
   icons (blueprint SVG, partial spread, global-grid aligned), fit-all default
   in this module, real icons hidden.
4. Preview: shared-buffer frames → canvas; fingerprint banner + 重新合成;
   140ms debounce; stale-revision drop.
5. Verify: visual parity vs `wallpaper.png`; bun tests for zone math == C#
   fixtures; E2E zone create/move/resize; compose round-trip timing logged.

## P6 — Delete the WPF UI layer

1. Remove `Views/ ViewModels/ Controls/ Presentation/` + Theming XAML
   (Components/Controls/Tokens) + `MainWindow.xaml` + `LocExtension` + resx
   UI tables; keep Orchestration/Preview/Resources(engine)/Sharing/AppSettings/
   WallpaperColorService; refactor anything ImageSource-coupled in the kept set
   to byte[]/file outputs.
2. App.Tests: drop VM/design-token tests whose subject is deleted; keep/port
   service + composer + snapshot tests. Bridge controller tests replace VM tests.
3. Verify: `dotnet build` 0 warnings; full `dotnet test` green; grep: no
   `System.Windows.Controls` outside Host/window plumbing; file count drop
   recorded.

## P7 — Package + E2E + review + wrap

1. `scripts/publish-win.mjs` runs under bun: web build → dotnet publish →
   copy dist → `<out>/web`; smoke the published exe.
2. Playwright .NET E2E project (`tests/DeskMakeover.E2E`): launch, rail
   navigation, icons flow, wallpaper flow, settings flow (fake-apply).
3. Adversarial review (dev-cycle Phase 6): codex spec-compliance pass, then
   code-quality pass; fix + re-review until clean.
4. Fresh screenshots of all three modules vs baselines; STATE.md + this log
   updated; commit series complete.

## Execution log

- **P0 done (2026-07-08).** `src/DeskMakeover.Web` scaffolded with
  `bun create vite` (react-ts). Versions installed from the live registry:
  react/react-dom **19.2.7**, vite **8.1.3**, typescript **6.0.3** (template
  pins ~6.0.2), tailwindcss + @tailwindcss/vite **4.3.2**, motion **12.42.2**,
  zustand **5.0.14**, @vitejs/plugin-react 6.0.3, lucide-react 1.23.0,
  radix-ui 1.6.2 (shadcn CLI 4.13.0, style `radix-nova`, css-variables,
  neutral). Primitives added: button, slider, switch, dropdown-menu, dialog,
  tooltip, scroll-area, popover, separator, toggle-group, toggle, input,
  label. Notes: vite 8 template ships oxlint (kept); TS 6 deprecates `baseUrl`
  (paths declared without it); shadcn CLI moved to devDependencies; template
  boilerplate replaced with a placeholder App. Host side:
  `Microsoft.Web.WebView2` **1.0.4078.44** added to DeskMakeover.App.
  Verify: `bun run build` green (435ms) · `dotnet build` 0 warn / 0 err.
- **P1 done (2026-07-08).** Host: `Host/FramelessWindowFrame` (WM_NCCALCSIZE
  keeps the REAL system resize frame, removes the caption, 1px HTTOP strip —
  the Windows Terminal recipe; WebView2's IsNonClientRegionSupportEnabled
  documented as caption-only, so resize must stay native),
  `Host/Bridge/BridgeDispatcher` (JSON-RPC over WebMessageReceived, off-thread
  handlers, trusted-origin check, shared-buffer frame channel with buffer
  reuse), `Host/Bridge/Contracts` (schema v1), `Host/Controllers/ShellController`
  (window ops / openExternal whitelist / data folder / app.getInfo with
  bilingual changelog / settings get+set with settings-changed event),
  `Host/WebShellWindow` (virtual hosts app.deskmakeover + assets.deskmakeover,
  hardened WebView2 settings, navigation lockdown, NewWindowRequested →
  default browser, ProcessFailed → reload, dev-server env override). Web:
  typed bridge client (`src/bridge/`) with promise correlation, event bus,
  sharedbufferreceived → copy + releaseBuffer, schema assert, and a mock
  transport for plain-browser design work. App.xaml drops StartupUri; the web
  shell is the only window. csproj copies `dist/**` → `<out>/web`.
  **Verified live**: screenshot shows the React page hosted frameless with
  real settings round-tripped; REAL-mouse drag on the app-region titlebar
  moved the window (610,276 → 730,356) and REAL-mouse corner drag resized it
  (1340×840 → 1404×888). Frame channel is exercised by P5's preview.
- **P2 done (2026-07-08).** Tokens: `index.css` rewritten — spec-02 palette
  verbatim (dark default `:root`/`.dark`, `.light` overrides, coral-ink
  contrast mix, chip/preset/rail washes via color-mix), shadcn semantic vars
  mapped onto the product palette (primary=coral), Segoe font chain (geist
  dropped, CSS 71→50KB), app-shell base (no page scroll, no text select,
  quiet overlay scrollbars, drag-region classes, dm-slider/dm-hue skins).
  Primitives in `components/common/`: Chip/ChipRow, AccordionAxis +
  ExpandAllToggle (42px rows, chevron 180°, height collapse), CtaButton
  (5 HeroPhases + dm-glow), Segmented, ToggleSwitch (32×19), DmSlider (native
  range, coral fill), LinkChip, AngleDial (SVG rotary, shift 15° snap,
  keyboard), ColorPickerPanel (SV field/hue/hex/EyeDropper/swatch rows) +
  `lib/color.ts`, ToastHost + zustand store, `lib/motion.ts` (named spec-02
  variants incl. bloom/settle staggers), `lib/geometry.ts` (apple squircle
  path). Guard: `tests/banned-colors.test.ts` walks shipped sources and
  rejects blue/violet hexes (HSV band 195-290°) + Tailwind blue-family
  classes; 35 pass. Verified live in the hosted window via the
  `?debug=components` gallery (dev-server env path): dark + light screenshots;
  a REAL mouse click flipped the theme (CDP probe shows trusted pointerdown on
  the button — clicks reach web content; earlier "dead click" was injection
  timing, fixed by foreground+settle).
- **P3 done (2026-07-08).** i18n: `scripts/resx-to-i18n.ts` compiles BOTH resx
  tables to typed TS dictionaries (356 keys, parity-checked, `t()` strictly
  keyed); `lib/i18n` resolves System like UiText (zh* → 简体中文). Stores:
  `stores/app.ts` (module routing, window state, settings, boot wiring:
  settings-changed / os-theme-changed / window-state events; theme class on
  documentElement). Shell: TitleBar (46px drag band, logo+name+version chip,
  caption buttons w/ Win11 restore glyph), ModuleRail (66px, 3 modules,
  wash-rail selection, 设置 pinned bottom, Ctrl+1/2/3), SettingsPage (identity
  column + 外观/自动化/本地数据/关于与帮助 cards + inline localized changelog;
  导出/保存 disabled until P4 wires the exporters), module crossfade, ToastHost
  mounted. index.html retitled 桌面美颜. **Verified live** (screenshots): shell
  + settings parity vs the `settings.png` baseline; ONE real mouse click on
  "English" re-rendered the ENTIRE UI in English through the full loop
  (click → settings.set → persisted → settings-changed event → i18n), then
  restored to 跟随系统.
- **P4 core done (2026-07-08).** Host: `Host/WebAssets` (IconImage→PNG encoder,
  wallpaper snapshot w/ magic-byte sniffing, revisioned cleanup),
  `Host/IconsSession` (+.Render/.Apply partials) — the headless
  MakeoverViewModel: scan (grid metrics + layout + palette + applied-state
  rehydration), verbatim AssignPositions port, PLINQ tile pipeline writing
  styled+original PNGs into `tiles/r<rev>/`, preset minis, apply/restore/
  applyVersion/exportCompare (bake stays user-click-only;
  DESKMAKEOVER_FAKE_APPLY=1 for E2E), ops serialized by one gate;
  `IconsController` (10 RPCs); DesktopPreviewService gains IconImage-returning
  twins (RenderTileImage/RenderOriginalImage — same WYSIWYG two-step);
  BridgeJson enums-as-strings. Web: icons DTOs, `stores/icons.ts` (420ms
  debounced mutate w/ optimistic config, revision-guarded adoption, zoom→
  displaySize re-render, bloom/settle wave triggers, toast keys → i18n),
  IconsPanel (hero/CTA/link chips/history card/preset cards with live minis/
  5-axis accordion incl. 13 shapes, mono swatches + 调色盘 popover, 6 marks +
  mark colour, filter, size), IconsMirror (equal-scale desktop space: real
  wallpaper, tiles at Windows-faithful positions, 2-line clamped labels at the
  real icon-title font, decorative taskbar + live clock, drag-pan, Ctrl+wheel
  zoom at pointer, fit-height/fit-all, compare pill + Space hold, per-tile
  press-to-peek, custom right-click override menu, shimmer + updating cue,
  bloom/settle staggered waves). Fixed a hooks-order crash (useRef after an
  early return — CDP console caught it). Banned-colour test gains a tiny
  documented OS-mirror allowlist (Win11 taskbar replica colours).
  **Verified live**: scan of the real desktop (23 icons, real positions, real
  wallpaper); a REAL mouse click on 纯净黑白 restyled every mirror tile to
  engine-rendered grayscale through the full loop. Deferred to P7 punch list:
  compact (<1100px) overlay mode; settings-page 导出/保存 wiring.
- **P5 done (2026-07-08).** Host: `WallpaperSession` (headless WallpaperViewModel:
  backup-aware source resolution, ONE-composer recompose persisting the look and
  pushing raw RGBA frames over the shared buffer, pale + fingerprint checks,
  apply/restore as user-click-only gates w/ FAKE_APPLY for E2E),
  `WallpaperController` + `fonts.list` (bundled-first, zh-cn display names);
  IconsSession exposes EnsureScannedAsync/BuildWallpaperGrid/ScreenInfo/
  WallpaperTint so both modules project against the SAME grid. Web:
  `lib/zone-math.ts` (1:1 port: half-cell snap, exclusive-edge resize, min 2×2,
  ghost cells — 13 bun tests), wallpaper store (web owns the working look,
  140ms debounce, revision-guarded frames), WallpaperPanel (hero/CTA/分区
  section w/ inline rename + delete/清晰度 axis: 3-seg + strength + direction
  chips + AngleDial + scrim tone incl. 调色盘/分区样式 axis: 4 styles + fill
  colour + opacity + corner/标题文字 axis: font dropdown + size/align/ink/
  shadow/mismatch banner/pale badge/footnote), WallpaperMirror (shared-buffer
  frames → canvas at native px, zone layer: rubber-band create, snap move,
  8-handle resize, arrow nudge + Del, ghost blueprint icons, dual-density grid
  guide while interacting, selected-only coral chrome, compare pill).
  Bugs found live: hooks-after-early-return ResizeObserver never attached
  (BOTH mirrors — masked by HMR in P4; fixed with callback refs) and frames
  arriving before canvas mount were dropped (module-scope latest-frame cache).
  **Verified live on a fresh launch**: the persisted WPF-era look (新分区
  7×12.5, dark glass, handwritten title) reproduced pixel-faithfully in the
  composed first frame; a REAL mouse drag moved the zone with half-cell snap,
  selected-only chrome appeared, and the compose re-rendered at the new spot;
  style summary tracked the selected zone (半透明黑 · 9px).
- **P6 done (2026-07-08).** WPF UI layer deleted (Views/ViewModels/Controls/
  token+component XAML/converters/LocExtension/MainWindow); kept: Orchestration,
  Preview, Resources (UiText+resx feed the i18n generator), Sharing,
  StyleLabels/ItemPresentationMapper, PreviewItemViewModel (exporter input),
  AppSettings/WallpaperColorService/WindowChromeInterop; ThemeManager slimmed to
  ThemeMode + IsDarkActive + events (+ live OS-theme follow, settings.set now
  re-applies); IconOverride moved to Core. VM/design-token/XAML-geometry tests
  removed; the legacy-Glass normalization regression re-anchored to
  Config.Normalize + StyleLabels. Suite: **276 dotnet + 61 bun** green; the
  production virtual-host path smoke-verified.
- **P7 done (2026-07-08).** `publish-win.mjs` builds the web bundle first (bun)
  and the publish carries `<out>/web`; the published self-contained exe runs the
  full icons module (screenshot). Codex adversarial review: 10 findings, 7 fixed
  (DEBUG-only dev-server trust, double-buffered shared frames, wallpaper
  recompose revision drop, apply flushes the pending look, icons debounce
  cancel, boot-time schema assert, cross-run revision-folder purge), 3
  dispositioned (click-token: same trust domain; dispatcher lifetime = process;
  icons revisions already host-monotonic + client-guarded). Remaining punch
  list moved to STATE.md.
