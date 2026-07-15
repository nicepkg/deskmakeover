import { beforeEach, describe, expect, test } from 'bun:test'
import { CALM_CATALOG, type CalmControlId } from '../src/lib/calm/catalog'
import type { CalmRowState } from '../src/lib/calm/states'
import { MockCalmBackend } from '../src/bridge/mock-calm'
import { countOwnedWrites, countQuieted, countRestorable, groupedRows, guidedOnlyFace, reopenedRows, setCalmBackend, useCalm } from '../src/stores/calm'

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
    skipReasons: {},
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

  test('an all-fail-closed probe wears the guided-only face, never an eternal spinner', () => {
    // ADR-0023 D2 regression (owner 2026-07-16): the real-Windows stack ships fail-closed
    // pre-W3 — every automatic candidate probes unsupported. The hero must switch to the
    // honest guided-only face instead of spinning on 「扫描中」 forever.
    const failClosed = Object.fromEntries(
      CALM_CATALOG.map((c) => [c.id, c.tier === 'guided' ? 'pushing' : 'unsupported']),
    ) as Record<CalmControlId, CalmRowState>
    expect(guidedOnlyFace(true, failClosed)).toBe(true)
    expect(guidedOnlyFace(false, failClosed)).toBe(false) // pre-probe stays on the spinner
    // Any real one-click state (a candidate, a verified write, a reopened drift) → normal hero.
    expect(guidedOnlyFace(true, { ...failClosed, 'taskbar.search': 'pushing' })).toBe(false)
    expect(guidedOnlyFace(true, { ...failClosed, 'taskbar.search': 'verified' })).toBe(false)
    expect(guidedOnlyFace(true, { ...failClosed, 'taskbar.search': 'reopened' })).toBe(false)
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

  test('a skipped write returns the row to pushing, is counted, and carries its reason', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, skipping: ['taskbar.taskview'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    const s = useCalm.getState()
    expect(s.rows['taskbar.taskview']).toBe('pushing')
    expect(s.lastApply).toEqual({ verified: 2, awaiting: 0, reverted: 0, skipped: 1 })
    expect(s.skipReasons['taskbar.taskview']).toBe('changed')
  })

  test('a lost apply reply never claims rollback — rows recover through a fresh probe', async () => {
    class ExplodingBackend extends MockCalmBackend {
      override async apply(): Promise<never> {
        throw new Error('ipc down')
      }
    }
    resetStore(new ExplodingBackend({ latencyMs: 0 }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll() // awaits the recovery probe internally
    const s = useCalm.getState()
    for (const id of ['start.recommendations', 'taskbar.search', 'taskbar.taskview'] as CalmControlId[]) {
      // Environment truth (mock: nothing committed) — NOT an unproven 「已还原」 claim.
      expect(s.rows[id]).toBe('pushing')
    }
    expect(s.op).toBe('idle')
    expect(s.lastApply).toEqual({ verified: 0, awaiting: 0, reverted: 0, skipped: 0 })
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

  test('restoreOne undoes exactly one row and leaves the rest owned', async () => {
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().restoreOne('taskbar.search')
    const s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('pushing') // that surface pushes again
    expect(s.rows['taskbar.taskview']).toBe('verified') // others untouched
    expect(countOwnedWrites(s.rows)).toBe(2)
  })

  test('restoreOne on a hand-edited row marks it external, never clobbers', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, drifted: ['taskbar.search'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().restoreOne('taskbar.search')
    expect(useCalm.getState().rows['taskbar.search']).toBe('external')
  })

  test('a row we quieted that turns back on RE-PROPOSES — never a silent replay, never a plain candidate', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, drifted: ['taskbar.search'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll() // all three verified (the flip fires after our write)
    await useCalm.getState().probe() // HealthCheck: ledger-owned + the value moved
    let s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('reopened') // candidate again — WITH provenance
    expect(reopenedRows(s.rows)).toEqual(['taskbar.search'])
    expect(s.rows['taskbar.taskview']).toBe('verified') // intact rows untouched
    // 「重新关闭」 = a SCOPED re-apply through the same verify pipeline.
    await useCalm.getState().applyAll(reopenedRows(s.rows))
    s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('verified')
    expect(reopenedRows(s.rows)).toEqual([])
    expect(s.lastApply?.verified).toBe(1) // scoped: nothing else re-ran
    // The re-close must SURVIVE the next HealthCheck (codex R4 #1: the mock's
    // drift is a real one-shot flip, not an immortal option).
    await useCalm.getState().probe()
    s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('verified')
    expect(reopenedRows(s.rows)).toEqual([])
  })

  test('when EVERY write drifts, the restore entrance survives and disowns them all (codex R5 #1)', async () => {
    const all: CalmControlId[] = ['start.recommendations', 'taskbar.search', 'taskbar.taskview']
    resetStore(new MockCalmBackend({ latencyMs: 0, drifted: all }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().probe() // every write drifted
    let s = useCalm.getState()
    for (const id of all) expect(s.rows[id]).toBe('reopened')
    expect(countOwnedWrites(s.rows)).toBe(0) // no intact writes…
    expect(countRestorable(s.rows)).toBe(3) // …but the ledger still has rows to act on
    await useCalm.getState().restoreAll() // must reach the backend and disown
    s = useCalm.getState()
    for (const id of all) expect(s.rows[id]).toBe('external')
    expect(countRestorable(s.rows)).toBe(0)
  })

  test('an excluded reopened row is user-silenced: the scoped re-close never touches it (codex R5 #2)', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, drifted: ['taskbar.search'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().probe() // taskbar.search → reopened
    useCalm.getState().toggleExcluded('taskbar.search')
    await useCalm.getState().applyAll(['taskbar.search']) // the notice button's scope
    const s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('reopened') // untouched — exclusion wins
    expect(s.rows['taskbar.taskview']).toBe('verified') // and nothing else re-ran
    expect(s.op).toBe('idle') // the empty scope was a stated no-op
    useCalm.getState().toggleExcluded('taskbar.search') // restore for later tests' localStorage
  })

  test('restore over a reopened row disowns it as external — never an illegal-transition crash (codex R4 #1)', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, drifted: ['taskbar.search'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().probe() // drift lands: taskbar.search → reopened
    expect(useCalm.getState().rows['taskbar.search']).toBe('reopened')
    await useCalm.getState().restoreAll() // must not throw
    const s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('external') // theirs now — marked, not clobbered
    expect(s.rows['taskbar.taskview']).toBe('pushing') // intact rows restored normally
    expect(reopenedRows(s.rows)).toEqual([]) // the notice is gone with the disown
    expect(countOwnedWrites(s.rows)).toBe(0)
  })

  test('a stale guided re-probe never clears a NEWER walk token (codex R3 #4)', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 15 }))
    await useCalm.getState().probe()
    await useCalm.getState().walkGuided('taskbar.widgetsButton') // readable row
    const stale = useCalm.getState().reProbeWalked() // in flight…
    await useCalm.getState().walkGuided('widgets.feed') // …user opens another walk
    await stale
    const s = useCalm.getState()
    expect(s.rows['taskbar.widgetsButton']).toBe('confirmedOff') // the probe's truth still lands
    expect(s.walkedId).toBe('widgets.feed') // the NEW token survives
  })

  test('two concurrent re-probes of the SAME row are idempotent — never a double confirmedOff crash (codex R4 #4)', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 15 }))
    await useCalm.getState().probe()
    await useCalm.getState().walkGuided('taskbar.widgetsButton')
    // Two window-focus events racing — both must settle without throwing.
    await Promise.all([useCalm.getState().reProbeWalked(), useCalm.getState().reProbeWalked()])
    const s = useCalm.getState()
    expect(s.rows['taskbar.widgetsButton']).toBe('confirmedOff')
    expect(s.walkedId).toBeNull()
  })

  test('a lost restoreOne reply never claims restored — the row recovers via probe', async () => {
    class ExplodingRestoreOne extends MockCalmBackend {
      override async restoreOne(): Promise<never> {
        throw new Error('ipc down')
      }
    }
    resetStore(new ExplodingRestoreOne({ latencyMs: 0 }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().restoreOne('taskbar.search') // awaits the recovery probe
    const s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('verified') // ledger truth: the write is intact
    expect(s.op).toBe('idle')
  })

  test('a lost restoreAll reply never claims restored — owned rows recover via probe', async () => {
    class ExplodingRestore extends MockCalmBackend {
      override async restore(): Promise<never> {
        throw new Error('ipc down')
      }
    }
    resetStore(new ExplodingRestore({ latencyMs: 0 }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().restoreAll()
    const s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('verified')
    expect(s.op).toBe('idle')
  })

  test('restoreOne is refused while another operation holds the lock', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 20 }))
    await useCalm.getState().probe()
    const applying = useCalm.getState().applyAll()
    await useCalm.getState().restoreOne('taskbar.search') // no-op — lock held
    await applying
    const s = useCalm.getState()
    expect(s.rows['taskbar.search']).toBe('verified') // the apply outcome, not a restore
    expect(countOwnedWrites(s.rows)).toBe(3)
  })

  test('restoreOne on a setAwaiting row returns it to pushing', async () => {
    resetStore(new MockCalmBackend({ latencyMs: 0, awaiting: ['start.recommendations'] }))
    await useCalm.getState().probe()
    await useCalm.getState().applyAll()
    await useCalm.getState().restoreOne('start.recommendations')
    expect(useCalm.getState().rows['start.recommendations']).toBe('pushing')
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

  test("applyAll resolves THIS call's summary; a lock-refused call resolves null, never a stale summary (codex R6)", async () => {
    resetStore(new MockCalmBackend({ latencyMs: 20 }))
    await useCalm.getState().probe()
    const first = useCalm.getState().applyAll()
    const second = useCalm.getState().applyAll() // double-click: lock held
    expect(await second).toBeNull() // a celebration keyed on this MUST stay quiet
    expect(await first).toEqual({ verified: 3, awaiting: 0, reverted: 0, skipped: 0 })
    // And with nothing applicable, the stated no-op also resolves null.
    expect(await useCalm.getState().applyAll()).toBeNull()
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
