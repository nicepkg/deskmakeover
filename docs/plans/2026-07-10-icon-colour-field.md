# Plan — Icon Colour-Field Default (ADR-0016)

Scope: implement the 满彩 Field colour mode + preset lineup rework, web side (Mac
mock loop). C# TileRenderer sync moves to F8 (see Global constraints). Panel:
`docs/reviews/2026-07-10-icon-findability-panel.md`.

> **Status 2026-07-10:** T1-T4 + T6(presets/mock) SHIPPED; the constants table
> below is SUPERSEDED by the as-accepted recipe v7 (ADR-0016 amendment / spec 02
> §Default Composition): knockout deleted (owner iron law — subjects never
> recoloured), Vivid plates C[0.09,0.12]@L0.87, plated clamp [0.60,0.80] with
> neutral exemption, pale contrast-target lane + ring halo, box 36/256, hue
> spread 12° min-gap in ±18° cap. Designer-seat acceptance: PASS. Owed: T5
> (kind families/affordances/kindShapes), T6 colour-row UI (满彩 swatch + band
> depth), T7 glass rim, T8 letter badge, D4 corpus ΔE test, T9 adversarial
> review sweep.

## Global constraints

- WYSIWYG: preview and bake share `renderTile`; anything per-icon (seed, plate,
  kind) must flow through BOTH the render and bake worker messages.
- Determinism: the hue-spread pass is a pure function of the scanned item set
  (sorted by id) — no randomness, no time. Same desktop → same plates → bake
  reproducible.
- The worker pool shards sources by id: cross-icon knowledge (hue spread) is
  computed on the MAIN thread from per-source seeds reported at decode time.
- `FULL_BLEED_FOREGROUND_FRACTION` 0.88 → 0.82 and the Field content padding are
  web-side law now; **F8 owes the C# TileRenderer re-port BEFORE parity goldens**
  (geometry/oracle direction is already web→C#, ADR-0015 amendment).
- 500-line law; PENDING-RESX for new strings; banned-colors test untouched
  (plates are user content, not chrome).

## Constants (decided, not placeholders)

| Name | Value | Where |
|---|---|---|
| `FIELD_CONTENT_PADDING_FRACTION` | 26/256 (~80% linear glyph) | compose.ts (Field lanes only) |
| `FULL_BLEED_FOREGROUND_FRACTION` | 0.82 (was 0.88) | compose.ts (global) |
| Vivid band | L clamp [0.58, 0.72] · chroma clamp [0.09, 0.145], gamut-fit loop | color.ts `fieldTone` |
| Quiet band | L 0.91 fixed · chroma target 0.055 clamp [0.04, 0.07] | color.ts `fieldTone` |
| Knockout ink | plate L' < 0.68 → near-white (L 0.985, C 0.015, seed hue); else deep hue ink (L 0.24, C 0.06) | color.ts `fieldInk` |
| Fidelity gate | hue circular dispersion > 0.35 OR degenerate segment (<2% mask) OR no seed | analysis.ts `hueDispersion` + compose.ts |
| Hue spread | cluster ΔH < 10°; spread cluster-mates ±8° steps around mean, id-sorted; cap ±16° | hue-spread.ts |
| ΔE net | min pairwise plate distance (OKLab) ≥ 0.03 across the synthetic corpus after spread | tests |

## Tasks

**T1 — `analysis.ts`: `dominantColor` + `hueDispersion`.** Chroma-weighted OKLab
hue histogram over `segmentSubject(c).mask` pixels (skip a<128, skip
near-neutral chroma<0.03); returns the histogram-peak bucket's chroma-weighted
mean colour `{r,g,b}` or null (no qualifying pixels → the no-hue tail).
`hueDispersion` = 1 − |Σ chroma·e^{iθ}| / Σ chroma (circular). Both memoized in
the existing WeakMap pattern. Tests: solid-colour canvas → its colour; two-tone
gradient → dispersion high; gray canvas → null.

**T2 — `color.ts`: `fieldTone(seed, band)` + `fieldInk(plate, seed)`.** Band
normalisation per the constants table, gamut-fit like `monoTone` (retry ×0.82).
Tests: known seeds land inside band; ink contrast (OKLab ΔL ≥ 0.4 vs plate).

**T3 — `compose.ts`: `colorMode 'Field'` branch.** Order inside `composeTile`
before the shape===None branch (Field implies a plate; None+Field falls back to
classic None). Lanes: (knockout) seed ∧ dispersion ≤ 0.35 ∧ mask ≥2% → plate =
`fieldTone(spreadSeed, band)`, subject = mask layer recoloured to `fieldInk`,
drawn at Field padding box; (fidelity) otherwise → same plate colour, artwork
drawn as-is via `composeFromPlate` geometry; (no-hue tail) seed null → kind
family plate (T5; until T5: neutral `monoTone(bandL, 0.4, seedFallback)` slate).
`ConfigDto`: `colorMode` union += 'Field'; new `fieldBand: 'Vivid' | 'Quiet'`
(default Vivid); `tileStyleKey` += fieldBand + the resolved per-item plate hex
is NOT keyed (per-item, epoch-invalidated). Tests: lane decision table; padding;
white-fallback absent in Field.

**T4 — seed pipeline + `hue-spread.ts` + ΔE test.** Worker `sourceReady` ack
gains `seed: {r,g,b} | null`; `render`/`bake` messages gain `fieldPlate: string
| null` + `kindBucket`. `IconCompositor.getTile/bakeMasterPng` accept
`opts { fieldPlate, kindBucket }`. Store collects seeds; when a scan's sources
are all ready, `computeHueSpread(entries)` (pure, id-sorted) yields per-id plate
hex via `fieldTone`; if any plate differs from the raw-seed first paint,
`invalidateAll()` once. Folder/System buckets use the family plate (unified);
File uses doc-type seed else family; App per-icon. ΔE separability bun test over
`public/mock-icons/manifest.json` corpus (threshold table above).

**T5 — kind families + affordances (`field-kind.ts`).** Family plate hues:
Folder amber family (seed #E8A93C), System slate (seed #6B7683), File cool
gray-blue (seed #7B93AE) — all pushed through `fieldTone` so they sit in the
same band. Affordances drawn between plate fill and subject: folder tab (top
strip, same hue L−0.08, width 46%, height 12%, radius follows shape), document
dog-ear (top-right folded triangle, L+0.06 over L−0.06 shadow line, 22% side).
`kindShapes: boolean` on ConfigDto (default false): when true, Folder→Bookmark,
File→Tile, System→Circle shape assignment upstream of `shapeMask` (opt-in
four-shape split, D2). Tests: affordance pixels present/absent per bucket flag.

**T6 — preset lineup + colour row UI.** `PRESET_CONFIGS` → `field` (默认:
Apple + Field/Vivid) · `minimal` (极简白: Apple + Original + plateColor
#FFFFFF) · `quiet` (安静: Apple + Field/Quiet) · `faithful` (原彩保真: Apple +
Original + plateColor null — the ONLY home of the white fallback). `candy`/`bw`
leave the preset row (glass filter + BW colour stay reachable on their axes).
Colour row: 满彩 swatch FIRST (default), then 原彩/黑白/单色; Field selected
reveals 鲜明/柔和 depth segmented (mirrors monoStyle grammar). Strings zh+en
PENDING-RESX. Preset card thumbnails re-render automatically (they render live
configs).

**T7 — glass rim rework (`filters.ts`).** 玻璃 becomes a rim highlight (edge
band specular + subtle inner top gloss), never a full-tile desaturating wash;
keep the name. Visual acceptance mandatory (render before/after on the corpus).

**T8 — letter-badge tail fallback (`marks.ts` ext).** Tail-only (seed null):
bottom-right mini-plate with the label's first grapheme, family-coloured.
Deferred-allowed; ships behind the Field fallback only.

**T9 — gates.** `./node_modules/.bin/tsc -b` clean; full bun suite green
(297+new); browser visual acceptance: default vs 极简白/安静/原彩保真 over the
real-icon dev pack (screenshots to evidence dir); adversarial review
(multi-ai/codex) over the engine diff; STATE checkpoint + journal sweep.

## Sequencing

T1 → T2 → T3 (engine core, testable sync-fallback path) → T4 (pipeline + ΔE)
→ T6 (presets/UI, user-visible) → T5 (kind) → T9; T7/T8 independent tails —
if deferred past this session they stay on STATE as owed items.
