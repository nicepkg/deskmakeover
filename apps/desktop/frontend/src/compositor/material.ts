import type { ZoneDto, ZoneMaterial, ZoneTitleStyle } from '@/bridge/types'
import type { Oklch } from './oklch'
import { hexToOklch, oklchToHex } from './oklch'
import type { RegionSample } from './sampling'

// Zone materials (spec 04 §4.1, designer set 2026-07-09). ONE adaptive base —
// per-zone OKLCH sampling + tone decision — feeding five finishes: 磨砂玻璃
// Frost (default) · 晨光玻璃 Luminous · 实色卡片 Solid · 柔光晕影 Halo ·
// 描边卡片 Outline, plus a shared baked drop-shadow finish. Pure recipes: the
// renderer and the bake path both consume this; nothing else invents colours.

/** Curated per-zone accent palette (auto-assigned by zone order, overridable).
 *  All hues live outside the banned blue/violet band (tests/banned-colors). */
export const ACCENT_PALETTE = [
  '#D9973B', // 琥珀 amber
  '#7FA678', // 苔绿 sage
  '#4FA396', // 青瓷 celadon
  '#C96F4A', // 陶土 terracotta
  '#C56B85', // 玫瑰 rose
  '#8C8578', // 暖灰 warm slate
] as const

export const CORNER_MIN = 8
export const CORNER_MAX = 28

/** Frost blur sigma relative to the icon cell (σ = cellHeight/6). */
export const BLUR_SIGMA_PER_CELL = 1 / 6

/** Default fill alpha per material × tone (fillOpacity overrides). Frost/Luminous
 *  raised 2026-07-09 (owner: a high-contrast wallpaper — a dark mountain under
 *  the zone — bled through the old 0.6 fill as an ugly dark gradient on one
 *  side; a denser frost reads as a calm panel while keeping translucency). */
const OPACITY_DEFAULTS: Record<ZoneMaterial, { Light: number; Dark: number }> = {
  Frost: { Light: 0.74, Dark: 0.76 },
  Luminous: { Light: 0.74, Dark: 0.72 }, // midpoint of the gradient stops
  Solid: { Light: 0.94, Dark: 0.92 },
  Halo: { Light: 0.55, Dark: 0.55 },
  Outline: { Light: 0.05, Dark: 0.05 },
}

/** Default corner radius per material (user slider 8–28 overrides). */
export const MATERIAL_RADIUS_DEFAULT: Record<ZoneMaterial, number> = {
  Frost: 20,
  Luminous: 24,
  Solid: 20,
  Halo: 24,
  Outline: 20,
}

/** Title styles a material supports (combo matrix): Halo/Outline have no solid
 *  top edge / visible body — Tab and Bar would float orphaned. */
export function allowedTitleStyles(material: ZoneMaterial): ZoneTitleStyle[] {
  return material === 'Halo' || material === 'Outline' ? ['Chip', 'Bare'] : ['Chip', 'Bare', 'Tab', 'Bar']
}

/** Designer-recommended pairing applied when the user switches material. */
export const MATERIAL_TITLE_DEFAULT: Record<ZoneMaterial, ZoneTitleStyle> = {
  Frost: 'Chip',
  Luminous: 'Chip',
  Solid: 'Tab',
  Halo: 'Bare',
  Outline: 'Bare',
}

export interface ZonePaint {
  material: ZoneMaterial
  tone: 'Light' | 'Dark'
  /** Panel fill. For Luminous this is the gradient midpoint (see gradient). */
  fill: { color: string; alpha: number }
  /** Luminous only: vertical two-stop gradient (top → bottom). */
  gradient: { top: { color: string; alpha: number }; bottom: { color: string; alpha: number } } | null
  /** Halo only: edge feather sigma in desktop px (alpha-mask gaussian). */
  featherSigma: number
  /** Top inner highlight; alpha 0 = none. */
  highlight: { color: string; alpha: number; width: number }
  /** 1px outer contour; alpha 0 = none. */
  contour: { color: string; alpha: number }
  /** Outline material ring (2px accent-tinted); null otherwise. */
  outlineRing: { color: string; alpha: number } | null
  /** Luminous accent inner glow (inset soft stroke); null otherwise. */
  innerGlow: { color: string; alpha: number; inset: number; blur: number } | null
  /** Baked drop shadow (投影 finish); null when off/unsupported. */
  shadow: { offsetY: number; blur: number; alpha: number } | null
  /** Wallpaper frost sigma under the panel; 0 = none. */
  blurSigma: number
  cornerRadius: number
  accent: string
  chip: {
    fill: { color: string; alpha: number }
    ink: { color: string; alpha: number }
    forced: boolean
  }
}

const clamp = (v: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, v))

/** Resolve the effective accent for a zone (explicit wins, else palette by index). */
export function resolveAccent(zone: ZoneDto, index: number): string {
  return zone.accent ?? ACCENT_PALETTE[index % ACCENT_PALETTE.length]
}

export interface PaintOptions {
  zone: ZoneDto
  index: number
  sample: RegionSample
  tone: 'Light' | 'Dark'
  cellHeight: number
  /** Blur-less tier (video overlay / weak GPU): no wallpaper frost, denser fill. */
  blurless?: boolean
}

export function zonePaint({ zone, index, sample, tone, cellHeight, blurless = false }: PaintOptions): ZonePaint {
  const accent = resolveAccent(zone, index)
  const accentLch = hexToOklch(accent)
  const material = zone.material
  const light = tone === 'Light'

  // Panel hue follows the wallpaper (深度融合); Halo leans accent instead
  // (no chip surface — the glow itself carries a hint of identity).
  const sampled = sample.c > 0.008
  const hue = material === 'Halo'
    ? (sampled ? accentLch.h * 0.6 + sample.h * 0.4 : accentLch.h)
    : (sampled ? sample.h * 0.65 + accentLch.h * 0.35 : accentLch.h)

  const chromaCap: Record<ZoneMaterial, number> = { Frost: 0.5, Luminous: 0.4, Solid: 0.35, Halo: 0.4, Outline: 0.5 }
  const chromaMax: Record<ZoneMaterial, number> = { Frost: 0.03, Luminous: 0.028, Solid: 0.022, Halo: 0.025, Outline: 0.03 }
  const chroma = clamp(Math.max(sample.c, 0.02) * chromaCap[material], 0, chromaMax[material])

  const baseAlpha = zone.fillOpacity ?? OPACITY_DEFAULTS[material][tone]
  const densify = blurless && (material === 'Frost' || material === 'Luminous' || material === 'Halo')
  const alpha = clamp(baseAlpha + (densify ? (material === 'Halo' ? 0.1 : material === 'Luminous' ? 0.1 : 0.12) : 0), 0, 0.98)

  const fillL: Record<ZoneMaterial, [number, number]> = {
    Frost: [0.92, 0.2],
    Luminous: [0.935, 0.21], // gradient midpoint
    Solid: [0.96, 0.17],
    Halo: [0.93, 0.18],
    Outline: [0.92, 0.2],
  }
  const fillLch: Oklch = { l: light ? fillL[material][0] : fillL[material][1], c: chroma, h: hue }

  // Frost sigma: Solid/Outline never blur; blur-less tier kills it everywhere.
  const frosts = material === 'Frost' || material === 'Luminous' || material === 'Halo'
  const blurSigma = frosts && !blurless ? cellHeight * BLUR_SIGMA_PER_CELL : 0

  // Luminous vertical gradient (top lighter/airier than bottom).
  const gradient = material === 'Luminous'
    ? {
        top: { color: oklchToHex({ l: light ? 0.97 : 0.26, c: chroma, h: hue }), alpha: clamp(alpha - 0.02, 0, 0.98) },
        bottom: { color: oklchToHex({ l: light ? 0.9 : 0.16, c: chroma, h: hue }), alpha: clamp(alpha + 0.02, 0, 0.98) },
      }
    : null

  const highlight =
    material === 'Halo'
      ? { color: '#FFFFFF', alpha: 0, width: 0 }
      : material === 'Luminous'
        ? { color: '#FFFFFF', alpha: light ? 0.5 : 0.2, width: 1.5 }
        : material === 'Solid'
          ? { color: '#FFFFFF', alpha: light ? 0.5 : 0.1, width: 1 }
          : material === 'Outline'
            ? { color: '#FFFFFF', alpha: 0, width: 0 }
            : { color: '#FFFFFF', alpha: light ? 0.35 : 0.14, width: 1 }

  const contour =
    material === 'Halo' || material === 'Outline'
      ? { color: '#000000', alpha: 0 }
      : light
        ? { color: '#000000', alpha: material === 'Luminous' || material === 'Solid' ? 0.08 : 0.1 }
        : { color: '#FFFFFF', alpha: material === 'Frost' ? 0.12 : 0.14 }

  // Chip carries the ACCENT; ink auto-inverts against it (≥4.5:1 by L gap).
  const chipFill: Oklch = light
    ? { l: 0.94, c: Math.min(0.055, accentLch.c * 0.45), h: accentLch.h }
    : { l: 0.2, c: Math.min(0.05, accentLch.c * 0.4), h: accentLch.h }
  const chipAlpha = clamp(alpha + (light ? 0.22 : 0.18), 0.55, 0.98)
  const ink: Oklch = light
    ? { l: 0.28, c: Math.min(0.06, accentLch.c * 0.6), h: accentLch.h }
    : { l: 0.97, c: 0.01, h: accentLch.h }

  // 投影 finish: real bodies only (Halo feathers, Outline has no body).
  const shadowOk = zone.shadow && (material === 'Frost' || material === 'Luminous' || material === 'Solid')

  return {
    material,
    tone,
    fill: { color: oklchToHex(fillLch), alpha },
    gradient,
    featherSigma: material === 'Halo' ? cellHeight * 0.45 : 0,
    highlight,
    contour,
    outlineRing:
      material === 'Outline'
        ? { color: oklchToHex({ l: 0.5, c: Math.min(accentLch.c, 0.1), h: accentLch.h }), alpha: 0.85 }
        : null,
    innerGlow:
      material === 'Luminous'
        ? { color: oklchToHex({ l: 0.6, c: Math.min(accentLch.c, 0.09), h: accentLch.h }), alpha: 0.25, inset: 3, blur: 3 }
        : null,
    shadow: shadowOk
      ? { offsetY: Math.round(cellHeight * 0.06), blur: cellHeight * 0.14, alpha: light ? 0.16 : 0.28 }
      : null,
    blurSigma,
    cornerRadius: clamp(zone.cornerRadius, CORNER_MIN, CORNER_MAX),
    accent,
    chip: {
      fill: { color: oklchToHex(chipFill), alpha: chipAlpha },
      ink: { color: oklchToHex(ink), alpha: 0.96 },
      forced: material === 'Outline',
    },
  }
}
