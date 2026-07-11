// M6 single-truth cutover — the pure OKLab / sRGB colour helpers the main-thread
// SEED/SWATCH layer needs: the cross-icon hue-spread pre-pass (`hue-spread.ts`,
// which picks each icon's adjusted plate seed) and the Auto-plate swatch
// (`mono-ramp.ts`). Ported byte-for-byte from the frozen `icon-compositor/`
// oracle (`color.ts` + `raster.ts`) so these survivors carry ZERO dependency on
// the deleted pixel modules. This is NOT the certified tile kernel — that lives
// in Rust (`dm-icon-wasm`); this is the tiny bit of colour math that runs on the
// main thread to choose inputs the kernel consumes. The math is identical to the
// frozen source; the m6 cert + the m5 hue gate pin that equivalence.

/** An opaque-or-translucent colour in 0-255 channels (straight alpha). */
export interface Rgba {
  r: number
  g: number
  b: number
  a: number
}

export function fromRgbInt(rgb: number): Rgba {
  return { r: (rgb >> 16) & 0xff, g: (rgb >> 8) & 0xff, b: rgb & 0xff, a: 255 }
}

export function clampByte(v: number): number {
  return v < 0 ? 0 : v > 255 ? 255 : Math.round(v)
}

// ---- sRGB ↔ linear (SrgbLinear.cs) ----

const DECODE_LUT = buildDecodeLut()

function buildDecodeLut(): Float64Array {
  const lut = new Float64Array(256)
  for (let i = 0; i < 256; i++) {
    const srgb = i / 255
    lut[i] = srgb <= 0.04045 ? srgb / 12.92 : Math.pow((srgb + 0.055) / 1.055, 2.4)
  }
  return lut
}

/** Linear-light [0,1] → sRGB byte (exact transfer curve). */
function srgbEncode(linear: number): number {
  const v = linear < 0 ? 0 : linear > 1 ? 1 : linear
  const srgb = v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055
  return clampByte(srgb * 255)
}

// ---- OKLab (private math the ramp + hue-spread share) ----

interface OkLab {
  L: number
  A: number
  B: number
}

export function toOkLab(r: number, g: number, b: number): { L: number; A: number; B: number } {
  const rl = DECODE_LUT[r]
  const gl = DECODE_LUT[g]
  const bl = DECODE_LUT[b]
  const l = Math.cbrt(0.4122214708 * rl + 0.5363325363 * gl + 0.0514459929 * bl)
  const m = Math.cbrt(0.2119034982 * rl + 0.6806995451 * gl + 0.1073969566 * bl)
  const s = Math.cbrt(0.0883024619 * rl + 0.2817188376 * gl + 0.6299787005 * bl)
  return {
    L: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    A: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    B: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  }
}

function okLabToLinear(lab: OkLab): [number, number, number] {
  let l = lab.L + 0.3963377774 * lab.A + 0.2158037573 * lab.B
  let m = lab.L - 0.1055613458 * lab.A - 0.0638541728 * lab.B
  let s = lab.L - 0.0894841775 * lab.A - 1.291485548 * lab.B
  l = l * l * l
  m = m * m * m
  s = s * s * s
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ]
}

function tryOkLabToSrgb(lab: OkLab): { rgb: Rgba; inGamut: boolean } {
  const [r, g, b] = okLabToLinear(lab)
  const inGamut = r >= -0.0005 && r <= 1.0005 && g >= -0.0005 && g <= 1.0005 && b >= -0.0005 && b <= 1.0005
  return {
    rgb: {
      r: srgbEncode(Math.min(1, Math.max(0, r))),
      g: srgbEncode(Math.min(1, Math.max(0, g))),
      b: srgbEncode(Math.min(1, Math.max(0, b))),
      a: 255,
    },
    inGamut,
  }
}

function gamutFit(L: number, ua: number, ub: number, c: number): Rgba {
  for (let attempt = 0; attempt < 8; attempt++) {
    const fit = tryOkLabToSrgb({ L, A: ua * c, B: ub * c })
    if (fit.inGamut) return fit.rgb
    c *= 0.82
  }
  return tryOkLabToSrgb({ L, A: ua * c, B: ub * c }).rgb
}

function hueUnit(seed: Rgba): { ua: number; ub: number; chroma: number; L: number } {
  const lab = toOkLab(seed.r, seed.g, seed.b)
  const chroma = Math.sqrt(lab.A * lab.A + lab.B * lab.B)
  return {
    ua: chroma < 1e-6 ? 0 : lab.A / chroma,
    ub: chroma < 1e-6 ? 0 : lab.B / chroma,
    chroma,
    L: lab.L,
  }
}

/** One tone of the tint's hue: OKLab lightness + chroma scale, gamut-fit (MonoTone). */
export function monoTone(lightness: number, chromaScale: number, tint: number): Rgba {
  const seedC = fromRgbInt(tint)
  const seed = toOkLab(seedC.r, seedC.g, seedC.b)
  const chroma = Math.sqrt(seed.A * seed.A + seed.B * seed.B)
  const ua = chroma < 1e-6 ? 0 : seed.A / chroma
  const ub = chroma < 1e-6 ? 0 : seed.B / chroma
  let c = Math.min(0.145, Math.max(0.035, chroma)) * chromaScale
  for (let attempt = 0; attempt < 8; attempt++) {
    const fit = tryOkLabToSrgb({ L: lightness, A: ua * c, B: ub * c })
    if (fit.inGamut) return fit.rgb
    c *= 0.82
  }
  return tryOkLabToSrgb({ L: lightness, A: ua * c, B: ub * c }).rgb
}

/** The seed with its OKLab hue rotated by `deltaRad` (L + chroma preserved,
 *  gamut-fit) — the hue-spread pass separates colliding PLATE hues with this;
 *  subject pixels are never touched (law 4). */
export function rotateSeedHue(seed: Rgba, deltaRad: number): Rgba {
  if (deltaRad === 0) return { ...seed, a: 255 }
  const { ua, ub, chroma, L } = hueUnit(seed)
  if (chroma < 1e-6) return { ...seed, a: 255 }
  const cos = Math.cos(deltaRad)
  const sin = Math.sin(deltaRad)
  return gamutFit(L, ua * cos - ub * sin, ua * sin + ub * cos, chroma)
}
