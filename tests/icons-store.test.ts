import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import type { ConfigDto, IconItemDto, IconsStateDto } from '../src/bridge/types'
import { effectiveTileConfig, resetIconsHistoryForTests, useIcons } from '../src/stores/icons'
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
