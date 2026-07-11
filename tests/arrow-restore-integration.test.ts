import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, test } from 'bun:test'
import type { IconsOpResultDto, IconsStateDto } from '../src/bridge/types'
import {
  __setBridgeForTests,
  __setScanRetryMsForTests,
  resetIconsHistoryForTests,
  useIcons,
} from '../src/stores/icons'
import { BASE_CONFIGS, mockIconsCall, probeRealWallpaper } from '../src/bridge/mock-desktop'
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

const opResult = (over: Partial<IconsStateDto>, extra: Partial<IconsOpResultDto> = {}): IconsOpResultDto => ({
  state: seedState(over),
  toast: null,
  ok: true,
  ...extra,
})

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
      config: BASE_CONFIGS.spectrum,
      typeOverrides: {},
      overrides: [],
      label: 'x',
    })) as IconsOpResultDto
    expect(res.state.arrowOverlay).toBe('hidden')
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
      if (method !== 'icons.scan') throw new Error(`unexpected ${method}`)
      calls++
      if (calls === 1) return Promise.reject(new Error('bridge down')) // initial attempt fails
      return Promise.resolve({ revision: 1, items: [], state: seedState() }) // the retry succeeds
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
      if (method === 'icons.scan') return scanInflight
      if (method === 'icons.restoreOverlay') return restoreInflight
      throw new Error(`unexpected ${method}`)
    }) as unknown as Parameters<typeof __setBridgeForTests>[0])
    seedStore({ arrowOverlay: 'hidden', activeUserProfiles: 1 })

    // rescan starts FIRST (older generation), restore starts SECOND (newer).
    const pScan = useIcons.getState().rescan()
    const pRestore = useIcons.getState().restoreOverlay()

    // The OLDER rescan lands FIRST with a now-stale full state — must be dropped.
    resolveScan({ revision: 1, items: [], state: seedState({ arrowOverlay: 'hidden', activeUserProfiles: 1 }) })
    // The NEWER restore lands second with the truth the user actually wants.
    resolveRestore(opResult({ arrowOverlay: 'native', activeUserProfiles: 3 }))
    await Promise.all([pScan, pRestore])

    // Ordering by START, not arrival: the newer restore is applied, not falsely
    // dropped by the older rescan that merely returned first.
    expect(useIcons.getState().state!.arrowOverlay).toBe('native')
    expect(useIcons.getState().state!.activeUserProfiles).toBe(3)
  })

  test('scan retry backs off, terminates into scanExhausted, then retryScan recovers (review P2-2)', async () => {
    __setScanRetryMsForTests(1) // 1ms base → the whole backoff ladder resolves fast
    let calls = 0
    let mode: 'fail' | 'ok' = 'fail'
    __setBridgeForTests(((method: string) => {
      if (method !== 'icons.scan') throw new Error(`unexpected ${method}`)
      calls++
      if (mode === 'fail') return Promise.reject(new Error('permanently down'))
      return Promise.resolve({ revision: 1, items: [], state: seedState() })
    }) as unknown as Parameters<typeof __setBridgeForTests>[0])
    useIcons.setState({ loaded: false, state: null, scanExhausted: false, revision: 0 })

    await useIcons.getState().scan()
    // Poll until the backoff ladder gives up (robust to a loaded machine).
    await waitUntil(() => useIcons.getState().scanExhausted)
    expect(useIcons.getState().scanExhausted).toBe(true)
    const callsAtTerminal = calls
    expect(callsAtTerminal).toBeGreaterThan(1) // it DID retry
    expect(callsAtTerminal).toBeLessThanOrEqual(8) // but a bounded number of times

    // Terminal means TERMINAL: no more auto-retries spinning (no 3s log spam).
    await new Promise((r) => setTimeout(r, 40))
    expect(calls).toBe(callsAtTerminal)

    // The manual re-read entry recovers without a restart.
    mode = 'ok'
    useIcons.getState().retryScan()
    expect(useIcons.getState().scanExhausted).toBe(false)
    await waitUntil(() => useIcons.getState().state !== null)
    expect(useIcons.getState().loaded).toBe(true)
    expect(useIcons.getState().state).not.toBeNull()
  })
})
