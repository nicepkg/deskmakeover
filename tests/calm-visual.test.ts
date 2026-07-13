import { describe, expect, test } from 'bun:test'
import { highlightVisual, noiseGone } from '../src/components/calm/schematic-parts'
import type { CalmRowState } from '../src/lib/calm/states'

// Schematic honesty contract (spec 08 §2.1, codex R3 #2): the highlight and the
// noise must agree. A frame drawn around noise that has already left the surface
// is a ghost socket; a vanished frame over still-present noise hides the WHERE.

const ALL_STATES: CalmRowState[] = [
  'unknown',
  'quiet',
  'pushing',
  'pending',
  'verified',
  'setAwaiting',
  'reverted',
  'needsReconfirm',
  'external',
  'unsupported',
  'managed',
  'confirmedOff',
  'userAttested',
]

describe('schematic honesty contract', () => {
  test('every noise-gone state renders NO highlight — no frame around removed noise', () => {
    for (const s of ALL_STATES) {
      if (noiseGone(s)) expect(highlightVisual(s)).toBe('done')
    }
  })

  test('done is exclusive to noise-gone states — the highlight never vanishes while noise is on screen', () => {
    for (const s of ALL_STATES) {
      if (highlightVisual(s) === 'done') expect(noiseGone(s)).toBe(true)
    }
  })
})
