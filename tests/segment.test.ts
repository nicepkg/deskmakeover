import { describe, expect, test } from 'bun:test'
import { makeRaster } from '../src/icon-compositor/raster'
import type { Raster } from '../src/icon-compositor/raster'
import { segmentSubject } from '../src/icon-compositor/segment'

// Synthetic fixtures mirroring the Python prototype's validation set
// (scratchpad segment-proto3): alpha silhouettes, plate+glyph splits and
// gradient backgrounds must all segment the way the prototype proved.

const S = 64

function fill(c: Raster, x0: number, y0: number, x1: number, y1: number, r: number, g: number, b: number): void {
  for (let y = y0; y < y1; y++) {
    for (let x = x0; x < x1; x++) {
      const i4 = (y * c.width + x) * 4
      c.data[i4] = r
      c.data[i4 + 1] = g
      c.data[i4 + 2] = b
      c.data[i4 + 3] = 255
    }
  }
}

function maskAt(mask: Uint8Array, x: number, y: number): number {
  return mask[y * S + x]
}

describe('segmentSubject', () => {
  test('transparent-edge logo: subject = the opaque silhouette', () => {
    const c = makeRaster(S)
    fill(c, 16, 16, 48, 48, 200, 60, 60) // floating square logo
    const { mask, mode } = segmentSubject(c)
    expect(mode).toBe('alpha')
    expect(maskAt(mask, 32, 32)).toBe(1)
    expect(maskAt(mask, 4, 4)).toBe(0)
  })

  test('full-bleed plate + glyph: the flood alone isolates the glyph', () => {
    const c = makeRaster(S)
    fill(c, 0, 0, S, S, 40, 120, 220) // blue plate to the canvas edge
    fill(c, 20, 20, 44, 44, 250, 250, 250) // white glyph
    const { mask, mode } = segmentSubject(c)
    expect(mode).toBe('flood')
    expect(maskAt(mask, 32, 32)).toBe(1) // glyph = subject
    expect(maskAt(mask, 8, 8)).toBe(0) // plate = background
  })

  test('floating plate + glyph (Photos case): the plate splits, ink = subject', () => {
    const c = makeRaster(S)
    fill(c, 6, 6, S - 6, S - 6, 245, 245, 245) // white plate on transparency
    fill(c, 22, 26, 44, 44, 30, 80, 160) // blue mountain-ish glyph
    const { mask, mode } = segmentSubject(c)
    expect(mode).toBe('alpha+split')
    expect(maskAt(mask, 32, 34)).toBe(1) // glyph = subject
    expect(maskAt(mask, 12, 12)).toBe(0) // plate field = background
    expect(maskAt(mask, 2, 2)).toBe(0) // transparent corner = background
  })

  test('gradient background: the flood follows the ramp, the subject survives', () => {
    const c = makeRaster(S)
    for (let y = 0; y < S; y++) {
      const v = 40 + Math.round((y / (S - 1)) * 160) // smooth vertical ramp
      fill(c, 0, y, S, y + 1, v, v, v + 20)
    }
    fill(c, 22, 22, 42, 42, 220, 40, 40) // red subject in the middle
    const { mask, mode } = segmentSubject(c)
    expect(mode.startsWith('flood')).toBe(true)
    expect(maskAt(mask, 32, 32)).toBe(1)
    expect(maskAt(mask, 6, 6)).toBe(0)
    expect(maskAt(mask, 6, S - 6)).toBe(0) // far end of the ramp still background
  })

  test('inset plate (transparent margin) exposes the plate colour as field', () => {
    const c = makeRaster(S)
    fill(c, 8, 8, S - 8, S - 8, 40, 120, 220) // blue plate INSET on transparency
    fill(c, 22, 22, 42, 42, 250, 250, 250) // white glyph
    const seg = segmentSubject(c)
    expect(seg.mode).toBe('alpha+split')
    expect(seg.field).toBeDefined()
    expect(seg.field!.b).toBeGreaterThan(seg.field!.r) // plate = blue, not the white glyph
    expect(Math.abs(seg.field!.r - 40) + Math.abs(seg.field!.b - 220)).toBeLessThan(40)
  })

  test('gradient plate exposes NO field (owner: 不 hotfix 渐变)', () => {
    const c = makeRaster(S)
    // radial gradient rounded-square plate + white subject: shape is fine but
    // the body is not flat, so it must NOT be hotfixed.
    const cx = S / 2, cy = S / 2
    for (let y = 6; y < S - 6; y++) for (let x = 6; x < S - 6; x++) {
      const d = Math.hypot(x - cx, y - cy) / (S / 2)
      const v = Math.round(60 + d * 150) // dark centre → light edge (strong gradient)
      const i4 = (y * S + x) * 4
      c.data[i4] = 30; c.data[i4 + 1] = 80; c.data[i4 + 2] = v; c.data[i4 + 3] = 255
    }
    for (let y = 26; y < 38; y++) for (let x = 26; x < 38; x++) {
      const i4 = (y * S + x) * 4
      c.data[i4] = 250; c.data[i4 + 1] = 250; c.data[i4 + 2] = 250
    }
    expect(segmentSubject(c).field).toBeUndefined()
  })

  test('irregular (non square/round) plate exposes NO field (owner: 绝对形状)', () => {
    const c = makeRaster(S)
    // an L / notch shape that fills a square bbox poorly — low shape IoU.
    for (let y = 8; y < S - 8; y++) for (let x = 8; x < S - 8; x++) {
      if (x > S / 2 && y > S / 2) continue // cut a whole quadrant → not square/round
      const i4 = (y * S + x) * 4
      c.data[i4] = 40; c.data[i4 + 1] = 120; c.data[i4 + 2] = 220; c.data[i4 + 3] = 255
    }
    for (let y = 14; y < 26; y++) for (let x = 14; x < 26; x++) {
      const i4 = (y * S + x) * 4
      c.data[i4] = 250; c.data[i4 + 1] = 250; c.data[i4 + 2] = 250
    }
    expect(segmentSubject(c).field).toBeUndefined()
  })

  test('full-canvas plate (flood mode) withholds field — tryDetectBackground owns it', () => {
    const c = makeRaster(S)
    fill(c, 0, 0, S, S, 40, 120, 220)
    fill(c, 20, 20, 44, 44, 250, 250, 250)
    expect(segmentSubject(c).field).toBeUndefined()
  })

  test('bare logo without a plate exposes NO field', () => {
    const c = makeRaster(S)
    fill(c, 16, 16, 48, 48, 200, 60, 60) // floating square logo on transparency
    expect(segmentSubject(c).field).toBeUndefined()
  })

  test('featureless plate: nothing to segment, mask stays empty', () => {
    const c = makeRaster(S)
    fill(c, 0, 0, S, S, 90, 90, 90)
    const { mask } = segmentSubject(c)
    let solid = 0
    for (const v of mask) solid += v
    expect(solid).toBeLessThan(S * S * 0.05)
  })

  test('uniform silhouette without internal contrast keeps the whole silhouette', () => {
    const c = makeRaster(S)
    fill(c, 12, 12, 52, 52, 240, 190, 60) // folder-like solid block on transparency
    const { mask, mode } = segmentSubject(c)
    expect(mode).toBe('alpha') // no +split: unimodal colour, guard rejects
    expect(maskAt(mask, 32, 32)).toBe(1)
  })
})
