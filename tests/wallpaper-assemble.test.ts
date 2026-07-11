import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { assembleWallpaperState, loadPersistedLooks, lookDirty, savePersistedLook } from '../src/lib/wallpaper-assemble'
import { type PersistedLook, type ScreenLook, emptyLook } from '../src/lib/monitor-reconcile'
import type { LookDto, MonitorBounds, ScreenInfoDto, WallpaperPosition, WallpaperScreensDto } from '../src/bridge/types'

// Frontend wallpaper-state assembly + localStorage persistence (schema 6, D1). The
// mock's old buildState() + the setLook bridge verb both moved here; this suite is the
// unit coverage that used to ride on the (now deleted) fat-DTO decode + the mock's
// reconcile-on-getState path. DOM-free — a MemoryStorage polyfill stands in for the
// browser's localStorage (bun has none), matching the guarded persistence idiom.

const LAND: MonitorBounds = { x: 0, y: 0, w: 1920, h: 1080 }
const PORT: MonitorBounds = { x: 1920, y: 0, w: 1080, h: 1920 }
const A = '\\\\?\\DISPLAY#MOCK#0'
const B = '\\\\?\\DISPLAY#MOCK#1'

function look(dirty = false): LookDto {
  const l = emptyLook()
  if (dirty) l.clarity.level = 'Soft' // a distinguishable non-default look
  return l
}

function info(id: string, bounds: MonitorBounds): ScreenInfoDto {
  return {
    monitorId: id,
    name: id,
    bounds,
    orientation: bounds.h > bounds.w ? 'portrait' : 'landscape',
    source: { url: `scene://${id}`, width: 3840, height: 2400 },
    slideshowActive: false,
    hasReadableSource: true,
  }
}

function screensDto(screens: ScreenInfoDto[], position: WallpaperPosition = 'Fill', spanActive = false, hasBackup = false): WallpaperScreensDto {
  return { screens, position, spanActive, hasBackup }
}

function liveScreen(l: LookDto): ScreenLook {
  return { look: l, source: null, sourceName: null, sourceUrl: null, selected: null, past: [], future: [] }
}

/** Minimal in-memory Web Storage — bun has no localStorage. */
class MemoryStorage {
  private m = new Map<string, string>()
  get length(): number {
    return this.m.size
  }
  key(i: number): string | null {
    return [...this.m.keys()][i] ?? null
  }
  getItem(k: string): string | null {
    return this.m.has(k) ? (this.m.get(k) as string) : null
  }
  setItem(k: string, v: string): void {
    this.m.set(k, String(v))
  }
  removeItem(k: string): void {
    this.m.delete(k)
  }
  clear(): void {
    this.m.clear()
  }
}

describe('assembleWallpaperState — reconcile on load', () => {
  test('a persisted look is restored onto its screen; a new screen defaults', () => {
    const persisted = new Map<string, PersistedLook>([[A, { look: look(true), bounds: LAND }]])
    const state = assembleWallpaperState(screensDto([info(A, LAND), info(B, PORT)]), persisted, {}, { prevActiveId: null })
    const a = state.screens.find((s) => s.monitorId === A)!
    const b = state.screens.find((s) => s.monitorId === B)!
    expect(a.look.clarity.level).toBe('Soft') // restored from persistence
    expect(b.look.clarity.level).toBe('Off') // genuinely new monitor → default
    expect(state.dirty).toBe(true) // A carries a non-empty draft
    expect(state.activeScreenId).toBe(A) // first screen active
    expect(state.look).toEqual(a.look) // top-level mirrors the active screen
    expect(state.originalUrl).toBe(a.source?.url) // originalUrl mirrors the active screen's source
  })

  test('derives per-screen grid from bounds (portrait taller/narrower)', () => {
    const state = assembleWallpaperState(screensDto([info(A, LAND), info(B, PORT)]), new Map(), {}, { prevActiveId: null })
    const a = state.screens.find((s) => s.monitorId === A)!
    const b = state.screens.find((s) => s.monitorId === B)!
    expect(a.grid.columns).toBeGreaterThan(a.grid.rows)
    expect(b.grid.rows).toBeGreaterThan(b.grid.columns)
  })

  test('threads hasBackup + global position/span from the DTO', () => {
    const state = assembleWallpaperState(screensDto([info(A, LAND), info(B, PORT)], 'Span', true, true), new Map(), {}, { prevActiveId: null })
    expect(state.hasBackup).toBe(true)
    expect(state.position).toBe('Span')
    expect(state.spanActive).toBe(true)
  })

  test('a bounds-fingerprint fallback restores a look across a new device path', () => {
    // Same bounds, new device path (replug on a different port) → bounds match.
    const persisted = new Map<string, PersistedLook>([['OLD-PATH', { look: look(true), bounds: LAND }]])
    const state = assembleWallpaperState(screensDto([info(A, LAND)]), persisted, {}, { prevActiveId: null })
    expect(state.screens[0].look.clarity.level).toBe('Soft')
  })

  test('an empty payload assembles a clean, screen-less state (never throws)', () => {
    const state = assembleWallpaperState(screensDto([]), new Map(), {}, { prevActiveId: null })
    expect(state.screens).toHaveLength(0)
    expect(state.dirty).toBe(false)
  })
})

describe('assembleWallpaperState — re-fetch flow (apply / restore)', () => {
  test('a live draft beats the stale persisted look (mid-edit, ahead of the debounce)', () => {
    // localStorage still holds the OLD (empty) look for A; the live store holds a dirtied draft.
    const persisted = new Map<string, PersistedLook>([[A, { look: look(false), bounds: LAND }]])
    const live = { [A]: liveScreen(look(true)) }
    const state = assembleWallpaperState(screensDto([info(A, LAND)], 'Fill', false, true), persisted, live, { prevActiveId: A })
    expect(state.screens[0].look.clarity.level).toBe('Soft') // live draft, not the stale persisted 'Off'
    expect(state.hasBackup).toBe(true)
    expect(state.dirty).toBe(true)
  })

  test('keeps the user on their active screen across a re-fetch', () => {
    const live = { [A]: liveScreen(look()), [B]: liveScreen(look()) }
    const state = assembleWallpaperState(screensDto([info(A, LAND), info(B, PORT)]), new Map(), live, { prevActiveId: B })
    expect(state.activeScreenId).toBe(B)
    expect(state.grid.screenHeight).toBeGreaterThan(state.grid.screenWidth) // active mirror = portrait B
  })

  test('a restore-shaped re-fetch (hasBackup false) re-derives dirty from the surviving drafts', () => {
    const live = { [A]: liveScreen(look(true)) }
    const state = assembleWallpaperState(screensDto([info(A, LAND)]), new Map(), live, { prevActiveId: A })
    expect(state.hasBackup).toBe(false) // snapshot reverted
    expect(state.dirty).toBe(true) // the draft survives the restore
  })
})

describe('persisted-look localStorage round-trip', () => {
  // bun has no localStorage; polyfill the global with MemoryStorage for this block.
  const globalRef = globalThis as { localStorage?: unknown }
  const realLS = globalRef.localStorage
  let store: MemoryStorage
  beforeEach(() => {
    store = new MemoryStorage()
    globalRef.localStorage = store
  })
  afterEach(() => {
    globalRef.localStorage = realLS
  })

  test('save then load restores the exact look + bounds, keyed by monitorId', () => {
    savePersistedLook(A, look(true), LAND)
    savePersistedLook(B, look(false), PORT)
    const map = loadPersistedLooks()
    expect(map.size).toBe(2)
    expect(map.get(A)!.look.clarity.level).toBe('Soft')
    expect(map.get(A)!.bounds).toEqual(LAND)
    expect(map.get(B)!.look.clarity.level).toBe('Off')
    expect(map.get(B)!.bounds).toEqual(PORT)
  })

  test('ignores unrelated localStorage keys (only wallpaper.look.v2:: entries)', () => {
    store.setItem('dm.icons.bareLook', '1')
    savePersistedLook(A, look(true), LAND)
    const map = loadPersistedLooks()
    expect(map.size).toBe(1)
    expect([...map.keys()]).toEqual([A])
  })

  test('a corrupt entry is skipped, the rest still load', () => {
    savePersistedLook(A, look(true), LAND)
    store.setItem('wallpaper.look.v2::corrupt', '{not json')
    const map = loadPersistedLooks()
    expect(map.get(A)!.look.clarity.level).toBe('Soft')
    expect(map.has('corrupt')).toBe(false)
  })

  test('the round-tripped map drives an assemble end-to-end (persist → load → restore)', () => {
    savePersistedLook(A, look(true), LAND)
    const state = assembleWallpaperState(screensDto([info(A, LAND)]), loadPersistedLooks(), {}, { prevActiveId: null })
    expect(state.screens[0].look.clarity.level).toBe('Soft')
    expect(state.dirty).toBe(true)
  })
})

describe('lookDirty', () => {
  test('empty look is clean; a zone or clarity makes it dirty', () => {
    expect(lookDirty(look(false))).toBe(false)
    expect(lookDirty(look(true))).toBe(true)
  })
})
