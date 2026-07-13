// The real 清爽 (calm-Windows) backend under Tauri (bridge schema 8): it implements the
// CalmBackend port by calling the generated `tweaks*` commands and converting the wire DTOs to
// the port shapes. The DTO field names + enum literals mirror the port exactly (dm-contracts is
// authored to match mock-calm.ts), so the conversion is nearly an identity — only the guided
// return maps the tri-state enum to the port's `boolean | null`.
//
// The store never learns which side it talks to: the mock and this backend share the CalmBackend
// port. Wired only inside a Tauri WebView; `bun test` / the browser mock loop never load it.

import type { CalmControlId } from '@/lib/calm/catalog'
import type { CalmBackend, CalmProbeRow, CalmApplyRow, CalmRestoreRow } from '@/bridge/mock-calm'

type OkErr<T> = { status: 'ok'; data: T } | { status: 'error'; error: string }

function unwrap<T>(result: OkErr<T>): T {
  if (result.status === 'error') throw new Error(result.error)
  return result.data
}

async function commands() {
  // Lazy so the browser/mock path never imports @tauri-apps/api.
  return (await import('./generated')).commands
}

/** The Tauri implementation of the CalmBackend port (Wave 1 Rust decision core). */
export class TauriCalmBackend implements CalmBackend {
  async probe(): Promise<CalmProbeRow[]> {
    const rows = unwrap(await (await commands()).tweaksProbe())
    return rows.map((row) => ({
      id: row.id as CalmControlId,
      state: row.state,
      ownedByUs: row.ownedByUs,
      driftedFromUs: row.driftedFromUs,
    }))
  }

  async apply(ids: CalmControlId[]): Promise<CalmApplyRow[]> {
    const rows = unwrap(await (await commands()).tweaksApply(ids))
    return rows.map((row) => ({
      id: row.id as CalmControlId,
      outcome: row.outcome,
      reason: row.reason ?? undefined,
    }))
  }

  async restore(): Promise<CalmRestoreRow[]> {
    const rows = unwrap(await (await commands()).tweaksRestore())
    return rows.map((row) => ({ id: row.id as CalmControlId, outcome: row.outcome }))
  }

  async restoreOne(id: CalmControlId): Promise<CalmRestoreRow> {
    const row = unwrap(await (await commands()).tweaksRestoreOne(id))
    return { id: row.id as CalmControlId, outcome: row.outcome }
  }

  async openRoute(id: CalmControlId): Promise<void> {
    unwrap(await (await commands()).tweaksOpenRoute(id))
  }

  async reProbeGuided(id: CalmControlId): Promise<boolean | null> {
    const answer = unwrap(await (await commands()).tweaksReProbeGuided(id))
    // 'off' → readable + confirmed off; 'stillOn' → readable + still on; 'unreadable' → the app
    // cannot know, so the page asks the user to attest.
    if (answer === 'off') return true
    if (answer === 'stillOn') return false
    return null
  }
}
