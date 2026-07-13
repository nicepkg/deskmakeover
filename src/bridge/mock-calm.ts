// 清爽 module — CalmBackend port + the browser-loop mock (Wave 0, plan
// 2026-07-13-calm-windows-module.md). The REAL backend lands in Wave 1 as Rust
// commands behind dm-contracts (bridge schema 8); this port is the seam so the
// store never knows which side it is talking to. The mock simulates the honest
// environment truth: starter-slice rows are certified, next-in-line rows are
// fail-closed (uncertified), guided rows can only be walked. Ownership is
// ledger-shaped: an applied row re-probes as quiet + ownedByUs.

import { CALM_CATALOG, controlById, type CalmControlId } from '@/lib/calm/catalog'
import type { CalmProbeState } from '@/lib/calm/states'

export interface CalmProbeRow {
  id: CalmControlId
  state: CalmProbeState
  /** Ledger truth: we wrote this row and the raw value still matches our write. */
  ownedByUs?: boolean
  /** HealthCheck drift (spec 08 §6): the ledger holds our write but the raw value
   *  no longer matches AND the surface pushes again — the row was turned back on
   *  since we quieted it. Drives the 「又打开了 → 重新关闭」 re-propose notice;
   *  NEVER an auto-replay (codex R3 #1). */
  driftedFromUs?: boolean
}

export interface CalmApplyRow {
  id: CalmControlId
  /** skipped = no write happened (environment changed between probe and apply). */
  outcome: 'verified' | 'setAwaiting' | 'reverted' | 'skipped'
  /** Why a row was skipped — surfaces as the row's honest caption (codex R2). */
  reason?: 'changed'
}

export interface CalmRestoreRow {
  id: CalmControlId
  outcome: 'restored' | 'skippedDrift'
}

export interface CalmBackend {
  probe(): Promise<CalmProbeRow[]>
  /** Batch apply (snapshot → journaled writes → verify) over the given writable ids. */
  apply(ids: CalmControlId[]): Promise<CalmApplyRow[]>
  /** Restore every DeskMakeover-written value toward its recorded original. */
  restore(): Promise<CalmRestoreRow[]>
  /** Restore ONE control toward its recorded original (per-row undo). */
  restoreOne(id: CalmControlId): Promise<CalmRestoreRow>
  /** Open the documented route for a guided row (ms-settings / Win+W instruction). */
  openRoute(id: CalmControlId): Promise<void>
  /** Guided return-probe: readable rows report off(true)/on(false); unreadable → null. */
  reProbeGuided(id: CalmControlId): Promise<boolean | null>
}

export interface MockCalmOptions {
  /** Rows the fake environment reports as policy/MDM managed. */
  managed?: CalmControlId[]
  /** Rows whose apply lands as 已设置·重启后生效 instead of 已生效. */
  awaiting?: CalmControlId[]
  /** Rows whose apply fails and rolls back (exercises the honest reverted path). */
  failing?: CalmControlId[]
  /** Rows whose apply is skipped without a write (probe→apply environment change). */
  skipping?: CalmControlId[]
  /** Rows the user "hand-edited" after apply — restore must skip them (mark, don't clobber). */
  drifted?: CalmControlId[]
  /** Rows behind a crossed certification boundary (feature update) — 需重新确认. */
  boundaryCrossed?: CalmControlId[]
  latencyMs?: number
}

export class MockCalmBackend implements CalmBackend {
  private applied = new Set<CalmControlId>()
  private walked = new Set<CalmControlId>()
  /** One-shot external-flip simulation (codex R4 #1). `armedFlips` (from opts)
   *  fires right after OUR first write → moves to `flipped` (the value moved,
   *  probe reports driftedFromUs). A successful RE-apply overwrites the flip and
   *  clears it — the environment then reads as ours again. */
  private armedFlips: Set<CalmControlId>
  private flipped = new Set<CalmControlId>()
  private opts: MockCalmOptions
  constructor(opts: MockCalmOptions = {}) {
    this.opts = opts
    this.armedFlips = new Set(opts.drifted ?? [])
  }

  private async wait() {
    const ms = this.opts.latencyMs ?? 120
    if (ms > 0) await new Promise((r) => setTimeout(r, ms))
  }

  async probe(): Promise<CalmProbeRow[]> {
    await this.wait()
    return CALM_CATALOG.map((c): CalmProbeRow => {
      if (this.opts.managed?.includes(c.id)) return { id: c.id, state: 'managed' }
      if (c.tier === 'guided') return { id: c.id, state: 'pushing' }
      if (this.opts.boundaryCrossed?.includes(c.id)) return { id: c.id, state: 'needsReconfirm' }
      // Mock environment truth: only the starter slice is lab-certified today;
      // every other automatic row fails closed — exactly what group 3 is for.
      if (!c.starterSlice) return { id: c.id, state: 'unsupported' }
      if (this.applied.has(c.id)) {
        // Ledger-owned but the value moved: the surface pushes again — report the
        // drift so the store can re-propose (never silently a plain candidate).
        if (this.flipped.has(c.id)) return { id: c.id, state: 'pushing', driftedFromUs: true }
        return { id: c.id, state: 'quiet', ownedByUs: true }
      }
      return { id: c.id, state: 'pushing' }
    })
  }

  async apply(ids: CalmControlId[]): Promise<CalmApplyRow[]> {
    await this.wait()
    return ids.map((id): CalmApplyRow => {
      controlById(id) // throws on unknown ids — the mock is as strict as Rust will be
      if (this.opts.failing?.includes(id)) return { id, outcome: 'reverted' }
      if (this.opts.skipping?.includes(id)) return { id, outcome: 'skipped', reason: 'changed' }
      this.applied.add(id)
      if (this.flipped.has(id)) {
        this.flipped.delete(id) // the re-write overwrites the external flip
      } else if (this.armedFlips.has(id)) {
        this.armedFlips.delete(id)
        this.flipped.add(id) // the one-shot external flip fires after OUR write
      }
      if (this.opts.awaiting?.includes(id)) return { id, outcome: 'setAwaiting' }
      return { id, outcome: 'verified' }
    })
  }

  async restore(): Promise<CalmRestoreRow[]> {
    await this.wait()
    const rows = [...this.applied].map((id): CalmRestoreRow => {
      // A restore-skip DISOWNS the ledger row (theirs now) — a later probe must
      // not keep re-proposing a row we no longer claim.
      this.applied.delete(id)
      if (this.flipped.has(id)) return { id, outcome: 'skippedDrift' }
      return { id, outcome: 'restored' }
    })
    return rows
  }

  async restoreOne(id: CalmControlId): Promise<CalmRestoreRow> {
    await this.wait()
    controlById(id)
    this.applied.delete(id) // restored or disowned — either way no longer ours
    if (this.flipped.has(id)) return { id, outcome: 'skippedDrift' }
    return { id, outcome: 'restored' }
  }

  async openRoute(id: CalmControlId): Promise<void> {
    await this.wait()
    this.walked.add(id)
  }

  async reProbeGuided(id: CalmControlId): Promise<boolean | null> {
    await this.wait()
    const c = controlById(id)
    if (!c.readableState) return null
    // Readable guided row: walking it in the mock succeeds (the user turned it off).
    return this.walked.has(id)
  }
}
