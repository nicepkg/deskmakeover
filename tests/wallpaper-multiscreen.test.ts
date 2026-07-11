import { beforeEach, describe, expect, test } from 'bun:test'
import { makeZone, useWallpaper } from '../src/stores/wallpaper'
import type { ScreenLook } from '../src/stores/wallpaper'
import type { LookDto, MonitorBounds, MonitorLookDto, WallpaperStateDto, ZoneDto } from '../src/bridge/types'

// Per-screen independence (spec 04 §B2): editing / undoing one monitor never
// touches another, and switching screens preserves each screen's draft + undo
// stack. Seeds a 2-monitor store directly (DOM-free; the compositor registry
// returns null in tests and commit tolerates it).

const A = '\\\\?\\DISPLAY#MOCK#0'
const B = '\\\\?\\DISPLAY#MOCK#1'
const LAND: MonitorBounds = { x: 0, y: 0, w: 1920, h: 1080 }
const PORT: MonitorBounds = { x: 1920, y: 0, w: 1080, h: 1920 }

function look(zones: ZoneDto[] = []): LookDto {
  return { zones, clarity: { level: 'Off', gradient: 'Linear', angleDeg: 0, dimOverride: null, tone: 'Dark', customScrim: null } }
}

function zone(cellX: number, title = 'z'): ZoneDto {
  return makeZone({ cellX, cellY: 0.5, cellsWide: 4, cellsTall: 4, title })
}

function screenLook(l: LookDto): ScreenLook {
  return { look: l, source: null, sourceName: null, sourceUrl: null, selected: null, past: [], future: [] }
}

function monitorDto(id: string, bounds: MonitorBounds, l: LookDto): MonitorLookDto {
  return {
    monitorId: id,
    name: id,
    bounds,
    orientation: bounds.h > bounds.w ? 'portrait' : 'landscape',
    look: l,
    source: null,
    grid: { screenWidth: bounds.w, screenHeight: bounds.h, taskbarHeight: 48, iconPx: 48, cellWidth: 92, cellHeight: 92, inset: 14, columns: 20, rows: 11 },
    slideshowActive: false,
    hasReadableSource: true,
  }
}

/** Seed a 2-monitor store, active = A. */
function seedTwo(): void {
  const la = look()
  const lb = look()
  const state: WallpaperStateDto = {
    look: la,
    grid: monitorDto(A, LAND, la).grid,
    originalUrl: null,
    hasBackup: false,
    working: false,
    dirty: false,
    pale: false,
    fingerprintMismatch: false,
    wallTint: '#7A6E62',
    screens: [monitorDto(A, LAND, la), monitorDto(B, PORT, lb)],
    activeScreenId: A,
    position: 'Fill',
    spanActive: false,
  }
  useWallpaper.setState({
    loaded: true,
    state,
    screens: { [A]: screenLook(la), [B]: screenLook(lb) },
    activeScreenId: A,
    look: la,
    selected: null,
    sourceName: null,
    sourceUrl: null,
    past: [],
    future: [],
    canUndo: false,
    canRedo: false,
  })
  useWallpaper.getState().endInteraction()
}

beforeEach(() => seedTwo())

describe('per-screen edit isolation', () => {
  test('adding a zone to A leaves B untouched', () => {
    const s = useWallpaper.getState()
    s.addZone(zone(1, 'on-A'))
    const st = useWallpaper.getState()
    expect(st.screens[A].look.zones).toHaveLength(1)
    expect(st.screens[B].look.zones).toHaveLength(0)
    // The active-screen mirror follows A.
    expect(st.look?.zones).toHaveLength(1)
  })

  test('editing continues on the screen you switched to, not the old one', () => {
    const s = useWallpaper.getState()
    s.addZone(zone(1, 'on-A'))
    s.selectScreen(B)
    expect(useWallpaper.getState().look?.zones).toHaveLength(0) // B is empty
    useWallpaper.getState().addZone(zone(2, 'on-B'))
    const st = useWallpaper.getState()
    expect(st.screens[A].look.zones).toHaveLength(1)
    expect(st.screens[B].look.zones).toHaveLength(1)
    expect(st.screens[A].look.zones[0]?.title).toBe('on-A')
    expect(st.screens[B].look.zones[0]?.title).toBe('on-B')
  })
})

describe('per-screen undo', () => {
  test('undo on A does not affect B', () => {
    const s = useWallpaper.getState()
    s.addZone(zone(1, 'on-A')) // A: 1 snapshot
    s.selectScreen(B)
    useWallpaper.getState().addZone(zone(2, 'on-B')) // B: 1 snapshot
    // Back to A and undo — only A reverts.
    useWallpaper.getState().selectScreen(A)
    useWallpaper.getState().undo()
    const st = useWallpaper.getState()
    expect(st.screens[A].look.zones).toHaveLength(0)
    expect(st.screens[B].look.zones).toHaveLength(1) // B's edit survives A's undo
  })

  test('each screen keeps its own undo/redo stack', () => {
    const s = useWallpaper.getState()
    s.addZone(zone(1)) // A past = 1
    expect(useWallpaper.getState().past).toHaveLength(1)
    s.selectScreen(B)
    // B has an independent, empty history.
    expect(useWallpaper.getState().past).toHaveLength(0)
    expect(useWallpaper.getState().canUndo).toBe(false)
    useWallpaper.getState().addZone(zone(3))
    useWallpaper.getState().addZone(zone(9))
    expect(useWallpaper.getState().past).toHaveLength(2) // B past = 2
    // A's history is unchanged.
    useWallpaper.getState().selectScreen(A)
    expect(useWallpaper.getState().past).toHaveLength(1)
  })
})

describe('selectScreen preserves in-progress edits + undo stack', () => {
  test('switching away and back keeps the draft AND the redo/undo history', () => {
    const s = useWallpaper.getState()
    s.addZone(zone(1, 'draft'))
    s.undo() // A now: 0 zones, redo available
    expect(useWallpaper.getState().canRedo).toBe(true)
    s.selectScreen(B)
    s.selectScreen(A)
    const st = useWallpaper.getState()
    expect(st.look?.zones).toHaveLength(0)
    expect(st.canRedo).toBe(true) // redo survived the round-trip
    st.redo()
    const back = useWallpaper.getState()
    expect(back.look?.zones).toHaveLength(1)
    expect(back.look?.zones[0]?.title).toBe('draft')
  })

  test('selecting the active screen or an unknown id is a no-op', () => {
    const s = useWallpaper.getState()
    s.addZone(zone(1))
    s.selectScreen(A) // already active
    s.selectScreen('\\\\?\\DISPLAY#GONE')
    expect(useWallpaper.getState().activeScreenId).toBe(A)
    expect(useWallpaper.getState().look?.zones).toHaveLength(1)
  })

  test('the mirror follows the active screen grid (portrait B is taller)', () => {
    const s = useWallpaper.getState()
    s.selectScreen(B)
    const st = useWallpaper.getState()
    expect(st.state?.activeScreenId).toBe(B)
    expect(st.state?.grid.screenHeight).toBeGreaterThan(st.state?.grid.screenWidth ?? 0)
  })
})

describe('dirty reflects any screen', () => {
  test('editing one screen marks the whole state dirty', () => {
    const s = useWallpaper.getState()
    expect(useWallpaper.getState().state?.dirty).toBe(false)
    s.addZone(zone(1))
    expect(useWallpaper.getState().state?.dirty).toBe(true)
  })
})
