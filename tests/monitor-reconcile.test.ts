import { describe, expect, test } from 'bun:test'
import {
  type PersistedLook,
  type PhysicalMonitor,
  type ScreenLook,
  emptyLook,
  mergeScreenMap,
  orientationOf,
  pickActiveScreenId,
  reconcileMonitorLook,
  reconcileScreens,
  screenLookFromDto,
} from '../src/lib/monitor-reconcile'
import type { LookDto, MonitorBounds, MonitorLookDto } from '../src/bridge/types'

// Pure multi-monitor reconcile rules (spec 04 §B3). No DOM — the identity +
// fallback + hot-plug logic is verified here in isolation.

const LAND: MonitorBounds = { x: 0, y: 0, w: 1920, h: 1080 }
const PORT: MonitorBounds = { x: 1920, y: 0, w: 1080, h: 1920 }

function look(n = 0): LookDto {
  const l = emptyLook()
  if (n > 0) l.clarity.level = 'Soft' // a distinguishable non-default look
  return l
}

function monitor(id: string, bounds: MonitorBounds): PhysicalMonitor {
  return { monitorId: id, name: id, bounds, source: null, slideshowActive: false, hasReadableSource: true }
}

describe('orientationOf', () => {
  test('portrait when height > width, else landscape', () => {
    expect(orientationOf(PORT)).toBe('portrait')
    expect(orientationOf(LAND)).toBe('landscape')
    expect(orientationOf({ x: 0, y: 0, w: 2560, h: 1440 })).toBe('landscape')
  })
})

describe('emptyLook', () => {
  test('no zones, clarity off', () => {
    const l = emptyLook()
    expect(l.zones).toHaveLength(0)
    expect(l.clarity.level).toBe('Off')
  })
})

describe('reconcileMonitorLook', () => {
  test('device-path match restores that look', () => {
    const persisted = new Map<string, PersistedLook>([['A', { look: look(1), bounds: LAND }]])
    const r = reconcileMonitorLook(monitor('A', LAND), persisted, new Set(['A']))
    expect(r.matchedBy).toBe('path')
    expect(r.look.clarity.level).toBe('Soft')
  })

  test('bounds fingerprint fallback when the path is unknown', () => {
    // A monitor came back on a new port (new device path) but the SAME bounds.
    const persisted = new Map<string, PersistedLook>([['OLD-PATH', { look: look(1), bounds: LAND }]])
    const r = reconcileMonitorLook(monitor('NEW-PATH', LAND), persisted, new Set())
    expect(r.matchedBy).toBe('bounds')
    expect(r.look.clarity.level).toBe('Soft')
  })

  test('a bounds match already claimed by a present path is NOT stolen', () => {
    const persisted = new Map<string, PersistedLook>([['A', { look: look(1), bounds: LAND }]])
    // 'A' is present (claimed); a different new monitor with the same bounds must
    // fall through to default rather than adopt A's dormant look.
    const r = reconcileMonitorLook(monitor('B', LAND), persisted, new Set(['A']))
    expect(r.matchedBy).toBe('default')
    expect(r.look.clarity.level).toBe('Off')
  })

  test('default empty look when nothing matches (a genuinely new monitor)', () => {
    const r = reconcileMonitorLook(monitor('Z', PORT), new Map(), new Set())
    expect(r.matchedBy).toBe('default')
    expect(r.look.zones).toHaveLength(0)
  })
})

describe('reconcileScreens (present → restore / detached → dormant / new → default)', () => {
  test('present monitor restores its persisted look by path; new one defaults', () => {
    const persisted = new Map<string, PersistedLook>([['A', { look: look(1), bounds: LAND }]])
    const screens = reconcileScreens([monitor('A', LAND), monitor('B', PORT)], persisted)
    expect(screens.find((s) => s.monitorId === 'A')!.look.clarity.level).toBe('Soft')
    expect(screens.find((s) => s.monitorId === 'B')!.look.clarity.level).toBe('Off')
    expect(screens.find((s) => s.monitorId === 'B')!.orientation).toBe('portrait')
  })

  test('detached monitor stays dormant in persistence and resumes on replug', () => {
    const persisted = new Map<string, PersistedLook>()
    // 1) A + B present, both edited (persisted).
    persisted.set('A', { look: look(1), bounds: LAND })
    persisted.set('B', { look: look(1), bounds: PORT })
    // 2) B unplugged — only A present. B's entry is NOT pruned by reconcile.
    const only = reconcileScreens([monitor('A', LAND)], persisted)
    expect(only).toHaveLength(1)
    expect(persisted.has('B')).toBe(true) // dormant, retained
    // 3) B replugged (same path) — its look resumes.
    const back = reconcileScreens([monitor('A', LAND), monitor('B', PORT)], persisted)
    expect(back.find((s) => s.monitorId === 'B')!.look.clarity.level).toBe('Soft')
  })

  test('per-screen grid follows the monitor bounds (portrait is taller/narrower)', () => {
    const [land, port] = reconcileScreens([monitor('A', LAND), monitor('B', PORT)], new Map())
    expect(land.grid.columns).toBeGreaterThan(land.grid.rows)
    expect(port.grid.rows).toBeGreaterThan(port.grid.columns)
  })
})

describe('mergeScreenMap (hot-plug map merge)', () => {
  const dto = (id: string, bounds: MonitorBounds): MonitorLookDto => ({
    monitorId: id,
    name: id,
    bounds,
    orientation: orientationOf(bounds),
    look: emptyLook(),
    source: null,
    grid: { screenWidth: bounds.w, screenHeight: bounds.h, taskbarHeight: 48, iconPx: 48, cellWidth: 92, cellHeight: 92, inset: 14, columns: 20, rows: 11 },
    slideshowActive: false,
    hasReadableSource: true,
  })

  test('keeps in-progress ScreenLook for still-present monitors, seeds new, drops absent', () => {
    const edited: ScreenLook = { look: look(1), source: null, sourceName: 'x.png', sourceUrl: 'blob:x', selected: 'zid', past: [emptyLook()], future: [] }
    const prev: Record<string, ScreenLook> = { A: edited, GONE: screenLookFromDto(dto('GONE', LAND)) }
    const next = mergeScreenMap(prev, [dto('A', LAND), dto('NEW', PORT)])
    expect(next.A).toBe(edited) // same reference — draft + undo preserved
    expect(next.NEW).toBeDefined()
    expect(next.NEW.past).toHaveLength(0) // seeded fresh
    expect(next.GONE).toBeUndefined() // detached, dropped from active map
  })
})

describe('pickActiveScreenId', () => {
  const dto = (id: string): MonitorLookDto => ({
    monitorId: id, name: id, bounds: LAND, orientation: 'landscape', look: emptyLook(), source: null,
    grid: { screenWidth: 1920, screenHeight: 1080, taskbarHeight: 48, iconPx: 48, cellWidth: 92, cellHeight: 92, inset: 14, columns: 20, rows: 11 },
    slideshowActive: false, hasReadableSource: true,
  })

  test('keeps the current screen when still present', () => {
    expect(pickActiveScreenId('B', [dto('A'), dto('B')], 'A')).toBe('B')
  })
  test('falls back to the host activeScreenId when the current is gone', () => {
    expect(pickActiveScreenId('GONE', [dto('A'), dto('B')], 'B')).toBe('B')
  })
  test('falls back to the first screen when neither matches', () => {
    expect(pickActiveScreenId('GONE', [dto('A'), dto('B')], 'ALSO-GONE')).toBe('A')
  })
})
