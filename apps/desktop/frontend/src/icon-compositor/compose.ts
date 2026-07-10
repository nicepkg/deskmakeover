// FROZEN 2026-07-10 (ADR-0019): parity oracle for the Rust port. No new styles,
// no fixes except oracle corrections. Deleted after M6 certification.

import type { ConfigDto, IconShape } from '@/bridge/types'
import type { ContentBounds } from './analysis'
import { analysis, boundsH, boundsW, colorDistance } from './analysis'
import type { Raster } from './raster'
import { fromRgbInt, hexToInt, makeRaster, overAt, shapeMask, WHITE } from './raster'
import { shapeContains } from './shapes'
import { fieldShadowTone, luminance, monoMapAdaptive, monoRamp, neutralContrastTone, themedContrastTone, transformPixelInPlace } from './color'
import { segmentSubject } from './segment'
import { iconProfile } from './profile'
import { drawScaled, sampleBilinear } from './sampling'
import type { MarkContext } from './marks'
import { drawClassicArrow, resolveMark } from './marks'
import { applyFilter } from './filters'

// The tile composer — 1:1 port of the frozen C# oracle (TileRenderer.cs,
// ADR-0015 D3). ONE function renders the on-screen preview AND the 256 bake
// master, so what you see is what lands on the desktop (same functions, two
// resolutions — the wallpaper compositor's law applied to icons).

// Reference StyleBitmap default: the logo occupies the centre ~67% of the tile.
const CONTENT_PADDING_FRACTION = 42 / 256
// A logo that fills ≥82% of its own plate is really full-bleed artwork
// (ADR-0016 D6: was 0.88 — more full-bleed originals now stay full-bleed;
// the C# TileRenderer re-port owes this constant at F8, before parity goldens).
const FULL_BLEED_FOREGROUND_FRACTION = 0.82

// ---- 满彩 Field mode (ADR-0016 D1; recipe v5 — see color.ts note) ----
// ~72% linear: enough colour signal without letting messy source artwork
// dominate the grid (owner steering: 80% read as chaos, 67% starved colour).
const FIELD_CONTENT_PADDING_FRACTION = 36 / 256
// Kind FAMILY COLOURS were removed (owner law 2026-07-10: 「没必要执着提取
// 非灰黑白的主题色」— assigned hues on gray artwork were exactly the
// 「背景颜色根本不是主题色」complaint). Grouping is EMERGENT now: yellow
// folders derive amber plates from their own art; gray art gets the neutral
// lightness-contrast board. Kind still drives the opt-in shape split below.

/** Per-icon inputs resolved OUTSIDE the tile (cross-icon hue spread, kind). */
export interface RenderOpts {
  /** Hue-spread-adjusted seed colour (hex). null/absent = derive from artwork. */
  fieldSeed?: string | null
  kindBucket?: 'App' | 'Folder' | 'File' | 'System' | null
}
/** Shapes whose usable area sits far under the square: full-bleed content
 *  spills past the pinched edges (owner call 2026-07-09: Diamond/Flower
 *  "中间主体快溢出来了"), so artwork is inscribed via maxScaleInside exactly
 *  like Circle. Near-square shapes keep full bleed. */
const INSCRIBE_SHAPES: ReadonlySet<IconShape> = new Set(['Circle', 'Diamond', 'Flower', 'Pebble'])

/** Breathing room applied on top of the TRUE maximum fit — at the max the
 *  silhouette touches the shape edge, so pinched shapes need real air
 *  (owner 2026-07-09: square Photos plate kissing the Diamond's edges). */
const INSCRIBE_MARGINS: Partial<Record<IconShape, number>> = {
  Circle: 0.94,
  Pebble: 0.88,
  Diamond: 0.82,
  Flower: 0.82,
}

function inscribeMargin(shape: IconShape): number {
  return INSCRIBE_MARGINS[shape] ?? 0.94
}

// Largest centred axis-aligned square (side as a fraction of the box) whose
// corners AND edge midpoints sit inside the shape — the glyph/bare-logo
// content cap for pinched shapes (a 67% keyline box overflows a Diamond,
// whose axis-aligned square maxes out near 52%).
const squareFitCache = new Map<IconShape, number>()

function maxCentredSquareFactor(shape: IconShape): number {
  let f = squareFitCache.get(shape)
  if (f === undefined) {
    let lo = 0
    let hi = 1
    for (let i = 0; i < 24; i++) {
      const mid = (lo + hi) / 2
      const pts: Array<[number, number]> = [
        [1 - mid, 1 - mid], [1 + mid, 1 - mid], [1 - mid, 1 + mid], [1 + mid, 1 + mid],
        [1, 1 - mid], [1, 1 + mid], [1 - mid, 1], [1 + mid, 1],
      ]
      if (pts.every(([x, y]) => shapeContains(shape, x, y, 2))) lo = mid
      else hi = mid
    }
    f = lo
    squareFitCache.set(shape, f)
  }
  return f
}

/** The content keyline for a shape: square-ish shapes keep the classic ~67%
 *  box; pinched shapes cap at their inscribed square with breathing room. */
function contentBox(shape: IconShape, cardSize: number): number {
  const inner = innerBox(cardSize)
  if (!INSCRIBE_SHAPES.has(shape)) return inner
  return Math.min(inner, Math.max(8, Math.round(cardSize * maxCentredSquareFactor(shape) * inscribeMargin(shape))))
}

/**
 * Render one styled tile at `size` px. `artwork` is the 256px source raster;
 * `config` the resolved style (per-icon overrides already folded in by the
 * caller). `showOriginal` returns the untouched artwork plus the classic arrow.
 */
export function renderTile(
  artwork: Raster,
  config: ConfigDto,
  isShortcut: boolean,
  showOriginal: boolean,
  size: number,
  opts?: RenderOpts,
): Raster {
  if (size <= 0) throw new RangeError('size must be positive')
  const tint = hexToInt(config.tint)

  if (showOriginal) {
    // One arrow ratio everywhere (owner-approved): originals and styled tiles
    // agree. The web fallback arrow sizes itself from the tile (C# fallback
    // parity); the host's real overlay frame arrives with the Windows batch.
    const original = buildOriginalCard(artwork, size)
    if (isShortcut) drawClassicArrow(original, size)
    return original
  }

  // Shape arrives RESOLVED (ADR-0017): the per-type ladder + shortcut layer
  // are folded upstream by effectiveTileConfig — the tile renders one config.
  const shape = config.shape
  const tileAlpha = shapeMask(shape, size, size, 0, 0)

  const mark = isShortcut && config.distinction === 'Mark' ? resolveMark(config.markStyle) : null

  // Geometry-only context for inset/carve; the REAL adaptivity context is
  // built after composition from the composed tile.
  const geometryCtx: MarkContext = {
    size,
    shape,
    luminance: 0.5,
    markColor: config.markColor ? hexToInt(config.markColor) : null,
    tileAlpha,
  }

  const pad = mark ? mark.cardInset(geometryCtx) : 0
  const cardSize = size - 2 * pad
  let cardMask = shapeMask(shape, size, cardSize, pad, pad)
  const carves = mark?.carvesCard === true
  if (carves && mark) {
    cardMask = Float64Array.from(cardMask) // cached masks are shared — clone before carving
    mark.carveCard(cardMask, geometryCtx)
  }

  const { tile, passThrough } = composeTile(artwork, size, pad, cardSize, shape, config, tint, opts)

  if (!passThrough || carves) applyCoverage(tile, cardMask)

  if (config.filter !== 'None') applyFilter(tile, size, config.filter, config.subject, tint)

  // Marks adapt to the tile the user actually SEES. Free-form tiles hand
  // marks the icon's REAL alpha silhouette instead of a phantom box/Apple
  // sibling (owner call 2026-07-09: marks floated in empty space and never
  // followed 异形 icons).
  const markAlpha = shape === 'None' ? alphaFieldOf(tile) : tileAlpha
  const ctx: MarkContext = {
    size,
    shape,
    luminance: composedLuminance(tile),
    markColor: config.markColor ? hexToInt(config.markColor) : null,
    tileAlpha: markAlpha,
  }

  const target = makeRaster(size)

  if (mark && mark.placement === 'behind') mark.render(target, cardMask, ctx)
  compositeOver(target, tile)
  if (mark && mark.placement === 'over') mark.render(target, cardMask, ctx)

  if (isShortcut && config.distinction === 'Keep') {
    drawClassicArrow(target, size)
  }

  // No final whole-tile clip (ADR-0006 refined): badge marks legitimately
  // overhang the shape, exactly like the prototype's unclipped overlays.
  return target
}

/** The composed tile's own alpha as a coverage field (free-form mark geometry). */
function alphaFieldOf(tile: Raster): Float64Array {
  const n = tile.width * tile.height
  const field = new Float64Array(n)
  for (let i = 0; i < n; i++) field[i] = tile.data[i * 4 + 3] / 255
  return field
}

/** 保留原样 / peek: the artwork as-is in the card box (full mask, Original mode). */
function buildOriginalCard(artwork: Raster, size: number): Raster {
  const card = makeRaster(size)
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const u = (x + 0.5) / size
      const v = (y + 0.5) / size
      const [r, g, b, a] = sampleBilinear(artwork, u, v)
      if (a === 0) continue
      const i4 = (y * size + x) * 4
      card.data[i4] = r
      card.data[i4 + 1] = g
      card.data[i4 + 2] = b
      card.data[i4 + 3] = a
    }
  }
  return card
}

interface ComposeResult {
  tile: Raster
  passThrough: boolean
}

/** TileRenderer.ComposeTile — shape intelligence + colour treatment. */
function composeTile(
  artwork: Raster,
  size: number,
  pad: number,
  cardSize: number,
  shape: IconShape,
  config: ConfigDto,
  tint: number,
  opts?: RenderOpts,
): ComposeResult {
  const content = makeRaster(size)

  // A canvas with no visible pixels must render NOTHING.
  const contentB = analysis.contentBounds(artwork)
  if (analysis.solidBounds(artwork) === null && boundsW(contentB) <= 1 && boundsH(contentB) <= 1) {
    return { tile: content, passThrough: true }
  }

  // ---- 派生底板 lane (ADR-0018): subject Original × plate 随图标(derived).
  // 本色 (plateFallback 'white') takes the CLASSIC pipeline below instead —
  // byte-identical to the old 原彩保真 (anchored own boards, white fallback,
  // no silhouette shadows). 原始外形 (None) has no plate to colour.
  // Fixed plates (global pick or a type pin like the folder gold / the File
  // paper board) ride the SAME lane: composeField's user-plate branch keeps
  // the silhouette shadow + 72% box, so a white page on a warm paper plate
  // still separates (owner disposition A, 2026-07-10). Only 本色 (white
  // fallback) takes the classic faithful pipeline.
  if (config.subject === 'Original' && config.plateFallback !== 'white' && shape !== 'None') {
    composeField(artwork, content, size, pad, cardSize, shape, config, opts)
    return { tile: content, passThrough: false }
  }

  let passThrough = false

  // 背景色 override: Original swaps the SYNTHESIZED plate fill in the
  // branches below; Mono goes through the LAYERED path instead; BW keeps it
  // inert until v2 (chief-UI/UX matrix 2026-07-09).
  const plateOverride = config.plateColor ? fromRgbInt(hexToInt(config.plateColor)) : null
  const plate = config.subject === 'Original' ? plateOverride : null

  // ---- LAYERED Mono (极致单色 + custom plate): plate flat, subject on top ----
  // Flat: the segmented subject in ONE flat tint on a flat plate (owner
  // feature). Tonal + custom plate: subject keeps the tonal ramp, plate takes
  // the chosen colour raw. Auto plate = the ramp's light end (matches what a
  // white plate maps to, so Auto stays visually continuous with the classic
  // path). Degenerate segmentations fall through to the classic whole-tile map.
  if (config.subject === 'Mono' && shape !== 'None' && (config.monoStyle === 'Flat' || plateOverride)) {
    const layer = monoSubjectLayer(artwork, config.monoStyle === 'Flat' ? tint : null)
    if (layer) {
      if (config.monoStyle !== 'Flat') monoMapAdaptive(layer, tint)
      const fill = plateOverride ?? monoRamp(1, tint)
      fillRegion(content, size, pad, cardSize, fill.r, fill.g, fill.b)
      drawCentred(layer, analysis.contentBounds(layer), content, size, pad, cardSize, contentBox(shape, cardSize))
      return { tile: content, passThrough: false }
    }
  }

  if (shape === 'None') {
    // 原始外形: the icon keeps its own silhouette — normalized, no plate, no clip.
    const free = analysis.contentBounds(artwork)
    const [fw, fh] = fit(boundsW(free), boundsH(free), cardSize)
    drawScaled(artwork, free, content, size, pad + Math.trunc((cardSize - fw) / 2), pad + Math.trunc((cardSize - fh) / 2), fw, fh)
    passThrough = true
  } else if (analysis.matchesShape(artwork, shape) && analysis.solidBounds(artwork) !== null) {
    const solid = analysis.solidBounds(artwork)!
    const [w, h] = fit(boundsW(solid), boundsH(solid), cardSize)
    drawScaled(artwork, solid, content, size, pad + Math.trunc((cardSize - w) / 2), pad + Math.trunc((cardSize - h) / 2), w, h)
    passThrough = true
  } else {
    // Plate detection, best-first: the edge/full-bleed detector, THEN the
    // segmentation field — a squarish/round silhouette with a uniform surround
    // distinct from its subject (owner 2026-07-09: an inset rounded-square logo
    // like Twitter has NO canvas-edge plate, but its own blue IS the background;
    // fill with THAT, not white). White stays the fallback only when neither
    // finds a plate. Runs upstream of colour + filter, so both inherit the fix.
    const bg = analysis.tryDetectBackground(artwork) ?? segmentSubject(artwork).field ?? null
    if (bg) {
      composeFromPlate(artwork, content, size, pad, cardSize, shape, plate ?? bg)
    } else if (analysis.hasTransparentEdges(artwork)) {
      // Bare / irregular logo, genuinely no plate: white tile + centred content.
      const fill = plate ?? WHITE
      fillRegion(content, size, pad, cardSize, fill.r, fill.g, fill.b)
      drawCentred(artwork, analysis.contentBounds(artwork), content, size, pad, cardSize, contentBox(shape, cardSize))
    } else if (INSCRIBE_SHAPES.has(shape)) {
      // Opaque icon, no readable background: inscribe on white, never crop.
      const fill = plate ?? WHITE
      fillRegion(content, size, pad, cardSize, fill.r, fill.g, fill.b)
      inscribeContent(artwork, content, size, pad, cardSize, shape)
    } else {
      drawScaled(
        artwork,
        { left: 0, top: 0, right: artwork.width, bottom: artwork.height },
        content, size, pad, pad, cardSize, cardSize,
      )
    }
  }

  if (config.subject === 'Mono') {
    monoMapAdaptive(content, tint)
    return { tile: content, passThrough }
  }
  if (config.subject === 'BlackWhite') {
    const d = content.data
    for (let i = 0; i < d.length; i += 4) {
      if (d[i + 3] > 0) transformPixelInPlace(d, i, 'BlackWhite', tint)
    }
  }
  return { tile: content, passThrough }
}

/** Field-lane content keyline: the enlarged (~80% linear) box, inscribed for
 *  pinched shapes exactly like the classic keyline. */
function fieldContentBox(shape: IconShape, cardSize: number): number {
  const inner = Math.max(8, cardSize - 2 * Math.round(cardSize * FIELD_CONTENT_PADDING_FRACTION))
  if (!INSCRIBE_SHAPES.has(shape)) return inner
  return Math.min(inner, Math.max(8, Math.round(cardSize * maxCentredSquareFactor(shape) * inscribeMargin(shape))))
}

/**
 * 满彩 Field composition (ADR-0016 D1, recipe v7 — designer acceptance round).
 * Iron law (owner 2026-07-10): **subject pixels are NEVER recoloured** —
 * separation comes from the plate and soft shadows, never from re-inking.
 *  - plated sources keep their OWN plate colour (chromatic plates clamped
 *    into the light window, neutral whites exempt) — the field's anchors;
 *  - bare artwork keeps its pixels on a SAME-HUE coloured plate (the plate
 *    now genuinely carries colour — designer FAIL item 1), lifted by a
 *    macOS-dock contact shadow;
 *  - NEAR-WHITE artwork gets a contrast-target plate (subject mean − 0.20)
 *    plus a 360° ring halo instead of a drop shadow, so pale art separates
 *    while the field stays light;
 *  - the no-hue tail gets the fallback family plate — never a white board.
 * `plateColor` overrides are deliberately inert here — the band governs.
 */
function composeField(
  artwork: Raster,
  content: Raster,
  size: number,
  pad: number,
  cardSize: number,
  shape: IconShape,
  config: ConfigDto,
  opts?: RenderOpts,
): void {
  const band = config.plateBand
  const box = fieldContentBox(shape, cardSize)
  // ONE metadata extraction feeds every branch (owner DRY order): the
  // profile answers classification, own background, subject colour and both
  // lightnesses — no branch re-derives anything.
  const profile = iconProfile(artwork)

  // Step 1: a filled standard square is already a complete tile — clip only.
  if (profile.kind === 'fullSquare') {
    drawScaled(
      artwork,
      { left: 0, top: 0, right: artwork.width, bottom: artwork.height },
      content, size, pad, pad, cardSize, cardSize,
    )
    return
  }

  // 背景色 override IS effective in Field (owner 2026-07-10, reversing the
  // short-lived inert rule): a hand-picked plate unifies the whole desktop.
  const userPlate = config.plateColor ? fromRgbInt(hexToInt(config.plateColor)) : null
  if (userPlate) {
    if (profile.kind === 'ownBoard' || !profile.transparentEdges) {
      composeFromPlate(artwork, content, size, pad, cardSize, shape, userPlate, box)
      return
    }
    fillRegion(content, size, pad, cardSize, userPlate.r, userPlate.g, userPlate.b)
    drawBareWithShadow(artwork, content, size, pad, cardSize, box, userPlate, 'dock')
    return
  }

  // Step 2 (owner final rule: 「本身自带背景，就使用其自带的背景颜色，不要
  // 改动」): the own background expands UNCHANGED to fill the target shape.
  if (profile.kind === 'ownBoard' && profile.background) {
    composeFromPlate(artwork, content, size, pad, cardSize, shape, profile.background, box)
    return
  }

  // Steps 3-5 (owner correction: the subject's OUTERMOST RING — not its
  // interior — decides separation from the plate): the plate is computed from
  // the rim's dominant colour and mean lightness; grayscale rims get the pure
  // neutral board. Interior colour/lightness stay recorded in the profile for
  // other consumers.
  const seed = opts?.fieldSeed ? fromRgbInt(hexToInt(opts.fieldSeed)) : profile.subjectRimColour
  const plate = seed
    ? themedContrastTone(seed, profile.subjectRimLightness, band)
    : neutralContrastTone(profile.subjectRimLightness)
  fillRegion(content, size, pad, cardSize, plate.r, plate.g, plate.b)

  // Derived plates lift their subject with a silhouette-shaped shadow that
  // OPPOSES the plate; own-board icons never get one — that background is
  // theirs.
  if (profile.transparentEdges) {
    drawBareWithShadow(artwork, content, size, pad, cardSize, box, plate, 'dock')
    return
  }
  composeFromPlate(artwork, content, size, pad, cardSize, shape, plate, box)
}

/** Shadow modes (designer air-feel + UX pale-halo acceptance numbers). */
const SHADOW_MODES = {
  /** Normal bare art: airy macOS-dock drop shadow. */
  dock: { alpha: 0.24, blurFraction: 0.04, offsetFraction: 0.015 },
  /** Pale art: 360° ring halo — depth all around, no direction. */
  halo: { alpha: 0.34, blurFraction: 0.035, offsetFraction: 0 },
} as const

/**
 * The artwork drawn ORIGINAL over a soft silhouette shadow — the separation
 * device that replaces recolouring (owner iron law). The shadow is a blurred
 * copy of the drawn alpha in a deep tone of the plate's own hue, so it reads
 * as depth, not as a new colour.
 */
/** Per-size scratch buffers for the shadow pass — renderTile runs strictly
 *  sequentially per worker/thread, so reuse is safe and kills the ~1MiB/icon
 *  allocation churn at 300-icon bakes (codex #11). */
const shadowScratch = new Map<number, { layer: Raster; alpha: Float32Array; tmp: Float32Array }>()

function shadowScratchFor(size: number): { layer: Raster; alpha: Float32Array; tmp: Float32Array } {
  let s = shadowScratch.get(size)
  if (!s) {
    s = { layer: makeRaster(size), alpha: new Float32Array(size * size), tmp: new Float32Array(size * size) }
    shadowScratch.set(size, s)
  } else {
    s.layer.data.fill(0)
  }
  return s
}

function drawBareWithShadow(
  artwork: Raster,
  content: Raster,
  size: number,
  pad: number,
  cardSize: number,
  box: number,
  plate: { r: number; g: number; b: number },
  mode: keyof typeof SHADOW_MODES,
): void {
  const spec = SHADOW_MODES[mode]
  const { layer, alpha, tmp } = shadowScratchFor(size)
  drawCentred(artwork, analysis.contentBounds(artwork), layer, size, pad, cardSize, box)

  const n = size * size
  for (let i = 0; i < n; i++) alpha[i] = layer.data[i * 4 + 3] / 255
  const radius = Math.max(1, Math.round(size * spec.blurFraction))
  boxBlurInPlace(alpha, tmp, size, size, radius)
  boxBlurInPlace(alpha, tmp, size, size, radius)

  const shadow = fieldShadowTone({ ...plate, a: 255 })
  const dy = spec.offsetFraction === 0 ? 0 : Math.max(1, Math.round(size * spec.offsetFraction))
  const d = content.data
  for (let y = 0; y < size; y++) {
    const sy = y - dy
    if (sy < 0) continue
    for (let x = 0; x < size; x++) {
      const a = alpha[sy * size + x] * spec.alpha
      if (a <= 0.004) continue
      overAt(d, (y * size + x) * 4, shadow.r, shadow.g, shadow.b, Math.round(a * 255))
    }
  }
  compositeOver(content, layer)
}

/** Separable box blur on a coverage field (two passes ≈ soft shadow falloff).
 *  `tmp` is caller-provided scratch (same length as `field`). */
function boxBlurInPlace(field: Float32Array, tmp: Float32Array, w: number, h: number, radius: number): void {
  const win = radius * 2 + 1
  for (let y = 0; y < h; y++) {
    let acc = 0
    const row = y * w
    for (let x = -radius; x <= radius; x++) acc += field[row + Math.min(w - 1, Math.max(0, x))]
    for (let x = 0; x < w; x++) {
      tmp[row + x] = acc / win
      const outX = Math.max(0, x - radius)
      const inX = Math.min(w - 1, x + radius + 1)
      acc += field[row + inX] - field[row + outX]
    }
  }
  for (let x = 0; x < w; x++) {
    let acc = 0
    for (let y = -radius; y <= radius; y++) acc += tmp[Math.min(h - 1, Math.max(0, y)) * w + x]
    for (let y = 0; y < h; y++) {
      field[y * w + x] = acc / win
      const outY = Math.max(0, y - radius)
      const inY = Math.min(h - 1, y + radius + 1)
      acc += tmp[inY * w + x] - tmp[outY * w + x]
    }
  }
}

/** The segmented subject as its own layer: background pixels drop to
 *  transparent; `flatTint` recolours the subject to one flat colour (极致单色).
 *  Returns null when segmentation is degenerate (subject < 2% of the canvas). */
function monoSubjectLayer(artwork: Raster, flatTint: number | null): Raster | null {
  const { mask } = segmentSubject(artwork)
  let solid = 0
  for (const v of mask) solid += v
  if (solid < mask.length * 0.02) return null
  const layer = makeRaster(artwork.width, artwork.height)
  const src = artwork.data
  const dst = layer.data
  const tr = flatTint === null ? 0 : (flatTint >> 16) & 0xff
  const tg = flatTint === null ? 0 : (flatTint >> 8) & 0xff
  const tb = flatTint === null ? 0 : flatTint & 0xff
  for (let i = 0; i < mask.length; i++) {
    if (!mask[i]) continue
    const i4 = i * 4
    if (src[i4 + 3] === 0) continue
    if (flatTint === null) {
      dst[i4] = src[i4]
      dst[i4 + 1] = src[i4 + 1]
      dst[i4 + 2] = src[i4 + 2]
    } else {
      dst[i4] = tr
      dst[i4 + 1] = tg
      dst[i4 + 2] = tb
    }
    dst[i4 + 3] = src[i4 + 3]
  }
  return layer
}

/** How close a pixel must sit to the icon's own backdrop to count as plate
 *  (matches foregroundBounds' fg/bg separation tolerance). */
const BG_SWAP_TOLERANCE = 48
/** Plate recolours closer than this need no swap (visually identical). */
const BG_SWAP_MIN_SHIFT = 12

/** Per-artwork cache of the last backdrop-swapped copy (keyed by plate hex). */
const bgSwapCache = new WeakMap<Raster, { key: number; out: Raster }>()

/** The artwork with pixels near its OWN detected backdrop swapped to the new
 *  plate colour. The fg crop is a rectangle, so without this a recoloured
 *  plate (Field clamp/Quiet/背景色 override) leaves the original backdrop as
 *  a visible rectangle behind the subject (codex #2). Subject pixels — those
 *  clearly distinct from the backdrop — are untouched (law 4). */
function backdropSwapped(
  artwork: Raster,
  own: { r: number; g: number; b: number; a: number },
  plate: { r: number; g: number; b: number },
): Raster {
  const key = (plate.r << 16) | (plate.g << 8) | plate.b
  const hit = bgSwapCache.get(artwork)
  if (hit && hit.key === key) return hit.out
  const out: Raster = {
    width: artwork.width,
    height: artwork.height,
    data: new Uint8ClampedArray(artwork.data),
  }
  const d = out.data
  for (let i4 = 0; i4 < d.length; i4 += 4) {
    if (d[i4 + 3] <= 24) continue
    if (
      Math.abs(d[i4] - own.r) + Math.abs(d[i4 + 1] - own.g) + Math.abs(d[i4 + 2] - own.b) <=
      BG_SWAP_TOLERANCE
    ) {
      d[i4] = plate.r
      d[i4 + 1] = plate.g
      d[i4 + 2] = plate.b
    }
  }
  bgSwapCache.set(artwork, { key, out })
  return out
}

/** Rebuild a plated icon in the target shape (ComposeFromPlate).
 *  `boxCap` overrides the content keyline (Field lanes pass their larger box). */
function composeFromPlate(
  artwork: Raster,
  content: Raster,
  size: number,
  pad: number,
  cardSize: number,
  shape: IconShape,
  bg: { r: number; g: number; b: number },
  boxCap?: number,
): void {
  fillRegion(content, size, pad, cardSize, bg.r, bg.g, bg.b)
  const plate = analysis.contentBounds(artwork)
  const plateMin = Math.max(1, Math.min(boundsW(plate), boundsH(plate)))
  const fg = analysis.foregroundBounds(artwork)

  // When the fill deviates from the icon's own backdrop, draw from the
  // backdrop-swapped copy so no crop carries the old plate colour (codex #2).
  const own = analysis.tryDetectBackground(artwork)
  const source =
    own && colorDistance(own, { ...bg, a: 255 }) > BG_SWAP_MIN_SHIFT
      ? backdropSwapped(artwork, own, bg)
      : artwork

  if (fg && Math.max(boundsW(fg), boundsH(fg)) <= plateMin * FULL_BLEED_FOREGROUND_FRACTION) {
    const fraction = Math.max(boundsW(fg), boundsH(fg)) / plateMin
    const box = Math.min(Math.round(cardSize * fraction), boxCap ?? contentBox(shape, cardSize))
    drawCentred(source, fg, content, size, pad, cardSize, Math.max(8, box))
    return
  }
  if (INSCRIBE_SHAPES.has(shape)) {
    inscribeContent(source, content, size, pad, cardSize, shape)
    return
  }
  // Fallback (no isolable foreground, or content-dense full bleed): CENTRE the
  // content at the keyline instead of stretching it wall-to-wall (owner
  // 2026-07-10: 「主体顶满整个容器，看起来不太美观」— a document sheet ran
  // edge to edge). Pure colour boards are unaffected: what this redraws is the
  // same colour as the already-filled plate, so it stays seamless; anything
  // with visible content gains the universal breathing margin.
  drawCentred(
    source,
    analysis.contentBounds(artwork),
    content, size, pad, cardSize,
    boxCap ?? contentBox(shape, cardSize),
  )
}

function inscribeContent(
  artwork: Raster, content: Raster, size: number, pad: number, cardSize: number, shape: IconShape,
): void {
  const bounds = analysis.contentBounds(artwork)
  const scale = analysis.maxScaleInside(artwork, shape) * inscribeMargin(shape)
  const box = Math.max(8, Math.round(cardSize * scale))
  drawCentred(artwork, bounds, content, size, pad, cardSize, box)
}

function drawCentred(
  artwork: Raster, bounds: ContentBounds, content: Raster, size: number, pad: number, cardSize: number, box: number,
): void {
  const [w, h] = fit(Math.max(1, boundsW(bounds)), Math.max(1, boundsH(bounds)), box)
  drawScaled(artwork, bounds, content, size, pad + Math.trunc((cardSize - w) / 2), pad + Math.trunc((cardSize - h) / 2), w, h)
}

function innerBox(cardSize: number): number {
  return Math.max(8, cardSize - 2 * Math.round(cardSize * CONTENT_PADDING_FRACTION))
}

function fillRegion(content: Raster, size: number, pad: number, cardSize: number, r: number, g: number, b: number): void {
  const end = Math.min(size, pad + cardSize)
  for (let y = Math.max(0, pad); y < end; y++) {
    for (let x = Math.max(0, pad); x < end; x++) {
      const i4 = (y * size + x) * 4
      content.data[i4] = r
      content.data[i4 + 1] = g
      content.data[i4 + 2] = b
      content.data[i4 + 3] = 255
    }
  }
}

function fit(w: number, h: number, max: number): [number, number] {
  const scale = Math.min(max / w, max / h)
  return [Math.max(1, Math.round(w * scale)), Math.max(1, Math.round(h * scale))]
}

/** Clip the composed tile to the shape coverage (anti-aliased edges). */
function applyCoverage(tile: Raster, mask: Float64Array): void {
  const d = tile.data
  for (let i = 0; i < mask.length; i++) {
    const cover = mask[i]
    if (cover >= 1) continue
    const i4 = i * 4
    if (cover <= 0) {
      d[i4] = 0
      d[i4 + 1] = 0
      d[i4 + 2] = 0
      d[i4 + 3] = 0
    } else {
      d[i4 + 3] = Math.round(d[i4 + 3] * cover)
    }
  }
}

function compositeOver(target: Raster, over: Raster): void {
  const od = over.data
  const td = target.data
  for (let i4 = 0; i4 < od.length; i4 += 4) {
    if (od[i4 + 3] > 0) overAt(td, i4, od[i4], od[i4 + 1], od[i4 + 2], od[i4 + 3])
  }
}

/** Alpha-weighted mean luminance of the composed tile — what the eye reads. */
function composedLuminance(tile: Raster): number {
  const d = tile.data
  let sum = 0
  let weight = 0
  for (let i4 = 0; i4 < d.length; i4 += 4) {
    const a = d[i4 + 3]
    if (a === 0) continue
    sum += luminance(d[i4], d[i4 + 1], d[i4 + 2]) * a
    weight += a
  }
  return weight <= 0 ? 0.5 : sum / weight
}
