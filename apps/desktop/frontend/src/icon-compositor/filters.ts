import type { FilterStyle, Subject } from '@/bridge/types'
import type { Raster } from './raster'
import { clampByte, cloneRaster } from './raster'
import { luminance, monoRamp, paleTone, srgbDecode, srgbEncode, stretchedLightness } from './color'
import { downscale } from './sampling'

// 滤镜 — 1:1 port of the frozen C# oracle (IconFilters.cs, ADR-0015 D3).
// Algorithmic finishes over the COMPOSED, shape-clipped tile; marks draw after
// and adapt to the result. Luminance-structure driven, never colour driven.

export function applyFilter(tile: Raster, size: number, filter: FilterStyle, subject: Subject, tint: number): void {
  // 玻璃 alone is colour-aware: Mono tiles keep their tinted ramp on the slab.
  const hue = subject === 'Mono' ? tint : null
  switch (filter) {
    case 'Gloss':
      gloss(tile, size)
      break
    case 'Glass':
      glass(tile, size, hue)
      break
    case 'Pixel':
      pixelate(tile, size)
      break
    case 'Sticker':
      sticker(tile, size)
      break
    default:
      break
  }
}

// ---- 光泽 (glossy sheen) ----
// The classic aqua finish the FilterSwatch previews: a white specular sweep
// over the tile's upper third whose lower boundary bows UP at the centre,
// plus a whisper of depth below. Structure-only — the icon's colours stay.

const GLOSS_SHEEN_TOP = 0.34
const GLOSS_SHEEN_EDGE = 0.12
const GLOSS_DEPTH = 0.07

function gloss(tile: Raster, size: number): void {
  const d = tile.data
  for (let y = 0; y < size; y++) {
    const v = y / (size - 1)
    for (let x = 0; x < size; x++) {
      const i4 = (y * size + x) * 4
      if (d[i4 + 3] === 0) continue
      const u = x / (size - 1)
      // Sweep boundary: 0.42 at the sides bowing up to 0.315 at the centre
      // (matches the swatch art's quadratic).
      const boundary = 0.42 - 0.105 * (1 - (2 * u - 1) * (2 * u - 1))
      if (v < boundary) {
        // Screen-blend white in linear light — stronger at the top, soft at
        // the boundary so the sweep never bands.
        const fade = smoothStepRange(0, 0.05, boundary - v)
        const a = (GLOSS_SHEEN_EDGE + (GLOSS_SHEEN_TOP - GLOSS_SHEEN_EDGE) * (1 - v / boundary)) * fade
        for (let c = 0; c < 3; c++) {
          const lin = srgbDecode(d[i4 + c])
          d[i4 + c] = srgbEncode(lin + (1 - lin) * a)
        }
      } else {
        // Gentle darkening toward the bottom sells the curved-glass body.
        const depth = smoothStepRange(boundary, 1, v) * GLOSS_DEPTH
        d[i4] = clampByte(d[i4] * (1 - depth))
        d[i4 + 1] = clampByte(d[i4 + 1] * (1 - depth))
        d[i4 + 2] = clampByte(d[i4 + 2] * (1 - depth))
      }
    }
  }
}

// ---- 玻璃 (liquid glass) ----
// RESTORED to the original translucent slab (owner order 2026-07-10: the T7
// rim rework lost the 透明玻璃 look he shipped this filter for; his call
// outranks the findability panel's contrast complaint). Body/frosted-subject/
// fresnel/refraction/grounding-halo semantics are the frozen C# oracle's.

const PLATE_BODY_ALPHA = 0.44
const PLATE_FRESNEL_ALPHA = 0.16
const GLYPH_ALPHA = 0.94

function glass(tile: Raster, size: number, hue: number | null): void {
  const t = stretchedLightness(tile)

  let plateR: number
  let plateG: number
  let plateB: number
  let glyphR: number
  let glyphG: number
  let glyphB: number
  if (hue !== null) {
    const plate = paleTone(hue)
    plateR = plate.r
    plateG = plate.g
    plateB = plate.b
    const pale = monoRamp(0.985, hue)
    glyphR = Math.min(255, pale.r + 16)
    glyphG = Math.min(255, pale.g + 16)
    glyphB = Math.min(255, pale.b + 16)
  } else {
    plateR = 238; plateG = 243; plateB = 248
    glyphR = 252; glyphG = 253; glyphB = 255
  }

  const dist = chamferDistance(tile, size, true)
  const outside = chamferDistance(tile, size, false)
  const falloff = size * 0.05
  const warpPx = size * 0.024
  const haloPx = Math.max(2, size * 0.014)

  const src = cloneRaster(tile)
  const sd = src.data
  const td = tile.data
  const subject = new Float64Array(size * size)

  const distAt = (x: number, y: number, fallback: number) => {
    if (x < 0 || y < 0 || x >= size || y >= size) return 0
    const v = dist[y * size + x]
    return v < 0 ? fallback : v
  }

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x
      const i4 = i * 4
      if (sd[i4 + 3] === 0) {
        // The grounding halo just outside the slab.
        const od = outside[i] / 3
        if (od >= 0 && od <= haloPx) {
          const fadeT = 1 - od / haloPx
          td[i4] = 12
          td[i4 + 1] = 14
          td[i4 + 2] = 18
          td[i4 + 3] = Math.round(34 * fadeT * fadeT)
        }
        continue
      }

      const d = dist[i] / 3
      const edge = Math.exp(-d / falloff)

      const gx = distAt(x - 1, y, dist[i]) - distAt(x + 1, y, dist[i])
      const gy = distAt(x, y - 1, dist[i]) - distAt(x, y + 1, dist[i])
      const glen = Math.sqrt(gx * gx + gy * gy)
      let nx = 0
      let ny = 0
      if (glen > 1e-6) {
        nx = gx / glen
        ny = gy / glen
      }

      // Refraction: content near the rim samples slightly inward.
      const warp = Math.pow(edge, 1.5) * warpPx
      const sx = Math.min(size - 1, Math.max(0, Math.round(x - nx * warp)))
      const sy = Math.min(size - 1, Math.max(0, Math.round(y - ny * warp)))
      const si = sy * size + sx
      const dense = 1 - (sd[si * 4 + 3] === 0 ? t[i] : t[si])

      subject[i] = smoothStepRange(0.48, 0.78, dense)

      let alpha = PLATE_BODY_ALPHA + PLATE_FRESNEL_ALPHA * edge + 0.06 * (1 - y / size)
      let r: number = plateR
      let g: number = plateG
      let b: number = plateB

      const lx = -0.7071
      const ly = -0.7071
      const facing = nx * lx + ny * ly
      if (facing > 0) {
        const specular = edge * edge * facing * facing
        r += (255 - r) * specular
        g += (255 - g) * specular
        b += (255 - b) * specular
        alpha += specular * 0.24
      } else {
        const shade = edge * facing * facing * 0.4
        r *= 1 - 0.22 * shade
        g *= 1 - 0.22 * shade
        b *= 1 - 0.22 * shade
      }

      const m = subject[i]
      r += (glyphR - r) * m
      g += (glyphG - g) * m
      b += (glyphB - b) * m
      alpha += (GLYPH_ALPHA - alpha) * m

      alpha = Math.min(0.96, alpha)
      td[i4] = clampByte(r)
      td[i4 + 1] = clampByte(g)
      td[i4 + 2] = clampByte(b)
      td[i4 + 3] = Math.round(alpha * sd[i4 + 3])
    }
  }

  // Soft drop shadow under the glyph (light from the top-left).
  const off = Math.max(1, Math.round(size * 0.008))
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x
      const i4 = i * 4
      if (td[i4 + 3] === 0 || subject[i] > 0.15) continue
      const sx2 = Math.min(size - 1, Math.max(0, x - off))
      const sy2 = Math.min(size - 1, Math.max(0, y - off))
      const shadow = subject[sy2 * size + sx2] * 0.3
      if (shadow > 0.01) {
        td[i4] = Math.round(td[i4] * (1 - shadow))
        td[i4 + 1] = Math.round(td[i4 + 1] * (1 - shadow))
        td[i4 + 2] = Math.round(td[i4 + 2] * (1 - shadow))
        td[i4 + 3] = Math.min(255, td[i4 + 3] + Math.round(shadow * 70))
      }
    }
  }
}

function smoothStepRange(lo: number, hi: number, v: number): number {
  const t = Math.min(1, Math.max(0, (v - lo) / (hi - lo)))
  return t * t * (3 - 2 * t)
}

/**
 * Two-pass 3-4 chamfer distance transform (IconFilters.ChamferDistance).
 * inside: distance from opaque pixels to the nearest transparency (−1 on
 * transparent); outside: the reverse. Units are chamfer weights (÷3 ≈ px).
 */
export function chamferDistance(tile: Raster, size: number, inside: boolean): Float64Array {
  const INF = Number.MAX_VALUE / 4
  const d = tile.data
  const dist = new Float64Array(size * size)
  for (let i = 0; i < dist.length; i++) {
    const opaque = d[i * 4 + 3] >= 32
    dist[i] = opaque === inside ? INF : -1
  }

  const cost = (x: number, y: number, w: number) => {
    if (x < 0 || y < 0 || x >= size || y >= size) return inside ? w : INF
    const v = dist[y * size + x]
    return v < 0 ? w : v + w
  }

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x
      if (dist[i] < 0) continue
      let best = dist[i]
      best = Math.min(best, cost(x - 1, y, 3))
      best = Math.min(best, cost(x, y - 1, 3))
      best = Math.min(best, cost(x - 1, y - 1, 4))
      best = Math.min(best, cost(x + 1, y - 1, 4))
      dist[i] = best
    }
  }
  for (let y = size - 1; y >= 0; y--) {
    for (let x = size - 1; x >= 0; x--) {
      const i = y * size + x
      if (dist[i] < 0) continue
      let best = dist[i]
      best = Math.min(best, cost(x + 1, y, 3))
      best = Math.min(best, cost(x, y + 1, 3))
      best = Math.min(best, cost(x - 1, y + 1, 4))
      best = Math.min(best, cost(x + 1, y + 1, 4))
      dist[i] = best
    }
  }
  return dist
}

// ---- 像素 (kawaii pixel-art) ----

const PIXEL_CELLS = 24
const OUTLINE_R = 24
const OUTLINE_G = 24
const OUTLINE_B = 30

function pixelate(tile: Raster, size: number): void {
  const cell = size / PIXEL_CELLS
  const td = tile.data
  const colors = new Uint8ClampedArray(PIXEL_CELLS * PIXEL_CELLS * 3)
  const opaque = new Uint8Array(PIXEL_CELLS * PIXEL_CELLS)

  // 1) Cell grid: linear-light box average + candy palette.
  for (let cy = 0; cy < PIXEL_CELLS; cy++) {
    const y0 = Math.round(cy * cell)
    const y1 = Math.min(size, Math.round((cy + 1) * cell))
    for (let cx = 0; cx < PIXEL_CELLS; cx++) {
      const x0 = Math.round(cx * cell)
      const x1 = Math.min(size, Math.round((cx + 1) * cell))
      let r = 0
      let g = 0
      let b = 0
      let a = 0
      let n = 0
      for (let y = y0; y < y1; y++) {
        for (let x = x0; x < x1; x++) {
          const i4 = (y * size + x) * 4
          const w = td[i4 + 3] / 255
          r += srgbDecode(td[i4]) * w
          g += srgbDecode(td[i4 + 1]) * w
          b += srgbDecode(td[i4 + 2]) * w
          a += w
          n++
        }
      }
      const ci = cy * PIXEL_CELLS + cx
      if (n === 0 || a / n < 0.5) continue
      opaque[ci] = 1
      const [cr, cg, cb] = candy(srgbEncode(r / a), srgbEncode(g / a), srgbEncode(b / a))
      colors[ci * 3] = cr
      colors[ci * 3 + 1] = cg
      colors[ci * 3 + 2] = cb
    }
  }

  // 2) Contours: silhouette ring + darker side of strong internal edges.
  const outline = new Uint8Array(opaque.length)
  const neighbors: Array<[number, number]> = [[-1, 0], [1, 0], [0, -1], [0, 1]]
  for (let cy = 0; cy < PIXEL_CELLS; cy++) {
    for (let cx = 0; cx < PIXEL_CELLS; cx++) {
      const ci = cy * PIXEL_CELLS + cx
      if (!opaque[ci]) continue
      const lum = luminance(colors[ci * 3], colors[ci * 3 + 1], colors[ci * 3 + 2])
      for (const [dx, dy] of neighbors) {
        const nx = cx + dx
        const ny = cy + dy
        if (nx < 0 || ny < 0 || nx >= PIXEL_CELLS || ny >= PIXEL_CELLS || !opaque[ny * PIXEL_CELLS + nx]) {
          outline[ci] = 1
          break
        }
        const ni = ny * PIXEL_CELLS + nx
        const nl = luminance(colors[ni * 3], colors[ni * 3 + 1], colors[ni * 3 + 2])
        if (nl - lum > 0.3) {
          outline[ci] = 1
          break
        }
      }
    }
  }

  // 3) Paint: contour cells dark; the row under a top contour catches light.
  for (let cy = 0; cy < PIXEL_CELLS; cy++) {
    for (let cx = 0; cx < PIXEL_CELLS; cx++) {
      const ci = cy * PIXEL_CELLS + cx
      if (!opaque[ci]) continue
      if (outline[ci]) {
        // (byte) casts in C# truncate — mirror that, not clamped-array rounding.
        colors[ci * 3] = Math.trunc(OUTLINE_R * 0.75 + colors[ci * 3] * 0.25)
        colors[ci * 3 + 1] = Math.trunc(OUTLINE_G * 0.75 + colors[ci * 3 + 1] * 0.25)
        colors[ci * 3 + 2] = Math.trunc(OUTLINE_B * 0.75 + colors[ci * 3 + 2] * 0.25)
      } else if (cy > 0 && outline[(cy - 1) * PIXEL_CELLS + cx]) {
        colors[ci * 3] = Math.trunc(Math.min(255, colors[ci * 3] + (255 - colors[ci * 3]) * 0.22))
        colors[ci * 3 + 1] = Math.trunc(Math.min(255, colors[ci * 3 + 1] + (255 - colors[ci * 3 + 1]) * 0.22))
        colors[ci * 3 + 2] = Math.trunc(Math.min(255, colors[ci * 3 + 2] + (255 - colors[ci * 3 + 2]) * 0.22))
      }
    }
  }

  // 4) Expand cells back to pixels (nearest-neighbour blocks, hard alpha).
  for (let cy = 0; cy < PIXEL_CELLS; cy++) {
    const y0 = Math.round(cy * cell)
    const y1 = Math.min(size, Math.round((cy + 1) * cell))
    for (let cx = 0; cx < PIXEL_CELLS; cx++) {
      const x0 = Math.round(cx * cell)
      const x1 = Math.min(size, Math.round((cx + 1) * cell))
      const ci = cy * PIXEL_CELLS + cx
      const on = opaque[ci] === 1
      for (let y = y0; y < y1; y++) {
        for (let x = x0; x < x1; x++) {
          const i4 = (y * size + x) * 4
          if (on) {
            td[i4] = colors[ci * 3]
            td[i4 + 1] = colors[ci * 3 + 1]
            td[i4 + 2] = colors[ci * 3 + 2]
            td[i4 + 3] = 255
          } else {
            td[i4] = 0
            td[i4 + 1] = 0
            td[i4 + 2] = 0
            td[i4 + 3] = 0
          }
        }
      }
    }
  }
}

function candy(r: number, g: number, b: number): [number, number, number] {
  const m = 0.299 * r + 0.587 * g + 0.114 * b
  // C# casts (byte) BEFORE Posterize — truncation, not rounding. A value at a
  // posterization boundary (e.g. 31.9) must land on the SAME level as C#.
  const br = Math.trunc(Math.min(255, Math.max(0, m + (r - m) * 1.3)))
  const bg = Math.trunc(Math.min(255, Math.max(0, m + (g - m) * 1.3)))
  const bb = Math.trunc(Math.min(255, Math.max(0, m + (b - m) * 1.3)))
  return [posterize(br), posterize(bg), posterize(bb)]
}

function posterize(v: number): number {
  const levels = 5
  const q = Math.round((v / 255) * (levels - 1)) / (levels - 1)
  return clampByte(q * 255)
}

// ---- 贴纸 (die-cut sticker) ----

function sticker(tile: Raster, size: number): void {
  const border = Math.max(3, size * 0.05)
  const shadow = Math.max(2, size * 0.016)
  const inset = Math.ceil(border + shadow + 1)

  const target = size - 2 * inset
  const shrunk = downscale(cloneRaster(tile), target)
  tile.data.fill(0)
  for (let y = 0; y < target; y++) {
    for (let x = 0; x < target; x++) {
      const s4 = (y * target + x) * 4
      const d4 = ((y + inset) * size + x + inset) * 4
      tile.data[d4] = shrunk.data[s4]
      tile.data[d4 + 1] = shrunk.data[s4 + 1]
      tile.data[d4 + 2] = shrunk.data[s4 + 2]
      tile.data[d4 + 3] = shrunk.data[s4 + 3]
    }
  }

  const dist = chamferDistance(tile, size, false)
  const td = tile.data
  for (let i = 0; i < dist.length; i++) {
    if (dist[i] < 0) continue
    const d = dist[i] / 3
    const i4 = i * 4
    if (d <= border) {
      const coverage = Math.min(1, Math.max(0, border + 0.75 - d))
      td[i4] = 253
      td[i4 + 1] = 253
      td[i4 + 2] = 251
      td[i4 + 3] = Math.round(coverage * 255)
    } else if (d <= border + shadow) {
      const fadeT = 1 - (d - border) / shadow
      td[i4] = 20
      td[i4 + 1] = 22
      td[i4 + 2] = 26
      td[i4 + 3] = Math.round(46 * fadeT * fadeT)
    }
  }
}
