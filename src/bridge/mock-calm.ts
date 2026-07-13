// 清爽 module — CalmBackend port + the browser-loop mock (Wave 0, plan
// 2026-07-13-calm-windows-module.md). The REAL backend lands in Wave 1 as Rust
// commands behind dm-contracts (bridge schema 8); this port is the seam so the
// store never knows which side it is talking to. The mock simulates the honest
// environment truth: starter-slice rows are certified, next-in-line rows are
// fail-closed (uncertified), guided rows can only be walked.

import { CALM_CATALOG, controlById, type CalmControlId } from '@/lib/calm/catalog'

export type CalmProbeState = 'quiet' | 'pushing' | 'unsupported' | 'managed'

export interface CalmProbeRow {
  id: CalmControlId
  state: CalmProbeState
}

export interface CalmApplyRow {
  id: CalmControlId
  outcome: 'verified' | 'setAwaiting' | 'reverted'
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
  /** Rows the user "hand-edited" after apply — restore must skip them (mark, don't clobber). */
  drifted?: CalmControlId[]
  latencyMs?: number
}

export class MockCalmBackend implements CalmBackend {
  private applied = new Set<CalmControlId>()
  private walked = new Set<CalmControlId>()
  private opts: MockCalmOptions
  constructor(opts: MockCalmOptions = {}) {
    this.opts = opts
  }

  private async wait() {
    const ms = this.opts.latencyMs ?? 120
    if (ms > 0) await new Promise((r) => setTimeout(r, ms))
  }

  async probe(): Promise<CalmProbeRow[]> {
    await this.wait()
    return CALM_CATALOG.map((c) => {
      if (this.opts.managed?.includes(c.id)) return { id: c.id, state: 'managed' as const }
      if (c.tier === 'guided') return { id: c.id, state: 'pushing' as const }
      // Mock environment truth: only the starter slice is lab-certified today;
      // every other automatic row fails closed — exactly what group 3 is for.
      if (!c.starterSlice) return { id: c.id, state: 'unsupported' as const }
      return { id: c.id, state: this.applied.has(c.id) ? ('quiet' as const) : ('pushing' as const) }
    })
  }

  async apply(ids: CalmControlId[]): Promise<CalmApplyRow[]> {
    await this.wait()
    return ids.map((id) => {
      controlById(id) // throws on unknown ids — the mock is as strict as Rust will be
      if (this.opts.failing?.includes(id)) return { id, outcome: 'reverted' as const }
      this.applied.add(id)
      if (this.opts.awaiting?.includes(id)) return { id, outcome: 'setAwaiting' as const }
      return { id, outcome: 'verified' as const }
    })
  }

  async restore(): Promise<CalmRestoreRow[]> {
    await this.wait()
    const rows = [...this.applied].map((id) => {
      if (this.opts.drifted?.includes(id)) return { id, outcome: 'skippedDrift' as const }
      this.applied.delete(id)
      return { id, outcome: 'restored' as const }
    })
    return rows
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
