import type { TitleSize, ZoneDto, ZoneTitleStyle } from '@/bridge/types'
import { hexToOklch, oklchToHex } from './oklch'
import type { ZonePaint } from './material'

// Title system (spec 04 §4.2, designer set 2026-07-09): four styles.
//   Chip 胶囊标签 — accent-tinted pill, overhangs the panel top (default).
//   Bare 净色标题 — accent dot + text, baked soft halo for contrast, no backing.
//   Tab  折角页签 — folder tab riding the panel's top edge (accent backing).
//   Bar  顶栏标题 — full-width in-panel header band + accent divider; always
//        reserves icon row 1 (the band is a real header).
// Layout is pure (unit-tested); rasterization draws on Canvas2D at 2×.

export const CHIP_FONT_STACK = '"HarmonyOS Sans SC", "Inter", system-ui, sans-serif'
const SIZE_FACTOR: Record<TitleSize, number> = { S: 0.17, M: 0.2, L: 0.24 }
const PAD_X = 10
const PAD_Y = 5
const TAB_PAD_X = 12
const TAB_PAD_Y = 6
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

  if (input.style === 'Bar') {
    // Full-width in-panel header; row 1 is ALWAYS reserved.
    const height = fontPx + 16
    return { x: r.left, y: r.top, width: r.width, height, fontPx, overhang: false, reserveFirstRow: true }
  }

  if (input.style === 'Tab') {
    const height = fontPx + TAB_PAD_Y * 2
    const width = Math.min(input.textWidth + TAB_PAD_X * 2, Math.max(48, r.width - 24))
    const x = r.left + input.cornerRadius + 12
    const overhang = input.clearanceAbove >= height
    return {
      x,
      // Overhang: tab bottom sits ON the panel top edge; else it sits just
      // inside the reserved first row's top (the folder metaphor survives).
      y: overhang ? r.top - height : r.top,
      width,
      height,
      fontPx,
      overhang,
      reserveFirstRow: !overhang,
    }
  }

  // Chip / Bare share the chip lanes.
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
    case 'Tab': {
      // Folder tab: top-rounded, bottom-square, accent-tinted backing.
      const tabFill = light
        ? oklchToHex({ l: 0.9, c: Math.min(accentLch.c * 0.6, 0.08), h: accentLch.h })
        : oklchToHex({ l: 0.3, c: Math.min(accentLch.c * 0.5, 0.07), h: accentLch.h })
      const ink = light
        ? oklchToHex({ l: 0.28, c: Math.min(accentLch.c * 0.5, 0.05), h: accentLch.h })
        : oklchToHex({ l: 0.97, c: 0.01, h: accentLch.h })
      ctx.globalAlpha = light ? 0.92 : 0.9
      ctx.fillStyle = tabFill
      ctx.beginPath()
      ctx.roundRect(0, 0, w, h + 8, [8, 8, 0, 0]) // bottom bleeds past the crop = square
      ctx.fill()
      ctx.globalAlpha = 1
      ctx.strokeStyle = paint.contour.alpha > 0 ? paint.contour.color : '#000000'
      ctx.globalAlpha = Math.max(paint.contour.alpha, 0.08)
      ctx.stroke()
      ctx.globalAlpha = 0.96
      ctx.font = fontOf(zone, layout.fontPx)
      ctx.fillStyle = ink
      ctx.fillText(text, TAB_PAD_X, h / 2 + 0.5, w - TAB_PAD_X * 2)
      break
    }
    case 'Bar': {
      // Band + divider are renderer-drawn (they span the panel); text only here.
      const ink = light ? oklchToHex({ l: 0.25, c: 0.01, h: accentLch.h }) : oklchToHex({ l: 0.97, c: 0.01, h: accentLch.h })
      ctx.font = fontOf(zone, layout.fontPx)
      ctx.globalAlpha = 0.98
      ctx.fillStyle = ink
      ctx.fillText(text, 14, h / 2 + 0.5, w - 28)
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

/** Bar band + accent divider colours (renderer draws them across the panel).
 *  Luminous panels skip the band's lightness step (they already gradient). */
export function barBandPaint(paint: ZonePaint): {
  band: { color: string; alpha: number } | null
  divider: { color: string; alpha: number; width: number }
} {
  const accentLch = hexToOklch(paint.accent)
  const fillLch = hexToOklch(paint.fill.color)
  const light = paint.tone === 'Light'
  return {
    band:
      paint.material === 'Luminous'
        ? null
        : {
            color: oklchToHex({ ...fillLch, l: Math.min(1, fillLch.l + (light ? 0.03 : -0.04)) }),
            alpha: Math.min(0.98, paint.fill.alpha + 0.06),
          },
    divider: {
      color: oklchToHex({ l: 0.55, c: Math.min(accentLch.c, 0.1), h: accentLch.h }),
      alpha: 0.7,
      width: 1.5,
    },
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
