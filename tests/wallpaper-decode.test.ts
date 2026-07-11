import { describe, expect, test } from 'bun:test'
import { WallpaperDecodeError, decodeWallpaperResult, decodeWallpaperScreens } from '../src/bridge/wallpaper-decode'
import type { ScreenInfoDto, WallpaperScreensDto } from '../src/bridge/types'

// Strict client decoders for the SCHEMA-6 thin wallpaper contract (D1): getScreens
// (raw screens + globals) and the applyBaked/restore result (ok/toast/hasBackup).
// Guards the payload-widening trap: the decoder must ROUND-TRIP a valid (even widened)
// payload — never a silent empty collapse — and throw LOUDLY on a malformed field.

function screen(id: string, portrait = false): ScreenInfoDto {
  return {
    monitorId: id,
    name: id,
    bounds: portrait ? { x: 1920, y: 0, w: 1080, h: 1920 } : { x: 0, y: 0, w: 1920, h: 1080 },
    orientation: portrait ? 'portrait' : 'landscape',
    source: { url: 'mock://wall', width: 3840, height: 2400 },
    slideshowActive: false,
    hasReadableSource: true,
  }
}

function validScreens(): WallpaperScreensDto {
  return { screens: [screen('A'), screen('B', true)], position: 'Fill', spanActive: false }
}

describe('decodeWallpaperScreens — round-trip', () => {
  test('a single-monitor payload round-trips unchanged', () => {
    const dto = { screens: [screen('A')], position: 'Fill' as const, spanActive: false }
    expect(decodeWallpaperScreens(dto)).toEqual(dto)
  })

  test('a multi-monitor (landscape + portrait) payload round-trips unchanged', () => {
    const dto = validScreens()
    expect(decodeWallpaperScreens(dto)).toEqual(dto)
  })

  test('a span payload round-trips', () => {
    const dto = { ...validScreens(), position: 'Span' as const, spanActive: true }
    expect(decodeWallpaperScreens(dto)).toEqual(dto)
  })

  test('an unreadable-source screen round-trips (source null)', () => {
    const dyn = { ...screen('B', true), source: null, hasReadableSource: false }
    const dto = { ...validScreens(), screens: [screen('A'), dyn] }
    expect(decodeWallpaperScreens(dto)).toEqual(dto)
  })

  test('an empty screens[] round-trips (0-monitor host, never a throw)', () => {
    const dto = { screens: [], position: 'Fill' as const, spanActive: false }
    expect(decodeWallpaperScreens(dto)).toEqual(dto)
  })

  test('unknown extra fields are tolerated (forward-compat), not rejected', () => {
    const dto = { ...validScreens(), futureField: 42 }
    expect(() => decodeWallpaperScreens(dto)).not.toThrow()
  })
})

describe('decodeWallpaperScreens — loud failure (never a silent empty)', () => {
  test('missing screens[] throws', () => {
    const bad = { ...validScreens() } as Record<string, unknown>
    delete bad.screens
    expect(() => decodeWallpaperScreens(bad)).toThrow(WallpaperDecodeError)
  })

  test('a bad position enum throws', () => {
    expect(() => decodeWallpaperScreens({ ...validScreens(), position: 'Sideways' })).toThrow(WallpaperDecodeError)
  })

  test('a bad orientation enum throws', () => {
    const bad = validScreens()
    ;(bad.screens[0] as { orientation: unknown }).orientation = 'sideways'
    expect(() => decodeWallpaperScreens(bad)).toThrow(/orientation/)
  })

  test('a mistyped nested field (bounds.w string) throws', () => {
    const bad = validScreens()
    ;(bad.screens[0].bounds as { w: unknown }).w = '1920'
    expect(() => decodeWallpaperScreens(bad)).toThrow(/bounds\.w/)
  })

  test('a mistyped slideshowActive (not boolean) throws', () => {
    const bad = validScreens()
    ;(bad.screens[0] as { slideshowActive: unknown }).slideshowActive = 'no'
    expect(() => decodeWallpaperScreens(bad)).toThrow(/slideshowActive/)
  })

  test('a malformed source (width string) throws', () => {
    const bad = validScreens()
    ;(bad.screens[0].source as { width: unknown }).width = '3840'
    expect(() => decodeWallpaperScreens(bad)).toThrow(/source\.width/)
  })

  test('a non-object throws rather than collapsing', () => {
    expect(() => decodeWallpaperScreens(null)).toThrow(WallpaperDecodeError)
    expect(() => decodeWallpaperScreens('oops')).toThrow(WallpaperDecodeError)
  })
})

describe('decodeWallpaperResult', () => {
  test('round-trips a plain success result', () => {
    const op = { ok: true, toast: null, hasBackup: true }
    expect(decodeWallpaperResult(op)).toEqual(op)
  })

  test('round-trips a toasted result', () => {
    const op = { ok: true, toast: { key: 'Toast_Applied', arg: null }, hasBackup: true }
    expect(decodeWallpaperResult(op)).toEqual(op)
  })

  test('round-trips a failure (ok false, no backup)', () => {
    const op = { ok: false, toast: { key: 'Toast_ApplyFailed', arg: null }, hasBackup: false }
    expect(decodeWallpaperResult(op)).toEqual(op)
  })

  test('missing hasBackup throws (never silently defaults the safety flag)', () => {
    expect(() => decodeWallpaperResult({ ok: true, toast: null })).toThrow(/hasBackup/)
  })

  test('a mistyped ok throws', () => {
    expect(() => decodeWallpaperResult({ ok: 'yes', toast: null, hasBackup: true })).toThrow(/ok/)
  })

  test('a malformed toast throws', () => {
    expect(() => decodeWallpaperResult({ ok: true, toast: { arg: null }, hasBackup: true })).toThrow(/toast\.key/)
  })
})
