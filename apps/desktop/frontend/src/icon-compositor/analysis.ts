// FROZEN 2026-07-10 (ADR-0019): parity oracle for the Rust port. No new styles,
// no fixes except oracle corrections. Deleted after M6 certification.

import type { IconShape } from '@/bridge/types'
import type { Raster, Rgba } from './raster'
import { shapeContains } from './shapes'
import { perceivedLightness, toOkLab } from './color'

// Artwork analysis — 1:1 port of the frozen C# oracle (IconBackgroundAnalyzer,
// IconSilhouetteClassifier, IconAnalysisCache; ADR-0015 D3). Understands what
// the REAL icon already looks like so styling helps instead of hurts. Runs on
// 256px sources, once per source (memoized), never in the per-knob hot path.

export interface ContentBounds {
  left: number
  top: number
  right: number
  bottom: number
}

const boundsW = (b: ContentBounds) => b.right - b.left
const boundsH = (b: ContentBounds) => b.bottom - b.top
export { boundsW, boundsH }

const SOLID_ALPHA = 128
const MATCH_IOU = 0.985

function alphaAt(c: Raster, x: number, y: number): number {
  return c.data[(y * c.width + x) * 4 + 3]
}

function pixelAt(c: Raster, x: number, y: number): Rgba {
  const i4 = (y * c.width + x) * 4
  return { r: c.data[i4], g: c.data[i4 + 1], b: c.data[i4 + 2], a: c.data[i4 + 3] }
}

/** Manhattan RGB distance — the reference's cheap colour-similarity metric. */
export function colorDistance(a: Rgba, b: Rgba): number {
  return Math.abs(a.r - b.r) + Math.abs(a.g - b.g) + Math.abs(a.b - b.b)
}

// ---- IconBackgroundAnalyzer ----

/** >10% of the 1px border is see-through → the icon floats on transparency. */
export function hasTransparentEdges(c: Raster): boolean {
  const last = c.width - 1
  let transparent = 0
  let total = 0
  for (let i = 0; i < c.width; i++) {
    if (alphaAt(c, i, 0) < 245) transparent++
    if (alphaAt(c, i, last) < 245) transparent++
    if (alphaAt(c, 0, i) < 245) transparent++
    if (alphaAt(c, last, i) < 245) transparent++
    total += 4
  }
  return transparent > total / 10
}

/** Tight bounding box of pixels with alpha > 24; the whole canvas if fully empty. */
export function findContentBounds(c: Raster): ContentBounds {
  let minX = c.width
  let minY = c.height
  let maxX = -1
  let maxY = -1
  for (let y = 0; y < c.height; y++) {
    for (let x = 0; x < c.width; x++) {
      if (c.data[(y * c.width + x) * 4 + 3] > 24) {
        if (x < minX) minX = x
        if (y < minY) minY = y
        if (x > maxX) maxX = x
        if (y > maxY) maxY = y
      }
    }
  }
  return maxX < minX || maxY < minY
    ? { left: 0, top: 0, right: c.width, bottom: c.height }
    : { left: minX, top: minY, right: maxX + 1, bottom: maxY + 1 }
}

/** The icon's own background colour, or null when it's a bare logo needing a plate. */
export function tryDetectBackground(c: Raster): Rgba | null {
  return tryCanvasBackground(c) ?? tryShapeBackground(c)
}

function tryCanvasBackground(c: Raster): Rgba | null {
  const innerInset = Math.max(4, Math.floor(c.width / 32))
  const outer = tryUniformRectRing(c, 0, 18)
  if (!outer) return null
  const inner = tryUniformRectRing(c, innerInset, 18)
  if (!inner) return null
  return colorDistance(outer, inner) > 18 ? null : outer
}

function tryShapeBackground(c: Raster): Rgba | null {
  const bounds = findContentBounds(c)
  if (opaqueCoverage(c, bounds) < 0.62) return null
  const minDim = Math.min(boundsW(bounds), boundsH(bounds))
  if (minDim < c.width / 3) return null
  // Owner law ②: only square / rounded-square / circle SILHOUETTES own a
  // board, and all of those are corner-symmetric. A dog-eared document page
  // (one corner clipped by the fold) sails through the ring probes on blank
  // pages — its uniform white ring is NOT a board (designer FAIL v19: six
  // white-page tiles anchored white while their twins took derived plates).
  if (!cornersSymmetric(c, bounds, minDim)) return null

  // Owner rule (2026-07-10): ONLY the outermost ring decides — a uniform ring
  // IS the background. The old inner-ring consistency check rejected genuine
  // boards whose CENTRE holds a big logo (the Xbox class: perfect green ring,
  // sphere in the middle). Ring depth is probed at several insets because a
  // 1px highlight border fattens under source upscaling (the terminal class);
  // the first depth that reads uniform wins.
  const offsets = [
    Math.max(2, Math.floor(minDim / 96)),
    Math.max(5, Math.floor(minDim / 48)),
    Math.max(9, Math.floor(minDim / 24)),
  ]
  for (const offset of offsets) {
    if (offset * 2 + 2 >= minDim) break
    const ring = tryUniformShapeRing(c, bounds, offset, 24)
    if (ring) return ring
  }
  return null
}

/** Diagonal inset from each content-bounds corner to the first fully-solid
 *  pixel. Squares, rounded squares and circles give four near-equal insets;
 *  a fold/notch on one corner breaks the symmetry. */
function cornersSymmetric(c: Raster, b: ContentBounds, minDim: number): boolean {
  const walk = (sx: number, sy: number, dx: number, dy: number): number => {
    const limit = Math.ceil(minDim / 2)
    for (let k = 0; k < limit; k++) {
      if (alphaAt(c, sx + dx * k, sy + dy * k) >= 245) return k
    }
    return limit
  }
  const insets = [
    walk(b.left, b.top, 1, 1),
    walk(b.right - 1, b.top, -1, 1),
    walk(b.left, b.bottom - 1, 1, -1),
    walk(b.right - 1, b.bottom - 1, -1, -1),
  ]
  const spread = Math.max(...insets) - Math.min(...insets)
  return spread <= Math.max(2, Math.floor(minDim / 24))
}

function opaqueCoverage(c: Raster, b: ContentBounds): number {
  const total = Math.max(1, boundsW(b) * boundsH(b))
  let opaque = 0
  for (let y = b.top; y < b.bottom; y++) {
    for (let x = b.left; x < b.right; x++) {
      if (c.data[(y * c.width + x) * 4 + 3] > 24) opaque++
    }
  }
  return opaque / total
}

class RingAccumulator {
  samples: Rgba[] = []
  total = 0
  private opaque = 0
  private sumR = 0
  private sumG = 0
  private sumB = 0

  add(color: Rgba): void {
    this.total++
    if (color.a < 245) return
    this.opaque++
    this.samples.push(color)
    this.sumR += color.r
    this.sumG += color.g
    this.sumB += color.b
  }

  resolve(tolerance: number, opaqueFraction: number, closeFraction: number): Rgba | null {
    if (this.total === 0 || this.opaque < Math.floor(this.total * opaqueFraction)) return null
    const avg: Rgba = {
      r: Math.floor(this.sumR / this.opaque),
      g: Math.floor(this.sumG / this.opaque),
      b: Math.floor(this.sumB / this.opaque),
      a: 255,
    }
    let close = 0
    for (const s of this.samples) {
      if (colorDistance(s, avg) <= tolerance) close++
    }
    return close >= Math.floor(this.samples.length * closeFraction) ? avg : null
  }
}

function tryUniformRectRing(c: Raster, inset: number, tolerance: number): Rgba | null {
  const min = inset
  const max = c.width - 1 - inset
  if (max <= min) return null
  const acc = new RingAccumulator()
  for (let i = min; i <= max; i++) {
    acc.add(pixelAt(c, i, min))
    acc.add(pixelAt(c, i, max))
    acc.add(pixelAt(c, min, i))
    acc.add(pixelAt(c, max, i))
  }
  return acc.resolve(tolerance, 0.9, 0.95)
}

function tryUniformShapeRing(c: Raster, b: ContentBounds, offset: number, tolerance: number): Rgba | null {
  const acc = new RingAccumulator()
  for (let y = b.top; y < b.bottom; y++) {
    const span = opaqueRowSpan(c, y, b.left, b.right - 1)
    if (span && span[1] - span[0] > offset * 2) {
      acc.add(pixelAt(c, span[0] + offset, y))
      acc.add(pixelAt(c, span[1] - offset, y))
    }
  }
  for (let x = b.left; x < b.right; x++) {
    const span = opaqueColumnSpan(c, x, b.top, b.bottom - 1)
    if (span && span[1] - span[0] > offset * 2) {
      acc.add(pixelAt(c, x, span[0] + offset))
      acc.add(pixelAt(c, x, span[1] - offset))
    }
  }
  return acc.total < 32 ? null : acc.resolve(tolerance, 0.92, 0.92)
}

// Spans walk to the SOLID edge (A >= 245), not the first faint pixel — a
// resized source grows a soft anti-aliased band that the fixed ring offsets
// otherwise land inside, silently failing the uniform-ring probe on icons
// that genuinely carry a colour board (owner 2026-07-10: the Twitter class).
function opaqueRowSpan(c: Raster, y: number, minX: number, maxX: number): [number, number] | null {
  let left = -1
  let right = -1
  for (let x = minX; x <= maxX; x++) {
    if (alphaAt(c, x, y) >= 245) { left = x; break }
  }
  for (let x = maxX; x >= minX; x--) {
    if (alphaAt(c, x, y) >= 245) { right = x; break }
  }
  return left >= 0 && right >= left ? [left, right] : null
}

function opaqueColumnSpan(c: Raster, x: number, minY: number, maxY: number): [number, number] | null {
  let top = -1
  let bottom = -1
  for (let y = minY; y <= maxY; y++) {
    if (alphaAt(c, x, y) >= 245) { top = y; break }
  }
  for (let y = maxY; y >= minY; y--) {
    if (alphaAt(c, x, y) >= 245) { bottom = y; break }
  }
  return top >= 0 && bottom >= top ? [top, bottom] : null
}

// ---- IconSilhouetteClassifier ----

/** Tight bounding box of solid (A ≥ 128) pixels; null when the canvas has none. */
export function solidBounds(c: Raster): ContentBounds | null {
  let minX = c.width
  let minY = c.height
  let maxX = -1
  let maxY = -1
  for (let y = 0; y < c.height; y++) {
    for (let x = 0; x < c.width; x++) {
      if (c.data[(y * c.width + x) * 4 + 3] >= SOLID_ALPHA) {
        if (x < minX) minX = x
        if (y < minY) minY = y
        if (x > maxX) maxX = x
        if (y > maxY) maxY = y
      }
    }
  }
  return maxX < minX ? null : { left: minX, top: minY, right: maxX + 1, bottom: maxY + 1 }
}

/** True when the icon's solid silhouette IS the target shape (IoU ≥ 0.985). */
export function matchesShape(c: Raster, shape: IconShape): boolean {
  const b = solidBounds(c)
  if (!b || boundsW(b) < c.width / 3 || boundsH(b) < c.width / 3) return false
  const w = boundsW(b)
  const h = boundsH(b)
  if (Math.min(w, h) / Math.max(w, h) < 0.95) return false

  const s = Math.max(w, h)
  const ox = b.left + (w - s) / 2
  const oy = b.top + (h - s) / 2
  const step = Math.max(1, Math.floor(s / 96))
  let inter = 0
  let union = 0
  for (let y = b.top; y < b.bottom; y += step) {
    for (let x = b.left; x < b.right; x += step) {
      const solid = alphaAt(c, x, y) >= SOLID_ALPHA
      const inShape = shapeContains(shape, x + 0.5 - ox, y + 0.5 - oy, s)
      if (solid && inShape) inter++
      if (solid || inShape) union++
    }
  }
  return union > 0 && inter / union >= MATCH_IOU
}

/** The bounding box of the icon's own logo INSIDE its solid plate. */
export function foregroundBounds(
  c: Raster,
  plate: ContentBounds,
  background: Rgba,
  tolerance = 48,
): ContentBounds | null {
  let minX = plate.right
  let minY = plate.bottom
  let maxX = plate.left - 1
  let maxY = plate.top - 1
  for (let y = plate.top; y < plate.bottom; y++) {
    for (let x = plate.left; x < plate.right; x++) {
      const p = pixelAt(c, x, y)
      if (p.a > 24 && colorDistance(p, background) > tolerance) {
        if (x < minX) minX = x
        if (y < minY) minY = y
        if (x > maxX) maxX = x
        if (y > maxY) maxY = y
      }
    }
  }
  if (maxX < minX) return null
  const margin = Math.max(1, Math.floor(Math.min(boundsW(plate), boundsH(plate)) / 48))
  return {
    left: Math.max(plate.left, minX - margin),
    top: Math.max(plate.top, minY - margin),
    right: Math.min(plate.right, maxX + 1 + margin),
    bottom: Math.min(plate.bottom, maxY + 1 + margin),
  }
}

/** Largest scale (fraction of the shape box) at which the solid silhouette fits inside. */
export function maxScaleInside(c: Raster, b: ContentBounds, shape: IconShape): number {
  const boundary: Array<[number, number]> = []
  for (let y = b.top; y < b.bottom; y++) {
    for (let x = b.left; x < b.right; x++) {
      if (alphaAt(c, x, y) < SOLID_ALPHA) continue
      const edge =
        x === 0 || y === 0 || x === c.width - 1 || y === c.height - 1 ||
        alphaAt(c, x - 1, y) < SOLID_ALPHA || alphaAt(c, x + 1, y) < SOLID_ALPHA ||
        alphaAt(c, x, y - 1) < SOLID_ALPHA || alphaAt(c, x, y + 1) < SOLID_ALPHA
      if (edge) boundary.push([x + 0.5, y + 0.5])
    }
  }
  if (boundary.length === 0) return 1

  const cx = b.left + boundsW(b) / 2
  const cy = b.top + boundsH(b) / 2
  const half = Math.max(boundsW(b), boundsH(b)) / 2

  const fitsAt = (scale: number) => {
    for (const [x, y] of boundary) {
      const u = 1 + ((x - cx) / half) * scale
      const v = 1 + ((y - cy) / half) * scale
      if (!shapeContains(shape, u, v, 2)) return false
    }
    return true
  }

  let lo = 0.5
  let hi = 1
  for (let i = 0; i < 7; i++) {
    const mid = (lo + hi) / 2
    if (fitsAt(mid)) lo = mid
    else hi = mid
  }
  return lo
}

// ---- Dominant colour + hue dispersion (ADR-0016 Field mode) ----

/** Pixels this transparent or duller never vote for a dominant hue. */
const DOMINANT_MIN_ALPHA = 128
const DOMINANT_MIN_CHROMA = 0.03
/** Owner spec (2026-07-10): the theme colour must cover AT LEAST HALF of the
 *  SUBJECT's pixels (the segmented subject, not the whole icon) — a
 *  decorative accent on grayscale art is NOT a theme. */
const THEME_MAJORITY = 0.5
/** Neighbouring-hue merging: adjacent buckets join the theme band while they
 *  carry at least this fraction of the peak's weight (light-blue→deep-blue
 *  and Microsoft-style near-hue gradients read as ONE colour; owner asked
 *  for a GENEROUS merge)… */
const NEIGHBOUR_RATIO = 0.1
/** …but a band never grows past ±6 buckets (±65°) — beyond that it is a
 *  rainbow, not a theme. */
const NEIGHBOUR_MAX_SPAN = 6
const HUE_BUCKETS = 36

export interface DominantColour {
  /** Chroma-weighted mean colour of the peak hue band (±1 bucket). */
  colour: Rgba
  /** Circular hue dispersion over ALL voters: 0 = one hue, →1 = scattered.
   *  The Field fidelity gate reads this (multi-hue artwork keeps its own art). */
  dispersion: number
}

/**
 * The artwork's dominant colour, chroma-weighted in OKLab hue space.
 * `mask` (usually `segmentSubject(c).mask` — passed in because segment.ts
 * imports THIS module) restricts voting to subject pixels; null = whole canvas.
 * Returns null for the no-hue tail (photos of gray things, near-white logos,
 * plain documents) — Field mode then falls back to the kind-family plate.
 */
export function dominantColor(c: Raster, mask: Uint8Array | null): DominantColour | null {
  const d = c.data
  const n = c.width * c.height
  // ONE pass (codex #10): histogram + circular moments + per-bucket weighted
  // RGB sums, so the peak-band mean needs no second image scan.
  const bucketWeight = new Float64Array(HUE_BUCKETS)
  const bucketVoters = new Uint32Array(HUE_BUCKETS)
  const bucketR = new Float64Array(HUE_BUCKETS)
  const bucketG = new Float64Array(HUE_BUCKETS)
  const bucketB = new Float64Array(HUE_BUCKETS)
  let sumCos = 0
  let sumSin = 0
  let totalWeight = 0
  let voters = 0
  let visible = 0
  for (let i = 0; i < n; i++) {
    const i4 = i * 4
    if (d[i4 + 3] < DOMINANT_MIN_ALPHA) continue
    if (mask && !mask[i]) continue
    visible++
    const lab = toOkLab(d[i4], d[i4 + 1], d[i4 + 2])
    const chroma = Math.sqrt(lab.A * lab.A + lab.B * lab.B)
    if (chroma < DOMINANT_MIN_CHROMA) continue
    const theta = Math.atan2(lab.B, lab.A)
    const bucket = Math.floor(((theta + Math.PI) / (2 * Math.PI)) * HUE_BUCKETS) % HUE_BUCKETS
    bucketWeight[bucket] += chroma
    bucketVoters[bucket]++
    bucketR[bucket] += d[i4] * chroma
    bucketG[bucket] += d[i4 + 1] * chroma
    bucketB[bucket] += d[i4 + 2] * chroma
    sumCos += chroma * Math.cos(theta)
    sumSin += chroma * Math.sin(theta)
    totalWeight += chroma
    voters++
  }
  if (visible === 0 || voters === 0) return null

  let peak = 0
  for (let b = 1; b < HUE_BUCKETS; b++) {
    if (bucketWeight[b] > bucketWeight[peak]) peak = b
  }
  if (bucketWeight[peak] <= 0) return null

  // Grow the theme band by neighbouring-hue merging (owner spec): walk out
  // from the peak while the adjacent bucket still carries real weight.
  const inBand = new Set<number>([peak])
  for (const dir of [-1, 1]) {
    for (let step = 1; step <= NEIGHBOUR_MAX_SPAN; step++) {
      const b = (peak + dir * step + HUE_BUCKETS * 8) % HUE_BUCKETS
      if (bucketWeight[b] < bucketWeight[peak] * NEIGHBOUR_RATIO) break
      inBand.add(b)
    }
  }

  let w = 0
  let bandVoters = 0
  let r = 0
  let g = 0
  let bl = 0
  for (const b of inBand) {
    w += bucketWeight[b]
    bandVoters += bucketVoters[b]
    r += bucketR[b]
    g += bucketG[b]
    bl += bucketB[b]
  }
  // Majority gate (owner spec): the merged theme band must cover >=50% of the
  // subject's pixels — decorative accents on grayscale art never qualify.
  if (w <= 0 || bandVoters < visible * THEME_MAJORITY) return null

  const dispersion = 1 - Math.sqrt(sumCos * sumCos + sumSin * sumSin) / totalWeight
  return {
    colour: { r: Math.round(r / w), g: Math.round(g / w), b: Math.round(bl / w), a: 255 },
    dispersion,
  }
}

/** Mean perceived lightness over solid (A≥128) pixels — the Field pale-class
 *  gate + contrast-target plate. (The old span/light-mass/dark-mass metrics
 *  died with the knockout lane — codex #10.) */
export interface LightnessStats {
  mean: number
}

export function visibleLightnessStats(c: Raster): LightnessStats {
  const d = c.data
  const n = c.width * c.height
  let sum = 0
  let visible = 0
  for (let i = 0; i < n; i++) {
    const i4 = i * 4
    if (d[i4 + 3] < SOLID_ALPHA) continue
    sum += perceivedLightness(d[i4], d[i4 + 1], d[i4 + 2])
    visible++
  }
  if (visible === 0) return { mean: 0.5 }
  return { mean: sum / visible }
}

// ---- IconAnalysisCache (WeakMap-memoized per source raster) ----

interface AnalysisEntry {
  transparentEdges?: boolean
  backgroundComputed?: boolean
  background?: Rgba | null
  content?: ContentBounds
  solidComputed?: boolean
  solid?: ContentBounds | null
  foregroundComputed?: boolean
  foreground?: ContentBounds | null
  dominantComputed?: boolean
  dominant?: DominantColour | null
  lightnessStats?: LightnessStats
  matches: Map<IconShape, boolean>
  maxScale: Map<IconShape, number>
}

const cache = new WeakMap<Raster, AnalysisEntry>()

function entryOf(c: Raster): AnalysisEntry {
  let e = cache.get(c)
  if (!e) {
    e = { matches: new Map(), maxScale: new Map() }
    cache.set(c, e)
  }
  return e
}

export const analysis = {
  hasTransparentEdges(c: Raster): boolean {
    const e = entryOf(c)
    e.transparentEdges ??= hasTransparentEdges(c)
    return e.transparentEdges
  },
  tryDetectBackground(c: Raster): Rgba | null {
    const e = entryOf(c)
    if (!e.backgroundComputed) {
      e.background = tryDetectBackground(c)
      e.backgroundComputed = true
    }
    return e.background ?? null
  },
  contentBounds(c: Raster): ContentBounds {
    const e = entryOf(c)
    e.content ??= findContentBounds(c)
    return e.content
  },
  solidBounds(c: Raster): ContentBounds | null {
    const e = entryOf(c)
    if (!e.solidComputed) {
      e.solid = solidBounds(c)
      e.solidComputed = true
    }
    return e.solid ?? null
  },
  matchesShape(c: Raster, shape: IconShape): boolean {
    const e = entryOf(c)
    let m = e.matches.get(shape)
    if (m === undefined) {
      m = matchesShape(c, shape)
      e.matches.set(shape, m)
    }
    return m
  },
  foregroundBounds(c: Raster): ContentBounds | null {
    const e = entryOf(c)
    if (!e.foregroundComputed) {
      const bg = this.tryDetectBackground(c)
      e.foreground = bg ? foregroundBounds(c, this.contentBounds(c), bg) : null
      e.foregroundComputed = true
    }
    return e.foreground ?? null
  },
  maxScaleInside(c: Raster, shape: IconShape): number {
    const e = entryOf(c)
    let s = e.maxScale.get(shape)
    if (s === undefined) {
      s = maxScaleInside(c, this.contentBounds(c), shape)
      e.maxScale.set(shape, s)
    }
    return s
  },
  /** Memoized dominant colour over the SUBJECT mask (owner spec: the 50%
   *  majority is measured against the subject, not the whole icon). The mask
   *  is derived deterministically from this raster by the caller — segment.ts
   *  imports this module, so the mask rides in as a parameter. */
  dominantColor(c: Raster, mask: Uint8Array | null): DominantColour | null {
    const e = entryOf(c)
    if (!e.dominantComputed) {
      e.dominant = dominantColor(c, mask)
      e.dominantComputed = true
    }
    return e.dominant ?? null
  },
  visibleLightnessStats(c: Raster): LightnessStats {
    const e = entryOf(c)
    e.lightnessStats ??= visibleLightnessStats(c)
    return e.lightnessStats
  },
}
