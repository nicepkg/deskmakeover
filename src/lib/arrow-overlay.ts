import type { IconsStateDto } from '@/bridge/types'
import type { StringKey } from '@/lib/i18n'

// Pure decision logic for the machine-wide shortcut-arrow overlay (ADR-0021,
// panel record 2026-07-11). Kept host-free and DOM-free so every edge the
// review flagged — unknown state, DoneCard truthfulness, the multi-user consent
// gate, and the Applied|Declined|Failed restore contract — is unit-testable
// without a bridge or a browser. The panels/store/mock all consume these.

/** The overlay may be UNRESOLVED (undefined) when the icons module has not
 *  finished (or has failed) its scan. That is NOT 'native': coercing it to
 *  native would falsely claim "Windows default" and strip the restore action
 *  exactly when the user needs it (review P2-2). */
export type ArrowOverlay = IconsStateDto['arrowOverlay']

export interface ArrowRowView {
  /** Status line — the authority (panel record §5). */
  statusKey: StringKey
  /** Whether the 「恢复系统箭头」 action + machine-wide constraint note show. */
  showRestore: boolean
  /** True only while the real overlay state is still unknown (scan pending/failed). */
  unknown: boolean
}

/** Settings arrow-row view model. Three states, never two: hidden (actionable),
 *  native (nothing to restore), unknown (checking — never asserts a value). */
export function arrowRowView(overlay: ArrowOverlay | undefined): ArrowRowView {
  if (overlay === 'hidden') return { statusKey: 'Settings_ArrowStatusHidden', showRestore: true, unknown: false }
  if (overlay === 'native') return { statusKey: 'Settings_ArrowStatusNative', showRestore: false, unknown: false }
  return { statusKey: 'Settings_ArrowStatusChecking', showRestore: false, unknown: true }
}

/** DoneCard truthfulness (review P2-1): the "arrow is now hidden" line may only
 *  appear when the overlay is actually hidden right now. A failed apply, or an
 *  apply whose overlay step was UAC-declined, leaves it native and must NOT claim
 *  the arrow was hidden. */
export function showDoneArrowNote(overlay: ArrowOverlay | undefined): boolean {
  return overlay === 'hidden'
}

/** First-run consent gate (owner disposition #3, review P2-3). `v2` = this
 *  build's consent, which includes the machine-wide arrow disclosure. A legacy
 *  (pre-disclosure) consent grandfathers SINGLE-user machines only; a machine
 *  with more than one active profile must (re)see the non-skippable disclosure.
 *  The single-user exception is EXACTLY one profile: a malformed host count of
 *  zero or negative is not "known single user" and must fail closed into the
 *  disclosure (review new-P3), never grandfather. */
export function consentSatisfied(opts: { v2: boolean; legacy: boolean; profiles: number }): boolean {
  return opts.v2 || (opts.legacy && opts.profiles === 1)
}

/** Mirrors the Rust `OverlayOutcome` (dm-domain ports.rs): Applied | Declined |
 *  Failed. Declined = UAC cancel; Failed = the restore could not be confirmed
 *  (the real host re-observes the registry — restore() can mutate it then fail
 *  cleanup, so the outcome alone is not authoritative). */
export type OverlayOutcome = 'applied' | 'declined' | 'failed'

export interface OverlayRestoreResult {
  /** The truthful post-operation overlay state (observed, not inferred). */
  arrowOverlay: ArrowOverlay
  toastKey: StringKey
  ok: boolean
}

/** Map an elevated overlay-restore outcome to its user-facing result. Only a
 *  confirmed Applied flips the overlay back to native; Declined and Failed leave
 *  it hidden so the restore entry stays available for a retry (review P2-5/P3). */
export function overlayRestoreResult(outcome: OverlayOutcome): OverlayRestoreResult {
  switch (outcome) {
    case 'applied':
      return { arrowOverlay: 'native', toastKey: 'Toast_ArrowRestored', ok: true }
    case 'declined':
      return { arrowOverlay: 'hidden', toastKey: 'Toast_ArrowRestoreDeclined', ok: false }
    case 'failed':
      return { arrowOverlay: 'hidden', toastKey: 'Toast_RestoreArrowFailed', ok: false }
  }
}
