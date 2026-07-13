// 清爽 module — per-control honest-state machine (spec 08 §3, ADR-0023 D3).
// Pure data + pure functions: the store drives transitions, tests assert legality.
// Two channels, both encoded here:
//   1. the WRITE PIPELINE (canTransition/assertTransition) — 已生效 (verified) is
//      reachable ONLY through pending (write → delayed read-back → effect verify);
//   2. the PROBE channel (probeTransition) — ledger-backed environment truth. A
//      probe may RESTORE verified for a row the ledger owns whose value is intact,
//      but can never mint verified for a row we did not write.

/** Every state a control row can be in. */
export type CalmRowState =
  | 'unknown' // not probed yet
  | 'quiet' // probed: already off (not ours)
  | 'pushing' // probed: the surface still pushes content
  | 'pending' // write issued, verification running (shimmer)
  | 'verified' // 已生效 — raw + delayed + effect verification all passed
  | 'setAwaiting' // 已设置 — written, effect needs sign-out / surface reopen
  | 'reverted' // apply failed, rolled back to original (honest toast; retryable)
  | 'needsReconfirm' // certification boundary crossed (feature update) — never auto-replay
  | 'external' // we wrote it, the user changed it since — theirs now, untouched
  | 'unsupported' // fail-closed: this environment is not certified (our restraint)
  | 'managed' // policy/MDM managed — 由你的组织管理
  | 'confirmedOff' // guided + readable state re-probed off after the walk
  | 'userAttested' // guided + unreadable state, user says done — never counted as ours

/** States that count into the hero summary 「已让 N 处安静」 (verified writes only). */
export const COUNTED_AS_OURS: ReadonlySet<CalmRowState> = new Set(['verified'])

/** States meaning DeskMakeover currently owns a live write (gates the Restore action). */
export const OWNED_WRITES: ReadonlySet<CalmRowState> = new Set(['verified', 'setAwaiting'])

/** States rendered in group 3 「这个 Windows 版本暂时不碰的」. */
export const HELD_STATES: ReadonlySet<CalmRowState> = new Set(['unsupported', 'managed'])

// Legal transitions for WRITABLE (automatic/advanced) rows. Guided rows have their
// own map below — they can never enter the write pipeline.
const WRITABLE_TRANSITIONS: Readonly<Record<CalmRowState, readonly CalmRowState[]>> = {
  unknown: ['quiet', 'pushing', 'unsupported', 'managed', 'needsReconfirm'],
  quiet: ['pushing', 'unsupported', 'managed', 'needsReconfirm'], // drift / re-probe
  pushing: ['pending', 'quiet', 'unsupported', 'managed', 'needsReconfirm'],
  pending: ['verified', 'setAwaiting', 'reverted', 'pushing', 'unknown'], // pushing = skipped; unknown = reply lost, re-probe required
  verified: ['pushing', 'needsReconfirm', 'quiet', 'external'],
  setAwaiting: ['verified', 'pushing', 'needsReconfirm', 'external'],
  reverted: ['pushing', 'pending', 'quiet', 'unsupported', 'managed'], // retry or re-probe
  needsReconfirm: ['pushing', 'quiet', 'unsupported', 'managed'], // only via a fresh probe
  external: ['pushing', 'quiet', 'unsupported', 'managed'], // re-probe reads it fresh
  unsupported: ['pushing', 'quiet', 'managed', 'needsReconfirm'], // a later certified probe may open it
  managed: ['pushing', 'quiet', 'unsupported'],
  confirmedOff: [],
  userAttested: [],
}

const GUIDED_TRANSITIONS: Readonly<Record<CalmRowState, readonly CalmRowState[]>> = {
  unknown: ['pushing', 'quiet', 'confirmedOff'],
  pushing: ['confirmedOff', 'userAttested', 'quiet'],
  quiet: ['pushing'],
  confirmedOff: ['pushing'], // Windows turned it back on
  userAttested: ['pushing', 'confirmedOff'],
  pending: [],
  verified: [],
  setAwaiting: [],
  reverted: [],
  needsReconfirm: [],
  external: [],
  unsupported: ['pushing', 'quiet'],
  managed: ['pushing', 'quiet'],
}

export function canTransition(kind: 'writable' | 'guided', from: CalmRowState, to: CalmRowState): boolean {
  const table = kind === 'writable' ? WRITABLE_TRANSITIONS : GUIDED_TRANSITIONS
  return table[from].includes(to)
}

/** Guard used by the store — throws in dev if a transition would lie to the user. */
export function assertTransition(kind: 'writable' | 'guided', from: CalmRowState, to: CalmRowState): CalmRowState {
  if (!canTransition(kind, from, to)) {
    throw new Error(`calm: illegal ${kind} transition ${from} -> ${to}`)
  }
  return to
}

/** What a probe may report for a row. */
export type CalmProbeState = 'quiet' | 'pushing' | 'unsupported' | 'managed' | 'needsReconfirm'

/**
 * The PROBE channel (ledger-backed). `ownedByUs` = the backend's durable ledger says
 * DeskMakeover wrote this row AND the raw value still matches what it wrote.
 * - owned + quiet  → verified   (ownership restored — the ledger re-verified it)
 * - owned + pushing → pushing   (drift: the value moved; ownership is NOT claimed)
 * - not owned      → the probe state as-is
 * A probe can never produce pending, and never mints verified without ownership.
 */
export function probeTransition(
  kind: 'writable' | 'guided',
  from: CalmRowState,
  probe: { state: CalmProbeState; ownedByUs?: boolean },
): CalmRowState {
  if (kind === 'guided') {
    // Guided rows have no ledger — settled guided outcomes survive a re-probe of
    // 'pushing' only by explicit drift (handled where the walk result is read).
    if (from === 'confirmedOff' && probe.state === 'quiet') return 'confirmedOff'
    if (from === 'userAttested' && probe.state !== 'pushing') return from
    const to = probe.state === 'managed' || probe.state === 'needsReconfirm' ? 'pushing' : probe.state
    return from === to ? from : assertTransition('guided', from, to)
  }
  // A crossed certification boundary outranks ownership (codex R2): a stale
  // ledger claim must never bypass 需重新确认.
  if (probe.state === 'needsReconfirm') {
    return from === 'needsReconfirm' ? from : assertTransition('writable', from, 'needsReconfirm')
  }
  if (probe.ownedByUs && probe.state === 'quiet') return 'verified'
  const to = probe.state
  return from === to ? from : assertTransition('writable', from, to)
}
