// sRGB ↔ OKLCH conversions (Björn Ottosson's OKLab, D65). The Adaptive Frost
// recipe (spec 04 §4.1) reasons about panel fills in OKLCH so lightness and
// chroma edits never rotate hue. Pure math, no dependencies.

export interface Oklch {
  l: number // 0..1
  c: number // 0..~0.37
  h: number // 0..360, meaningless when c ≈ 0
}

const srgbToLinear = (c: number): number => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4)
const linearToSrgb = (c: number): number => (c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055)

/** sRGB components 0..1 → OKLCH. */
export function rgbToOklch(r: number, g: number, b: number): Oklch {
  const lr = srgbToLinear(r)
  const lg = srgbToLinear(g)
  const lb = srgbToLinear(b)

  const l_ = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb)
  const m_ = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb)
  const s_ = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb)

  const L = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_
  const a = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_
  const bb = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_

  const c = Math.sqrt(a * a + bb * bb)
  let h = (Math.atan2(bb, a) * 180) / Math.PI
  if (h < 0) h += 360
  return { l: L, c, h }
}

/** OKLCH → sRGB components 0..1, channel-clamped (fine for UI-range colours). */
export function oklchToRgb({ l, c, h }: Oklch): { r: number; g: number; b: number } {
  const rad = (h * Math.PI) / 180
  const a = c * Math.cos(rad)
  const bb = c * Math.sin(rad)

  const l_ = (l + 0.3963377774 * a + 0.2158037573 * bb) ** 3
  const m_ = (l - 0.1055613458 * a - 0.0638541728 * bb) ** 3
  const s_ = (l - 0.0894841775 * a - 1.291485548 * bb) ** 3

  const lr = 4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_
  const lg = -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_
  const lb = -0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_

  const clamp01 = (v: number) => Math.min(1, Math.max(0, v))
  return { r: clamp01(linearToSrgb(lr)), g: clamp01(linearToSrgb(lg)), b: clamp01(linearToSrgb(lb)) }
}

export function oklchToHex(color: Oklch): string {
  const { r, g, b } = oklchToRgb(color)
  const to = (v: number) => Math.round(v * 255).toString(16).padStart(2, '0')
  return `#${to(r)}${to(g)}${to(b)}`.toUpperCase()
}

export function hexToOklch(hex: string): Oklch {
  const s = hex.replace('#', '')
  const n = s.length === 3 ? s.split('').map((c) => c + c).join('') : s
  return rgbToOklch(
    parseInt(n.slice(0, 2), 16) / 255,
    parseInt(n.slice(2, 4), 16) / 255,
    parseInt(n.slice(4, 6), 16) / 255,
  )
}
