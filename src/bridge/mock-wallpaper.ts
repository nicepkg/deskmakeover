import type { LookDto, MonitorBounds, WallpaperOpDto, WallpaperPosition, WallpaperSourceDto, WallpaperStateDto } from './types'
import { mockWallpaperUrl, probeRealWallpaper } from './mock-desktop'
import { type PersistedLook, type PhysicalMonitor, reconcileScreens } from '@/lib/monitor-reconcile'

// Browser-only multi-monitor wallpaper mock (spec 04 §B1–B3). Delegated from
// mock.ts exactly as icons.* delegates to mock-desktop. Owns: the dev-knob
// monitor set, per-monitor wallpaper sources, the per-monitor persistence store
// (simulating SQLite wallpaper.look.v2::<monitorId>) and the reconcile-on-getState
// path. The reconcile RULES live in lib/monitor-reconcile (pure, DOM-free,
// unit-tested); this file is the browser glue (scene bitmaps + dev knob).
//
// [WINDOWS-VERIFY] every host seam below is mock-only. The real host reports the
// monitor set (IDesktopWallpaper GetMonitorDevicePathAt/GetMonitorRECT), the true
// per-monitor GetWallpaper source + slideshow flags, and persists looks in SQLite;
// per-monitor SetWallpaper/restore write the real desktop.

const WALL_TINT = '#7A6E62'

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

// ---- per-monitor persistence (mock SQLite) + global desktop flags ----
const persisted = new Map<string, PersistedLook>()
const globalState = { hasBackup: false, fingerprintMismatch: false }

// ---- per-monitor scene bitmaps (distinct wallpapers per screen) ----
const sceneCache = new Map<string, string>()

function sceneFor(monitor: LayoutMonitor): string {
  if (monitor.monitorId === PRIMARY) return mockWallpaperUrl()
  const cached = sceneCache.get(monitor.monitorId)
  if (cached) return cached
  const url = generateScene(monitor)
  sceneCache.set(monitor.monitorId, url)
  return url
}

// Distinct warm gradient per secondary monitor (blue/violet accents are banned —
// this file is colour-scanned). Portrait monitors render at their true aspect so
// the switcher crop + fit-height preview have a correctly-shaped source.
const SCENE_PALETTES: Record<string, [string, string, string]> = {
  [PORTRAIT]: ['#2E2A3A', '#6E4526', '#D9A06B'],
  [THIRD]: ['#1F2A24', '#3F6E5A', '#A8C9B6'],
}

function generateScene(monitor: LayoutMonitor): string {
  if (typeof document === 'undefined') return `mock-scene://${monitor.monitorId}`
  const canvas = document.createElement('canvas')
  canvas.width = monitor.bounds.w
  canvas.height = monitor.bounds.h
  const ctx = canvas.getContext('2d')
  if (!ctx) return `mock-scene://${monitor.monitorId}`
  const [a, b, c] = SCENE_PALETTES[monitor.monitorId] ?? ['#3A322A', '#8A5A33', '#E8C9A0']
  const grad = ctx.createLinearGradient(0, 0, 0, canvas.height)
  grad.addColorStop(0, a)
  grad.addColorStop(0.55, b)
  grad.addColorStop(1, c)
  ctx.fillStyle = grad
  ctx.fillRect(0, 0, canvas.width, canvas.height)
  return canvas.toDataURL('image/jpeg', 0.85)
}

function sourceFor(monitor: LayoutMonitor): WallpaperSourceDto | null {
  if (!monitor.hasReadableSource) return null
  // [WINDOWS-VERIFY] real source dims come from the WIC decode; the mock uses the
  // monitor bounds (the compositor cover-crops either way).
  return { url: sceneFor(monitor), width: monitor.bounds.w, height: monitor.bounds.h }
}

function physicalMonitors(): { monitors: PhysicalMonitor[]; position: WallpaperPosition } {
  const layout = MONITOR_LAYOUTS[currentMonitorScenario()]
  return {
    position: layout.position,
    monitors: layout.monitors.map((m) => ({
      monitorId: m.monitorId,
      name: m.name,
      bounds: m.bounds,
      source: sourceFor(m),
      slideshowActive: m.slideshowActive,
      hasReadableSource: m.hasReadableSource,
    })),
  }
}

function dirtyOf(look: LookDto): boolean {
  return look.zones.length > 0 || look.clarity.level !== 'Off'
}

function buildState(): WallpaperStateDto {
  const { monitors, position } = physicalMonitors()
  const screens = reconcileScreens(monitors, persisted)
  // The mock's default active screen is the primary; the store owns switching
  // client-side (selectScreen never crosses the bridge).
  const active = screens[0]
  return {
    look: active.look,
    grid: active.grid,
    originalUrl: active.source?.url ?? null,
    hasBackup: globalState.hasBackup,
    working: false,
    dirty: screens.some((s) => dirtyOf(s.look)),
    pale: false,
    fingerprintMismatch: globalState.fingerprintMismatch,
    wallTint: WALL_TINT,
    screens,
    activeScreenId: active.monitorId,
    position,
    spanActive: position === 'Span',
  }
}

function boundsOf(monitorId: string): MonitorBounds {
  const layout = MONITOR_LAYOUTS[currentMonitorScenario()]
  return layout.monitors.find((m) => m.monitorId === monitorId)?.bounds ?? LANDSCAPE
}

export async function mockWallpaperCall(method: string, params: unknown): Promise<unknown> {
  switch (method) {
    case 'wallpaper.getState':
      await probeRealWallpaper()
      return buildState()
    case 'wallpaper.getSource': {
      await probeRealWallpaper()
      const state = buildState()
      const active = state.screens.find((s) => s.monitorId === state.activeScreenId)
      const src = active?.source
      return src ?? { url: mockWallpaperUrl(), width: state.grid.screenWidth, height: state.grid.screenHeight }
    }
    case 'wallpaper.setLook': {
      const { monitorId, look } = params as { monitorId: string; look: LookDto }
      // Persist the draft look keyed by device path (mock SQLite). Detached
      // monitors keep their entry (never pruned) so a replug resumes.
      persisted.set(monitorId, { look, bounds: boundsOf(monitorId) })
      return null
    }
    case 'wallpaper.applyBaked': {
      const { monitorId, look, pngBase64 } = params as { monitorId: string; look: LookDto; pngBase64: string }
      ;(window as { __dmBakedPng?: string }).__dmBakedPng = `data:image/png;base64,${pngBase64}`
      persisted.set(monitorId, { look, bounds: boundsOf(monitorId) })
      globalState.hasBackup = true
      return { state: buildState(), toast: null, ok: true } satisfies WallpaperOpDto
    }
    case 'wallpaper.restore': {
      // Whole-desktop restore ('all') reverts to the pre-first-apply snapshot: the
      // desktop no longer reflects a design, but the DRAFT looks survive (the store
      // derives dirty from them). A per-monitor restore is [WINDOWS-VERIFY] on the
      // host — the mock treats any scope as the whole-desktop hasBackup flip.
      globalState.hasBackup = false
      return { state: buildState(), toast: null, ok: true } satisfies WallpaperOpDto
    }
    default:
      throw new Error(`[mock wallpaper] unhandled method: ${method}`)
  }
}

/** Test/dev seam: reset the mock's persistence + global flags. */
export function __resetWallpaperMock(): void {
  persisted.clear()
  sceneCache.clear()
  globalState.hasBackup = false
  globalState.fingerprintMismatch = false
}
