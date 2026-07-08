// Colour maths + the palette / ink-contrast / lightness-polarity axes (spec 06 §5).

import { clampByte } from './constants.mjs'
import { range } from './prng.mjs'

export function hsl(h, s, l) {
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
export const luma = (c) => 0.299 * c.r + 0.587 * c.g + 0.114 * c.b
export const shade = (c, f) => ({ r: clampByte(c.r * f), g: clampByte(c.g * f), b: clampByte(c.b * f) })
export const lerp = (a, b, t) => {
  const aa = a.a ?? 255
  const ba = b.a ?? 255
  return { r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t, a: aa + (ba - aa) * t }
}

// Palette + the ink/contrast + lightness-polarity axes (spec 06 §5).
export function palette(rng) {
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
