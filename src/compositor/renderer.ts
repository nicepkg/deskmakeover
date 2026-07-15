import { Application, BlurFilter, Container, Matrix, RenderTexture, Sprite, Texture } from 'pixi.js'
import type { LookDto, WallpaperGridInfoDto, ZoneDto } from '@/bridge/types'
import { zonePaint } from './material'
import { buildSampleBuffer, resolveTone, sampleRegion } from './sampling'
import type { SampleBuffer } from './sampling'
import { clarityCanvas } from './clarity'
import { coverFit } from './cover-fit'
import { ZoneNode } from './zone-node'

// The ONE wallpaper compositor (spec 04 §4, ADR-0014 D1). Renders source +
// 壁纸压暗 + adaptive-material zones. Live: viewport resolution,
// invalidate-driven (no free-running ticker). Bake: same scene at native
// resolution into a RenderTexture → PNG. All layout is authored in
// DESKTOP-PIXEL space; the stage scale maps it to the backing store.

/** Map frost gaussian σ to pixi BlurFilter strength (visually tuned; the parity
 *  fixtures on Windows pin the final constant). */
// Raised 1.9 → 2.6 (owner 2026-07-09): a stronger frost blur homogenizes a
// high-contrast wallpaper under a zone (mountain vs water) into one calm tone
// instead of showing its hard edge through the panel. F8 re-pins the parity const.
const SIGMA_TO_STRENGTH = 3.6
/** Zone delete exit duration — matches the DOM chrome's 140ms scale/fade. */
const EXIT_MS = 140

export interface CompositorSource {
  bitmap: ImageBitmap
  width: number
  height: number
}

/** Per-zone facts the DOM editor chrome needs (ghost slots, rename band). */
export interface ZoneMeta {
  tone: 'Light' | 'Dark'
  /** Title rides above the panel top (gutter lane). */
  overhang: boolean
  /** Ghost slots (and mentally, icons) skip the zone's first row. */
  reserveFirstRow: boolean
}

export class WallpaperCompositor {
  private app!: Application
  private grid!: WallpaperGridInfoDto
  private sourceTexture!: Texture
  private sourceBitmap: ImageBitmap | null = null
  private samples!: SampleBuffer
  private wallTint = '#888888'
  /** REAL source image dims — the Liquid-Glass backdrop cover-fits to the SAME
   *  pixels as the displayed wallpaper so its refracted rim lines up. */
  private sourceW = 0
  private sourceH = 0

  private sourceSprite!: Sprite
  private claritySprite!: Sprite
  private zonesLayer!: Container
  private nodes = new Map<string, ZoneNode>()
  /** Zones mid-exit: material alpha fades over EXIT_MS in step with the DOM
   *  chrome's AnimatePresence exit (spec 04 §3 delete exit — the two layers
   *  must leave together or the frost pops while the shell lingers). */
  private dying = new Map<string, { node: ZoneNode; at: number }>()
  // Frost backdrop blur — applied ONCE to the full-screen wallpaper (frostSource →
  // frostRT), not per zone. Every frost zone then samples the pre-blurred frostRT
  // through its rounded mask as a plain textured fill: no per-zone filter, no
  // rectangular filterArea, so the old "hard square leaking past the mask over a
  // clarity gradient" is geometrically impossible. `repeatEdgePixels` clamps the
  // screen-edge samples instead of fading them to transparent.
  private blur = new BlurFilter({ strength: 8, quality: 6 })
  // Pre-blurred full-screen backdrop (wallpaper + dim scrim): the ONE frost pass
  // shared by every zone. Includes the dim so a frost zone reads as the DIMMED
  // wallpaper instead of glowing bright over a dimmed desktop (owner 2026-07-14).
  private frostRT: RenderTexture | null = null
  // SHARP full-screen backdrop (wallpaper + dim, no blur): what Liquid Glass
  // refracts — the exact pixels the desktop shows, dim included.
  private backdropRT: RenderTexture | null = null
  /** The dims×scale the RTs were CREATED for (see ensureFrostRT — float-safe key). */
  private backdropKey = ''
  /** Consecutive renderNow faults — bounds the self-heal retry (see renderNow). */
  private renderFailures = 0
  /** GPU `MAX_TEXTURE_SIZE`, probed once at create — bake caps its RTs to this so a
   *  native-res target on a 5K/8K screen or an old iGPU (4096 cap) never silently
   *  allocates an oversized texture and bakes a BLACK image (§A5). */
  private maxTextureSize = 4096
  private frostWall!: Sprite
  private backdropWall!: Sprite
  private frostDim!: Sprite

  private look: LookDto | null = null
  private renderScale = 1
  private dirty = false
  private raf = 0
  private clarityKey = ''
  private disposed = false
  private hiddenTitleId: string | null = null
  private provisional: { cellX: number; cellY: number; cellsWide: number; cellsTall: number } | null = null
  private metaListener: ((meta: Record<string, ZoneMeta>) => void) | null = null
  private lastMetaKey = ''

  static async create(
    canvas: HTMLCanvasElement,
    source: CompositorSource,
    grid: WallpaperGridInfoDto,
    wallTint: string,
  ): Promise<WallpaperCompositor> {
    const c = new WallpaperCompositor()
    c.blur.repeatEdgePixels = true // set once so a first-action bake is safe; re-asserted per live render for HMR
    c.grid = grid
    c.wallTint = wallTint
    c.app = new Application()
    await c.app.init({
      canvas,
      width: Math.max(2, Math.round(grid.screenWidth * c.renderScale)),
      height: Math.max(2, Math.round(grid.screenHeight * c.renderScale)),
      backgroundAlpha: 0,
      antialias: true,
      autoStart: false,
      preference: 'webgl',
    })
    c.app.ticker.stop()
    c.maxTextureSize = readMaxTextureSize(c.app.renderer)
    c.setSource(source)

    c.sourceSprite = new Sprite(c.sourceTexture)
    coverFit(c.sourceSprite, source.width, source.height, grid.screenWidth, grid.screenHeight)
    c.claritySprite = new Sprite(Texture.EMPTY)
    c.zonesLayer = new Container()
    c.app.stage.addChild(c.sourceSprite, c.claritySprite, c.zonesLayer)
    // Offscreen frost backdrop, composited into frostRT each frame (NOT on the
    // stage): the BLURRED wallpaper with the SHARP dim scrim overlaid, so a frost
    // zone carries the full local dim and matches the dimmed desktop.
    c.frostWall = new Sprite(c.sourceTexture)
    c.frostWall.filters = [c.blur] // blur the wallpaper; the dim scrim stays sharp
    c.backdropWall = new Sprite(c.sourceTexture) // sharp twin for backdropRT
    c.frostDim = new Sprite(Texture.EMPTY)
    return c
  }

  setSource(source: CompositorSource): void {
    this.sourceTexture?.destroy(true)
    // The compositor OWNS the bitmap from here: close the previous one or its
    // decoded backing store lives until context loss (codex review M3).
    this.sourceBitmap?.close()
    this.sourceBitmap = source.bitmap
    this.sourceW = source.width
    this.sourceH = source.height
    this.sourceTexture = Texture.from(source.bitmap)
    // Sampling buffer for the adaptive material (small, rebuilt per source).
    const sc = document.createElement('canvas')
    const sw = Math.min(256, source.width)
    const sh = Math.max(1, Math.round((source.height / source.width) * sw))
    sc.width = sw
    sc.height = sh
    const ctx = sc.getContext('2d')!
    ctx.drawImage(source.bitmap, 0, 0, sw, sh)
    const img = ctx.getImageData(0, 0, sw, sh)
    this.samples = buildSampleBuffer(img.data, sw, sh)
    if (this.sourceSprite) {
      this.sourceSprite.texture = this.sourceTexture
      coverFit(this.sourceSprite, source.width, source.height, this.grid.screenWidth, this.grid.screenHeight)
      for (const node of this.nodes.values()) node.setSourceTexture(this.sourceTexture)
      this.clarityKey = ''
      this.invalidate()
    }
  }

  /** Backing-store scale: min(1, viewZoom × dpr), additionally capped so the LIVE
   *  canvas long edge stays ≤4096 (8K monitors must not pay native-res preview
   *  targets per frame) AND never exceeds the probed GPU MAX_TEXTURE_SIZE (a sub-4096
   *  iGPU would otherwise allocate past its limit). Bake caps separately in bake().  */
  setRenderScale(scale: number): void {
    const longEdge = Math.max(this.grid.screenWidth, this.grid.screenHeight)
    const cap = Math.min(1, Math.min(4096, this.maxTextureSize) / longEdge)
    const k = Math.min(cap, Math.max(0.05, scale))
    if (Math.abs(k - this.renderScale) < 0.01) return
    this.renderScale = k
    this.app.renderer.resize(
      Math.max(2, Math.round(this.grid.screenWidth * k)),
      Math.max(2, Math.round(this.grid.screenHeight * k)),
    )
    this.invalidate()
  }

  /** The zone whose title should hide (its DOM rename editor is open). */
  setRenamingZone(id: string | null): void {
    if (this.hiddenTitleId === id) return
    this.hiddenTitleId = id
    this.invalidate()
  }

  /** Subscribe to per-zone facts (resolved tone, title lane). */
  onZoneMeta(cb: ((meta: Record<string, ZoneMeta>) => void) | null): void {
    this.metaListener = cb
    this.lastMetaKey = ''
  }

  /** Forming material during rubber-band create (spec 04 §3). */
  setProvisional(rect: { cellX: number; cellY: number; cellsWide: number; cellsTall: number } | null): void {
    this.provisional = rect
    this.invalidate()
  }

  update(look: LookDto): void {
    this.look = look
    this.invalidate()
  }

  invalidate(): void {
    if (this.dirty || this.disposed) return
    this.dirty = true
    this.raf = requestAnimationFrame(() => {
      this.dirty = false
      this.renderNow()
    })
  }

  private zoneRectPx(z: ZoneDto) {
    const g = this.grid
    return {
      left: g.inset + z.cellX * g.cellWidth,
      top: g.inset + z.cellY * g.cellHeight,
      width: z.cellsWide * g.cellWidth,
      height: z.cellsTall * g.cellHeight,
    }
  }

  /** Free space above a zone before the screen edge or the nearest zone above. */
  private clearanceAbove(zone: ZoneDto, zones: ZoneDto[]): number {
    const r = this.zoneRectPx(zone)
    let clearance = r.top
    for (const other of zones) {
      if (other.id === zone.id) continue
      const o = this.zoneRectPx(other)
      const overlapsX = o.left < r.left + r.width && o.left + o.width > r.left
      if (overlapsX && o.top + o.height <= r.top + 1) {
        clearance = Math.min(clearance, r.top - (o.top + o.height))
      }
    }
    return clearance
  }

  private syncScene(): void {
    if (!this.look) return
    const look = this.look
    const seen = new Set<string>()
    const g = this.grid

    // Clarity layer (canvas gradient, keyed on its params).
    const ck = JSON.stringify(look.clarity) + this.wallTint
    if (ck !== this.clarityKey) {
      this.clarityKey = ck
      const canvas = clarityCanvas(look.clarity, g, this.wallTint)
      this.claritySprite.texture.destroy(true)
      this.claritySprite.texture = canvas ? Texture.from(canvas) : Texture.EMPTY
      this.claritySprite.visible = !!canvas
      if (canvas) {
        this.claritySprite.width = g.screenWidth
        this.claritySprite.height = g.screenHeight
      }
    }

    const zones: ZoneDto[] = this.provisional
      ? [
          ...look.zones,
          {
            id: '__provisional__',
            ...this.provisional,
            title: '',
            emoji: null,
            accent: null,
            tone: 'Auto' as const,
            material: 'Frost' as const,
            titleStyle: 'Chip' as const,
            shadow: false,
            fillOpacity: null,
            cornerRadius: 20,
            titleSize: 'M' as const,
            fontFamily: null,
          },
        ]
      : look.zones

    const deps = {
      grid: g,
      sourceTexture: this.sourceTexture,
      frostTexture: this.frostRT!, // pre-blurred backdrop (rendered in renderNow/bake)
      backdropTexture: this.backdropRT!, // sharp backdrop for Liquid Glass
      renderScale: this.renderScale,
    }

    zones.forEach((zone, index) => {
      seen.add(zone.id)
      let node = this.nodes.get(zone.id)
      if (!node) {
        node = new ZoneNode(deps)
        this.nodes.set(zone.id, node)
        this.zonesLayer.addChild(node.root)
      }
      const r = this.zoneRectPx(zone)
      const sample = sampleRegion(
        this.samples,
        r.left / g.screenWidth,
        r.top / g.screenHeight,
        r.width / g.screenWidth,
        r.height / g.screenHeight,
      )
      const tone = resolveTone(zone.tone, sample, node.tone)
      node.tone = tone
      const paint = zonePaint({ zone, index, sample, tone, cellHeight: g.cellHeight })
      node.sync(zone, paint, r, deps, this.clearanceAbove(zone, zones), this.hiddenTitleId === zone.id)
    })

    for (const [id, node] of this.nodes) {
      if (!seen.has(id)) {
        // Hand the node to the exit lane instead of destroying it this frame.
        this.nodes.delete(id)
        if (id === '__provisional__') node.destroy()
        else this.dying.set(id, { node, at: performance.now() })
      }
    }
    // A re-added id (undo right after delete) must not fight its own ghost.
    for (const id of seen) {
      const dying = this.dying.get(id)
      if (dying) {
        dying.node.destroy()
        this.dying.delete(id)
      }
    }

    // Emit per-zone facts to the DOM chrome when they change.
    if (this.metaListener) {
      const meta: Record<string, ZoneMeta> = {}
      for (const z of look.zones) {
        const node = this.nodes.get(z.id)
        if (node) meta[z.id] = { tone: node.tone, overhang: node.overhang, reserveFirstRow: node.reserveFirstRow }
      }
      const key = JSON.stringify(meta)
      if (key !== this.lastMetaKey) {
        this.lastMetaKey = key
        this.metaListener(meta)
      }
    }
  }

  /** (Re)allocate the backdrop RTs to the current screen dims × render scale.
   *  Cheap no-op when nothing changed; recreated on a scale/dims change.
   *
   *  The no-op check compares the CREATION key, never `frostRT.width` — that getter
   *  is `pixelWidth / resolution`, a float division that misses strict equality with
   *  `screenWidth` whenever screenWidth × renderScale is not an integer (routine at
   *  125%/150% Windows DPI). The old getter comparison recreated BOTH RTs every
   *  frame, and any zone mid-delete-exit (evicted from `nodes`, so never re-synced)
   *  kept sampling the just-destroyed RT — the batcher then read a null texture
   *  source and the render died mid-frame: the owner's "delete a zone → white
   *  canvas until the next interaction" (2026-07-15, caught live in the debugger). */
  private ensureFrostRT(): void {
    const g = this.grid
    const key = `${g.screenWidth}x${g.screenHeight}@${this.renderScale}`
    if (this.frostRT && this.backdropRT && this.backdropKey === key) return
    this.backdropKey = key
    const oldFrost = this.frostRT
    const oldBackdrop = this.backdropRT
    this.frostRT = RenderTexture.create({ width: g.screenWidth, height: g.screenHeight, resolution: this.renderScale })
    this.backdropRT = RenderTexture.create({ width: g.screenWidth, height: g.screenHeight, resolution: this.renderScale })
    // Re-point every zone at the fresh targets BEFORE destroying the old ones —
    // INCLUDING mid-exit dying ghosts, which syncScene no longer visits. A ghost
    // left on a destroyed RT is exactly the white-canvas crash above.
    for (const node of this.nodes.values()) {
      node.repointBackdrop(oldFrost, this.frostRT, oldBackdrop, this.backdropRT)
    }
    for (const d of this.dying.values()) {
      d.node.repointBackdrop(oldFrost, this.frostRT, oldBackdrop, this.backdropRT)
    }
    oldFrost?.destroy(true)
    oldBackdrop?.destroy(true)
  }

  /** Composite the frost backdrop into frostRT — one pass every frost zone samples,
   *  replacing N per-zone blur filters (and the square they could leak). Two draws:
   *  (1) the BLURRED wallpaper, then (2) the SHARP (unblurred) dim scrim over it, so
   *  a frost zone carries the FULL local dim at its position — not a blur-averaged
   *  one — and reads as the dimmed desktop instead of glowing bright over it. */
  private renderFrostRT(): void {
    const g = this.grid
    this.frostWall.texture = this.sourceTexture
    coverFit(this.frostWall, this.sourceW, this.sourceH, g.screenWidth, g.screenHeight)
    this.app.renderer.render({ container: this.frostWall, target: this.frostRT!, clear: true })
    this.backdropWall.texture = this.sourceTexture
    coverFit(this.backdropWall, this.sourceW, this.sourceH, g.screenWidth, g.screenHeight)
    this.app.renderer.render({ container: this.backdropWall, target: this.backdropRT!, clear: true })
    if (this.claritySprite.visible) {
      this.frostDim.texture = this.claritySprite.texture
      this.frostDim.width = g.screenWidth
      this.frostDim.height = g.screenHeight
      this.app.renderer.render({ container: this.frostDim, target: this.frostRT!, clear: false })
      this.app.renderer.render({ container: this.frostDim, target: this.backdropRT!, clear: false })
    }
  }

  private renderNow(): void {
    if (this.disposed || !this.look) return
    // A fault below aborts the frame AFTER the canvas was cleared — without a
    // recovery the user stares at a white canvas until the next interaction
    // happens to invalidate (the zone-delete crash's visible symptom). Catch,
    // report, and retry a few frames; a healthy render resets the budget.
    try {
      // Blur the whole wallpaper into frostRT BEFORE the zones sample it (one pass).
      this.blur.strength = Math.max(0.5, this.grid.cellHeight * (1 / 6) * SIGMA_TO_STRENGTH * this.renderScale)
      this.blur.repeatEdgePixels = true // clamp the screen-edge samples (see field note)
      this.ensureFrostRT()
      this.syncScene() // updates the clarity scrim texture BEFORE frostRT mirrors it
      this.renderFrostRT()
      // Exit lane: fade dying zones' material alpha in step with the DOM exit.
      const now = performance.now()
      for (const [id, d] of this.dying) {
        const t = (now - d.at) / EXIT_MS
        if (t >= 1) {
          d.node.destroy()
          this.dying.delete(id)
        } else {
          d.node.root.alpha = 1 - t
        }
      }
      this.app.stage.setFromMatrix(new Matrix().scale(this.renderScale, this.renderScale))
      this.app.render()
    } catch (err) {
      this.renderFailures++
      console.error(`wallpaper compositor render failed (attempt ${this.renderFailures})`, err)
      if (this.renderFailures <= 5) this.invalidate() // bounded: a permanent fault must not error-loop at rAF pace
      return
    }
    this.renderFailures = 0
    if (this.dying.size > 0) this.invalidate() // keep ticking until the exit lane drains
  }

  /** Native-resolution bake → PNG blob. One frame; heavy but rare. */
  async bake(): Promise<Blob> {
    const liveScale = this.renderScale
    const liveStrength = this.blur.strength
    const g = this.grid
    // Bake at native res (scale 1) — but never allocate an RT larger than the GPU's
    // MAX_TEXTURE_SIZE, or the frost/backdrop/final targets come back BLACK on a 5K/8K
    // screen or an old iGPU (§A5). When native overflows the cap we bake at the largest
    // fitting scale: a slightly-soft wallpaper (Windows upscales it) beats a black one.
    // Every downstream target keys off renderScale (ensureFrostRT) so one cap covers all.
    const bakeScale = Math.min(1, this.maxTextureSize / Math.max(g.screenWidth, g.screenHeight))
    if (bakeScale < 1) {
      console.warn(
        `bake: native ${g.screenWidth}×${g.screenHeight} exceeds GPU MAX_TEXTURE_SIZE ${this.maxTextureSize}; ` +
          `baking at ${Math.round(bakeScale * 100)}% to avoid a black export`,
      )
    }
    this.renderScale = bakeScale
    this.blur.strength = Math.max(0.5, this.grid.cellHeight * (1 / 6) * SIGMA_TO_STRENGTH)
    this.blur.repeatEdgePixels = true
    for (const d of this.dying.values()) d.node.destroy()
    this.dying.clear() // a bake is a committed look — no mid-exit ghosts in it
    this.ensureFrostRT() // frostRT at bakeScale for the native-res bake
    this.syncScene() // clarity scrim current before frostRT mirrors it
    this.renderFrostRT()
    const rt = RenderTexture.create({ width: g.screenWidth, height: g.screenHeight, resolution: bakeScale })
    try {
      this.app.renderer.render({ container: this.app.stage, target: rt, transform: new Matrix() })
      const canvas = this.app.renderer.extract.canvas(rt) as HTMLCanvasElement
      return await new Promise<Blob>((resolve, reject) => {
        canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('bake: PNG encode failed'))), 'image/png')
      })
    } finally {
      this.blur.strength = liveStrength
      this.renderScale = liveScale
      rt.destroy(true)
      this.invalidate() // restore the live frame (re-syncs filters at live scale)
    }
  }

  destroy(): void {
    this.disposed = true
    cancelAnimationFrame(this.raf)
    for (const node of this.nodes.values()) node.destroy()
    for (const d of this.dying.values()) d.node.destroy()
    this.dying.clear()
    this.nodes.clear()
    this.frostRT?.destroy(true)
    this.backdropRT?.destroy(true)
    this.frostWall?.destroy()
    this.backdropWall?.destroy()
    this.frostDim?.destroy()
    this.app.destroy(undefined, { children: true, texture: true })
    this.sourceBitmap?.close()
    this.sourceBitmap = null
  }
}

/** GPU `MAX_TEXTURE_SIZE` off the live pixi WebGL renderer (we init with
 *  `preference:'webgl'`, so `renderer.gl` is always present). Duck-typed rather than
 *  cast to a pixi type so a WebGPU/software fallback can't throw; defaults to 4096
 *  (WebGL2 guarantees ≥2048, and 4096 is the common low-end iGPU cap) when unreadable. */
function readMaxTextureSize(renderer: unknown): number {
  const gl = (renderer as { gl?: WebGL2RenderingContext }).gl
  if (gl && typeof gl.getParameter === 'function') {
    const max = gl.getParameter(gl.MAX_TEXTURE_SIZE) as unknown
    if (typeof max === 'number' && max > 0) return max
  }
  return 4096
}

// Vite/React Fast Refresh preserves the compositor instance across an HMR edit
// (the create-effect's deps are the screen dims — stable), so a compositor-code
// change would keep stale ZoneNode instances and silently not take — the source of
// the "already fixed but still shows" square/glass confusion. Force a full reload
// on any edit to a compositor module so the whole scene rebuilds. Dev only.
if (import.meta.hot) import.meta.hot.accept(() => location.reload())
