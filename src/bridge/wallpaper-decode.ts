import type { ScreenInfoDto, WallpaperResultDto, WallpaperScreensDto, WallpaperSourceDto } from './types'

// Strict client decoders for the SCHEMA-6 thin wallpaper contract (D1). Only two
// payloads still cross the bridge: `wallpaper.getScreens` (raw screens + globals)
// and the `applyBaked`/`restore` result (ok/toast/hasBackup). Looks no longer cross
// the bridge (localStorage-persisted, frontend-assembled), so there is nothing here
// to decode a look/zone/clarity/grid.
//
// The lesson this guards (payload-widening trap): a strict decoder that silently
// rejects a widened payload collapses the module to empty state, and the bug hides
// because nothing throws. So these decoders are strict about REQUIRED fields (a
// missing/mistyped field throws LOUDLY — never a silent empty) and tolerant of
// UNKNOWN extra fields (forward-compatible — a host that adds a field must not brick
// an older web). They validate in place and return the same object typed, so ANY
// valid superset round-trips unchanged.

export class WallpaperDecodeError extends Error {
  constructor(message: string) {
    super(`[wallpaper-decode] ${message}`)
    this.name = 'WallpaperDecodeError'
  }
}

type Rec = Record<string, unknown>

function asObject(v: unknown, path: string): Rec {
  if (v === null || typeof v !== 'object' || Array.isArray(v)) {
    throw new WallpaperDecodeError(`${path}: expected object, got ${describe(v)}`)
  }
  return v as Rec
}

function asArray(v: unknown, path: string): unknown[] {
  if (!Array.isArray(v)) throw new WallpaperDecodeError(`${path}: expected array, got ${describe(v)}`)
  return v
}

function asString(v: unknown, path: string): string {
  if (typeof v !== 'string') throw new WallpaperDecodeError(`${path}: expected string, got ${describe(v)}`)
  return v
}

function asNumber(v: unknown, path: string): number {
  if (typeof v !== 'number' || Number.isNaN(v)) {
    throw new WallpaperDecodeError(`${path}: expected number, got ${describe(v)}`)
  }
  return v
}

function asBool(v: unknown, path: string): boolean {
  if (typeof v !== 'boolean') throw new WallpaperDecodeError(`${path}: expected boolean, got ${describe(v)}`)
  return v
}

function nullableString(v: unknown, path: string): string | null {
  return v === null ? null : asString(v, path)
}

function oneOf<T extends string>(v: unknown, allowed: readonly T[], path: string): T {
  const s = asString(v, path)
  if (!allowed.includes(s as T)) {
    throw new WallpaperDecodeError(`${path}: "${s}" not one of ${allowed.join(' | ')}`)
  }
  return s as T
}

function describe(v: unknown): string {
  if (v === null) return 'null'
  if (Array.isArray(v)) return 'array'
  return typeof v
}

const POSITIONS = ['Center', 'Tile', 'Stretch', 'Fit', 'Fill', 'Span'] as const
const ORIENTATIONS = ['portrait', 'landscape'] as const

function decodeSource(v: unknown, path: string): WallpaperSourceDto | null {
  if (v === null) return null
  const o = asObject(v, path)
  asString(o.url, `${path}.url`)
  asNumber(o.width, `${path}.width`)
  asNumber(o.height, `${path}.height`)
  return v as WallpaperSourceDto
}

function decodeScreenInfo(v: unknown, path: string): ScreenInfoDto {
  const o = asObject(v, path)
  asString(o.monitorId, `${path}.monitorId`)
  asString(o.name, `${path}.name`)
  const b = asObject(o.bounds, `${path}.bounds`)
  asNumber(b.x, `${path}.bounds.x`)
  asNumber(b.y, `${path}.bounds.y`)
  asNumber(b.w, `${path}.bounds.w`)
  asNumber(b.h, `${path}.bounds.h`)
  oneOf(o.orientation, ORIENTATIONS, `${path}.orientation`)
  decodeSource(o.source, `${path}.source`)
  asBool(o.slideshowActive, `${path}.slideshowActive`)
  asBool(o.hasReadableSource, `${path}.hasReadableSource`)
  return v as ScreenInfoDto
}

/** Strictly validate a raw `wallpaper.getScreens` result into a WallpaperScreensDto.
 *  Throws on any missing/mistyped required field (loud — never a silent empty).
 *  Unknown extra fields pass through untouched (forward-compat). */
export function decodeWallpaperScreens(raw: unknown): WallpaperScreensDto {
  const o = asObject(raw, 'WallpaperScreensDto')
  asArray(o.screens, 'screens').forEach((s, i) => decodeScreenInfo(s, `screens[${i}]`))
  oneOf(o.position, POSITIONS, 'position')
  asBool(o.spanActive, 'spanActive')
  return raw as WallpaperScreensDto
}

/** Strictly validate a `wallpaper.applyBaked`/`restore` thin result. */
export function decodeWallpaperResult(raw: unknown): WallpaperResultDto {
  const o = asObject(raw, 'WallpaperResultDto')
  asBool(o.ok, 'ok')
  asBool(o.hasBackup, 'hasBackup')
  if (o.toast !== null) {
    const t = asObject(o.toast, 'toast')
    asString(t.key, 'toast.key')
    nullableString(t.arg, 'toast.arg')
  }
  return raw as WallpaperResultDto
}
