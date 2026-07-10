import type { Raster, Rgba } from './raster'
import { analysis, boundsH, boundsW, dominantColor } from './analysis'
import { perceivedLightness } from './color'
import { segmentSubject } from './segment'

// The ONE per-icon metadata extraction (owner architecture order 2026-07-10):
// every icon yields the same profile — classification, own background colour
// and lightness, subject/theme colour and lightness, subject mask — computed
// once per source raster and consumed by every downstream stage (plates,
// shadows, future auto-format/marks). DRY: no stage re-derives these.

/** Owner five-step classification. */
export type IconProfileKind =
  /** A full-bleed opaque square: the icon IS the tile — no background needed. */
  | 'fullSquare'
  /** Square/rounded/circle silhouette with a uniform outer ring: its own board. */
  | 'ownBoard'
  /** Everything irregular: subject only, background is OURS to add. */
  | 'bare'

export interface IconProfile {
  kind: IconProfileKind
  /** True when the canvas edge is see-through (free-floating artwork). */
  transparentEdges: boolean
  /** The icon's OWN background colour (ownBoard only). */
  background: Rgba | null
  backgroundLightness: number | null
  /** Theme colour: neighbour-hue merged majority of the SUBJECT (null = grayscale). */
  subjectColour: Rgba | null
  /** Mean perceived lightness of the subject's solid pixels. */
  subjectLightness: number
  /** Subject mask (null for fullSquare — the whole canvas is the subject). */
  subjectMask: Uint8Array | null
  /** The artwork's OUTERMOST BAND (owner 2026-07-10: this ring — not the
   *  subject's interior — decides whether the icon separates from the plate):
   *  its MAJORITY colour (null = neutral ring) and mean lightness. Derived
   *  plates take THIS hue, pushed light when the rim is dark and dark when
   *  the rim is light (淡黄/深黄 for a yellow ring). */
  subjectRimColour: Rgba | null
  subjectRimLightness: number
}

const FULL_SQUARE_MIN_COVERAGE = 0.98

const cache = new WeakMap<Raster, IconProfile>()

function maskMeanLightness(c: Raster, mask: Uint8Array | null): number {
  const d = c.data
  let sum = 0
  let n = 0
  const total = c.width * c.height
  for (let i = 0; i < total; i++) {
    if (mask && !mask[i]) continue
    const i4 = i * 4
    if (d[i4 + 3] < 128) continue
    sum += perceivedLightness(d[i4], d[i4 + 1], d[i4 + 2])
    n++
  }
  return n === 0 ? 0.5 : sum / n
}

/** Rim pixels must be FULLY solid — anti-aliased fringes and soft drop
 *  shadows (blended alpha) lied about the rim's colour and lightness when
 *  a 1px alpha≥128 boundary ring was used (owner 2026-07-10: dark-rimmed
 *  icons were reading as light and got dark plates). */
const RIM_SOLID_MIN_ALPHA = 245
/** The rim is a BAND, not a 1px ring: deep enough to out-vote thin
 *  highlight outlines, shallow enough to stay "the outermost ring" the
 *  eye sees (minDim/16 ≈ 6%). */
const RIM_BAND_MIN_DEPTH = 2
const RIM_BAND_DEPTH_DIVISOR = 16

/** The artwork's outermost BAND: what actually borders the plate. NOT
 *  mask-aware — the whole opaque artwork's edge touches the plate, even
 *  when segmentation calls part of it non-subject (GitHub's dark disc).
 *  Colour = the band's MAJORITY hue (owner: 占比最多 — Explorer's yellow
 *  ring must win over its small blue accents), via the same neighbour-hue
 *  merged majority as the theme colour; null = neutral band. */
function subjectRim(c: Raster): { colour: Rgba | null; lightness: number } {
  const d = c.data
  const W = c.width
  const H = c.height
  const N = W * H
  const depth = Math.max(RIM_BAND_MIN_DEPTH, Math.round(Math.min(W, H) / RIM_BAND_DEPTH_DIVISOR))
  const band = new Uint8Array(N)
  for (const minAlpha of [RIM_SOLID_MIN_ALPHA, 128]) {
    let cur = new Uint8Array(N)
    for (let i = 0; i < N; i++) cur[i] = d[i * 4 + 3] >= minAlpha ? 1 : 0
    for (let pass = 0; pass < depth; pass++) {
      const next = cur.slice()
      for (let y = 0; y < H; y++) {
        for (let x = 0; x < W; x++) {
          const i = y * W + x
          if (!cur[i]) continue
          const interior =
            x > 0 && cur[i - 1] && x < W - 1 && cur[i + 1] && y > 0 && cur[i - W] && y < H - 1 && cur[i + W]
          if (!interior) {
            band[i] = 1
            next[i] = 0
          }
        }
      }
      cur = next
    }
    let sumL = 0
    let n = 0
    for (let i = 0; i < N; i++) {
      if (!band[i]) continue
      const i4 = i * 4
      sumL += perceivedLightness(d[i4], d[i4 + 1], d[i4 + 2])
      n++
    }
    // All-soft artwork (nothing fully solid): retry once at alpha>=128.
    if (n === 0) continue
    const colour = dominantColor(c, band)?.colour ?? null
    return { colour: colour ? { ...colour, a: 255 } : null, lightness: sumL / n }
  }
  return { colour: null, lightness: 0.5 }
}

export function iconProfile(c: Raster): IconProfile {
  const hit = cache.get(c)
  if (hit) return hit

  const transparentEdges = analysis.hasTransparentEdges(c)
  const bounds = analysis.contentBounds(c)
  const coverage = (boundsW(bounds) * boundsH(bounds)) / (c.width * c.height)

  let profile: IconProfile
  if (!transparentEdges && coverage >= FULL_SQUARE_MIN_COVERAGE) {
    // Step 1: a filled standard square is a complete subject by itself.
    const rim = subjectRim(c)
    profile = {
      kind: 'fullSquare',
      transparentEdges,
      background: null,
      backgroundLightness: null,
      subjectColour: analysis.dominantColor(c, null)?.colour ?? null,
      subjectLightness: maskMeanLightness(c, null),
      subjectMask: null,
      subjectRimColour: rim.colour,
      subjectRimLightness: rim.lightness,
    }
  } else {
    const background = analysis.tryDetectBackground(c)
    const mask = segmentSubject(c).mask
    const subjectColour = analysis.dominantColor(c, mask)?.colour ?? null
    const subjectLightness = maskMeanLightness(c, mask)
    const rim = subjectRim(c)
    profile = background
      ? {
          kind: 'ownBoard',
          transparentEdges,
          background: { ...background, a: 255 },
          backgroundLightness: perceivedLightness(background.r, background.g, background.b),
          subjectColour,
          subjectLightness,
          subjectMask: mask,
          subjectRimColour: rim.colour,
          subjectRimLightness: rim.lightness,
        }
      : {
          kind: 'bare',
          transparentEdges,
          background: null,
          backgroundLightness: null,
          subjectColour,
          subjectLightness,
          subjectMask: mask,
          subjectRimColour: rim.colour,
          subjectRimLightness: rim.lightness,
        }
  }
  cache.set(c, profile)
  return profile
}
