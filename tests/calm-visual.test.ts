import { describe, expect, test } from 'bun:test'
import { highlightVisual, noiseGone } from '../src/components/calm/schematic-parts'
import { FRAME_H, FRAME_W, SCHEMATICS } from '../src/lib/calm/schematic-map'
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
  'reopened',
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

  test('a reopened row is ARMED again — its noise is back and the operation area shows', () => {
    expect(noiseGone('reopened')).toBe(false)
    expect(highlightVisual('reopened')).toBe('armed')
  })
})

describe('scene region geometry (copy↔picture agreement)', () => {
  test('every highlight region stays inside the frame', () => {
    for (const [id, spec] of Object.entries(SCHEMATICS)) {
      const { x, y, w, h } = spec.region
      expect(x, id).toBeGreaterThanOrEqual(0)
      expect(y, id).toBeGreaterThanOrEqual(0)
      expect(x + w, id).toBeLessThanOrEqual(FRAME_W)
      expect(y + h, id).toBeLessThanOrEqual(FRAME_H)
    }
  })

  test('the tray region frames the overflowable icons, never the clock/status area (codex R4 #6)', () => {
    const r = SCHEMATICS['tray.entries'].region
    // TrayIcons occupy x81..89.6 in scenes.tsx; the status pills start at x94.
    expect(r.x).toBeLessThanOrEqual(81)
    expect(r.x + r.w).toBeGreaterThanOrEqual(89.6)
    expect(r.x + r.w).toBeLessThan(94)
  })
})
