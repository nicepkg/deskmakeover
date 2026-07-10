import { Application, BlurFilter, Container, Matrix, RenderTexture, Sprite, Texture } from 'pixi.js'
import type { LookDto, WallpaperGridInfoDto, ZoneDto } from '@/bridge/types'
import { zonePaint } from './material'
import { buildSampleBuffer, resolveTone, sampleRegion } from './sampling'
import type { SampleBuffer } from './sampling'
import { clarityCanvas } from './clarity'
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

  private sourceSprite!: Sprite
  private claritySprite!: Sprite
  private zonesLayer!: Container
  private nodes = new Map<string, ZoneNode>()
  /** Zones mid-exit: material alpha fades over EXIT_MS in step with the DOM
   *  chrome's AnimatePresence exit (spec 04 §3 delete exit — the two layers
   *  must leave together or the frost pops while the shell lingers). */
  private dying = new Map<string, { node: ZoneNode; at: number }>()
  private blur = new BlurFilter({ strength: 8, quality: 6 })

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
    c.setSource(source)

    c.sourceSprite = new Sprite(c.sourceTexture)
    c.sourceSprite.width = grid.screenWidth
    c.sourceSprite.height = grid.screenHeight
    c.claritySprite = new Sprite(Texture.EMPTY)
    c.zonesLayer = new Container()
    c.app.stage.addChild(c.sourceSprite, c.claritySprite, c.zonesLayer)
    return c
  }

  setSource(source: CompositorSource): void {
    this.sourceTexture?.destroy(true)
    // The compositor OWNS the bitmap from here: close the previous one or its
    // decoded backing store lives until context loss (codex review M3).
    this.sourceBitmap?.close()
    this.sourceBitmap = source.bitmap
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
      this.sourceSprite.width = this.grid.screenWidth
      this.sourceSprite.height = this.grid.screenHeight
      for (const node of this.nodes.values()) node.setSourceTexture(this.sourceTexture)
      this.clarityKey = ''
      this.invalidate()
    }
  }

  /** Backing-store scale: min(1, viewZoom × dpr), additionally capped so the
   *  LIVE canvas long edge stays ≤4096 (8K monitors must not pay native-res
   *  preview targets per frame; bake is unaffected — codex review M1 partial;
   *  the full MAX_TEXTURE_SIZE / SwiftShader probe stays on the STATE list). */
  setRenderScale(scale: number): void {
    const longEdge = Math.max(this.grid.screenWidth, this.grid.screenHeight)
    const cap = Math.min(1, 4096 / longEdge)
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
      sharedBlur: this.blur,
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

  private renderNow(): void {
    if (this.disposed || !this.look) return
    this.syncScene()
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
    this.blur.strength = Math.max(0.5, this.grid.cellHeight * (1 / 6) * SIGMA_TO_STRENGTH * this.renderScale)
    this.app.stage.setFromMatrix(new Matrix().scale(this.renderScale, this.renderScale))
    this.app.render()
    if (this.dying.size > 0) this.invalidate() // keep ticking until the exit lane drains
  }

  /** Native-resolution bake → PNG blob. One frame; heavy but rare. */
  async bake(): Promise<Blob> {
    const liveScale = this.renderScale
    const liveStrength = this.blur.strength
    // Bake at scale 1: per-node filters (feather/shadow/glow) read renderScale.
    this.renderScale = 1
    this.blur.strength = Math.max(0.5, this.grid.cellHeight * (1 / 6) * SIGMA_TO_STRENGTH)
    for (const d of this.dying.values()) d.node.destroy()
    this.dying.clear() // a bake is a committed look — no mid-exit ghosts in it
    this.syncScene()
    const g = this.grid
    const rt = RenderTexture.create({ width: g.screenWidth, height: g.screenHeight, resolution: 1 })
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
    this.app.destroy(undefined, { children: true, texture: true })
    this.sourceBitmap?.close()
    this.sourceBitmap = null
  }
}
