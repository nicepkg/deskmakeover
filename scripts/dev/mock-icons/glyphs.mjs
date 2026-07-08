// Own-art glyph + monogram cover builders (glyph-kind, letter, and CJK axes).

import { range } from './prng.mjs'
import { coverCircle, coverRing, coverRRect, coverRect, coverPoly, union } from './raster.mjs'

export function glyphCover(kind, cx, cy, s) {
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
export const GLYPHS = ['disc', 'ring', 'bars', 'play', 'grid', 'diamond', 'note', 'spark']

// Bold geometric monogram (own art) — the letter-tile axis. Latin from rect unions,
// CJK as an abstract stroke lattice (reads as 汉字 without needing a bundled font).
export function letterCover(ch, cx, cy, s) {
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
export function hanziCover(rng, cx, cy, s) {
  const h = s / 2
  const t = s * 0.12
  const parts = [coverRect(cx, cy - h + t, s, t), coverRect(cx, cy + h - t, s, t)]
  const rows = 1 + Math.floor(rng() * 2)
  for (let i = 0; i < rows; i++) parts.push(coverRect(cx, cy + range(rng, -h * 0.4, h * 0.4), s * 0.9, t))
  const cols = 1 + Math.floor(rng() * 2)
  for (let i = 0; i < cols; i++) parts.push(coverRect(cx + range(rng, -h * 0.4, h * 0.4), cy, t, s * 0.92))
  return union(...parts)
}
