import type { ZoneDto, ZoneMaterial, ZoneTitleStyle } from '@/bridge/types'
import type { Oklch } from './oklch'
import { hexToOklch, oklchToHex } from './oklch'
import type { PanelTextureKind } from './panel-textures'
import type { RegionSample } from './sampling'

// Zone materials (spec 04 §4.1 round 3, 2026-07-15). ONE adaptive base —
// per-zone OKLCH sampling + tone decision — feeding six finishes that each own
// a NAMEABLE axis: 描边 Outline (contour) · 磨砂玻璃 Frost (blurred glass,
// default) · 流体玻璃 LiquidGlass (physical refraction) · 棱纹玻璃 Fluted
// (vertical fluted glass — light-band texture over frost) · 素笺 Paper (warm
// matte paper — grain + letterpress) · 拉丝金属 Brushed (anisotropic brushed
// metal + diagonal sheen — near-opaque premium). Retired round 3: Luminous/
// Solid/Halo (one brightness axis, read as identical); owner cut Glaze (muddy
// accent×wallpaper mix) and Float (invisible) same day — texture beats
// translucency games (素笺 verdict). Pure recipes: the renderer and the bake
// path both consume this; nothing else invents colours. Binding record:
// docs/reviews/2026-07-15-zone-material-title-ux.md.

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

// Round 3 (2026-07-15): 0 = true square corners are a legitimate style; 60
// matches the liquid-glass demo's best look. zone-node additionally guards
// radius ≤ shortestSide/2 at render time so small zones never turn pill.
export const CORNER_MIN = 0
export const CORNER_MAX = 60

/** Frost blur sigma relative to the icon cell (σ = cellHeight/6). */
export const BLUR_SIGMA_PER_CELL = 1 / 6

/** Default fill alpha per material × tone (fillOpacity overrides; null = use
 *  these). Frost raised 2026-07-09 (owner: high-contrast wallpapers bled
 *  through 0.6). */
export const OPACITY_DEFAULTS: Record<ZoneMaterial, { Light: number; Dark: number }> = {
  Frost: { Light: 0.74, Dark: 0.76 },
  Outline: { Light: 0.05, Dark: 0.05 },
  // Glass paints NO fill — fillOpacity maps to the shader's Tint, default 0 =
  // pure refraction (owner 2026-07-15). Chip alpha rides its 0.55 floor.
  LiquidGlass: { Light: 0, Dark: 0 },
  Fluted: { Light: 0.5, Dark: 0.55 },
  Paper: { Light: 0.96, Dark: 0.94 },
  Brushed: { Light: 0.88, Dark: 0.9 },
}

/** Default corner radius per material (user slider 0–60 overrides). */
export const MATERIAL_RADIUS_DEFAULT: Record<ZoneMaterial, number> = {
  Frost: 20,
  Outline: 20,
  LiquidGlass: 44, // glass wears the most generous rounding (iOS 26 look, round 3)
  Fluted: 22,
  Paper: 20,
  Brushed: 18, // slightly harder corners — metal plate, not a cushion
}

/** Title styles a material supports. 无 None leads every list (hiding is a
 *  legitimate answer to "how is this zone labeled" — round 3). Outline has no
 *  visible body and Fluted no solid top edge — the full-width Bar seam would
 *  float orphaned on either. */
export function allowedTitleStyles(material: ZoneMaterial): ZoneTitleStyle[] {
  return material === 'Outline' || material === 'Fluted'
    ? ['None', 'Etched', 'Chip', 'Bare']
    : ['None', 'Etched', 'Chip', 'Bare', 'Bar']
}

/** Designer-recommended pairing applied when the user switches material
 *  (touched axes survive — see the inspector's switch semantics). */
export const MATERIAL_TITLE_DEFAULT: Record<ZoneMaterial, ZoneTitleStyle> = {
  Frost: 'Chip',
  Outline: 'Bare',
  LiquidGlass: 'Etched', // glass-native lozenge, not a sticker (round 3)
  Fluted: 'Etched', // translucent glass family shares the etched lozenge
  Paper: 'Bar', // editorial header on matte paper
  Brushed: 'Chip',
}

export interface ZonePaint {
  material: ZoneMaterial
  tone: 'Light' | 'Dark'
  /** Panel fill. */
  fill: { color: string; alpha: number }
  /** Top inner highlight; alpha 0 = none. */
  highlight: { color: string; alpha: number; width: number }
  /** 1px outer contour; alpha 0 = none. */
  contour: { color: string; alpha: number }
  /** Outline material ring (2px accent-tinted); null otherwise. */
  outlineRing: { color: string; alpha: number } | null
  /** Glaze accent inner glow (inset soft stroke); null otherwise. */
  innerGlow: { color: string; alpha: number; inset: number; blur: number } | null
  /** Baked drop shadow (投影 finish); null when off/unsupported. LiquidGlass is
   *  always null here — its shader draws the reference's own gaussian ring. */
  shadow: { offsetY: number; blur: number; alpha: number } | null
  /** Plate letterpress: 1px bottom inner dark line (top uses highlight);
   *  Paper + Brushed; null otherwise. */
  letterpressBottom: { color: string; alpha: number } | null
  /** Procedural tile overlay (panel-textures.ts): Paper grain / Fluted ribs /
   *  Brushed streaks. alpha = master alpha (rib/streak tiles carry their own
   *  per-pixel alpha and use 1). Null = none. */
  texture: { kind: PanelTextureKind; alpha: number } | null
  /** Brushed anisotropic sheen: one soft diagonal light band; null otherwise. */
  sheen: { color: string; alpha: number } | null
  /** Liquid Glass refraction (spec 04 §4.1, 2026-07-14) — a complete port of
   *  archisvaze/liquid-glass webgl.html. Bevel widths are FIXED desktop px, like
   *  a real pane's bevel (owner 2026-07-14): enlarging a zone grows its flat 1:1
   *  center, never the rim. Zone-node only scales them DOWN when a small zone
   *  can't fit the demo's proportions (bezel ≤30% / thickness ≤25% of min dim).
   *  Null for every non-glass material. */
  liquidGlass: {
    /** Optical slab thickness, desktop px. */
    thickness: number
    /** Curved-bezel dome width, desktop px. */
    bezel: number
    /** Index of refraction. */
    ior: number
    /** 16-tap Poisson backdrop blur radius, desktop px. */
    blur: number
    specular: number
    /** Mix toward white (reference "Tint"; fillOpacity drives it, default 0). */
    tint: number
    /** Outer gaussian shadow strength (reference "Shadow"; 投影 toggle). */
    shadow: number
  } | null
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

  // Panel hue follows the wallpaper (深度融合). Exceptions: Paper keeps its own
  // warm identity (hue ~78 — 素笺 is paper, not tinted glass); Brushed is a
  // warm-graphite metal that only faintly reflects the environment.
  const sampled = sample.c > 0.008
  const hue = sampled ? sample.h * 0.65 + accentLch.h * 0.35 : accentLch.h

  const chromaMax: Record<ZoneMaterial, number> = {
    Frost: 0.03,
    Outline: 0.03,
    LiquidGlass: 0.03,
    Fluted: 0.018, // near-neutral: the rib light-bands are the identity, never a color mix
    Paper: 0.02,
    Brushed: 0.01,
  }
  const chroma = clamp(Math.max(sample.c, 0.02) * 0.5, 0, chromaMax[material])

  const baseAlpha = zone.fillOpacity ?? OPACITY_DEFAULTS[material][tone]
  const densify = blurless && (material === 'Frost' || material === 'Fluted' || material === 'LiquidGlass')
  const alpha = clamp(baseAlpha + (densify ? 0.12 : 0), 0, 0.98)

  const fillLch: Oklch =
    material === 'Fluted'
      ? { l: light ? 0.9 : 0.24, c: chroma, h: hue }
      : material === 'Paper'
        ? { l: light ? 0.95 : 0.18, c: 0.018, h: 78 }
        : material === 'Brushed'
          ? { l: light ? 0.82 : 0.3, c: 0.01, h: sampled ? 75 * 0.7 + sample.h * 0.3 : 75 }
          : { l: light ? 0.92 : 0.2, c: chroma, h: hue }

  // Frost sigma: only the blur-borne finishes frost the wallpaper.
  const frosts = material === 'Frost' || material === 'Fluted'
  const blurSigma = frosts && !blurless ? cellHeight * BLUR_SIGMA_PER_CELL : 0

  const highlight =
    material === 'LiquidGlass' || material === 'Outline'
      ? { color: '#FFFFFF', alpha: 0, width: 0 } // glass: the shader's rim IS the edge
      : material === 'Paper'
        ? { color: '#FFFFFF', alpha: light ? 0.55 : 0.12, width: 1 } // letterpress top
        : material === 'Fluted'
          ? { color: '#FFFFFF', alpha: light ? 0.4 : 0.16, width: 1 } // glass top edge
          : material === 'Brushed'
            ? { color: '#FFFFFF', alpha: light ? 0.45 : 0.18, width: 1 } // metal bevel
            : { color: '#FFFFFF', alpha: light ? 0.35 : 0.14, width: 1 }

  const contour =
    material === 'Outline' || material === 'LiquidGlass'
      ? { color: '#000000', alpha: 0 }
      : light
        ? { color: '#000000', alpha: material === 'Paper' || material === 'Fluted' ? 0.08 : 0.1 }
        : { color: '#FFFFFF', alpha: material === 'Frost' || material === 'Fluted' || material === 'Brushed' ? 0.12 : 0.14 }

  // Chip carries the ACCENT; ink auto-inverts against it (≥4.5:1 by L gap).
  const chipFill: Oklch = light
    ? { l: 0.94, c: Math.min(0.055, accentLch.c * 0.45), h: accentLch.h }
    : { l: 0.2, c: Math.min(0.05, accentLch.c * 0.4), h: accentLch.h }
  const chipAlpha = clamp(alpha + (light ? 0.22 : 0.18), 0.55, 0.98)
  const ink: Oklch = light
    ? { l: 0.28, c: Math.min(0.06, accentLch.c * 0.6), h: accentLch.h }
    : { l: 0.97, c: 0.01, h: accentLch.h }

  // 投影 finish: real bodies via the toggle; Outline has no body; LiquidGlass's
  // shader owns its own gaussian ring.
  const shadow =
    zone.shadow && (material === 'Frost' || material === 'Fluted' || material === 'Paper' || material === 'Brushed')
      ? { offsetY: Math.round(cellHeight * 0.06), blur: cellHeight * 0.14, alpha: light ? 0.16 : 0.28 }
      : null

  const cornerRadius = clamp(zone.cornerRadius, CORNER_MIN, CORNER_MAX)

  // archisvaze/liquid-glass webgl.html defaults (bezel 60 / thickness 50). tint
  // maps the reference Tint slider: fillOpacity drives it over the FULL 0–1
  // range so "opacity" still means "milkier glass"; default 0 = pure refraction
  // (owner 2026-07-15). shadow follows the 投影 toggle.
  const liquidGlass =
    material === 'LiquidGlass'
      ? {
          thickness: 50,
          bezel: 60,
          ior: 3,
          blur: 1.5,
          specular: 0.55,
          tint: clamp(zone.fillOpacity ?? 0, 0, 1),
          shadow: zone.shadow ? 0.5 : 0,
        }
      : null

  return {
    material,
    tone,
    // LiquidGlass: the shader IS the panel (opaque refracted glass, reference
    // parity) — any fill painted over it fogs the refraction into "仿制品".
    fill: { color: oklchToHex(fillLch), alpha: material === 'LiquidGlass' ? 0 : alpha },
    highlight,
    contour,
    outlineRing:
      material === 'Outline'
        ? { color: oklchToHex({ l: 0.5, c: Math.min(accentLch.c, 0.1), h: accentLch.h }), alpha: 0.85 }
        : null,
    innerGlow: null, // hook retained for future finishes; no current user
    shadow,
    letterpressBottom:
      material === 'Paper'
        ? { color: '#000000', alpha: light ? 0.14 : 0.3 }
        : material === 'Brushed'
          ? { color: '#000000', alpha: 0.12 } // plate-thickness bottom edge
          : null,
    texture:
      material === 'Paper'
        ? { kind: 'noise' as const, alpha: 0.045 }
        : material === 'Fluted'
          ? { kind: 'flute' as const, alpha: 1 } // rib tile carries its own alpha
          : material === 'Brushed'
            ? { kind: 'brush' as const, alpha: 1 }
            : null,
    sheen:
      // Dark tone raised 0.14→0.2: the sheen is the anti-invisibility weapon —
      // on a dark panel it must still read as living metal light.
      material === 'Brushed' ? { color: '#FFFFFF', alpha: light ? 0.22 : 0.2 } : null,
    liquidGlass,
    blurSigma,
    cornerRadius,
    accent,
    chip: {
      fill: { color: oklchToHex(chipFill), alpha: chipAlpha },
      ink: { color: oklchToHex(ink), alpha: 0.96 },
      forced: material === 'Outline',
    },
  }
}
