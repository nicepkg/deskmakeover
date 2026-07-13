import { beforeEach, describe, expect, test } from 'bun:test'
import { CALM_CATALOG, type CalmControlId } from '../src/lib/calm/catalog'
import type { CalmRowState } from '../src/lib/calm/states'
import { MockCalmBackend } from '../src/bridge/mock-calm'
import { countQuieted, groupedRows, setCalmBackend, useCalm } from '../src/stores/calm'

// Store behaviour tests over the CalmBackend mock (plan W0.3). The mock's fake
// environment: starter slice certified, other automatic rows fail-closed, guided
// rows walkable — so these tests also exercise the honest group-3 path.

function resetStore(backend: MockCalmBackend) {
  setCalmBackend(backend)
  useCalm.setState({
    probed: false,
    probing: false,
    applying: false,
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

  test('applyAll verifies the certified slice and counts only verified writes', async () => {
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    const s = useCalm.getState()
    expect(s.lastApply).toEqual({ verified: 3, awaiting: 0, reverted: 0 })
    expect(s.rows['taskbar.search']).toBe('verified')
    expect(countQuieted(s.rows)).toBe(3)
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
    expect(s.lastApply).toEqual({ verified: 2, awaiting: 0, reverted: 1 })
    expect(countQuieted(s.rows)).toBe(2)
  })

  test('sign-out rows land setAwaiting and are NOT counted as quieted yet', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, awaiting: ['start.recommendations'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    const s = useCalm.getState()
    expect(s.rows['start.recommendations']).toBe('setAwaiting')
    expect(countQuieted(s.rows)).toBe(2)
  })

  test('restore skips hand-edited rows (mark, don\'t clobber) and reopens the rest', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, drifted: ['taskbar.search'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().restoreAll()
    const s = useCalm.getState()
    expect(s.rows['taskbar.taskview']).toBe('pushing') // restored → the surface pushes again
    expect(s.rows['taskbar.search']).toBe('verified') // drifted → untouched, still marked
  })

  test('guided readable row confirms off after the walk; unreadable needs attestation', async () => {
    await useCalm.getState().probe()
    // readable: the taskbar widgets button (TaskbarDa read-only observation)
    await useCalm.getState().walkGuided('taskbar.widgetsButton')
    await useCalm.getState().reProbeWalked()
    expect(useCalm.getState().rows['taskbar.widgetsButton']).toBe('confirmedOff')
    // unreadable: the widgets feed — re-probe cannot know; user attests
    await useCalm.getState().walkGuided('widgets.feed')
    await useCalm.getState().reProbeWalked()
    expect(useCalm.getState().rows['widgets.feed']).toBe('pushing')
    useCalm.getState().attestGuided('widgets.feed')
    expect(useCalm.getState().rows['widgets.feed']).toBe('userAttested')
    // neither guided outcome ever counts as ours
    expect(countQuieted(useCalm.getState().rows)).toBe(0)
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
