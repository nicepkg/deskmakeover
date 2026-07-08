// Inline PNG encoder with adaptive per-row filtering (node:zlib, dependency-free).
// WebP is the spec's preferred container, but a dependency-free Node toolchain has no
// WebP encoder — this is the same path scripts/dev/render-app-icon.mjs already ships.

import { deflateSync } from 'node:zlib'

export function encodePng(width, height, rgba) {
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
