import type { LookDto, MonitorBounds, MonitorLookDto, ScreenOrientation, WallpaperSourceDto } from '@/bridge/types'
import { observedGrid } from '@/lib/observed-grid'

// Pure, DOM-free multi-monitor reconciliation (spec 04 §B3). Shared by the
// browser mock (persistence + on-load reconcile) and the wallpaper store
// (per-screen map merge across hot-plug / resolution / orientation changes), so
// the identity/fallback rules live in ONE tested place. No browser APIs here —
// unit-testable under `bun test` without a DOM (gridForBounds reads the
// observed-grid cache, which tests simply leave empty → deterministic constants).

/** h > w ⇒ portrait. Ultrawide (very wide landscape) is still landscape; the
 *  fit-mode decision (≥21:9 → fit-all) is a UI concern (Task 2), not here. */
export function orientationOf(bounds: MonitorBounds): ScreenOrientation {
  return bounds.h > bounds.w ? 'portrait' : 'landscape'
}

/** The untouched default look: no zones, clarity off (matches a fresh desktop). */
export function emptyLook(): LookDto {
  return {
    zones: [],
    clarity: { level: 'Off', gradient: 'Linear', angleDeg: 0, dimOverride: null, tone: 'Dark', customScrim: null },
  }
}

/** A persisted per-monitor look (mock: in-memory; host: SQLite
 *  wallpaper.look.v2::<monitorId>). `bounds` is the fallback fingerprint used
 *  when the device path can't be matched. */
export interface PersistedLook {
  look: LookDto
  bounds: MonitorBounds
}

/** A present physical monitor as reported by the host/mock, BEFORE its look is
 *  reconciled from persistence. */
export interface PhysicalMonitor {
  monitorId: string
  name: string
  bounds: MonitorBounds
  source: WallpaperSourceDto | null
  slideshowActive: boolean
  hasReadableSource: boolean
}

export type ReconcileMatch = 'path' | 'bounds' | 'default'

/** Decide one present monitor's look from the persistence store (§B3):
 *  1. device-path match → restore that look;
 *  2. else bounds fingerprint match against a persisted entry NOT already
 *     claimed by a present device path → restore ([WINDOWS-VERIFY]: the real host
 *     matches on EDID/DisplayConfig, not raw bounds, and confirms when ambiguous);
 *  3. else → default empty look (a genuinely new monitor).
 *  Detached monitors are NEVER pruned by the caller — a reconnected monitor
 *  resumes its dormant look. */
export function reconcileMonitorLook(
  monitor: PhysicalMonitor,
  persisted: Map<string, PersistedLook>,
  claimedByPath: ReadonlySet<string>,
): { look: LookDto; matchedBy: ReconcileMatch } {
  const byPath = persisted.get(monitor.monitorId)
  if (byPath) return { look: byPath.look, matchedBy: 'path' }
  for (const [id, entry] of persisted) {
    if (claimedByPath.has(id)) continue
    if (boundsEqual(entry.bounds, monitor.bounds)) return { look: entry.look, matchedBy: 'bounds' }
  }
  return { look: emptyLook(), matchedBy: 'default' }
}

/** Build the reconciled screens[] for a set of present monitors. The set of
 *  device paths present becomes the `claimedByPath` guard so a bounds-fallback
 *  never steals a look that a still-present path already owns. */
export function reconcileScreens(
  monitors: PhysicalMonitor[],
  persisted: Map<string, PersistedLook>,
): MonitorLookDto[] {
  const claimedByPath = new Set(monitors.filter((m) => persisted.has(m.monitorId)).map((m) => m.monitorId))
  return monitors.map((m) => ({
    monitorId: m.monitorId,
    name: m.name,
    bounds: m.bounds,
    orientation: orientationOf(m.bounds),
    look: reconcileMonitorLook(m, persisted, claimedByPath).look,
    source: m.source,
    grid: gridForBounds(m.bounds),
    slideshowActive: m.slideshowActive,
    hasReadableSource: m.hasReadableSource,
  }))
}

function boundsEqual(a: MonitorBounds, b: MonitorBounds): boolean {
  return a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h
}

// Grid geometry derived from a monitor's physical bounds. The cell PITCH + icon size come from
// the last icon scan's OBSERVED platform grid when available (observed-grid.ts — IFolderView
// GetSpacing truth), so zones snap to the SAME cells the real desktop icons sit on; the
// constants below are the pre-scan / browser-mock fallback (owner report 2026-07-16: a second
// fabricated 92px lattice drifted from the real desktop grid). The taskbar reserve stays the
// per-bounds constant on purpose: the observed reading is the PRIMARY monitor's, and applying
// it to a secondary monitor (which may have no taskbar) would wrongly drop a row (codex P2).
// The wallpaper store re-derives grids via `regridScreens` once a scan lands, so a screen
// reconciled before the first scan does not keep the fallback pitch (codex P2 timing).
const TASKBAR_HEIGHT = 48
const ICON_PX = 48
const CELL = 92
const INSET = 14

export function gridForBounds(bounds: MonitorBounds) {
  const o = observedGrid()
  const iconPx = o?.iconPx ?? ICON_PX
  const cellW = o?.cellWidth ?? CELL
  const cellH = o?.cellHeight ?? CELL
  const usableH = bounds.h - TASKBAR_HEIGHT - INSET * 2
  return {
    screenWidth: bounds.w,
    screenHeight: bounds.h,
    taskbarHeight: TASKBAR_HEIGHT,
    iconPx,
    cellWidth: cellW,
    cellHeight: cellH,
    inset: INSET,
    columns: Math.max(1, Math.floor((bounds.w - INSET * 2) / cellW)),
    rows: Math.max(1, Math.floor(usableH / cellH)),
  }
}

// ---- store-side per-screen runtime state (spec 04 §B2) ----
// The wallpaper store's source of truth is `screens: Record<monitorId, ScreenLook>`
// + `activeScreenId`. Each screen carries its own draft look, imported source,
// zone selection AND undo/redo stacks, so editing/undoing screen A never touches
// screen B. The top-level store fields (`look`, `past`, `selected`, …) mirror the
// active screen for UI back-compat + single-monitor parity.

export interface ScreenLook {
  look: LookDto
  /** Per-screen imported/desktop source ref (mirrors the active compositor source). */
  source: WallpaperSourceDto | null
  /** Imported source name (壁纸导入); null = the screen's current desktop wallpaper. */
  sourceName: string | null
  /** Object URL of the imported source; null = the screen's originalUrl. */
  sourceUrl: string | null
  /** Selected zone id within THIS screen's look. */
  selected: string | null
  past: LookDto[]
  future: LookDto[]
}

/** Seed a fresh per-screen runtime entry from a reconciled monitor DTO — the
 *  persisted draft look, no imported source, empty undo history. */
export function screenLookFromDto(dto: MonitorLookDto): ScreenLook {
  return {
    look: dto.look,
    source: dto.source,
    sourceName: null,
    sourceUrl: null,
    selected: null,
    past: [],
    future: [],
  }
}

/** Merge the previous per-screen map with a freshly reported screens[] (initial
 *  load OR a monitor hot-plug / resolution / orientation change §B3):
 *  - present + already tracked → KEEP the in-progress ScreenLook (draft + undo);
 *  - present + new → seed from the DTO;
 *  - absent → dropped from the ACTIVE map (its persisted look stays dormant on
 *    the host / in the mock, so a reconnected monitor resumes). */
export function mergeScreenMap(
  prev: Record<string, ScreenLook>,
  dtoScreens: MonitorLookDto[],
): Record<string, ScreenLook> {
  const next: Record<string, ScreenLook> = {}
  for (const dto of dtoScreens) {
    next[dto.monitorId] = prev[dto.monitorId] ?? screenLookFromDto(dto)
  }
  return next
}

/** Keep the user on their current screen across a reconcile when it's still
 *  present; otherwise honor the host's activeScreenId, else the first screen. */
export function pickActiveScreenId(
  prevActive: string | null,
  screens: MonitorLookDto[],
  dtoActive: string,
): string {
  if (prevActive && screens.some((s) => s.monitorId === prevActive)) return prevActive
  if (screens.some((s) => s.monitorId === dtoActive)) return dtoActive
  return screens[0]?.monitorId ?? dtoActive
}
