#!/usr/bin/env bun
// Spike 4 (ADR-0019 M1 gate) — TS side of the tri-target pixel slice.
//
// Renders the SPIKE SLICE — "Circle shape + fixed white plate + subject blit +
// silhouette dock shadow" — over the whole M0b mock pack with the FROZEN TS
// compositor primitives, and dumps raw RGBA for the Rust-native / Rust-WASM
// comparison harness (tests/icon-parity/spike4).
//
// The slice is DEFINED HERE (no tier-B cell is classification-free): every
// pixel-touching helper below is a VERBATIM copy of a private function in the
// frozen compose.ts (line refs inline) — copied, not re-derived, because the
// frozen file must not gain exports (ADR-0019: no fixes except oracle
// corrections). Everything exported by the frozen modules is imported directly.
//
// Output (repo-root target/spike4/, gitignored):
//   sources/<id>.rgba      raw 256×256 straight-alpha RGBA (canonical input)
//   ts/<id>-<size>.rgba    slice render at each size
//   cells.tsv              id \t size \t sha256(RGBA) \t sampler-lane
//   fixtures.tsv           cross-language probes (decode LUT bits, shadow tone,
//                          srgbEncode probes, shapeMask probes) for xtask compare

import { mkdirSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import type { IconShape } from '@/bridge/types'
import type { ContentBounds } from '@/icon-compositor/analysis'
import { boundsH, boundsW, findContentBounds } from '@/icon-compositor/analysis'
import { fieldShadowTone, srgbDecode, srgbEncode } from '@/icon-compositor/color'
import type { Raster } from '@/icon-compositor/raster'
import { makeRaster, overAt, shapeMask, WHITE } from '@/icon-compositor/raster'
import { drawScaled } from '@/icon-compositor/sampling'
import { shapeContains } from '@/icon-compositor/shapes'
import { loadMockSources } from './oracle/desktop-session'
import { rasterHash } from './oracle/png-codec'

const FRONTEND = resolve(import.meta.dir, '..')
const REPO_ROOT = resolve(FRONTEND, '../../..')
const OUT = join(REPO_ROOT, 'target/spike4')

// ---- slice spec (fixed for the spike; the Rust side hardcodes the same) -----
const SHAPE: IconShape = 'Circle'
/** 256 = downscale lane (area-averaged), 512 = upscale lane (supersampled) for
 *  full-canvas sources — both drawScaled paths execute across the corpus. */
const SIZES = [256, 512] as const

// ---- verbatim copies of frozen compose.ts internals (not exported there) ----

// compose.ts:33
const FIELD_CONTENT_PADDING_FRACTION = 36 / 256
// compose.ts:90-95 (Circle entry only — the slice is Circle-fixed)
const INSCRIBE_MARGIN_CIRCLE = 0.94
// compose.ts:478-483 (dock mode only)
const SHADOW_DOCK = { alpha: 0.24, blurFraction: 0.04, offsetFraction: 0.015 } as const

// compose.ts:105-125
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

// compose.ts:381-385 (Circle is an INSCRIBE_SHAPES member)
function fieldContentBox(shape: IconShape, cardSize: number): number {
  const inner = Math.max(8, cardSize - 2 * Math.round(cardSize * FIELD_CONTENT_PADDING_FRACTION))
  return Math.min(inner, Math.max(8, Math.round(cardSize * maxCentredSquareFactor(shape) * INSCRIBE_MARGIN_CIRCLE)))
}

// compose.ts:713-724
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

// compose.ts:726-729
function fit(w: number, h: number, max: number): [number, number] {
  const scale = Math.min(max / w, max / h)
  return [Math.max(1, Math.round(w * scale)), Math.max(1, Math.round(h * scale))]
}

// compose.ts:702-707
function drawCentred(
  artwork: Raster, bounds: ContentBounds, content: Raster, size: number, pad: number, cardSize: number, box: number,
): void {
  const [w, h] = fit(Math.max(1, boundsW(bounds)), Math.max(1, boundsH(bounds)), box)
  drawScaled(artwork, bounds, content, size, pad + Math.trunc((cardSize - w) / 2), pad + Math.trunc((cardSize - h) / 2), w, h)
}

// compose.ts:544-567 — the SHADOW blur is Float32Array storage with JS-number
// (f64) accumulation; the Rust port must narrow to f32 on every store.
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

// compose.ts:507-540 (dock spec; scratch allocated fresh — parity-neutral: the
// frozen scratch is zeroed/fully overwritten before every read).
function drawBareWithShadow(
  artwork: Raster,
  content: Raster,
  size: number,
  pad: number,
  cardSize: number,
  box: number,
  plate: { r: number; g: number; b: number },
): void {
  const spec = SHADOW_DOCK
  const layer = makeRaster(size)
  const alpha = new Float32Array(size * size)
  const tmp = new Float32Array(size * size)
  drawCentred(artwork, findContentBounds(artwork), layer, size, pad, cardSize, box)

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

// compose.ts:732-747 (identical body to raster.ts clipToMask)
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

// compose.ts:749-755
function compositeOver(target: Raster, over: Raster): void {
  const od = over.data
  const td = target.data
  for (let i4 = 0; i4 < od.length; i4 += 4) {
    if (od[i4 + 3] > 0) overAt(td, i4, od[i4], od[i4 + 1], od[i4 + 2], od[i4 + 3])
  }
}

// ---- the slice pipeline ------------------------------------------------------

/** Which drawScaled lane the cell takes (recorded for coverage reporting). */
function samplerLane(artwork: Raster, size: number): 'area' | 'supersample' {
  const b = findContentBounds(artwork)
  const box = fieldContentBox(SHAPE, size)
  const [w, h] = fit(Math.max(1, boundsW(b)), Math.max(1, boundsH(b)), box)
  return boundsW(b) >= w && boundsH(b) >= h ? 'area' : 'supersample'
}

/** Circle + fixed white plate + subject blit + dock silhouette shadow, then the
 *  renderTile tail: clip to the circle mask and composite onto a fresh target
 *  (compose.ts renderTile:184-223, mark/filter/arrow branches all inert). */
function renderSliceTile(artwork: Raster, size: number): Raster {
  const pad = 0 // no mark → cardInset 0
  const cardSize = size
  const tile = makeRaster(size)
  const box = fieldContentBox(SHAPE, cardSize)
  fillRegion(tile, size, pad, cardSize, WHITE.r, WHITE.g, WHITE.b)
  drawBareWithShadow(artwork, tile, size, pad, cardSize, box, WHITE)
  const cardMask = shapeMask(SHAPE, size, cardSize, pad, pad)
  applyCoverage(tile, cardMask)
  const target = makeRaster(size)
  compositeOver(target, tile)
  return target
}

// ---- cross-language probes (consumed by xtask spike4-compare) ----------------

function f64Bits(v: number): string {
  const buf = new ArrayBuffer(8)
  new Float64Array(buf)[0] = v
  const [lo, hi] = new Uint32Array(buf)
  return (BigInt(hi) << 32n | BigInt(lo)).toString(16).padStart(16, '0')
}

function fixtures(): string {
  const lines: string[] = []
  // The sRGB decode LUT, bit-exact: pins JSC Math.pow vs Rust libm::pow.
  for (let i = 0; i < 256; i++) lines.push(`lut\t${i}\t${f64Bits(srgbDecode(i))}`)
  // srgbEncode probes across the transfer curve (pins libm::pow on 1/2.4).
  for (let i = 0; i <= 4096; i++) {
    const v = i / 4096
    lines.push(`enc\t${f64Bits(v)}\t${srgbEncode(v)}`)
  }
  // The one shadow tone the slice uses (pins cbrt + the OKLab round trip).
  const s = fieldShadowTone({ ...WHITE })
  lines.push(`shadow\t${s.r}\t${s.g}\t${s.b}`)
  // Circle mask probes at both sizes (pins the 16×16 edge supersampler):
  // one interior, one exterior, and the first 6 FRACTIONAL boundary pixels.
  for (const size of SIZES) {
    const mask = shapeMask(SHAPE, size, size, 0, 0)
    const probes = [0, (size / 2) * size + size / 2]
    for (let i = 0; i < mask.length && probes.length < 8; i++) {
      if (mask[i] > 0 && mask[i] < 1) probes.push(i)
    }
    for (const i of probes) lines.push(`mask\t${size}\t${i}\t${f64Bits(mask[i])}`)
  }
  return `${lines.join('\n')}\n`
}

// ---- main ---------------------------------------------------------------------

function main(): void {
  mkdirSync(join(OUT, 'sources'), { recursive: true })
  mkdirSync(join(OUT, 'ts'), { recursive: true })
  const sources = loadMockSources(FRONTEND)
  const rows: string[] = []
  let area = 0
  let supersample = 0
  for (const s of sources) {
    writeFileSync(join(OUT, `sources/${s.id}.rgba`), Buffer.from(s.raster.data.buffer, s.raster.data.byteOffset, s.raster.data.byteLength))
    for (const size of SIZES) {
      const tile = renderSliceTile(s.raster, size)
      const lane = samplerLane(s.raster, size)
      if (lane === 'area') area++
      else supersample++
      writeFileSync(join(OUT, `ts/${s.id}-${size}.rgba`), Buffer.from(tile.data.buffer, tile.data.byteOffset, tile.data.byteLength))
      rows.push(`${s.id}\t${size}\t${rasterHash(tile)}\t${lane}`)
    }
  }
  writeFileSync(join(OUT, 'cells.tsv'), `${rows.join('\n')}\n`)
  writeFileSync(join(OUT, 'fixtures.tsv'), fixtures())
  console.log(`spike4 TS side: ${sources.length} sources × ${SIZES.length} sizes = ${rows.length} cells (${area} area-averaged, ${supersample} supersampled) → ${OUT}`)
}

main()
