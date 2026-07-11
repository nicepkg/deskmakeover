import { beforeEach, describe, expect, test } from 'bun:test'
import { makeZone, singleScreenSeed, useWallpaper } from '../src/stores/wallpaper'
import { createFromDrag } from '../src/lib/zone-math'
import type { LookDto, MonitorLookDto, WallpaperGridInfoDto, WallpaperStateDto, ZoneDto } from '../src/bridge/types'

// Session undo/redo over `look` snapshots (spec 04 v2.0, ADR-0014). Seeds the
// store directly so the tests exercise the history logic without the host or
// the compositor (registry returns null in tests — commit() tolerates that).
//
// SINGLE-MONITOR PARITY: this whole suite seeds ONE screen and its assertions are
// unchanged from the pre-multi-monitor store. A green run proves screens.length===1
// behaves exactly as before (spec 04 §B2 acceptance / global regression guard).

const GRID: WallpaperGridInfoDto = {
  screenWidth: 1920,
  screenHeight: 1080,
  taskbarHeight: 48,
  iconPx: 48,
  cellWidth: 92,
  cellHeight: 92,
  inset: 14,
  columns: 20,
  rows: 11,
}

const MONITOR_ID = '\\\\?\\DISPLAY#TEST#0'

function zone(cellX: number, title = 'z'): ZoneDto {
  return makeZone({ cellX, cellY: 0.5, cellsWide: 4, cellsTall: 4, title })
}

function screenDto(look: LookDto): MonitorLookDto {
  return {
    monitorId: MONITOR_ID,
    name: 'Test',
    bounds: { x: 0, y: 0, w: 1920, h: 1080 },
    orientation: 'landscape',
    look,
    source: null,
    grid: GRID,
    slideshowActive: false,
    hasReadableSource: true,
  }
}

function seed(zones: ZoneDto[] = []): void {
  const look: LookDto = {
    zones,
    clarity: { level: 'Off', gradient: 'Linear', angleDeg: 180, dimOverride: null, tone: 'Dark', customScrim: null },
  }
  const state: WallpaperStateDto = {
    look,
    grid: GRID,
    originalUrl: null,
    hasBackup: false,
    working: false,
    dirty: false,
    pale: false,
    fingerprintMismatch: false,
    wallTint: '#7A6E62',
    screens: [screenDto(look)],
    activeScreenId: MONITOR_ID,
    position: 'Fill',
    spanActive: false,
  }
  const { screens, activeScreenId } = singleScreenSeed(MONITOR_ID, look)
  useWallpaper.setState({
    loaded: true,
    state,
    screens,
    activeScreenId,
    look,
    selected: null,
    sourceName: null,
    sourceUrl: null,
    past: [],
    future: [],
    canUndo: false,
    canRedo: false,
  })
  useWallpaper.getState().endInteraction() // clear any leaked gesture-coalescing flag
}

beforeEach(() => seed())

describe('makeZone defaults (Adaptive Frost)', () => {
  test('stable unique ids + spec defaults', () => {
    const a = zone(1)
    const b = zone(1)
    expect(a.id).not.toBe(b.id)
    expect(a.tone).toBe('Auto')
    expect(a.material).toBe('Frost')
    expect(a.titleStyle).toBe('Chip')
    expect(a.shadow).toBe(false)
    expect(a.cornerRadius).toBe(20)
    expect(a.titleSize).toBe('M')
    expect(a.accent).toBeNull()
    expect(a.emoji).toBeNull()
  })
})

describe('history snapshots', () => {
  test('create / restyle / delete each push exactly one snapshot', () => {
    const s = useWallpaper.getState()
    const z = zone(1)
    s.addZone(z)
    expect(useWallpaper.getState().past).toHaveLength(1)
    s.mutateZone(z.id, (v) => ({ ...v, tone: 'Dark' }))
    expect(useWallpaper.getState().past).toHaveLength(2)
    s.removeZone(z.id)
    expect(useWallpaper.getState().past).toHaveLength(3)
    expect(useWallpaper.getState().canUndo).toBe(true)
  })

  test('mutateLook pushes a snapshot (clarity)', () => {
    seed([zone(1), zone(6)])
    useWallpaper.getState().mutateLook((l) => ({ ...l, clarity: { ...l.clarity, level: 'Soft' } }))
    expect(useWallpaper.getState().past).toHaveLength(1)
    expect(useWallpaper.getState().look?.clarity.level).toBe('Soft')
  })
})

describe('undo / redo', () => {
  test('undo restores the prior look; redo re-applies it', () => {
    const s = useWallpaper.getState()
    s.addZone(zone(1))
    expect(useWallpaper.getState().look?.zones).toHaveLength(1)

    s.undo()
    expect(useWallpaper.getState().look?.zones).toHaveLength(0)
    expect(useWallpaper.getState().canUndo).toBe(false)
    expect(useWallpaper.getState().canRedo).toBe(true)

    s.redo()
    expect(useWallpaper.getState().look?.zones).toHaveLength(1)
    expect(useWallpaper.getState().canRedo).toBe(false)
  })

  test('Ctrl+Z undoes a delete (the zone comes back)', () => {
    const keep = zone(1, 'keep')
    const gone = zone(6, 'gone')
    seed([keep, gone])
    const s = useWallpaper.getState()
    s.removeZone(gone.id)
    expect(useWallpaper.getState().look?.zones).toHaveLength(1)
    s.undo()
    const zones = useWallpaper.getState().look?.zones ?? []
    expect(zones).toHaveLength(2)
    expect(zones[1]?.title).toBe('gone')
  })

  test('a new mutation clears the redo future', () => {
    const s = useWallpaper.getState()
    s.addZone(zone(1))
    s.undo()
    expect(useWallpaper.getState().canRedo).toBe(true)
    s.addZone(zone(6))
    expect(useWallpaper.getState().future).toHaveLength(0)
    expect(useWallpaper.getState().canRedo).toBe(false)
  })

  test('undo with an empty stack is a no-op', () => {
    const s = useWallpaper.getState()
    expect(() => s.undo()).not.toThrow()
    expect(useWallpaper.getState().look?.zones).toHaveLength(0)
  })

  test('undo clamps a now-invalid selection to null', () => {
    const s = useWallpaper.getState()
    const z = zone(1)
    s.addZone(z) // selects the new id
    expect(useWallpaper.getState().selected).toBe(z.id)
    s.undo() // zones empty again → the id no longer exists
    expect(useWallpaper.getState().selected).toBeNull()
  })
})

describe('gesture coalescing', () => {
  test('a whole move gesture is ONE undo step', () => {
    const z = zone(1)
    seed([z])
    const s = useWallpaper.getState()
    s.beginInteraction()
    s.mutateZone(z.id, (v) => ({ ...v, cellX: 2 }))
    s.mutateZone(z.id, (v) => ({ ...v, cellX: 3 }))
    s.mutateZone(z.id, (v) => ({ ...v, cellX: 4 }))
    s.endInteraction()
    expect(useWallpaper.getState().past).toHaveLength(1)
    s.undo()
    expect(useWallpaper.getState().look?.zones[0]?.cellX).toBe(1)
  })
})

describe('endInteraction safety (Fix B)', () => {
  test('endInteraction is safe to call with no open gesture (idempotent)', () => {
    const z = zone(1)
    seed([z])
    const s = useWallpaper.getState()
    expect(() => {
      s.endInteraction()
      s.endInteraction()
    }).not.toThrow()
    s.mutateZone(z.id, (v) => ({ ...v, cellX: 3 }))
    expect(useWallpaper.getState().past).toHaveLength(1)
  })

  test('endInteraction from an abnormal end truly closes coalescing (no leak into next snapshot)', () => {
    const z = zone(1)
    seed([z])
    const s = useWallpaper.getState()
    s.beginInteraction()
    s.mutateZone(z.id, (v) => ({ ...v, cellX: 2 }))
    s.endInteraction()
    expect(useWallpaper.getState().past).toHaveLength(1)
    s.mutateZone(z.id, (v) => ({ ...v, cellX: 5 }))
    expect(useWallpaper.getState().past).toHaveLength(2)
  })
})

describe('replaceZones (presets)', () => {
  test('replaces all zones, snapshots, and is undoable', () => {
    seed([zone(1, 'old')])
    const s = useWallpaper.getState()
    s.replaceZones([zone(2, 'a'), zone(8, 'b'), zone(14, 'c')])
    expect(useWallpaper.getState().look?.zones).toHaveLength(3)
    expect(useWallpaper.getState().selected).toBeNull()
    expect(useWallpaper.getState().past).toHaveLength(1)
    s.undo()
    const zones = useWallpaper.getState().look?.zones ?? []
    expect(zones).toHaveLength(1)
    expect(zones[0]?.title).toBe('old')
  })
})

describe('duplicateZone (Alt-drag)', () => {
  test('clones the zone with a fresh id and selects the copy', () => {
    const z = zone(1, 'src')
    seed([z])
    const s = useWallpaper.getState()
    const copyId = s.duplicateZone(z.id, { cellX: 8, cellY: 2 })
    expect(copyId).toBeTruthy()
    const zones = useWallpaper.getState().look?.zones ?? []
    expect(zones).toHaveLength(2)
    expect(zones[1]?.id).toBe(copyId!)
    // Spec 04 §3: the clone carries a copy suffix so the pair stays tellable.
    expect(zones[1]?.title).toBe('src copy')
    expect(zones[1]?.cellX).toBe(8)
    expect(useWallpaper.getState().selected).toBe(copyId)
    expect(useWallpaper.getState().past).toHaveLength(1)
  })
})

describe('applyToAllZones (the ONLY mass path — spec 04 §2.4)', () => {
  test('snapshots once, then patches EVERY zone, and is undoable', () => {
    seed([zone(1), zone(6), zone(11)])
    const s = useWallpaper.getState()
    s.applyToAllZones({ tone: 'Dark', material: 'Solid', fillOpacity: 0.5, cornerRadius: 8 })
    const zones = useWallpaper.getState().look?.zones ?? []
    expect(zones).toHaveLength(3)
    for (const z of zones) {
      expect(z.tone).toBe('Dark')
      expect(z.material).toBe('Solid')
      expect(z.fillOpacity).toBe(0.5)
      expect(z.cornerRadius).toBe(8)
    }
    expect(useWallpaper.getState().past).toHaveLength(1)
    s.undo()
    expect(useWallpaper.getState().look?.zones.every((z) => z.tone === 'Auto')).toBe(true)
  })

  test('is a no-op with no zones — never a phantom snapshot', () => {
    seed([])
    useWallpaper.getState().applyToAllZones({ tone: 'Dark' })
    expect(useWallpaper.getState().past).toHaveLength(0)
    expect(useWallpaper.getState().look?.zones).toHaveLength(0)
  })
})

describe('selection honesty', () => {
  test('removeZone clears selection deliberately', () => {
    const a = zone(1)
    const b = zone(6)
    seed([a, b])
    const s = useWallpaper.getState()
    s.select(b.id)
    s.removeZone(b.id)
    expect(useWallpaper.getState().selected).toBeNull()
  })
})

describe('live-snap contract', () => {
  test('the rubber-band span is the SNAPPED rect, not the raw pointer rect', () => {
    const span = createFromDrag({ cx: 1.2, cy: 0.9 }, { cx: 4.7, cy: 3.4 }, GRID.columns, GRID.rows)
    expect(span.cellX).toBe(1)
    expect(span.cellY).toBe(0.5)
    expect(span.cellsWide % 0.5).toBe(0)
    expect(span.cellsTall % 0.5).toBe(0)
    expect(span.cellsWide).toBeGreaterThanOrEqual(2)
  })
})
