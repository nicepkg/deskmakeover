import type { MonitorBounds, ScreenInfoDto, WallpaperPosition, WallpaperResultDto, WallpaperScreensDto, WallpaperSourceDto } from './types'
import { mockWallpaperUrl, probeRealWallpaper } from './mock-desktop'
import { orientationOf } from '@/lib/monitor-reconcile'

// Browser-only multi-monitor wallpaper mock (schema 6, owner ruling D1 2026-07-12).
// Serves the THIN host verbs ONLY: getScreens (raw topology + global position/span),
// getSource (the active screen's decoded source), and the thin applyBaked/restore
// (an in-memory pre-first-apply-snapshot flag flip + a stashed baked PNG for the dev
// loop). Reconcile, per-monitor draft persistence and WallpaperStateDto assembly are
// FRONTEND now (lib/wallpaper-assemble + the store) — this file no longer knows about
// looks. It stays the browser glue: the dev-knob monitor set + per-monitor scene bitmaps.
//
// [WINDOWS-VERIFY] every host seam below is mock-only. The real host reports the
// monitor set (IDesktopWallpaper GetMonitorDevicePathAt/GetMonitorRECT), the true
// per-monitor GetWallpaper source + slideshow flags; applyBaked/restore write the real
// desktop and capture/restore the pre-first-apply snapshot durably.

// ---- DEV monitor-set knob (dev menu; parallels the icons SCENARIO_KEY) ----
// 1 = single monitor (parity), 2 = landscape + portrait (default), 3 = + a third
// landscape, span = Span position (unified canvas), slideshow = secondary is a
// Windows slideshow, nosource = secondary is an unreadable dynamic wallpaper.
export type MonitorScenario = '1' | '2' | '3' | 'span' | 'slideshow' | 'nosource'
export const MONITOR_SCENARIO_KEY = 'dm.dev.monitors'

export function currentMonitorScenario(): MonitorScenario {
  const v = typeof localStorage !== 'undefined' ? localStorage.getItem(MONITOR_SCENARIO_KEY) : null
  return v === '1' || v === '3' || v === 'span' || v === 'slideshow' || v === 'nosource' ? v : '2'
}

/** Stable mock device paths. [WINDOWS-VERIFY] real paths come from the host. */
const PRIMARY = '\\\\?\\DISPLAY#MOCK#0'
const PORTRAIT = '\\\\?\\DISPLAY#MOCK#1'
const THIRD = '\\\\?\\DISPLAY#MOCK#2'

interface LayoutMonitor {
  monitorId: string
  name: string
  bounds: MonitorBounds
  slideshowActive: boolean
  hasReadableSource: boolean
}

interface Layout {
  position: WallpaperPosition
  monitors: LayoutMonitor[]
}

const LANDSCAPE: MonitorBounds = { x: 0, y: 0, w: 1920, h: 1080 }
const PORTRAIT_BOUNDS: MonitorBounds = { x: 1920, y: 0, w: 1080, h: 1920 }
const THIRD_BOUNDS: MonitorBounds = { x: 3000, y: 0, w: 2560, h: 1440 }

const primary: LayoutMonitor = { monitorId: PRIMARY, name: '主显示器', bounds: LANDSCAPE, slideshowActive: false, hasReadableSource: true }
const portrait: LayoutMonitor = { monitorId: PORTRAIT, name: '竖屏', bounds: PORTRAIT_BOUNDS, slideshowActive: false, hasReadableSource: true }

// Pure layout table (no scene URLs) — exported so it is inspectable/testable.
export const MONITOR_LAYOUTS: Record<MonitorScenario, Layout> = {
  '1': { position: 'Fill', monitors: [primary] },
  '2': { position: 'Fill', monitors: [primary, portrait] },
  '3': { position: 'Fill', monitors: [primary, portrait, { monitorId: THIRD, name: '副屏', bounds: THIRD_BOUNDS, slideshowActive: false, hasReadableSource: true }] },
  span: { position: 'Span', monitors: [primary, portrait] },
  slideshow: { position: 'Fill', monitors: [primary, { ...portrait, slideshowActive: true }] },
  nosource: { position: 'Fill', monitors: [primary, { ...portrait, hasReadableSource: false }] },
}

// ---- per-monitor scene bitmaps (a distinct REAL wallpaper per screen) ----
// Owner call: no synthetic gradients for secondary monitors — every screen shows a
// real image, cover-cropped to its aspect exactly like the primary, so the switcher
// crop + fit-height preview read as genuine desktops. Picks avoid the primary's blue
// `wallpaper-default` so the two screens are clearly distinguishable at a glance.
const SECONDARY_WALLPAPERS: Record<string, string> = {
  [PORTRAIT]: '/real-icons/wallpapers/wallpaper-dark.jpg',
  [THIRD]: '/real-icons/wallpapers/wallpaper-office.jpg',
}

function sceneFor(monitor: LayoutMonitor): string {
  if (monitor.monitorId === PRIMARY) return mockWallpaperUrl()
  return SECONDARY_WALLPAPERS[monitor.monitorId] ?? '/real-icons/wallpapers/wallpaper-gamer.jpg'
}

// The mock wallpapers (public/real-icons/wallpapers/*) are all 3840×2400. Report
// their TRUE dims — NOT the monitor bounds — so the compositor cover-crops to each
// screen's aspect; reporting the bounds made a landscape image stretch vertically
// onto a portrait monitor. [WINDOWS-VERIFY] the real host reports each image's true
// decoded dims (WIC), which likewise must be the image's, not the monitor's.
const MOCK_WALLPAPER_DIMS = { w: 3840, h: 2400 }
function sourceFor(monitor: LayoutMonitor): WallpaperSourceDto | null {
  if (!monitor.hasReadableSource) return null
  return { url: sceneFor(monitor), width: MOCK_WALLPAPER_DIMS.w, height: MOCK_WALLPAPER_DIMS.h }
}

/** The thin getScreens payload for the current dev scenario. */
function screenInfos(): WallpaperScreensDto {
  const layout = MONITOR_LAYOUTS[currentMonitorScenario()]
  const screens: ScreenInfoDto[] = layout.monitors.map((m) => ({
    monitorId: m.monitorId,
    name: m.name,
    bounds: m.bounds,
    orientation: orientationOf(m.bounds),
    source: sourceFor(m),
    slideshowActive: m.slideshowActive,
    hasReadableSource: m.hasReadableSource,
  }))
  return { screens, position: layout.position, spanActive: layout.position === 'Span' }
}

// Whole-desktop pre-first-apply snapshot flag (host-side truth on Windows; here an
// in-memory flip). true after the first apply, false after a whole-desktop restore.
let hasBackup = false

export async function mockWallpaperCall(method: string, params: unknown): Promise<unknown> {
  switch (method) {
    case 'wallpaper.getScreens':
      await probeRealWallpaper()
      return screenInfos()
    case 'wallpaper.getSource': {
      await probeRealWallpaper()
      const src = screenInfos().screens[0]?.source
      return src ?? { url: mockWallpaperUrl(), width: MOCK_WALLPAPER_DIMS.w, height: MOCK_WALLPAPER_DIMS.h }
    }
    case 'wallpaper.applyBaked': {
      const { pngBase64 } = params as { monitorId: string; pngBase64: string }
      if (typeof window !== 'undefined') (window as { __dmBakedPng?: string }).__dmBakedPng = `data:image/png;base64,${pngBase64}`
      hasBackup = true // the mock "captures" a pre-first-apply snapshot on the first apply
      return { ok: true, toast: null, hasBackup } satisfies WallpaperResultDto
    }
    case 'wallpaper.restore': {
      // Whole-desktop restore ('all') reverts to the pre-first-apply snapshot: the
      // desktop no longer reflects a design. Draft looks survive on the frontend, so
      // the store still derives `dirty` from them. A per-monitor restore is
      // [WINDOWS-VERIFY] on the host — the mock treats any scope as the hasBackup flip.
      hasBackup = false
      return { ok: true, toast: null, hasBackup } satisfies WallpaperResultDto
    }
    default:
      throw new Error(`[mock wallpaper] unhandled method: ${method}`)
  }
}

/** Test/dev seam: reset the mock's snapshot flag. */
export function __resetWallpaperMock(): void {
  hasBackup = false
}
