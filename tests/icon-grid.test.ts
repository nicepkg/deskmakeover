import { describe, expect, test } from 'bun:test'
import { iconGrid } from '../src/lib/icons-assemble'
import type { GridMetricsDto } from '../src/bridge/types'

// The icons preview grid math (owner 2026-07-16; codex P2). A mirror tile centers its glyph in
// `cellWidth`, so the cell must reflect the TRUE observed pitch — a fabricated wider cell shifts
// every icon right of where Windows draws it.

const NO_OBS: GridMetricsDto = {
  screenWidth: 1920, screenHeight: 1080, taskbarHeight: 48,
  cellWidth: null, cellHeight: null, iconPx: null,
}
// A real Win11 medium-icon reading: ~75px horizontal pitch around a 48px icon.
const OBS: GridMetricsDto = {
  screenWidth: 1920, screenHeight: 1080, taskbarHeight: 48,
  cellWidth: 75, cellHeight: 97, iconPx: 48,
}

describe('iconGrid', () => {
  test('without observation, falls back to the iconPx + 44/48 approximation', () => {
    const g = iconGrid('Mid', NO_OBS)
    expect(g.cellWidth).toBe(48 + 44)
    expect(g.cellHeight).toBe(48 + 48)
  })

  test('for the observed size, the cell IS the observed pitch (no left-shift)', () => {
    const g = iconGrid('Mid', OBS)
    expect(g.iconPx).toBe(48)
    expect(g.cellWidth).toBe(75)
    expect(g.cellHeight).toBe(97)
    // The centered-glyph inset is (cell − icon) / 2 — the real Windows ~13.5px, not the
    // fabricated 22px the old 92px cell produced.
    expect((g.cellWidth - g.iconPx) / 2).toBeCloseTo(13.5, 5)
  })

  test('for a DIFFERENT size, preserves the observed absolute gutter (never scales it)', () => {
    // Observed gutter at 48px: 75 − 48 = 27 (w), 97 − 48 = 49 (h). Big icons (96px) must keep
    // that same absolute gutter, NOT a proportional 2× (which Windows never promises — codex P2).
    const big = iconGrid('Big', OBS)
    expect(big.iconPx).toBe(96)
    expect(big.cellWidth).toBe(96 + (75 - 48)) // 123, not 150
    expect(big.cellHeight).toBe(96 + (97 - 48)) // 145, not 194
  })

  test('never lets the cell fall narrower than the icon (freak reading guard)', () => {
    const weird: GridMetricsDto = { ...OBS, cellWidth: 50, cellHeight: 50, iconPx: 48 }
    const g = iconGrid('Big', weird) // gutter 50-48=2, icon 96 → 98
    expect(g.cellWidth).toBeGreaterThanOrEqual(g.iconPx)
    expect(g.cellHeight).toBeGreaterThanOrEqual(g.iconPx)
  })
})
