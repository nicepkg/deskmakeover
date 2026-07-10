// Source-image decode + normalize for the real-icon oracle pack (ADR-0015 D9). The
// frozen golden codec (png-codec) only decodes the 8-bit colorType-6 PNGs WE encode;
// harvested real icons arrive as colorType 0/2/3/4/6 (incl. 4-bit indexed). The pack is
// normalized to 100% PNG at harvest (the one upstream .webp is converted with sips at
// harvest time), so this decoder is pure PNG — no runtime image-tool dependency, and the
// always-on gate runs on any platform.
//
// Sources are decoded to RGBA and normalized to the 256² master the app feeds renderTile
// (render.worker resizes every source to MASTER via canvas drawImage). The normalized
// raster is what the corpus captures and what --verify re-derives, so the resampler only
// needs to match ITSELF (deterministic), never the browser.
//
// zlib note: PNG IDAT is zlib-format (RFC 1950). Bun's `Bun.inflateSync` is RAW deflate
// and cannot read it; `node:zlib` (a native implementation under Bun) is the correct
// zlib-format decoder and is used deliberately here.

import { inflateSync } from 'node:zlib'
import type { Raster } from '@/icon-compositor/raster'

export const MASTER = 256

const SIG = [137, 80, 78, 71, 13, 10, 26, 10]

function readU32(b: Uint8Array, o: number): number {
  return ((b[o] << 24) | (b[o + 1] << 16) | (b[o + 2] << 8) | b[o + 3]) >>> 0
}

function paeth(a: number, b: number, c: number): number {
  const p = a + b - c
  const pa = Math.abs(p - a)
  const pb = Math.abs(p - b)
  const pc = Math.abs(p - c)
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c
}

function channels(colorType: number): number {
  switch (colorType) {
    case 0:
      return 1 // grayscale
    case 2:
      return 3 // RGB
    case 3:
      return 1 // palette index
    case 4:
      return 2 // gray + alpha
    case 6:
      return 4 // RGBA
    default:
      throw new Error(`unsupported PNG colorType ${colorType}`)
  }
}

function concat(parts: Uint8Array[]): Uint8Array {
  let total = 0
  for (const p of parts) total += p.length
  const out = new Uint8Array(total)
  let o = 0
  for (const p of parts) {
    out.set(p, o)
    o += p.length
  }
  return out
}

/** Palette/grayscale sample at (x,y) for sub-byte or 8-bit depths (MSB-first packing). */
function sampleAt(buf: Uint8Array, bytesPerRow: number, bitDepth: number, x: number, y: number): number {
  const rowStart = y * bytesPerRow
  if (bitDepth === 8) return buf[rowStart + x]
  const perByte = 8 / bitDepth
  const byte = buf[rowStart + Math.floor(x / perByte)]
  const shift = (perByte - 1 - (x % perByte)) * bitDepth
  return (byte >> shift) & ((1 << bitDepth) - 1)
}

/** Decode a PNG (colorType 0/2/3/4/6, 8-bit or sub-byte palette, non-interlaced) to RGBA,
 *  handling PLTE + tRNS. Broader than the frozen golden codec, which only needs type 6. */
export function decodePngAny(bytes: Uint8Array): Raster {
  for (let i = 0; i < 8; i++) if (bytes[i] !== SIG[i]) throw new Error('not a PNG (bad signature)')
  let width = 0
  let height = 0
  let bitDepth = 8
  let colorType = 6
  let interlace = 0
  let plte: Uint8Array | null = null
  let trns: Uint8Array | null = null
  const idat: Uint8Array[] = []
  let off = 8
  while (off < bytes.length) {
    const len = readU32(bytes, off)
    const type = String.fromCharCode(bytes[off + 4], bytes[off + 5], bytes[off + 6], bytes[off + 7])
    const d = off + 8
    if (type === 'IHDR') {
      width = readU32(bytes, d)
      height = readU32(bytes, d + 4)
      bitDepth = bytes[d + 8]
      colorType = bytes[d + 9]
      interlace = bytes[d + 12]
    } else if (type === 'PLTE') {
      plte = bytes.subarray(d, d + len)
    } else if (type === 'tRNS') {
      trns = bytes.subarray(d, d + len)
    } else if (type === 'IDAT') {
      idat.push(bytes.subarray(d, d + len))
    } else if (type === 'IEND') {
      break
    }
    off = d + len + 4 // data + CRC
  }
  if (interlace !== 0) throw new Error('interlaced PNG unsupported')
  if (bitDepth !== 8 && colorType !== 3) {
    throw new Error(`unsupported PNG (bitDepth=${bitDepth} colorType=${colorType})`)
  }
  const ch = channels(colorType)

  // Unfilter into raw channel/index bytes (filters act on bytes; bpp = ceil(bits/8)).
  const bpp = Math.max(1, Math.ceil((bitDepth * ch) / 8))
  const bytesPerRow = Math.ceil((width * bitDepth * ch) / 8)
  const raw = inflateSync(idat.length === 1 ? idat[0] : concat(idat))
  const buf = new Uint8Array(bytesPerRow * height)
  let rp = 0
  for (let y = 0; y < height; y++) {
    const filter = raw[rp++]
    const row = y * bytesPerRow
    const prev = row - bytesPerRow
    for (let x = 0; x < bytesPerRow; x++) {
      const cur = raw[rp++]
      const a = x >= bpp ? buf[row + x - bpp] : 0
      const b = y > 0 ? buf[prev + x] : 0
      const c = y > 0 && x >= bpp ? buf[prev + x - bpp] : 0
      let v: number
      switch (filter) {
        case 0:
          v = cur
          break
        case 1:
          v = cur + a
          break
        case 2:
          v = cur + b
          break
        case 3:
          v = cur + ((a + b) >> 1)
          break
        case 4:
          v = cur + paeth(a, b, c)
          break
        default:
          throw new Error(`bad PNG filter ${filter} at row ${y}`)
      }
      buf[row + x] = v & 0xff
    }
  }

  const out = new Uint8ClampedArray(width * height * 4)
  const n = width * height
  if (colorType === 6) {
    out.set(buf)
  } else if (colorType === 2) {
    for (let i = 0; i < n; i++) {
      out[i * 4] = buf[i * 3]
      out[i * 4 + 1] = buf[i * 3 + 1]
      out[i * 4 + 2] = buf[i * 3 + 2]
      out[i * 4 + 3] = 255
    }
    if (trns && trns.length >= 6) {
      const [tr, tg, tb] = [trns[1], trns[3], trns[5]]
      for (let i = 0; i < n; i++) {
        if (out[i * 4] === tr && out[i * 4 + 1] === tg && out[i * 4 + 2] === tb) out[i * 4 + 3] = 0
      }
    }
  } else if (colorType === 4) {
    for (let i = 0; i < n; i++) {
      const g = buf[i * 2]
      out[i * 4] = g
      out[i * 4 + 1] = g
      out[i * 4 + 2] = g
      out[i * 4 + 3] = buf[i * 2 + 1]
    }
  } else if (colorType === 0) {
    for (let i = 0; i < n; i++) {
      const s = sampleAt(buf, bytesPerRow, bitDepth, i % width, Math.floor(i / width))
      const g = bitDepth === 8 ? s : Math.round((s * 255) / ((1 << bitDepth) - 1))
      out[i * 4] = g
      out[i * 4 + 1] = g
      out[i * 4 + 2] = g
      out[i * 4 + 3] = 255
    }
  } else {
    if (!plte) throw new Error('palette PNG missing PLTE')
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const idx = sampleAt(buf, bytesPerRow, bitDepth, x, y)
        const o = (y * width + x) * 4
        out[o] = plte[idx * 3]
        out[o + 1] = plte[idx * 3 + 1]
        out[o + 2] = plte[idx * 3 + 2]
        out[o + 3] = trns && idx < trns.length ? trns[idx] : 255
      }
    }
  }
  return { width, height, data: out }
}

/** Premultiplied bilinear resize to size×size (identity when already that size). */
function resizeTo(src: Raster, size: number): Raster {
  if (src.width === size && src.height === size) return src
  const out = new Uint8ClampedArray(size * size * 4)
  const d = src.data
  const sx = src.width / size
  const sy = src.height / size
  const clampI = (v: number, hi: number) => (v < 0 ? 0 : v > hi ? hi : v)
  for (let y = 0; y < size; y++) {
    const fy = (y + 0.5) * sy - 0.5
    const y0 = clampI(Math.floor(fy), src.height - 1)
    const y1 = Math.min(src.height - 1, y0 + 1)
    const ty = fy <= 0 ? 0 : fy >= src.height - 1 ? 1 : fy - Math.floor(fy)
    for (let x = 0; x < size; x++) {
      const fx = (x + 0.5) * sx - 0.5
      const x0 = clampI(Math.floor(fx), src.width - 1)
      const x1 = Math.min(src.width - 1, x0 + 1)
      const tx = fx <= 0 ? 0 : fx >= src.width - 1 ? 1 : fx - Math.floor(fx)
      const i00 = (y0 * src.width + x0) * 4
      const i10 = (y0 * src.width + x1) * 4
      const i01 = (y1 * src.width + x0) * 4
      const i11 = (y1 * src.width + x1) * 4
      const a00 = d[i00 + 3]
      const a10 = d[i10 + 3]
      const a01 = d[i01 + 3]
      const a11 = d[i11 + 3]
      const w00 = (1 - tx) * (1 - ty)
      const w10 = tx * (1 - ty)
      const w01 = (1 - tx) * ty
      const w11 = tx * ty
      const a = w00 * a00 + w10 * a10 + w01 * a01 + w11 * a11
      const o = (y * size + x) * 4
      if (a <= 0) {
        out[o] = 0
        out[o + 1] = 0
        out[o + 2] = 0
        out[o + 3] = 0
        continue
      }
      for (let c = 0; c < 3; c++) {
        const pm =
          w00 * d[i00 + c] * a00 + w10 * d[i10 + c] * a10 + w01 * d[i01 + c] * a01 + w11 * d[i11 + c] * a11
        out[o + c] = Math.round(pm / a)
      }
      out[o + 3] = Math.round(a)
    }
  }
  return { width: size, height: size, data: out }
}

/** Decode a harvested source PNG and normalize to the 256² master. The pack is all-PNG
 *  (normalized at harvest), so this covers every source; a non-PNG throws loudly in
 *  `decodePngAny` — never silent. */
export function decodeSourceImage(bytes: Uint8Array): Raster {
  return resizeTo(decodePngAny(bytes), MASTER)
}
