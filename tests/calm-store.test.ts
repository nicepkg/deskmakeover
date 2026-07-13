import { beforeEach, describe, expect, test } from 'bun:test'
import { CALM_CATALOG, type CalmControlId } from '../src/lib/calm/catalog'
import type { CalmRowState } from '../src/lib/calm/states'
import { MockCalmBackend } from '../src/bridge/mock-calm'
import { countOwnedWrites, countQuieted, groupedRows, setCalmBackend, useCalm } from '../src/stores/calm'

// Store behaviour tests over the CalmBackend mock (plan W0.3 + codex R1 fixes).
// The mock's fake environment: starter slice certified, other automatic rows
// fail-closed, guided rows walkable.

function resetStore(backend: MockCalmBackend) {
  setCalmBackend(backend)
  useCalm.setState({
    probed: false,
    op: 'idle',
    rows: Object.fromEntries(CALM_CATALOG.map((c) => [c.id, 'unknown'])) as Record<CalmControlId, CalmRowState>,
    excluded: new Set(),
    walkedId: null,
    lastApply: null,
  })
}

beforeEach(() => resetStore(new MockCalmBackend({ latencyMs: 0 })))

describe('calm store', () => {
  test('probe sorts the catalog into the three honest groups', async () => {
    await useCalm.getState().probe()
    const groups = groupedRows(useCalm.getState().rows)
    expect(groups.oneClick.sort()).toEqual(['start.recommendations', 'taskbar.search', 'taskbar.taskview'])
    expect(groups.guided[0]).toBe('widgets.feed') // the opening act leads
    expect(groups.held.length).toBeGreaterThan(0) // uncertified rows sit honestly in group 3
    expect(useCalm.getState().rows['search.highlights']).toBe('unsupported')
  })

  test('managed rows land in group 3 as managed', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, managed: ['taskbar.search'] }))
    await useCalm.getState().probe()
    expect(useCalm.getState().rows['taskbar.search']).toBe('managed')
    expect(groupedRows(useCalm.getState().rows).held).toContain('taskbar.search')
  })

  test('a crossed certification boundary probes to needsReconfirm and is never re-applied', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, boundaryCrossed: ['taskbar.search'] }))
    await useCalm.getState().probe()
    expect(useCalm.getState().rows['taskbar.search']).toBe('needsReconfirm')
    await useCalm.getState().applyAll()
    expect(useCalm.getState().rows['taskbar.search']).toBe('needsReconfirm') // untouched
    expect(useCalm.getState().lastApply?.verified).toBe(2)
  })

  test('applyAll verifies the certified slice and counts only verified writes', async () => {
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    const s = useCalm.getState()
    expect(s.lastApply).toEqual({ verified: 3, awaiting: 0, reverted: 0, skipped: 0 })
    expect(s.rows['taskbar.search']).toBe('verified')
    expect(countQuieted(s.rows)).toBe(3)
  })

  test('ownership survives a re-probe (ledger truth) — the summary and Restore never vanish', async () => {
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().probe() // module re-entry HealthCheck
    const s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('verified')
    expect(countQuieted(s.rows)).toBe(3)
    expect(countOwnedWrites(s.rows)).toBe(3)
  })

  test('an excluded row is never written', async () => {
    await useCalm.getState().probe()
    useCalm.getState().toggleExcluded('taskbar.search')
    await useCalm.getState().applyAll()
    const s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('pushing')
    expect(s.lastApply?.verified).toBe(2)
  })

  test('a failing write lands reverted, honestly, without poisoning the batch', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, failing: ['taskbar.taskview'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    const s = useCalm.getState()
    expect(s.rows['taskbar.taskview']).toBe('reverted')
    expect(s.lastApply).toEqual({ verified: 2, awaiting: 0, reverted: 1, skipped: 0 })
    expect(countQuieted(s.rows)).toBe(2)
  })

  test('a skipped write returns the row to pushing and is counted as skipped', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, skipping: ['taskbar.taskview'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    const s = useCalm.getState()
    expect(s.rows['taskbar.taskview']).toBe('pushing')
    expect(s.lastApply).toEqual({ verified: 2, awaiting: 0, reverted: 0, skipped: 1 })
  })

  test('total backend failure strands no row in pending', async () => {
    class ExplodingBackend extends MockCalmBackend {
      override async apply(): Promise<never> {
        throw new Error('ipc down')
      }
    }
    resetStore(new ExplodingBackend({ latencyMs: 0 }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    const s = useCalm.getState()
    for (const id of ['start.recommendations', 'taskbar.search', 'taskbar.taskview'] as CalmControlId[]) {
      expect(s.rows[id]).toBe('reverted')
    }
    expect(s.op).toBe('idle')
    // reverted rows stay actionable — the next applyAll retries them
    await useCalm.getState().applyAll()
    expect(useCalm.getState().rows['taskbar.search']).toBe('reverted') // still failing backend, still honest
  })

  test('sign-out rows land setAwaiting: not counted as quieted, but owned (Restore stays)', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, awaiting: ['start.recommendations'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    const s = useCalm.getState()
    expect(s.rows['start.recommendations']).toBe('setAwaiting')
    expect(countQuieted(s.rows)).toBe(2)
    expect(countOwnedWrites(s.rows)).toBe(3)
  })

  test("restore marks hand-edited rows external (mark, don't clobber) — no longer ours, not counted", async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, drifted: ['taskbar.search'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().restoreAll()
    const s = useCalm.getState()
    expect(s.rows['taskbar.taskview']).toBe('pushing') // restored → the surface pushes again
    expect(s.rows['taskbar.search']).toBe('external') // drifted → theirs now, untouched
    expect(countQuieted(s.rows)).toBe(0)
    expect(countOwnedWrites(s.rows)).toBe(0)
  })

  test('guided readable row confirms off after the walk and the walk token clears', async () => {
    await useCalm.getState().probe()
    await useCalm.getState().walkGuided('taskbar.widgetsButton')
    await useCalm.getState().reProbeWalked()
    expect(useCalm.getState().rows['taskbar.widgetsButton']).toBe('confirmedOff')
    expect(useCalm.getState().walkedId).toBeNull()
    // A second refocus is a no-op, not an illegal-transition crash (codex R1 #9).
    await useCalm.getState().reProbeWalked()
    expect(useCalm.getState().rows['taskbar.widgetsButton']).toBe('confirmedOff')
  })

  test('guided unreadable row needs attestation and never counts as ours', async () => {
    await useCalm.getState().probe()
    await useCalm.getState().walkGuided('widgets.feed')
    await useCalm.getState().reProbeWalked()
    expect(useCalm.getState().rows['widgets.feed']).toBe('pushing')
    useCalm.getState().attestGuided('widgets.feed')
    expect(useCalm.getState().rows['widgets.feed']).toBe('userAttested')
    expect(useCalm.getState().walkedId).toBeNull()
    expect(countQuieted(useCalm.getState().rows)).toBe(0)
  })

  test('operations never interleave: probe/restore during apply are no-ops', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 20 }))
    await useCalm.getState().probe()
    const applying = useCalm.getState().applyAll()
    await useCalm.getState().probe() // rejected — op lock held
    await useCalm.getState().restoreAll() // rejected — op lock held
    await applying
    const s = useCalm.getState()
    expect(s.lastApply?.verified).toBe(3)
    expect(countOwnedWrites(s.rows)).toBe(3) // restore did NOT run mid-apply
  })

  test('applyAll with nothing applicable is a stated no-op, never a fake re-run', async () => {
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    const before = useCalm.getState().rows
    await useCalm.getState().applyAll()
    expect(useCalm.getState().rows).toEqual(before)
    expect(useCalm.getState().lastApply?.verified).toBe(3) // summary unchanged
  })
})
