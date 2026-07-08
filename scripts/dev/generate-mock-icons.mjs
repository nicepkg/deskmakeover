#!/usr/bin/env node
// Dev-only mock icon pack generator (spec 06 §5). Procedurally draws ~120 messy
// desktop icons at 256px so the icons module has ship-safe, encumbrance-free source
// art to render, style and stress-test against. Everything here is OWN art: no
// extracted Windows icons, no brand marks. Output + manifest live in the web app's
// public/ folder and are committed; this script is re-runnable and deterministic
// (seeded RNG — pass a seed as argv[2] to fork a fresh pack).
//
// Encoder note: WebP is the spec's preferred container, but a dependency-free Node
// toolchain has no WebP encoder. We emit PNG via an inline node:zlib encoder (the
// same path scripts/dev/render-app-icon.mjs already ships) with adaptive per-row
// filtering so flat/palette art still deflates small. Size tradeoff is reported.

import { deflateSync } from 'node:zlib'
import { writeFileSync, mkdirSync, rmSync, existsSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const root = new URL('../..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')
const OUT_DIR = join(root, 'src', 'DeskMakeover.Web', 'public', 'mock-icons')
const SIZE = 256
const SEED = Number(process.argv[2]) || 0x9e3779b9

// Distribution (spec 06 §5), summing to 120.
const PLAN = [
  ['flat', 36],
  ['skeuo', 18],
  ['photo', 14],
  ['badged', 12],
  ['transparent', 14],
  ['letter', 10],
  ['folder', 10],
  ['document', 6],
]

// ---- seeded PRNG -----------------------------------------------------------
function mulberry32(seed) {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}
const pick = (rng, arr) => arr[Math.floor(rng() * arr.length)]
const range = (rng, lo, hi) => lo + rng() * (hi - lo)

// ---- colour ----------------------------------------------------------------
function hsl(h, s, l) {
  h = (((h % 360) + 360) % 360) / 360
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s
  const p = 2 * l - q
  const ch = (n) => {
    if (n < 0) n += 1
    if (n > 1) n -= 1
    if (n < 1 / 6) return p + (q - p) * 6 * n
    if (n < 1 / 2) return q
    if (n < 2 / 3) return p + (q - p) * (2 / 3 - n) * 6
    return p
  }
  return { r: Math.round(ch(h + 1 / 3) * 255), g: Math.round(ch(h) * 255), b: Math.round(ch(h - 1 / 3) * 255) }
}
const luma = (c) => 0.299 * c.r + 0.587 * c.g + 0.114 * c.b
const shade = (c, f) => ({ r: clampByte(c.r * f), g: clampByte(c.g * f), b: clampByte(c.b * f) })
const lerp = (a, b, t) => {
  const aa = a.a ?? 255
  const ba = b.a ?? 255
  return { r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t, a: aa + (ba - aa) * t }
}

// Palette + the ink/contrast + lightness-polarity axes (spec 06 §5).
function palette(rng) {
  const roll = rng()
  if (roll < 0.05) return { plate: { r: 0, g: 0, b: 0 }, ink: { r: 255, g: 255, b: 255 }, badge: { r: 235, g: 64, b: 52 } }
  if (roll < 0.1) return { plate: { r: 255, g: 255, b: 255 }, ink: { r: 24, g: 24, b: 28 }, badge: { r: 20, g: 120, b: 210 } }
  const h = rng() * 360
  const s = range(rng, 0.42, 0.95)
  const l = range(rng, 0.28, 0.72)
  const plate = hsl(h, s, l)
  const lightPlate = luma(plate) > 140
  let ink = lightPlate ? { r: 20, g: 20, b: 26 } : { r: 245, g: 246, b: 250 }
  // Glyph-plate contrast axis: sometimes collapse toward the plate to cross the
  // 0.66 ink / 0.58 mark thresholds the analysis classifiers key on.
  if (rng() < 0.22) ink = { r: clampByte(lerp(plate, ink, 0.4).r), g: clampByte(lerp(plate, ink, 0.4).g), b: clampByte(lerp(plate, ink, 0.4).b) }
  const badge = hsl((h + (rng() < 0.5 ? 150 : -45) + 360) % 360, 0.78, 0.52)
  return { plate, ink, badge, h }
}

// ---- geometry (SDF + point tests) ------------------------------------------
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v)
const clampByte = (v) => (v < 0 ? 0 : v > 255 ? 255 : Math.round(v))
function sdRoundRect(x, y, cx, cy, w, h, r) {
  const qx = Math.abs(x - cx) - w / 2 + r
  const qy = Math.abs(y - cy) - h / 2 + r
  return Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) + Math.min(Math.max(qx, qy), 0) - r
}
// Cover functions return 0..1 coverage (1px analytic AA from the SDF).
const cAA = (d) => clamp01(0.5 - d)
const coverRRect = (cx, cy, w, h, r) => (x, y) => cAA(sdRoundRect(x, y, cx, cy, w, h, r))
const coverRect = (cx, cy, w, h) => coverRRect(cx, cy, w, h, 0)
const coverCircle = (cx, cy, rad) => (x, y) => cAA(Math.hypot(x - cx, y - cy) - rad)
const coverRing = (cx, cy, rad, t) => (x, y) => {
  const d = Math.abs(Math.hypot(x - cx, y - cy) - rad) - t / 2
  return cAA(d)
}
const coverEllipse = (cx, cy, rx, ry) => (x, y) => {
  const dx = (x - cx) / rx
  const dy = (y - cy) / ry
  return Math.hypot(dx, dy) <= 1 ? 1 : 0
}
const coverSquircle = (cx, cy, half, n) => (x, y) => {
  const dx = Math.abs((x - cx) / half)
  const dy = Math.abs((y - cy) / half)
  return Math.pow(dx, n) + Math.pow(dy, n) <= 1 ? 1 : 0
}
function coverPoly(pts) {
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
const union = (...cs) => (x, y) => {
  let m = 0
  for (const c of cs) {
    const v = c(x, y)
    if (v > m) m = v
    if (m >= 1) break
  }
  return m
}
function plateCover(shape, cx, cy, size) {
  const half = size / 2
  if (shape === 'circle') return coverCircle(cx, cy, half)
  if (shape === 'squircle') return coverSquircle(cx, cy, half, 4.2)
  if (shape === 'superellipse') return coverSquircle(cx, cy, half, 2.6)
  if (shape === 'square') return coverRect(cx, cy, size, size)
  return coverRRect(cx, cy, size, size, size * 0.22)
}

// ---- colour fields ---------------------------------------------------------
const solid = (c) => () => ({ ...c, a: c.a ?? 255 })
const linear = (c0, c1, ax, ay) => {
  const len = Math.hypot(ax, ay) * 256 || 1
  return (x, y) => lerp(c0, c1, clamp01((x * ax + y * ay) / len))
}
const radial = (c0, c1, cx, cy, rad) => (x, y) => lerp(c0, c1, clamp01(Math.hypot(x - cx, y - cy) / rad))
const pattern = (c0, c1, cell) => (x, y) => (((((x + y) / cell) | 0) % 2) === 0 ? { ...c0, a: 255 } : { ...c1, a: 255 })
function hash2(x, y, seed) {
  let h = (Math.imul(x | 0, 374761393) + Math.imul(y | 0, 668265263) + Math.imul(seed | 0, 40503)) >>> 0
  h = Math.imul(h ^ (h >>> 13), 1274126177) >>> 0
  return ((h ^ (h >>> 16)) >>> 0) / 4294967295
}
// Procedural photo-ish fill: smooth gradient base + quantized low-amp grain. Quantizing
// keeps deflate from exploding on the noise (spec wants photo variety, not big files).
const grain = (baseFn, amp, seed) => (x, y) => {
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
const layer = (cover, color) => ({ cover, color })
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
function renderIcon(layers, opts) {
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

// ---- glyphs (own Fluent-ish marks) -----------------------------------------
function glyphCover(kind, cx, cy, s) {
  const h = s / 2
  switch (kind) {
    case 'disc':
      return coverCircle(cx, cy, h * 0.82)
    case 'ring':
      return coverRing(cx, cy, h * 0.7, s * 0.2)
    case 'bars':
      return union(
        coverRRect(cx - h * 0.35, cy + h * 0.2, s * 0.16, s * 0.7, s * 0.08),
        coverRRect(cx, cy, s * 0.16, s * 1.0, s * 0.08),
        coverRRect(cx + h * 0.35, cy - h * 0.15, s * 0.16, s * 0.5, s * 0.08),
      )
    case 'play':
      return coverPoly([
        [cx - h * 0.45, cy - h * 0.6],
        [cx + h * 0.65, cy],
        [cx - h * 0.45, cy + h * 0.6],
      ])
    case 'grid':
      return union(
        coverRRect(cx - h * 0.42, cy - h * 0.42, s * 0.42, s * 0.42, s * 0.1),
        coverRRect(cx + h * 0.42, cy - h * 0.42, s * 0.42, s * 0.42, s * 0.1),
        coverRRect(cx - h * 0.42, cy + h * 0.42, s * 0.42, s * 0.42, s * 0.1),
        coverRRect(cx + h * 0.42, cy + h * 0.42, s * 0.42, s * 0.42, s * 0.1),
      )
    case 'diamond':
      return coverPoly([
        [cx, cy - h],
        [cx + h, cy],
        [cx, cy + h],
        [cx - h, cy],
      ])
    case 'note':
      return union(
        coverRRect(cx - h * 0.05, cy - h * 0.55, s * 0.14, s * 1.0, s * 0.06),
        coverCircle(cx - h * 0.35, cy + h * 0.45, s * 0.2),
        coverRRect(cx + h * 0.25, cy - h * 0.62, s * 0.55, s * 0.14, s * 0.06),
      )
    case 'spark':
      return coverPoly([
        [cx, cy - h],
        [cx + h * 0.28, cy - h * 0.28],
        [cx + h, cy],
        [cx + h * 0.28, cy + h * 0.28],
        [cx, cy + h],
        [cx - h * 0.28, cy + h * 0.28],
        [cx - h, cy],
        [cx - h * 0.28, cy - h * 0.28],
      ])
    default:
      return coverCircle(cx, cy, h * 0.7)
  }
}
const GLYPHS = ['disc', 'ring', 'bars', 'play', 'grid', 'diamond', 'note', 'spark']

// Bold geometric monogram (own art) — the letter-tile axis. Latin from rect unions,
// CJK as an abstract stroke lattice (reads as 汉字 without needing a bundled font).
function letterCover(ch, cx, cy, s) {
  const h = s / 2
  const t = s * 0.17
  const V = (ox) => coverRect(cx + ox, cy, t, s)
  const H = (oy) => coverRect(cx, cy + oy, s, t)
  const seg = { I: () => V(0), T: () => union(H(-h + t / 2), V(0)), L: () => union(V(-h + t / 2), H(h - t / 2)) }
  seg.H = () => union(V(-h + t / 2), V(h - t / 2), H(0))
  seg.E = () => union(V(-h + t / 2), H(-h + t / 2), H(0), H(h - t / 2))
  seg.F = () => union(V(-h + t / 2), H(-h + t / 2), H(0))
  seg.U = () => union(coverRect(cx - h + t / 2, cy - t, t, s - t * 2), coverRect(cx + h - t / 2, cy - t, t, s - t * 2), H(h - t / 2))
  seg.O = () => coverRing(cx, cy, h * 0.72, t)
  return (seg[ch] ?? seg.O)()
}
function hanziCover(rng, cx, cy, s) {
  const h = s / 2
  const t = s * 0.12
  const parts = [coverRect(cx, cy - h + t, s, t), coverRect(cx, cy + h - t, s, t)]
  const rows = 1 + Math.floor(rng() * 2)
  for (let i = 0; i < rows; i++) parts.push(coverRect(cx, cy + range(rng, -h * 0.4, h * 0.4), s * 0.9, t))
  const cols = 1 + Math.floor(rng() * 2)
  for (let i = 0; i < cols; i++) parts.push(coverRect(cx + range(rng, -h * 0.4, h * 0.4), cy, t, s * 0.92))
  return union(...parts)
}

// ---- category builders -----------------------------------------------------
function bgField(rng, pal) {
  switch (pick(rng, ['solid', 'solid', 'linear', 'radial', 'noise', 'pattern'])) {
    case 'linear':
      return linear(shade(pal.plate, 1.12), shade(pal.plate, 0.78), rng() < 0.5 ? 1 : 0.4, 0.9)
    case 'radial':
      return radial(shade(pal.plate, 1.18), shade(pal.plate, 0.7), range(rng, 96, 160), range(rng, 96, 160), 200)
    case 'noise':
      return grain(solid(pal.plate), 0.12, (SEED ^ (rng() * 1e6)) | 0)
    case 'pattern':
      return pattern(pal.plate, shade(pal.plate, 0.86), 16 + Math.floor(rng() * 18))
    default:
      return solid(pal.plate)
  }
}
function baseAxes(rng, over1 = {}) {
  return {
    srcRes: rng() < 0.14 ? 32 : rng() < 0.12 ? 48 : SIZE,
    edge: pick(rng, ['aa', 'aa', 'aa', 'hard', 'glow', 'semi']),
    safe: range(rng, 0.6, 1.0),
    ss: 2,
    ...over1,
  }
}

function buildFlat(rng) {
  const pal = palette(rng)
  const shape = pick(rng, ['squircle', 'rrect', 'rrect', 'circle', 'superellipse', 'square'])
  const ax = baseAxes(rng)
  const plate = SIZE * range(rng, 0.82, 0.94)
  const layers = [layer(plateCover(shape, 128, 128, plate), bgField(rng, pal))]
  const g = pick(rng, GLYPHS)
  layers.push(layer(glyphCover(g, 128, 128, plate * 0.5 * ax.safe), solid(pal.ink)))
  return { layers, opts: { ...ax, glow: pal.plate } }
}

function buildSkeuo(rng) {
  const pal = palette(rng)
  const ax = baseAxes(rng, { edge: pick(rng, ['aa', 'aa', 'semi']) })
  // Pre-baked rounded corners on a transparent square canvas = the double-rounding trap.
  const plate = SIZE * range(rng, 0.86, 0.96)
  const r = plate * range(rng, 0.16, 0.26)
  const layers = [
    layer(coverRRect(128, 128, plate, plate, r), linear(shade(pal.plate, 1.22), shade(pal.plate, 0.72), 0.5, 0.9)),
    // glossy top sheen
    layer(coverEllipse(128, 128 - plate * 0.24, plate * 0.44, plate * 0.26), solid({ r: 255, g: 255, b: 255, a: 64 })),
    // bevel base
    layer(coverRRect(128, 128 + plate * 0.36, plate * 0.9, plate * 0.22, r * 0.6), solid({ r: 0, g: 0, b: 0, a: 40 })),
  ]
  layers.push(layer(glyphCover(pick(rng, GLYPHS), 128, 128, plate * 0.46 * ax.safe), solid(pal.ink)))
  if (rng() < 0.4) addBadge(layers, rng, pick(rng, ['tl', 'tr', 'bl', 'br']), pal)
  return { layers, opts: ax }
}

function buildPhoto(rng) {
  // Two distinct mid-light hues keep the photo-fill category visibly gradient-y; the
  // pure-black / pure-white degenerates arrive through the palette axis in the other
  // categories, so photos don't need to (and shouldn't) collapse to a flat black plate.
  const h1 = rng() * 360
  const c0 = hsl(h1, range(rng, 0.55, 0.92), range(rng, 0.4, 0.62))
  const c1 = hsl((h1 + range(rng, 40, 190)) % 360, range(rng, 0.55, 0.92), range(rng, 0.3, 0.55))
  const ax = baseAxes(rng, { ss: 2, edge: pick(rng, ['aa', 'hard']) })
  const pre = rng() < 0.5
  const plate = SIZE * (pre ? 0.94 : 1.0)
  const cover = pre ? coverRRect(128, 128, plate, plate, plate * 0.2) : coverRect(128, 128, SIZE, SIZE)
  const gradient = rng() < 0.5 ? linear(c0, c1, range(rng, 0.3, 1), range(rng, 0.3, 1)) : radial(c0, c1, range(rng, 70, 186), range(rng, 60, 160), range(rng, 180, 260))
  const layers = [layer(cover, grain(gradient, range(rng, 0.05, 0.14), (SEED + rng() * 1e6) | 0))]
  // soft vignette
  layers.push(layer(cover, radial({ r: 0, g: 0, b: 0, a: 0 }, { r: 0, g: 0, b: 0, a: 60 }, 128, 128, 210)))
  if (rng() < 0.5) layers.push(layer(glyphCover(pick(rng, GLYPHS), 128, 128, plate * 0.4), solid({ r: 255, g: 255, b: 255, a: 210 })))
  return { layers, opts: ax }
}

function buildBadged(rng, corner) {
  const base = buildFlat(rng)
  const pal = palette(rng)
  addBadge(base.layers, rng, corner, pal)
  return base
}
function addBadge(layers, rng, corner, pal) {
  const off = 128 * range(rng, 0.52, 0.62)
  const cx = corner.includes('l') ? 128 - off : 128 + off
  const cy = corner.includes('t') ? 128 - off : 128 + off
  const rad = SIZE * range(rng, 0.11, 0.15)
  layers.push(layer(coverCircle(cx, cy, rad + 3), solid({ r: 255, g: 255, b: 255, a: 235 })))
  layers.push(layer(coverCircle(cx, cy, rad), solid(pal.badge)))
  const mark = pick(rng, ['dot', 'plus', 'check'])
  const t = rad * 0.28
  if (mark === 'dot') layers.push(layer(coverCircle(cx, cy, rad * 0.4), solid({ r: 255, g: 255, b: 255 })))
  else if (mark === 'plus') layers.push(layer(union(coverRect(cx, cy, rad, t), coverRect(cx, cy, t, rad)), solid({ r: 255, g: 255, b: 255 })))
  else
    layers.push(
      layer(
        union(coverPoly([[cx - rad * 0.5, cy], [cx - rad * 0.15, cy + rad * 0.4], [cx - rad * 0.28, cy + rad * 0.5], [cx - rad * 0.5, cy + rad * 0.2]]), coverPoly([[cx - rad * 0.2, cy + rad * 0.4], [cx + rad * 0.5, cy - rad * 0.4], [cx + rad * 0.62, cy - rad * 0.24], [cx - rad * 0.12, cy + rad * 0.52]])),
        solid({ r: 255, g: 255, b: 255 }),
      ),
    )
}

function buildTransparent(rng) {
  const pal = palette(rng)
  const ax = baseAxes(rng, { edge: pick(rng, ['glow', 'glow', 'hard', 'semi', 'aa']), srcRes: rng() < 0.12 ? 32 : SIZE })
  const kind = pick(rng, ['hex', 'tri', 'diamond', 'ring', 'blob'])
  const s = SIZE * range(rng, 0.62, 0.84)
  const h = s / 2
  const fill = rng() < 0.5 ? solid(pal.plate) : linear(shade(pal.plate, 1.15), shade(pal.plate, 0.75), 0.6, 0.8)
  let cover
  if (kind === 'hex') {
    const pts = []
    for (let i = 0; i < 6; i++) pts.push([128 + h * Math.cos((i / 6) * 2 * Math.PI + 0.5), 128 + h * Math.sin((i / 6) * 2 * Math.PI + 0.5)])
    cover = coverPoly(pts)
  } else if (kind === 'tri') {
    cover = coverPoly([[128, 128 - h], [128 + h, 128 + h * 0.8], [128 - h, 128 + h * 0.8]])
  } else if (kind === 'diamond') {
    cover = coverPoly([[128, 128 - h], [128 + h, 128], [128, 128 + h], [128 - h, 128]])
  } else if (kind === 'ring') {
    cover = coverRing(128, 128, h * 0.7, s * 0.26)
  } else {
    const pts = []
    const lobes = 5 + Math.floor(rng() * 3)
    for (let i = 0; i < lobes * 2; i++) {
      const rr = i % 2 === 0 ? h : h * range(rng, 0.55, 0.8)
      pts.push([128 + rr * Math.cos((i / (lobes * 2)) * 2 * Math.PI), 128 + rr * Math.sin((i / (lobes * 2)) * 2 * Math.PI)])
    }
    cover = coverPoly(pts)
  }
  const layers = [layer(cover, fill)]
  if (kind !== 'ring' && rng() < 0.5) layers.push(layer(glyphCover(pick(rng, GLYPHS), 128, 128, s * 0.42), solid(pal.ink)))
  return { layers, opts: { ...ax, glow: pal.plate } }
}

function buildLetter(rng, latin) {
  const pal = palette(rng)
  const shape = pick(rng, ['squircle', 'rrect', 'circle'])
  const ax = baseAxes(rng, { ss: 3 })
  const plate = SIZE * range(rng, 0.84, 0.94)
  const layers = [layer(plateCover(shape, 128, 128, plate), bgField(rng, pal))]
  if (latin) layers.push(layer(letterCover(pick(rng, ['A', 'H', 'E', 'F', 'T', 'L', 'U', 'I', 'O']), 128, 128, plate * 0.5 * ax.safe), solid(pal.ink)))
  else layers.push(layer(hanziCover(rng, 128, 128, plate * 0.52 * ax.safe), solid(pal.ink)))
  return { layers, opts: ax }
}

function buildFolder(rng) {
  const family = pick(rng, [42, 45, 205, 150, 20, 0])
  const isGray = family === 0
  const body = isGray ? { r: 150, g: 156, b: 164 } : hsl(family, 0.62, 0.55)
  const front = isGray ? { r: 176, g: 182, b: 190 } : hsl(family, 0.66, 0.66)
  const ax = baseAxes(rng, { edge: pick(rng, ['aa', 'aa', 'semi']), srcRes: rng() < 0.12 ? 32 : SIZE })
  const w = SIZE * 0.82
  const layers = [
    // back panel + tab
    layer(coverRRect(128, 150, w, SIZE * 0.5, 14), solid(shade(body, 0.9))),
    layer(coverRRect(128 - w * 0.24, 96, w * 0.42, 26, 10), solid(shade(body, 0.9))),
    // front flap
    layer(coverRRect(128, 158, w, SIZE * 0.44, 16), solid(front)),
  ]
  if (rng() < 0.45) addBadge(layers, rng, 'br', palette(rng))
  return { layers, opts: ax }
}

function buildDocument(rng) {
  const pal = palette(rng)
  const ax = baseAxes(rng, { edge: 'aa', srcRes: SIZE })
  const w = SIZE * 0.62
  const hh = SIZE * 0.8
  const left = 128 - w / 2
  const paper = { r: 250, g: 250, b: 252 }
  const fold = SIZE * 0.16
  const layers = [
    layer(coverPoly([[left, 44], [left + w - fold, 44], [left + w, 44 + fold], [left + w, 44 + hh], [left, 44 + hh]]), solid(paper)),
    // dog-ear
    layer(coverPoly([[left + w - fold, 44], [left + w, 44 + fold], [left + w - fold, 44 + fold]]), solid(shade(paper, 0.82))),
    // header band (type colour)
    layer(coverRect(128, 44 + hh - 26, w, 34), solid(pal.badge)),
  ]
  // text lines
  for (let i = 0; i < 3; i++) layers.push(layer(coverRRect(128, 90 + i * 26, w * 0.7, 9, 4), solid({ r: 200, g: 202, b: 208 })))
  return { layers, opts: ax }
}

// ---- labels + kinds --------------------------------------------------------
const FAKE_EN = ['PhotonChat', 'DevForge', 'MeteorPlay', 'NovaEdit', 'PixelForge', 'AuroraDB', 'VoltMail', 'QuellNote', 'DriftSync', 'EmberCast', 'HollowIDE', 'LumenCAD', 'GristCalc', 'ZephyrVPN', 'TidalDraw', 'FluxBoard', 'CinderTerm', 'OrbitPay']
const FAKE_ZH = ['星穹笔记', '云图相册', '墨刻文档', '潮汐音乐', '微光邮箱', '折光剪辑', '蜂巢云盘', '拾光日记', '磐石数据库', '流沙同步', '暗河终端', '朝霞画板']
const DESK_ZH = ['工作文档', '季度报表', '原型稿_v3', '会议纪要', '项目计划', '本地相册', '读书清单', '报销单', '某某启动器', '待办清单', '装机必备', '临时文件', '家庭账本']
const FOLDER_LB = ['素材库', '下载', '项目归档', '2024备份', '截图', 'Documents', 'Projects', '工作', '家庭照片', '设计稿', 'node_modules', '临时']
const DOC_LB = ['合同_最终版', '预算表_2024', '周报', '需求文档', '说明书', 'README', '简历_2024']
const URL_LB = ['内网门户', '打卡系统', '工单平台', 'DevPortal', 'StatusPage', '知识库', '监控大盘']

function labelFor(rng, cat, kind) {
  if (kind === 'folder') return pick(rng, FOLDER_LB)
  if (kind === 'file') return pick(rng, DOC_LB) + pick(rng, ['.pdf', '.docx', '.xlsx', '.txt', ''])
  if (kind === 'url') return pick(rng, URL_LB)
  if (kind === 'bin') return pick(rng, ['回收站', '废纸篓'])
  const base = pick(rng, [...FAKE_EN, ...FAKE_ZH, ...DESK_ZH])
  return rng() < 0.22 ? base + pick(rng, [' (1)', '_副本', ' - 快捷方式', '_v2']) : base
}

// ---- PNG encoder (adaptive filtering, node:zlib) ---------------------------
function encodePng(width, height, rgba) {
  const stride = width * 4
  const raw = Buffer.alloc((stride + 1) * height)
  const prev = Buffer.alloc(stride)
  const cand = Buffer.alloc(stride)
  for (let y = 0; y < height; y++) {
    const row = rgba.subarray(y * stride, (y + 1) * stride)
    let bestType = 0, bestSum = Infinity
    const bestBuf = Buffer.alloc(stride)
    for (let f = 0; f < 5; f++) {
      filterRow(row, prev, f, cand)
      let sum = 0
      for (let i = 0; i < stride; i++) sum += cand[i] < 128 ? cand[i] : 256 - cand[i]
      if (sum < bestSum) {
        bestSum = sum
        bestType = f
        cand.copy(bestBuf)
      }
    }
    raw[y * (stride + 1)] = bestType
    bestBuf.copy(raw, y * (stride + 1) + 1)
    row.copy(prev)
  }
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(width, 0)
  ihdr.writeUInt32BE(height, 4)
  ihdr[8] = 8
  ihdr[9] = 6
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ])
}
function filterRow(row, prev, type, out) {
  const bpp = 4
  for (let i = 0; i < row.length; i++) {
    const a = i >= bpp ? row[i - bpp] : 0
    const b = prev[i]
    const c = i >= bpp ? prev[i - bpp] : 0
    const x = row[i]
    let v = x
    if (type === 1) v = x - a
    else if (type === 2) v = x - b
    else if (type === 3) v = x - ((a + b) >> 1)
    else if (type === 4) v = x - paeth(a, b, c)
    out[i] = v & 0xff
  }
}
function paeth(a, b, c) {
  const p = a + b - c
  const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c)
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c
}
function chunk(type, data) {
  const name = Buffer.from(type)
  const body = Buffer.concat([name, data])
  const len = Buffer.alloc(4)
  len.writeUInt32BE(data.length, 0)
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(body), 0)
  return Buffer.concat([len, body, crc])
}
function crc32(buf) {
  let c = 0xffffffff
  for (const byte of buf) {
    c ^= byte
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
  }
  return (c ^ 0xffffffff) >>> 0
}

// ---- main ------------------------------------------------------------------
function build(cat, rng, i) {
  if (cat === 'flat') return buildFlat(rng)
  if (cat === 'skeuo') return buildSkeuo(rng)
  if (cat === 'photo') return buildPhoto(rng)
  if (cat === 'badged') return buildBadged(rng, ['tl', 'tr', 'bl', 'br'][i % 4])
  if (cat === 'transparent') return buildTransparent(rng)
  if (cat === 'letter') return buildLetter(rng, i % 2 === 0)
  if (cat === 'folder') return buildFolder(rng)
  return buildDocument(rng)
}
// kind per category; the mock bridge (spec 06 §5) maps these onto item taxonomy.
function kindFor(rng, cat) {
  if (cat === 'folder') return 'folder'
  if (cat === 'document') return 'file'
  if (cat === 'transparent') return pick(rng, ['url', 'lnk', 'lnk'])
  return pick(rng, ['lnk', 'lnk', 'lnk', 'exe', 'exe', 'url', 'uwp'])
}

function main() {
  const rng = mulberry32(SEED)
  if (existsSync(OUT_DIR)) for (const f of readdirSync(OUT_DIR)) if (/\.(png|webp|json)$/.test(f)) rmSync(join(OUT_DIR, f))
  mkdirSync(OUT_DIR, { recursive: true })

  const specs = []
  for (const [cat, count] of PLAN) for (let i = 0; i < count; i++) specs.push({ cat, i })
  // Guarantee every taxonomy kind appears at least once (manifest integrity).
  const forced = ['lnk', 'exe', 'url', 'uwp', 'bin']
  const nonReserved = specs.map((s, idx) => idx).filter((idx) => specs[idx].cat !== 'folder' && specs[idx].cat !== 'document')

  const manifest = []
  let total = 0, min = Infinity, max = 0
  const kindTally = {}
  specs.forEach((s, idx) => {
    const { layers, opts } = build(s.cat, rng, s.i)
    let kind
    const fpos = nonReserved.indexOf(idx)
    if (fpos > -1 && fpos < forced.length) kind = forced[fpos]
    else kind = kindFor(rng, s.cat)
    const png = renderIcon(layers, opts)
    const file = `icon-${String(idx).padStart(3, '0')}.png`
    writeFileSync(join(OUT_DIR, file), png)
    total += png.length
    min = Math.min(min, png.length)
    max = Math.max(max, png.length)
    kindTally[kind] = (kindTally[kind] ?? 0) + 1
    manifest.push({ file, id: `mock-${String(idx).padStart(3, '0')}`, kind, label: labelFor(rng, s.cat, kind) })
  })
  writeFileSync(join(OUT_DIR, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n')

  const kb = (n) => (n / 1024).toFixed(1)
  console.log(`mock-icons: ${manifest.length} icons + manifest.json → ${OUT_DIR}`)
  console.log(`pack size: ${kb(total)} KB total · min ${kb(min)} KB · max ${kb(max)} KB · avg ${kb(total / manifest.length)} KB`)
  console.log(`distribution: ${PLAN.map(([c, n]) => `${c}=${n}`).join(' · ')}`)
  console.log(`kinds: ${Object.entries(kindTally).map(([k, n]) => `${k}=${n}`).join(' · ')}`)
  console.log(`seed: ${SEED}`)
}

main()
