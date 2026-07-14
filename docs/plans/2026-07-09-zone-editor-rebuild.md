# Plan — Zone editor rebuild (spec 04 v2.0, ADR-0014)

> **Status:** ✅ EXECUTED — historical build record (the Pixi zone-editor rebuild; host I/O later
> moved to Rust per ADR-0019). See `docs/journal/2026-07.md`.

Mac/mock loop builds everything except host handoff (F8). Vertical slices; each
lands green (`bun test` + tsc) before the next. No backward compat (pre-release).

## Global constraints

- ONE compositor renders material live AND baked; chrome stays DOM. Nothing in
  the editor may render zone material outside the compositor.
- Zone identity: `id: string` (nanoid-style from crypto.randomUUID), never index
  keys. Look mutations preserve ids.
- All parameters authored in desktop-pixel space; compositor scales uniformly.
- Files ≤500 lines; accent/coral rules; no 破折号 in strings; PENDING-RESX for
  new i18n keys.
- Dependency adds go into package.json with exact installed versions.

## Z1 — Compositor core (`src/DeskMakeover.Web/src/compositor/`)

- `types.ts`: CompositorSource {bitmap: ImageBitmap, width, height} · ZoneVisual
  (projected px rect + material params) · reserved SourceProvider/OutputTarget
  interfaces (ADR-0014 D2).
- `sampling.ts`: region OKLCH mean (L̄,C̄,H̄) over downsampled source (≤64×64 per
  zone), tone decision with hysteresis; reuse `lib/color` OKLCH helpers if
  present, else implement (sRGB↔OKLab↔OKLCH).
- `material.ts`: Adaptive Frost recipe (spec §4.1) → concrete fills/lines per
  zone from look + sample. Pure, unit-tested (fixtures: pale→dark tone, accent
  distinctness, outline forces chip).
- `title-chip.ts`: Canvas2D rasterizer for chip+emoji+text (spec §4.2) → texture
  source; measures via canvas metrics; overhang lane vs in-panel fallback logic.
  Pure layout part unit-tested.
- `renderer.ts`: pixi.js v8 Application over the module canvas; source sprite
  (uploaded once per source change); per-zone: blurred sub-sprite (BlurFilter,
  σ scaled) masked by rounded rect + fill/highlight/contour/chip layers; renders
  from `look` every invalidation (gesture frames) — target ≤5ms/frame at
  viewport res.
- `bake.ts`: same scene graph at native res → `extract`/OffscreenCanvas →
  PNG blob. Worker preferred; main-thread fallback acceptable v1 (one frame,
  masked by the apply wave).
- Dependency: `pixi.js@^8` (exact version pinned in package.json).
- Verify: new compositor unit tests green; debug gallery page renders a fixture
  look over the mock scene in-browser.

## Z2 — Bridge + store rewiring

- `bridge/types.ts`: ZoneDto gains `id, accent, emoji, tone('Auto'|'Light'|
  'Dark'), outline: boolean, cornerRadius, titleSize('S'|'M'|'L'),
  fontFamily?`; look-level title block reduced (per-zone now); methods:
  `wallpaper.getSource` → {width,height,pngBase64|sharedbuffer}, `wallpaper.
  applyBaked` (png bytes) replaces recompose-frame streaming.
- `bridge/mock.ts` + `mock-desktop.ts`: serve the scene bitmap as source;
  applyBaked stores blob URL (debug-inspectable); zones finally VISIBLE on Mac.
- `stores/wallpaper.ts`: delete debounce/revision/frame plumbing; look mutations
  just bump a version the compositor watches; apply = bake → applyBaked → wave
  trigger; undo/redo + interaction coalescing kept; suppress any residual
  composing flag during open gestures.
- Verify: bun tests (store history, id stability) green; mirror shows composed
  material in browser.

## Z3 — Editor interactions (wallpaper-mirror/zone-layer)

- Same-frame material (compositor invalidate on every mutateZone), rubber-band
  = forming material + W×H badge, auto-select + auto-rename on release.
- Guides rework: only snapped edges; magnetism ≤0.35 cell; span lines +
  equal-gap ticks; overlap warn-wash; pulse-on-release only.
- Stable keys + AnimatePresence exit; Alt-drag duplicate; visible undo/redo in
  canvas toolbar; delete toast with 撤销 action.
- New chrome: double-stroke selection, white-core handles, corner-only <5 cells.
- Verify: zone-math/store tests + interaction state tests; manual browser pass.

## Z4 — Panel rebuild (wallpaper-panel + zone-list)

- 图标清晰度 rename (i18n zh/en, PENDING-RESX); kill 4-style chips; per-zone
  editing block: opacity, tone, outline, accent swatches, corner, title row
  (emoji picker + S/M/L + font popover); explicit 应用到全部分区; list rows show
  material+accent swatch.
- Verify: tests + browser screenshots.

## Z5 — Presets + apply ceremony

- `panels/paper-presets.tsx`: 4–6 curated presets (data module with semantic
  names/emoji/accent/layout in cell fractions); gallery thumbnails rendered by
  the compositor at thumb res over the CURRENT source; replace-confirm.
- Apply wave (coral sweep + staggered bloom, reduced-motion path) + DoneCard
  最后一步 + [去桌面整理] (shell.minimize).
- Empty-state leads with the gallery + one anchored coach line.
- Verify: browser run-through + screenshots to evidence dir.

## Z6 — Tests, fixtures, docs sweep

- TS bake fixtures pin compositor output (hash/SSIM on small fixtures); spec §7
  computed tests (chip contrast, tone auto, accent distinctness).
- STATE.md sweep; F8 list gains: host getSource/applyBaked, delete C# renderer
  + tests after parity fixtures run on Windows, parity gate ΔE<2/SSIM>0.99.
- Full suite green; dark/zh regression screenshots.
