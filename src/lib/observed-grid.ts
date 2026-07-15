import type { GridMetricsDto } from '@/bridge/types'

// The last icon scan's OBSERVED platform grid, shared across modules: the icons store records
// it, and the wallpaper zone lattice (monitor-reconcile `gridForBounds`) reads it so zones snap
// to the SAME real desktop cells the icons live on — not a second fabricated grid (owner report
// 2026-07-16: the preview/marker grids drifted from the real desktop). A dedicated module keeps
// stores/wallpaper.ts from importing stores/icons.ts (no store-to-store cycle).
let observed: GridMetricsDto | null = null

export function recordObservedGrid(metrics: GridMetricsDto): void {
  observed = metrics
}

/** The observed metrics, or null before the first scan / when the shell walk failed. */
export function observedGrid(): GridMetricsDto | null {
  return observed
}

/** Test-hygiene reset (the cache is a process-global): lets a suite assert the pre-scan
 *  fallback deterministically regardless of a prior test having recorded a scan. */
export function resetObservedGrid(): void {
  observed = null
}
