import type { TitleSize, ZoneDto, ZoneTitleStyle } from '@/bridge/types'
import { hexToOklch, oklchToHex } from './oklch'
import type { ZonePaint } from './material'

// Title system (spec 04 §4.2 round 3, 2026-07-15): four styles + 无.
//   None   无 — hidden; a first-class member of the style axis (round 3).
//   Etched 冰签 — glass-native: translucent frosted lozenge (white α.16) with a
//          1px top-light / bottom-dark bevel ("etched into the glass"), adaptive
//          ink, NO accent block. LiquidGlass's default title.
//   Chip 胶囊标签 — accent-tinted pill, overhangs the panel top (default).
//   Bare 净色标题 — accent dot + text, baked soft halo for contrast, no backing.
//   Bar  顶栏标题 — full-width editorial header: a header-weight neutral title
//        over a full-width hairline seam (the title-bar baseline). No colour band,
//        no accent dot; always reserves icon row 1 (a real header, quietly).
// Retired round 3: Tab 折角页签 (folder skeuomorph; persisted zones migrate → Chip).
// Layout is pure (unit-tested); rasterization draws on Canvas2D at 2×.

export const CHIP_FONT_STACK = '"HarmonyOS Sans SC", "Inter", system-ui, sans-serif'
const SIZE_FACTOR: Record<TitleSize, number> = { S: 0.17, M: 0.2, L: 0.24 }
const PAD_X = 10
const PAD_Y = 5
/** Bar header type reads clearly larger than the float styles so the title reads
 *  as a real header; the band height is sized to it (headerPx + BAR_PAD_Y*2). */
const BAR_FONT_SCALE = 1.18
const BAR_PAD_Y = 8
/** Fraction of a cell the chip/bare overhangs above the panel top. */
export const OVERHANG_CELLS = 0.4
const RASTER_SCALE = 2

export interface TitleLayout {
  /** Raster rect in desktop px, relative to the wallpaper origin. */
  x: number
  y: number
  width: number
  height: number
  fontPx: number
  /** true = riding above the panel top (gutter lane). */
  overhang: boolean
  /** Ghost slots (and icons, mentally) skip the zone's first row. */
  reserveFirstRow: boolean
}

export function titleFontPx(size: TitleSize, cellHeight: number): number {
  return Math.round(Math.min(22, Math.max(15, cellHeight * SIZE_FACTOR[size])))
}

export interface TitleLayoutInput {
  style: ZoneTitleStyle
  zoneRect: { left: number; top: number; width: number; height: number }
  cellHeight: number
  titleSize: TitleSize
  cornerRadius: number
  /** Measured text width in desktop px (incl. emoji/dot allowances by caller). */
  textWidth: number
  /** Free space above the panel top (px) to the screen edge / nearest zone. */
  clearanceAbove: number
}

export function titleLayout(input: TitleLayoutInput): TitleLayout {
  const fontPx = titleFontPx(input.titleSize, input.cellHeight)
  const r = input.zoneRect

  if (input.style === 'None') {
    // Hidden title: zero footprint, no gutter lane, no reserved icon row.
    return { x: r.left, y: r.top, width: 0, height: 0, fontPx, overhang: false, reserveFirstRow: false }
  }

  if (input.style === 'Bar') {
    // Full-width editorial header; row 1 is ALWAYS reserved. Header type is bumped
    // so the title reads as a header (not a small floating label), and its height
    // clears the seam the renderer draws at the header/body boundary. The seam sits
    // below the panel's rounded top (height > cornerRadius), so it spans edge-to-
    // edge as a clean title-bar baseline — no colour band (owner 2026-07-14).
    const headerPx = Math.round(fontPx * BAR_FONT_SCALE)
    const height = headerPx + BAR_PAD_Y * 2
    return { x: r.left, y: r.top, width: r.width, height, fontPx: headerPx, overhang: false, reserveFirstRow: true }
  }

  // Chip / Bare / Etched share the chip lanes.
  const height = fontPx + PAD_Y * 2
  const width = Math.min(input.textWidth + PAD_X * 2, Math.max(40, r.width - 16))
  const x = r.left + input.cornerRadius * 0.5 + 14
  const overhangLift = input.cellHeight * OVERHANG_CELLS
  const overhang = input.clearanceAbove >= overhangLift
  return {
    x,
    y: overhang ? r.top - height / 2 : r.top + 10,
    width,
    height,
    fontPx,
    overhang,
    reserveFirstRow: !overhang,
  }
}

export interface TitleRaster {
  canvas: HTMLCanvasElement
  width: number
  height: number
  scale: number
}

function fontOf(zone: ZoneDto, fontPx: number): string {
  const family = zone.fontFamily ? `"${zone.fontFamily}", ${CHIP_FONT_STACK}` : CHIP_FONT_STACK
  return `600 ${fontPx}px ${family}`
}

/** Rasterize the title per style at 2× desktop resolution. */
export function rasterizeTitle(zone: ZoneDto, paint: ZonePaint, layout: TitleLayout): TitleRaster {
  const w = Math.ceil(layout.width)
  const h = Math.ceil(layout.height)
  const canvas = document.createElement('canvas')
  canvas.width = Math.max(2, w * RASTER_SCALE)
  canvas.height = Math.max(2, h * RASTER_SCALE)
  const ctx = canvas.getContext('2d')!
  ctx.scale(RASTER_SCALE, RASTER_SCALE)
  ctx.textBaseline = 'middle'
  const text = zone.emoji ? `${zone.emoji} ${zone.title}` : zone.title
  const light = paint.tone === 'Light'
  const accentLch = hexToOklch(paint.accent)

  switch (zone.titleStyle) {
    case 'Bare': {
      // [accent dot] [emoji+text] over a baked soft halo (contrast guarantee).
      const ink = light
        ? oklchToHex({ l: 0.25, c: Math.min(accentLch.c * 0.5, 0.05), h: accentLch.h })
        : oklchToHex({ l: 0.98, c: 0.01, h: accentLch.h })
      const dotR = Math.round(layout.fontPx * 0.275)
      ctx.font = fontOf(zone, layout.fontPx)
      ctx.shadowColor = light ? 'rgba(255,255,255,0.55)' : 'rgba(0,0,0,0.55)'
      ctx.shadowBlur = Math.round(layout.fontPx * 0.36) // ≈ σ·3 in canvas terms
      ctx.globalAlpha = 0.98
      ctx.fillStyle = paint.accent
      ctx.beginPath()
      ctx.arc(dotR + 1, h / 2, dotR, 0, Math.PI * 2)
      ctx.fill()
      ctx.fillStyle = ink
      ctx.fillText(text, dotR * 2 + 7, h / 2 + 0.5, w - dotR * 2 - 8)
      break
    }
    case 'Etched': {
      // 冰签 — frosted lozenge etched into the glass: translucent white body,
      // 1px top-light edge, neutral adaptive ink. No accent block — the title
      // shares the glass's light language, it is not a sticker. (The original
      // bottom-dark bevel line read as an ugly underline — owner 2026-07-15.)
      const radius = Math.min(10, h / 2)
      ctx.beginPath()
      ctx.roundRect(0.5, 0.5, w - 1, h - 1, radius)
      ctx.globalAlpha = 0.16
      ctx.fillStyle = '#FFFFFF'
      ctx.fill()
      ctx.globalAlpha = 1
      ctx.lineWidth = 1
      ctx.strokeStyle = 'rgba(255,255,255,0.55)'
      ctx.beginPath()
      ctx.moveTo(radius, 1)
      ctx.lineTo(w - radius, 1)
      ctx.stroke()
      const ink = light ? oklchToHex({ l: 0.22, c: 0.01, h: 260 }) : oklchToHex({ l: 0.97, c: 0.008, h: 260 })
      ctx.globalAlpha = 0.97
      ctx.font = fontOf(zone, layout.fontPx)
      ctx.fillStyle = ink
      ctx.fillText(text, PAD_X, h / 2 + 0.5, w - PAD_X * 2)
      break
    }
    case 'Bar': {
      // Editorial header: a header-weight title in the app's NEUTRAL ink (t1 on
      // light, near-white on dark — cool hue 260, like --t1), left-inset to the
      // panel's rounded corner, optically centred. The full-width hairline seam
      // below it is renderer-drawn. No colour band, no accent dot — the header
      // reads via type + seam alone (owner 2026-07-14: 圆角长条/一撇 removed).
      const inset = barInset(paint.cornerRadius)
      const ink = light ? oklchToHex({ l: 0.25, c: 0.012, h: 260 }) : oklchToHex({ l: 0.965, c: 0.008, h: 260 })
      ctx.font = fontOf(zone, layout.fontPx)
      ctx.globalAlpha = 0.98
      ctx.fillStyle = ink
      ctx.fillText(text, inset, h / 2 + 0.5, Math.max(8, w - inset * 2))
      break
    }
    default: {
      // Chip 胶囊 (spec §4.2 original recipe).
      const radius = Math.min(10, h / 2)
      ctx.beginPath()
      ctx.roundRect(0.5, 0.5, w - 1, h - 1, radius)
      ctx.globalAlpha = paint.chip.fill.alpha
      ctx.fillStyle = paint.chip.fill.color
      ctx.fill()
      ctx.globalAlpha = paint.chip.ink.alpha
      ctx.font = fontOf(zone, layout.fontPx)
      ctx.fillStyle = paint.chip.ink.color
      ctx.fillText(text, PAD_X, h / 2 + 0.5, w - PAD_X * 2)
    }
  }
  ctx.globalAlpha = 1

  return { canvas, width: w, height: h, scale: RASTER_SCALE }
}

/** Left inset for the Bar header title, tracking the panel's rounded corner
 *  (bigger radius → deeper inset) so the title clears the curve. */
export function barInset(cornerRadius: number): number {
  return Math.round(cornerRadius * 0.5) + 12
}

/** Bar's hairline seam — the title-bar baseline the renderer draws full-width
 *  across the panel. NEUTRAL, matching the app's --hair language (flat with
 *  intent: surfaces separate by seam, not by a colour block). The header reads
 *  via type + this seam; there is no band (owner 2026-07-14). */
export function barSeamPaint(paint: ZonePaint): { color: string; alpha: number; width: number } {
  const light = paint.tone === 'Light'
  return {
    color: oklchToHex({ l: light ? 0.2 : 0.98, c: 0, h: 260 }),
    alpha: light ? 0.15 : 0.14,
    width: 1,
  }
}

/** Measure the title text width in desktop px (shared measuring context). */
let measureCtx: CanvasRenderingContext2D | null = null
export function measureTitle(zone: ZoneDto, fontPx: number): number {
  if (!measureCtx) measureCtx = document.createElement('canvas').getContext('2d')!
  measureCtx.font = fontOf(zone, fontPx)
  const text = zone.emoji ? `${zone.emoji} ${zone.title}` : zone.title
  const base = measureCtx.measureText(text).width
  // Bare adds the leading accent dot + gap.
  return zone.titleStyle === 'Bare' ? base + fontPx * 0.55 + 7 : base
}
