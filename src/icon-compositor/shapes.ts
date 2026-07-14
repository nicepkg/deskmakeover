// FROZEN 2026-07-10 (ADR-0019): parity oracle for the Rust port. No new styles,
// no fixes except oracle corrections. Deleted after M6 certification.

import type { IconShape } from '@/bridge/types'

// Icon-shape geometry — the CANONICAL authoring truth for every silhouette.
// The panel chips (lib/shape-paths.ts) and the engine masks (raster.ts
// shapeMask, analysis IoU) both derive from THIS file, so what the user picks
// is exactly what bakes — chip and tile can never drift apart again.
//
// Curve engine (owner call 2026-07-09): Figma-style corner rounding +
// cornerSmoothing for arbitrary polygons, ported from msurguy/squircle-path-kit
// (MIT; itself derived from Figma's "Desperately seeking squircles" and
// verified Figma-exact to 0.01px). Plain border-radius corners read cheap;
// the smoothing ramp (tangent cubics flanking a shrunken arc) is what makes
// iOS-class silhouettes. Proportions for the rounded family come from
// progressier.com/maskable-icons-editor (the catalog's original source);
// Flower is maskable.app's OEM mask (MIT). Apple keeps the TRUE iOS
// continuous-corner constants; Samsung keeps the official One UI path.
// The C# IconShapeGeometry re-port from this file rides the Windows batch.

type Pt = [number, number]

/** Apple continuous-corner radius as a fraction of the tile size (iOS = 0.225). */
export const APPLE_CORNER_FACTOR = 0.225

/** True when the point (in the size×size box) is inside the shape. */
export function shapeContains(shape: IconShape, x: number, y: number, size: number): boolean {
  if (size <= 0) throw new RangeError('size must be positive')
  const h = size / 2
  switch (shape) {
    case 'Circle': {
      const dx = (x - h) / h
      const dy = (y - h) / h
      return dx * dx + dy * dy <= 1
    }
    case 'None':
      return x >= 0 && y >= 0 && x <= size && y <= size
    default:
      return pointInPolygon(polygonFor(shape, size), x, y)
  }
}

/** The closed boundary polygon for a shape at a size (tests + geometry reuse). */
export function shapeOutline(shape: IconShape, size: number): readonly Pt[] {
  if (shape === 'None') return [[0, 0], [size, 0], [size, size], [0, size]]
  if (shape === 'Circle') return circleOutline(size)
  return polygonFor(shape, size)
}

const polyCache = new Map<string, Pt[]>()

function polygonFor(shape: IconShape, size: number): Pt[] {
  const key = `${shape}|${size}`
  let poly = polyCache.get(key)
  if (!poly) {
    poly = buildPolygon(shape, size)
    polyCache.set(key, poly)
  }
  return poly
}

function buildPolygon(shape: IconShape, size: number): Pt[] {
  switch (shape) {
    case 'Apple':
      return applePolygon(size)
    case 'Circle':
      return circleOutline(size)
    case 'Tile':
    case 'Teardrop':
    case 'Bookmark':
    case 'Lemon':
    case 'Diamond':
    case 'Folder':
      return flattenCorners(smoothCorners(SMOOTH_SHAPES[shape]), size)
    case 'Samsung':
    case 'Flower':
    case 'Pebble':
      return samplePath(PATHS[shape], size)
    default:
      throw new RangeError(`no polygon for shape ${shape}`)
  }
}

// ---- Figma corner smoothing for arbitrary polygons ---------------------------
// Ported from squircle-path-kit/src/core.ts (MIT). Per corner with opening
// angle φ, radius R, smoothing ξ: q = R/tan(φ/2), p = (1+ξ)q, the circular
// arc keeps (1−ξ) of the turn and a tangent cubic ramps in on each side.
// Budgets split shared edges proportionally so corners never collide.

export interface SmoothVertex {
  x: number
  y: number
  /** Corner radius (0..100 units). */
  r: number
  /** Figma cornerSmoothing ξ ∈ [0, 1]. */
  s: number
}

interface SmoothDef {
  vertices: SmoothVertex[]
  /** Rescale so the ROUNDED outline fills 0..100 (polygons whose rounding
   *  pulls the extremes inside the box: hexagon points, diamond tips). */
  fit?: boolean
}

interface Cubic {
  cp1: Pt
  cp2: Pt
  end: Pt
}

type Corner =
  | { sharp: true; point: Pt }
  | { sharp: false; startPoint: Pt; endPoint: Pt; segments: Cubic[] }

const vsub = (a: Pt, b: Pt): Pt => [a[0] - b[0], a[1] - b[1]]
const vadd = (a: Pt, b: Pt): Pt => [a[0] + b[0], a[1] + b[1]]
const vscale = (v: Pt, s: number): Pt => [v[0] * s, v[1] * s]
const vdot = (a: Pt, b: Pt): number => a[0] * b[0] + a[1] * b[1]
const vnorm = (v: Pt): Pt => {
  const l = Math.hypot(v[0], v[1])
  return l < 1e-12 ? [0, 0] : [v[0] / l, v[1] / l]
}

/** Circular arc → cubic segments (κ construction, sign-carrying sweep). */
function arcCubics(center: Pt, radius: number, startAngle: number, sweep: number): Cubic[] {
  if (Math.abs(sweep) < 1e-10) return []
  const maxSweep = Math.PI / 2
  if (Math.abs(sweep) > maxSweep + 1e-6) {
    const n = Math.ceil(Math.abs(sweep) / maxSweep)
    const seg = sweep / n
    const out: Cubic[] = []
    for (let i = 0; i < n; i++) out.push(...arcCubics(center, radius, startAngle + i * seg, seg))
    return out
  }
  const kappa = (4 / 3) * Math.tan(sweep / 4)
  const cosS = Math.cos(startAngle)
  const sinS = Math.sin(startAngle)
  const cosE = Math.cos(startAngle + sweep)
  const sinE = Math.sin(startAngle + sweep)
  const p0: Pt = vadd(center, [cosS * radius, sinS * radius])
  const p3: Pt = vadd(center, [cosE * radius, sinE * radius])
  return [{
    cp1: vadd(p0, vscale([-sinS, cosS], kappa * radius)),
    cp2: vsub(p3, vscale([-sinE, cosE], kappa * radius)),
    end: p3,
  }]
}

function computeCorner(prev: Pt, curr: Pt, next: Pt, radius: number, smoothness: number, budget: number): Corner {
  const dirIn = vnorm(vsub(prev, curr))
  const dirOut = vnorm(vsub(next, curr))
  const d = Math.max(-1, Math.min(1, vdot(dirIn, dirOut)))
  const phi = Math.acos(d)
  const halfPhi = phi / 2
  if (halfPhi < 1e-6 || Math.abs(phi - Math.PI) < 1e-6 || radius < 1e-6) {
    return { sharp: true, point: curr }
  }

  const sinHalf = Math.sin(halfPhi)
  const tanHalf = Math.tan(halfPhi)
  let q = radius / tanHalf
  let xi = Math.max(0, Math.min(1, smoothness))
  if (q > budget) {
    q = budget
    xi = 0
  } else if ((1 + xi) * q > budget) {
    xi = budget / q - 1
  }
  const p = (1 + xi) * q
  const effR = q * tanHalf
  if (effR < 1e-6 || q < 1e-6) return { sharp: true, point: curr }

  const bisector = vnorm(vadd(dirIn, dirOut))
  const center = vadd(curr, vscale(bisector, effR / sinHalf))
  const tangentIn = vadd(curr, vscale(dirIn, q))
  const tangentOut = vadd(curr, vscale(dirOut, q))

  const radialIn = vnorm(vsub(tangentIn, center))
  const isCCW = vdot([-radialIn[1], radialIn[0]], vscale(dirIn, -1)) > 0
  const startAngle = Math.atan2(radialIn[1], radialIn[0])
  const radialOut = vnorm(vsub(tangentOut, center))
  let sweep = Math.atan2(radialOut[1], radialOut[0]) - startAngle
  if (isCCW) {
    while (sweep < 0) sweep += 2 * Math.PI
    if (sweep > 2 * Math.PI - 1e-6) sweep -= 2 * Math.PI
  } else {
    while (sweep > 0) sweep -= 2 * Math.PI
    if (sweep < -2 * Math.PI + 1e-6) sweep += 2 * Math.PI
  }

  if (xi < 1e-6) {
    return { sharp: false, startPoint: tangentIn, endPoint: tangentOut, segments: arcCubics(center, effR, startAngle, sweep) }
  }

  const turn = Math.PI - phi
  const beta = (turn / 2) * xi
  const t = effR * Math.tan(beta / 2)
  const b = (p - (q - t)) / 3
  const a = 2 * b

  const reducedSweep = sweep * (1 - xi)
  const rStart = startAngle + sweep / 2 - reducedSweep / 2
  const arcSegments = Math.abs(reducedSweep) > 1e-6 ? arcCubics(center, effR, rStart, reducedSweep) : []
  const arcStartPt: Pt = vadd(center, [Math.cos(rStart) * effR, Math.sin(rStart) * effR])

  const inBezier: Cubic = {
    cp1: vadd(curr, vscale(dirIn, p - a)),
    cp2: vadd(curr, vscale(dirIn, q - t)),
    end: arcStartPt,
  }
  const outBezier: Cubic = {
    cp1: vadd(curr, vscale(dirOut, q - t)),
    cp2: vadd(curr, vscale(dirOut, p - a)),
    end: vadd(curr, vscale(dirOut, p)),
  }
  return {
    sharp: false,
    startPoint: vadd(curr, vscale(dirIn, p)),
    endPoint: outBezier.end,
    segments: [inBezier, ...arcSegments, outBezier],
  }
}

/** Order-independent proportional per-edge budget split (squircle-path-kit). */
function resolveBudgets(demands: { q: number; p: number }[], edgeLengths: number[]): number[] {
  const n = demands.length
  const maxP = demands.map((d) => d.p)
  const allowNext = new Array<number>(n)
  const allowPrev = new Array<number>(n)
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n
    const total = maxP[i] + maxP[j]
    if (total > edgeLengths[i] && total > 1e-6) {
      allowNext[i] = edgeLengths[i] * (maxP[i] / total)
      allowPrev[j] = edgeLengths[i] * (maxP[j] / total)
    } else {
      allowNext[i] = maxP[i]
      allowPrev[j] = maxP[j]
    }
  }
  const budgets = new Array<number>(n)
  for (let i = 0; i < n; i++) budgets[i] = Math.min(maxP[i], allowNext[i], allowPrev[i])
  return budgets
}

function smoothCorners(def: SmoothDef): Corner[] {
  let vertices = def.vertices
  let corners = cornersOf(vertices)
  if (def.fit) {
    // Rounding pulls extremes inside the box — rescale the VERTICES so the
    // rounded outline fills 0..100 (two passes converge under 0.1).
    for (let pass = 0; pass < 2; pass++) {
      const pts = flattenCorners(corners, 100)
      const xs = pts.map((p) => p[0])
      const ys = pts.map((p) => p[1])
      const minX = Math.min(...xs)
      const maxX = Math.max(...xs)
      const minY = Math.min(...ys)
      const maxY = Math.max(...ys)
      vertices = vertices.map((v) => ({
        ...v,
        x: ((v.x - minX) * 100) / (maxX - minX),
        y: ((v.y - minY) * 100) / (maxY - minY),
      }))
      corners = cornersOf(vertices)
    }
  }
  return corners
}

function cornersOf(vertices: SmoothVertex[]): Corner[] {
  const n = vertices.length
  const edgeLengths: number[] = []
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n
    edgeLengths.push(Math.hypot(vertices[j].x - vertices[i].x, vertices[j].y - vertices[i].y))
  }
  const demands = vertices.map((v, i) => {
    const prev = vertices[(i - 1 + n) % n]
    const next = vertices[(i + 1) % n]
    const dirIn = vnorm([prev.x - v.x, prev.y - v.y])
    const dirOut = vnorm([next.x - v.x, next.y - v.y])
    const d = Math.max(-1, Math.min(1, vdot(dirIn, dirOut)))
    const tanHalf = Math.tan(Math.acos(d) / 2)
    const q = tanHalf > 1e-9 ? v.r / tanHalf : 0
    return { q, p: (1 + Math.max(0, Math.min(1, v.s))) * q }
  })
  const budgets = resolveBudgets(demands, edgeLengths)
  return vertices.map((v, i) => {
    const prev = vertices[(i - 1 + n) % n]
    const next = vertices[(i + 1) % n]
    return computeCorner([prev.x, prev.y], [v.x, v.y], [next.x, next.y], v.r, v.s, budgets[i])
  })
}

/** Steps per cubic when flattening for the mask — chord error < 0.1 px @256. */
const FLATTEN_STEPS = 12

function flattenCorners(corners: Corner[], size: number): Pt[] {
  const s = size / 100
  const pts: Pt[] = []
  let cur: Pt | null = null
  const emit = (p: Pt) => pts.push([p[0] * s, p[1] * s])
  for (const corner of corners) {
    if (corner.sharp) {
      emit(corner.point)
      cur = corner.point
      continue
    }
    emit(corner.startPoint)
    cur = corner.startPoint
    for (const c of corner.segments) {
      for (let i = 1; i <= FLATTEN_STEPS; i++) {
        const t = i / FLATTEN_STEPS
        const u = 1 - t
        const w0 = u * u * u
        const w1 = 3 * u * u * t
        const w2 = 3 * u * t * t
        const w3 = t * t * t
        emit([
          w0 * cur[0] + w1 * c.cp1[0] + w2 * c.cp2[0] + w3 * c.end[0],
          w0 * cur[1] + w1 * c.cp1[1] + w2 * c.cp2[1] + w3 * c.end[1],
        ])
      }
      cur = c.end
    }
  }
  return pts
}

export type SmoothShape = 'Tile' | 'Teardrop' | 'Bookmark' | 'Lemon' | 'Diamond' | 'Folder' | 'File'

/** Rect-family corner order: TL, TR, BR, BL (radii from the Progressier
 *  catalog); ξ = 0.6 ≈ the iOS smoothing feel. Corners whose radius already
 *  eats the whole edge (r=50 pairs) auto-clamp ξ — silhouettes stay true. */
const XI = 0.6

function smoothRect(tl: number, tr: number, br: number, bl: number, s = XI): SmoothDef {
  return {
    vertices: [
      { x: 0, y: 0, r: tl, s },
      { x: 100, y: 0, r: tr, s },
      { x: 100, y: 100, r: br, s },
      { x: 0, y: 100, r: bl, s },
    ],
  }
}

const SMOOTH_SHAPES: Record<SmoothShape, SmoothDef> = {
  Tile: smoothRect(10, 10, 10, 10),
  // maskable.app's drop proportions (round 50% 50% 20%): three semicircle
  // corners + a softly-pointed BR — reads far silkier than the 40% family
  // (owner call 2026-07-09: "teardrop 的曲线不够丝滑").
  Teardrop: smoothRect(50, 50, 20, 50),
  Bookmark: smoothRect(20, 20, 50, 50),
  Lemon: smoothRect(10, 50, 10, 50),
  // The FOLDER silhouette (ADR-0017 ladder fix, owner 2026-07-10): a body
  // with a raised left tab — the one shape that reads "folder" without a
  // label. Tab depth 10 units ≈ 5px at 48px: visible as a top-edge step at
  // grid size, gentle at 256. Body top sits at y=16 so the ~67% content box
  // (y≈16.5..83.5) never clips into the tab notch.
  Folder: {
    vertices: [
      { x: 0, y: 6, r: 8, s: XI },
      { x: 36, y: 6, r: 6, s: XI },
      { x: 46, y: 16, r: 4, s: XI },
      { x: 100, y: 16, r: 10, s: XI },
      { x: 100, y: 100, r: 12, s: XI },
      { x: 0, y: 100, r: 12, s: XI },
    ],
  },
  // 文件 File (spec 02 V2, owner-disposed 2026-07-15; corners softened per owner
  // "别像狗啃"): dog-eared document — top-right 45° cut c=30, outer corners r12
  // (Folder-family weight). The two cut-edge endpoints carry a generous r16 +
  // high smoothing (s=0.85) so the fold is a soft rounded corner, not a sharp
  // chamfer. Solid cut-away; folded-page light belongs to the Fold MARK.
  File: {
    vertices: [
      { x: 0, y: 0, r: 12, s: XI },
      { x: 70, y: 0, r: 16, s: 0.85 },
      { x: 100, y: 30, r: 16, s: 0.85 },
      { x: 100, y: 100, r: 12, s: XI },
      { x: 0, y: 100, r: 12, s: XI },
    ],
  },
  // 45°-rotated square, softly rounded tips.
  Diamond: {
    fit: true,
    vertices: [
      { x: 50, y: 0, r: 20, s: 0.8 },
      { x: 100, y: 50, r: 20, s: 0.8 },
      { x: 50, y: 100, r: 20, s: 0.8 },
      { x: 0, y: 50, r: 20, s: 0.8 },
    ],
  },
}

/** SVG path `d` for a smooth shape scaled to a box — exact cubics, chips use
 *  this so the swatch silhouette and the engine mask share one authoring.
 *  `inset` shrinks the ink inside the box (chip breathing room; masks use 0). */
export function smoothShapePathD(shape: SmoothShape, box: number, inset = 0): string {
  const corners = smoothCorners(SMOOTH_SHAPES[shape])
  const s = (box - 2 * inset) / 100
  const f = (v: number) => +(inset + v * s).toFixed(2)
  let d = ''
  for (let i = 0; i < corners.length; i++) {
    const corner = corners[i]
    const start: Pt = corner.sharp ? corner.point : corner.startPoint
    d += i === 0 ? `M${f(start[0])} ${f(start[1])}` : ` L${f(start[0])} ${f(start[1])}`
    if (!corner.sharp) {
      for (const c of corner.segments) {
        d += ` C${f(c.cp1[0])} ${f(c.cp1[1])} ${f(c.cp2[0])} ${f(c.cp2[1])} ${f(c.end[0])} ${f(c.end[1])}`
      }
    }
  }
  return `${d} Z`
}

// ---- Authored cubic silhouettes ----------------------------------------------

type Seg =
  | { t: 'L'; to: Pt }
  | { t: 'Q'; c: Pt; to: Pt }
  | { t: 'C'; c1: Pt; c2: Pt; to: Pt }

interface ShapeDef {
  start: Pt
  segs: Seg[]
}

export type CurvedShape = 'Samsung' | 'Flower' | 'Pebble'

const PATHS: Record<CurvedShape, ShapeDef> = {
  // The official One UI adaptive-icon mask (four cubics, unchanged numbers).
  Samsung: {
    start: [50, 0],
    segs: [
      { t: 'C', c1: [10, 0], c2: [0, 10], to: [0, 50] },
      { t: 'C', c1: [0, 90], c2: [10, 100], to: [50, 100] },
      { t: 'C', c1: [90, 100], c2: [100, 90], to: [100, 50] },
      { t: 'C', c1: [100, 10], c2: [90, 0], to: [50, 0] },
    ],
  },
  // Four-petal OEM flower — maskable.app's `flower` clipPath (MIT), arcs
  // converted to cubics and normalized to the full 0..100 box.
  Flower: {
    start: [50, 0],
    segs: [
      { t: 'C', c1: [60.6, 0], c2: [69.9, 5.3], to: [75.6, 13.5] },
      { t: 'C', c1: [78.56, 17.81], c2: [82.29, 21.54], to: [86.6, 24.5] },
      { t: 'C', c1: [95, 30.27], c2: [100.01, 39.81], to: [100, 50] },
      { t: 'C', c1: [100, 60.6], c2: [94.7, 69.9], to: [86.5, 75.6] },
      { t: 'C', c1: [82.19, 78.56], c2: [78.46, 82.29], to: [75.5, 86.6] },
      { t: 'C', c1: [69.73, 95], c2: [60.19, 100.01], to: [50, 100] },
      { t: 'C', c1: [39.4, 100], c2: [30.1, 94.7], to: [24.4, 86.5] },
      { t: 'C', c1: [21.44, 82.19], c2: [17.71, 78.46], to: [13.4, 75.5] },
      { t: 'C', c1: [5, 69.73], c2: [-0.01, 60.19], to: [0, 50] },
      { t: 'C', c1: [0, 39.4], c2: [5.3, 30.1], to: [13.5, 24.4] },
      { t: 'C', c1: [17.81, 21.44], c2: [21.54, 17.71], to: [24.5, 13.4] },
      { t: 'C', c1: [30.27, 5], c2: [39.81, -0.01], to: [50, 0] },
    ],
  },
  // Organic pebble — maskable.app's `pebble` clipPath (MIT), normalized to the
  // full 0..100 box (owner-picked addition 2026-07-09).
  Pebble: {
    start: [55, 0],
    segs: [
      { t: 'C', c1: [25, 0], c2: [0, 25], to: [0, 50] },
      { t: 'C', c1: [0, 78], c2: [28, 100], to: [55, 100] },
      { t: 'C', c1: [85, 100], c2: [100, 85], to: [100, 58] },
      { t: 'C', c1: [100, 30], c2: [86, 0], to: [55, 0] },
    ],
  },
}

/** Steps per curved segment — chord error at 256 px is < 0.1 px. */
const CURVE_STEPS = 24

function samplePath(def: ShapeDef, size: number): Pt[] {
  const s = size / 100
  const pts: Pt[] = [[def.start[0] * s, def.start[1] * s]]
  let cur = def.start
  for (const seg of def.segs) {
    if (seg.t === 'L') {
      pts.push([seg.to[0] * s, seg.to[1] * s])
    } else {
      // Quadratics are promoted to cubics: c1 = p0 + ⅔(c−p0), c2 = p1 + ⅔(c−p1).
      const c1: Pt = seg.t === 'C' ? seg.c1
        : [cur[0] + (2 / 3) * (seg.c[0] - cur[0]), cur[1] + (2 / 3) * (seg.c[1] - cur[1])]
      const c2: Pt = seg.t === 'C' ? seg.c2
        : [seg.to[0] + (2 / 3) * (seg.c[0] - seg.to[0]), seg.to[1] + (2 / 3) * (seg.c[1] - seg.to[1])]
      for (let i = 1; i <= CURVE_STEPS; i++) {
        const t = i / CURVE_STEPS
        const u = 1 - t
        const w0 = u * u * u
        const w1 = 3 * u * u * t
        const w2 = 3 * u * t * t
        const w3 = t * t * t
        pts.push([
          (w0 * cur[0] + w1 * c1[0] + w2 * c2[0] + w3 * seg.to[0]) * s,
          (w0 * cur[1] + w1 * c1[1] + w2 * c2[1] + w3 * seg.to[1]) * s,
        ])
      }
    }
    cur = seg.to
  }
  return pts
}

/** SVG path `d` for a curved shape scaled to a box — the chips consume THIS,
 *  so the swatch silhouette and the engine mask are the same authoring.
 *  `inset` shrinks the ink inside the box (chip breathing room; masks use 0). */
export function curvedShapePathD(shape: CurvedShape, box: number, inset = 0): string {
  const s = (box - 2 * inset) / 100
  const n = (v: number) => +(inset + v * s).toFixed(2)
  const def = PATHS[shape]
  const parts = [`M${n(def.start[0])} ${n(def.start[1])}`]
  for (const seg of def.segs) {
    if (seg.t === 'L') parts.push(`L${n(seg.to[0])} ${n(seg.to[1])}`)
    else if (seg.t === 'Q') parts.push(`Q${n(seg.c[0])} ${n(seg.c[1])} ${n(seg.to[0])} ${n(seg.to[1])}`)
    else parts.push(`C${n(seg.c1[0])} ${n(seg.c1[1])} ${n(seg.c2[0])} ${n(seg.c2[1])} ${n(seg.to[0])} ${n(seg.to[1])}`)
  }
  parts.push('Z')
  return parts.join(' ')
}

// ---- Apple: the TRUE iOS continuous-corner squircle (three cubics/corner) ----

const APPLE_CORNER_STEPS = 12

function applePolygon(size: number): Pt[] {
  const r = APPLE_CORNER_FACTOR * size
  const tl = (x: number, y: number): Pt => [x * r, y * r]
  const tr = (x: number, y: number): Pt => [size - x * r, y * r]
  const br = (x: number, y: number): Pt => [size - x * r, size - y * r]
  const bl = (x: number, y: number): Pt => [x * r, size - y * r]

  const pts: Pt[] = [tl(1.528665, 0)]
  let cur = line(pts, tr(1.528665, 0))
  cur = corner(pts, cur,
    tr(1.08849296, 0), tr(0.86840694, 0), tr(0.63149379, 0.07491139),
    tr(0.37282383, 0.16905956), tr(0.16905956, 0.37282383), tr(0.07491139, 0.63149379),
    tr(0, 0.86840694), tr(0, 1.08849296), tr(0, 1.52866498))
  cur = line(pts, br(0, 1.528665))
  cur = corner(pts, cur,
    br(0, 1.08849296), br(0, 0.86840694), br(0.07491139, 0.63149379),
    br(0.16905956, 0.37282383), br(0.37282383, 0.16905956), br(0.63149379, 0.07491139),
    br(0.86840694, 0), br(1.08849296, 0), br(1.52866498, 0))
  cur = line(pts, bl(1.528665, 0))
  cur = corner(pts, cur,
    bl(1.08849296, 0), bl(0.86840694, 0), bl(0.63149379, 0.07491139),
    bl(0.37282383, 0.16905956), bl(0.16905956, 0.37282383), bl(0.07491139, 0.63149379),
    bl(0, 0.86840694), bl(0, 1.08849296), bl(0, 1.52866498))
  cur = line(pts, tl(0, 1.528665))
  corner(pts, cur,
    tl(0, 1.08849296), tl(0, 0.86840694), tl(0.07491139, 0.63149379),
    tl(0.16905956, 0.37282383), tl(0.37282383, 0.16905956), tl(0.63149379, 0.07491139),
    tl(0.86840694, 0), tl(1.08849296, 0), tl(1.52866498, 0))
  return pts
}

/** SVG path `d` for the TRUE iOS Apple squircle scaled to a box — chips consume THIS,
 *  not lib/geometry.ts's Lamé (|x|^5+|y|^5) approximation, so the picked swatch and the
 *  engine mask (`shapeOutline('Apple')` = this same polygon) are ONE authoring and can't
 *  drift. `inset` shrinks the ink inside the box (chip breathing room; masks use 0). */
export function applePathD(box: number, inset = 0): string {
  const pts = applePolygon(box - 2 * inset)
  let d = ''
  for (let i = 0; i < pts.length; i++) {
    d += `${i === 0 ? 'M' : ' L'}${+(inset + pts[i][0]).toFixed(2)} ${+(inset + pts[i][1]).toFixed(2)}`
  }
  return `${d} Z`
}

function line(pts: Pt[], end: Pt): Pt {
  pts.push(end)
  return end
}

function corner(
  pts: Pt[], cur: Pt,
  a1: Pt, a2: Pt, a3: Pt, b1: Pt, b2: Pt, b3: Pt, c1: Pt, c2: Pt, c3: Pt,
): Pt {
  cur = bezier(pts, cur, a1, a2, a3)
  cur = bezier(pts, cur, b1, b2, b3)
  return bezier(pts, cur, c1, c2, c3)
}

function bezier(pts: Pt[], p0: Pt, c1: Pt, c2: Pt, end: Pt): Pt {
  for (let i = 1; i <= APPLE_CORNER_STEPS; i++) {
    const t = i / APPLE_CORNER_STEPS
    const u = 1 - t
    const w0 = u * u * u
    const w1 = 3 * u * u * t
    const w2 = 3 * u * t * t
    const w3 = t * t * t
    pts.push([
      w0 * p0[0] + w1 * c1[0] + w2 * c2[0] + w3 * end[0],
      w0 * p0[1] + w1 * c1[1] + w2 * c2[1] + w3 * end[1],
    ])
  }
  return end
}

function circleOutline(size: number): Pt[] {
  const n = 128
  const h = size / 2
  const pts: Pt[] = []
  for (let i = 0; i < n; i++) {
    const t = (i / n) * Math.PI * 2
    pts.push([h + h * Math.cos(t), h + h * Math.sin(t)])
  }
  return pts
}

function pointInPolygon(poly: Pt[], x: number, y: number): boolean {
  let inside = false
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i]
    const [xj, yj] = poly[j]
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) {
      inside = !inside
    }
  }
  return inside
}
