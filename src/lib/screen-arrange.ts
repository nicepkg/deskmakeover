import type { MonitorBounds, MonitorLookDto, WallpaperStateDto } from '@/bridge/types'

// Pure, DOM-free geometry for the screen switcher (spec 04 §B4). The switcher
// reproduces the OS "Displays arrangement": every monitor's real bounds scaled by
// ONE factor into a small box, preserving each screen's aspect (orientation =
// shape) AND their relative positions. Kept pure so the render-gating + layout are
// unit-testable without a DOM (the component is a thin skin over these).

type ScreenLike = Pick<MonitorLookDto, 'monitorId' | 'bounds'>

/** One placed tile inside the arrangement box (top-left origin, px). */
export interface TileLayout {
  monitorId: string
  left: number
  top: number
  width: number
  height: number
  /** 0-based order in the reported screens[] — the badge shows index + 1. */
  index: number
  isPrimary: boolean
}

export interface Arrangement {
  tiles: TileLayout[]
  /** The arrangement box size in px — the switcher sizes its inner canvas to this. */
  width: number
  height: number
}

/** Show the switcher only with ≥2 real screens and NOT in Span mode — Span paints
 *  ONE image across every monitor, so per-screen selection is undefined and the UI
 *  degrades to a unified canvas (§B6). A single monitor renders nothing (parity). */
export function shouldShowSwitcher(screenCount: number, spanActive: boolean): boolean {
  return screenCount >= 2 && !spanActive
}

/** The primary monitor sits at the virtual-desktop origin (0,0); fall back to the
 *  first reported screen when no monitor is anchored there. */
export function primaryScreenId(screens: ScreenLike[]): string | null {
  if (screens.length === 0) return null
  const atOrigin = screens.find((s) => s.bounds.x === 0 && s.bounds.y === 0)
  return (atOrigin ?? screens[0]).monitorId
}

/** Scale every monitor's bounds by one factor so the whole arrangement fits within
 *  maxW×maxH while keeping each screen's aspect and relative position intact. */
export function arrangeTiles(screens: ScreenLike[], maxW: number, maxH: number): Arrangement {
  if (screens.length === 0) return { tiles: [], width: 0, height: 0 }
  const minX = Math.min(...screens.map((s) => s.bounds.x))
  const minY = Math.min(...screens.map((s) => s.bounds.y))
  const maxX = Math.max(...screens.map((s) => s.bounds.x + s.bounds.w))
  const maxY = Math.max(...screens.map((s) => s.bounds.y + s.bounds.h))
  const spanW = Math.max(1, maxX - minX)
  const spanH = Math.max(1, maxY - minY)
  const scale = Math.min(maxW / spanW, maxH / spanH)
  const primaryId = primaryScreenId(screens)
  const tiles: TileLayout[] = screens.map((s, index) => ({
    monitorId: s.monitorId,
    left: (s.bounds.x - minX) * scale,
    top: (s.bounds.y - minY) * scale,
    width: s.bounds.w * scale,
    height: s.bounds.h * scale,
    index,
    isPrimary: s.monitorId === primaryId,
  }))
  return { tiles, width: spanW * scale, height: spanH * scale }
}

/** Aspect ratio of a monitor's bounds (w/h) — the tile shape that signals
 *  orientation without an icon. Exported for tests/pins. */
export function boundsAspect(bounds: MonitorBounds): number {
  return bounds.w / bounds.h
}

/** Active-screen facts the panel + its notices both read (§B5/A4) — derived ONCE
 *  so the CTA rename, the per-screen header and the dynamic banners can never
 *  disagree about which monitor is live. A single-monitor host yields
 *  multiScreen === false, so every multi-screen affordance stays hidden (parity). */
export interface ActiveScreenFacts {
  activeScreen: MonitorLookDto | undefined
  activeIndex: number
  /** ≥2 real screens and NOT span — the per-screen affordances render. */
  multiScreen: boolean
  noReadableSource: boolean
  slideshowActive: boolean
  /** slideshow OR unreadable ⇒ an apply overwrites a live wallpaper (§A4 confirm). */
  liveWallpaper: boolean
}

export function activeScreenFacts(state: WallpaperStateDto | null): ActiveScreenFacts {
  const screens = state?.screens ?? []
  const activeIndex = state ? screens.findIndex((s) => s.monitorId === state.activeScreenId) : -1
  const activeScreen = activeIndex >= 0 ? screens[activeIndex] : undefined
  const multiScreen = screens.length >= 2 && !(state?.spanActive ?? false)
  const noReadableSource = !!activeScreen && !activeScreen.hasReadableSource
  const slideshowActive = !!activeScreen && activeScreen.slideshowActive
  return { activeScreen, activeIndex, multiScreen, noReadableSource, slideshowActive, liveWallpaper: noReadableSource || slideshowActive }
}
