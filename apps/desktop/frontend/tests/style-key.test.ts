import { describe, expect, test } from 'bun:test'
import type { ConfigDto } from '../src/bridge/types'
import { tileStyleKey } from '../src/icon-compositor/icon-renderer'

// Every pixel-affecting config axis MUST move the style cache key (codex #1:
// kindShapes was missing and served stale uniform previews after a toggle).

const BASE: ConfigDto = {
  shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'derived', shortcutShape: null,
  monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'None', markStyle: 'Shadow',
  markColor: null, plateColor: null, size: 'Mid', filter: 'None',
}

describe('tileStyleKey axis coverage', () => {
  const variants: Array<[string, Partial<ConfigDto>]> = [
    ['shape', { shape: 'Circle' }],
    ['subject', { subject: 'BlackWhite' as const }],
    ['plateFallback', { plateFallback: 'white' as const }],
    ['plateBand', { plateBand: 'Quiet' }],
    ['shortcutShape', { shortcutShape: 'Circle' as const }],
    ['monoStyle', { monoStyle: 'Flat' }],
    ['tint', { tint: '#3FB6A8' }],
    ['distinction', { distinction: 'Mark' }],
    ['markStyle', { markStyle: 'Ring' }],
    ['markColor', { markColor: '#141414' }],
    ['plateColor', { plateColor: '#FFFFFF' }],
    ['filter', { filter: 'Gloss' }],
  ]
  for (const [axis, change] of variants) {
    test(`${axis} changes the key`, () => {
      expect(tileStyleKey({ ...BASE, ...change }, false, false, 48)).not.toBe(
        tileStyleKey(BASE, false, false, 48),
      )
    })
  }

  test('isShortcut and size change the key; showOriginal collapses style detail', () => {
    expect(tileStyleKey(BASE, true, false, 48)).not.toBe(tileStyleKey(BASE, false, false, 48))
    expect(tileStyleKey(BASE, false, false, 96)).not.toBe(tileStyleKey(BASE, false, false, 48))
    expect(tileStyleKey(BASE, false, true, 48)).toBe(tileStyleKey({ ...BASE, shape: 'Circle' }, false, true, 48))
  })
})
