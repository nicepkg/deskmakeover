// FROZEN 2026-07-10 (ADR-0019): parity oracle for the Rust port. No new styles,
// no fixes except oracle corrections. Deleted after M6 certification.

import type { ConfigDto } from '@/bridge/types'
import winNativeArrow from '@/assets/win-native-arrow.png'
import type { Raster } from './raster'
import { makeRaster } from './raster'
import type { RenderOpts } from './compose'
import { renderTile } from './compose'
import { iconProfile } from './profile'
import type { FromWorker, ToWorker } from './render.worker'

// The icon-compositor facade: a Web Worker POOL renders tiles off the main
// thread (spec 06 deps note; owner perf order 2026-07-09). Items shard across
// workers by stable id hash — each worker holds only its shard's sources and
// analysis caches, so nothing is cloned N times. Results come back as
// zero-copy ImageBitmaps; the main thread only ever drawImage()s. A newer
// request for the same tile slot SUPERSEDES the older one (stale responses
// are dropped, not painted). Falls back to synchronous in-thread rendering
// where Worker/OffscreenCanvas are unavailable (bun tests).

/** The style inputs that change a rendered tile — the cache key. */
export function tileStyleKey(config: ConfigDto, isShortcut: boolean, showOriginal: boolean, size: number): string {
  if (showOriginal) return `orig|${isShortcut}|${size}`
  return [
    config.shape, config.subject, config.tint, config.monoStyle, config.plateBand, config.plateFallback, config.shortcutShape ?? '',
    config.distinction, config.markStyle, config.markColor ?? '-', config.plateColor ?? '-', config.filter,
    isShortcut, size,
  ].join('|')
}

/** A tile slot: committed/original/hover renders of one item live side by side. */
function slotKey(id: string, showOriginal: boolean, size: number): string {
  return `${id}|${showOriginal}|${size}`
}

const MASTER_SIZE = 256

/**
 * Style-level LRU (owner order 2026-07-09): the cache is keyed by the FULL
 * style key (config axes + size + original flag) and holds the whole
 * desktop's bitmaps for each of the last N styles — so re-hovering or
 * switching back to a recent style blits instantly, and memory stays bounded
 * (N styles × items × ~37KB at 96px ≈ 90MB worst case at the default cap;
 * evicted ImageBitmaps are close()d, returning their backing memory).
 */
const STYLE_LRU_CAPACITY = 20

const workersSupported =
  typeof Worker !== 'undefined' && typeof OffscreenCanvas !== 'undefined' && typeof createImageBitmap !== 'undefined'

export class IconCompositor {
  // ---- worker pool ----
  private workers: Worker[] = []
  private poolSize = 0
  private reqSeq = 0
  /** slotKey → the styleKey we currently WANT for that slot (supersede gate). */
  private wanted = new Map<string, string>()
  /** req id → {slot, styleKey, epoch} of in-flight renders. */
  private inflight = new Map<number, { slot: string; styleKey: string; epoch: number }>()
  private bakeWaiters = new Map<number, (png: ArrayBuffer | null) => void>()
  private sourceWaiters = new Map<string, Array<(ok: boolean) => void>>()

  // ---- shared state ----
  /** id → url of the ack'd (decoded) source. */
  private ready = new Map<string, string>()
  /** id → dominant-colour seed hex (null = no-hue tail) from decode. */
  private seeds = new Map<string, string | null>()
  /** styleKey (epoch-prefixed) → per-item bitmaps. Map iteration order = LRU. */
  private styleLru = new Map<string, Map<string, CanvasImageSource>>()
  private epoch = 0
  private onReady: (() => void) | null = null
  private notifyScheduled = false

  // ---- sync fallback (no Worker: bun tests) ----
  private localSources = new Map<string, Raster>()

  constructor() {
    if (workersSupported) {
      this.poolSize = Math.min(6, Math.max(2, (navigator.hardwareConcurrency || 4) - 2))
      for (let i = 0; i < this.poolSize; i++) {
        const worker = new Worker(new URL('./render.worker.ts', import.meta.url), { type: 'module' })
        worker.onmessage = (e: MessageEvent<FromWorker>) => this.onWorkerMessage(e.data)
        // FIRST message: the real Win11 shortcut-arrow badge. The worker's
        // sequential dispatch guarantees it lands before any render.
        worker.postMessage({ t: 'arrow', url: new URL(winNativeArrow, location.origin).href } satisfies ToWorker)
        this.workers.push(worker)
      }
    }
  }

  /** Register the single repaint listener (the store bumps renderTick). */
  setOnReady(cb: () => void): void {
    this.onReady = cb
  }

  private notify(): void {
    if (this.notifyScheduled || !this.onReady) return
    this.notifyScheduled = true
    requestAnimationFrame(() => {
      this.notifyScheduled = false
      this.onReady?.()
    })
  }

  private workerFor(id: string): Worker {
    let h = 0
    for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) | 0
    return this.workers[Math.abs(h) % this.poolSize]
  }

  private post(id: string, msg: ToWorker): void {
    this.workerFor(id).postMessage(msg)
  }

  private onWorkerMessage(msg: FromWorker): void {
    if (msg.t === 'sourceReady') {
      // Generation gate (codex #4): a rescan can swap the URL for an id while
      // the old decode is in flight — only the ack matching the CURRENT url
      // may resolve waiters or write the seed; stale acks are dropped (the
      // newer load's own ack follows on the same worker's sequential chain).
      if (this.ready.get(msg.id) !== msg.url) return
      const waiters = this.sourceWaiters.get(msg.id) ?? []
      this.sourceWaiters.delete(msg.id)
      if (!msg.ok) {
        this.ready.delete(msg.id)
        this.seeds.delete(msg.id) // never let a dead source keep voting (codex #6)
      } else {
        this.seeds.set(msg.id, msg.seed)
      }
      for (const w of waiters) w(msg.ok)
      this.notify()
      return
    }
    if (msg.t === 'rendered') {
      const flight = this.inflight.get(msg.req)
      this.inflight.delete(msg.req)
      if (!flight) return
      // Supersede gate: paint only if this is still the style the slot wants,
      // AND the render was dispatched in the CURRENT epoch — a pre-invalidation
      // render must never be cached under the new epoch's key (codex #5).
      if (flight.epoch !== this.epoch || this.wanted.get(flight.slot) !== flight.styleKey) {
        ;(msg.bitmap as ImageBitmap).close()
        return
      }
      this.storeInCache(msg.id, `${flight.epoch}|${flight.styleKey}`, msg.bitmap)
      this.notify()
      return
    }
    if (msg.t === 'baked') {
      const waiter = this.bakeWaiters.get(msg.req)
      this.bakeWaiters.delete(msg.req)
      waiter?.(msg.png)
    }
  }

  private storeInCache(id: string, fullKey: string, image: CanvasImageSource): void {
    let perItem = this.styleLru.get(fullKey)
    if (!perItem) {
      perItem = new Map()
      this.styleLru.set(fullKey, perItem)
      while (this.styleLru.size > STYLE_LRU_CAPACITY) {
        const oldest = this.styleLru.keys().next().value as string
        closeAll(this.styleLru.get(oldest)!)
        this.styleLru.delete(oldest)
      }
    }
    const prev = perItem.get(id)
    if (prev && typeof ImageBitmap !== 'undefined' && prev instanceof ImageBitmap) prev.close()
    perItem.set(id, image)
  }

  /** LRU touch: re-insert the style at the tail (most recently used). */
  private touchStyle(fullKey: string): Map<string, CanvasImageSource> | undefined {
    const perItem = this.styleLru.get(fullKey)
    if (perItem) {
      this.styleLru.delete(fullKey)
      this.styleLru.set(fullKey, perItem)
    }
    return perItem
  }

  /** Decode a source (worker-side fetch+decode). Resolves when usable. */
  async loadSource(id: string, url: string): Promise<void> {
    if (this.ready.get(id) === url) return
    if (!workersSupported) {
      await this.loadSourceLocal(id, url)
      return
    }
    // Join an in-flight decode only when it is for the SAME url; a new url
    // always dispatches its own source message (codex #4 — the old code let a
    // rescan's new url silently ride the outgoing decode's waiter list).
    const sameInFlight = this.ready.get(id) === url && this.sourceWaiters.has(id)
    this.ready.set(id, url) // claim; the ack gate checks against this value
    await new Promise<boolean>((resolve) => {
      const waiters = this.sourceWaiters.get(id)
      if (waiters) waiters.push(resolve)
      else this.sourceWaiters.set(id, [resolve])
      if (!sameInFlight) this.post(id, { t: 'source', id, url })
    }).then((ok) => {
      if (!ok) throw new Error(`source failed: ${id}`)
    })
  }

  hasSource(id: string, url: string): boolean {
    return this.ready.get(id) === url && !this.sourceWaiters.has(id)
  }

  /** Drop every cached render (rescan / config-independent invalidation). */
  invalidateAll(): void {
    for (const perItem of this.styleLru.values()) closeAll(perItem)
    this.styleLru.clear()
    this.wanted.clear()
    this.epoch++
  }

  /**
   * Cached tile for these exact inputs, or null. On a miss the render is
   * dispatched to the item's shard worker; the onReady listener fires when
   * ANY tile lands (rAF-coalesced) and callers re-pull.
   */
  getTile(
    id: string,
    config: ConfigDto,
    isShortcut: boolean,
    showOriginal: boolean,
    size: number,
    opts?: RenderOpts,
  ): CanvasImageSource | null {
    const styleKey = tileStyleKey(config, isShortcut, showOriginal, size)
    const fullKey = `${this.epoch}|${styleKey}`
    const hit = this.touchStyle(fullKey)?.get(id)
    if (hit) return hit

    if (!workersSupported) {
      const source = this.localSources.get(id)
      if (!source) return null
      const raster = renderTile(source, config, isShortcut, showOriginal, size, opts)
      const canvas = rasterToCanvas(raster)
      this.storeInCache(id, fullKey, canvas)
      return canvas
    }

    if (!this.ready.has(id)) return null
    const slot = slotKey(id, showOriginal, size)
    if (this.wanted.get(slot) === styleKey) return null // already in flight/queued
    this.wanted.set(slot, styleKey)
    const req = ++this.reqSeq
    this.inflight.set(req, { slot, styleKey, epoch: this.epoch })
    this.post(id, { t: 'render', req, id, config, isShortcut, showOriginal, size, opts })
    return null
  }

  /** The decode-time dominant-colour seed (hex) for the hue-spread pass;
   *  null = no-hue tail, undefined = source not decoded yet. */
  seedOf(id: string): string | null | undefined {
    if (!workersSupported) {
      const source = this.localSources.get(id)
      if (!source) return undefined
      const colour = iconProfile(source).subjectRimColour
      if (!colour) return null
      const h = (v: number) => v.toString(16).padStart(2, '0')
      return `#${h(colour.r)}${h(colour.g)}${h(colour.b)}`.toUpperCase()
    }
    return this.seeds.get(id)
  }

  /** The 256px bake master as PNG base64 — the ONLY size the bridge accepts. */
  async bakeMasterPng(id: string, config: ConfigDto, isShortcut: boolean, opts?: RenderOpts): Promise<string | null> {
    if (!workersSupported) {
      const source = this.localSources.get(id)
      if (!source) return null
      const raster = renderTile(source, config, isShortcut, false, MASTER_SIZE, opts)
      return canvasToPngBase64(rasterToCanvas(raster))
    }
    if (!this.ready.has(id)) return null
    const req = ++this.reqSeq
    const png = await new Promise<ArrayBuffer | null>((resolve) => {
      this.bakeWaiters.set(req, resolve)
      this.post(id, { t: 'bake', req, id, config, isShortcut, opts })
    })
    if (!png) return null
    let binary = ''
    const bytes = new Uint8Array(png)
    for (let i = 0; i < bytes.length; i += 0x8000) {
      binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
    }
    return btoa(binary)
  }

  /** Outstanding tile renders in flight. 0 (with the store's sourcesLoading also
   *  0) = a fully-painted desktop → the store lifts the first-paint veil. Also
   *  used as a visual-acceptance probe. */
  pendingCount(): number {
    return this.inflight.size
  }

  // ---- sync fallback internals ----

  private async loadSourceLocal(id: string, url: string): Promise<void> {
    if (this.ready.get(id) === url) return
    const response = await fetch(url)
    const blob = await response.blob()
    const bitmap = await createImageBitmap(blob)
    const canvas = document.createElement('canvas')
    canvas.width = MASTER_SIZE
    canvas.height = MASTER_SIZE
    const ctx = canvas.getContext('2d')!
    ctx.drawImage(bitmap, 0, 0, MASTER_SIZE, MASTER_SIZE)
    bitmap.close()
    const data = ctx.getImageData(0, 0, MASTER_SIZE, MASTER_SIZE)
    this.localSources.set(id, { width: MASTER_SIZE, height: MASTER_SIZE, data: data.data })
    this.ready.set(id, url)
  }
}

function closeAll(perItem: Map<string, CanvasImageSource>): void {
  if (typeof ImageBitmap === 'undefined') return
  for (const image of perItem.values()) {
    if (image instanceof ImageBitmap) image.close()
  }
}

function rasterToCanvas(raster: Raster): HTMLCanvasElement {
  const canvas = document.createElement('canvas')
  canvas.width = raster.width
  canvas.height = raster.height
  canvas.getContext('2d')!.putImageData(new ImageData(raster.data as Uint8ClampedArray<ArrayBuffer>, raster.width, raster.height), 0, 0)
  return canvas
}

async function canvasToPngBase64(canvas: HTMLCanvasElement): Promise<string | null> {
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'))
  if (!blob) return null
  const buffer = await blob.arrayBuffer()
  let binary = ''
  const bytes = new Uint8Array(buffer)
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
  }
  return btoa(binary)
}

/** Test seam: a solid synthetic source raster (mirrors C# test canvases). */
export function solidSource(size: number, r: number, g: number, b: number, a = 255): Raster {
  const raster = makeRaster(size)
  for (let i = 0; i < raster.data.length; i += 4) {
    raster.data[i] = r
    raster.data[i + 1] = g
    raster.data[i + 2] = b
    raster.data[i + 3] = a
  }
  return raster
}

// One shared compositor per web session; the store and the mirror share it.
let active: IconCompositor | null = null

export function getIconCompositor(): IconCompositor {
  if (!active) active = new IconCompositor()
  return active
}
