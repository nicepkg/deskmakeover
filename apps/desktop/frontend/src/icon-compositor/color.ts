import type { Subject } from '@/bridge/types'
import type { Raster, Rgba } from './raster'
import { clampByte, fromRgbInt } from './raster'

// Colour math — 1:1 port of the frozen C# oracle (IconColorTreatment.cs +
// SrgbLinear.cs, ADR-0015 D3): Rec.601 luma, prototype gray/HSL primitives,
// and the Material-style OKLab tonal duotone for 单色 with the per-tile
// adaptive P5-P95 lightness stretch.

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

/** sRGB byte → linear-light [0,1]. */
export function srgbDecode(value: number): number {
  return DECODE_LUT[value]
}

/** Linear-light [0,1] → sRGB byte (exact transfer curve). */
export function srgbEncode(linear: number): number {
  const v = linear < 0 ? 0 : linear > 1 ? 1 : linear
  const srgb = v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055
  return clampByte(srgb * 255)
}

// ---- prototype primitives ----

/** Ink threshold for 原彩 (0.66) — deliberately distinct from the mark threshold 0.58. */
export const ORIGINAL_INK_THRESHOLD = 0.66

/** Rec.601 luminance of 0-255 channels. */
export function luminance(r: number, g: number, b: number): number {
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255
}

/** The prototype grayOf value: 255·clamp(0.5+(l−0.5)·1.4, 0.08, 0.94). */
export function grayValue(l: number): number {
  return Math.round(255 * Math.min(0.94, Math.max(0.08, 0.5 + (l - 0.5) * 1.4)))
}

/** Hue (0-360) and saturation (0-1) of a packed 0xRRGGBB (prototype hsl). */
export function hslOf(rgb: number): { h: number; s: number } {
  const r = ((rgb >> 16) & 0xff) / 255
  const g = ((rgb >> 8) & 0xff) / 255
  const b = (rgb & 0xff) / 255
  const mx = Math.max(r, g, b)
  const mn = Math.min(r, g, b)
  const d = mx - mn
  let h = 0
  if (d > 0) {
    if (mx === r) h = ((g - b) / d) % 6
    else if (mx === g) h = (b - r) / d + 2
    else h = (r - g) / d + 4
    h = Math.round(h * 60)
    if (h < 0) h += 360
  }
  const light = (mx + mn) / 2
  const s = d > 0 ? d / (1 - Math.abs(2 * light - 1)) : 0
  return { h, s }
}

/** CSS hsl(h[0-360], s[0-1], l[0-100]) → opaque RGB (IconColorTreatment.HslToRgb). */
export function hslToRgb(h: number, s: number, lPercent: number): Rgba {
  const l = lPercent / 100
  const sat = Math.min(1, Math.max(0, s))
  const c = (1 - Math.abs(2 * l - 1)) * sat
  const hp = (((h % 360) + 360) % 360) / 60
  const x = c * (1 - Math.abs((hp % 2) - 1))
  let r = 0
  let g = 0
  let b = 0
  switch (Math.floor(hp) % 6) {
    case 0: r = c; g = x; break
    case 1: r = x; g = c; break
    case 2: g = c; b = x; break
    case 3: g = x; b = c; break
    case 4: r = x; b = c; break
    default: r = c; b = x; break
  }
  const m = l - c / 2
  return { r: clampByte((r + m) * 255), g: clampByte((g + m) * 255), b: clampByte((b + m) * 255), a: 255 }
}

// ---- OKLab (private math the ramp + wallpaper tones share) ----

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

/** Perceived lightness (OKLab L) of 0-255 channels, alpha ignored. */
export function perceivedLightness(r: number, g: number, b: number): number {
  return toOkLab(r, g, b).L
}

// ---- 单色: Material-style tonal duotone (IconColorTreatment ramp) ----

const rampCache = new Map<number, Uint8ClampedArray>()

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

/** A pale, near-white tone of the tint's hue (glass / relief plate base). */
export function paleTone(tint: number): Rgba {
  return monoTone(0.965, 0.5, tint)
}

// ---- 满彩 Field harmony band (ADR-0016 D1; recipe v2, owner rejection of the
// v1 knockout-first fill 2026-07-10: "很多 icon 根本认不出" — the artwork is
// PRESERVED and the plate contrasts it in LIGHTNESS within the icon's own hue,
// the proven themed-pack recipe. A saturated fill under same-hue artwork
// self-camouflages; flattening subjects to ink destroys recognizability.) ----

export type FieldBand = 'Vivid' | 'Quiet'

// v7 (designer acceptance round 2026-07-10): ONE light line per band; the
// plate must actually CARRY colour (v6's C≤0.065 read as a white board —
// FAIL item 1). At L0.87 the gamut caps blues near C≈0.08 while warm hues
// reach 0.12 — the designer counts that spread as natural hue separation.
const FIELD_SLOTS: Record<FieldBand, { L: number; cMin: number; cMax: number }> = {
  Vivid: { L: 0.87, cMin: 0.09, cMax: 0.12 },
  Quiet: { L: 0.91, cMin: 0.04, cMax: 0.07 },
}

/** Plated anchors live in the light field too (designer FAIL item 2:
 *  [0.42,0.78] produced dark islands); near-neutral plates (white Office
 *  boards, C<0.04) are EXEMPT so white stays white (law 1/4). */
const PLATED_L_MIN = 0.6
const PLATED_L_MAX = 0.8
const PLATED_NEUTRAL_CHROMA = 0.04

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

/**
 * The Field plate for a seed colour: the seed's HUE in the band's shared
 * lightness line — lightness normalised desktop-wide (tidy as a set), hue and
 * gamut-limited chroma per icon (colour pop-out). Gamut-fit like monoTone.
 */
export function fieldPlateTone(seed: Rgba, band: FieldBand, chromaWindow?: { min: number; max: number }): Rgba {
  const slot = FIELD_SLOTS[band]
  const cMin = chromaWindow?.min ?? slot.cMin
  const cMax = chromaWindow?.max ?? slot.cMax
  const { ua, ub, chroma } = hueUnit(seed)
  const c = Math.min(cMax, Math.max(cMin, chroma))
  return gamutFit(slot.L, ua, ub, c)
}

/** A plated source keeps its OWN plate colour; chromatic plates are clamped
 *  into the light window, near-neutral plates pass untouched (white stays
 *  white — law 1/4). Hue + chroma preserved, gamut-fit. */
export function clampPlateLightness(bg: Rgba): Rgba {
  const { ua, ub, chroma, L } = hueUnit(bg)
  if (chroma < PLATED_NEUTRAL_CHROMA) return { ...bg, a: 255 }
  if (L >= PLATED_L_MIN && L <= PLATED_L_MAX) return { ...bg, a: 255 }
  const target = Math.min(PLATED_L_MAX, Math.max(PLATED_L_MIN, L))
  return gamutFit(target, ua, ub, chroma)
}

/**
 * The NEUTRAL contrast plate for artwork with no clear theme colour (owner
 * law 2026-07-10: grayscale icons get a pure LIGHTNESS-contrast board — white
 * subjects ride a darkish plate, dark subjects a light one, and WHITE is a
 * legal plate again; never force a hue onto gray art).
 */
/** Only subjects that genuinely read LIGHT take a dark board; everything at
 *  or below mid lightness reads "dark-ish" to the eye and gets the bright
 *  board (owner 2026-07-10: 「主体都偏暗了，你还搞个暗色背景给他？」). */
const DARK_BOARD_SUBJECT_MIN_L = 0.7

export function neutralContrastTone(subjectMeanL: number): Rgba {
  const L =
    subjectMeanL >= DARK_BOARD_SUBJECT_MIN_L
      ? Math.min(0.42, Math.max(0.2, subjectMeanL - 0.45))
      : Math.min(0.97, Math.max(0.82, subjectMeanL + 0.45))
  return gamutFit(L, 0, 0, 0)
}

/**
 * The THEMED contrast plate (owner formula 2026-07-10): the subject keeps its
 * colours untouched; the plate takes the THEME HUE at whichever lightness
 * side sits further from the subject's mean — strong contrast always, ties
 * lean light. One formula drives every derived plate.
 */
/** Deep boards must still CARRY their hue: below this FITTED chroma a deep
 *  plate reads as mud, not as a dark version of the colour (designer v19:
 *  deep-yellow boards rendered at C≈0.053 — olive, not 深黄). */
const DEEP_MIN_CHROMA = 0.09
/** L may rise to ~0.42 chasing chroma — sRGB physically cannot hold C 0.09
 *  at L 0.30 near amber, so richness wins over absolute depth (still well
 *  below the 0.7 board/subject divide). */
const DEEP_MAX_LIFT = 0.12

export function themedContrastTone(seed: Rgba, subjectMeanL: number, band: FieldBand): Rgba {
  let { ua, ub } = hueUnit(seed)
  const chroma = hueUnit(seed).chroma
  const lightL = band === 'Quiet' ? 0.91 : 0.87
  const darkL = band === 'Quiet' ? 0.34 : 0.3
  const useDark = subjectMeanL >= DARK_BOARD_SUBJECT_MIN_L
  if (!useDark) return gamutFit(lightL, ua, ub, Math.min(0.1, Math.max(0.06, chroma)))

  // Deep boards. sRGB has no dark saturated yellow-greens — that hue's dark
  // side IS olive. Pull the zone toward amber (深金, never 军绿)…
  let deg = (Math.atan2(ub, ua) * 180) / Math.PI
  if (deg > 82 && deg < 125) {
    const rad = ((78 + (deg - 82) * 0.15) * Math.PI) / 180
    ua = Math.cos(rad)
    ub = Math.sin(rad)
  }
  // …and buy chroma headroom by lifting L until the FITTED plate keeps its
  // colour (the eye grades the rendered value, not the token).
  const cReq = Math.min(0.12, Math.max(0.06, chroma))
  let L = darkL
  let plate = gamutFit(L, ua, ub, cReq)
  while (cReq >= DEEP_MIN_CHROMA && L < darkL + DEEP_MAX_LIFT) {
    const lab = toOkLab(plate.r, plate.g, plate.b)
    if (Math.sqrt(lab.A * lab.A + lab.B * lab.B) >= DEEP_MIN_CHROMA) break
    L += 0.02
    plate = gamutFit(L, ua, ub, cReq)
  }
  return plate
}

/** The silhouette-shadow tone for a Field plate, always OPPOSING the plate:
 *  light plates take a deep same-hue shadow, DARK plates take a light glow —
 *  a dark shadow on a dark board is invisible and lifts nothing (owner:
 *  the subject must clearly stand out). Never recolours the subject. */
export function fieldShadowTone(plate: Rgba): Rgba {
  const { ua, ub, L } = hueUnit(plate)
  return L < 0.5 ? gamutFit(0.92, ua, ub, 0.03) : gamutFit(0.38, ua, ub, 0.05)
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

/** The colour with its OKLab lightness shifted by `dL` (hue/chroma kept,
 *  gamut-fit) — kind affordances (folder tab, dog-ear) shade the PLATE's own
 *  colour, never introduce a foreign one. */
export function shiftLightness(c: Rgba, dL: number): Rgba {
  const { ua, ub, chroma, L } = hueUnit(c)
  return gamutFit(Math.min(0.98, Math.max(0.05, L + dL)), ua, ub, chroma)
}

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

/**
 * Per-pixel ADAPTIVE-stretched lightness t∈[0,1]: the tile's visible P5-P95
 * range remapped to full scale (polarity preserved; near-flat tiles pass
 * through). Transparent pixels get 0 (StretchedLightness).
 */
export function stretchedLightness(tile: Raster): Float64Array {
  const d = tile.data
  const n = tile.width * tile.height
  const hist = new Uint32Array(256)
  const light = new Uint8Array(n)
  const result = new Float64Array(n)
  let visible = 0
  for (let i = 0; i < n; i++) {
    const i4 = i * 4
    if (d[i4 + 3] === 0) continue
    const v = clampByte(perceivedLightness(d[i4], d[i4 + 1], d[i4 + 2]) * 255)
    light[i] = v
    hist[v]++
    visible++
  }
  if (visible === 0) return result

  const percentile = (p: number) => {
    const target = visible * p
    let cum = 0
    for (let v = 0; v < 256; v++) {
      cum += hist[v]
      if (cum >= target) return v
    }
    return 255
  }
  const lo = percentile(0.05)
  const hi = percentile(0.95)
  const span = hi - lo
  const stretch = span >= 26
  for (let i = 0; i < n; i++) {
    if (d[i * 4 + 3] === 0) continue
    result[i] = stretch ? Math.min(1, Math.max(0, (light[i] - lo) / span)) : light[i] / 255
  }
  return result
}

/** Whole-tile adaptive 单色 mapping (MonoMapAdaptive). */
export function monoMapAdaptive(tile: Raster, tint: number): void {
  const t = stretchedLightness(tile)
  const d = tile.data
  let lut = rampCache.get(tint)
  if (!lut) {
    lut = buildRamp(tint)
    rampCache.set(tint, lut)
  }
  for (let i = 0; i < t.length; i++) {
    const i4 = i * 4
    if (d[i4 + 3] === 0) continue
    const li = Math.min(255, Math.max(0, Math.round(t[i] * 255)))
    d[i4] = lut[li * 3]
    d[i4 + 1] = lut[li * 3 + 1]
    d[i4 + 2] = lut[li * 3 + 2]
  }
}

/**
 * Per-pixel recolour for 黑白 (TransformPixel; 原彩 is identity and 单色 goes
 * through monoMapAdaptive at the tile level — this covers the BW branch and the
 * per-pixel mono used by the original/peek card).
 */
export function transformPixelInPlace(d: Uint8ClampedArray, i4: number, mode: Subject, tint: number): void {
  if (mode === 'Original' || d[i4 + 3] === 0) return
  if (mode === 'BlackWhite') {
    const l = luminance(d[i4], d[i4 + 1], d[i4 + 2])
    const v = grayValue(l)
    d[i4] = v
    d[i4 + 1] = v
    d[i4 + 2] = v
    return
  }
  const t = perceivedLightness(d[i4], d[i4 + 1], d[i4 + 2])
  const toned = monoRamp(t, tint)
  d[i4] = toned.r
  d[i4 + 1] = toned.g
  d[i4 + 2] = toned.b
}
