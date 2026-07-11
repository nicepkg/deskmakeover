// M6 single-truth cutover — the Auto-plate swatch helper (`monoRamp`), ported
// byte-for-byte from the frozen `icon-compositor/color.ts` so the panel swatch
// (`icon-axis-options.ts` paleOf) survives the pixel-oracle deletion. This is a
// UI swatch helper, NOT on the certified render path.

import type { Rgba } from './color-math'
import { monoTone } from './color-math'

const rampCache = new Map<number, Uint8ClampedArray>()

function buildRamp(tint: number): Uint8ClampedArray {
  const lut = new Uint8ClampedArray(256 * 3)
  for (let i = 0; i < 256; i++) {
    let t = i / 255
    const sep = t * t * (3 - 2 * t)
    t = 0.42 * t + 0.58 * sep
    // Light end 0.965/0.22 (was 0.94/0.35): white plates in Mono must read
    // near-white with a whisper of tint, not a solid pastel card
    // (owner call 2026-07-09: "目前颜色略深,调淡一些更偏白").
    const lightness = 0.4 + (0.965 - 0.4) * t
    const chromaScale = 1.15 + (0.22 - 1.15) * t
    const tone = monoTone(lightness, chromaScale, tint)
    lut[i * 3] = tone.r
    lut[i * 3 + 1] = tone.g
    lut[i * 3 + 2] = tone.b
  }
  return lut
}

/** The mono tonal ramp: t∈[0,1] darkest→lightest position of the tint's hue. */
export function monoRamp(t: number, tint: number): Rgba {
  let lut = rampCache.get(tint)
  if (!lut) {
    lut = buildRamp(tint)
    rampCache.set(tint, lut)
  }
  const i = Math.min(255, Math.max(0, Math.round(t * 255)))
  return { r: lut[i * 3], g: lut[i * 3 + 1], b: lut[i * 3 + 2], a: 255 }
}
