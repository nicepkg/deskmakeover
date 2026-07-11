// M6 worker-pool message protocol. Defined here (not in the frozen
// `icon-compositor` worker) so it survives the pixel-module deletion in P4. The
// WASM render worker and the `IconCompositor` facade both speak it; the shapes
// match the frozen `render.worker.ts` contract exactly, so the WASM worker is a
// drop-in for the TS worker behind the dual-path flag.

import type { ConfigDto } from '@/bridge/types'

/** Per-icon inputs resolved outside the tile (cross-icon hue spread, kind).
 *  Mirror of the frozen `compose.ts` `RenderOpts`; the store's import moves here
 *  when the frozen modules are deleted (P4). */
export interface RenderOpts {
  /** Hue-spread-adjusted seed colour (hex). null/absent = derive from artwork. */
  fieldSeed?: string | null
  kindBucket?: 'App' | 'Folder' | 'File' | 'System' | null
}

export interface ArrowMsg {
  t: 'arrow'
  url: string
}
export interface SourceMsg {
  t: 'source'
  id: string
  url: string
}
export interface RenderMsg {
  t: 'render'
  req: number
  id: string
  config: ConfigDto
  isShortcut: boolean
  showOriginal: boolean
  size: number
  opts?: RenderOpts
}
export interface BakeMsg {
  t: 'bake'
  req: number
  id: string
  config: ConfigDto
  isShortcut: boolean
  opts?: RenderOpts
}
export type ToWorker = ArrowMsg | SourceMsg | RenderMsg | BakeMsg

export type FromWorker =
  /** `seed` = the artwork's dominant colour (hex) for the main-side hue spread;
   *  null for the no-hue tail. `url` echoes the request so the main thread can
   *  drop stale-generation acks (rescan URL swap). */
  | { t: 'sourceReady'; id: string; ok: boolean; seed: string | null; url: string }
  | { t: 'rendered'; req: number; id: string; key: string; bitmap: ImageBitmap }
  | { t: 'baked'; req: number; id: string; png: ArrayBuffer | null }

/** The tile slot a render belongs to — committed/original/hover renders of one
 *  item coexist. Latest-generation coalescing collapses to the newest render
 *  per slot. Matches `icon-renderer.ts` `slotKey`. */
export function slotKeyOf(id: string, showOriginal: boolean, size: number): string {
  return `${id}|${showOriginal}|${size}`
}
