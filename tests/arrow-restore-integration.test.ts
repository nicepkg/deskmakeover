import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, test } from 'bun:test'
import type { IconOpResultDto, IconPersistedDto, IconScanDto, IconsStateDto } from '../src/bridge/types'
import {
  __setBridgeForTests,
  __setScanRetryMsForTests,
  resetIconsHistoryForTests,
  scanRetryDelayMs,
  SCAN_RETRY_CAP_MS,
  SCAN_RETRY_MAX,
  useIcons,
} from '../src/stores/icons'
import { mockIconsCall, probeRealWallpaper } from '../src/bridge/mock-desktop'
import { BASE_CONFIGS } from '../src/lib/icons-assemble'
import { useToasts } from '../src/stores/toasts'
import { DEFAULT_KIND_POLICY } from '../src/lib/kind-policy'
import { t } from '../src/lib/i18n'

// Integration coverage for the arrow-restore wiring (review P3-8): unlike the
// pure decision tests, these import the STORE and the MOCK and exercise the real
// action bodies, so reverting the actual wiring (single-flight, cross-op race,
// scan self-recovery, the faithful mock outcomes) makes a test go RED.
//
// Two halves:
//  · Real-mock: the store calls the untouched `call` → mock-desktop bridge, so
//    the mock's icons.restoreOverlay / apply / restore transitions are pinned.
//  · Fake-bridge: an injected controllable bridge drives out-of-order / rejected
//    responses that a real mock can't produce deterministically.

// ---- minimal browser-global stubs so the real mock's state() resolves without
// a DOM. probeRealWallpaper caches a wallpaper URL from a HEAD fetch, which lets
// mockWallpaperUrl() short-circuit before it ever touches a <canvas>. ----
function makeStorage(): Storage {
  const m = new Map<string, string>()
  return {
    get length() {
      return m.size
    },
    clear: () => m.clear(),
    getItem: (k: string) => (m.has(k) ? m.get(k)! : null),
    key: (i: number) => [...m.keys()][i] ?? null,
    removeItem: (k: string) => void m.delete(k),
    setItem: (k: string, v: string) => void m.set(k, String(v)),
  } as Storage
}

const g = globalThis as Record<string, unknown>
const saved = {
  localStorage: g.localStorage,
  document: g.document,
  window: g.window,
  fetch: g.fetch,
}

beforeAll(async () => {
  g.localStorage = makeStorage()
  g.document = { documentElement: { classList: { contains: () => false } } }
  g.window = g.window ?? {}
  g.fetch = async () => ({ ok: true, json: async () => [] })
  // Cache a real wallpaper URL so the mock's state() never builds a canvas.
  await probeRealWallpaper()
})

afterAll(() => {
  g.localStorage = saved.localStorage
  g.document = saved.document
  g.window = saved.window
  g.fetch = saved.fetch
})

const seedState = (over: Partial<IconsStateDto> = {}): IconsStateDto => ({
  scanning: false,
  working: false,
  applied: false,
  dirty: false,
  styleableCount: 0,
  config: { ...BASE_CONFIGS.spectrum },
  activePresetId: null,
  presets: [],
  history: [],
  palette: [],
  monoSwatches: [],
  markSwatches: [],
  grid: {
    screenWidth: 1920, screenHeight: 1080, taskbarHeight: 48,
    iconPx: 48, cellWidth: 92, cellHeight: 96, inset: 14, labelFontPx: 12,
  },
  wallpaperUrl: null,
  kindPolicy: { ...DEFAULT_KIND_POLICY },
  typeOverrides: {},
  arrowOverlay: 'native',
  activeUserProfiles: 1,
  ...over,
})

const seedStore = (over: Partial<IconsStateDto> = {}) =>
  useIcons.setState({ loaded: true, state: seedState(over), items: [], revision: 0, overlayRestoring: false })

// Thin bridge shapes (schema 7): op results carry `persisted`, scans carry only revision + items,
// and the store fetches getPersisted alongside every scan.
const persistedDto = (over: Partial<IconPersistedDto> = {}): IconPersistedDto => ({
  savedStyleJson: null,
  history: [],
  applied: false,
  arrowOverlay: 'native',
  activeUserProfiles: 1,
  ...over,
})

const opResult = (over: Partial<IconPersistedDto>, extra: Partial<IconOpResultDto> = {}): IconOpResultDto => ({
  ok: true,
  toast: null,
  persisted: persistedDto(over),
  ...extra,
})

const scanDto = (revision = 1): IconScanDto => ({ revision, items: [] })

beforeEach(() => {
  resetIconsHistoryForTests()
  useToasts.setState({ toasts: [] })
  localStorage.removeItem('dm.dev.restoreOutcome')
  localStorage.removeItem('dm.dev.userProfiles')
})

afterEach(() => resetIconsHistoryForTests())

const lastToast = () => useToasts.getState().toasts.at(-1)?.text

// Poll a predicate instead of guessing a fixed wall-clock wait — the retry
// backoff runs on real timers, so a machine under load must not flake the test.
const waitUntil = async (pred: () => boolean, timeoutMs = 3000): Promise<void> => {
  const start = Date.now()
  while (!pred() && Date.now() - start < timeoutMs) {
    await new Promise((r) => setTimeout(r, 5))
  }
}

type FakeBridge = (method: string, params?: unknown) => Promise<unknown>
const setBridge = (fn: FakeBridge) =>
  __setBridgeForTests(fn as unknown as Parameters<typeof __setBridgeForTests>[0])
const deferred = <T>() => {
  let resolve!: (v: T) => void
  let reject!: (e: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('real mock through the store — the mock transitions are pinned', () => {
  test('restoreOverlay (default applied) flips the arrow to native and shows the restore toast', async () => {
    seedStore({ arrowOverlay: 'hidden' })
    await useIcons.getState().restoreOverlay()
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
    expect(lastToast()).toBe(t('Toast_ArrowRestored'))
  })

  test('restoreOverlay declined leaves the arrow hidden with the declined toast (not apply-failed)', async () => {
    localStorage.setItem('dm.dev.restoreOutcome', 'declined')
    seedStore({ arrowOverlay: 'hidden' })
    await useIcons.getState().restoreOverlay()
    expect(useIcons.getState().state!.arrowOverlay).toBe('hidden')
    expect(lastToast()).toBe(t('Toast_ArrowRestoreDeclined'))
    expect(lastToast()).not.toBe(t('Toast_ApplyFailed'))
  })

  test('restoreOverlay failed leaves the arrow hidden with the restore-specific failure toast', async () => {
    localStorage.setItem('dm.dev.restoreOutcome', 'failed')
    seedStore({ arrowOverlay: 'hidden' })
    await useIcons.getState().restoreOverlay()
    expect(useIcons.getState().state!.arrowOverlay).toBe('hidden')
    expect(lastToast()).toBe(t('Toast_RestoreArrowFailed'))
    expect(lastToast()).not.toBe(t('Toast_ApplyFailed'))
  })

  test('full restore lifts the arrow back to native', async () => {
    seedStore({ arrowOverlay: 'hidden', applied: true })
    await useIcons.getState().restore()
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
  })

  test('applyBakedCommit hides the arrow (machine-wide overlay installed)', async () => {
    const res = (await mockIconsCall('icons.applyBakedCommit', {
      styleJson: JSON.stringify({ config: BASE_CONFIGS.spectrum, kindPolicy: {}, typeOverrides: {} }),
      label: 'x',
    })) as IconOpResultDto
    expect(res.persisted.arrowOverlay).toBe('hidden')
  })
})

describe('store restore logic — fake bridge for deterministic timing', () => {
  test('restoreOverlay is single-flight: a second click while in flight never re-calls the host', async () => {
    let calls = 0
    let resolveRestore!: (v: IconsOpResultDto) => void
    const inflight = new Promise<IconsOpResultDto>((r) => (resolveRestore = r))
    __setBridgeForTests(((method: string) => {
      if (method === 'icons.restoreOverlay') {
        calls++
        return inflight
      }
      throw new Error(`unexpected ${method}`)
    }) as unknown as Parameters<typeof __setBridgeForTests>[0])
    seedStore({ arrowOverlay: 'hidden' })

    const p1 = useIcons.getState().restoreOverlay()
    const p2 = useIcons.getState().restoreOverlay() // blocked by overlayRestoring
    expect(calls).toBe(1)
    expect(useIcons.getState().overlayRestoring).toBe(true)
    resolveRestore(opResult({ arrowOverlay: 'native' }))
    await Promise.all([p1, p2])
    expect(calls).toBe(1)
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
    expect(useIcons.getState().overlayRestoring).toBe(false)
  })

  test('a stale restore response is DROPPED after an intervening op (no state / profile regression)', async () => {
    let resolveRestore!: (v: IconsOpResultDto) => void
    const inflight = new Promise<IconsOpResultDto>((r) => (resolveRestore = r))
    __setBridgeForTests(((method: string) => {
      if (method === 'icons.restoreOverlay') return inflight
      if (method === 'icons.restore') return Promise.resolve(opResult({ arrowOverlay: 'native', activeUserProfiles: 3 }))
      throw new Error(`unexpected ${method}`)
    }) as unknown as Parameters<typeof __setBridgeForTests>[0])
    seedStore({ arrowOverlay: 'hidden', activeUserProfiles: 1 })

    const p = useIcons.getState().restoreOverlay() // in flight; generation claimed
    // An intervening authoritative op (full restore) starts + lands, taking a newer generation.
    await useIcons.getState().restore()
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
    expect(useIcons.getState().state!.activeUserProfiles).toBe(3)

    // The late restore response now arrives with a STALE observation (hidden / 1).
    resolveRestore(opResult({ arrowOverlay: 'hidden', activeUserProfiles: 1 }, { toast: { key: 'Toast_ArrowRestored', arg: null } }))
    await p

    // Dropped: the intervening op's truth stands, not the stale restore's.
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
    expect(useIcons.getState().state!.activeUserProfiles).toBe(3)
    expect(useIcons.getState().overlayRestoring).toBe(false)
  })

  test('a transport rejection reports the restore-specific failure, never Toast_ApplyFailed (review P3-7)', async () => {
    __setBridgeForTests(((method: string) => {
      if (method === 'icons.restoreOverlay') return Promise.reject(new Error('bridge down'))
      throw new Error(`unexpected ${method}`)
    }) as unknown as Parameters<typeof __setBridgeForTests>[0])
    seedStore({ arrowOverlay: 'hidden' })
    await useIcons.getState().restoreOverlay()
    expect(lastToast()).toBe(t('Toast_RestoreArrowFailed'))
    expect(lastToast()).not.toBe(t('Toast_ApplyFailed'))
    // Overlay stays hidden → the restore entry remains reachable for a retry.
    expect(useIcons.getState().state!.arrowOverlay).toBe('hidden')
    expect(useIcons.getState().overlayRestoring).toBe(false)
  })

  test('a failed initial scan self-retries and recovers (never stranded in checking) — review P2-2', async () => {
    __setScanRetryMsForTests(10)
    let calls = 0
    __setBridgeForTests(((method: string) => {
      if (method === 'icons.getPersisted') return Promise.resolve(persistedDto())
      if (method !== 'icons.scan') throw new Error(`unexpected ${method}`)
      calls++
      if (calls === 1) return Promise.reject(new Error('bridge down')) // initial attempt fails
      return Promise.resolve(scanDto(1)) // the retry succeeds
    }) as unknown as Parameters<typeof __setBridgeForTests>[0])
    useIcons.setState({ loaded: false, state: null, revision: 0 })

    await useIcons.getState().scan()
    // Failed attempt: gate reset (retryable) and state still null — NEVER a false native.
    expect(calls).toBe(1)
    expect(useIcons.getState().loaded).toBe(false)
    expect(useIcons.getState().state).toBeNull()

    // The store's OWN scheduled retry fires with no external trigger and recovers.
    await waitUntil(() => useIcons.getState().state !== null)
    expect(calls).toBe(2)
    expect(useIcons.getState().loaded).toBe(true)
    expect(useIcons.getState().state).not.toBeNull()
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
  })

  test('a stale exportCompare response does NOT overwrite a newer op (review P2-4a)', async () => {
    let resolveExport!: (v: IconsOpResultDto) => void
    const inflight = new Promise<IconsOpResultDto>((r) => (resolveExport = r))
    __setBridgeForTests(((method: string) => {
      if (method === 'icons.exportCompare') return inflight
      if (method === 'icons.restore') return Promise.resolve(opResult({ arrowOverlay: 'native', activeUserProfiles: 3 }))
      throw new Error(`unexpected ${method}`)
    }) as unknown as Parameters<typeof __setBridgeForTests>[0])
    seedStore({ arrowOverlay: 'hidden', activeUserProfiles: 1 })

    const p = useIcons.getState().exportCompare() // gen claimed, in flight
    // A newer op (full restore) starts + lands with the authoritative truth.
    await useIcons.getState().restore()
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
    expect(useIcons.getState().state!.activeUserProfiles).toBe(3)

    // Export's late response carries a STALE full-state snapshot (hidden / 1).
    resolveExport(opResult({ arrowOverlay: 'hidden', activeUserProfiles: 1 }))
    await p

    // Dropped the stale wholesale replace — the newer restore's truth stands.
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
    expect(useIcons.getState().state!.activeUserProfiles).toBe(3)
  })

  test('a newer restore is NOT falsely dropped by an older rescan that lands first (review P2-4b)', async () => {
    let resolveScan!: (v: unknown) => void
    let resolveRestore!: (v: IconsOpResultDto) => void
    const scanInflight = new Promise((r) => (resolveScan = r))
    const restoreInflight = new Promise<IconsOpResultDto>((r) => (resolveRestore = r))
    __setBridgeForTests(((method: string) => {
      if (method === 'icons.getPersisted') return Promise.resolve(persistedDto())
      if (method === 'icons.scan') return scanInflight
      if (method === 'icons.restoreOverlay') return restoreInflight
      throw new Error(`unexpected ${method}`)
    }) as unknown as Parameters<typeof __setBridgeForTests>[0])
    seedStore({ arrowOverlay: 'hidden', activeUserProfiles: 1 })

    // rescan starts FIRST (older generation), restore starts SECOND (newer).
    const pScan = useIcons.getState().rescan()
    const pRestore = useIcons.getState().restoreOverlay()

    // The OLDER rescan lands FIRST with a now-stale full state — must be dropped.
    resolveScan(scanDto(1))
    // The NEWER restore lands second with the truth the user actually wants.
    resolveRestore(opResult({ arrowOverlay: 'native', activeUserProfiles: 3 }))
    await Promise.all([pScan, pRestore])

    // Ordering by START, not arrival: the newer restore is applied, not falsely
    // dropped by the older rescan that merely returned first.
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
    expect(useIcons.getState().state!.activeUserProfiles).toBe(3)
  })

  test('a modal apply commit self-drops when superseded, and releases working (review P2 modal + item 1)', async () => {
    const commit = deferred<IconsOpResultDto>()
    setBridge((method) => {
      if (method === 'icons.applyBakedBegin' || method === 'icons.applyBakedChunk') return Promise.resolve(null)
      if (method === 'icons.applyBakedCommit') return commit.promise
      // A DELTA-MERGE superseder (arrow restore) that does NOT touch `working`, so
      // apply's optimistic working:true survives until apply's own branch clears it.
      if (method === 'icons.restoreOverlay') return Promise.resolve(opResult({ arrowOverlay: 'native', activeUserProfiles: 3 }))
      throw new Error(`unexpected ${method}`)
    })
    seedStore({ arrowOverlay: 'native', activeUserProfiles: 1 }) // no items → apply skips the bake loop

    const pApply = useIcons.getState().apply() // gen claimed; awaiting the commit (working:true)
    await useIcons.getState().restoreOverlay() // newer writer lands first, leaves working:true
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
    expect(useIcons.getState().state!.activeUserProfiles).toBe(3)

    // The stale apply commit now arrives with the OLD look (hidden / 1).
    commit.resolve(opResult({ arrowOverlay: 'hidden', activeUserProfiles: 1 }))
    expect(await pApply).toBe(false) // superseded apply does not claim success
    expect(useIcons.getState().state!.arrowOverlay).toBe('native') // stale look dropped
    expect(useIcons.getState().state!.activeUserProfiles).toBe(3)
    expect(useIcons.getState().applyProgress).toBeNull() // progress veil cleared
    expect(useIcons.getState().state!.working).toBe(false) // working released (no UI lock)
  })

  test('a modal full-restore self-drops when superseded, and releases working (review P2 modal + item 1)', async () => {
    const full = deferred<IconsOpResultDto>()
    setBridge((method) => {
      if (method === 'icons.restore') return full.promise
      // Delta-merge superseder leaves this restore's optimistic working:true in place.
      if (method === 'icons.restoreOverlay') return Promise.resolve(opResult({ arrowOverlay: 'hidden', activeUserProfiles: 2 }))
      throw new Error(`unexpected ${method}`)
    })
    seedStore({ arrowOverlay: 'native', activeUserProfiles: 1, applied: true })

    const pFull = useIcons.getState().restore() // gen claimed; awaiting host (working:true)
    await useIcons.getState().restoreOverlay() // newer writer lands first, leaves working:true
    expect(useIcons.getState().state!.arrowOverlay).toBe('hidden')
    expect(useIcons.getState().state!.activeUserProfiles).toBe(2)

    // The stale full-restore now arrives with the OLD truth (native / 1).
    full.resolve(opResult({ arrowOverlay: 'native', activeUserProfiles: 1 }))
    await pFull
    expect(useIcons.getState().state!.arrowOverlay).toBe('hidden') // stale truth dropped
    expect(useIcons.getState().state!.activeUserProfiles).toBe(2)
    expect(useIcons.getState().state!.working).toBe(false) // working released (no UI lock)
  })

  test('a stale rescan that lands LAST is dropped — its gen guard is load-bearing (review scan gate)', async () => {
    const scan = deferred<unknown>()
    setBridge((method) => {
      if (method === 'icons.getPersisted') return Promise.resolve(persistedDto())
      if (method === 'icons.scan') return scan.promise
      if (method === 'icons.restoreOverlay') return Promise.resolve(opResult({ arrowOverlay: 'native', activeUserProfiles: 3 }))
      throw new Error(`unexpected ${method}`)
    })
    seedStore({ arrowOverlay: 'hidden', activeUserProfiles: 1 })

    const pScan = useIcons.getState().rescan() // OLDER gen, in flight
    // A newer writer lands FIRST and establishes the truth.
    await useIcons.getState().restoreOverlay()
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')

    // The OLDER rescan now lands LAST with a stale full-state snapshot — the
    // guard must drop it (this is the case the reverse-start test could not pin,
    // because there the late restore masked the transient clobber).
    scan.resolve(scanDto(1))
    await pScan
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
    expect(useIcons.getState().state!.activeUserProfiles).toBe(3)
  })

  test('scan backoff terminates after EXACTLY SCAN_RETRY_MAX attempts, then stays terminal (review P2-2 item 3)', async () => {
    __setScanRetryMsForTests(1) // 1ms base → the whole ladder resolves fast
    let calls = 0
    setBridge((method) => {
      if (method === 'icons.getPersisted') return Promise.resolve(persistedDto())
      if (method !== 'icons.scan') throw new Error(`unexpected ${method}`)
      calls++
      return Promise.reject(new Error('permanently down'))
    })
    useIcons.setState({ loaded: false, state: null, scanExhausted: false, revision: 0 })

    await useIcons.getState().scan()
    await waitUntil(() => useIcons.getState().scanExhausted)
    expect(useIcons.getState().scanExhausted).toBe(true)
    // Exactly SCAN_RETRY_MAX total attempts (initial + retries), no off-by-one.
    expect(calls).toBe(SCAN_RETRY_MAX)
    // Terminal is TERMINAL: no more auto-retries spinning (no log spam).
    await new Promise((r) => setTimeout(r, 40))
    expect(calls).toBe(SCAN_RETRY_MAX)
  })

  test('a successful rescan clears a prior scanExhausted (review P2-2 item 4)', async () => {
    __setScanRetryMsForTests(1)
    let calls = 0
    let mode: 'fail' | 'ok' = 'fail'
    setBridge((method) => {
      if (method === 'icons.getPersisted') return Promise.resolve(persistedDto())
      if (method !== 'icons.scan') throw new Error(`unexpected ${method}`)
      calls++
      if (mode === 'fail') return Promise.reject(new Error('down'))
      return Promise.resolve(scanDto(calls))
    })
    useIcons.setState({ loaded: false, state: null, scanExhausted: false, revision: 0 })

    // Exhaust the budget into the terminal state.
    await useIcons.getState().scan()
    await waitUntil(() => useIcons.getState().scanExhausted)
    expect(useIcons.getState().scanExhausted).toBe(true)

    // A successful full load through ANY path (here a rescan, which does not go
    // through retryScan's own reset) must heal the exhausted flag — adoptScan owns
    // the reset so every successful load clears it, not just the manual button.
    mode = 'ok'
    await useIcons.getState().rescan()
    expect(useIcons.getState().scanExhausted).toBe(false)
    expect(useIcons.getState().state).not.toBeNull()
  })

  test('a scheduled scan retry that was superseded is discarded, not re-fired (review timer gen)', async () => {
    __setScanRetryMsForTests(30) // long enough to interpose a newer op before the retry fires
    let scanCalls = 0
    setBridge((method) => {
      if (method === 'icons.getPersisted') return Promise.resolve(persistedDto())
      if (method === 'icons.scan') {
        scanCalls++
        return Promise.reject(new Error('down'))
      }
      if (method === 'icons.restoreOverlay') return Promise.resolve(opResult({ arrowOverlay: 'native', activeUserProfiles: 3 }))
      throw new Error(`unexpected ${method}`)
    })
    useIcons.setState({ loaded: false, state: seedState({ arrowOverlay: 'hidden', activeUserProfiles: 1 }), scanExhausted: false, revision: 0 })

    // A failed scan schedules a retry that belongs to its generation.
    await useIcons.getState().scan()
    expect(scanCalls).toBe(1)
    // A newer op supersedes the failed scan BEFORE its retry timer fires.
    await useIcons.getState().restoreOverlay()
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')

    // Past the retry delay: the superseded retry must be discarded — no fresh scan
    // that would claim a newer gen and invalidate the restore's legitimate truth.
    await new Promise((r) => setTimeout(r, 70))
    expect(scanCalls).toBe(1) // retry NOT re-fired
    expect(useIcons.getState().state!.arrowOverlay).toBe('native') // restore's truth intact
    expect(useIcons.getState().state!.activeUserProfiles).toBe(3)
  })

  test('a superseded failed scan does not flip loaded back to false (review item 2)', async () => {
    const g1 = deferred<unknown>() // the OLD scan, will reject
    const g2 = deferred<unknown>() // the NEWER scan, will succeed
    let n = 0
    setBridge((method) => {
      if (method === 'icons.getPersisted') return Promise.resolve(persistedDto())
      if (method !== 'icons.scan') throw new Error(`unexpected ${method}`)
      n++
      return n === 1 ? g1.promise : g2.promise
    })
    useIcons.setState({ loaded: false, state: null, scanExhausted: false, revision: 0 })

    const p1 = useIcons.getState().scan() // G1: loaded:true, awaiting g1
    useIcons.getState().retryScan() // resets gate + starts G2: loaded:true, awaiting g2

    // The NEWER scan succeeds first and owns the loaded state.
    g2.resolve(scanDto(1))
    await waitUntil(() => useIcons.getState().state !== null)
    expect(useIcons.getState().loaded).toBe(true)

    // The OLD scan now rejects — it must NOT flip loaded back to false, because
    // the gen guard runs BEFORE the state write (item 2: order of the two lines).
    g1.reject(new Error('stale down'))
    await p1.catch(() => {})
    expect(useIcons.getState().loaded).toBe(true)
  })
})

describe('scanRetryDelayMs — exponential backoff ladder + cap (review P2-2)', () => {
  test('doubles each attempt then saturates at the cap', () => {
    expect(scanRetryDelayMs(1, 2000, SCAN_RETRY_CAP_MS)).toBe(2000)
    expect(scanRetryDelayMs(2, 2000, SCAN_RETRY_CAP_MS)).toBe(4000)
    expect(scanRetryDelayMs(3, 2000, SCAN_RETRY_CAP_MS)).toBe(8000)
    expect(scanRetryDelayMs(4, 2000, SCAN_RETRY_CAP_MS)).toBe(16000)
    // 2000 * 2^4 = 32000 > 30000 cap → clamped.
    expect(scanRetryDelayMs(5, 2000, SCAN_RETRY_CAP_MS)).toBe(SCAN_RETRY_CAP_MS)
    expect(scanRetryDelayMs(9, 2000, SCAN_RETRY_CAP_MS)).toBe(SCAN_RETRY_CAP_MS)
  })
})
