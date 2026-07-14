# Plan — Zone material lineup, title system & editor UX (round 3)

Spec: `docs/specs/04-wallpaper-module.md` §2/§3/§4.1/§4.2 (round-3 amendments).
Binding detail: `docs/reviews/2026-07-15-zone-material-title-ux.md`.
Gates after every phase: `bun run tsc -b` · `bun test ./tests/` · playwright
harness visual check for renderer-visible phases. Final: cross-vendor review.

## Global constraints

- Enums are TS-owned (`src/bridge/types.ts`) — NOT in the Rust contract; no
  bindings churn. Persisted looks migrate on load (TS store).
- All recipes stay pure data in `material.ts`; zone-node renders; bake parity
  (same recipe at k=1) must hold for every new finish.
- Files ≤500 lines; no new render pipeline for Glaze/Paper/Float (reuse
  fill/gradient/blur/innerGlow/shadow hooks; Paper's noise = one tiny generated
  canvas texture, the only new primitive).

## Phases

### P1 — Params & switch semantics (no type changes)
1. `wallpaper-zone-inspector.tsx`: opacity slider `min={0} max={100}`; corner
   slider `min={CORNER_MIN} max={CORNER_MAX}` step 2.
2. `material.ts`: `CORNER_MIN=0`, `CORNER_MAX=60`;
   `MATERIAL_RADIUS_DEFAULT.LiquidGlass=44`; glass tint =
   `clamp(zone.fillOpacity ?? 0, 0, 1)` (default 0 = pure refraction); remove
   `clamp(zone.cornerRadius, 8, 28)` → `clamp(zone.cornerRadius, CORNER_MIN, CORNER_MAX)`.
3. `zone-node.ts`: render invariant `radius = min(paint.cornerRadius, min(w,h)/2)`.
4. Inspector material-switch handler → touched-model: per axis (titleStyle /
   cornerRadius / fillOpacity): `current == MATERIAL_*_DEFAULT[old]` ⇒ adopt
   `MATERIAL_*_DEFAULT[new]`, else keep; titleStyle legality fallback via
   `allowedTitleStyles(new)`. (fillOpacity default = `OPACITY_DEFAULTS[old][tone]`
   vs null-sentinel: treat `null` as untouched.)
5. Verify: gates + glass zone at radius 44/tint 0 in harness.

### P2 — Types & migration
1. `bridge/types.ts`: `ZoneMaterial = 'Frost'|'LiquidGlass'|'Glaze'|'Paper'|'Float'|'Outline'`;
   `ZoneTitleStyle = 'None'|'Etched'|'Chip'|'Bare'|'Bar'`.
2. `stores/wallpaper.ts` load path: migrate `Luminous→Frost`, `Solid→Paper`,
   `Halo→Float`, `Tab→Chip` (one-way, silent). Sweep `zone-presets.ts`,
   ceremony/demo fixtures, tests for retired names.
3. i18n en/zh: `Material_Glaze 釉色 · Material_Paper 素笺 · Material_Float 浮屿 ·
   TitleStyle_None 无 · TitleStyle_Etched 冰签`; delete retired keys.

### P3 — New material recipes (`material.ts` + `zone-node.ts`)
1. **Glaze**: fill from ACCENT (not wallpaper hue), chroma 0.10–0.12 (per-finish
   chroma table exemption), L light 0.72 / dark 0.32, α default 0.5, frost blur
   on, accent innerGlow (reuse hook), 1px top highlight.
2. **Paper**: opaque warm (hue 60–90) off-white L0.95 / off-charcoal L0.18, no
   blur, letterpress (1px top-light + 1px bottom-dark inner strokes), noise
   dither: one 64×64 generated canvas tiled at α≈0.04 masked to the panel.
3. **Float**: fill α 0.18, blurSigma 0, baked drop shadow ALWAYS on (blur
   cellHeight×0.3, offsetY cellHeight×0.10, α≈0.22), micro top highlight, no
   contour.
4. Remove Luminous/Solid/Halo recipes + gradient/halo dead branches left unused;
   keep shared hooks. Update OPACITY_DEFAULTS/fillL/chroma tables.
5. Verify: harness screenshots of all six finishes over the ribbon wallpaper.

### P4 — Title system (`title-chip.ts` + material pairing + inspector)
1. **Etched**: frosted lozenge — white α0.16 roundRect + 1px top-light
   (α≈0.55) + 1px bottom-dark (α≈0.35) bevel, adaptive ink, chip lanes/layout.
2. **None**: `titleLayout` returns hidden sentinel; zone-node skips title;
   overhang/reserveFirstRow false.
3. Retire Tab case; `MATERIAL_TITLE_DEFAULT`: LiquidGlass→Etched, others Chip;
   `allowedTitleStyles`: None first for every material; Halo/Outline branch
   updated for new lineup (Outline forces visible title? — Outline keeps
   requiring a title style but None is still allowed; drop the force rule only
   if tests say otherwise).
4. Inspector: title-style row renders None as first swatch (slash-circle
   dialect); selecting None collapses size segmented + font row.
5. Verify: harness — glass zone shows Etched by default; None hides title.

### P5 — WYSIWYG pickers (`wallpaper-panel-popovers.tsx` + inspector)
1. MaterialSwatch → wallpaper-crop tile: `<img>` crop (preset-popover pattern) +
   per-finish DOM approximation overlay, ≥40px, rounded; unified selected token
   (ring-coral); persistent caption line under the row showing the selected
   material name.
2. TitleStyleSwatch: add None (NoneMini) + Etched glyphs; ≥40px hit areas;
   same selected token.
3. Remove EmojiPicker from the title-style row sub (moves in P6).

### P6 — Emoji beside title + zone context menu
1. `wallpaper-zone-list.tsx`: row emoji span → EmojiPicker trigger (compact),
   stopPropagation, works for emoji-less zones (slash placeholder on hover).
2. Context menu in the wallpaper canvas: host `onContextMenu preventDefault`;
   right-click zone = select + open TileMenu-dialect menu at cursor
   (icons-mirror.tsx `TileMenu`/`MenuRow` grammar): 重命名(startRename) ·
   改emoji(EmojiPicker popover) · 隐藏标题/显示标题(titleStyle None toggle) ·
   复制分区(duplicateZone) · 应用样式到全部(applyToAllZones) ┊ 删除分区(red,
   removeZone, no confirm). Guard: suppressed during active gesture.
3. Verify: harness — right-click opens menu; delete works with undo toast.

### P7 — Ship gates
1. Full gates + banned-colors test sweep (Glaze chroma exemption may need the
   test's blue/violet band assertion checked — Glaze uses ACCENT hues which are
   already curated outside the banned band).
2. Playwright: six-material board + five-title board screenshots; judge against
   the review doc's axis definitions.
3. Cross-vendor review (`/multi-ai` codex) on the full diff; fix; STATE.md
   checkpoint + journal sweep.
