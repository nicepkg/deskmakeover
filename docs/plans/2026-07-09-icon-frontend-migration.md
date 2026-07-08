# Plan — Icon Frontend Migration (spec 06, ADR-0015)

Date: 2026-07-09 · Mode: iterate · Overnight owner-approved run (docs → build →
codex review with verification → visual acceptance). No legacy compat (pre-release).

**Global constraints**: files ≤500 lines; extreme DRY; coral never in OS-mirror
layer (OS blue `#0067C0` exemption list); PENDING-RESX for every new string; no
dashes in user copy; `bun test` + `tsc` green at every commit; commit per slice.

## I1 — TS icon compositor core

New module `src/DeskMakeover.Web/src/icon-compositor/` (each file ≤500 lines):

- `raster.ts` — Rgba32 buffer helpers on `Uint8ClampedArray` (over-composite,
  premultiplied bilinear sample, area-average downsample, box blur, shift). Ports
  `Marks/RasterOps.cs` + `IconResampler.cs` (display path only — sub-256 ICO ladder
  stays C#, ADR-0015 D2).
- `color.ts` — sRGB↔linear LUT, Rec.601 luma, OKLab via culori (`useMode`), the
  Material mono ramp (256-entry LUT, smoothstep split curve + chroma convergence)
  and adaptive P5-P95 lightness stretch. Ports `IconColorTreatment.cs` +
  `SrgbLinear.cs`. Consumes existing `compositor/oklch.ts` where shared.
- `analysis.ts` — solid bounds, content bounds, foreground bounds, shape IoU match
  (≥0.985), MaxScaleInside (boundary points + binary search), border/inset ring
  background vote (solid plate / shaped plate / border / transparent + fill color),
  transparent-edge detection, opaque coverage. Ports `IconSilhouetteClassifier.cs`
  + `IconBackgroundAnalyzer.cs` + `BackgroundClassifier.cs` + memoization
  (`IconAnalysisCache.cs`) keyed per source bitmap. Runs once per source (CPU, 256px).
- `shapes.ts` — 13 shape paths as Path2D builders sharing `lib/shape-paths.ts`
  authoring (apple continuous-corner squircle, samsung cubic, superellipse, rounded
  rect, authored polygons) + point-in-polygon for analysis. Ports
  `IconShapeGeometry.cs`; consolidates with `lib/geometry.ts` (DRY: one home).
- `compose.ts` — the tile intelligence: pass-through if artwork already matches the
  shape / rebuild from detected plate / bare-logo tile / inscribe-no-crop /
  full-bleed; color treatment application; per-tile ink/tone resolution. Ports
  `TileRenderer.ComposeTile` + treatment sequencing.
- `filters.ts` — glass (chamfer distance field + normals + refraction warp +
  Fresnel + specular + shadow), pixel (cell downsample + candy posterize + contour
  + top light), sticker (outside die-cut border + shadow halo). Direct CPU port of
  `IconFilters.cs` — same math at any resolution, D5 tolerances apply to the GPU-free
  output too (should land near-exact).
- `marks.ts` — 7 marks (card/echo/satin/arc/fold/ring/glass-seat) + arrow glyph +
  adaptive mark tone reading post-compose luminance. Ports `Marks/*.cs` +
  `ShortcutMarkRenderer.cs` (classic-arrow real-frame path stays host-supplied:
  the scan may include an `arrowUrl` asset; mock draws the fallback arrow).
- `icon-renderer.ts` — the CPU orchestrator: per-item staged cache (source → compose
  keyed by shape/analysis → color keyed by treatment → filter → mark), display-size
  interactive render + `bakeMaster(id) → 256 RGBA` from the SAME functions, hover
  try-on renders a candidate config without committing, worker-pool offload for bake
  and >N-tile recomputes, ImageBitmap/canvas texture lifecycle.
- Deps: NONE added (deliberate deviation from the panel's pixi-filters/culori
  toolkit — CPU port wins on parity/testability/determinism; spec 06 records it).

Verify: `bun test` new suites `tests/icon-color.test.ts`, `tests/icon-analysis.test.ts`,
`tests/icon-compose.test.ts` (synthetic canvas fixtures: solid plate / bare logo /
transparent edge / pre-rounded corners / pure black / pure white); `tsc` clean.

## I2 — Bridge v2 + store rewrite

- `bridge/types.ts` — icons contract v2 per spec 06 §2 (IconItemDto with
  `sourceUrls[]` + `statusReason`, applyBakedBegin/Chunk/Commit, setLook). Delete
  render-era fields (styledUrl/originalUrl/displaySize params).
- `bridge/mock.ts` + `bridge/mock-desktop.ts` — mock serves grid + items +
  `sourceUrls` from the dev pack manifest; DELETE the styling approximation
  (renderTile/composeMarkedTile/plateFor and friends); keep wallpaper snapshot
  drawing. mock Big icon 64→96.
- `stores/icons.ts` — drop the 420ms round-trip; config/overrides mutate → renderer
  invalidate (same frame) + 400ms `setLook` persistence; per-pick undo/redo stack +
  gesture-coalesced wheel steps (pattern from `stores/wallpaper.ts`, extracted into
  `lib/history.ts` if shareable without contortion); hover try-on state; apply flow
  bakes masters + chunks ≤20 + progress; applyVersion = load config → same ceremonied
  apply flow.

Verify: `tests/icons-store.test.ts` (undo granularity: two picks = two steps; wheel
drag = one; hover never snapshots; chunk math 300→15 chunks) + existing suites green.

## I3 — Interaction layer (spec 06 §3)

- `components/canvas/icons-mirror.tsx` — tiles render from the icon-renderer
  textures (canvas/sprite host, not `<img>`); styleable gating (no ⋯/menu on
  false + info tooltip); exception corner badges; size = in-place scale +
  caption 「应用后位置由 Windows 重新排列」; press-to-peek keeps + cursor hint;
  selection/hover states per spec 06 §4.6.
- Context menus: tile (keep/follow/tint with FULL 调色盘) + empty-canvas (图标大小 /
  刷新), app-styled, owned verbs only.
- `components/panels/icons-panel.tsx` — hover try-on wiring on preset/shape/filter/
  mark swatches; 「例外 N · 清除所有例外」row; native-arrow gate softened to
  one-time explainer + 8s pause (`ceremony.tsx`); applyVersion ceremony = apply's.
- Canvas toolbar: undo/redo wired (same component as wallpaper).
- i18n: new keys PENDING-RESX in `lib/i18n/{en,zh-hans}.ts`.

Verify: bun tests for gating/badges logic; browser DOM checks in acceptance.

## I4 — Desktop chrome + mock pack (parallelizable)

- `components/canvas/taskbar-strip.tsx` — designer P0 rebuild per spec 06 §4
  (pinned row + indicator pills + tray cluster + theme acrylic + Start flag).
  OS-blue exemption respected; zero interactivity.
- Labels: double text-shadow; 2-line clamp verified.
- `scripts/dev/generate-mock-icons.mjs` — Node script, seeded RNG, spec 06 §5
  distribution + axes → 256px WebP ~120 + `manifest.json` into
  `src/DeskMakeover.Web/public/mock-icons/`. Committed output; script re-runnable.

Verify: script runs clean; pack renders in mock; visual acceptance covers variety.

## I5 — C# freeze + dead-code deletion

- Freeze banners (comment header: FROZEN 2026-07-09, ADR-0015 D3 — parity oracle +
  reserved background renderer; styles ship TS-only) on: `TileRenderer.cs`,
  `IconFilters.cs`, `IconColorTreatment.cs`, `IconShapeGeometry.cs`, `Marks/*.cs`,
  `IconStyler.cs`→delete-candidate first (below).
- Grep-verified deletion of the dead v0.9 chain: `IconStyler.cs`,
  `IconStylePlan.cs`, `MakeoverService.cs`, `PreviewItemFactory.cs`,
  `OverlayBadgeIconFactory.cs` and friends — DELETE only what a full-text reference
  scan proves unreferenced (namespace + type names across *.cs/*.csproj/*.xaml).
  `dotnet build` re-verification is Windows-batch; note in STATE.
- `IconsSession.Render.cs` PNG-per-tile path: marked for deletion at the Windows
  batch (host must still compile today's contract until applyBaked lands host-side —
  bridge v2 is web+mock first; the host controller migration is listed in F8).

## I6 — Review + acceptance + sweep

1. codex adversarial review (multi-ai solo, 30m) over the full diff; VERIFY each
   finding in code before fixing; design-as-intended findings dispositioned, real
   bugs fixed. Record dispositions in the panel review doc.
2. Visual acceptance in the browser (claude-in-chrome; fallback /chrome-cdp):
   scrub latency feel, hover try-on, exception badges, size honesty, taskbar vs
   spec values, label legibility both wallpapers, pack variety, un-editable
   tooltip, menus. Screenshots to `docs/plans/evidence/2026-07-icons-v2/`.
3. `bun test` + `tsc` full green; STATE.md sweep (journal completed items);
   CHANGELOG note; commits per slice on main (no push).

## Windows batch additions (appended to F8)

Host `icons.scan` v2 (sourceUrls incl. bin=2 + arrowUrl) · `icons.applyBaked`
chunked endpoint → GeneratedIconStore · golden fixture generation (oracle run) +
parity execution · discovery fix (shell-namespace scan surfaces Recycle Bin; This
PC/Network CLSID writers) · delete `IconsSession.Render.cs` PNG path + dead v0.9
files' final `dotnet build && dotnet test` verification.
