import type { WallpaperCompositor } from './renderer'

// The mirror owns the compositor's lifecycle (it owns the canvas); the store
// needs it at apply time (bake). This tiny registry is the seam — no store↔
// component import cycle, trivially mockable in tests.

let active: WallpaperCompositor | null = null

export function registerCompositor(c: WallpaperCompositor | null): void {
  active = c
}

export function getCompositor(): WallpaperCompositor | null {
  return active
}
