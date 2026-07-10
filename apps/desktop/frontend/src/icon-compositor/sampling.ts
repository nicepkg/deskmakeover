import type { ContentBounds } from './analysis'
import type { Raster } from './raster'
import { makeRaster, overAt } from './raster'
import { srgbDecode, srgbEncode } from './color'

// Resampling — 1:1 port of the frozen C# oracle (TileRenderer.DrawScaled +
// IconResampler.cs, ADR-0015 D3). Downscales are TRUE area averages in linear
// light with premultiplied alpha (no dark fringing); upscales are 4×4
// supersampled premultiplied bilinear. NOTE: the sub-256 ICO ladder stays in
// C# (ADR-0015 D2) — this file serves the display path and the tile composer.

/** Premultiplied-alpha bilinear sample at fractional source coords (centre-based). */
export function sampleBilinearAt(src: Raster, fx: number, fy: number): [number, number, number, number] {
  fx = Math.min(src.width - 1, Math.max(0, fx))
  fy = Math.min(src.height - 1, Math.max(0, fy))
  const x0 = Math.floor(fx)
  const y0 = Math.floor(fy)
  const x1 = Math.min(x0 + 1, src.width - 1)
  const y1 = Math.min(y0 + 1, src.height - 1)
  const tx = fx - x0
  const ty = fy - y0
  const d = src.data
  const i00 = (y0 * src.width + x0) * 4
  const i10 = (y0 * src.width + x1) * 4
  const i01 = (y1 * src.width + x0) * 4
  const i11 = (y1 * src.width + x1) * 4
  const w00 = (1 - tx) * (1 - ty)
  const w10 = tx * (1 - ty)
  const w01 = (1 - tx) * ty
  const w11 = tx * ty

  const a = w00 * d[i00 + 3] + w10 * d[i10 + 3] + w01 * d[i01 + 3] + w11 * d[i11 + 3]
  if (a <= 0) return [0, 0, 0, 0]
  const r = (w00 * d[i00] * d[i00 + 3] + w10 * d[i10] * d[i10 + 3] + w01 * d[i01] * d[i01 + 3] + w11 * d[i11] * d[i11 + 3]) / a
  const g = (w00 * d[i00 + 1] * d[i00 + 3] + w10 * d[i10 + 1] * d[i10 + 3] + w01 * d[i01 + 1] * d[i01 + 3] + w11 * d[i11 + 1] * d[i11 + 3]) / a
  const b = (w00 * d[i00 + 2] * d[i00 + 3] + w10 * d[i10 + 2] * d[i10 + 3] + w01 * d[i01 + 2] * d[i01 + 3] + w11 * d[i11 + 2] * d[i11 + 3]) / a
  return [Math.round(r), Math.round(g), Math.round(b), Math.round(a)]
}

/** Bilinear sample in [0,1] UV space (TileRenderer.SampleBilinear). */
export function sampleBilinear(src: Raster, u: number, v: number): [number, number, number, number] {
  return sampleBilinearAt(src, u * src.width - 0.5, v * src.height - 0.5)
}

/**
 * Draw src[bounds] scaled to (dstW×dstH) at (dstX,dstY), composited OVER the
 * square content raster (TileRenderer.DrawScaled dispatch).
 */
export function drawScaled(
  src: Raster,
  b: ContentBounds,
  content: Raster,
  size: number,
  dstX: number,
  dstY: number,
  dstW: number,
  dstH: number,
): void {
  if (b.right - b.left >= dstW && b.bottom - b.top >= dstH) {
    drawAreaAveraged(src, b, content, size, dstX, dstY, dstW, dstH)
  } else {
    drawSupersampled(src, b, content, size, dstX, dstY, dstW, dstH)
  }
}

function drawAreaAveraged(
  src: Raster,
  b: ContentBounds,
  content: Raster,
  size: number,
  dstX: number,
  dstY: number,
  dstW: number,
  dstH: number,
): void {
  const bw = b.right - b.left
  const bh = b.bottom - b.top
  const scaleX = bw / dstW
  const scaleY = bh / dstH
  const sd = src.data
  for (let yy = 0; yy < dstH; yy++) {
    const ty = dstY + yy
    if (ty < 0 || ty >= size) continue
    const top = b.top + yy * scaleY
    const bottom = b.top + (yy + 1) * scaleY
    const y0 = Math.max(0, Math.trunc(top))
    const y1 = Math.min(src.height, Math.ceil(bottom))
    for (let xx = 0; xx < dstW; xx++) {
      const tx = dstX + xx
      if (tx < 0 || tx >= size) continue
      const left = b.left + xx * scaleX
      const right = b.left + (xx + 1) * scaleX
      const x0 = Math.max(0, Math.trunc(left))
      const x1 = Math.min(src.width, Math.ceil(right))

      // Linear-light, alpha-premultiplied accumulation.
      let r = 0
      let g = 0
      let bl = 0
      let aSum = 0
      let area = 0
      for (let y = y0; y < y1; y++) {
        const hy = Math.min(y + 1, bottom) - Math.max(y, top)
        if (hy <= 0) continue
        for (let x = x0; x < x1; x++) {
          const wx = Math.min(x + 1, right) - Math.max(x, left)
          if (wx <= 0) continue
          const w = wx * hy
          const i4 = (y * src.width + x) * 4
          const af = (sd[i4 + 3] / 255) * w
          r += srgbDecode(sd[i4]) * af
          g += srgbDecode(sd[i4 + 1]) * af
          bl += srgbDecode(sd[i4 + 2]) * af
          aSum += sd[i4 + 3] * w
          area += w
        }
      }
      if (area <= 0 || aSum <= 0) continue
      const weight = aSum / 255
      const outA = Math.min(255, Math.max(0, Math.round(aSum / area)))
      if (outA === 0) continue
      overAt(content.data, (ty * size + tx) * 4, srgbEncode(r / weight), srgbEncode(g / weight), srgbEncode(bl / weight), outA)
    }
  }
}

function drawSupersampled(
  src: Raster,
  b: ContentBounds,
  content: Raster,
  size: number,
  dstX: number,
  dstY: number,
  dstW: number,
  dstH: number,
): void {
  const sub = 4
  const subStep = 1 / sub
  const bw = b.right - b.left
  const bh = b.bottom - b.top
  for (let yy = 0; yy < dstH; yy++) {
    const ty = dstY + yy
    if (ty < 0 || ty >= size) continue
    for (let xx = 0; xx < dstW; xx++) {
      const tx = dstX + xx
      if (tx < 0 || tx >= size) continue
      let r = 0
      let g = 0
      let bl = 0
      let a = 0
      for (let sy2 = 0; sy2 < sub; sy2++) {
        for (let sx2 = 0; sx2 < sub; sx2++) {
          const sx = b.left + ((xx + (sx2 + 0.5) * subStep) / dstW) * bw
          const sy = b.top + ((yy + (sy2 + 0.5) * subStep) / dstH) * bh
          const [pr, pg, pb, pa] = sampleBilinearAt(src, sx - 0.5, sy - 0.5)
          r += srgbDecode(pr) * pa
          g += srgbDecode(pg) * pa
          bl += srgbDecode(pb) * pa
          a += pa
        }
      }
      if (a <= 0) continue
      overAt(
        content.data,
        (ty * size + tx) * 4,
        srgbEncode(r / a),
        srgbEncode(g / a),
        srgbEncode(bl / a),
        Math.round(a / (sub * sub)),
      )
    }
  }
}

/** IconResampler.Downscale — linear-light premultiplied box average to target². */
export function downscale(src: Raster, target: number): Raster {
  if (target >= src.width) return src
  const dst = makeRaster(target)
  const sw = src.width
  const sh = src.height
  const sd = src.data
  const scaleX = sw / target
  const scaleY = sh / target
  for (let dy = 0; dy < target; dy++) {
    const top = dy * scaleY
    const bottom = (dy + 1) * scaleY
    const y0 = Math.trunc(top)
    const y1 = Math.min(sh, Math.ceil(bottom))
    for (let dx = 0; dx < target; dx++) {
      const left = dx * scaleX
      const right = (dx + 1) * scaleX
      const x0 = Math.trunc(left)
      const x1 = Math.min(sw, Math.ceil(right))
      let rp = 0
      let gp = 0
      let bp = 0
      let aSum = 0
      let areaSum = 0
      for (let y = y0; y < y1; y++) {
        const hy = Math.min(y + 1, bottom) - Math.max(y, top)
        if (hy <= 0) continue
        for (let x = x0; x < x1; x++) {
          const wx = Math.min(x + 1, right) - Math.max(x, left)
          if (wx <= 0) continue
          const area = wx * hy
          const i4 = (y * sw + x) * 4
          const af = (sd[i4 + 3] / 255) * area
          rp += srgbDecode(sd[i4]) * af
          gp += srgbDecode(sd[i4 + 1]) * af
          bp += srgbDecode(sd[i4 + 2]) * af
          aSum += sd[i4 + 3] * area
          areaSum += area
        }
      }
      const o4 = (dy * target + dx) * 4
      if (areaSum <= 0 || aSum <= 0) continue
      const weight = aSum / 255
      dst.data[o4] = srgbEncode(rp / weight)
      dst.data[o4 + 1] = srgbEncode(gp / weight)
      dst.data[o4 + 2] = srgbEncode(bp / weight)
      dst.data[o4 + 3] = Math.min(255, Math.max(0, Math.round(aSum / areaSum)))
    }
  }
  return dst
}
