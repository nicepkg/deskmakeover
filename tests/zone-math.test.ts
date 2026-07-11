import { describe, expect, test } from 'bun:test'
import { clampZone, createFromDrag, firstFreeArea, ghostCells, halfFloor, halfRound, magnetizeMove, moveZone, nudgeZone, overlapRegions, rectsOverlap, resizeZone } from '../src/lib/zone-math'
import { ZONE_PRESETS, projectPreset } from '../src/lib/zone-presets'
import type { WallpaperGridInfoDto } from '../src/bridge/types'

// Mirrors the C# zone-editor fixtures: half-cell snapping, exclusive-edge resize,
// min 2×2, grid clamping.

describe('half-cell snapping', () => {
  test('floor and round snap to 0.5', () => {
    expect(halfFloor(3.74)).toBe(3.5)
    expect(halfFloor(3.49)).toBe(3)
    expect(halfRound(3.74)).toBe(3.5)
    expect(halfRound(3.76)).toBe(4)
  })
})

describe('createFromDrag', () => {
  test('spans snapped corners with a 2×2 minimum', () => {
    const z = createFromDrag({ cx: 1.2, cy: 0.9 }, { cx: 1.4, cy: 1.1 }, 20, 12)
    expect(z.cellsWide).toBe(2)
    expect(z.cellsTall).toBe(2)
    expect(z.cellX).toBe(1)
    expect(z.cellY).toBe(0.5)
  })

  test('normalizes reversed drags and clamps to the grid', () => {
    const z = createFromDrag({ cx: 19.9, cy: 11.8 }, { cx: 15.2, cy: 9.1 }, 20, 12)
    expect(z.cellX).toBe(15)
    expect(z.cellX + z.cellsWide).toBeLessThanOrEqual(20)
    expect(z.cellY + z.cellsTall).toBeLessThanOrEqual(12)
  })
})

describe('moveZone', () => {
  test('snaps to half cells and never leaves the grid', () => {
    const z = { cellX: 1, cellY: 1, cellsWide: 4, cellsTall: 4 }
    expect(moveZone(z, 0.26, 0, 20, 12).cellX).toBe(1.5)
    expect(moveZone(z, 100, 100, 20, 12)).toMatchObject({ cellX: 16, cellY: 8 })
    expect(moveZone(z, -100, -100, 20, 12)).toMatchObject({ cellX: 0, cellY: 0 })
  })
})

describe('resizeZone (exclusive edges)', () => {
  const z = { cellX: 4, cellY: 3, cellsWide: 6, cellsTall: 5 }

  test('east grows width only', () => {
    const r = resizeZone(z, 'e', 1.24, 0, 20, 12)
    expect(r).toMatchObject({ cellX: 4, cellsWide: 7 })
  })

  test('west moves x but keeps the right edge fixed', () => {
    const r = resizeZone(z, 'w', 1.0, 0, 20, 12)
    expect(r.cellX).toBe(5)
    expect(r.cellX + r.cellsWide).toBe(10)
  })

  test('never shrinks below 2×2', () => {
    const r = resizeZone(z, 'se', -100, -100, 20, 12)
    expect(r.cellsWide).toBe(2)
    expect(r.cellsTall).toBe(2)
  })

  test('nw corner respects both fixed edges', () => {
    const r = resizeZone(z, 'nw', -1, -1, 20, 12)
    expect(r.cellX + r.cellsWide).toBe(10)
    expect(r.cellY + r.cellsTall).toBe(8)
  })
})

describe('nudge + clamp', () => {
  test('nudges half a cell', () => {
    const z = { cellX: 2, cellY: 2, cellsWide: 3, cellsTall: 3 }
    expect(nudgeZone(z, 1, 0, 20, 12).cellX).toBe(2.5)
    expect(nudgeZone(z, 0, -1, 20, 12).cellY).toBe(1.5)
  })

  test('clampZone pulls persisted zones into a smaller grid', () => {
    const z = { cellX: 18, cellY: 10, cellsWide: 6, cellsTall: 6 }
    const r = clampZone(z, 20, 12)
    expect(r.cellX + r.cellsWide).toBeLessThanOrEqual(20)
    expect(r.cellY + r.cellsTall).toBeLessThanOrEqual(12)
  })
})

describe('ghostCells', () => {
  test('partial spread from row 1 (title chip overhangs — spec 04 v2.0), 3..12', () => {
    const cells = ghostCells({ cellX: 0.5, cellY: 0.5, cellsWide: 7, cellsTall: 12 })
    expect(cells.length).toBeGreaterThanOrEqual(3)
    expect(cells.length).toBeLessThanOrEqual(12)
    // Row 1 of the grid (the zone's first full row) is USABLE icon space again.
    expect(cells.some((c) => c.row === 1)).toBe(true)
    for (const c of cells) expect(c.row).toBeGreaterThanOrEqual(1)
  })

  test('degenerate zones yield nothing', () => {
    expect(ghostCells({ cellX: 0.2, cellY: 0.2, cellsWide: 0.5, cellsTall: 0.5 })).toEqual([])
  })

  test('reserveFirstRow shifts the spread down one row (in-panel chip lane)', () => {
    const zone = { cellX: 0, cellY: 0, cellsWide: 6, cellsTall: 6 }
    const normal = ghostCells(zone, false)
    const reserved = ghostCells(zone, true)
    expect(normal[0]?.row).toBe(0)
    expect(reserved[0]?.row).toBe(1)
    expect(reserved.every((c) => c.row >= 1)).toBe(true)
  })
})

describe('magnetism + guides (spec 04 v2.0 §3)', () => {
  const other = { cellX: 6, cellY: 1, cellsWide: 4, cellsTall: 4 }

  test('adjacent tiling: my right edge snaps to their left within 0.35 cells', () => {
    const raw = { cellX: 1.8, cellY: 1.2, cellsWide: 4, cellsTall: 4 } // right = 5.8, their left = 6
    const m = magnetizeMove(raw, [other], 20, 12)
    expect(m.fired.x).toBe(true)
    expect(m.rect.cellX + m.rect.cellsWide).toBe(6)
    expect(m.guides.some((g) => g.axis === 'x' && g.at === 6)).toBe(true)
  })

  test('same-edge alignment: tops align', () => {
    const raw = { cellX: 12, cellY: 1.3, cellsWide: 4, cellsTall: 4 }
    const m = magnetizeMove(raw, [other], 20, 12)
    expect(m.fired.y).toBe(true)
    expect(m.rect.cellY).toBeCloseTo(1, 9)
  })

  test('beyond the window nothing fires', () => {
    const raw = { cellX: 1.0, cellY: 8, cellsWide: 4, cellsTall: 3 } // right=5, gap 1.0 > 0.35
    const m = magnetizeMove(raw, [other], 20, 12)
    expect(m.fired).toEqual({ x: false, y: false })
    expect(m.rect).toEqual(raw)
  })

  test('overlapRegions returns the intersection', () => {
    const moving = { cellX: 4, cellY: 2, cellsWide: 4, cellsTall: 4 }
    const regions = overlapRegions(moving, [other])
    expect(regions).toHaveLength(1)
    expect(regions[0]).toEqual({ cellX: 6, cellY: 2, cellsWide: 2, cellsTall: 3 })
  })

  test('no overlap → no regions', () => {
    expect(overlapRegions({ cellX: 0, cellY: 0, cellsWide: 2, cellsTall: 2 }, [other])).toEqual([])
  })
})

describe('firstFreeArea (+ 添加分区 placement)', () => {
  const grid = { columns: 20, rows: 11 }

  test('empty grid → a 6×4 area at the half-cell origin', () => {
    expect(firstFreeArea(grid, [], 6, 4)).toMatchObject({ cellX: 0.5, cellY: 0.5, cellsWide: 6, cellsTall: 4 })
  })

  test('returns a rect that overlaps NO existing zone and fits in-bounds', () => {
    const zones = [
      { cellX: 0.5, cellY: 0.5, cellsWide: 6, cellsTall: 4 },
      { cellX: 0.5, cellY: 4.5, cellsWide: 6, cellsTall: 4 },
    ]
    const r = firstFreeArea(grid, zones, 6, 4)
    for (const z of zones) expect(rectsOverlap(r, z)).toBe(false)
    expect(r.cellX + r.cellsWide).toBeLessThanOrEqual(grid.columns)
    expect(r.cellY + r.cellsTall).toBeLessThanOrEqual(grid.rows)
  })

  test('cascades a bounded, valid rect when the grid is full', () => {
    const full = [{ cellX: 0, cellY: 0, cellsWide: 20, cellsTall: 11 }]
    const r = firstFreeArea(grid, full, 6, 4)
    expect(r.cellsWide).toBe(6)
    expect(r.cellsTall).toBe(4)
    expect(r.cellX + r.cellsWide).toBeLessThanOrEqual(grid.columns)
    expect(r.cellY + r.cellsTall).toBeLessThanOrEqual(grid.rows)
  })

  test('rectsOverlap treats touching edges as non-overlapping', () => {
    expect(rectsOverlap({ cellX: 0, cellY: 0, cellsWide: 4, cellsTall: 4 }, { cellX: 4, cellY: 0, cellsWide: 4, cellsTall: 4 })).toBe(false)
    expect(rectsOverlap({ cellX: 0, cellY: 0, cellsWide: 4, cellsTall: 4 }, { cellX: 3.5, cellY: 0, cellsWide: 4, cellsTall: 4 })).toBe(true)
  })
})

describe('projectPreset — zone seams never collapse (owner 2026-07-09)', () => {
  const grid = (columns: number, rows: number) =>
    ({ screenWidth: 1920, screenHeight: 1080, taskbarHeight: 48, iconPx: 48,
       cellWidth: 60, cellHeight: 60, inset: 0, columns, rows }) as WallpaperGridInfoDto

  // Sweep each preset ONLY over grid shapes of its own orientation — a portrait
  // layout is never shown on a wide-short grid (the picker filters by orientation
  // 2026-07-12), so asserting it there is meaningless. Landscape = wide/short,
  // portrait = narrow/tall; both cover the realistic screen range.
  const SWEEPS = {
    landscape: { cols: [16, 34, 2], rows: [7, 18, 1] },
    portrait: { cols: [9, 14, 1], rows: [16, 30, 2] },
  } as const

  test('every preset keeps a >= 1-cell gap between neighbours at every grid size in its orientation', () => {
    for (const preset of ZONE_PRESETS) {
      const s = SWEEPS[preset.orientation]
      for (let rows = s.rows[0]; rows <= s.rows[1]; rows += s.rows[2]) {
        for (let cols = s.cols[0]; cols <= s.cols[1]; cols += s.cols[2]) {
          const zs = projectPreset(preset, grid(cols, rows))
          for (const a of zs) {
            for (const b of zs) {
              if (a === b) continue
              const xOv = a.cellX < b.cellX + b.cellsWide && b.cellX < a.cellX + a.cellsWide
              const yOv = a.cellY < b.cellY + b.cellsTall && b.cellY < a.cellY + a.cellsTall
              expect(xOv && yOv, `${preset.id} overlap @${cols}x${rows}`).toBe(false)
              if (xOv && a.cellY < b.cellY) {
                expect(b.cellY - (a.cellY + a.cellsTall), `${preset.id} vgap @${cols}x${rows}`).toBeGreaterThanOrEqual(1)
              }
              if (yOv && a.cellX < b.cellX) {
                expect(b.cellX - (a.cellX + a.cellsWide), `${preset.id} hgap @${cols}x${rows}`).toBeGreaterThanOrEqual(1)
              }
            }
          }
        }
      }
    }
  })
})
