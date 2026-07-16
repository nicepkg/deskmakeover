import { describe, expect, test } from 'bun:test'
import { CALM_CATALOG, controlById, groupOf } from '../src/lib/calm/catalog'
import {
  COUNTED_AS_OURS,
  HELD_STATES,
  OWNED_WRITES,
  RESTORABLE_LEDGER,
  assertTransition,
  canTransition,
  probeTransition,
  type CalmRowState,
} from '../src/lib/calm/states'

// Spec 08 §3 + ADR-0023 D3: the honest-state grammar is load-bearing product law.
// These tests pin the grammar so no later refactor can quietly let a row lie.

const ALL_STATES: CalmRowState[] = [
  'unknown', 'quiet', 'pushing', 'pending', 'verified', 'setAwaiting', 'reopened', 'reverted',
  'needsReconfirm', 'external', 'unsupported', 'managed', 'confirmedOff', 'userAttested',
]

describe('calm state machine (write pipeline)', () => {
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

  test('a skipped write returns to pushing; a drifted restore lands external, never verified', () => {
    expect(canTransition('writable', 'pending', 'pushing')).toBe(true) // skipped, no write
    expect(canTransition('writable', 'verified', 'external')).toBe(true)
    expect(canTransition('writable', 'setAwaiting', 'external')).toBe(true)
    expect(canTransition('writable', 'external', 'verified')).toBe(false)
  })

  test('only verified counts as ours — guided/external/awaiting outcomes never do', () => {
    expect([...COUNTED_AS_OURS]).toEqual(['verified'])
    for (const s of ['userAttested', 'confirmedOff', 'setAwaiting', 'external', 'reopened'] as CalmRowState[]) {
      expect(COUNTED_AS_OURS.has(s)).toBe(false)
    }
    // Restore gating: setAwaiting IS an owned write even though not yet counted.
    expect(OWNED_WRITES.has('setAwaiting')).toBe(true)
    expect(OWNED_WRITES.has('verified')).toBe(true)
    expect(OWNED_WRITES.has('external')).toBe(false)
  })

  test('reopened: a candidate with provenance — a ledger row, never a claim, never an intact write', () => {
    // Re-enters the write pipeline; restore-skip disowns it to external.
    expect(canTransition('writable', 'reopened', 'pending')).toBe(true)
    expect(canTransition('writable', 'reopened', 'external')).toBe(true)
    expect(canTransition('writable', 'reopened', 'verified')).toBe(false) // only via pending
    // Not counted, not an intact write — but the global Restore CAN act on it.
    expect(COUNTED_AS_OURS.has('reopened')).toBe(false)
    expect(OWNED_WRITES.has('reopened')).toBe(false)
    expect(RESTORABLE_LEDGER.has('reopened')).toBe(true)
  })
})

describe('calm probe channel (ledger truth)', () => {
  test('an owned intact row re-probes back to verified — ownership survives re-probe', () => {
    expect(probeTransition('writable', 'unknown', { state: 'quiet', ownedByUs: true })).toBe('verified')
    expect(probeTransition('writable', 'verified', { state: 'quiet', ownedByUs: true })).toBe('verified')
  })

  test('a probe can never mint verified without ownership', () => {
    expect(probeTransition('writable', 'unknown', { state: 'quiet' })).toBe('quiet')
    expect(probeTransition('writable', 'pushing', { state: 'quiet' })).toBe('quiet')
  })

  test('ownership with a moved value is drift, not a claim', () => {
    expect(probeTransition('writable', 'verified', { state: 'pushing', ownedByUs: true })).toBe('pushing')
  })

  test('HealthCheck drift probes to reopened — provenance rides in the state machine', () => {
    expect(probeTransition('writable', 'verified', { state: 'pushing', driftedFromUs: true })).toBe('reopened')
    expect(probeTransition('writable', 'unknown', { state: 'pushing', driftedFromUs: true })).toBe('reopened')
    expect(probeTransition('writable', 'reopened', { state: 'pushing', driftedFromUs: true })).toBe('reopened') // idempotent
    // The boundary still outranks drift provenance.
    expect(probeTransition('writable', 'reopened', { state: 'needsReconfirm', driftedFromUs: true })).toBe('needsReconfirm')
  })

  test('the feature-update boundary is reachable via probe', () => {
    expect(probeTransition('writable', 'unknown', { state: 'needsReconfirm' })).toBe('needsReconfirm')
    expect(probeTransition('writable', 'verified', { state: 'needsReconfirm' })).toBe('needsReconfirm')
  })

  test('a crossed boundary outranks a stale ownership claim', () => {
    expect(probeTransition('writable', 'verified', { state: 'needsReconfirm', ownedByUs: true })).toBe('needsReconfirm')
  })

  test('guided probes never yield write-pipeline states and keep settled outcomes', () => {
    expect(probeTransition('guided', 'confirmedOff', { state: 'quiet' })).toBe('confirmedOff')
    expect(probeTransition('guided', 'userAttested', { state: 'quiet' })).toBe('userAttested')
    expect(probeTransition('guided', 'confirmedOff', { state: 'pushing' })).toBe('pushing') // Windows re-enabled it
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

  test('the widgets family leads the guided group (the opening act)', () => {
    const guided = CALM_CATALOG.filter((c) => c.tier === 'guided')
    expect(guided[0]?.id).toBe('widgets.feed')
  })

  test('groupOf: an uncertified automatic row we can walk to lands in guided, never a dead held row', () => {
    // ADR-0023 D2 group 2 (owner 2026-07-16): a fail-closed automatic row whose official page we
    // know is a WALK 「带你去系统里关的」, not a dead 「本版本不支持」 end. Certified → one-click;
    // org-managed → held (the Settings toggle is greyed); routeless → held (nowhere to walk).
    const search = controlById('taskbar.search') // carries a manual route
    expect(groupOf(search, 'pushing')).toBe('oneClick')
    expect(groupOf(search, 'verified')).toBe('oneClick')
    expect(groupOf(search, 'external')).toBe('oneClick')
    expect(groupOf(search, 'needsReconfirm')).toBe('oneClick')
    expect(groupOf(search, 'unsupported')).toBe('guided') // uncertified but routable → walk
    expect(groupOf(search, 'managed')).toBe('held') // policy-managed → we never fight policy

    const sync = controlById('explorer.syncNotifications') // NO ms-settings page → routeless
    for (const held of HELD_STATES) expect(groupOf(sync, held)).toBe('held')

    expect(groupOf(controlById('widgets.feed'), 'pushing')).toBe('guided') // guided-tier always guided
  })

  test('every fail-closed automatic row with an official page is walkable (routeKey); the pageless one is not', () => {
    // Frontend mirror of the Rust catalog's manual_route set — drift on either side must be caught.
    const walkable = [
      'start.recommendations', 'taskbar.search', 'taskbar.taskview', 'search.highlights',
      'notifications.suggestions', 'notifications.welcome', 'notifications.finishSetup', 'settings.suggestions',
    ] as const
    for (const id of walkable) expect(controlById(id).routeKey, id).toBeTruthy()
    expect(controlById('explorer.syncNotifications').routeKey).toBeFalsy() // no ms-settings page
  })
})
