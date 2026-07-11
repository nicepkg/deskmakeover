import type {
  ClarityDto,
  LookDto,
  MonitorLookDto,
  WallpaperGridInfoDto,
  WallpaperOpDto,
  WallpaperSourceDto,
  WallpaperStateDto,
  ZoneDto,
} from './types'

// Strict client decoder for the multi-monitor wallpaper contract (spec 04 §B1).
//
// The lesson this guards (payload-widening trap): a strict decoder that silently
// rejects a widened payload collapses the whole module to empty state, and the
// bug hides because nothing throws. So this decoder is strict about REQUIRED
// fields (a missing/mistyped field throws LOUDLY — never a silent empty), and
// tolerant of UNKNOWN extra fields (forward-compatible — a host that adds a field
// must not brick an older web). It validates in place and returns the same object
// typed, so ANY valid superset round-trips unchanged.
//
// This is the ONLY place the raw `wallpaper.getState`/apply/restore result is
// trusted into `WallpaperStateDto`; the store decodes here at the bridge boundary.

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

function nullableNumber(v: unknown, path: string): number | null {
  return v === null ? null : asNumber(v, path)
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

function decodeClarity(v: unknown, path: string): ClarityDto {
  const o = asObject(v, path)
  asString(o.level, `${path}.level`)
  asString(o.gradient, `${path}.gradient`)
  asNumber(o.angleDeg, `${path}.angleDeg`)
  nullableNumber(o.dimOverride, `${path}.dimOverride`)
  asString(o.tone, `${path}.tone`)
  nullableString(o.customScrim, `${path}.customScrim`)
  return v as ClarityDto
}

function decodeZone(v: unknown, path: string): ZoneDto {
  const o = asObject(v, path)
  asString(o.id, `${path}.id`)
  asNumber(o.cellX, `${path}.cellX`)
  asNumber(o.cellY, `${path}.cellY`)
  asNumber(o.cellsWide, `${path}.cellsWide`)
  asNumber(o.cellsTall, `${path}.cellsTall`)
  asString(o.title, `${path}.title`)
  nullableString(o.emoji, `${path}.emoji`)
  nullableString(o.accent, `${path}.accent`)
  asString(o.tone, `${path}.tone`)
  asString(o.material, `${path}.material`)
  asString(o.titleStyle, `${path}.titleStyle`)
  asBool(o.shadow, `${path}.shadow`)
  nullableNumber(o.fillOpacity, `${path}.fillOpacity`)
  asNumber(o.cornerRadius, `${path}.cornerRadius`)
  asString(o.titleSize, `${path}.titleSize`)
  nullableString(o.fontFamily, `${path}.fontFamily`)
  return v as ZoneDto
}

function decodeLook(v: unknown, path: string): LookDto {
  const o = asObject(v, path)
  asArray(o.zones, `${path}.zones`).forEach((z, i) => decodeZone(z, `${path}.zones[${i}]`))
  decodeClarity(o.clarity, `${path}.clarity`)
  return v as LookDto
}

function decodeGrid(v: unknown, path: string): WallpaperGridInfoDto {
  const o = asObject(v, path)
  for (const key of [
    'screenWidth',
    'screenHeight',
    'taskbarHeight',
    'iconPx',
    'cellWidth',
    'cellHeight',
    'inset',
    'columns',
    'rows',
  ] as const) {
    asNumber(o[key], `${path}.${key}`)
  }
  return v as WallpaperGridInfoDto
}

function decodeSource(v: unknown, path: string): WallpaperSourceDto | null {
  if (v === null) return null
  const o = asObject(v, path)
  asString(o.url, `${path}.url`)
  asNumber(o.width, `${path}.width`)
  asNumber(o.height, `${path}.height`)
  return v as WallpaperSourceDto
}

function decodeMonitorLook(v: unknown, path: string): MonitorLookDto {
  const o = asObject(v, path)
  asString(o.monitorId, `${path}.monitorId`)
  asString(o.name, `${path}.name`)
  const b = asObject(o.bounds, `${path}.bounds`)
  asNumber(b.x, `${path}.bounds.x`)
  asNumber(b.y, `${path}.bounds.y`)
  asNumber(b.w, `${path}.bounds.w`)
  asNumber(b.h, `${path}.bounds.h`)
  oneOf(o.orientation, ORIENTATIONS, `${path}.orientation`)
  decodeLook(o.look, `${path}.look`)
  decodeSource(o.source, `${path}.source`)
  decodeGrid(o.grid, `${path}.grid`)
  asBool(o.slideshowActive, `${path}.slideshowActive`)
  asBool(o.hasReadableSource, `${path}.hasReadableSource`)
  return v as MonitorLookDto
}

/** Strictly validate a raw `wallpaper.getState` result into a WallpaperStateDto.
 *  Throws WallpaperDecodeError on any missing/mistyped required field (loud —
 *  never a silent empty). Unknown extra fields pass through untouched. */
export function decodeWallpaperState(raw: unknown): WallpaperStateDto {
  const o = asObject(raw, 'WallpaperStateDto')
  // Active-screen mirror + global flags (legacy top-level shape).
  decodeLook(o.look, 'look')
  decodeGrid(o.grid, 'grid')
  nullableString(o.originalUrl, 'originalUrl')
  asBool(o.hasBackup, 'hasBackup')
  asBool(o.working, 'working')
  asBool(o.dirty, 'dirty')
  asBool(o.pale, 'pale')
  asBool(o.fingerprintMismatch, 'fingerprintMismatch')
  asString(o.wallTint, 'wallTint')
  // Multi-screen additions.
  const screens = asArray(o.screens, 'screens')
  screens.forEach((s, i) => decodeMonitorLook(s, `screens[${i}]`))
  const activeScreenId = asString(o.activeScreenId, 'activeScreenId')
  if (
    screens.length > 0 &&
    !screens.some((s) => (s as MonitorLookDto).monitorId === activeScreenId)
  ) {
    throw new WallpaperDecodeError(`activeScreenId "${activeScreenId}" is not a present screen`)
  }
  oneOf(o.position, POSITIONS, 'position')
  asBool(o.spanActive, 'spanActive')
  return raw as WallpaperStateDto
}

/** Strictly validate a `wallpaper.applyBaked`/`restore` op result. */
export function decodeWallpaperOp(raw: unknown): WallpaperOpDto {
  const o = asObject(raw, 'WallpaperOpDto')
  decodeWallpaperState(o.state)
  asBool(o.ok, 'ok')
  if (o.toast !== null) {
    const t = asObject(o.toast, 'toast')
    asString(t.key, 'toast.key')
    nullableString(t.arg, 'toast.arg')
  }
  return raw as WallpaperOpDto
}
