import { beforeEach, describe, expect, test } from 'bun:test'
import { claimCelebration, resetCelebrationLedger } from '../src/components/common/confetti'

// Celebration ledger contract (spec 02 §Ceremony "the ONE-per-launch celebration
// confetti"; spec 08 §4 "only if this is the launch's first module success"):
// exactly ONE confetti moment per app launch, across ALL modules — whichever
// module lands the first successful apply claims it (codex R7).

beforeEach(() => resetCelebrationLedger())

describe('celebration launch ledger', () => {
  test('the first claim of a launch wins; a repeat stays quiet', () => {
    expect(claimCelebration()).toBe(true)
    expect(claimCelebration()).toBe(false)
  })

  test('icons first → calm and wallpaper stay quiet', () => {
    expect(claimCelebration()).toBe(true) // icons apply
    expect(claimCelebration()).toBe(false) // calm apply
    expect(claimCelebration()).toBe(false) // wallpaper apply
  })

  test('calm first → icons and wallpaper stay quiet (the symmetric direction)', () => {
    expect(claimCelebration()).toBe(true) // calm apply
    expect(claimCelebration()).toBe(false) // icons apply
    expect(claimCelebration()).toBe(false) // wallpaper apply
  })

  test('a new launch (ledger reset) celebrates again', () => {
    expect(claimCelebration()).toBe(true)
    resetCelebrationLedger()
    expect(claimCelebration()).toBe(true)
  })
})
