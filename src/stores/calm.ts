import { create } from 'zustand'
import { CALM_CATALOG, controlById, groupOf, type CalmControlId, type CalmGroup } from '@/lib/calm/catalog'
import { COUNTED_AS_OURS, assertTransition, type CalmRowState } from '@/lib/calm/states'
import { MockCalmBackend, type CalmBackend } from '@/bridge/mock-calm'
import { format, t } from '@/lib/i18n'
import { useToasts } from '@/stores/toasts'

// 清爽 module store (spec 08 §3-§7). Every row mutation goes through
// assertTransition — the store CANNOT express a dishonest state change.
// Backend = the CalmBackend port; Wave 1 swaps in the Tauri implementation.

let backend: CalmBackend = new MockCalmBackend()

/** Wave-1 seam + test injection point. */
export function setCalmBackend(b: CalmBackend) {
  backend = b
}

const EXCLUDED_KEY = 'dm.calm.excluded'

function loadExcluded(): Set<CalmControlId> {
  try {
    const raw = localStorage.getItem(EXCLUDED_KEY)
    return new Set(raw ? (JSON.parse(raw) as CalmControlId[]) : [])
  } catch {
    return new Set()
  }
}

export interface CalmApplySummary {
  verified: number
  awaiting: number
  reverted: number
}

interface CalmState {
  probed: boolean
  probing: boolean
  applying: boolean
  rows: Record<CalmControlId, CalmRowState>
  excluded: Set<CalmControlId>
  /** The guided row whose route we last opened — re-probed on window refocus. */
  walkedId: CalmControlId | null
  lastApply: CalmApplySummary | null
  probe: () => Promise<void>
  toggleExcluded: (id: CalmControlId) => void
  applyAll: () => Promise<void>
  restoreAll: () => Promise<void>
  walkGuided: (id: CalmControlId) => Promise<void>
  /** Refocus/return probe for the last-walked guided row. */
  reProbeWalked: () => Promise<void>
  /** Unreadable guided rows: the user answers 关好了吗 — recorded as theirs, never ours. */
  attestGuided: (id: CalmControlId) => void
}

const initialRows = () =>
  Object.fromEntries(CALM_CATALOG.map((c) => [c.id, 'unknown'])) as Record<CalmControlId, CalmRowState>

function kindOf(id: CalmControlId): 'writable' | 'guided' {
  return controlById(id).tier === 'guided' ? 'guided' : 'writable'
}

export const useCalm = create<CalmState>((set, get) => {
  const setRow = (id: CalmControlId, to: CalmRowState) =>
    set((s) => ({ rows: { ...s.rows, [id]: assertTransition(kindOf(id), s.rows[id], to) } }))

  return {
    probed: false,
    probing: false,
    applying: false,
    rows: initialRows(),
    excluded: loadExcluded(),
    walkedId: null,
    lastApply: null,

    probe: async () => {
      if (get().probing) return
      set({ probing: true })
      try {
        const probed = await backend.probe()
        set((s) => {
          const rows = { ...s.rows }
          for (const p of probed) {
            // A fresh probe may legally re-open any settled state; route through the
            // guard so an illegal probe result (e.g. guided → managed) fails loudly.
            if (rows[p.id] !== p.state) rows[p.id] = assertTransition(kindOf(p.id), rows[p.id], p.state)
          }
          return { rows, probed: true }
        })
      } finally {
        set({ probing: false })
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
          /* private mode: exclusion just lives for the session */
        }
        return { excluded }
      })
    },

    applyAll: async () => {
      const { rows, excluded, applying } = get()
      if (applying) return
      const ids = CALM_CATALOG.filter(
        (c) =>
          groupOf(c, rows[c.id]) === 'oneClick' &&
          c.inDefaultPackage &&
          !excluded.has(c.id) &&
          rows[c.id] === 'pushing',
      ).map((c) => c.id)
      if (ids.length === 0) {
        useToasts.getState().show(t('Calm_Toast_NothingToDo'))
        return
      }
      set({ applying: true })
      try {
        for (const id of ids) setRow(id, 'pending')
        const results = await backend.apply(ids)
        const summary: CalmApplySummary = { verified: 0, awaiting: 0, reverted: 0 }
        for (const r of results) {
          if (r.outcome === 'verified') {
            setRow(r.id, 'verified')
            summary.verified++
          } else if (r.outcome === 'setAwaiting') {
            setRow(r.id, 'setAwaiting')
            summary.awaiting++
          } else {
            setRow(r.id, 'reverted')
            summary.reverted++
          }
        }
        set({ lastApply: summary })
        const toasts = useToasts.getState()
        if (summary.reverted > 0) {
          toasts.show(format(t('Calm_Toast_Partial'), summary.verified, summary.reverted), 'warn')
        } else if (summary.awaiting > 0) {
          toasts.show(format(t('Calm_Toast_AppliedAwaiting'), summary.verified, summary.awaiting), 'success')
        } else {
          toasts.show(format(t('Calm_Toast_Applied'), summary.verified), 'success')
        }
      } finally {
        set({ applying: false })
      }
    },

    restoreAll: async () => {
      const results = await backend.restore()
      let restored = 0
      let skipped = 0
      for (const r of results) {
        if (r.outcome === 'restored') {
          setRow(r.id, 'pushing') // the surface pushes again — stated plainly
          restored++
        } else {
          skipped++ // hand-edited since: mark, don't clobber (spec 08 §7)
        }
      }
      const toasts = useToasts.getState()
      if (skipped > 0) toasts.show(format(t('Calm_Toast_RestoredSkipped'), restored, skipped))
      else toasts.show(format(t('Calm_Toast_Restored'), restored))
    },

    walkGuided: async (id) => {
      set({ walkedId: id })
      await backend.openRoute(id)
    },

    reProbeWalked: async () => {
      const id = get().walkedId
      if (!id) return
      const off = await backend.reProbeGuided(id)
      if (off === true) setRow(id, 'confirmedOff')
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

/** Rows per page group, catalog order (widgets.feed leads guided by catalog order). */
export function groupedRows(rows: Record<CalmControlId, CalmRowState>) {
  const groups: Record<CalmGroup, CalmControlId[]> = { oneClick: [], guided: [], held: [] }
  for (const c of CALM_CATALOG) groups[groupOf(c, rows[c.id])].push(c.id)
  return groups
}
