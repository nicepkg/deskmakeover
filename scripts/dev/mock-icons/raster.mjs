// Rasterizer: SDF cover functions, colour fields, compositing, and the supersampled
// renderer that turns a layer stack into a 256px PNG (spec 06 §5 axes live upstream).

import { SIZE, clamp01, clampByte } from './constants.mjs'
import { lerp } from './color.mjs'
import { encodePng } from './png.mjs'

// ---- geometry (SDF + point tests) ------------------------------------------
function sdRoundRect(x, y, cx, cy, w, h, r) {
  const qx = Math.abs(x - cx) - w / 2 + r
  const qy = Math.abs(y - cy) - h / 2 + r
  return Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) + Math.min(Math.max(qx, qy), 0) - r
}
// Cover functions return 0..1 coverage (1px analytic AA from the SDF).
const cAA = (d) => clamp01(0.5 - d)
export const coverRRect = (cx, cy, w, h, r) => (x, y) => cAA(sdRoundRect(x, y, cx, cy, w, h, r))
export const coverRect = (cx, cy, w, h) => coverRRect(cx, cy, w, h, 0)
export const coverCircle = (cx, cy, rad) => (x, y) => cAA(Math.hypot(x - cx, y - cy) - rad)
export const coverRing = (cx, cy, rad, t) => (x, y) => {
  const d = Math.abs(Math.hypot(x - cx, y - cy) - rad) - t / 2
  return cAA(d)
}
export const coverEllipse = (cx, cy, rx, ry) => (x, y) => {
  const dx = (x - cx) / rx
  const dy = (y - cy) / ry
  return Math.hypot(dx, dy) <= 1 ? 1 : 0
}
export const coverSquircle = (cx, cy, half, n) => (x, y) => {
  const dx = Math.abs((x - cx) / half)
  const dy = Math.abs((y - cy) / half)
  return Math.pow(dx, n) + Math.pow(dy, n) <= 1 ? 1 : 0
}
export function coverPoly(pts) {
  return (x, y) => {
    let inside = false
    for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
      const [xi, yi] = pts[i]
      const [xj, yj] = pts[j]
      if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside
    }
    return inside ? 1 : 0
  }
}
export const union = (...cs) => (x, y) => {
  let m = 0
  for (const c of cs) {
    const v = c(x, y)
    if (v > m) m = v
    if (m >= 1) break
  }
  return m
}
export function plateCover(shape, cx, cy, size) {
  const half = size / 2
  if (shape === 'circle') return coverCircle(cx, cy, half)
  if (shape === 'squircle') return coverSquircle(cx, cy, half, 4.2)
  if (shape === 'superellipse') return coverSquircle(cx, cy, half, 2.6)
  if (shape === 'square') return coverRect(cx, cy, size, size)
  return coverRRect(cx, cy, size, size, size * 0.22)
}

// ---- colour fields ---------------------------------------------------------
export const solid = (c) => () => ({ ...c, a: c.a ?? 255 })
export const linear = (c0, c1, ax, ay) => {
  const len = Math.hypot(ax, ay) * 256 || 1
  return (x, y) => lerp(c0, c1, clamp01((x * ax + y * ay) / len))
}
export const radial = (c0, c1, cx, cy, rad) => (x, y) => lerp(c0, c1, clamp01(Math.hypot(x - cx, y - cy) / rad))
export const pattern = (c0, c1, cell) => (x, y) => (((((x + y) / cell) | 0) % 2) === 0 ? { ...c0, a: 255 } : { ...c1, a: 255 })
function hash2(x, y, seed) {
  let h = (Math.imul(x | 0, 374761393) + Math.imul(y | 0, 668265263) + Math.imul(seed | 0, 40503)) >>> 0
  h = Math.imul(h ^ (h >>> 13), 1274126177) >>> 0
  return ((h ^ (h >>> 16)) >>> 0) / 4294967295
}
// Procedural photo-ish fill: smooth gradient base + quantized low-amp grain. Quantizing
// keeps deflate from exploding on the noise (spec wants photo variety, not big files).
export const grain = (baseFn, amp, seed) => (x, y) => {
  const base = baseFn(x, y)
  const q = Math.round((hash2(Math.floor(x / 2), Math.floor(y / 2), seed) - 0.5) * amp * 8) / 8
  return { r: clampByte(base.r + q * 255), g: clampByte(base.g + q * 255), b: clampByte(base.b + q * 255), a: 255 }
}

// ---- compositing + rasterizer ---------------------------------------------
function over(src, dst) {
  const sa = src.a / 255
  const da = dst.a / 255
  const oa = sa + da * (1 - sa)
  if (oa <= 0) return { r: 0, g: 0, b: 0, a: 0 }
  return {
    r: (src.r * sa + dst.r * da * (1 - sa)) / oa,
    g: (src.g * sa + dst.g * da * (1 - sa)) / oa,
    b: (src.b * sa + dst.b * da * (1 - sa)) / oa,
    a: oa * 255,
  }
}
// A layer = { cover(x,y)->0..1, color(x,y)->{r,g,b,a?} }.
export const layer = (cover, color) => ({ cover, color })
function paint(layers, x, y) {
  let c = { r: 0, g: 0, b: 0, a: 0 }
  for (const l of layers) {
    const cov = l.cover(x, y)
    if (cov <= 0) continue
    const col = l.color(x, y)
    c = over({ r: col.r, g: col.g, b: col.b, a: (col.a ?? 255) * cov }, c)
  }
  return c
}
export function renderIcon(layers, opts) {
  const base = opts.srcRes ?? SIZE
  const eff = opts.edge === 'hard' ? 1 : opts.ss ?? 2
  const rgba = Buffer.alloc(base * base * 4)
  for (let y = 0; y < base; y++) {
    for (let x = 0; x < base; x++) {
      let r = 0, g = 0, b = 0, a = 0
      for (let sy = 0; sy < eff; sy++) {
        for (let sx = 0; sx < eff; sx++) {
          const lx = ((x + (sx + 0.5) / eff) / base) * SIZE
          const ly = ((y + (sy + 0.5) / eff) / base) * SIZE
          const c = paint(layers, lx, ly)
          const al = c.a / 255
          r += c.r * al
          g += c.g * al
          b += c.b * al
          a += al
        }
      }
      const o = (y * base + x) * 4
      const av = a / (eff * eff)
      rgba[o + 3] = Math.round(av * 255)
      if (a > 0) {
        rgba[o] = clampByte(r / a)
        rgba[o + 1] = clampByte(g / a)
        rgba[o + 2] = clampByte(b / a)
      }
    }
  }
  let out = base === SIZE ? rgba : nearestUpscale(rgba, base, SIZE)
  if (opts.edge === 'hard') hardenAlpha(out)
  if (opts.edge === 'semi') out = blurAlpha(out, SIZE, 1)
  if (opts.edge === 'glow') out = addGlow(out, SIZE, opts.glow ?? { r: 255, g: 255, b: 255 })
  return encodePng(SIZE, SIZE, out)
}
function nearestUpscale(src, from, to) {
  const dst = Buffer.alloc(to * to * 4)
  for (let y = 0; y < to; y++) {
    const sy = Math.floor((y / to) * from)
    for (let x = 0; x < to; x++) {
      const sx = Math.floor((x / to) * from)
      src.copy(dst, (y * to + x) * 4, (sy * from + sx) * 4, (sy * from + sx) * 4 + 4)
    }
  }
  return dst
}
function hardenAlpha(buf) {
  for (let i = 3; i < buf.length; i += 4) buf[i] = buf[i] >= 128 ? 255 : 0
}
function blurAlpha(buf, size, r) {
  const src = Buffer.from(buf)
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let sum = 0, n = 0
      for (let dy = -r; dy <= r; dy++) {
        for (let dx = -r; dx <= r; dx++) {
          const nx = x + dx, ny = y + dy
          if (nx < 0 || ny < 0 || nx >= size || ny >= size) continue
          sum += src[(ny * size + nx) * 4 + 3]
          n++
        }
      }
      buf[(y * size + x) * 4 + 3] = Math.round(sum / n)
    }
  }
  return buf
}
function addGlow(buf, size, color) {
  const alpha = new Float32Array(size * size)
  for (let i = 0; i < size * size; i++) alpha[i] = buf[i * 4 + 3]
  // two box-blur passes ≈ a soft halo
  const blurred = boxBlur(boxBlur(alpha, size, 6), size, 6)
  const out = Buffer.alloc(size * size * 4)
  for (let i = 0; i < size * size; i++) {
    const halo = Math.min(255, blurred[i] * 0.6)
    const glow = { r: color.r, g: color.g, b: color.b, a: halo }
    const top = { r: buf[i * 4], g: buf[i * 4 + 1], b: buf[i * 4 + 2], a: buf[i * 4 + 3] }
    const c = over(top, glow)
    out[i * 4] = clampByte(c.r)
    out[i * 4 + 1] = clampByte(c.g)
    out[i * 4 + 2] = clampByte(c.b)
    out[i * 4 + 3] = Math.round(c.a)
  }
  return out
}
function boxBlur(src, size, r) {
  const dst = new Float32Array(size * size)
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let sum = 0, n = 0
      for (let d = -r; d <= r; d++) {
        const nx = x + d
        if (nx < 0 || nx >= size) continue
        sum += src[y * size + nx]
        n++
      }
      dst[y * size + x] = sum / n
    }
  }
  const out = new Float32Array(size * size)
  for (let x = 0; x < size; x++) {
    for (let y = 0; y < size; y++) {
      let sum = 0, n = 0
      for (let d = -r; d <= r; d++) {
        const ny = y + d
        if (ny < 0 || ny >= size) continue
        sum += dst[ny * size + x]
        n++
      }
      out[y * size + x] = sum / n
    }
  }
  return out
}
