import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import type { ConfigDto, IconItemDto, IconsStateDto } from '../src/bridge/types'
import { __setBridgeForTests, effectiveTileConfig, persistBareLook, readBareLook, resetIconsHistoryForTests, resumeStatusKey, useIcons } from '../src/stores/icons'
import { DEFAULT_KIND_POLICY } from '../src/lib/kind-policy'

// Undo granularity + override semantics (spec 06 §3.3/§3.4): one step per
// discrete pick, gesture-coalesced wheels, hover never snapshots.

const config: ConfigDto = {
  shape: 'Apple',
  subject: 'Original',
  monoStyle: 'Tonal',
  tint: '#FF6F5E',
  distinction: 'None',
  markStyle: 'Arc',
  markColor: null,
  plateColor: null,
  size: 'Mid',
  filter: 'None',
}

const item = (id: string, over: Partial<IconItemDto> = {}): IconItemDto => ({
  id,
  label: id,
  kind: 'Shortcut',
  isShortcut: true,
  styleable: true,
  statusReason: null,
  x: 0,
  y: 0,
  sourceUrls: [],
  overrideMode: null,
  overrideTint: null,
  ...over,
})

const state = (): IconsStateDto => ({
  scanning: false,
  working: false,
  applied: false,
  dirty: false,
  styleableCount: 2,
  config: { ...config },
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
  arrowOverlay: 'native',
  activeUserProfiles: 1,
})

beforeEach(() => {
  resetIconsHistoryForTests()
  useIcons.setState({
    state: state(),
    items: [item('a'), item('b', { styleable: false, kind: 'AppxShortcut', isShortcut: false })],
    canUndo: false,
    canRedo: false,
    hoverConfig: null,
    bareLook: false,
  })
})

afterEach(() => resetIconsHistoryForTests())

describe('undo granularity', () => {
  test('two discrete picks are TWO steps (no time-window merging)', () => {
    const s = useIcons.getState()
    s.mutate({ shape: 'Circle' })
    s.mutate({ shape: 'Samsung' })
    expect(useIcons.getState().state!.config.shape).toBe('Samsung')
    useIcons.getState().undo()
    expect(useIcons.getState().state!.config.shape).toBe('Circle')
    useIcons.getState().undo()
    expect(useIcons.getState().state!.config.shape).toBe('Apple')
    expect(useIcons.getState().canUndo).toBe(false)
  })

  test('a wheel gesture coalesces every change into ONE step', () => {
    const s = useIcons.getState()
    s.beginGesture()
    s.mutate({ subject: 'Mono', tint: '#101010' })
    s.mutate({ tint: '#202020' })
    s.mutate({ tint: '#303030' })
    s.endGesture()
    expect(useIcons.getState().state!.config.tint).toBe('#303030')
    useIcons.getState().undo()
    expect(useIcons.getState().state!.config.tint).toBe('#FF6F5E')
    expect(useIcons.getState().state!.config.subject).toBe('Original')
    expect(useIcons.getState().canUndo).toBe(false)
  })

  test('redo replays an undone pick; a new pick clears the redo lane', () => {
    const s = useIcons.getState()
    s.mutate({ filter: 'Glass' })
    useIcons.getState().undo()
    expect(useIcons.getState().canRedo).toBe(true)
    useIcons.getState().redo()
    expect(useIcons.getState().state!.config.filter).toBe('Glass')
    useIcons.getState().undo()
    useIcons.getState().mutate({ filter: 'Sticker' })
    expect(useIcons.getState().canRedo).toBe(false)
  })

  test('hover try-on never touches config or history', () => {
    const s = useIcons.getState()
    s.hover({ shape: 'Flower' })
    expect(useIcons.getState().hoverConfig!.shape).toBe('Flower')
    expect(useIcons.getState().state!.config.shape).toBe('Apple')
    expect(useIcons.getState().canUndo).toBe(false)
    s.hover(null)
    expect(useIcons.getState().hoverConfig).toBeNull()
  })

  test('bare hover try-on (System Default card) previews without committing', () => {
    const s = useIcons.getState()
    const before = { ...s.state!.config }
    s.hoverBare(true)
    // Preview channel only — the working design, bare flag and history are untouched.
    expect(useIcons.getState().hoveringBare).toBe(true)
    expect(useIcons.getState().bareLook).toBe(false)
    expect(useIcons.getState().state!.config).toEqual(before)
    expect(useIcons.getState().canUndo).toBe(false)
    s.hoverBare(false)
    expect(useIcons.getState().hoveringBare).toBe(false)
  })

  test('override set + clear are single steps and round-trip through undo', () => {
    const s = useIcons.getState()
    s.setOverride('a', 'tint', '#3FB6A8')
    expect(useIcons.getState().items[0].overrideMode).toBe('tint')
    s.setOverride('a', 'keep')
    s.clearOverrides()
    expect(useIcons.getState().items[0].overrideMode).toBeNull()
    useIcons.getState().undo() // un-clear
    expect(useIcons.getState().items[0].overrideMode).toBe('keep')
    useIcons.getState().undo() // un-keep
    expect(useIcons.getState().items[0].overrideMode).toBe('tint')
  })
})

describe('effective tile config (override folding)', () => {
  test('follow renders the global config', () => {
    const eff = effectiveTileConfig(item('a'), config, DEFAULT_KIND_POLICY)
    expect(eff.showOriginal).toBe(false)
    expect(eff.config).toEqual(config)
  })

  test('keep renders the original artwork', () => {
    const eff = effectiveTileConfig(item('a', { overrideMode: 'keep' }), config, DEFAULT_KIND_POLICY)
    expect(eff.showOriginal).toBe(true)
  })

  test('tint renders Mono with the override tint', () => {
    const eff = effectiveTileConfig(item('a', { overrideMode: 'tint', overrideTint: '#3FB6A8' }), config, DEFAULT_KIND_POLICY)
    expect(eff.config.subject).toBe('Mono')
    expect(eff.config.tint).toBe('#3FB6A8')
  })

  test('un-styleable items always render their original', () => {
    const eff = effectiveTileConfig(item('b', { styleable: false }), config, DEFAULT_KIND_POLICY)
    expect(eff.showOriginal).toBe(true)
  })

  test('kindPolicy: an opted-out bucket renders original; a per-icon override wins', () => {
    const noFolders = { ...DEFAULT_KIND_POLICY, Folder: false }
    // a folder in an opted-out bucket → original
    expect(effectiveTileConfig(item('f', { kind: 'Folder' }), config, noFolders).showOriginal).toBe(true)
    // an app is unaffected
    expect(effectiveTileConfig(item('a', { kind: 'Shortcut' }), config, noFolders).showOriginal).toBe(false)
    // a per-icon 'tint' override on a folder still beats the type opt-out (cascade)
    const forced = effectiveTileConfig(item('f', { kind: 'Folder', overrideMode: 'tint', overrideTint: '#3FB6A8' }), config, noFolders)
    expect(forced.showOriginal).toBe(false)
    expect(forced.config.subject).toBe('Mono')
  })
})

// A1: System Default is a RESET, not a style. Selecting it flips the working
// design to bare (the mirror's show-original path) with NO host write; the CTA
// crossing is a restore, and it is undoable + cleared by any real look edit.
describe('System Default reset preset (A1)', () => {
  test('selecting it flips to bare and never fires a host apply', async () => {
    const calls: string[] = []
    __setBridgeForTests((async (method: unknown) => {
      calls.push(String(method))
      return null
    }) as unknown as Parameters<typeof __setBridgeForTests>[0])
    useIcons.getState().selectSystemDefault()
    expect(useIcons.getState().bareLook).toBe(true)
    // A reset can NEVER bake beautified icons — apply() is a guarded no-op that
    // reaches no bridge verb (spec A1: "no apply/host write").
    const applied = await useIcons.getState().apply()
    expect(applied).toBe(false)
    expect(calls).toEqual([])
    __setBridgeForTests(null)
  })

  test('config is untouched, so the highlight can only come from bareLook', () => {
    const before = { ...useIcons.getState().state!.config }
    useIcons.getState().selectSystemDefault()
    expect(useIcons.getState().state!.config).toEqual(before)
  })

  test('any real look edit leaves the bare look', () => {
    const s = useIcons.getState()
    s.selectSystemDefault()
    expect(useIcons.getState().bareLook).toBe(true)
    s.mutate({ shape: 'Circle' })
    expect(useIcons.getState().bareLook).toBe(false)
  })

  test('selecting a style preset clears the bare look', () => {
    useIcons.setState({
      state: { ...useIcons.getState().state!, presets: [{ id: 'p', config: { ...config, shape: 'Circle' }, typeOverrides: {} }] },
    })
    const s = useIcons.getState()
    s.selectSystemDefault()
    s.selectPreset('p')
    expect(useIcons.getState().bareLook).toBe(false)
    expect(useIcons.getState().state!.config.shape).toBe('Circle')
  })

  test('the bare look rides undo/redo', () => {
    const s = useIcons.getState()
    s.selectSystemDefault()
    expect(useIcons.getState().bareLook).toBe(true)
    useIcons.getState().undo()
    expect(useIcons.getState().bareLook).toBe(false)
    useIcons.getState().redo()
    expect(useIcons.getState().bareLook).toBe(true)
  })

  // A3: the bare-look intent persists to the client layer so a relaunch resumes
  // it (readBareLook is what scan() rehydrates from). Injected storage keeps the
  // test deterministic regardless of the runner's web-storage support.
  test('the bare-look intent persists to client storage and rehydrates', () => {
    const store: Record<string, string> = {}
    ;(globalThis as { localStorage?: unknown }).localStorage = {
      getItem: (k: string) => (k in store ? store[k] : null),
      setItem: (k: string, v: string) => { store[k] = v },
      removeItem: (k: string) => { delete store[k] },
    }
    try {
      persistBareLook(false)
      expect(readBareLook()).toBe(false)
      useIcons.getState().selectSystemDefault()
      expect(readBareLook()).toBe(true) // scan() would resume this on next launch
      useIcons.getState().mutate({ shape: 'Circle' })
      expect(readBareLook()).toBe(false) // a real edit clears the resumed intent
    } finally {
      delete (globalThis as { localStorage?: unknown }).localStorage
    }
  })
})

// Lens model (spec 06 §3.13, owner-disposed 2026-07-15): System Default is a
// read-only preview LENS over the draft. Toward-default actions (全部重置 /
// ↺跟随全局 / participation toggles) mutate the draft but never lift the lens;
// only value-asserting edits lift it. The draft survives the lens losslessly.
describe('System-Default lens (spec 06 §3.13)', () => {
  const folderPatch = { source: 'custom' as const, patch: { shape: 'Circle' as const } }

  beforeEach(() => {
    useIcons.setState({
      state: { ...useIcons.getState().state!, typeOverrides: { Folder: folderPatch } },
    })
  })

  test('全部重置 empties typeOverrides but NEVER lifts the lens (the reported bug)', () => {
    const s = useIcons.getState()
    s.selectSystemDefault()
    expect(useIcons.getState().bareLook).toBe(true)
    useIcons.getState().resetTypeOverrides()
    expect(useIcons.getState().state!.typeOverrides).toEqual({})
    expect(useIcons.getState().bareLook).toBe(true)
  })

  test('↺回到跟随全局 (clear branch) keeps the lens', () => {
    const s = useIcons.getState()
    s.selectSystemDefault()
    useIcons.getState().setTypeOverride('Folder', null)
    expect(useIcons.getState().state!.typeOverrides).toEqual({})
    expect(useIcons.getState().bareLook).toBe(true)
  })

  test('writing a custom type patch is value-asserting and lifts the lens', () => {
    const s = useIcons.getState()
    s.selectSystemDefault()
    useIcons.getState().setTypeOverride('File', { source: 'custom', patch: { shape: 'Tile' } })
    expect(useIcons.getState().bareLook).toBe(false)
    expect(useIcons.getState().state!.typeOverrides.File?.patch?.shape).toBe('Tile')
  })

  test('participation toggles are orthogonal to the lens', () => {
    const s = useIcons.getState()
    s.selectSystemDefault()
    useIcons.getState().setKindPolicy('Folder', false)
    expect(useIcons.getState().state!.kindPolicy.Folder).toBe(false)
    expect(useIcons.getState().bareLook).toBe(true)
  })

  test('the draft survives a lens round-trip losslessly', () => {
    const s = useIcons.getState()
    s.selectSystemDefault()
    expect(useIcons.getState().state!.typeOverrides).toEqual({ Folder: folderPatch })
    useIcons.getState().mutate({ shape: 'Samsung' }) // value-asserting → lens lifts
    expect(useIcons.getState().bareLook).toBe(false)
    expect(useIcons.getState().state!.typeOverrides).toEqual({ Folder: folderPatch })
    expect(useIcons.getState().state!.config.shape).toBe('Samsung')
  })
})

// A3: the resume status line maps the SAME applied/dirty signals every module
// reads to an honest phrase — an un-applied draft never silently reads "applied".
describe('resume status line (A3)', () => {
  test('un-applied draft (fresh OR resumed) reads ready, never applied', () => {
    expect(resumeStatusKey(false, false, false)).toBe('Hero_ReadyStatus')
    expect(resumeStatusKey(false, true, false)).toBe('Hero_ReadyStatus')
  })
  test('a draft that matches the live desktop reads resumed', () => {
    expect(resumeStatusKey(true, false, false)).toBe('Hero_ResumedStatus')
  })
  test('an applied desktop with a newer draft reads unapplied, never applied', () => {
    expect(resumeStatusKey(true, true, false)).toBe('Hero_UnappliedStatus')
  })
  test('the bare look has its own two-state line', () => {
    expect(resumeStatusKey(false, false, true)).toBe('Icons_BareStatus')
    expect(resumeStatusKey(true, true, true)).toBe('Icons_BareDirtyStatus')
  })
})
