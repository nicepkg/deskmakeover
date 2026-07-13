import { describe, expect, test } from 'bun:test'
import { CALM_CATALOG, controlById, groupOf } from '../src/lib/calm/catalog'
import {
  COUNTED_AS_OURS,
  HELD_STATES,
  assertTransition,
  canTransition,
  type CalmRowState,
} from '../src/lib/calm/states'

// Spec 08 §3 + ADR-0023 D3: the honest-state grammar is load-bearing product law.
// These tests pin the grammar so no later refactor can quietly let a row lie.

const ALL_STATES: CalmRowState[] = [
  'unknown', 'quiet', 'pushing', 'pending', 'verified', 'setAwaiting', 'reverted',
  'needsReconfirm', 'unsupported', 'managed', 'confirmedOff', 'userAttested',
]

describe('calm state machine', () => {
  test('verified is reachable ONLY through the pending pipeline (writable rows)', () => {
    for (const from of ALL_STATES) {
      const legal = canTransition('writable', from, 'verified')
      expect(legal).toBe(from === 'pending' || from === 'setAwaiting')
    }
  })

  test('a probed pushing row cannot jump straight to verified', () => {
    expect(canTransition('writable', 'pushing', 'verified')).toBe(false)
    expect(() => assertTransition('writable', 'pushing', 'verified')).toThrow()
  })

  test('guided rows can never enter the write pipeline', () => {
    for (const from of ALL_STATES) {
      expect(canTransition('guided', from, 'pending')).toBe(false)
      expect(canTransition('guided', from, 'verified')).toBe(false)
      expect(canTransition('guided', from, 'setAwaiting')).toBe(false)
    }
  })

  test('feature-update boundary: verified may drop to needsReconfirm, and needsReconfirm never re-applies directly', () => {
    expect(canTransition('writable', 'verified', 'needsReconfirm')).toBe(true)
    expect(canTransition('writable', 'needsReconfirm', 'pending')).toBe(false)
    expect(canTransition('writable', 'needsReconfirm', 'verified')).toBe(false)
  })

  test('only verified counts as ours — guided outcomes never do', () => {
    expect([...COUNTED_AS_OURS]).toEqual(['verified'])
    expect(COUNTED_AS_OURS.has('userAttested' as CalmRowState)).toBe(false)
    expect(COUNTED_AS_OURS.has('confirmedOff' as CalmRowState)).toBe(false)
    expect(COUNTED_AS_OURS.has('setAwaiting' as CalmRowState)).toBe(false)
  })
})

describe('calm catalog invariants', () => {
  test('ids are unique', () => {
    const ids = CALM_CATALOG.map((c) => c.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  test('guided rows carry a route and never sit in the default package', () => {
    for (const c of CALM_CATALOG.filter((c) => c.tier === 'guided')) {
      expect(c.routeKey).toBeTruthy()
      expect(c.inDefaultPackage).toBe(false)
      expect(typeof c.readableState).toBe('boolean')
    }
  })

  test('the starter write slice is exactly the three ADR-0023 D6 candidates', () => {
    const slice = CALM_CATALOG.filter((c) => c.starterSlice).map((c) => c.id).sort()
    expect(slice).toEqual(['start.recommendations', 'taskbar.search', 'taskbar.taskview'])
    for (const id of slice) {
      const c = controlById(id)
      expect(c.tier).toBe('automatic')
      expect(c.inDefaultPackage).toBe(true)
    }
  })

  test('admission rule d: sync-provider notifications disclose their collateral on the row face', () => {
    expect(controlById('explorer.syncNotifications').collateralKey).toBeTruthy()
  })

  test('non-evaluable controls are absent from the v1 catalog (ad ID / Device Usage — ADR-0023 D5)', () => {
    const ids = CALM_CATALOG.map((c) => String(c.id))
    for (const banned of ['advertising', 'adId', 'deviceUsage', 'intent']) {
      expect(ids.some((id) => id.toLowerCase().includes(banned.toLowerCase()))).toBe(false)
    }
  })

  test('the widgets feed leads the guided group (the opening act)', () => {
    const guided = CALM_CATALOG.filter((c) => c.tier === 'guided')
    expect(guided[0]?.id).toBe('widgets.feed')
  })

  test('groupOf: writable rows fall to held only when the environment says so', () => {
    const c = controlById('taskbar.search')
    expect(groupOf(c, 'pushing')).toBe('oneClick')
    expect(groupOf(c, 'verified')).toBe('oneClick')
    for (const held of HELD_STATES) expect(groupOf(c, held)).toBe('held')
    expect(groupOf(controlById('widgets.feed'), 'pushing')).toBe('guided')
  })
})
