// 清爽 module — per-control honest-state machine (spec 08 §3, ADR-0023 D3).
// Pure data + pure functions: the store drives transitions, tests assert legality.
// The iron rule this file encodes: 已生效 (verified) is reachable ONLY through
// pending (write → delayed read-back → effect verification), never directly.

/** Every state a control row can be in. */
export type CalmRowState =
  | 'unknown' // not probed yet
  | 'quiet' // probed: already off
  | 'pushing' // probed: the surface still pushes content
  | 'pending' // write issued, verification running (shimmer)
  | 'verified' // 已生效 — raw + delayed + effect verification all passed
  | 'setAwaiting' // 已设置 — written, effect needs sign-out / surface reopen
  | 'reverted' // apply failed, rolled back to original (honest toast)
  | 'needsReconfirm' // certification boundary crossed (feature update) — never auto-replay
  | 'unsupported' // fail-closed: this environment is not certified (our restraint)
  | 'managed' // policy/MDM managed — 由你的组织管理
  | 'confirmedOff' // guided + readable state re-probed off after the walk
  | 'userAttested' // guided + unreadable state, user says done — never counted as ours

/** States that count into the hero summary 「已让 N 处安静」 (verified writes only). */
export const COUNTED_AS_OURS: ReadonlySet<CalmRowState> = new Set(['verified'])

/** States rendered in group 3 「这个 Windows 版本暂时不碰的」. */
export const HELD_STATES: ReadonlySet<CalmRowState> = new Set(['unsupported', 'managed'])

// Legal transitions for WRITABLE (automatic/advanced) rows. Guided rows have their
// own map below — they can never enter the write pipeline.
const WRITABLE_TRANSITIONS: Readonly<Record<CalmRowState, readonly CalmRowState[]>> = {
  unknown: ['quiet', 'pushing', 'unsupported', 'managed'],
  quiet: ['pushing', 'unsupported', 'managed'], // drift / re-probe
  pushing: ['pending', 'quiet', 'unsupported', 'managed'],
  pending: ['verified', 'setAwaiting', 'reverted'],
  verified: ['pushing', 'needsReconfirm', 'quiet'], // in-boundary drift re-opens; boundary crossing
  setAwaiting: ['verified', 'pushing', 'needsReconfirm'],
  reverted: ['pushing', 'pending'], // user may retry
  needsReconfirm: ['pushing', 'quiet', 'unsupported', 'managed'], // only via a fresh probe
  unsupported: ['pushing', 'quiet', 'managed'], // a later certified probe may open it
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
