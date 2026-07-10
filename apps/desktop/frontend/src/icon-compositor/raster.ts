import type { IconShape } from '@/bridge/types'
import { shapeContains } from './shapes'

// Pure raster primitives — a 1:1 TypeScript port of the frozen C# oracle
// (IconRendering/Marks/RasterOps.cs, ADR-0015 D3). Straight-alpha RGBA on
// Uint8ClampedArray, row-major; alpha/coverage fields are Float64Array.
// Same math at every resolution, so display preview and 256 bake agree.

/** A straight-alpha RGBA bitmap (row-major, 4 bytes per pixel). */
export interface Raster {
  width: number
  height: number
  data: Uint8ClampedArray
}

/** An opaque-or-translucent colour in 0-255 channels (straight alpha). */
export interface Rgba {
  r: number
  g: number
  b: number
  a: number
}

export const TRANSPARENT: Rgba = { r: 0, g: 0, b: 0, a: 0 }
export const WHITE: Rgba = { r: 255, g: 255, b: 255, a: 255 }

export function makeRaster(width: number, height = width): Raster {
  return { width, height, data: new Uint8ClampedArray(width * height * 4) }
}

export function cloneRaster(src: Raster): Raster {
  return { width: src.width, height: src.height, data: new Uint8ClampedArray(src.data) }
}

export function fromRgbInt(rgb: number): Rgba {
  return { r: (rgb >> 16) & 0xff, g: (rgb >> 8) & 0xff, b: rgb & 0xff, a: 255 }
}

export function rgbaOf(rgb: number, alpha: number): Rgba {
  return { r: (rgb >> 16) & 0xff, g: (rgb >> 8) & 0xff, b: rgb & 0xff, a: Math.round(clamp01(alpha) * 255) }
}

/** '#RRGGBB' → packed 0xRRGGBB int (the C# side used ints throughout). */
export function hexToInt(hex: string): number {
  return parseInt(hex.replace('#', ''), 16) & 0xffffff
}

export function clamp01(v: number): number {
  return v < 0 ? 0 : v > 1 ? 1 : v
}

export function clampByte(v: number): number {
  return v < 0 ? 0 : v > 255 ? 255 : Math.round(v)
}

/**
 * Straight-alpha Porter-Duff "over" written INTO the raster at pixel index i
 * (byte offset i*4). Result RGB normalised by the output alpha, so translucent
 * colour over transparency never darkens (RasterOps.Over).
 */
export function overAt(dst: Uint8ClampedArray, i4: number, r: number, g: number, b: number, a: number): void {
  if (a <= 0) return
  const ba = dst[i4 + 3]
  if (a >= 255 || ba === 0) {
    dst[i4] = r
    dst[i4 + 1] = g
    dst[i4 + 2] = b
    dst[i4 + 3] = a
    return
  }
  const ta = a / 255
  const bf = (ba / 255) * (1 - ta)
  const outA = ta + bf
  const inv = 1 / outA
  dst[i4] = Math.round((r * ta + dst[i4] * bf) * inv)
  dst[i4 + 1] = Math.round((g * ta + dst[i4 + 1] * bf) * inv)
  dst[i4 + 2] = Math.round((b * ta + dst[i4 + 2] * bf) * inv)
  dst[i4 + 3] = Math.round(outA * 255)
}

/** RasterOps.Mix — CSS color-mix(in srgb): component-wise blend incl. alpha; pct weights a. */
export function mix(a: Rgba, b: Rgba, pct: number): Rgba {
  const q = 1 - pct
  return {
    r: clampByte(a.r * pct + b.r * q),
    g: clampByte(a.g * pct + b.g * q),
    b: clampByte(a.b * pct + b.b * q),
    a: clampByte(a.a * pct + b.a * q),
  }
}

/** RasterOps.Fade — color-mix toward transparent: keep rgb, scale alpha. */
export function fade(c: Rgba, pct: number): Rgba {
  return { r: c.r, g: c.g, b: c.b, a: clampByte(c.a * pct) }
}

/** RasterOps.Paint — composite a translucent colour over target[i], gated by coverage. */
export function paint(target: Raster, i: number, colour: Rgba, coverage: number): void {
  if (coverage <= 0 || colour.a === 0) return
  const cov = coverage > 1 ? 1 : coverage
  overAt(target.data, i * 4, colour.r, colour.g, colour.b, Math.round(colour.a * cov))
}

// ---- shape masks (RasterOps.ShapeMask) --------------------------------------

// Boundary pixels get 16×16 coverage sampling; interior/exterior classified from
// a shared corner grid. Cached per (shape, buffer, size, offset) exactly like C#.
const EDGE_SUB_SAMPLES = 16
const maskCache = new Map<string, Float64Array>()

export function shapeMask(
  shape: IconShape,
  bufferSize: number,
  shapeSize: number,
  offsetX: number,
  offsetY: number,
): Float64Array {
  if (shapeSize <= 0) throw new RangeError('shapeSize must be positive')
  const key = `${shape}|${bufferSize}|${shapeSize}|${offsetX}|${offsetY}`
  let mask = maskCache.get(key)
  if (!mask) {
    mask = buildShapeMask(shape, bufferSize, shapeSize, offsetX, offsetY)
    maskCache.set(key, mask)
  }
  return mask
}

function buildShapeMask(
  shape: IconShape,
  bufferSize: number,
  shapeSize: number,
  offsetX: number,
  offsetY: number,
): Float64Array {
  const mask = new Float64Array(bufferSize * bufferSize)
  const gridW = bufferSize + 1
  const grid = new Uint8Array(gridW * gridW)
  for (let y = 0; y <= bufferSize; y++) {
    for (let x = 0; x <= bufferSize; x++) {
      grid[y * gridW + x] = shapeContains(shape, x - offsetX, y - offsetY, shapeSize) ? 1 : 0
    }
  }

  const step = 1 / EDGE_SUB_SAMPLES
  for (let y = 0; y < bufferSize; y++) {
    for (let x = 0; x < bufferSize; x++) {
      const tl = grid[y * gridW + x]
      const tr = grid[y * gridW + x + 1]
      const bl = grid[(y + 1) * gridW + x]
      const br = grid[(y + 1) * gridW + x + 1]

      if (tl === tr && tl === bl && tl === br) {
        // Confirm with the pixel centre — a thin curve can slice a pixel whose
        // four corners agree.
        const centre = shapeContains(shape, x + 0.5 - offsetX, y + 0.5 - offsetY, shapeSize) ? 1 : 0
        if (centre === tl) {
          mask[y * bufferSize + x] = tl ? 1 : 0
          continue
        }
      }

      let inside = 0
      for (let sy = 0; sy < EDGE_SUB_SAMPLES; sy++) {
        for (let sx = 0; sx < EDGE_SUB_SAMPLES; sx++) {
          if (shapeContains(shape, x + (sx + 0.5) * step - offsetX, y + (sy + 0.5) * step - offsetY, shapeSize)) {
            inside++
          }
        }
      }
      mask[y * bufferSize + x] = inside / (EDGE_SUB_SAMPLES * EDGE_SUB_SAMPLES)
    }
  }
  return mask
}

/** RasterOps.ClipToMask — multiply every pixel's alpha by the mask coverage. */
export function clipToMask(pixels: Raster, mask: Float64Array): void {
  const d = pixels.data
  for (let i = 0; i < mask.length; i++) {
    const cov = mask[i]
    if (cov >= 1) continue
    const i4 = i * 4
    if (cov <= 0) {
      d[i4] = 0
      d[i4 + 1] = 0
      d[i4 + 2] = 0
      d[i4 + 3] = 0
    } else {
      d[i4 + 3] = Math.round(d[i4 + 3] * cov)
    }
  }
}

/** RasterOps.Shift — offset an alpha field by (dx, dy). */
export function shift(src: Float64Array, size: number, dx: number, dy: number): Float64Array {
  const o = new Float64Array(size * size)
  for (let y = 0; y < size; y++) {
    const sy = y - dy
    if (sy < 0 || sy >= size) continue
    for (let x = 0; x < size; x++) {
      const sx = x - dx
      if (sx >= 0 && sx < size) o[y * size + x] = src[sy * size + sx]
    }
  }
  return o
}

/** RasterOps.BoxBlur — separable box blur of an alpha field. */
export function boxBlur(src: Float64Array, size: number, radius: number): Float64Array {
  if (radius < 1) return src
  const w = 2 * radius + 1
  const tmp = new Float64Array(size * size)
  const clampI = (v: number) => (v < 0 ? 0 : v >= size ? size - 1 : v)
  for (let y = 0; y < size; y++) {
    let sum = 0
    for (let x = -radius; x <= radius; x++) sum += src[y * size + clampI(x)]
    for (let x = 0; x < size; x++) {
      tmp[y * size + x] = sum / w
      sum += src[y * size + clampI(x + radius + 1)] - src[y * size + clampI(x - radius)]
    }
  }
  const o = new Float64Array(size * size)
  for (let x = 0; x < size; x++) {
    let sum = 0
    for (let y = -radius; y <= radius; y++) sum += tmp[clampI(y) * size + x]
    for (let y = 0; y < size; y++) {
      o[y * size + x] = sum / w
      sum += tmp[clampI(y + radius + 1) * size + x] - tmp[clampI(y - radius) * size + x]
    }
  }
  return o
}

/** RasterOps.BackdropBlur — blurred copy of the buffer's colour (frosted seat backdrop). */
export function backdropBlur(src: Raster, radius: number): Raster {
  if (radius < 1) return cloneRaster(src)
  const size = src.width
  const n = size * size
  const chans = [new Float64Array(n), new Float64Array(n), new Float64Array(n), new Float64Array(n)]
  for (let i = 0; i < n; i++) {
    const i4 = i * 4
    chans[0][i] = src.data[i4]
    chans[1][i] = src.data[i4 + 1]
    chans[2][i] = src.data[i4 + 2]
    chans[3][i] = src.data[i4 + 3]
  }
  const blurred = chans.map((c) => boxBlur(c, size, radius))
  const o = makeRaster(size)
  for (let i = 0; i < n; i++) {
    const i4 = i * 4
    o.data[i4] = clampByte(blurred[0][i])
    o.data[i4 + 1] = clampByte(blurred[1][i])
    o.data[i4 + 2] = clampByte(blurred[2][i])
    o.data[i4 + 3] = clampByte(blurred[3][i])
  }
  return o
}

export function smoothStep01(u: number): number {
  u = clamp01(u)
  return u * u * (3 - 2 * u)
}

export function distToSegment(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const vx = bx - ax
  const vy = by - ay
  const wx = px - ax
  const wy = py - ay
  const c1 = vx * wx + vy * wy
  const c2 = vx * vx + vy * vy
  const t = c2 <= 0 ? 0 : Math.min(1, Math.max(0, c1 / c2))
  const qx = px - (ax + t * vx)
  const qy = py - (ay + t * vy)
  return Math.sqrt(qx * qx + qy * qy)
}

export function inTriangle(
  px: number, py: number, ax: number, ay: number, bx: number, by: number, cx: number, cy: number,
): boolean {
  const sign = (x1: number, y1: number, x2: number, y2: number, x3: number, y3: number) =>
    (x1 - x3) * (y2 - y3) - (x2 - x3) * (y1 - y3)
  const d1 = sign(px, py, ax, ay, bx, by)
  const d2 = sign(px, py, bx, by, cx, cy)
  const d3 = sign(px, py, cx, cy, ax, ay)
  const neg = d1 < 0 || d2 < 0 || d3 < 0
  const pos = d1 > 0 || d2 > 0 || d3 > 0
  return !(neg && pos)
}
