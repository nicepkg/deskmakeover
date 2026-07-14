import type {
  LookDto,
  MonitorBounds,
  MonitorLookDto,
  WallpaperScreensDto,
  WallpaperStateDto,
} from '@/bridge/types'
import {
  type PersistedLook,
  type ScreenLook,
  emptyLook,
  gridForBounds,
  pickActiveScreenId,
  reconcileScreens,
} from './monitor-reconcile'
import { migrateWallpaperZoneEnums } from './preset-migrations'

// Frontend wallpaper-state ASSEMBLY (schema 6, owner ruling D1 2026-07-12). The
// host is thin platform I/O: `wallpaper.getScreens` returns raw screens + globals
// (NO looks). This module is the ONE place that turns that thin DTO into the store's
// `WallpaperStateDto` — reconciling per-monitor draft looks from localStorage, deriving
// each grid from bounds, and mirroring the active screen to the top level. The mock's
// old `buildState()` lived in the browser mock; per D1 it moves HERE so the real host
// and the browser mock share ONE assembly path (DOM-free, `bun test`-friendly).

/** Neutral desk tint painted behind an unreadable dynamic-wallpaper screen (§A4). */
export const WALL_TINT = '#7A6E62'

/** localStorage key for one monitor's persisted draft look — `dm.icons.bareLook`
 *  sibling. One entry PER device path; the value is a `PersistedLook` (look + bounds
 *  fingerprint for the reconcile bounds-fallback). */
const LOOK_KEY_PREFIX = 'wallpaper.look.v2::'

/** Landscape fallback bounds for a 0-screen host (the store swaps in a virtual
 *  screen; this only keeps `assembleWallpaperState` from dividing by an empty set). */
const FALLBACK_BOUNDS: MonitorBounds = { x: 0, y: 0, w: 1920, h: 1080 }

/** A look is dirty (worth applying/exporting) once it carries any zone or clarity. */
export function lookDirty(look: LookDto): boolean {
  return look.zones.length > 0 || look.clarity.level !== 'Off'
}

/** Read every persisted per-monitor draft look into a reconcile-ready map. Guarded so
 *  a non-browser / privacy-mode env degrades to no persistence, never crashes; a single
 *  corrupt entry is skipped rather than bricking the whole load. */
export function loadPersistedLooks(): Map<string, PersistedLook> {
  const map = new Map<string, PersistedLook>()
  try {
    if (typeof localStorage === 'undefined') return map
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i)
      if (!key || !key.startsWith(LOOK_KEY_PREFIX)) continue
      const raw = localStorage.getItem(key)
      if (!raw) continue
      try {
        const parsed = JSON.parse(raw) as PersistedLook
        if (parsed?.look && parsed?.bounds) {
          migrateLook(parsed.look)
          map.set(key.slice(LOOK_KEY_PREFIX.length), parsed)
        }
      } catch {
        // Corrupt entry: skip it, keep the rest of the persistence readable.
      }
    }
  } catch {
    // Storage unavailable (SSR / privacy mode): reconcile against an empty map.
  }
  return map
}

/** Round-3 lineup migration (2026-07-15, one-way, silent) — the mapping tables
 *  live in the shared migration chain (lib/preset-migrations, spec 09 §3) so
 *  load-from-disk and future wallpaper-preset import migrate identically. */
function migrateLook(look: LookDto): void {
  for (const zone of look.zones) {
    migrateWallpaperZoneEnums(zone as unknown as { material: string; titleStyle: string })
  }
}

/** Persist one monitor's draft look, keyed by device path (the seam that replaced the
 *  `wallpaper.setLook` bridge verb, D1). The `bounds` ride along as the reconcile
 *  bounds-fallback fingerprint. Guarded like `persistBareLook`. */
export function savePersistedLook(monitorId: string, look: LookDto, bounds: MonitorBounds): void {
  try {
    if (typeof localStorage === 'undefined') return
    localStorage.setItem(LOOK_KEY_PREFIX + monitorId, JSON.stringify({ look, bounds } satisfies PersistedLook))
  } catch {
    // Storage unavailable: the draft stays session-only (the store still renders it).
  }
}

/** Assemble the store's `WallpaperStateDto` from a thin `getScreens` DTO:
 *  1. reconcile each raw screen against the persisted looks (monitor-reconcile rules);
 *  2. let a LIVE per-screen draft (mid-edit, ahead of the debounced localStorage write)
 *     win over the persisted look;
 *  3. mirror the active screen to the top-level fields (single-monitor parity).
 *  `hasBackup` comes straight off the getScreens payload (the host's durable-snapshot
 *  truth), so it is fresh on cold start AND after every mutating op's re-fetch. */
export function assembleWallpaperState(
  dto: WallpaperScreensDto,
  persisted: Map<string, PersistedLook>,
  liveScreens: Record<string, ScreenLook>,
  opts: { prevActiveId: string | null },
): WallpaperStateDto {
  const reconciled = reconcileScreens(dto.screens, persisted)
  const screens: MonitorLookDto[] = reconciled.map((m) => {
    const live = liveScreens[m.monitorId]
    return live ? { ...m, look: live.look } : m
  })
  const activeId = pickActiveScreenId(opts.prevActiveId, screens, screens[0]?.monitorId ?? '')
  const active = screens.find((s) => s.monitorId === activeId)
  return {
    look: active?.look ?? emptyLook(),
    grid: active?.grid ?? gridForBounds(FALLBACK_BOUNDS),
    originalUrl: active?.source?.url ?? null,
    hasBackup: dto.hasBackup,
    working: false,
    dirty: screens.some((s) => lookDirty(s.look)),
    pale: false,
    fingerprintMismatch: false,
    wallTint: WALL_TINT,
    screens,
    activeScreenId: active?.monitorId ?? activeId,
    position: dto.position,
    spanActive: dto.spanActive,
  }
}
