import { BlurFilter, Container, Graphics, Rectangle, Sprite, Texture } from 'pixi.js'
import { panelTexture, sheenTexture } from './panel-textures'
import type { WallpaperGridInfoDto, ZoneDto } from '@/bridge/types'
import type { ZonePaint } from './material'
import { LiquidGlassFilter } from './liquid-glass-filter'
import { barSeamPaint, measureTitle, rasterizeTitle, titleFontPx, titleLayout } from './title-chip'
import type { TitleLayout } from './title-chip'

// One zone's scene subtree (split from renderer.ts, ≤500-line law): shadow →
// frost → fill (+gradient/glow/band) → title sprite. All geometry in desktop
// pixels; the stage transform maps to the backing store.

export interface ZoneNodeDeps {
  grid: WallpaperGridInfoDto
  sourceTexture: Texture
  /** Pre-blurred full-screen backdrop (renderer.frostRT): the frost backdrop is a
   *  plain textured fill sampled from this — NO per-zone blur filter, so the rounded
   *  mask clips it cleanly and a rectangular filterArea can never leak a square. */
  frostTexture: Texture
  /** SHARP full-screen backdrop (wallpaper + dim scrim, renderer.backdropRT):
   *  what Liquid Glass refracts — the same pixels the desktop shows, dim included,
   *  so the glass never glows bright over a dimmed wallpaper. */
  backdropTexture: Texture
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
  /** Wrapper so the frost's ROUNDED mask clips the blurred sprite's output (a
   *  mask on a filtered sprite directly does not reliably clip the filter rect). */
  private frostWrap = new Container()
  private maskG = new Graphics()
  private fillG = new Graphics()
  private glowG = new Graphics()
  /** Glow's OWN mask — a mask object may own only one target, so it can't share
   *  maskG with the frost (sharing left the frost unmasked → a hard square). */
  private glowMaskG = new Graphics()
  private title = new Sprite(Texture.EMPTY)
  private glowFilter: BlurFilter | null = null
  private shadowFilter: BlurFilter | null = null
  private glassFilter: LiquidGlassFilter | null = null
  private titleKey = ''

  tone: 'Light' | 'Dark' = 'Dark'
  overhang = true
  reserveFirstRow = false

  constructor(deps: ZoneNodeDeps) {
    this.frost = new Sprite(deps.sourceTexture)
    // Frost = pre-blurred wallpaper sampled through a rounded mask, NO filter (set
    // in sync). Glass reassigns the filter per frame. frostWrap lets the mask clip
    // the sprite; glow keeps its own mask — a mask object owns only one target.
    this.frostWrap.addChild(this.frost)
    this.frostWrap.mask = this.maskG
    this.glowG.mask = this.glowMaskG
    this.root.addChild(this.shadowG, this.frostWrap, this.maskG, this.fillG, this.glowMaskG, this.glowG, this.title)
  }

  setSourceTexture(texture: Texture): void {
    this.frost.texture = texture
  }

  /** Swap a recreated shared backdrop RT under this node (identity-matched), so a
   *  node the scene sync no longer visits — a mid-exit dying ghost — never keeps
   *  sampling a destroyed texture (the zone-delete white-canvas crash). A node on
   *  neither old target (source texture, or hidden) is left untouched. */
  repointBackdrop(
    oldFrost: Texture | null,
    frost: Texture,
    oldBackdrop: Texture | null,
    backdrop: Texture,
  ): void {
    if (oldFrost && this.frost.texture === oldFrost) this.frost.texture = frost
    else if (oldBackdrop && this.frost.texture === oldBackdrop) this.frost.texture = backdrop
  }

  destroy(): void {
    this.title.texture?.destroy(true)
    this.glassFilter?.destroy() // custom GL program — release it (shared blur is NOT ours)
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
    // Render invariant (round 3): the 0–60 slider may exceed a small zone's
    // geometry — cap at half the shortest side so a panel rounds, never pills.
    const radius = Math.min(paint.cornerRadius, Math.min(r.width, r.height) / 2)

    // Backdrop under the panel — three paths, all sharing the ONE frost sprite:
    //  · Liquid Glass — the SHARP full-screen backdrop (wallpaper + dim) refracted
    //    by the complete archisvaze/liquid-glass shader. The shader owns the
    //    rounded shape (self-clipping AA) AND the outer shadow ring, so the wrap
    //    mask is DISABLED here — a 28px mask would amputate both.
    //  · Frost / Luminous — the pre-blurred backdrop through the rounded mask.
    //  · else — no backdrop.
    // Halo skips the generic frost in v1 (a hard-edged mask under the feathered
    // glow would betray the feather; polish item).
    const isGlass = paint.material === 'LiquidGlass' && !!paint.liquidGlass
    const frosting = paint.blurSigma > 0.5
    this.frost.visible = isGlass || frosting
    this.frostWrap.mask = isGlass ? null : this.maskG
    if (isGlass && paint.liquidGlass) {
      const lg = paint.liquidGlass
      this.frost.texture = deps.backdropTexture // reset (a frost zone had it on frostRT)
      this.frost.x = 0
      this.frost.y = 0
      this.frost.width = g.screenWidth
      this.frost.height = g.screenHeight
      // Bevel widths are FIXED px like a real pane's bevel (owner 2026-07-14:
      // enlarging a zone must grow the flat 1:1 center, never the rim). Only a
      // SMALL zone scales them down, to the demo's bezel≤30%/thickness≤25% of
      // min dim. Corners use the zone's own cornerRadius so glass rounds like
      // every other material (owner 2026-07-14); the shader's bezel cap no
      // longer involves the radius, so moderate corners keep the full dome.
      const minDim = Math.min(r.width, r.height)
      const bezel = Math.max(8, Math.min(lg.bezel, 0.3 * minDim))
      const thickness = Math.max(8, Math.min(lg.thickness, 0.25 * minDim))
      if (!this.glassFilter) this.glassFilter = new LiquidGlassFilter()
      // Uniforms in DESKTOP px — the reference's unit (its px constants are CSS
      // sized); k only converts px↔UV in-shader, so preview and bake match.
      this.glassFilter.configure({
        centerX: r.left + r.width / 2,
        centerY: r.top + r.height / 2,
        halfW: r.width / 2,
        halfH: r.height / 2,
        radius,
        thickness,
        bezel,
        ior: lg.ior,
        blur: lg.blur,
        specular: lg.specular,
        tint: lg.tint,
        shadow: lg.shadow,
        k,
      })
      this.frost.filters = [this.glassFilter]
      // The Snell displacement rises sharply at the curved rim; the shadow ring
      // extends past the SDF (exp(-sd²/800) ≈ 0 by 60px). Capture both.
      const refractionPad = thickness * (2 + 2 / Math.max(1, lg.ior))
      const pad = refractionPad + lg.blur * 2 + 60
      const x0 = Math.max(0, r.left - pad)
      const y0 = Math.max(0, r.top - pad)
      const x1 = Math.min(g.screenWidth, r.left + r.width + pad)
      const y1 = Math.min(g.screenHeight, r.top + r.height + pad)
      this.frost.filterArea = new Rectangle(x0, y0, x1 - x0, y1 - y0)
    } else if (frosting) {
      // Frost backdrop = the pre-blurred full-screen wallpaper (renderer.frostRT),
      // drawn as a plain textured fill with NO filter. The rounded frostWrap mask
      // clips an UNFILTERED sprite — the reliable Pixi path — so no rectangular
      // filterArea exists to leak a hard square over a clarity gradient.
      this.frost.filters = []
      this.frost.filterArea = undefined
      this.frost.texture = deps.frostTexture
      this.frost.x = 0
      this.frost.y = 0
      this.frost.width = g.screenWidth
      this.frost.height = g.screenHeight
    }

    // maskG is ALSO a child of root: while detached (glass mode) it must stay
    // EMPTY, or its white fill renders as an opaque panel over the shader.
    this.maskG.clear()
    if (!isGlass) this.maskG.roundRect(r.left, r.top, r.width, r.height, radius).fill(0xffffff)
    this.glowMaskG.clear().roundRect(r.left, r.top, r.width, r.height, radius).fill(0xffffff)

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
    f.roundRect(r.left, r.top, r.width, r.height, radius).fill({ color: paint.fill.color, alpha: paint.fill.alpha })

    // Procedural tile (Paper grain / Fluted ribs / Brushed streaks): px-scale,
    // repeat-tiled — stretching a small tile would smear the pattern on large
    // zones and break grain-density parity between preview and bake.
    if (paint.texture) {
      // textureSpace 'global': tile in WORLD px (repeat), not stretched to the
      // shape — local space would smear a 12px rib tile across the whole panel.
      f.roundRect(r.left, r.top, r.width, r.height, radius).fill({
        texture: panelTexture(paint.texture.kind),
        alpha: paint.texture.alpha,
        textureSpace: 'global',
      })
    }

    // Brushed sheen: ONE soft diagonal light band (anisotropic gloss). The band
    // is baked into a canvas texture (Canvas2D gradients honour alpha reliably)
    // and stretched over the panel — local space, so it scales with the zone
    // and bakes 1:1.
    if (paint.sheen) {
      f.roundRect(r.left, r.top, r.width, r.height, radius).fill({
        texture: sheenTexture(),
        alpha: paint.sheen.alpha,
        textureSpace: 'local',
      })
    }

    // Bar header seam spans the panel (title style Bar).
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
      // Full-width hairline seam = the title-bar baseline. layout.height > radius,
      // so at this y the panel edges are already vertical: the seam spans edge to
      // edge as a clean rule (no colour band — owner 2026-07-14).
      const seam = barSeamPaint(paint)
      f.rect(r.left, r.top + layout.height - seam.width, r.width, seam.width)
        .fill({ color: seam.color, alpha: seam.alpha })
    }

    // Depth: top inner highlight + outer contour (per-material values).
    if (paint.highlight.alpha > 0) {
      const w = Math.max(paint.highlight.width, hair)
      f.moveTo(r.left + radius, r.top + w / 2)
        .lineTo(r.left + r.width - radius, r.top + w / 2)
        .stroke({ width: w, color: paint.highlight.color, alpha: paint.highlight.alpha })
    }
    // Paper letterpress: the dark bottom inner line paired with the light top.
    if (paint.letterpressBottom) {
      f.moveTo(r.left + radius, r.top + r.height - hair / 2)
        .lineTo(r.left + r.width - radius, r.top + r.height - hair / 2)
        .stroke({ width: hair, color: paint.letterpressBottom.color, alpha: paint.letterpressBottom.alpha })
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
    this.title.visible = !hideTitle && zone.titleStyle !== 'None' && (zone.title.length > 0 || !!zone.emoji)
    if (this.title.visible) {
      const key = [
        zone.titleStyle, zone.title, zone.emoji, zone.titleSize, zone.fontFamily, paint.accent, paint.tone,
        paint.chip.fill.color, paint.chip.fill.alpha.toFixed(2), paint.cornerRadius, Math.round(layout.width), Math.round(layout.height),
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


// Rebuild the whole scene on edit — HMR keeps stale ZoneNode instances, so a
// constructor/scene-graph change here would otherwise not take (see renderer.ts).
if (import.meta.hot) import.meta.hot.accept(() => location.reload())
