import { describe, expect, test } from 'bun:test'
import {
  arrowRowView,
  consentSatisfied,
  overlayRestoreResult,
  showDoneArrowNote,
} from '../src/lib/arrow-overlay'

// Behavioral coverage for the arrow-overlay decision layer (review Phase 9
// gate). These are the exact edges the code review flagged: unknown-state
// status guards, DoneCard truthfulness, the multi-user consent gate, and the
// Applied|Declined|Failed elevated-restore contract. Kept pure so they run in
// the DOM-free test harness the store tests already use.

describe('arrowRowView — status guard (review P2-2)', () => {
  test('hidden → actionable: hidden status + restore shown', () => {
    expect(arrowRowView('hidden')).toEqual({
      statusKey: 'Settings_ArrowStatusHidden',
      showRestore: true,
      unknown: false,
    })
  })

  test('native → nothing to restore: default status, no action', () => {
    expect(arrowRowView('native')).toEqual({
      statusKey: 'Settings_ArrowStatusNative',
      showRestore: false,
      unknown: false,
    })
  })

  test('undefined (scan pending/failed) → checking, NEVER a false "Windows default", and keeps no action rather than a wrong one', () => {
    const v = arrowRowView(undefined)
    expect(v.statusKey).toBe('Settings_ArrowStatusChecking')
    expect(v.unknown).toBe(true)
    expect(v.showRestore).toBe(false)
    // The bug being guarded: unknown must not masquerade as native.
    expect(v.statusKey).not.toBe('Settings_ArrowStatusNative')
  })
})

describe('showDoneArrowNote — DoneCard truthfulness (review P2-1)', () => {
  test('claims the hide only when the overlay is actually hidden now', () => {
    expect(showDoneArrowNote('hidden')).toBe(true)
    expect(showDoneArrowNote('native')).toBe(false)
    expect(showDoneArrowNote(undefined)).toBe(false)
  })
})

describe('consentSatisfied — multi-user disclosure gate (review P2-3)', () => {
  test('v2 consent always skips, regardless of profile count', () => {
    expect(consentSatisfied({ v2: true, legacy: false, profiles: 1 })).toBe(true)
    expect(consentSatisfied({ v2: true, legacy: false, profiles: 5 })).toBe(true)
  })

  test('legacy consent grandfathers a single-user machine', () => {
    expect(consentSatisfied({ v2: false, legacy: true, profiles: 1 })).toBe(true)
  })

  test('legacy consent does NOT skip on a multi-user machine — the non-skippable disclosure must re-show', () => {
    expect(consentSatisfied({ v2: false, legacy: true, profiles: 2 })).toBe(false)
  })

  test('legacy consent fails CLOSED on a malformed count — zero/negative is not "known single user"', () => {
    // The single-user exception is EXACTLY 1; a bogus host count must gate, not
    // grandfather (review new-P3). `<= 1` would have let 0 and -1 slip through.
    expect(consentSatisfied({ v2: false, legacy: true, profiles: 0 })).toBe(false)
    expect(consentSatisfied({ v2: false, legacy: true, profiles: -1 })).toBe(false)
  })

  test('never consented → always gated', () => {
    expect(consentSatisfied({ v2: false, legacy: false, profiles: 1 })).toBe(false)
    expect(consentSatisfied({ v2: false, legacy: false, profiles: 3 })).toBe(false)
  })
})

describe('overlayRestoreResult — Applied|Declined|Failed contract (review P2-5, P3)', () => {
  test('Applied is the only outcome that flips the overlay back to native', () => {
    expect(overlayRestoreResult('applied')).toEqual({
      arrowOverlay: 'native',
      toastKey: 'Toast_ArrowRestored',
      ok: true,
    })
  })

  test('Declined (UAC cancel) leaves the arrow hidden and is not an error toast key', () => {
    const r = overlayRestoreResult('declined')
    expect(r.arrowOverlay).toBe('hidden')
    expect(r.ok).toBe(false)
    expect(r.toastKey).toBe('Toast_ArrowRestoreDeclined')
  })

  test('Failed leaves the arrow hidden (restore not confirmed) with a restore-specific toast, never Toast_ApplyFailed', () => {
    const r = overlayRestoreResult('failed')
    expect(r.arrowOverlay).toBe('hidden')
    expect(r.ok).toBe(false)
    expect(r.toastKey).toBe('Toast_RestoreArrowFailed')
    expect(r.toastKey).not.toBe('Toast_ApplyFailed')
  })

  test('a non-applied outcome always keeps the restore entry reachable (still hidden)', () => {
    for (const o of ['declined', 'failed'] as const) {
      expect(arrowRowView(overlayRestoreResult(o).arrowOverlay).showRestore).toBe(true)
    }
  })
})
