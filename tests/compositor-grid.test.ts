import { describe, expect, test } from 'bun:test'
import type { WallpaperGridInfoDto } from '../src/bridge/types'
import { WallpaperCompositor } from '../src/compositor/renderer'

// Regression (owner report 2026-07-16, 选中框间距不等): the compositor is created
// once per screen-dims and captured the BOOT grid forever — when the icon scan
// re-recorded the true cell pitch (store regridScreens), the DOM zone overlay
// moved to the fresh lattice while the compositor kept painting panels on the
// stale one; the per-row error accumulated toward the bottom. setGrid adopts a
// same-dims lattice refresh (a dims change recreates the whole compositor).

const BOOT: WallpaperGridInfoDto = {
  screenWidth: 2560, screenHeight: 1440, taskbarHeight: 48, iconPx: 48,
  cellWidth: 92, cellHeight: 92, inset: 14, columns: 27, rows: 15,
}

function bare(grid: WallpaperGridInfoDto): { c: WallpaperCompositor; invalidated: () => number } {
  const c = Object.create(WallpaperCompositor.prototype) as WallpaperCompositor
  let n = 0
  ;(c as unknown as { grid: WallpaperGridInfoDto }).grid = grid
  ;(c as unknown as { invalidate: () => void }).invalidate = () => { n++ }
  return { c, invalidated: () => n }
}

describe('WallpaperCompositor.setGrid (same-dims lattice refresh)', () => {
  test('adopts a re-recorded cell pitch and repaints', () => {
    const { c, invalidated } = bare({ ...BOOT })
    const observed = { ...BOOT, cellWidth: 101, cellHeight: 102, inset: 12, columns: 25, rows: 13 }
    c.setGrid(observed)
    expect((c as unknown as { grid: WallpaperGridInfoDto }).grid).toEqual(observed)
    expect(invalidated()).toBe(1)
  })

  test('an identical lattice is a no-op (no repaint churn)', () => {
    const { c, invalidated } = bare({ ...BOOT })
    c.setGrid({ ...BOOT })
    expect(invalidated()).toBe(0)
  })

  test('a DIMS change is refused — the hook recreates the compositor instead', () => {
    const { c, invalidated } = bare({ ...BOOT })
    c.setGrid({ ...BOOT, screenWidth: 1920, screenHeight: 1080, cellHeight: 101 })
    expect((c as unknown as { grid: WallpaperGridInfoDto }).grid).toEqual(BOOT)
    expect(invalidated()).toBe(0)
  })
})
