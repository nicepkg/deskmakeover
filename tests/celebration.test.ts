import { beforeEach, describe, expect, test } from 'bun:test'
import { claimCelebration, resetCelebrationLedger } from '../src/components/common/confetti'

// Celebration gate contracts (codex R6): per-module dedupe for the owner-shipped
// icons/wallpaper behaviour, PLUS the spec 08 §4 launch-first gate the calm
// module rides ("confetti only if this is the launch's first module success").

beforeEach(() => resetCelebrationLedger())

describe('celebration launch ledger', () => {
  test('a module key fires once per launch, repeats stay quiet', () => {
    expect(claimCelebration('icons')).toBe(true)
    expect(claimCelebration('icons')).toBe(false)
  })

  test('per-module keys stay independent (owner-shipped icons/wallpaper grammar)', () => {
    expect(claimCelebration('icons')).toBe(true)
    expect(claimCelebration('wallpaper')).toBe(true)
  })

  test('launch-first: a SECOND module success never celebrates (spec 08 §4)', () => {
    expect(claimCelebration('icons')).toBe(true)
    expect(claimCelebration('calm-apply', true)).toBe(false) // another module already did
  })

  test('launch-first: fires when it truly is the first success of the launch', () => {
    expect(claimCelebration('calm-apply', true)).toBe(true)
    expect(claimCelebration('calm-apply', true)).toBe(false) // and only once
  })
})
