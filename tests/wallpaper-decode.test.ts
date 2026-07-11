import { describe, expect, test } from 'bun:test'
import { WallpaperDecodeError, decodeWallpaperOp, decodeWallpaperState } from '../src/bridge/wallpaper-decode'
import type { MonitorLookDto, WallpaperGridInfoDto, WallpaperStateDto } from '../src/bridge/types'

// Strict client decoder for the WIDENED multi-monitor DTO (spec 04 §B1). Guards
// the payload-widening trap: the decoder must ROUND-TRIP the new screens[] shape
// (never a silent empty collapse) and throw LOUDLY on a genuinely malformed field.

const GRID: WallpaperGridInfoDto = {
  screenWidth: 1920, screenHeight: 1080, taskbarHeight: 48, iconPx: 48,
  cellWidth: 92, cellHeight: 92, inset: 14, columns: 20, rows: 11,
}

function monitor(id: string, portrait = false): MonitorLookDto {
  return {
    monitorId: id,
    name: id,
    bounds: portrait ? { x: 1920, y: 0, w: 1080, h: 1920 } : { x: 0, y: 0, w: 1920, h: 1080 },
    orientation: portrait ? 'portrait' : 'landscape',
    look: { zones: [], clarity: { level: 'Off', gradient: 'Linear', angleDeg: 0, dimOverride: null, tone: 'Dark', customScrim: null } },
    source: { url: 'mock://wall', width: 1920, height: 1080 },
    grid: GRID,
    slideshowActive: false,
    hasReadableSource: true,
  }
}

function validState(): WallpaperStateDto {
  return {
    look: monitor('A').look,
    grid: GRID,
    originalUrl: 'mock://wall',
    hasBackup: false,
    working: false,
    dirty: false,
    pale: false,
    fingerprintMismatch: false,
    wallTint: '#7A6E62',
    screens: [monitor('A'), monitor('B', true)],
    activeScreenId: 'A',
    position: 'Fill',
    spanActive: false,
  }
}

describe('decodeWallpaperState — round-trip', () => {
  test('a valid single-monitor DTO round-trips unchanged', () => {
    const dto = { ...validState(), screens: [monitor('A')] }
    expect(decodeWallpaperState(dto)).toEqual(dto)
  })

  test('a valid multi-monitor (landscape + portrait) DTO round-trips unchanged', () => {
    const dto = validState()
    expect(decodeWallpaperState(dto)).toEqual(dto)
  })

  test('a span DTO round-trips', () => {
    const dto = { ...validState(), position: 'Span' as const, spanActive: true }
    expect(decodeWallpaperState(dto)).toEqual(dto)
  })

  test('an unreadable-source screen round-trips (source null)', () => {
    const noSource = { ...monitor('B', true), source: null, hasReadableSource: false }
    const dto = { ...validState(), screens: [monitor('A'), noSource] }
    expect(decodeWallpaperState(dto)).toEqual(dto)
  })

  test('unknown extra fields are tolerated (forward-compat), not rejected', () => {
    const dto = { ...validState(), futureField: 42 }
    expect(() => decodeWallpaperState(dto)).not.toThrow()
  })
})

describe('decodeWallpaperState — loud failure (never a silent empty)', () => {
  test('missing screens[] throws', () => {
    const bad = { ...validState() } as Record<string, unknown>
    delete bad.screens
    expect(() => decodeWallpaperState(bad)).toThrow(WallpaperDecodeError)
  })

  test('activeScreenId not among screens throws', () => {
    expect(() => decodeWallpaperState({ ...validState(), activeScreenId: 'NOPE' })).toThrow(/not a present screen/)
  })

  test('a bad position enum throws', () => {
    expect(() => decodeWallpaperState({ ...validState(), position: 'Sideways' })).toThrow(WallpaperDecodeError)
  })

  test('a mistyped nested field (bounds.w string) throws', () => {
    const bad = validState()
    ;(bad.screens[0].bounds as { w: unknown }).w = '1920'
    expect(() => decodeWallpaperState(bad)).toThrow(/bounds\.w/)
  })

  test('a non-object throws rather than collapsing', () => {
    expect(() => decodeWallpaperState(null)).toThrow(WallpaperDecodeError)
    expect(() => decodeWallpaperState('oops')).toThrow(WallpaperDecodeError)
  })
})

describe('decodeWallpaperOp', () => {
  test('round-trips a valid op result', () => {
    const op = { state: validState(), toast: null, ok: true }
    expect(decodeWallpaperOp(op)).toEqual(op)
  })

  test('round-trips a toasted op', () => {
    const op = { state: validState(), toast: { key: 'Toast_Applied', arg: null }, ok: true }
    expect(decodeWallpaperOp(op)).toEqual(op)
  })

  test('throws when the nested state is malformed', () => {
    expect(() => decodeWallpaperOp({ state: { nope: 1 }, toast: null, ok: true })).toThrow(WallpaperDecodeError)
  })
})
