import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { activeScreenFacts, activeScreenSourceUrl, arrangeTiles, boundsAspect, primaryScreenId, shouldShowSwitcher } from '../src/lib/screen-arrange'
import { pickFitMode } from '../src/lib/canvas-view'
import type { ScreenLook } from '../src/lib/monitor-reconcile'
import type { MonitorBounds, MonitorLookDto, WallpaperStateDto } from '../src/bridge/types'

// Task 2 pure UI logic (spec 04 §B4/A2/B6): the switcher's render-gating + OS
// arrangement geometry and the portrait fit-mode pick. Kept DOM-free so the
// visual acceptance layer only has to judge pixels, not correctness.

const LANDSCAPE: MonitorBounds = { x: 0, y: 0, w: 1920, h: 1080 }
const PORTRAIT: MonitorBounds = { x: 1920, y: 0, w: 1080, h: 1920 }
const THIRD: MonitorBounds = { x: 3000, y: 0, w: 2560, h: 1440 }

const s = (monitorId: string, bounds: MonitorBounds) => ({ monitorId, bounds })
const near = (a: number, b: number, eps = 0.01) => Math.abs(a - b) <= eps

describe('shouldShowSwitcher (gating + span degrade §B6)', () => {
  test('single monitor renders nothing (parity)', () => {
    expect(shouldShowSwitcher(1, false)).toBe(false)
  })
  test('two or more monitors show the switcher', () => {
    expect(shouldShowSwitcher(2, false)).toBe(true)
    expect(shouldShowSwitcher(3, false)).toBe(true)
  })
  test('span mode hides the switcher even with multiple monitors', () => {
    expect(shouldShowSwitcher(2, true)).toBe(false)
    expect(shouldShowSwitcher(3, true)).toBe(false)
  })
  test('zero monitors never shows', () => {
    expect(shouldShowSwitcher(0, false)).toBe(false)
  })
})

describe('primaryScreenId (origin-anchored)', () => {
  test('picks the monitor at the virtual-desktop origin', () => {
    expect(primaryScreenId([s('a', LANDSCAPE), s('b', PORTRAIT)])).toBe('a')
  })
  test('honors origin even when it is not first', () => {
    expect(primaryScreenId([s('left', { x: -1920, y: 0, w: 1920, h: 1080 }), s('main', LANDSCAPE)])).toBe('main')
  })
  test('falls back to the first screen when none is at the origin', () => {
    expect(primaryScreenId([s('x', { x: 100, y: 100, w: 800, h: 600 })])).toBe('x')
  })
  test('empty set → null', () => {
    expect(primaryScreenId([])).toBeNull()
  })
})

describe('arrangeTiles (OS arrangement, aspect + relative position)', () => {
  const { tiles, width, height } = arrangeTiles([s('main', LANDSCAPE), s('port', PORTRAIT)], 140, 84)

  test('one tile per screen, in order', () => {
    expect(tiles.map((t) => t.monitorId)).toEqual(['main', 'port'])
    expect(tiles.map((t) => t.index)).toEqual([0, 1])
  })
  test('each tile keeps its real aspect (orientation = shape)', () => {
    expect(near(tiles[0].width / tiles[0].height, boundsAspect(LANDSCAPE))).toBe(true)
    expect(near(tiles[1].width / tiles[1].height, boundsAspect(PORTRAIT))).toBe(true)
    // portrait tile is taller than wide, landscape the reverse
    expect(tiles[0].width).toBeGreaterThan(tiles[0].height)
    expect(tiles[1].height).toBeGreaterThan(tiles[1].width)
  })
  test('tiles are positioned relative to each other per bounds', () => {
    expect(near(tiles[0].left, 0)).toBe(true)
    expect(near(tiles[0].top, 0)).toBe(true)
    // portrait sits immediately to the RIGHT of the landscape (its left == landscape width)
    expect(near(tiles[1].left, tiles[0].width)).toBe(true)
    expect(near(tiles[1].top, 0)).toBe(true)
  })
  test('arrangement box fits within the budget, constrained by height here', () => {
    expect(width).toBeLessThanOrEqual(140 + 0.01)
    expect(near(height, 84)).toBe(true)
  })
  test('primary flag rides the origin monitor', () => {
    expect(tiles[0].isPrimary).toBe(true)
    expect(tiles[1].isPrimary).toBe(false)
  })
  test('three-monitor layout preserves span order + right-most edge', () => {
    const { tiles: t3, width: w3 } = arrangeTiles([s('a', LANDSCAPE), s('b', PORTRAIT), s('c', THIRD)], 160, 84)
    expect(t3.map((t) => t.monitorId)).toEqual(['a', 'b', 'c'])
    // c starts at x=3000, so its scaled left equals the portrait's right edge
    expect(near(t3[2].left, t3[1].left + t3[1].width)).toBe(true)
    expect(w3).toBeLessThanOrEqual(160 + 0.01)
  })
  test('empty set → empty arrangement (never crashes)', () => {
    expect(arrangeTiles([], 140, 84)).toEqual({ tiles: [], width: 0, height: 0 })
  })
})

// Regression guard for the trickiest wiring (Task 1 flagged it): the single-instance
// compositor re-inits ONLY on grid-dims change, so switching between two SAME-dims
// screens must swap the source EXPLICITLY or the canvas keeps the wrong screen's
// wallpaper. And the swap must read the store mirror, NOT wallpaper.getSource (which
// returns the HOST's active screen — the client-only selectScreen never syncs it).
describe('compositor source-swap seam (§B2/B4)', () => {
  const SRC = readFileSync(
    join(import.meta.dir, '..', 'src', 'components', 'canvas', 'use-wallpaper-compositor.ts'),
    'utf8',
  )
  test('an effect keyed on activeScreenId swaps the compositor source', () => {
    expect(SRC).toContain('activeScreenId')
    expect(SRC).toContain('setSource')
    expect(SRC).toMatch(/\}, \[activeScreenId\]\)/)
  })
  test('resolves the source from the store mirror, not wallpaper.getSource', () => {
    expect(SRC).toContain('s.sourceUrl')
    expect(SRC).not.toContain("'wallpaper.getSource'")
  })
})

describe('activeScreenFacts (CTA/header/banner truth §B5/A4)', () => {
  const scr = (monitorId: string, extra: Partial<MonitorLookDto> = {}): MonitorLookDto =>
    ({ monitorId, hasReadableSource: true, slideshowActive: false, orientation: 'landscape', ...extra }) as unknown as MonitorLookDto
  const st = (screens: MonitorLookDto[], activeScreenId: string, spanActive = false): WallpaperStateDto =>
    ({ screens, activeScreenId, spanActive }) as unknown as WallpaperStateDto

  test('null state is inert (no multi-screen affordance)', () => {
    const f = activeScreenFacts(null)
    expect(f.multiScreen).toBe(false)
    expect(f.activeIndex).toBe(-1)
    expect(f.activeScreen).toBeUndefined()
    expect(f.liveWallpaper).toBe(false)
  })
  test('single monitor → multiScreen false (parity)', () => {
    expect(activeScreenFacts(st([scr('a')], 'a')).multiScreen).toBe(false)
  })
  test('two normal monitors → multiScreen true, correct active index', () => {
    const f = activeScreenFacts(st([scr('a'), scr('b')], 'b'))
    expect(f.multiScreen).toBe(true)
    expect(f.activeIndex).toBe(1)
    expect(f.liveWallpaper).toBe(false)
  })
  test('active slideshow screen ⇒ liveWallpaper (confirm before apply)', () => {
    const f = activeScreenFacts(st([scr('a'), scr('b', { slideshowActive: true })], 'b'))
    expect(f.slideshowActive).toBe(true)
    expect(f.liveWallpaper).toBe(true)
  })
  test('active unreadable-source screen ⇒ liveWallpaper + noReadableSource', () => {
    const f = activeScreenFacts(st([scr('a'), scr('b', { hasReadableSource: false })], 'b'))
    expect(f.noReadableSource).toBe(true)
    expect(f.liveWallpaper).toBe(true)
  })
  test('span degrades multiScreen to false even with two monitors', () => {
    expect(activeScreenFacts(st([scr('a'), scr('b')], 'a', true)).multiScreen).toBe(false)
  })
})

describe('activeScreenSourceUrl (resetSource per-screen fix §B2)', () => {
  const scr = (url: string | null): ScreenLook =>
    ({ source: url ? { url, width: 100, height: 100 } : null }) as unknown as ScreenLook
  const screens: Record<string, ScreenLook> = {
    primary: scr('scene://primary'),
    portrait: scr('scene://portrait'),
  }
  test('resolves the ACTIVE screen’s OWN source (the bug: getSource returned primary)', () => {
    // resetSource used wallpaper.getSource → the HOST's active screen (primary) → the
    // wrong monitor's wallpaper when a non-primary screen was active. The fix reads
    // the per-screen map: a portrait-active reset restores PORTRAIT's source.
    expect(activeScreenSourceUrl(screens, 'portrait')).toBe('scene://portrait')
    expect(activeScreenSourceUrl(screens, 'primary')).toBe('scene://primary')
  })
  test('a dynamic screen (no readable source) → null (nothing to restore to)', () => {
    expect(activeScreenSourceUrl({ dyn: scr(null) }, 'dyn')).toBeNull()
  })
  test('null / unknown active screen → null', () => {
    expect(activeScreenSourceUrl(screens, null)).toBeNull()
    expect(activeScreenSourceUrl(screens, 'ghost')).toBeNull()
  })
})

describe('pickFitMode (portrait fit-to-height §A2)', () => {
  test('landscape opens letterboxed whole', () => {
    expect(pickFitMode(1920, 1080)).toBe('all')
  })
  test('portrait opens filling the viewport vertically', () => {
    expect(pickFitMode(1080, 1920)).toBe('height')
  })
  test('ultrawide (≥21:9) stays fit-all, never fit-width', () => {
    expect(pickFitMode(3440, 1440)).toBe('all')
    expect(pickFitMode(5120, 1440)).toBe('all')
  })
  test('square is treated as landscape (fit-all)', () => {
    expect(pickFitMode(1000, 1000)).toBe('all')
  })
})
