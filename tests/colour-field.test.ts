import { describe, expect, test } from 'bun:test'
import type { ConfigDto, TypeOverrides } from '../src/bridge/types'
import { resolveTypeConfig } from '../src/lib/type-config'
import type { Raster } from '../src/icon-compositor/raster'
import { makeRaster } from '../src/icon-compositor/raster'
import { dominantColor } from '../src/icon-compositor/analysis'
import { fieldPlateTone, fieldShadowTone, perceivedLightness, themedContrastTone, toOkLab } from '../src/icon-compositor/color'
import { renderTile } from '../src/icon-compositor/compose'
import { iconProfile } from '../src/icon-compositor/profile'

// 满彩 Field mode (ADR-0016, recipe v2): dominant-colour extraction, harmony
// band tiers, and the recognizability-first lane decision — artwork preserved,
// plate contrasts in lightness within the icon's own hue; knockout only for
// flat single-hue silhouettes (owner rejection of the v1 flatten-everything).

const S = 64

const FIELD_CONFIG: ConfigDto = {
  shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'derived', shortcutShape: null, monoStyle: 'Tonal',
  tint: '#FF6F5E', distinction: 'None', markStyle: 'Shadow', markColor: null,
  plateColor: null, size: 'Mid', filter: 'None',
}

function px(c: Raster, x: number, y: number): [number, number, number, number] {
  const i4 = (y * c.width + x) * 4
  return [c.data[i4], c.data[i4 + 1], c.data[i4 + 2], c.data[i4 + 3]]
}

function fill(c: Raster, x0: number, y0: number, x1: number, y1: number, r: number, g: number, b: number, a = 255): void {
  for (let y = y0; y < y1; y++) {
    for (let x = x0; x < x1; x++) {
      const i4 = (y * c.width + x) * 4
      c.data[i4] = r
      c.data[i4 + 1] = g
      c.data[i4 + 2] = b
      c.data[i4 + 3] = a
    }
  }
}

/** A flat single-hue glyph floating on transparency (knockout-eligible). */
function greenGlyph(): Raster {
  const c = makeRaster(S)
  fill(c, 16, 16, 48, 48, 30, 185, 90)
  return c
}

/** A PLUS-shaped glyph: genuinely NO own background (in-bounds coverage ~47%
 *  fails the shape-ring probe) — the true bare-lane fixture under the owner's
 *  generic has-background rule, where any solid rectangle anchors. */
function plusGlyph(r: number, g: number, b: number): Raster {
  const c = makeRaster(S)
  fill(c, 26, 10, 38, 54, r, g, b)
  fill(c, 10, 26, 54, 38, r, g, b)
  return c
}

/** A four-hue quadrant logo on transparency (multi-hue, fidelity lane). */
function quadLogo(): Raster {
  const c = makeRaster(S)
  fill(c, 8, 8, 32, 32, 235, 60, 50)
  fill(c, 32, 8, 56, 32, 40, 120, 230)
  fill(c, 8, 32, 32, 56, 60, 200, 90)
  fill(c, 32, 32, 56, 56, 245, 200, 40)
  return c
}

/** A green disc with dark wave detail (Spotify-like): one hue, NOT flat. */
function wavyDisc(): Raster {
  const c = makeRaster(S)
  const h = S / 2
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const dx = x + 0.5 - h
      const dy = y + 0.5 - h
      if (dx * dx + dy * dy > (h - 6) * (h - 6)) continue
      const i4 = (y * S + x) * 4
      const wave = Math.abs((y % 14) - 7) < 2
      c.data[i4] = wave ? 12 : 30
      c.data[i4 + 1] = wave ? 30 : 185
      c.data[i4 + 2] = wave ? 18 : 90
      c.data[i4 + 3] = 255
    }
  }
  return c
}

function chromaOf(r: number, g: number, b: number): number {
  const lab = toOkLab(r, g, b)
  return Math.sqrt(lab.A * lab.A + lab.B * lab.B)
}

describe('dominantColor', () => {
  test('single-hue artwork yields its colour with low dispersion', () => {
    const dom = dominantColor(greenGlyph(), null)
    expect(dom).not.toBeNull()
    expect(dom!.colour.g).toBeGreaterThan(dom!.colour.r)
    expect(dom!.colour.g).toBeGreaterThan(dom!.colour.b)
    expect(dom!.dispersion).toBeLessThan(0.1)
  })

  test('neutral gray artwork is the no-hue tail (null)', () => {
    const c = makeRaster(S)
    fill(c, 8, 8, 56, 56, 128, 128, 130)
    expect(dominantColor(c, null)).toBeNull()
  })

  test('four scattered hues have NO majority theme (owner 50% rule)', () => {
    // Each quadrant holds 25% — no band reaches the 50% subject majority.
    expect(dominantColor(quadLogo(), null)).toBeNull()
  })

  test('a decorative accent on neutral art is NOT the theme colour', () => {
    const c = makeRaster(S)
    fill(c, 0, 0, S, S, 245, 245, 245) // near-white plate dominates
    fill(c, 24, 24, 40, 40, 40, 120, 230) // small blue accent (~6%)
    expect(dominantColor(c, null)).toBeNull() // owner: theme needs >=50%
  })

  test('near-hue gradients merge into ONE theme colour (owner spec)', () => {
    // Light blue top / deep blue bottom (the Microsoft two-blue pattern).
    const c = makeRaster(S)
    fill(c, 8, 8, 56, 32, 120, 180, 240)
    fill(c, 8, 32, 56, 56, 20, 80, 200)
    const dom = dominantColor(c, null)
    expect(dom).not.toBeNull()
    expect(dom!.colour.b).toBeGreaterThan(dom!.colour.r) // averaged blue theme
  })
})

describe('fieldPlateTone / fieldInk', () => {
  const seeds = [
    { r: 30, g: 185, b: 90, a: 255 }, // green
    { r: 40, g: 120, b: 230, a: 255 }, // blue
    { r: 235, g: 60, b: 50, a: 255 }, // red
    { r: 245, g: 200, b: 40, a: 255 }, // yellow
    { r: 20, g: 24, b: 60, a: 255 }, // near-black navy (lifted into the slot)
  ]

  test('Vivid plates sit on the light line and genuinely carry colour (v7)', () => {
    for (const seed of seeds) {
      const plate = fieldPlateTone(seed, 'Vivid')
      expect(perceivedLightness(plate.r, plate.g, plate.b)).toBeGreaterThan(0.76)
      // Designer FAIL item 1: the plate must read as a COLOUR, not a white
      // board. Gamut caps blues lower than warm hues — floor at 0.05.
      expect(chromaOf(plate.r, plate.g, plate.b)).toBeGreaterThan(0.05)
    }
  })

  test('themed contrast plate: only truly LIGHT subjects take a dark board', () => {
    for (const seed of seeds) {
      const dark = themedContrastTone(seed, 0.9, 'Vivid')
      expect(perceivedLightness(dark.r, dark.g, dark.b)).toBeLessThan(0.45)
      // Mid lightness reads dark-ish to the eye → bright board (owner call).
      const mid = themedContrastTone(seed, 0.65, 'Vivid')
      expect(perceivedLightness(mid.r, mid.g, mid.b)).toBeGreaterThan(0.76)
      const light = themedContrastTone(seed, 0.2, 'Vivid')
      expect(perceivedLightness(light.r, light.g, light.b)).toBeGreaterThan(0.76)
    }
  })

  test('Quiet is a pastel that still carries the hue', () => {
    for (const seed of seeds) {
      const plate = fieldPlateTone(seed, 'Quiet')
      const L = perceivedLightness(plate.r, plate.g, plate.b)
      expect(L).toBeGreaterThan(0.86)
      expect(chromaOf(plate.r, plate.g, plate.b)).toBeGreaterThan(0.02)
    }
  })

  test('shadow tone is a deep quiet tone of the plate hue', () => {
    for (const seed of seeds) {
      for (const band of ['Vivid', 'Quiet'] as const) {
        const plate = fieldPlateTone(seed, band)
        const shadow = fieldShadowTone(plate)
        expect(perceivedLightness(shadow.r, shadow.g, shadow.b)).toBeLessThan(0.45)
      }
    }
  })
})

/** Repaint the glyph's outermost solid layer (4-neighbour boundary) in a
 *  given colour — models the thin highlight outlines / white matte fringes
 *  that fooled the old 1px rim ring. */
function outlineGlyph(c: Raster, r: number, g: number, b: number): Raster {
  const { width: W, height: H, data: d } = c
  const solid = (x: number, y: number): boolean =>
    x >= 0 && y >= 0 && x < W && y < H && d[(y * W + x) * 4 + 3] >= 128
  const edges: number[] = []
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      if (!solid(x, y)) continue
      if (solid(x - 1, y) && solid(x + 1, y) && solid(x, y - 1) && solid(x, y + 1)) continue
      edges.push(y * W + x)
    }
  }
  for (const i of edges) {
    d[i * 4] = r
    d[i * 4 + 1] = g
    d[i * 4 + 2] = b
  }
  return c
}

describe('subjectRim band (owner rim law 2026-07-10)', () => {
  test('a thin LIGHT outline on a dark glyph cannot fake a light rim', () => {
    // GitHub/terminal pathology: dark body + 1px light stroke → the old 1px
    // ring read "light" and dealt a dark plate (dark-on-dark, owner rejection).
    const c = outlineGlyph(plusGlyph(35, 38, 44), 246, 246, 246)
    const p = iconProfile(c)
    expect(p.kind).toBe('bare')
    expect(p.subjectRimLightness).toBeLessThan(0.7) // band out-votes the stroke
    const tile = renderTile(c, FIELD_CONFIG, false, false, S)
    const [r, g, b] = px(tile, 4, S / 2)
    expect(perceivedLightness(r, g, b)).toBeGreaterThan(0.76) // bright board
  })

  test('rim colour is the MAJORITY hue, not the loudest accent', () => {
    // Explorer pathology: yellow ring + small blue accent → the plate must
    // stay in the YELLOW family (owner: 外围一圈黄色占比最多 → 淡黄/深黄底).
    const c = plusGlyph(250, 204, 60)
    fill(c, 26, 10, 34, 18, 30, 90, 220) // blue accent crossing the rim
    const p = iconProfile(c)
    expect(p.kind).toBe('bare')
    expect(p.subjectRimColour).not.toBeNull()
    expect(p.subjectRimColour!.r).toBeGreaterThan(p.subjectRimColour!.b) // yellow, not blue
    // Light-yellow rim (L≥0.7) → DEEP yellow board (深黄), same hue family.
    expect(p.subjectRimLightness).toBeGreaterThanOrEqual(0.7)
    const tile = renderTile(c, FIELD_CONFIG, false, false, S)
    const [r, g, b] = px(tile, 4, S / 2)
    expect(perceivedLightness(r, g, b)).toBeLessThan(0.45) // dark board
    expect(chromaOf(r, g, b)).toBeGreaterThan(0.03) // themed, not gray
    expect(r).toBeGreaterThan(b) // yellow-family plate
  })

  test('soft drop shadows (blended alpha) never join the rim vote', () => {
    const c = plusGlyph(250, 250, 250)
    // A soft dark shadow halo around the glyph, alpha below the solid gate.
    for (let y = 0; y < S; y++) {
      for (let x = 0; x < S; x++) {
        const i4 = (y * S + x) * 4
        if (c.data[i4 + 3] > 0) continue
        const nearGlyph = x >= 8 && x < 56 && y >= 8 && y < 56
        if (!nearGlyph) continue
        c.data[i4] = 10
        c.data[i4 + 1] = 10
        c.data[i4 + 2] = 12
        c.data[i4 + 3] = 160
      }
    }
    const p = iconProfile(c)
    // The white glyph's rim stays light → dark board keeps the glyph visible.
    expect(p.subjectRimLightness).toBeGreaterThanOrEqual(0.7)
  })
})

describe('renderTile Field mode (recipe v2)', () => {
  test('flat silhouette: light themed plate for a mid-light subject', () => {
    // Mid-light green (L~0.65) reads dark-ish to the eye → BRIGHT green-hue
    // plate (owner: only near-white subjects take the dark board).
    const tile = renderTile(plusGlyph(30, 185, 90), FIELD_CONFIG, false, false, S)
    const [r, g, b, a] = px(tile, 4, S / 2) // plate zone (inside squircle, outside glyph)
    expect(a).toBe(255)
    expect(chromaOf(r, g, b)).toBeGreaterThan(0.03) // themed, NOT gray
    expect(g).toBeGreaterThan(b) // green-family plate
    expect(perceivedLightness(r, g, b)).toBeGreaterThan(0.76) // bright board

    // Subject pixels are LAW-PROTECTED: the centre keeps the original green.
    const [cr, cg, cb] = px(tile, S / 2, S / 2)
    expect(Math.abs(cr - 30)).toBeLessThanOrEqual(14)
    expect(Math.abs(cg - 185)).toBeLessThanOrEqual(14)
    expect(Math.abs(cb - 90)).toBeLessThanOrEqual(14)
  })

  test('bare artwork gets a silhouette shadow OPPOSING the plate', () => {
    // Bright plate here → deep shadow band under the glyph (on dark plates
    // the same mechanism flips to a light glow).
    const tile = renderTile(plusGlyph(30, 185, 90), FIELD_CONFIG, false, false, S)
    const glyphBottom = Math.round(S / 2 + (S * 0.72) / 2)
    let minL = 1
    for (let y = glyphBottom - 1; y <= Math.min(S - 6, glyphBottom + 3); y++) {
      for (let x = S / 2 - 8; x <= S / 2 + 8; x++) {
        const [r2, g2, b2, a2] = px(tile, x, y)
        if (a2 === 0) continue
        minL = Math.min(minL, perceivedLightness(r2, g2, b2))
      }
    }
    const [pr, pg, pb] = px(tile, S / 2, 8) // plate far above the glyph
    expect(minL).toBeLessThan(perceivedLightness(pr, pg, pb) - 0.03)
  })

  test('FIXED-plate bare artwork still gets the silhouette shadow (regression, STATE §-1a ①)', () => {
    // A user-set / preset fixed plate must NOT drop the silhouette shadow the derived lane gets
    // — the shadow relapsed once on the fixed-plate lane (T2). Bare artwork on a bright fixed
    // plate (#EAD6A8, the stationery/pebble folder board) → a deep shadow band under the glyph,
    // exactly as the derived-plate case above.
    const tile = renderTile(plusGlyph(30, 185, 90), { ...FIELD_CONFIG, plateColor: '#EAD6A8' }, false, false, S)
    const glyphBottom = Math.round(S / 2 + (S * 0.72) / 2)
    let minL = 1
    for (let y = glyphBottom - 1; y <= Math.min(S - 6, glyphBottom + 3); y++) {
      for (let x = S / 2 - 8; x <= S / 2 + 8; x++) {
        const [r2, g2, b2, a2] = px(tile, x, y)
        if (a2 === 0) continue
        minL = Math.min(minL, perceivedLightness(r2, g2, b2))
      }
    }
    const [pr, pg, pb] = px(tile, S / 2, 8) // the fixed plate, far above the glyph
    expect(minL).toBeLessThan(perceivedLightness(pr, pg, pb) - 0.03)
  })

  test('detailed bare artwork keeps its own pixels (no re-inking)', () => {
    const tile = renderTile(wavyDisc(), FIELD_CONFIG, false, false, S)
    let darkGreen = 0
    for (let y = 8; y < S - 8; y++) {
      for (let x = 8; x < S - 8; x++) {
        const [r2, g2, b2, a2] = px(tile, x, y)
        if (a2 > 0 && r2 < 60 && g2 < 80 && b2 < 60) darkGreen++
      }
    }
    expect(darkGreen).toBeGreaterThan(20) // the dark waves survived
  })

  test('fidelity lane: multi-hue artwork keeps its own colours', () => {
    const tile = renderTile(quadLogo(), FIELD_CONFIG, false, false, S)
    const q = Math.round(S * 0.35)
    const [r1] = px(tile, q, q)
    const [, , b2] = px(tile, S - q, q)
    expect(r1).toBeGreaterThan(150) // red quadrant stays red-ish
    expect(b2).toBeGreaterThan(150) // blue quadrant stays blue-ish
  })

  test('plated source keeps its own plate colour (identity preserved)', () => {
    // A deep-green plate with a white glyph (Excel-like), opaque canvas.
    const c = makeRaster(S)
    fill(c, 0, 0, S, S, 16, 110, 70)
    fill(c, 20, 20, 44, 44, 250, 250, 250)
    const tile = renderTile(c, FIELD_CONFIG, false, false, S)
    const [r, g, b] = px(tile, 6, S / 2)
    expect(g).toBeGreaterThan(r) // still the icon's own green, not a band pastel
    expect(g).toBeGreaterThan(b)
    expect(perceivedLightness(r, g, b)).toBeLessThan(0.75) // not washed to pastel
  })

  test('user 背景色 leaves NO backdrop rectangle behind the crop (codex #2)', () => {
    // A floating deep-green board + white glyph with a USER plate override:
    // the crop must not carry old green around the subject — backdrop pixels
    // inside the crop swap to the user plate.
    const c = makeRaster(S)
    fill(c, 8, 8, 56, 56, 16, 110, 70)
    fill(c, 24, 24, 40, 40, 250, 250, 250)
    const tile = renderTile(c, { ...FIELD_CONFIG, plateColor: '#D9A94E' }, false, false, S)
    let residue = 0
    for (let y = 6; y < S - 6; y++) {
      for (let x = 6; x < S - 6; x++) {
        const [r, g, b, a] = px(tile, x, y)
        if (a === 0) continue
        if (Math.abs(r - 16) + Math.abs(g - 110) + Math.abs(b - 70) < 60) residue++
      }
    }
    expect(residue).toBe(0)
    // And the subject itself is still white (law 4).
    const [wr, wg, wb] = px(tile, S / 2, S / 2)
    expect(wr).toBeGreaterThan(230)
    expect(wg).toBeGreaterThan(230)
    expect(wb).toBeGreaterThan(230)
  })

  test('no-hue tail falls back to a non-white plate (never the white board)', () => {
    const c = makeRaster(S)
    fill(c, 16, 16, 48, 48, 128, 128, 130) // gray logo, dominantColor null
    const tile = renderTile(c, FIELD_CONFIG, false, false, S)
    const [r, g, b] = px(tile, 4, S / 2)
    expect(r === 255 && g === 255 && b === 255).toBe(false)
  })

  test('near-pure-light BARE artwork gets the far (dark) themed plate', () => {
    const tile = renderTile(plusGlyph(250, 235, 120), FIELD_CONFIG, false, false, S)
    const [r, g, b] = px(tile, 4, S / 2)
    expect(perceivedLightness(r, g, b)).toBeLessThan(0.45) // strong contrast side
    expect(chromaOf(r, g, b)).toBeGreaterThan(0.03) // still carries the yellow hue
  })

  test('shape None degrades to the classic path (byte-equal to Original)', () => {
    const field = renderTile(greenGlyph(), { ...FIELD_CONFIG, shape: 'None' }, false, false, S)
    const original = renderTile(
      greenGlyph(), { ...FIELD_CONFIG, shape: 'None', colorMode: 'Original' }, false, false, S,
    )
    expect(Buffer.from(field.data).equals(Buffer.from(original.data))).toBe(true)
  })

  test('Quiet band renders a light pastel plate for dark bare artwork', () => {
    const tile = renderTile(plusGlyph(20, 90, 50), { ...FIELD_CONFIG, plateBand: 'Quiet' }, false, false, S)
    const [r, g, b] = px(tile, 4, S / 2)
    expect(perceivedLightness(r, g, b)).toBeGreaterThan(0.85)
  })

  test('pipeline-resolved fieldSeed wins over the derived seed', () => {
    const tile = renderTile(plusGlyph(30, 185, 90), FIELD_CONFIG, false, false, S, { fieldSeed: '#8850C8' })
    const [r, g, b] = px(tile, 4, S / 2)
    expect(b).toBeGreaterThan(g) // purple-family plate from the spread pass
    expect(r).toBeGreaterThan(g)
  })
})

describe('kind families + affordances (D2, plan T5)', () => {
  test('Folder bucket takes the amber family plate (no tab stripe — owner cut)', () => {
    const c = makeRaster(S)
    fill(c, 16, 20, 48, 52, 240, 196, 90) // folder-ish artwork
    const tile = renderTile(c, FIELD_CONFIG, false, false, S, { kindBucket: 'Folder' })
    const [r, g, b] = px(tile, 4, S / 2) // plate
    expect(r).toBeGreaterThan(b) // amber family, not the artwork's own hue swing
    // The plate is UNIFORM (the unreadable tab stripe was cut, owner 2026-07-10):
    // top strip pixels match the mid-plate colour.
    // (0.06 tolerance: the ring halo's tail grazes this point; the cut tab
    // stripe was a 0.14 lightness drop — well outside this band.)
    const [tr, tg, tb] = px(tile, Math.round(S * 0.2), Math.round(S * 0.12))
    expect(Math.abs(perceivedLightness(tr, tg, tb) - perceivedLightness(r, g, b))).toBeLessThan(0.06)
  })

  test('white grayscale artwork (no own bg) gets a darkish NEUTRAL contrast plate', () => {
    const tile = renderTile(plusGlyph(250, 250, 250), FIELD_CONFIG, false, false, S, { kindBucket: 'File' })
    const [r, g, b] = px(tile, 4, S / 2)
    expect(chromaOf(r, g, b)).toBeLessThan(0.02) // no forced hue on gray art
    expect(perceivedLightness(r, g, b)).toBeLessThan(0.5) // white subject → dark board
  })

  test('dark grayscale artwork (no own bg) gets a light NEUTRAL plate (white legal)', () => {
    const tile = renderTile(plusGlyph(40, 44, 50), FIELD_CONFIG, false, false, S, { kindBucket: 'System' })
    const [r, g, b] = px(tile, 4, S / 2)
    expect(chromaOf(r, g, b)).toBeLessThan(0.02)
    expect(perceivedLightness(r, g, b)).toBeGreaterThan(0.8) // dark subject → light board
  })

  test('type ladder reshapes buckets via the resolve chain (ADR-0017)', () => {
    // The per-type shape now arrives RESOLVED: renderTile itself is
    // ladder-blind, resolveTypeConfig folds the bucket patch upstream.
    const art = greenGlyph()
    const ladder: TypeOverrides = { System: { source: 'custom', patch: { shape: 'Circle' } } }
    const follower = renderTile(art, resolveTypeConfig(FIELD_CONFIG, ladder, 'App'), false, false, S)
    const patched = renderTile(art, resolveTypeConfig(FIELD_CONFIG, ladder, 'System'), false, false, S)
    // (8,8) sits inside the Apple squircle's rounded corner but OUTSIDE the
    // inscribed circle.
    const [, , , followerCorner] = px(follower, 8, 8)
    const [, , , patchedCorner] = px(patched, 8, 8)
    expect(followerCorner).toBeGreaterThan(0) // App follows the global Apple squircle
    expect(patchedCorner).toBe(0) // System's Circle patch wins
  })
})

describe('own-board anchoring (owner 2026-07-10: no nested boards)', () => {
  test('a Twitter-class colour board becomes THE plate (no second board)', () => {
    // A big square blue board floating on transparency, white glyph inside.
    const c = makeRaster(S)
    fill(c, 6, 6, 58, 58, 29, 155, 240)
    fill(c, 24, 26, 40, 38, 252, 252, 252)
    const tile = renderTile(c, FIELD_CONFIG, false, false, S)
    // The squircle face is the board's own blue (clamped), not a pastel wrap.
    const [r, g, b] = px(tile, 6, S / 2)
    expect(b).toBeGreaterThan(r + 40)
    expect(chromaOf(r, g, b)).toBeGreaterThan(0.09) // saturated board, not band pastel
  })

  test('an own white background is used UNCHANGED (owner final anchor rule)', () => {
    // 「如果这个图标本身自带背景，就使用其自带的背景颜色，不要改动」— the
    // solid white rectangle anchors as its own white board, expanded as-is.
    const c = makeRaster(S)
    fill(c, 22, 10, 44, 54, 250, 250, 250)
    const tile = renderTile(c, FIELD_CONFIG, false, false, S, { kindBucket: 'File' })
    const [r, g, b] = px(tile, 6, S / 2)
    expect(perceivedLightness(r, g, b)).toBeGreaterThan(0.9)
    expect(chromaOf(r, g, b)).toBeLessThan(0.02)
  })

  test('user 背景色 override takes effect in Field (owner reversal)', () => {
    const tile = renderTile(greenGlyph(), { ...FIELD_CONFIG, plateColor: '#D9A94E' }, false, false, S)
    const [r, g, b] = px(tile, 4, S / 2)
    expect(r).toBeGreaterThan(180) // amber user plate, not the derived green
    expect(g).toBeGreaterThan(120)
    expect(b).toBeLessThan(140)
  })
})
