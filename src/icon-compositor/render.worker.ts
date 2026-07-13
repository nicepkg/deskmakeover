// FROZEN 2026-07-10 (ADR-0019): parity oracle for the Rust port. No new styles,
// no fixes except oracle corrections. Deleted after M6 certification.

import type { ConfigDto } from '@/bridge/types'
import type { Raster } from './raster'
import type { RenderOpts } from './compose'
import { renderTile } from './compose'
import { setNativeArrowRaster } from './marks'
import { iconProfile } from './profile'

// Render worker — one shard of the icon-compositor pool (spec 06 deps note:
// "a Web Worker pool for bake and full recomputes"). Holds its shard's 256px
// source rasters + their analysis caches (WeakMap-keyed on the raster, so it
// lives here with the raster), runs the pure renderTile, and transfers
// ImageBitmaps back — the main thread never runs pixel math for this shard.

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
  /** Per-icon Field inputs resolved main-side (hue spread) — WYSIWYG: the
   *  bake message carries the same values. */
  opts?: RenderOpts
}
export interface BakeMsg {
  t: 'bake'
  req: number
  id: string
  config: ConfigDto
  isShortcut: boolean
  /** Render the untouched original (compare-sheet Before) rather than the styled master.
   *  Default false — the apply path only ever bakes styled masters. */
  showOriginal?: boolean
  opts?: RenderOpts
}
export type ToWorker = ArrowMsg | SourceMsg | RenderMsg | BakeMsg

export type FromWorker =
  /** `seed` = the artwork's dominant colour (hex) for the main-side hue
   *  spread; null for the no-hue tail. `url` echoes the request so the main
   *  thread can drop stale-generation acks (rescan URL swap). */
  | { t: 'sourceReady'; id: string; ok: boolean; seed: string | null; url: string }
  | { t: 'rendered'; req: number; id: string; key: string; bitmap: ImageBitmap }
  | { t: 'baked'; req: number; id: string; png: ArrayBuffer | null }

const MASTER_SIZE = 256
const sources = new Map<string, Raster>()

const wctx = self as unknown as {
  onmessage: ((e: MessageEvent<ToWorker>) => void) | null
  postMessage(msg: FromWorker, transfer?: Transferable[]): void
}


function rasterToOffscreen(raster: Raster): OffscreenCanvas {
  const canvas = new OffscreenCanvas(raster.width, raster.height)
  canvas.getContext('2d')!.putImageData(new ImageData(raster.data as Uint8ClampedArray<ArrayBuffer>, raster.width, raster.height), 0, 0)
  return canvas
}

async function decodeToRaster(url: string, targetSize?: number): Promise<Raster | null> {
  try {
    const response = await fetch(url)
    const blob = await response.blob()
    const bitmap = await createImageBitmap(blob)
    const w = targetSize ?? bitmap.width
    const h = targetSize ?? bitmap.height
    const canvas = new OffscreenCanvas(w, h)
    const ctx = canvas.getContext('2d')!
    ctx.drawImage(bitmap, 0, 0, w, h)
    bitmap.close()
    const data = ctx.getImageData(0, 0, w, h)
    return { width: w, height: h, data: data.data }
  } catch {
    return null
  }
}

async function handle(msg: ToWorker): Promise<void> {
  if (msg.t === 'arrow') {
    setNativeArrowRaster(await decodeToRaster(msg.url))
    return
  }
  if (msg.t === 'source') {
    const raster = await decodeToRaster(msg.url, MASTER_SIZE)
    if (raster) sources.set(msg.id, raster)
    const colour = raster ? iconProfile(raster).subjectRimColour : null
    const seed = colour
      ? `#${[colour.r, colour.g, colour.b].map((v) => v.toString(16).padStart(2, '0')).join('').toUpperCase()}`
      : null
    wctx.postMessage({ t: 'sourceReady', id: msg.id, ok: raster !== null, seed, url: msg.url })
    return
  }
  if (msg.t === 'render') {
    const source = sources.get(msg.id)
    if (!source) return // source dropped/never loaded — main thread re-requests after sourceReady
    const raster = renderTile(source, msg.config, msg.isShortcut, msg.showOriginal, msg.size, msg.opts)
    const bitmap = rasterToOffscreen(raster).transferToImageBitmap()
    const key = `${msg.id}|${msg.showOriginal}|${msg.size}` // slot key; style detail is main-side
    wctx.postMessage({ t: 'rendered', req: msg.req, id: msg.id, key, bitmap }, [bitmap])
    return
  }
  if (msg.t === 'bake') {
    const source = sources.get(msg.id)
    if (!source) {
      wctx.postMessage({ t: 'baked', req: msg.req, id: msg.id, png: null })
      return
    }
    const raster = renderTile(source, msg.config, msg.isShortcut, msg.showOriginal ?? false, MASTER_SIZE, msg.opts)
    const blob = await rasterToOffscreen(raster).convertToBlob({ type: 'image/png' })
    const png = await blob.arrayBuffer()
    wctx.postMessage({ t: 'baked', req: msg.req, id: msg.id, png }, [png])
  }
}

// Strictly sequential dispatch: the boot-time 'arrow' message is guaranteed to
// finish decoding before the first render — a Keep-render must never cache the
// drawn fallback because the real badge was still in flight.
let chain: Promise<void> = Promise.resolve()
wctx.onmessage = (e: MessageEvent<ToWorker>) => {
  chain = chain.then(() => handle(e.data)).catch(() => {})
}
