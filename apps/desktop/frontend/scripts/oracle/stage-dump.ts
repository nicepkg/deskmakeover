// Per-source stage dumps for the M0b oracle corpus. Captures the outputs of the
// frozen analysis/profile stages (the classification decisions the Rust port
// must reproduce bit-for-bit) as machine-comparable JSON, plus the subject mask
// as a grayscale PNG. Every value comes from a public frozen function — no
// re-derivation — so the M5 stage-level differential can pin a break to one
// function. Source-only (config-independent): computed once, shared by looks.

import type { Raster, Rgba } from '@/icon-compositor/raster'
import {
  boundsH,
  boundsW,
  cornersSymmetric,
  findContentBounds,
  foregroundBounds,
  matchesShape,
  maxScaleInside,
  solidBounds,
  tryDetectBackground,
  visibleLightnessStats,
} from '@/icon-compositor/analysis'
import type { ContentBounds } from '@/icon-compositor/analysis'
import { iconProfile } from '@/icon-compositor/profile'
import { segmentSubject } from '@/icon-compositor/segment'
import { encodeGrayPng } from './png-codec'

function hex(c: Rgba | null): string | null {
  if (!c) return null
  const h = (v: number) => v.toString(16).padStart(2, '0')
  return `#${h(c.r)}${h(c.g)}${h(c.b)}`.toUpperCase()
}

function rect(b: ContentBounds | null): [number, number, number, number] | null {
  return b ? [b.left, b.top, b.right, b.bottom] : null
}

function round(n: number, dp = 6): number {
  const f = 10 ** dp
  return Math.round(n * f) / f
}

/** The machine-comparable profile of one source (all frozen-stage outputs). */
export interface StageProfile {
  /** Silhouette classification (profile.kind). */
  kind: 'fullSquare' | 'ownBoard' | 'bare'
  transparentEdges: boolean
  /** Alpha (>24) bounding box [l,t,r,b] and its coverage of the canvas. */
  alphaBBox: [number, number, number, number]
  coverage: number
  /** Solid (a>=128) silhouette bbox, or null. */
  solidBBox: [number, number, number, number] | null
  /** Own-background verdict + the anchor rect composeFromPlate expands from. */
  ownBackground: string | null
  ownBackgroundLightness: number | null
  anchorRect: [number, number, number, number]
  /** Corner-symmetry probe (the dog-eared-document discriminator). */
  cornerSymmetric: boolean
  /** Outermost rim band: majority colour + mean lightness (derived-plate seed). */
  rimColour: string | null
  rimLightness: number
  /** Subject dominant/theme colour + subject mean lightness. */
  subjectColour: string | null
  subjectLightness: number
  /** Foreground-logo bbox inside the plate, or null. */
  foregroundBBox: [number, number, number, number] | null
  /** Overall solid-pixel mean lightness (Field pale-class gate). */
  visibleLightness: number
  /** Which target shapes the silhouette already matches (parity of matchesShape). */
  matchesCircle: boolean
  matchesApple: boolean
  /** maxScaleInside for the two inscribe shapes that gate on it. */
  maxScaleCircle: number
  /** The decode-time hue-spread seed (iconProfile rim colour). */
  seed: string | null
  /** Subject-mask coverage fraction (mask solid / canvas). */
  maskCoverage: number
  /** Whether iconProfile keeps the mask (false for fullSquare — whole canvas). */
  profileKeepsMask: boolean
}

/** The profile JSON + subject-mask PNG for one source, sharing a single mask
 *  (reuses iconProfile's mask when present, segments fresh only for fullSquare). */
export function dumpSource(raster: Raster, seed: string | null): { profile: StageProfile; maskPng: Uint8Array } {
  const profile = iconProfile(raster)
  const content = findContentBounds(raster)
  const minDim = Math.min(boundsW(content), boundsH(content))
  const bg = tryDetectBackground(raster)
  const fg = bg ? foregroundBounds(raster, content, bg) : null
  const canvas = raster.width * raster.height
  const mask = profile.subjectMask ?? segmentSubject(raster).mask
  let maskSolid = 0
  for (let i = 0; i < mask.length; i++) maskSolid += mask[i]

  const stage: StageProfile = {
    kind: profile.kind,
    transparentEdges: profile.transparentEdges,
    alphaBBox: rect(content)!,
    coverage: round((boundsW(content) * boundsH(content)) / canvas),
    solidBBox: rect(solidBounds(raster)),
    ownBackground: hex(profile.background),
    ownBackgroundLightness: profile.backgroundLightness === null ? null : round(profile.backgroundLightness),
    anchorRect: rect(content)!,
    cornerSymmetric: cornersSymmetric(raster, content, minDim),
    rimColour: hex(profile.subjectRimColour),
    rimLightness: round(profile.subjectRimLightness),
    subjectColour: hex(profile.subjectColour),
    subjectLightness: round(profile.subjectLightness),
    foregroundBBox: rect(fg),
    visibleLightness: round(visibleLightnessStats(raster).mean),
    matchesCircle: matchesShape(raster, 'Circle'),
    matchesApple: matchesShape(raster, 'Apple'),
    maxScaleCircle: round(maxScaleInside(raster, content, 'Circle')),
    seed,
    maskCoverage: round(maskSolid / canvas),
    profileKeepsMask: profile.subjectMask !== null,
  }

  const gray = new Uint8Array(canvas)
  for (let i = 0; i < canvas; i++) gray[i] = mask[i] ? 255 : 0
  return { profile: stage, maskPng: encodeGrayPng(raster.width, raster.height, gray) }
}
