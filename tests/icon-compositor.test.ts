import { describe, expect, test } from 'bun:test'
import type { ConfigDto } from '../src/bridge/types'
import { makeRaster, shapeMask } from '../src/icon-compositor/raster'
import type { Raster } from '../src/icon-compositor/raster'
import {
  grayValue, hslToRgb, monoRamp, monoTone, perceivedLightness, srgbDecode, srgbEncode, stretchedLightness,
} from '../src/icon-compositor/color'
import { shapeContains, shapeOutline } from '../src/icon-compositor/shapes'
import {
  analysis, findContentBounds, hasTransparentEdges, matchesShape, maxScaleInside, solidBounds, tryDetectBackground,
} from '../src/icon-compositor/analysis'
import { chamferDistance } from '../src/icon-compositor/filters'
import { renderTile } from '../src/icon-compositor/compose'
import { downscale } from '../src/icon-compositor/sampling'

// Synthetic-source fixtures mirroring the C# oracle's test canvases: the port
// must make the same structural decisions (plate detection, pass-through,
// bare-logo plating) the frozen TileRenderer makes.

const S = 64

function px(c: Raster, x: number, y: number): [number, number, number, number] {
  const i4 = (y * c.width + x) * 4
  return [c.data[i4], c.data[i4 + 1], c.data[i4 + 2], c.data[i4 + 3]]
}

function solidPlate(size: number, r: number, g: number, b: number): Raster {
  const c = makeRaster(size)
  for (let i = 0; i < c.data.length; i += 4) {
    c.data[i] = r
    c.data[i + 1] = g
    c.data[i + 2] = b
    c.data[i + 3] = 255
  }
  return c
}

/** A dark glyph square centred on a solid plate. */
function glyphOnPlate(size: number): Raster {
  const c = solidPlate(size, 40, 120, 220)
  const q = Math.floor(size / 4)
  for (let y = q; y < size - q; y++) {
    for (let x = q; x < size - q; x++) {
      const i4 = (y * size + x) * 4
      c.data[i4] = 250
      c.data[i4 + 1] = 250
      c.data[i4 + 2] = 250
    }
  }
  return c
}

/** A round opaque logo floating on transparency. */
function circleLogo(size: number): Raster {
  const c = makeRaster(size)
  const h = size / 2
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const dx = x + 0.5 - h
      const dy = y + 0.5 - h
      if (dx * dx + dy * dy <= (h - 2) * (h - 2)) {
        const i4 = (y * size + x) * 4
        c.data[i4] = 200
        c.data[i4 + 1] = 60
        c.data[i4 + 2] = 60
        c.data[i4 + 3] = 255
      }
    }
  }
  return c
}

const config = (over: Partial<ConfigDto> = {}): ConfigDto => ({
  shape: 'Apple',
  subject: 'Original',
  plateBand: 'Vivid',
  // Classic-pipeline fixtures assert the FAITHFUL lane (white fallback);
  // derived-lane behaviour is colour-field.test.ts territory.
  plateFallback: 'white',
  shortcutShape: null,
  monoStyle: 'Tonal',
  tint: '#FF6F5E',
  distinction: 'None',
  markStyle: 'Arc',
  markColor: null,
  plateColor: null,
  size: 'Mid',
  filter: 'None',
  ...over,
})

describe('color math (IconColorTreatment parity)', () => {
  test('srgb roundtrip is identity on bytes', () => {
    for (const v of [0, 1, 8, 64, 128, 200, 254, 255]) {
      expect(srgbEncode(srgbDecode(v))).toBe(v)
    }
  })

  test('grayValue matches the prototype curve', () => {
    expect(grayValue(0)).toBe(Math.round(255 * 0.08))
    expect(grayValue(1)).toBe(Math.round(255 * 0.94))
    expect(grayValue(0.5)).toBe(128)
  })

  test('hslToRgb hits the primaries', () => {
    expect(hslToRgb(0, 1, 50)).toEqual({ r: 255, g: 0, b: 0, a: 255 })
    expect(hslToRgb(120, 1, 50)).toEqual({ r: 0, g: 255, b: 0, a: 255 })
    expect(hslToRgb(240, 1, 50)).toEqual({ r: 0, g: 0, b: 255, a: 255 })
  })

  test('mono ramp: dark end deep, light end pastel, same hue family', () => {
    const tint = 0xff6f5e
    const dark = monoRamp(0, tint)
    const light = monoRamp(1, tint)
    expect(perceivedLightness(dark.r, dark.g, dark.b)).toBeLessThan(0.5)
    expect(perceivedLightness(light.r, light.g, light.b)).toBeGreaterThan(0.85)
    // Warm tint keeps r >= b at both ends (hue preserved).
    expect(dark.r).toBeGreaterThan(dark.b)
    expect(light.r).toBeGreaterThanOrEqual(light.b)
  })

  test('monoTone gamut-fits a saturated seed without throwing', () => {
    const t = monoTone(0.4, 1.15, 0x00ff00)
    expect(t.a).toBe(255)
    for (const v of [t.r, t.g, t.b]) {
      expect(v).toBeGreaterThanOrEqual(0)
      expect(v).toBeLessThanOrEqual(255)
    }
  })

  test('stretchedLightness stretches contrast but passes near-flat tiles through', () => {
    const flat = solidPlate(8, 128, 128, 128)
    const flatT = stretchedLightness(flat)
    const l = perceivedLightness(128, 128, 128)
    expect(Math.abs(flatT[0] - l)).toBeLessThan(0.01)

    const contrasty = solidPlate(8, 90, 90, 90)
    for (let x = 0; x < 4; x++) {
      const i4 = x * 4
      contrasty.data[i4] = 180
      contrasty.data[i4 + 1] = 180
      contrasty.data[i4 + 2] = 180
    }
    const t = stretchedLightness(contrasty)
    expect(t[0]).toBeGreaterThan(0.9) // bright half → top of the ramp
    expect(t[32]).toBeLessThan(0.1) // dark half → bottom
  })
})

describe('shape geometry (IconShapeGeometry parity)', () => {
  test('circle contains centre, excludes corner', () => {
    expect(shapeContains('Circle', 32, 32, 64)).toBe(true)
    expect(shapeContains('Circle', 1, 1, 64)).toBe(false)
  })

  test('apple squircle covers more corner than the circle but less than the box', () => {
    expect(shapeContains('Apple', 32, 32, 64)).toBe(true)
    expect(shapeContains('Apple', 2, 2, 64)).toBe(false)
    expect(shapeContains('Apple', 6, 6, 64)).toBe(true) // circle excludes this point
    expect(shapeContains('Circle', 6, 6, 64)).toBe(false)
  })

  test('None is the full box', () => {
    expect(shapeContains('None', 0, 0, 64)).toBe(true)
    expect(shapeContains('None', 64, 64, 64)).toBe(true)
    expect(shapeContains('None', -1, 0, 64)).toBe(false)
  })

  test('every silhouette fills the box — outline touches all four edges', () => {
    // Full-extent authoring rule: no shape may render smaller than its peers
    // (the old codex-ported organics floated with 5-12% margins; smoothed
    // polygons carry fit-to-box so rounding cannot shrink them).
    const shapes: import('../src/bridge/types').IconShape[] = [
      'Apple', 'Circle', 'Samsung', 'Bookmark', 'Lemon',
      'Tile', 'Teardrop', 'Diamond', 'Flower', 'Pebble',
    ]
    for (const shape of shapes) {
      const pts = shapeOutline(shape, 100)
      const xs = pts.map((p) => p[0])
      const ys = pts.map((p) => p[1])
      expect(Math.min(...xs), `${shape} left`).toBeLessThan(1.5)
      expect(Math.min(...ys), `${shape} top`).toBeLessThan(1.5)
      expect(Math.max(...xs), `${shape} right`).toBeGreaterThan(98.5)
      expect(Math.max(...ys), `${shape} bottom`).toBeGreaterThan(98.5)
    }
  })

  test('curved silhouettes are smooth — dense sampled outlines', () => {
    // Smoothed/authored outlines flatten to short, evenly-turning chords;
    // the retired hand-plotted polygons had ~35px straight edges.
    for (const shape of ['Teardrop', 'Flower', 'Lemon', 'Pebble'] as const) {
      const pts = shapeOutline(shape, 256)
      expect(pts.length, `${shape} density`).toBeGreaterThan(40)
    }
  })

  test('plateColor overrides the synthesized plate (Original and layered Mono)', () => {
    // glyphOnPlate's detected bg is blue (40,120,220); the override paints red.
    const red = renderTile(glyphOnPlate(S), config({ plateColor: '#C03028' }), false, false, S)
    const [rr, , rb] = px(red, 6, S / 2)
    expect(rr).toBeGreaterThan(rb)
    // Mono composes LAYERED: the plate takes the chosen colour RAW while the
    // subject keeps the tonal ramp (chief-UI/UX matrix 2026-07-09).
    const mono = renderTile(glyphOnPlate(S), config({ plateColor: '#C03028', subject: 'Mono', tint: '#3FB6A8' }), false, false, S)
    const [mr, , mb] = px(mono, 6, S / 2)
    expect(mr).toBeGreaterThan(mb)
  })

  test('极致单色 Flat: subject ONE flat colour, plate another — no gradients', () => {
    const tile = renderTile(glyphOnPlate(S), config({ subject: 'Mono', monoStyle: 'Flat', tint: '#3FB6A8' }), false, false, S)
    // The white glyph square is the segmented subject: recoloured to the flat
    // tint, so any two subject pixels are identical.
    const a = px(tile, S / 2, S / 2)
    const b = px(tile, S / 2 - 4, S / 2 + 4)
    expect(a).toEqual(b)
    expect(a[2]).toBeGreaterThan(a[0]) // teal-ish: b > r
    // Plate pixel (Auto) = the ramp's light end — near-white, far brighter.
    const plate = px(tile, 6, S / 2)
    expect(plate[0]).toBeGreaterThan(200)
    expect(plate[0]).not.toBe(a[0])
  })

  test('inset plate icon fills with its OWN plate colour, not white', () => {
    // Owner case (Twitter): a rounded plate inset on transparency has no
    // canvas-edge background, but its uniform surround IS the plate.
    const c = makeRaster(S)
    // rounded-ish blue plate inset from the edges (transparent margin)
    for (let y = 8; y < S - 8; y++) for (let x = 8; x < S - 8; x++) {
      const i4 = (y * S + x) * 4
      c.data[i4] = 40; c.data[i4 + 1] = 120; c.data[i4 + 2] = 220; c.data[i4 + 3] = 255
    }
    // white subject in the middle
    for (let y = 22; y < 42; y++) for (let x = 22; x < 42; x++) {
      const i4 = (y * S + x) * 4
      c.data[i4] = 250; c.data[i4 + 1] = 250; c.data[i4 + 2] = 250
    }
    const tile = renderTile(c, config({ shape: 'Circle' }), false, false, S)
    // a mid-plate pixel (inside the circle, off the subject) must be BLUE, not white
    const [r, , b] = px(tile, 14, S / 2)
    expect(b).toBeGreaterThan(r + 40) // clearly blue
    expect(r).toBeLessThan(160) // not a white fill
  })

  test('marks on free-form icons stay on the REAL silhouette', () => {
    // 原始外形 + 珐琅弧 (owner case: the arc floated in the empty corner of a
    // phantom tile): every painted pixel must lie on/around the icon itself.
    const tile = renderTile(circleLogo(S), config({ shape: 'None', distinction: 'Mark', markStyle: 'Arc' }), true, false, S)
    expect(px(tile, 2, 2)[3]).toBe(0) // far corner: no floating glow
    expect(px(tile, 3, S - 3)[3]).toBe(0) // bottom-left corner outside the disc
  })

  test('Shadow and Halo render distinctly (Halo = silhouette outline)', () => {
    const card = renderTile(circleLogo(S), config({ distinction: 'Mark', markStyle: 'Shadow' }), true, false, S)
    const echo = renderTile(circleLogo(S), config({ distinction: 'Mark', markStyle: 'Halo' }), true, false, S)
    let diff = 0
    for (let i = 0; i < card.data.length; i += 4) {
      if (Math.abs(card.data[i + 3] - echo.data[i + 3]) > 24) diff++
    }
    expect(diff).toBeGreaterThan(S * 4) // whole regions differ, not stray pixels
  })

  test('gloss filter: sheen brightens the top, depth dims the bottom', () => {
    const plain = renderTile(solidPlate(S, 120, 120, 120), config(), false, false, S)
    const glossy = renderTile(solidPlate(S, 120, 120, 120), config({ filter: 'Gloss' }), false, false, S)
    const top = Math.floor(S * 0.12)
    const bottom = Math.floor(S * 0.9)
    expect(px(glossy, S / 2, top)[0]).toBeGreaterThan(px(plain, S / 2, top)[0])
    expect(px(glossy, S / 2, bottom)[0]).toBeLessThanOrEqual(px(plain, S / 2, bottom)[0])
  })

  test('shapeMask edges are anti-aliased (fractional coverage on the rim)', () => {
    const mask = shapeMask('Circle', 32, 32, 0, 0)
    let fractional = 0
    for (const v of mask) {
      if (v > 0.01 && v < 0.99) fractional++
    }
    expect(fractional).toBeGreaterThan(16)
  })
})

describe('artwork analysis (analyzer/classifier parity)', () => {
  test('solid plate: uniform background detected, no transparent edges', () => {
    const c = solidPlate(S, 40, 120, 220)
    expect(hasTransparentEdges(c)).toBe(false)
    const bg = tryDetectBackground(c)
    expect(bg).not.toBeNull()
    expect(Math.abs(bg!.r - 40) + Math.abs(bg!.g - 120) + Math.abs(bg!.b - 220)).toBeLessThan(6)
  })

  test('circle logo: transparent edges, no plate, matches Circle', () => {
    const c = circleLogo(S)
    expect(hasTransparentEdges(c)).toBe(true)
    expect(matchesShape(c, 'Circle')).toBe(true)
    expect(matchesShape(c, 'Apple')).toBe(false)
    const sb = solidBounds(c)
    expect(sb).not.toBeNull()
  })

  test('glyph on plate: foreground bounds isolate the glyph', () => {
    const c = glyphOnPlate(S)
    const fg = analysis.foregroundBounds(c)
    expect(fg).not.toBeNull()
    const q = Math.floor(S / 4)
    expect(fg!.left).toBeGreaterThanOrEqual(q - 3)
    expect(fg!.right).toBeLessThanOrEqual(S - q + 3)
  })

  test('maxScaleInside: a square silhouette cannot fill a circle', () => {
    const c = solidPlate(S, 10, 10, 10)
    const scale = maxScaleInside(c, findContentBounds(c), 'Circle')
    expect(scale).toBeLessThan(0.78) // 1/√2 ≈ 0.707 + binary-search granularity
    expect(scale).toBeGreaterThan(0.6)
  })
})

describe('tile composer (TileRenderer parity)', () => {
  test('plated icon is rebuilt: shape corners transparent, plate colour survives inside', () => {
    const tile = renderTile(glyphOnPlate(S), config(), false, false, S)
    expect(px(tile, 1, 1)[3]).toBe(0) // outside the Apple squircle
    const inside = px(tile, 10, Math.floor(S / 2))
    expect(inside[3]).toBeGreaterThan(200)
    expect(Math.abs(inside[0] - 40)).toBeLessThan(20)
    expect(Math.abs(inside[2] - 220)).toBeLessThan(20)
  })

  test('round logo with Circle passes through untouched (no plate, own pixels)', () => {
    const tile = renderTile(circleLogo(S), config({ shape: 'Circle' }), false, false, S)
    const centre = px(tile, Math.floor(S / 2), Math.floor(S / 2))
    expect(centre[0]).toBeGreaterThan(180) // the logo's own red
    expect(px(tile, 1, 1)[3]).toBe(0)
  })

  test('bare logo gets the white plate', () => {
    // An irregular sparse glyph on transparency (not shape-matching).
    const c = makeRaster(S)
    for (let y = 20; y < 28; y++) {
      for (let x = 8; x < 56; x++) {
        const i4 = (y * S + x) * 4
        c.data[i4] = 20
        c.data[i4 + 1] = 20
        c.data[i4 + 2] = 20
        c.data[i4 + 3] = 255
      }
    }
    const tile = renderTile(c, config(), false, false, S)
    const plate = px(tile, Math.floor(S / 2), 8)
    expect(plate[3]).toBeGreaterThan(200)
    expect(plate[0]).toBeGreaterThan(240) // white plate above the glyph band
  })

  test('failed-extraction artwork (one stray pixel) renders nothing', () => {
    // The oracle's empty guard (codex render #4): SolidBounds null + content
    // bounds ≤1px → no ghost white plate. A fully transparent canvas keeps the
    // C# behaviour (content bounds = whole canvas → bare-logo plate branch).
    const c = makeRaster(S)
    c.data[(5 * S + 5) * 4 + 3] = 60 // one faint stray pixel, below solid alpha
    const tile = renderTile(c, config(), false, false, S)
    let visible = 0
    for (let i = 3; i < tile.data.length; i += 4) {
      if (tile.data[i] > 0) visible++
    }
    expect(visible).toBe(0)
  })

  test('黑白 makes the tile grayscale', () => {
    const tile = renderTile(glyphOnPlate(S), config({ subject: 'BlackWhite' }), false, false, S)
    const [r, g, b, a] = px(tile, 10, Math.floor(S / 2))
    expect(a).toBeGreaterThan(200)
    expect(r).toBe(g)
    expect(g).toBe(b)
  })

  test('单色 maps plate and glyph to two tones of the tint hue', () => {
    const tile = renderTile(glyphOnPlate(S), config({ subject: 'Mono' }), false, false, S)
    const plate = px(tile, 10, Math.floor(S / 2))
    const glyph = px(tile, Math.floor(S / 2), Math.floor(S / 2))
    const plateL = perceivedLightness(plate[0], plate[1], plate[2])
    const glyphL = perceivedLightness(glyph[0], glyph[1], glyph[2])
    expect(Math.abs(plateL - glyphL)).toBeGreaterThan(0.3) // duotone separation
  })

  test('Keep draws the classic arrow at the bottom-left', () => {
    const tile = renderTile(glyphOnPlate(S), config({ distinction: 'Keep' }), true, false, S)
    const arrowPlate = px(tile, 4, S - 6)
    expect(arrowPlate[0]).toBeGreaterThan(200) // #F4F4F1 plate
    expect(arrowPlate[3]).toBeGreaterThan(200)
  })

  test('showOriginal returns the raw artwork + arrow', () => {
    const tile = renderTile(glyphOnPlate(S), config(), true, true, S)
    const corner = px(tile, 1, 1)
    expect(Math.abs(corner[0] - 40)).toBeLessThan(12) // original plate, unclipped
    const arrow = px(tile, 4, S - 6)
    expect(arrow[0]).toBeGreaterThan(200)
  })

  test('mark Ring insets the card and paints the rim in the mark colour', () => {
    const tile = renderTile(
      glyphOnPlate(S),
      config({ distinction: 'Mark', markStyle: 'Ring', markColor: '#00A040' }),
      true,
      false,
      S,
    )
    // On the shape rim (inside tileAlpha, outside the inset card) the ring shows.
    const rim = px(tile, Math.floor(S / 2), 1)
    expect(rim[3]).toBeGreaterThan(150)
    expect(rim[1]).toBeGreaterThan(rim[0]) // green dominates
  })
})

describe('filters + resampler', () => {
  test('sticker draws a white die-cut border outside the content', () => {
    const tile = renderTile(circleLogo(S), config({ shape: 'Circle', filter: 'Sticker' }), false, false, S)
    // Scan the horizontal centre for a bright near-white run outside the shrunken logo.
    let whiteRun = 0
    const y = Math.floor(S / 2)
    for (let x = 0; x < S; x++) {
      const [r, g, b, a] = px(tile, x, y)
      if (a > 200 && r > 240 && g > 240 && b > 235) whiteRun++
    }
    expect(whiteRun).toBeGreaterThan(3)
  })

  test('pixel filter hard-cuts alpha into blocks', () => {
    const tile = renderTile(glyphOnPlate(S), config({ filter: 'Pixel' }), false, false, S)
    for (let i = 3; i < tile.data.length; i += 4) {
      expect(tile.data[i] === 0 || tile.data[i] === 255).toBe(true)
    }
  })

  test('glass is the translucent liquid slab (owner restore 2026-07-10)', () => {
    // The T7 rim rework lost the 透明玻璃 look; the owner ordered the original
    // slab back: translucent body, frosted subject knockout, grounding halo.
    const plain = renderTile(glyphOnPlate(S), config({}), false, false, S)
    const tile = renderTile(glyphOnPlate(S), config({ filter: 'Glass' }), false, false, S)
    // TRANSLUCENCY is the point: a large share of the slab's pixels sit below
    // full opacity (the plate body rides ~0.44-0.7 alpha).
    let translucent = 0
    let opaqueish = 0
    for (let i = 0; i < tile.data.length; i += 4) {
      if (plain.data[i + 3] < 200) continue
      opaqueish++
      if (tile.data[i + 3] < 200) translucent++
    }
    // (share depends on glyph/edge coverage in the fixture; the rim-era glass
    // scored 0 here — any real share proves the slab is back.)
    expect(translucent).toBeGreaterThan(opaqueish * 0.2)
    // The frosted subject still reads: near-white glyph pixels at high alpha.
    let frosted = 0
    for (let y = 0; y < S; y++) {
      for (let x = 0; x < S; x++) {
        const i4 = (y * S + x) * 4
        if (tile.data[i4 + 3] > 210 && tile.data[i4] > 230 && tile.data[i4 + 1] > 230 && tile.data[i4 + 2] > 230) frosted++
      }
    }
    expect(frosted).toBeGreaterThan(20)
  })

  test('chamfer inside/outside sign convention', () => {
    const c = circleLogo(32)
    const inside = chamferDistance(c, 32, true)
    const outside = chamferDistance(c, 32, false)
    expect(inside[0]).toBe(-1) // transparent corner
    expect(outside[0]).toBeGreaterThan(0)
    const centre = 16 * 32 + 16
    expect(inside[centre]).toBeGreaterThan(0)
    expect(outside[centre]).toBe(-1)
  })

  test('downscale preserves solid colour and full alpha', () => {
    const small = downscale(solidPlate(64, 200, 100, 50), 16)
    const [r, g, b, a] = px(small, 8, 8)
    expect(a).toBe(255)
    expect(Math.abs(r - 200)).toBeLessThanOrEqual(1)
    expect(Math.abs(g - 100)).toBeLessThanOrEqual(1)
    expect(Math.abs(b - 50)).toBeLessThanOrEqual(1)
  })
})
