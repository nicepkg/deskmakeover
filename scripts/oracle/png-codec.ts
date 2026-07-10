// Deterministic PNG codec for the M0b oracle corpus (ADR-0019 §Parity). The
// mock pack is uniform 8-bit RGBA (colortype 6, no interlace, no colour chunks
// — verified over all 120 sources), so a pure node:zlib decode reproduces the
// browser canvas' straight-alpha pixels byte-for-byte with no colour management
// to diverge on. This decode is the CANONICAL oracle input: the Rust
// dm-icon-codec must reproduce it. Encode is deterministic (fixed zlib level +
// None filter), proven by the harness' capture-twice byte-diff.

import { createHash } from 'node:crypto'
import { deflateSync, inflateSync } from 'node:zlib'
import type { Raster } from '@/icon-compositor/raster'

const PNG_SIG = Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10])
const DEFLATE_LEVEL = 9

// ---- CRC32 (PNG chunk checksums) --------------------------------------------

const CRC_TABLE = (() => {
  const t = new Uint32Array(256)
  for (let n = 0; n < 256; n++) {
    let c = n
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    t[n] = c >>> 0
  }
  return t
})()

function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff
  for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8)
  return (c ^ 0xffffffff) >>> 0
}

// ---- decode -----------------------------------------------------------------

function readU32(b: Uint8Array, off: number): number {
  return ((b[off] << 24) | (b[off + 1] << 16) | (b[off + 2] << 8) | b[off + 3]) >>> 0
}

function paeth(a: number, b: number, c: number): number {
  const p = a + b - c
  const pa = Math.abs(p - a)
  const pb = Math.abs(p - b)
  const pc = Math.abs(p - c)
  if (pa <= pb && pa <= pc) return a
  return pb <= pc ? b : c
}

/**
 * Decode an 8-bit truecolour+alpha (colortype 6) non-interlaced PNG into a
 * straight-alpha Raster. Throws on any format the mock pack does not use, so a
 * silent wrong-format decode can never poison a golden.
 */
export function decodePng(bytes: Uint8Array): Raster {
  for (let i = 0; i < 8; i++) {
    if (bytes[i] !== PNG_SIG[i]) throw new Error('not a PNG (bad signature)')
  }
  let width = 0
  let height = 0
  const idat: Uint8Array[] = []
  let off = 8
  let sawIhdr = false
  while (off < bytes.length) {
    const len = readU32(bytes, off)
    const type = String.fromCharCode(bytes[off + 4], bytes[off + 5], bytes[off + 6], bytes[off + 7])
    const dataOff = off + 8
    if (type === 'IHDR') {
      width = readU32(bytes, dataOff)
      height = readU32(bytes, dataOff + 4)
      const bitDepth = bytes[dataOff + 8]
      const colorType = bytes[dataOff + 9]
      const interlace = bytes[dataOff + 12]
      if (bitDepth !== 8 || colorType !== 6 || interlace !== 0) {
        throw new Error(`unsupported PNG (bitDepth=${bitDepth} colorType=${colorType} interlace=${interlace})`)
      }
      sawIhdr = true
    } else if (type === 'IDAT') {
      idat.push(bytes.subarray(dataOff, dataOff + len))
    } else if (type === 'IEND') {
      break
    }
    off = dataOff + len + 4 // skip data + CRC
  }
  if (!sawIhdr) throw new Error('PNG missing IHDR')

  const raw = inflateSync(idat.length === 1 ? idat[0] : concat(idat))
  const stride = width * 4
  const out = new Uint8ClampedArray(width * height * 4)
  let rp = 0
  for (let y = 0; y < height; y++) {
    const filter = raw[rp++]
    const row = y * stride
    const prev = row - stride
    for (let x = 0; x < stride; x++) {
      const cur = raw[rp++]
      const a = x >= 4 ? out[row + x - 4] : 0
      const b = y > 0 ? out[prev + x] : 0
      const c = y > 0 && x >= 4 ? out[prev + x - 4] : 0
      let v: number
      switch (filter) {
        case 0: v = cur; break
        case 1: v = cur + a; break
        case 2: v = cur + b; break
        case 3: v = cur + ((a + b) >> 1); break
        case 4: v = cur + paeth(a, b, c); break
        default: throw new Error(`bad PNG filter ${filter} at row ${y}`)
      }
      out[row + x] = v & 0xff
    }
  }
  return { width, height, data: out }
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

// ---- encode -----------------------------------------------------------------

function chunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(12 + data.length)
  const len = data.length
  out[0] = (len >>> 24) & 0xff
  out[1] = (len >>> 16) & 0xff
  out[2] = (len >>> 8) & 0xff
  out[3] = len & 0xff
  out[4] = type.charCodeAt(0)
  out[5] = type.charCodeAt(1)
  out[6] = type.charCodeAt(2)
  out[7] = type.charCodeAt(3)
  out.set(data, 8)
  const crc = crc32(out.subarray(4, 8 + data.length))
  out[8 + data.length] = (crc >>> 24) & 0xff
  out[9 + data.length] = (crc >>> 16) & 0xff
  out[10 + data.length] = (crc >>> 8) & 0xff
  out[11 + data.length] = crc & 0xff
  return out
}

function ihdr(width: number, height: number, colorType: number): Uint8Array {
  const d = new Uint8Array(13)
  d[0] = (width >>> 24) & 0xff
  d[1] = (width >>> 16) & 0xff
  d[2] = (width >>> 8) & 0xff
  d[3] = width & 0xff
  d[4] = (height >>> 24) & 0xff
  d[5] = (height >>> 16) & 0xff
  d[6] = (height >>> 8) & 0xff
  d[7] = height & 0xff
  d[8] = 8 // bit depth
  d[9] = colorType
  return d // compression/filter/interlace default 0
}

/** Prefix each scanline with filter type 4 (Paeth), applied unconditionally —
 *  deterministic (no per-row heuristic to diverge across toolchains) and far
 *  smaller than None on gradient/photographic tiles, equal on flat plates. */
function filterPaeth(pixels: Uint8Array | Uint8ClampedArray, width: number, height: number, channels: number): Uint8Array {
  const stride = width * channels
  const out = new Uint8Array((stride + 1) * height)
  for (let y = 0; y < height; y++) {
    const dst = y * (stride + 1)
    out[dst] = 4
    const row = y * stride
    const prev = row - stride
    for (let x = 0; x < stride; x++) {
      const a = x >= channels ? pixels[row + x - channels] : 0
      const b = y > 0 ? pixels[prev + x] : 0
      const c = y > 0 && x >= channels ? pixels[prev + x - channels] : 0
      out[dst + 1 + x] = (pixels[row + x] - paeth(a, b, c)) & 0xff
    }
  }
  return out
}

function assemble(width: number, height: number, colorType: number, channels: number, pixels: Uint8Array | Uint8ClampedArray): Uint8Array {
  const compressed = deflateSync(filterPaeth(pixels, width, height, channels), { level: DEFLATE_LEVEL })
  return concat([PNG_SIG, chunk('IHDR', ihdr(width, height, colorType)), chunk('IDAT', compressed), chunk('IEND', new Uint8Array(0))])
}

/** Encode a straight-alpha RGBA Raster as an 8-bit colortype-6 PNG. */
export function encodeRgbaPng(raster: Raster): Uint8Array {
  return assemble(raster.width, raster.height, 6, 4, raster.data)
}

/** Encode an 8-bit grayscale (colortype 0) PNG from a single-channel buffer. */
export function encodeGrayPng(width: number, height: number, gray: Uint8Array): Uint8Array {
  return assemble(width, height, 0, 1, gray)
}

// ---- hashing ----------------------------------------------------------------

export function sha256Hex(bytes: Uint8Array): string {
  return createHash('sha256').update(bytes).digest('hex')
}

/** Hash of a Raster's raw RGBA pixels — the platform-independent parity anchor
 *  (PNG container bytes vary by zlib build; pixels do not). */
export function rasterHash(raster: Raster): string {
  return createHash('sha256').update(Buffer.from(raster.data.buffer, raster.data.byteOffset, raster.data.byteLength)).digest('hex')
}
