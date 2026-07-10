import { describe, expect, test } from 'bun:test'
import { computeHueSpread } from '../src/icon-compositor/hue-spread'
import type { SpreadEntry } from '../src/icon-compositor/hue-spread'
import { fieldPlateTone, toOkLab } from '../src/icon-compositor/color'
import { fromRgbInt, hexToInt } from '../src/icon-compositor/raster'

// The cross-icon hue-spread pass (ADR-0016; designer acceptance item 3):
// colliding plate hues pull apart deterministically; identical artwork keeps
// identical plates; brand hues never rotate past the cap.

function hueDeg(hex: string): number {
  const c = fromRgbInt(hexToInt(hex))
  const lab = toOkLab(c.r, c.g, c.b)
  return (Math.atan2(lab.B, lab.A) * 180) / Math.PI
}

function plateDistance(hexA: string, hexB: string): number {
  const a = fieldPlateTone(fromRgbInt(hexToInt(hexA)), 'Vivid')
  const b = fieldPlateTone(fromRgbInt(hexToInt(hexB)), 'Vivid')
  const la = toOkLab(a.r, a.g, a.b)
  const lb = toOkLab(b.r, b.g, b.b)
  return Math.hypot(la.L - lb.L, la.A - lb.A, la.B - lb.B)
}

const BLUE_PILE: SpreadEntry[] = [
  { id: 'outlook', artKey: 'a/outlook.png', seed: '#1B6FD4' },
  { id: 'skype', artKey: 'a/skype.png', seed: '#1E78D8' },
  { id: 'twitter', artKey: 'a/twitter.png', seed: '#1D9BF0' },
  { id: 'onedrive', artKey: 'a/onedrive.png', seed: '#1668C7' },
]

describe('computeHueSpread', () => {
  test('colliding hues pull apart, improving pairwise plate distance', () => {
    const spread = computeHueSpread(BLUE_PILE)
    const ids = BLUE_PILE.map((e) => e.id)
    let minBefore = Infinity
    let minAfter = Infinity
    for (let i = 0; i < ids.length; i++) {
      for (let j = i + 1; j < ids.length; j++) {
        minBefore = Math.min(minBefore, plateDistance(BLUE_PILE[i].seed!, BLUE_PILE[j].seed!))
        minAfter = Math.min(minAfter, plateDistance(spread.get(ids[i])!, spread.get(ids[j])!))
      }
    }
    expect(minAfter).toBeGreaterThan(minBefore)
    expect(minAfter).toBeGreaterThan(0.008) // separable at the plate level
  })

  test('rotation never exceeds the brand cap (~18 degrees)', () => {
    const spread = computeHueSpread(BLUE_PILE)
    for (const e of BLUE_PILE) {
      let delta = Math.abs(hueDeg(spread.get(e.id)!) - hueDeg(e.seed!))
      if (delta > 180) delta = 360 - delta
      expect(delta).toBeLessThanOrEqual(19)
    }
  })

  test('identical artwork keeps identical plates', () => {
    const entries: SpreadEntry[] = [
      { id: 'doc1', artKey: 'a/docx.png', seed: '#2B5CAD' },
      { id: 'doc2', artKey: 'a/docx.png', seed: '#2B5CAD' },
      { id: 'doc3', artKey: 'a/docx.png', seed: '#2B5CAD' },
      { id: 'app', artKey: 'a/word.png', seed: '#2D5FB0' },
    ]
    const spread = computeHueSpread(entries)
    expect(spread.get('doc1')).toBe(spread.get('doc2')!)
    expect(spread.get('doc2')).toBe(spread.get('doc3')!)
    expect(spread.get('app')).not.toBe(spread.get('doc1')!)
  })

  test('a PAIR straddling the +/-pi hue seam still pulls apart (codex #7)', () => {
    // Two magenta-ish seeds on opposite signs of the atan2 seam, ~3 deg apart.
    const entries: SpreadEntry[] = [
      { id: 'a', artKey: 'a.png', seed: '#E84FB0' },
      { id: 'b', artKey: 'b.png', seed: '#E44FB8' },
    ]
    const spread = computeHueSpread(entries)
    let gap = Math.abs(hueDeg(spread.get('a')!) - hueDeg(spread.get('b')!))
    if (gap > 180) gap = 360 - gap
    expect(gap).toBeGreaterThan(6) // clearly separated versus the ~1-3 deg input
  })

  test('deterministic: same input, same output; lone hues untouched', () => {
    const entries: SpreadEntry[] = [
      { id: 'x', artKey: 'x.png', seed: '#E23A2E' }, // lone red
      ...BLUE_PILE,
      { id: 'null', artKey: 'n.png', seed: null },
    ]
    const a = computeHueSpread(entries)
    const b = computeHueSpread([...entries].reverse())
    expect([...a.entries()].sort()).toEqual([...b.entries()].sort())
    expect(a.get('x')).toBe('#E23A2E')
    expect(a.has('null')).toBe(false)
  })
})
