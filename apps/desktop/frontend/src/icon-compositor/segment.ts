// FROZEN 2026-07-10 (ADR-0019): parity oracle for the Rust port. No new styles,
// no fixes except oracle corrections. Deleted after M6 certification.

import type { Raster, Rgba } from './raster'
import { hasTransparentEdges } from './analysis'

// Subject/background segmentation for 极致单色 (extreme duotone) — owner
// feature 2026-07-09: subject = ONE flat colour, background = ANOTHER, hard
// two-tone contrast. Algorithm validated on the real mock icon pack first
// (Python prototype, scratchpad segment-proto3): Spotify waves, Photos
// mountain+sun, Store window grid and the Recycle glyph all extract cleanly.
//
// Two stages:
//   1. SILHOUETTE — transparent-edge icons take the alpha silhouette; opaque
//      icons flood the background from border seeds with a LOCAL step
//      tolerance, so smooth gradient backdrops keep flooding while hard
//      subject boundaries stop the wave.
//   2. PLATE SPLIT — when the silhouette is plate-like (bbox fill > 0.72),
//      the rim's median colour is the "field"; Otsu over each pixel's colour
//      distance from the field separates ink from field, the field joins the
//      background. Guards: bimodal separation, ink fraction, and a
//      fragmentation check (photo-like full-bleed art must NOT shatter into
//      speckle — it falls back to the whole silhouette).

export interface Segmentation {
  /** 1 = subject pixel, 0 = background. Same indexing as the raster. */
  mask: Uint8Array
  mode: 'alpha' | 'flood' | 'alpha+split' | 'flood+split'
  /** The detected plate/background colour — present ONLY when a plate split
   *  occurred (a squarish/round silhouette with a uniform surround distinct
   *  from the subject). This is exactly the "fill with the icon's own plate
   *  colour, not white" signal (owner 2026-07-09). */
  field?: Rgba
}

const SOLID_ALPHA = 128
/** Max neighbour colour step that keeps the background flood moving. */
const FLOOD_LOCAL_TOLERANCE = 14
/** Border pixels within this distance of the border median seed the flood. */
const FLOOD_SEED_TOLERANCE = 42
/** Silhouettes at least this plate-like attempt the internal split. */
const PLATE_BBOX_FILL = 0.72
/** √(between-class variance) of the colour-distance Otsu split, 0..442. */
const SPLIT_SEPARATION = 26
const SPLIT_INK_MIN = 0.04
const SPLIT_INK_MAX = 0.6
/** The largest ink component must own this share of ink or the split is noise. */
const SPLIT_COHERENCE = 0.2
/** Ink owning more of the silhouette rim than this is suspect (inversion)... */
const RIM_OWNED = 0.6
/** ...UNLESS it is line-art: thin strokes (high edge density) at low fraction
 *  are legit outlined glyphs — the doc family's ruled lines + border
 *  (owner case 2026-07-09: 待办事项 vs 会议纪要 rendered differently because
 *  the old rim-median field flipped polarity with outline stroke width). */
const LINE_EDGE_DENSITY = 0.4
const LINE_FRAC_MAX = 0.3
// A detected FIELD counts as a real plate (fill-with-it, not white) only under
// the owner's STRICT 2026-07-09 criteria — it gates the FIELD only; the subject
// mask (极致单色) is unaffected, so non-plate shapes still segment:
//   1. the silhouette is an ABSOLUTE square / rounded-square / circle (tight
//      aspect AND a high IoU against the best-fit ideal — Outlook's envelope
//      shape fails here);
//   2. the outermost ring is ONE IDENTICAL colour (tight tolerance, not
//      approximate) — and that ring colour is the fill;
//   3. the plate body is FLAT — no gradients (the radial help icon fails here).
const PLATE_ASPECT_MIN = 0.92
const PLATE_ASPECT_MAX = 1.09
/** Silhouette must match a square/rounded-square/circle at least this well (IoU). */
const PLATE_SHAPE_IOU = 0.95
/** Outer-ring pixels within this RGB distance of the ring colour = "identical". */
const PLATE_RIM_TOLERANCE = 16
/** ...and at least this fraction of the outer ring must be that one colour. */
const PLATE_RIM_UNIFORM = 0.9
/** Plate-body pixels within this distance of the ring colour = "flat" (no gradient). */
const PLATE_FLAT_TOLERANCE = 22
/** ...and at least this fraction of the plate body must be that one flat colour. */
const PLATE_FLAT_FRACTION = 0.85

const segCache = new WeakMap<Raster, Segmentation>()

export function segmentSubject(c: Raster): Segmentation {
  const hit = segCache.get(c)
  if (hit) return hit
  const seg = computeSegmentation(c)
  segCache.set(c, seg)
  return seg
}

function computeSegmentation(c: Raster): Segmentation {
  const { width: w, height: h, data } = c
  const n = w * h

  let sil: Uint8Array
  let mode: Segmentation['mode']
  if (hasTransparentEdges(c)) {
    sil = new Uint8Array(n)
    for (let i = 0; i < n; i++) sil[i] = data[i * 4 + 3] >= SOLID_ALPHA ? 1 : 0
    mode = 'alpha'
  } else {
    sil = floodBackground(c)
    mode = 'flood'
  }
  sil = binaryMajority(sil, w, h, 2)

  let solid = 0
  for (let i = 0; i < n; i++) solid += sil[i]
  if (solid < n * 0.02) return { mask: sil, mode }

  const split = plateSplit(c, sil)
  if (split) {
    // `field` is a trustworthy PLATE colour only in alpha mode, where the
    // silhouette IS the whole plate+subject and its median = the plate. In
    // flood mode `sil` is already the subject (the plate was flooded away), so
    // the median would be the subject colour — and those icons are caught by
    // tryDetectBackground anyway, so we withhold the field there.
    return {
      mask: binaryMajority(split.ink, w, h, 1),
      mode: `${mode}+split` as Segmentation['mode'],
      field: mode === 'alpha' && split.field ? split.field : undefined,
    }
  }
  return { mask: sil, mode }
}

/** Border-seeded BFS flood over the background; returns the SUBJECT mask. */
function floodBackground(c: Raster): Uint8Array {
  const { width: w, height: h, data } = c
  const n = w * h
  const bg = new Uint8Array(n)
  const queue = new Int32Array(n)
  let head = 0
  let tail = 0

  // Border median colour (per channel) seeds the flood.
  const border: number[] = []
  for (let x = 0; x < w; x++) border.push(x, (h - 1) * w + x)
  for (let y = 0; y < h; y++) border.push(y * w, y * w + w - 1)
  const med = [0, 1, 2].map((ch) => {
    const vals = border.map((i) => data[i * 4 + ch]).sort((a, b) => a - b)
    return vals[vals.length >> 1]
  })

  const dist2 = (i: number, r: number, g: number, b: number) => {
    const dr = data[i * 4] - r
    const dg = data[i * 4 + 1] - g
    const db = data[i * 4 + 2] - b
    return dr * dr + dg * dg + db * db
  }

  const seedTol2 = FLOOD_SEED_TOLERANCE * FLOOD_SEED_TOLERANCE
  for (const i of border) {
    if (!bg[i] && dist2(i, med[0], med[1], med[2]) < seedTol2) {
      bg[i] = 1
      queue[tail++] = i
    }
  }

  const localTol2 = FLOOD_LOCAL_TOLERANCE * FLOOD_LOCAL_TOLERANCE
  while (head < tail) {
    const i = queue[head++]
    const x = i % w
    const r = data[i * 4]
    const g = data[i * 4 + 1]
    const b = data[i * 4 + 2]
    const tryStep = (j: number) => {
      if (!bg[j] && dist2(j, r, g, b) < localTol2) {
        bg[j] = 1
        queue[tail++] = j
      }
    }
    if (x > 0) tryStep(i - 1)
    if (x < w - 1) tryStep(i + 1)
    if (i >= w) tryStep(i - w)
    if (i < (h - 1) * w) tryStep(i + w)
  }

  const subject = new Uint8Array(n)
  for (let i = 0; i < n; i++) subject[i] = bg[i] ? 0 : 1
  return subject
}

/** Stage 2 — split a plate-like silhouette into field (→bg) and ink (subject). */
function plateSplit(c: Raster, sil: Uint8Array): { ink: Uint8Array; field: Rgba | null } | null {
  const { width: w, height: h, data } = c
  const n = w * h

  let minX = w
  let maxX = -1
  let minY = h
  let maxY = -1
  let solid = 0
  for (let i = 0; i < n; i++) {
    if (!sil[i]) continue
    solid++
    const x = i % w
    const y = (i / w) | 0
    if (x < minX) minX = x
    if (x > maxX) maxX = x
    if (y < minY) minY = y
    if (y > maxY) maxY = y
  }
  if (maxX < 0) return null
  const bboxFill = solid / ((maxX - minX + 1) * (maxY - minY + 1))
  if (bboxFill <= PLATE_BBOX_FILL) return null

  // Field = the silhouette's MEDIAN colour (the majority/plate colour). The
  // rim median used before flips polarity on outlined art: a thick border
  // stroke makes the rim read dark and the white page becomes "ink".
  const eroded = binaryErode(sil, w, h, 2)
  const chR: number[] = []
  const chG: number[] = []
  const chB: number[] = []
  for (let i = 0; i < n; i++) {
    if (sil[i]) {
      chR.push(data[i * 4])
      chG.push(data[i * 4 + 1])
      chB.push(data[i * 4 + 2])
    }
  }
  const med = (v: number[]) => v.sort((a, b) => a - b)[v.length >> 1]
  const fr = med(chR)
  const fg = med(chG)
  const fb = med(chB)

  // Otsu over colour distance from the field (64 bins across 0..442).
  const MAXD = 442
  const bins = 64
  const hist = new Float64Array(bins)
  const dist = new Float64Array(n)
  for (let i = 0; i < n; i++) {
    if (!sil[i]) continue
    const dr = data[i * 4] - fr
    const dg = data[i * 4 + 1] - fg
    const db = data[i * 4 + 2] - fb
    const d = Math.sqrt(dr * dr + dg * dg + db * db)
    dist[i] = d
    hist[Math.min(bins - 1, (d / MAXD * bins) | 0)]++
  }
  let sumAll = 0
  for (let i = 0; i < bins; i++) sumAll += ((i + 0.5) / bins) * MAXD * hist[i]
  let w0 = 0
  let sum0 = 0
  let bestT = 0
  let bestV = -1
  for (let i = 0; i < bins; i++) {
    w0 += hist[i]
    if (w0 === 0 || w0 === solid) continue
    sum0 += ((i + 0.5) / bins) * MAXD * hist[i]
    const m0 = sum0 / w0
    const m1 = (sumAll - sum0) / (solid - w0)
    const v = (w0 / solid) * (1 - w0 / solid) * (m0 - m1) * (m0 - m1)
    if (v > bestV) {
      bestV = v
      bestT = ((i + 0.5) / bins) * MAXD
    }
  }
  if (Math.sqrt(Math.max(0, bestV)) <= SPLIT_SEPARATION) return null

  const ink = new Uint8Array(n)
  let inkCount = 0
  for (let i = 0; i < n; i++) {
    if (sil[i] && dist[i] > bestT) {
      ink[i] = 1
      inkCount++
    }
  }
  const frac = inkCount / solid
  if (frac <= SPLIT_INK_MIN || frac >= SPLIT_INK_MAX) return null

  // Polarity guard: ink owning the rim is an inversion — except genuine
  // line-art (thin strokes, low fraction), which keeps its outlined glyph.
  let rimTotal = 0
  let rimInk = 0
  for (let i = 0; i < n; i++) {
    if (sil[i] && !eroded[i]) {
      rimTotal++
      if (ink[i]) rimInk++
    }
  }
  if (rimTotal >= 20 && rimInk / rimTotal > RIM_OWNED) {
    const inkEroded = binaryErode(ink, w, h, 1)
    let inner = 0
    for (let i = 0; i < n; i++) inner += inkEroded[i]
    const edgeDensity = 1 - inner / inkCount
    if (!(edgeDensity > LINE_EDGE_DENSITY && frac < LINE_FRAC_MAX)) return null
  }

  // Fragmentation guard: photo-like art shatters into speckle — reject.
  if (largestComponentShare(ink, w, h, inkCount) < SPLIT_COHERENCE) return null

  const field = detectFlatPlate(c, sil, ink, minX, minY, maxX, maxY, eroded)
  return { ink, field }
}

/** The STRICT plate detector (owner 2026-07-09): returns the outermost-ring
 *  colour ONLY for an absolute square/rounded-square/circle with an identical
 *  outer ring and a flat (non-gradient) body. Otherwise null → no hotfix. */
function detectFlatPlate(
  c: Raster, sil: Uint8Array, ink: Uint8Array,
  minX: number, minY: number, maxX: number, maxY: number, eroded: Uint8Array,
): Rgba | null {
  const { width: w, height: h, data } = c
  const bw = maxX - minX + 1
  const bh = maxY - minY + 1

  // 1. Absolute shape: tight aspect AND high IoU vs the best-fit ideal
  //    (square / circle / rounded-square). Outlook's envelope fails the IoU.
  const aspect = bw / bh
  if (aspect < PLATE_ASPECT_MIN || aspect > PLATE_ASPECT_MAX) return null
  const cx = minX + (bw - 1) / 2
  const cy = minY + (bh - 1) / 2
  const rx = bw / 2
  const ry = bh / 2
  const rr = 0.2 * Math.min(bw, bh) // rounded-square corner radius
  let interC = 0, unionC = 0, interR = 0, unionR = 0
  for (let y = minY; y <= maxY; y++) {
    for (let x = minX; x <= maxX; x++) {
      const s = sil[y * w + x]
      const ex = (x - cx) / rx
      const ey = (y - cy) / ry
      const inCircle = ex * ex + ey * ey <= 1 ? 1 : 0
      const qx = Math.max(0, (minX + rr) - x, x - (maxX - rr))
      const qy = Math.max(0, (minY + rr) - y, y - (maxY - rr))
      const inRound = qx * qx + qy * qy <= rr * rr ? 1 : 0
      if (s && inCircle) interC++
      if (s || inCircle) unionC++
      if (s && inRound) interR++
      if (s || inRound) unionR++
    }
  }
  let solid = 0
  for (let y = minY; y <= maxY; y++) for (let x = minX; x <= maxX; x++) solid += sil[y * w + x]
  const iouSquare = solid / (bw * bh)
  const iouCircle = unionC ? interC / unionC : 0
  const iouRound = unionR ? interR / unionR : 0
  if (Math.max(iouSquare, iouCircle, iouRound) < PLATE_SHAPE_IOU) return null

  // 2. Outer ring must be ONE identical colour; that colour is the fill.
  const or: number[] = []
  const og: number[] = []
  const ob: number[] = []
  for (let i = 0; i < w * h; i++) {
    if (sil[i] && !eroded[i]) {
      or.push(data[i * 4]); og.push(data[i * 4 + 1]); ob.push(data[i * 4 + 2])
    }
  }
  if (or.length < 20) return null
  const med = (v: number[]) => v.slice().sort((a, b) => a - b)[v.length >> 1]
  const er = med(or), eg = med(og), eb = med(ob)
  const ringTol2 = PLATE_RIM_TOLERANCE * PLATE_RIM_TOLERANCE
  let ringSame = 0
  for (let k = 0; k < or.length; k++) {
    const dr = or[k] - er, dg = og[k] - eg, db = ob[k] - eb
    if (dr * dr + dg * dg + db * db <= ringTol2) ringSame++
  }
  if (ringSame / or.length < PLATE_RIM_UNIFORM) return null

  // 3. Flat body (no gradient): the plate (silhouette minus subject) must be
  //    that one colour throughout. A radial/linear gradient fails here.
  const flatTol2 = PLATE_FLAT_TOLERANCE * PLATE_FLAT_TOLERANCE
  let body = 0, bodySame = 0
  for (let i = 0; i < w * h; i++) {
    if (!sil[i] || ink[i]) continue
    body++
    const dr = data[i * 4] - er, dg = data[i * 4 + 1] - eg, db = data[i * 4 + 2] - eb
    if (dr * dr + dg * dg + db * db <= flatTol2) bodySame++
  }
  if (body < 20 || bodySame / body < PLATE_FLAT_FRACTION) return null

  return { r: er, g: eg, b: eb, a: 255 }
}

/** Share of ink owned by its largest 4-connected component. */
function largestComponentShare(mask: Uint8Array, w: number, h: number, total: number): number {
  if (total === 0) return 0
  const seen = new Uint8Array(mask.length)
  const queue = new Int32Array(total)
  let best = 0
  for (let s = 0; s < mask.length; s++) {
    if (!mask[s] || seen[s]) continue
    let head = 0
    let tail = 0
    queue[tail++] = s
    seen[s] = 1
    while (head < tail) {
      const i = queue[head++]
      const x = i % w
      const tryStep = (j: number) => {
        if (mask[j] && !seen[j]) {
          seen[j] = 1
          queue[tail++] = j
        }
      }
      if (x > 0) tryStep(i - 1)
      if (x < w - 1) tryStep(i + 1)
      if (i >= w) tryStep(i - w)
      if (i < (h - 1) * w) tryStep(i + w)
    }
    if (tail > best) best = tail
  }
  return best / total
}

/** Binary majority filter over a (2r+1)² window — the prototype's median. */
function binaryMajority(mask: Uint8Array, w: number, h: number, r: number): Uint8Array {
  const integral = buildIntegral(mask, w, h)
  const out = new Uint8Array(mask.length)
  for (let y = 0; y < h; y++) {
    const y0 = Math.max(0, y - r)
    const y1 = Math.min(h - 1, y + r)
    for (let x = 0; x < w; x++) {
      const x0 = Math.max(0, x - r)
      const x1 = Math.min(w - 1, x + r)
      const area = (x1 - x0 + 1) * (y1 - y0 + 1)
      const sum = boxSum(integral, w, x0, y0, x1, y1)
      out[y * w + x] = sum * 2 > area ? 1 : 0
    }
  }
  return out
}

/** Binary erosion with a (2r+1)² structuring element. */
function binaryErode(mask: Uint8Array, w: number, h: number, r: number): Uint8Array {
  const integral = buildIntegral(mask, w, h)
  const out = new Uint8Array(mask.length)
  for (let y = 0; y < h; y++) {
    const y0 = Math.max(0, y - r)
    const y1 = Math.min(h - 1, y + r)
    for (let x = 0; x < w; x++) {
      const x0 = Math.max(0, x - r)
      const x1 = Math.min(w - 1, x + r)
      const area = (x1 - x0 + 1) * (y1 - y0 + 1)
      out[y * w + x] = boxSum(integral, w, x0, y0, x1, y1) === area ? 1 : 0
    }
  }
  return out
}

function buildIntegral(mask: Uint8Array, w: number, h: number): Float64Array {
  const integral = new Float64Array((w + 1) * (h + 1))
  for (let y = 0; y < h; y++) {
    let rowSum = 0
    for (let x = 0; x < w; x++) {
      rowSum += mask[y * w + x]
      integral[(y + 1) * (w + 1) + x + 1] = integral[y * (w + 1) + x + 1] + rowSum
    }
  }
  return integral
}

function boxSum(integral: Float64Array, w: number, x0: number, y0: number, x1: number, y1: number): number {
  const W = w + 1
  return (
    integral[(y1 + 1) * W + x1 + 1] -
    integral[y0 * W + x1 + 1] -
    integral[(y1 + 1) * W + x0] +
    integral[y0 * W + x0]
  )
}
