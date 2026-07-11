// M6 WASM render worker — one shard of the icon-compositor pool, backed by a
// long-lived WASM `RenderSession` instead of the frozen TS `renderTile`. Speaks
// the exact `protocol.ts` message contract, so it is a drop-in for
// `render.worker.ts` behind the dual-path flag. The browser decodes source PNGs
// (free, off the certified path); the pixels come from `dm-icon-wasm`.
//
// Coalescing (perf doc opt #1): renders queue per tile slot and only the NEWEST
// render per slot is ever computed — a slider drag through K styles collapses to
// one compute per slot instead of K. Arrow + source + bake keep strict order
// relative to each other (a render needs its source and the arrow first); the
// arrow-ready gate blocks rendering until the genuine badge is installed, or a
// shortcut tile would bake the drawn fallback and break byte-parity.

import { slotKeyOf } from './protocol'
import type { BakeMsg, FromWorker, RenderMsg, ToWorker } from './protocol'
import { WasmIconRenderer } from './wasm-loader'

const MASTER_SIZE = 256

const wctx = self as unknown as {
  onmessage: ((e: MessageEvent<ToWorker>) => void) | null
  postMessage(msg: FromWorker, transfer?: Transferable[]): void
}

const rendererReady: Promise<WasmIconRenderer> = WasmIconRenderer.create()

let markArrowReady!: () => void
const arrowReady = new Promise<void>((resolve) => {
  markArrowReady = resolve
})

async function decodeToRgba(url: string, size?: number): Promise<{ data: Uint8ClampedArray; w: number; h: number } | null> {
  try {
    const blob = await (await fetch(url)).blob()
    const bitmap = await createImageBitmap(blob)
    const w = size ?? bitmap.width
    const h = size ?? bitmap.height
    const canvas = new OffscreenCanvas(w, h)
    const ctx = canvas.getContext('2d')!
    ctx.drawImage(bitmap, 0, 0, w, h)
    bitmap.close()
    return { data: ctx.getImageData(0, 0, w, h).data, w, h }
  } catch {
    return null
  }
}

function rgbaToBitmap(data: Uint8ClampedArray<ArrayBuffer>, size: number): ImageBitmap {
  const canvas = new OffscreenCanvas(size, size)
  canvas.getContext('2d')!.putImageData(new ImageData(data, size, size), 0, 0)
  return canvas.transferToImageBitmap()
}

// ---- ordered lane: arrow / source / bake keep strict FIFO order ----

async function handleArrow(url: string): Promise<void> {
  const renderer = await rendererReady
  const arrow = await decodeToRgba(url)
  if (arrow) renderer.setArrow(arrow.data, arrow.w, arrow.h)
  markArrowReady() // renders may proceed now (or on a null arrow: the drawn fallback, matching a no-badge host)
}

async function handleSource(id: string, url: string): Promise<void> {
  const renderer = await rendererReady
  const src = await decodeToRgba(url, MASTER_SIZE)
  let seed: string | null = null
  let ok = false
  if (src) {
    seed = renderer.registerSource(id, src.data)
    ok = true
  }
  wctx.postMessage({ t: 'sourceReady', id, ok, seed, url })
}

async function handleBake(msg: BakeMsg): Promise<void> {
  const renderer = await rendererReady
  await arrowReady
  if (!renderer.hasSource(msg.id)) {
    wctx.postMessage({ t: 'baked', req: msg.req, id: msg.id, png: null })
    return
  }
  const rgba = renderer.render(msg.id, msg.config, msg.isShortcut, false, MASTER_SIZE, msg.opts)
  if (!rgba) {
    wctx.postMessage({ t: 'baked', req: msg.req, id: msg.id, png: null })
    return
  }
  const canvas = new OffscreenCanvas(MASTER_SIZE, MASTER_SIZE)
  canvas.getContext('2d')!.putImageData(new ImageData(rgba, MASTER_SIZE, MASTER_SIZE), 0, 0)
  const png = await (await canvas.convertToBlob({ type: 'image/png' })).arrayBuffer()
  wctx.postMessage({ t: 'baked', req: msg.req, id: msg.id, png }, [png])
}

let orderedChain: Promise<void> = Promise.resolve()
function enqueueOrdered(run: () => Promise<void>): void {
  orderedChain = orderedChain.then(run).catch(() => {})
}

// ---- coalesced lane: newest render per slot wins ----

const pendingRenders = new Map<string, RenderMsg>()
let draining = false

function scheduleDrain(): void {
  if (draining) return
  draining = true
  queueMicrotask(drainRenders)
}

async function drainRenders(): Promise<void> {
  const renderer = await rendererReady
  await arrowReady
  // Snapshot the newest render per slot, then clear so any render that arrives
  // mid-drain re-queues (and supersedes) for the next pass.
  const batch = [...pendingRenders.values()]
  pendingRenders.clear()
  for (const msg of batch) {
    if (!renderer.hasSource(msg.id)) continue // not decoded yet — main re-requests after sourceReady
    const rgba = renderer.render(msg.id, msg.config, msg.isShortcut, msg.showOriginal, msg.size, msg.opts)
    if (!rgba) continue
    const bitmap = rgbaToBitmap(rgba, msg.size)
    const key = slotKeyOf(msg.id, msg.showOriginal, msg.size)
    wctx.postMessage({ t: 'rendered', req: msg.req, id: msg.id, key, bitmap }, [bitmap])
    await Promise.resolve() // yield between icons so a fast drag stays responsive
  }
  draining = false
  if (pendingRenders.size > 0) scheduleDrain()
}

wctx.onmessage = (e: MessageEvent<ToWorker>) => {
  const msg = e.data
  if (msg.t === 'render') {
    // Newest render for a slot supersedes any still-pending one — coalesce.
    pendingRenders.set(slotKeyOf(msg.id, msg.showOriginal, msg.size), msg)
    scheduleDrain()
    return
  }
  if (msg.t === 'arrow') {
    enqueueOrdered(() => handleArrow(msg.url))
    return
  }
  if (msg.t === 'source') {
    enqueueOrdered(() => handleSource(msg.id, msg.url))
    return
  }
  enqueueOrdered(() => handleBake(msg))
}
