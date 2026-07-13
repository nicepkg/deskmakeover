import { create } from 'zustand'
import { CALM_CATALOG, controlById, groupOf, type CalmControlId, type CalmGroup } from '@/lib/calm/catalog'
import {
  COUNTED_AS_OURS,
  OWNED_WRITES,
  assertTransition,
  probeTransition,
  type CalmRowState,
} from '@/lib/calm/states'
import { MockCalmBackend, type CalmBackend } from '@/bridge/mock-calm'
import { format, t } from '@/lib/i18n'
import { useToasts } from '@/stores/toasts'

// 清爽 module store (spec 08 §3-§7). Every row mutation goes through
// assertTransition / probeTransition — the store CANNOT express a dishonest
// state change. One operation lock serializes probe/apply/restore (codex R1 #7).
// Backend = the CalmBackend port; Wave 1 swaps in the Tauri implementation.

let backend: CalmBackend = new MockCalmBackend()

/** Wave-1 seam + test injection point. */
export function setCalmBackend(b: CalmBackend) {
  backend = b
}

const EXCLUDED_KEY = 'dm.calm.excluded'

// A failed load silently re-arming excluded controls would betray the user's
// explicit choice (codex R2 #3) — track it and say so on the next probe.
let excludedLoadFailed = false

function loadExcluded(): Set<CalmControlId> {
  try {
    const raw = localStorage.getItem(EXCLUDED_KEY)
    return new Set(raw ? (JSON.parse(raw) as CalmControlId[]) : [])
  } catch {
    excludedLoadFailed = true
    return new Set()
  }
}

export interface CalmApplySummary {
  verified: number
  awaiting: number
  reverted: number
  skipped: number
}

type CalmOp = 'idle' | 'probe' | 'apply' | 'restore'

interface CalmState {
  probed: boolean
  /** One operation at a time — probe/apply/restore never interleave. */
  op: CalmOp
  rows: Record<CalmControlId, CalmRowState>
  excluded: Set<CalmControlId>
  /** Honest per-row skip captions from the last apply (cleared on apply/probe). */
  skipReasons: Partial<Record<CalmControlId, 'changed'>>
  /** The guided row whose route we last opened — re-probed on window refocus. */
  walkedId: CalmControlId | null
  lastApply: CalmApplySummary | null
  probe: () => Promise<void>
  toggleExcluded: (id: CalmControlId) => void
  applyAll: () => Promise<void>
  restoreAll: () => Promise<void>
  /** Per-row undo (owner 2026-07-13): one control back to its original. */
  restoreOne: (id: CalmControlId) => Promise<void>
  walkGuided: (id: CalmControlId) => Promise<void>
  /** Refocus/return probe for the last-walked guided row (idempotent). */
  reProbeWalked: () => Promise<void>
  /** Unreadable guided rows: the user answers 关好了吗 — recorded as theirs, never ours. */
  attestGuided: (id: CalmControlId) => void
}

const initialRows = () =>
  Object.fromEntries(CALM_CATALOG.map((c) => [c.id, 'unknown'])) as Record<CalmControlId, CalmRowState>

function kindOf(id: CalmControlId): 'writable' | 'guided' {
  return controlById(id).tier === 'guided' ? 'guided' : 'writable'
}

/** Rows the one-click may write right now: in the package, not excluded, actionable. */
export function applyCandidates(
  rows: Record<CalmControlId, CalmRowState>,
  excluded: Set<CalmControlId>,
): CalmControlId[] {
  return CALM_CATALOG.filter(
    (c) =>
      groupOf(c, rows[c.id]) === 'oneClick' &&
      c.inDefaultPackage &&
      !excluded.has(c.id) &&
      (rows[c.id] === 'pushing' || rows[c.id] === 'reverted'),
  ).map((c) => c.id)
}

export const useCalm = create<CalmState>((set, get) => {
  const setRow = (id: CalmControlId, to: CalmRowState) =>
    set((s) => ({ rows: { ...s.rows, [id]: assertTransition(kindOf(id), s.rows[id], to) } }))

  /** Honest apply toast assembled from ALL outcome counts (codex R1 #11). */
  const applyToast = (sum: CalmApplySummary) => {
    const parts: string[] = []
    if (sum.verified > 0) parts.push(format(t('Calm_ToastPart_Quieted'), sum.verified))
    if (sum.awaiting > 0) parts.push(format(t('Calm_ToastPart_Awaiting'), sum.awaiting))
    const failed = sum.reverted + sum.skipped
    if (failed > 0) parts.push(format(t('Calm_ToastPart_Failed'), failed))
    if (parts.length === 0) return
    useToasts.getState().show(parts.join(t('Calm_ToastJoin')), failed > 0 ? 'warn' : 'success')
  }

  return {
    probed: false,
    op: 'idle',
    rows: initialRows(),
    excluded: loadExcluded(),
    skipReasons: {},
    walkedId: null,
    lastApply: null,

    probe: async () => {
      if (get().op !== 'idle') return
      set({ op: 'probe' })
      try {
        const probed = await backend.probe()
        set((s) => {
          const rows = { ...s.rows }
          for (const p of probed) {
            rows[p.id] = probeTransition(kindOf(p.id), rows[p.id], p)
          }
          return { rows, probed: true, skipReasons: {} }
        })
        if (excludedLoadFailed) {
          excludedLoadFailed = false
          useToasts.getState().show(t('Calm_Toast_ExcludeLoadFailed'), 'warn')
        }
      } finally {
        set({ op: 'idle' })
      }
    },

    toggleExcluded: (id) => {
      set((s) => {
        const excluded = new Set(s.excluded)
        if (excluded.has(id)) excluded.delete(id)
        else excluded.add(id)
        try {
          localStorage.setItem(EXCLUDED_KEY, JSON.stringify([...excluded]))
        } catch {
          // The exclusion would silently die with the session — say so (codex R1 #10).
          useToasts.getState().show(t('Calm_Toast_ExcludeNotSaved'), 'warn')
        }
        return { excluded }
      })
    },

    applyAll: async () => {
      const { rows, excluded, op } = get()
      if (op !== 'idle') return
      const ids = applyCandidates(rows, excluded)
      if (ids.length === 0) {
        useToasts.getState().show(t('Calm_Toast_NothingToDo'))
        return
      }
      set({ op: 'apply', skipReasons: {} })
      let lostReply = false
      try {
        for (const id of ids) setRow(id, 'pending')
        const summary: CalmApplySummary = { verified: 0, awaiting: 0, reverted: 0, skipped: 0 }
        let results: Awaited<ReturnType<CalmBackend['apply']>> = []
        try {
          results = await backend.apply(ids)
        } catch {
          results = [] // reply lost — handled below as UNKNOWN, never a rollback claim
        }
        const byId = new Map(results.map((r) => [r.id, r]))
        const skipReasons: Partial<Record<CalmControlId, 'changed'>> = {}
        for (const id of ids) {
          const row = byId.get(id)
          if (!row) {
            // The backend never answered: the write MAY have committed. Claiming
            // 「已还原」 would be an unproven rollback (codex R2 #1) — drop to
            // unknown and re-probe the environment for the truth.
            setRow(id, 'unknown')
            lostReply = true
            continue
          }
          if (row.outcome === 'verified') {
            setRow(id, 'verified')
            summary.verified++
          } else if (row.outcome === 'setAwaiting') {
            setRow(id, 'setAwaiting')
            summary.awaiting++
          } else if (row.outcome === 'skipped') {
            setRow(id, 'pushing') // no write happened — the surface still pushes
            if (row.reason) skipReasons[id] = row.reason
            summary.skipped++
          } else {
            setRow(id, 'reverted') // an EXPLICIT rollback report from the backend
            summary.reverted++
          }
        }
        set({ lastApply: summary, skipReasons })
        if (lostReply) useToasts.getState().show(t('Calm_Toast_Unconfirmed'), 'warn')
        else applyToast(summary)
      } finally {
        set({ op: 'idle' })
      }
      // Recover the unknown rows from environment truth, outside the apply op.
      if (lostReply) await get().probe()
    },

    restoreAll: async () => {
      if (get().op !== 'idle') return
      set({ op: 'restore' })
      try {
        const results = await backend.restore()
        let restored = 0
        let skipped = 0
        for (const r of results) {
          if (r.outcome === 'restored') {
            setRow(r.id, 'pushing') // the surface pushes again — stated plainly
            restored++
          } else {
            // Hand-edited since our write: it is THEIRS now — mark, don't clobber,
            // and stop displaying it as ours (codex R1 Block #1).
            setRow(r.id, 'external')
            skipped++
          }
        }
        const toasts = useToasts.getState()
        if (skipped > 0) toasts.show(format(t('Calm_Toast_RestoredSkipped'), restored, skipped))
        else toasts.show(format(t('Calm_Toast_Restored'), restored))
      } finally {
        set({ op: 'idle' })
      }
    },

    restoreOne: async (id) => {
      if (get().op !== 'idle') return
      set({ op: 'restore' })
      try {
        const r = await backend.restoreOne(id)
        if (r.outcome === 'restored') {
          setRow(id, 'pushing') // the surface pushes again — stated plainly
          useToasts.getState().show(format(t('Calm_Toast_RestoredOne'), t(controlById(id).labelKey)))
        } else {
          setRow(id, 'external') // hand-edited since: theirs now, untouched
          useToasts.getState().show(format(t('Calm_Toast_RestoredSkipped'), 0, 1))
        }
      } finally {
        set({ op: 'idle' })
      }
    },

    walkGuided: async (id) => {
      set({ walkedId: id })
      await backend.openRoute(id)
    },

    reProbeWalked: async () => {
      const id = get().walkedId
      if (!id) return
      // Idempotent: a settled row needs no further probing (codex R1 #9).
      const state = get().rows[id]
      if (state !== 'pushing') {
        set({ walkedId: null })
        return
      }
      const off = await backend.reProbeGuided(id)
      if (off === true) {
        setRow(id, 'confirmedOff')
        set({ walkedId: null })
      }
      // off === false → still pushing (row already says so); null → unreadable, the
      // page asks the user and records userAttested via attestGuided.
    },

    attestGuided: (id) => {
      setRow(id, 'userAttested')
      if (get().walkedId === id) set({ walkedId: null })
    },
  }
})

/** Verified-writes-only summary count (spec 08 §3: guided outcomes are never ours). */
export function countQuieted(rows: Record<CalmControlId, CalmRowState>): number {
  return CALM_CATALOG.filter((c) => COUNTED_AS_OURS.has(rows[c.id])).length
}

/** Live DeskMakeover-owned writes (verified + setAwaiting) — gates Restore (codex R1 Block #2). */
export function countOwnedWrites(rows: Record<CalmControlId, CalmRowState>): number {
  return CALM_CATALOG.filter((c) => OWNED_WRITES.has(rows[c.id])).length
}

/** Rows per page group, catalog order (widgets.feed leads guided by catalog order). */
export function groupedRows(rows: Record<CalmControlId, CalmRowState>) {
  const groups: Record<CalmGroup, CalmControlId[]> = { oneClick: [], guided: [], held: [] }
  for (const c of CALM_CATALOG) groups[groupOf(c, rows[c.id])].push(c.id)
  return groups
}
