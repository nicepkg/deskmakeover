import { BlurFilter, Container, FillGradient, Graphics, Rectangle, Sprite, Texture } from 'pixi.js'
import type { WallpaperGridInfoDto, ZoneDto } from '@/bridge/types'
import type { ZonePaint } from './material'
import { barBandPaint, measureTitle, rasterizeTitle, titleFontPx, titleLayout } from './title-chip'
import type { TitleLayout } from './title-chip'

// One zone's scene subtree (split from renderer.ts, ≤500-line law): shadow →
// frost → fill (+gradient/glow/band) → title sprite. All geometry in desktop
// pixels; the stage transform maps to the backing store.

/** Extra context the blur may sample around a zone before masking. */
const BLUR_PAD_SIGMAS = 3

export interface ZoneNodeDeps {
  grid: WallpaperGridInfoDto
  sourceTexture: Texture
  sharedBlur: BlurFilter
  renderScale: number
}

export interface ZoneRectPx {
  left: number
  top: number
  width: number
  height: number
}

export class ZoneNode {
  readonly root = new Container()
  private shadowG = new Graphics()
  private frost: Sprite
  private maskG = new Graphics()
  private fillG = new Graphics()
  private glowG = new Graphics()
  private title = new Sprite(Texture.EMPTY)
  private featherFilter: BlurFilter | null = null
  private glowFilter: BlurFilter | null = null
  private shadowFilter: BlurFilter | null = null
  private gradientCache: { key: string; fill: FillGradient } | null = null
  private titleKey = ''

  tone: 'Light' | 'Dark' = 'Dark'
  overhang = true
  reserveFirstRow = false

  constructor(deps: ZoneNodeDeps) {
    this.frost = new Sprite(deps.sourceTexture)
    this.frost.filters = [deps.sharedBlur]
    this.frost.mask = this.maskG
    this.glowG.mask = this.maskG
    this.root.addChild(this.shadowG, this.frost, this.maskG, this.fillG, this.glowG, this.title)
  }

  setSourceTexture(texture: Texture): void {
    this.frost.texture = texture
  }

  destroy(): void {
    this.title.texture?.destroy(true)
    this.root.destroy({ children: true })
  }

  /** Sync all layers from the zone + paint. `hideTitle` while the DOM rename
   *  editor is open. Returns the resolved title layout (lane feeds ZoneMeta). */
  sync(
    zone: ZoneDto,
    paint: ZonePaint,
    r: ZoneRectPx,
    deps: ZoneNodeDeps,
    clearanceAbove: number,
    hideTitle: boolean,
  ): TitleLayout {
    const g = deps.grid
    const k = deps.renderScale
    const hair = Math.max(1, 1 / k)
    const radius = paint.cornerRadius

    // Wallpaper frost under the panel. Halo skips it in v1 (a hard-edged mask
    // under the feathered glow would betray the feather; polish item).
    const frosting = paint.blurSigma > 0.5 && paint.material !== 'Halo'
    this.frost.visible = frosting
    if (frosting) {
      this.frost.width = g.screenWidth
      this.frost.height = g.screenHeight
      const pad = paint.blurSigma * BLUR_PAD_SIGMAS
      this.frost.filterArea = new Rectangle(
        Math.max(0, r.left - pad),
        Math.max(0, r.top - pad),
        Math.min(g.screenWidth, r.width + pad * 2),
        Math.min(g.screenHeight, r.height + pad * 2),
      )
    }

    this.maskG.clear().roundRect(r.left, r.top, r.width, r.height, radius).fill(0xffffff)

    // 投影 finish: blurred dark round rect behind the body.
    this.shadowG.clear()
    if (paint.shadow) {
      this.shadowG
        .roundRect(r.left, r.top + paint.shadow.offsetY, r.width, r.height, radius)
        .fill({ color: 0x000000, alpha: paint.shadow.alpha })
      const strength = Math.max(1, paint.shadow.blur * k)
      if (!this.shadowFilter) this.shadowFilter = new BlurFilter({ quality: 4, strength })
      this.shadowFilter.strength = strength
      this.shadowG.filters = [this.shadowFilter]
      const pad = paint.shadow.blur * 3
      this.shadowG.filterArea = new Rectangle(r.left - pad, r.top - pad, r.width + pad * 2, r.height + pad * 2 + paint.shadow.offsetY)
    } else {
      this.shadowG.filters = []
    }

    // Body fill (+ per-material dressing).
    const f = this.fillG.clear()
    if (paint.gradient) {
      // local-space stops → the gradient is rect-size independent; cache it so
      // a drag repaint does not allocate per frame (codex review m4).
      const gradKey = [paint.gradient.top.color, paint.gradient.top.alpha, paint.gradient.bottom.color, paint.gradient.bottom.alpha].join('|')
      if (this.gradientCache?.key !== gradKey) {
        this.gradientCache = {
          key: gradKey,
          fill: new FillGradient({
            type: 'linear',
            start: { x: 0, y: 0 },
            end: { x: 0, y: 1 },
            colorStops: [
              { offset: 0, color: withAlpha(paint.gradient.top.color, paint.gradient.top.alpha) },
              { offset: 1, color: withAlpha(paint.gradient.bottom.color, paint.gradient.bottom.alpha) },
            ],
            textureSpace: 'local',
          }),
        }
      }
      f.roundRect(r.left, r.top, r.width, r.height, radius).fill(this.gradientCache.fill)
    } else {
      f.roundRect(r.left, r.top, r.width, r.height, radius).fill({ color: paint.fill.color, alpha: paint.fill.alpha })
    }

    // Halo: feather the whole fill (edge fade IS the material identity).
    if (paint.featherSigma > 0) {
      const strength = Math.max(1, paint.featherSigma * k)
      if (!this.featherFilter) this.featherFilter = new BlurFilter({ quality: 6, strength })
      this.featherFilter.strength = strength
      this.fillG.filters = [this.featherFilter]
      const pad = paint.featherSigma * 3
      this.fillG.filterArea = new Rectangle(r.left - pad, r.top - pad, r.width + pad * 2, r.height + pad * 2)
    } else {
      this.fillG.filters = []
    }

    // Bar band + divider span the panel (title style Bar).
    const fontPx = titleFontPx(zone.titleSize, g.cellHeight)
    const layout = titleLayout({
      style: zone.titleStyle,
      zoneRect: r,
      cellHeight: g.cellHeight,
      titleSize: zone.titleSize,
      cornerRadius: radius,
      textWidth: measureTitle(zone, fontPx),
      clearanceAbove,
    })
    if (zone.titleStyle === 'Bar') {
      const bar = barBandPaint(paint)
      if (bar.band) {
        f.roundRect(r.left, r.top, r.width, layout.height + radius, radius)
          .fill({ color: bar.band.color, alpha: bar.band.alpha })
        // Re-mask the band overflow below the rounded top by the panel mask.
        f.rect(r.left, r.top + layout.height, r.width, radius).cut()
      }
      f.rect(r.left + radius * 0.5 + 8, r.top + layout.height - bar.divider.width, r.width - radius - 16, bar.divider.width)
        .fill({ color: bar.divider.color, alpha: bar.divider.alpha })
    }

    // Depth: top inner highlight + outer contour (per-material values).
    if (paint.highlight.alpha > 0) {
      const w = Math.max(paint.highlight.width, hair)
      f.moveTo(r.left + radius, r.top + w / 2)
        .lineTo(r.left + r.width - radius, r.top + w / 2)
        .stroke({ width: w, color: paint.highlight.color, alpha: paint.highlight.alpha })
    }
    if (paint.contour.alpha > 0) {
      f.roundRect(r.left - hair / 2, r.top - hair / 2, r.width + hair, r.height + hair, radius + hair / 2)
        .stroke({ width: hair, color: paint.contour.color, alpha: paint.contour.alpha })
    }
    if (paint.outlineRing) {
      f.roundRect(r.left, r.top, r.width, r.height, radius)
        .stroke({ width: Math.max(2, hair * 2), color: paint.outlineRing.color, alpha: paint.outlineRing.alpha })
    }

    // Luminous accent inner glow: soft inset stroke, masked to the panel.
    this.glowG.clear()
    if (paint.innerGlow) {
      const ig = paint.innerGlow
      this.glowG
        .roundRect(r.left + ig.inset, r.top + ig.inset, r.width - ig.inset * 2, r.height - ig.inset * 2, Math.max(4, radius - ig.inset))
        .stroke({ width: 2, color: ig.color, alpha: ig.alpha })
      const strength = Math.max(1, ig.blur * k)
      if (!this.glowFilter) this.glowFilter = new BlurFilter({ quality: 4, strength })
      this.glowFilter.strength = strength
      this.glowG.filters = [this.glowFilter]
      this.glowG.filterArea = new Rectangle(r.left, r.top, r.width, r.height)
    } else {
      this.glowG.filters = []
    }

    // Title raster.
    this.overhang = layout.overhang
    this.reserveFirstRow = layout.reserveFirstRow
    this.title.visible = !hideTitle && (zone.title.length > 0 || !!zone.emoji)
    if (this.title.visible) {
      const key = [
        zone.titleStyle, zone.title, zone.emoji, zone.titleSize, zone.fontFamily, paint.accent, paint.tone,
        paint.chip.fill.color, paint.chip.fill.alpha.toFixed(2), Math.round(layout.width), Math.round(layout.height),
      ].join('|')
      if (key !== this.titleKey) {
        this.titleKey = key
        const raster = rasterizeTitle(zone, paint, layout)
        this.title.texture?.destroy(true)
        this.title.texture = Texture.from(raster.canvas)
        this.title.scale.set(1 / raster.scale)
      }
      this.title.position.set(layout.x, layout.y)
    }

    return layout
  }
}

function withAlpha(hex: string, alpha: number): string {
  const a = Math.round(Math.min(1, Math.max(0, alpha)) * 255).toString(16).padStart(2, '0')
  return `${hex}${a}`
}
