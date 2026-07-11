import { describe, expect, test } from 'bun:test'
import { ZONE_PRESETS, orientationOfGrid, presetsForOrientation, projectPreset } from '../src/lib/zone-presets'
import { gridForBounds } from '../src/lib/monitor-reconcile'
import type { ZoneDto } from '../src/bridge/types'

// Portrait-native presets (owner 2026-07-12). The whole point is that they COMPOSE
// on a tall grid, not just fit — so the gate is: projected onto the REAL portrait
// grid, every authored zone survives (≥2×2, none silently dropped by projectPreset),
// stays in bounds, and no two zones overlap. Uses the real grid math (gridForBounds),
// not a hardcoded 11×19, so the test tracks the actual desktop.

const PORTRAIT = gridForBounds({ x: 0, y: 0, w: 1080, h: 1920 })
const LANDSCAPE = gridForBounds({ x: 0, y: 0, w: 1920, h: 1080 })

function overlaps(a: ZoneDto, b: ZoneDto): boolean {
  return (
    a.cellX < b.cellX + b.cellsWide &&
    b.cellX < a.cellX + a.cellsWide &&
    a.cellY < b.cellY + b.cellsTall &&
    b.cellY < a.cellY + a.cellsTall
  )
}

describe('orientation split', () => {
  test('every preset is tagged and the picker filters by orientation', () => {
    expect(ZONE_PRESETS.every((p) => p.orientation === 'portrait' || p.orientation === 'landscape')).toBe(true)
    expect(presetsForOrientation('landscape').map((p) => p.id)).toEqual([
      'workbench', 'minimal-duo', 'quadrants', 'side-rail',
    ])
    expect(presetsForOrientation('portrait').map((p) => p.id)).toEqual([
      'ladder', 'horizon', 'focus-split', 'totem',
    ])
    // Ladder leads the portrait set (owner default pick).
    expect(presetsForOrientation('portrait')[0].id).toBe('ladder')
  })

  test('orientationOfGrid follows the taller-than-wide rule', () => {
    expect(orientationOfGrid(PORTRAIT)).toBe('portrait')
    expect(orientationOfGrid(LANDSCAPE)).toBe('landscape')
    expect(orientationOfGrid({ screenWidth: 1000, screenHeight: 1000 })).toBe('landscape') // square = landscape
  })
})

describe('portrait presets project cleanly onto the real portrait grid', () => {
  for (const preset of presetsForOrientation('portrait')) {
    test(`${preset.id}: no zone dropped, all in-bounds, none overlap`, () => {
      const zones = projectPreset(preset, PORTRAIT)

      // Nothing silently dropped: projectPreset filters zones that fell below 2×2,
      // so a survived count < authored count means a fraction was too small.
      expect(zones.length).toBe(preset.zones.length)

      for (const z of zones) {
        expect(z.cellsWide).toBeGreaterThanOrEqual(2)
        expect(z.cellsTall).toBeGreaterThanOrEqual(2)
        expect(z.cellX).toBeGreaterThanOrEqual(0)
        expect(z.cellY).toBeGreaterThanOrEqual(0)
        expect(z.cellX + z.cellsWide).toBeLessThanOrEqual(PORTRAIT.columns)
        expect(z.cellY + z.cellsTall).toBeLessThanOrEqual(PORTRAIT.rows)
      }

      for (let i = 0; i < zones.length; i++) {
        for (let j = i + 1; j < zones.length; j++) {
          expect(overlaps(zones[i], zones[j])).toBe(false)
        }
      }
    })
  }
})

describe('landscape presets still project cleanly (unchanged behavior)', () => {
  for (const preset of presetsForOrientation('landscape')) {
    test(`${preset.id}: survives + in-bounds + non-overlapping on landscape`, () => {
      const zones = projectPreset(preset, LANDSCAPE)
      expect(zones.length).toBe(preset.zones.length)
      for (const z of zones) {
        expect(z.cellsWide).toBeGreaterThanOrEqual(2)
        expect(z.cellsTall).toBeGreaterThanOrEqual(2)
        expect(z.cellX + z.cellsWide).toBeLessThanOrEqual(LANDSCAPE.columns)
        expect(z.cellY + z.cellsTall).toBeLessThanOrEqual(LANDSCAPE.rows)
      }
      for (let i = 0; i < zones.length; i++) {
        for (let j = i + 1; j < zones.length; j++) {
          expect(overlaps(zones[i], zones[j])).toBe(false)
        }
      }
    })
  }
})
