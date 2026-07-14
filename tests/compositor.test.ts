import { describe, expect, test } from 'bun:test'
import { hexToOklch, oklchToHex, rgbToOklch } from '../src/compositor/oklch'
import { buildSampleBuffer, resolveTone, sampleRegion, TONE_THRESHOLD } from '../src/compositor/sampling'
import { ACCENT_PALETTE, resolveAccent, zonePaint } from '../src/compositor/material'
import { titleFontPx, titleLayout } from '../src/compositor/title-chip'
import { makeZone } from '../src/stores/wallpaper'

// Pure-layer tests for the Adaptive Frost compositor (spec 04 §7 computed
// acceptance): tone auto-selection, accent distinctness, outline→chip forcing,
// chip ink contrast, overhang lane fallback.

function flat(rgb: [number, number, number], w = 8, h = 8): Uint8ClampedArray {
  const data = new Uint8ClampedArray(w * h * 4)
  for (let i = 0; i < w * h; i++) {
    data[i * 4] = rgb[0]
    data[i * 4 + 1] = rgb[1]
    data[i * 4 + 2] = rgb[2]
    data[i * 4 + 3] = 255
  }
  return data
}

const CELL = 92

describe('oklch conversions', () => {
  test('roundtrips within 1/255 per channel', () => {
    for (const hex of ['#FF6F5E', '#101217', '#F6F5F2', '#7FA678']) {
      expect(oklchToHex(hexToOklch(hex))).toBe(hex)
    }
  })

  test('white is L≈1, black is L≈0', () => {
    expect(rgbToOklch(1, 1, 1).l).toBeCloseTo(1, 1)
    expect(rgbToOklch(0, 0, 0).l).toBeCloseTo(0, 1)
  })
})

describe('sampling + tone (spec 04 §4.1)', () => {
  test('pale wallpaper region reads Light, dark reads Dark', () => {
    const pale = buildSampleBuffer(flat([238, 233, 225]), 8, 8)
    const dark = buildSampleBuffer(flat([28, 30, 38]), 8, 8)
    expect(resolveTone('Auto', sampleRegion(pale, 0, 0, 1, 1), null)).toBe('Light')
    expect(resolveTone('Auto', sampleRegion(dark, 0, 0, 1, 1), null)).toBe('Dark')
  })

  test('hysteresis: a borderline sample never strobes the previous tone', () => {
    const border = { l: TONE_THRESHOLD + 0.01, c: 0.02, h: 60 }
    expect(resolveTone('Auto', border, 'Dark')).toBe('Dark') // within the band → hold
    expect(resolveTone('Auto', { ...border, l: TONE_THRESHOLD + 0.06 }, 'Dark')).toBe('Light')
  })

  test('explicit tone overrides sampling', () => {
    const pale = sampleRegion(buildSampleBuffer(flat([240, 240, 240]), 8, 8), 0, 0, 1, 1)
    expect(resolveTone('Dark', pale, null)).toBe('Dark')
  })
})

describe('Adaptive Frost material (spec 04 §4.1)', () => {
  const sample = { l: 0.7, c: 0.05, h: 70 }

  test('accents auto-assign DISTINCT palette entries by zone order', () => {
    const zones = [0, 1, 2, 3].map((i) => makeZone({ cellX: i, cellY: 0, cellsWide: 2, cellsTall: 2, title: 't' }))
    const accents = zones.map((z, i) => resolveAccent(z, i))
    expect(new Set(accents).size).toBe(4)
  })

  test('explicit accent wins over the palette', () => {
    const z = makeZone({ cellX: 0, cellY: 0, cellsWide: 2, cellsTall: 2, title: 't', accent: '#C96F4A' })
    expect(resolveAccent(z, 3)).toBe('#C96F4A')
  })

  test('Outline material: near-transparent fill + accent ring + forced chip + no blur', () => {
    const z = makeZone({ cellX: 0, cellY: 0, cellsWide: 4, cellsTall: 4, title: 't', material: 'Outline' })
    const p = zonePaint({ zone: z, index: 0, sample, tone: 'Light', cellHeight: CELL })
    expect(p.fill.alpha).toBeLessThanOrEqual(0.05)
    expect(p.outlineRing).not.toBeNull()
    expect(p.chip.forced).toBe(true)
    expect(p.blurSigma).toBe(0)
  })

  test('frost sigma follows the cell (σ = cell/6)', () => {
    const z = makeZone({ cellX: 0, cellY: 0, cellsWide: 4, cellsTall: 4, title: 't' })
    const p = zonePaint({ zone: z, index: 0, sample, tone: 'Light', cellHeight: CELL })
    expect(p.blurSigma).toBeCloseTo(CELL / 6, 5)
  })

  test('blur-less tier trades blur for density (+0.12 alpha)', () => {
    const z = makeZone({ cellX: 0, cellY: 0, cellsWide: 4, cellsTall: 4, title: 't' })
    const base = zonePaint({ zone: z, index: 0, sample, tone: 'Light', cellHeight: CELL })
    const flatTier = zonePaint({ zone: z, index: 0, sample, tone: 'Light', cellHeight: CELL, blurless: true })
    expect(flatTier.blurSigma).toBe(0)
    expect(flatTier.fill.alpha).toBeCloseTo(base.fill.alpha + 0.12, 5)
  })

  test('chip ink contrast ≥ 4.5:1 against the chip fill in BOTH tones (spec §7)', () => {
    const z = makeZone({ cellX: 0, cellY: 0, cellsWide: 4, cellsTall: 4, title: 't' })
    for (const tone of ['Light', 'Dark'] as const) {
      const p = zonePaint({ zone: z, index: 0, sample, tone, cellHeight: CELL })
      expect(wcagContrast(p.chip.ink.color, p.chip.fill.color)).toBeGreaterThanOrEqual(4.5)
    }
  })

  test('corner radius clamps to 0..60 (round 3; render side caps at shortestSide/2)', () => {
    const over = makeZone({ cellX: 0, cellY: 0, cellsWide: 4, cellsTall: 4, title: 't', cornerRadius: 99 })
    expect(zonePaint({ zone: over, index: 0, sample, tone: 'Light', cellHeight: CELL }).cornerRadius).toBe(60)
    const square = makeZone({ cellX: 0, cellY: 0, cellsWide: 4, cellsTall: 4, title: 't', cornerRadius: -3 })
    expect(zonePaint({ zone: square, index: 0, sample, tone: 'Light', cellHeight: CELL }).cornerRadius).toBe(0)
  })
})

describe('title layout (spec 04 §4.2, four styles)', () => {
  const rect = { left: 200, top: 300, width: 500, height: 400 }
  const base = { zoneRect: rect, cellHeight: CELL, titleSize: 'M' as const, cornerRadius: 20, textWidth: 80 }

  test('font size is clamp(cell×factor, 15, 22)', () => {
    expect(titleFontPx('M', CELL)).toBe(Math.round(Math.min(22, Math.max(15, CELL * 0.2))))
    expect(titleFontPx('S', 60)).toBe(15) // floor
    expect(titleFontPx('L', 400)).toBe(22) // ceiling
  })

  test('Chip: overhang lane straddles the panel edge; no row reservation', () => {
    const l = titleLayout({ ...base, style: 'Chip', clearanceAbove: CELL })
    expect(l.overhang).toBe(true)
    expect(l.reserveFirstRow).toBe(false)
    expect(l.y).toBeLessThan(rect.top)
    expect(l.y + l.height).toBeGreaterThan(rect.top)
  })

  test('Chip: in-panel fallback reserves row 1', () => {
    const l = titleLayout({ ...base, style: 'Chip', clearanceAbove: 4 })
    expect(l.overhang).toBe(false)
    expect(l.reserveFirstRow).toBe(true)
    expect(l.y).toBeGreaterThan(rect.top)
  })

  test('Chip width caps inside the zone', () => {
    const l = titleLayout({ ...base, style: 'Chip', textWidth: 9999, clearanceAbove: CELL })
    expect(l.width).toBeLessThanOrEqual(rect.width - 16)
  })

  test('None: zero footprint — no lane, no reserved row (round 3)', () => {
    const l = titleLayout({ ...base, style: 'None', clearanceAbove: CELL })
    expect(l.width).toBe(0)
    expect(l.height).toBe(0)
    expect(l.overhang).toBe(false)
    expect(l.reserveFirstRow).toBe(false)
  })

  test('Etched shares the chip lanes (overhang with clearance)', () => {
    const up = titleLayout({ ...base, style: 'Etched', clearanceAbove: CELL })
    expect(up.overhang).toBe(true)
    const flush = titleLayout({ ...base, style: 'Etched', clearanceAbove: 4 })
    expect(flush.overhang).toBe(false)
    expect(flush.reserveFirstRow).toBe(true)
  })

  test('Bar: full width, in panel, ALWAYS reserves row 1; header type owns the band', () => {
    const l = titleLayout({ ...base, style: 'Bar', clearanceAbove: CELL })
    expect(l.width).toBe(rect.width)
    expect(l.overhang).toBe(false)
    expect(l.reserveFirstRow).toBe(true)
    expect(l.x).toBe(rect.left)
    // Header reads clearly larger than the base title so it reads as a header…
    expect(l.fontPx).toBeGreaterThan(titleFontPx('M', CELL))
    // …and its height clears the seam (additive rhythm) and the panel corner.
    expect(l.height).toBe(l.fontPx + 16)
  })
})

describe('material set (designer 2026-07-09)', () => {
  const sample = { l: 0.7, c: 0.05, h: 70 }
  const mk = (material) => makeZone({ cellX: 0, cellY: 0, cellsWide: 4, cellsTall: 4, title: 't', material })

  test('Fluted: translucent ribbed glass — frost on, flute tile, near-neutral chroma', () => {
    const p = zonePaint({ zone: mk('Fluted'), index: 0, sample, tone: 'Light', cellHeight: CELL })
    expect(p.blurSigma).toBeGreaterThan(0)
    expect(p.texture?.kind).toBe('flute')
    expect(p.fill.alpha).toBeLessThanOrEqual(0.6) // translucent: the light bands need the wallpaper
    const fill = hexToOklch(p.fill.color)
    expect(fill.c).toBeLessThanOrEqual(0.02) // ≈0.018 cap + hex round-trip: never a color mix (the Glaze death)
  })

  test('Paper: opaque warm matte — no frost, letterpress + grain', () => {
    const p = zonePaint({ zone: mk('Paper'), index: 0, sample, tone: 'Light', cellHeight: CELL })
    expect(p.fill.alpha).toBeGreaterThanOrEqual(0.9)
    expect(p.blurSigma).toBe(0)
    expect(p.letterpressBottom).not.toBeNull()
    expect(p.texture?.kind).toBe('noise')
    const fill = hexToOklch(p.fill.color)
    expect(fill.h).toBeGreaterThan(55) // warm paper identity, not wallpaper-hue glass
    expect(fill.h).toBeLessThan(95)
  })

  test('Brushed: near-opaque metal — brush tile + sheen + plate edges, no frost', () => {
    const p = zonePaint({ zone: mk('Brushed'), index: 0, sample, tone: 'Light', cellHeight: CELL })
    expect(p.fill.alpha).toBeGreaterThanOrEqual(0.85) // presence (the Float death was α0.18)
    expect(p.blurSigma).toBe(0)
    expect(p.texture?.kind).toBe('brush')
    expect(p.sheen).not.toBeNull()
    expect(p.letterpressBottom).not.toBeNull()
    const fill = hexToOklch(p.fill.color)
    expect(fill.c).toBeLessThanOrEqual(0.02) // warm-graphite neutral, never muddy
  })

  test('投影 finish gates: toggle drives real bodies, never Outline; glass rides its shader', () => {
    const on = zonePaint({ zone: { ...mk('Brushed'), shadow: true }, index: 0, sample, tone: 'Light', cellHeight: CELL })
    expect(on.shadow).not.toBeNull()
    const outline = zonePaint({ zone: { ...mk('Outline'), shadow: true }, index: 0, sample, tone: 'Light', cellHeight: CELL })
    expect(outline.shadow).toBeNull()
    const glass = zonePaint({ zone: { ...mk('LiquidGlass'), shadow: true }, index: 0, sample, tone: 'Light', cellHeight: CELL })
    expect(glass.shadow).toBeNull() // shader-owned gaussian ring instead
    expect(glass.liquidGlass?.shadow).toBeGreaterThan(0)
  })
})

describe('accent palette hygiene', () => {
  test('no accent lands in the banned blue/violet band', () => {
    for (const hex of ACCENT_PALETTE) {
      const { h, c } = hexToOklch(hex)
      // OKLCH blue/violet territory ≈ hue 230..330 at visible chroma.
      expect(c < 0.04 || h < 230 || h > 330).toBe(true)
    }
  })
})

/** WCAG 2.x relative-luminance contrast between two hexes. */
function wcagContrast(a: string, b: string): number {
  const lum = (hex: string) => {
    const n = hex.replace('#', '')
    const ch = (i: number) => {
      const v = parseInt(n.slice(i, i + 2), 16) / 255
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
    }
    return 0.2126 * ch(0) + 0.7152 * ch(2) + 0.0722 * ch(4)
  }
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}
