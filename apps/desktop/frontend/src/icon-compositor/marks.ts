import type { IconShape, MarkStyle } from '@/bridge/types'
import type { Raster, Rgba } from './raster'
import {
  backdropBlur, boxBlur, distToSegment, fade, fromRgbInt, inTriangle, makeRaster, mix, paint,
  rgbaOf, shapeMask, shift, smoothStep01, WHITE,
} from './raster'
import { chamferDistance } from './filters'
import { drawScaled } from './sampling'

// 快捷方式标识 — 1:1 port of the frozen C# oracle (Marks/*.cs +
// ShortcutMarkRenderer.cs + ArrowGlyph.cs, ADR-0015 D3). Marks are stateless;
// the tile composer owns the z-order (behind siblings, over overlays).

/** 品牌珊瑚 — accent used when the user has not chosen a mark colour. */
export const MARK_ACCENT = 0xff6f5e

/** Mark adaptivity crossover (distinct from the 0.66 ink threshold). */
export const ADAPTIVE_THRESHOLD = 0.58

/** The owner's ratio for the styled 经典箭头 (65% of the shell footprint). */
export const STYLED_ARROW_SCALE = 0.65

export interface MarkContext {
  size: number
  shape: IconShape
  luminance: number
  markColor: number | null
  tileAlpha: Float64Array
}

const isLightTile = (ctx: MarkContext) => ctx.luminance > ADAPTIVE_THRESHOLD
const markRgb = (ctx: MarkContext) => fromRgbInt(ctx.markColor ?? MARK_ACCENT)

export interface Mark {
  placement: 'behind' | 'over'
  cardInset(ctx: MarkContext): number
  carvesCard: boolean
  carveCard(cardMask: Float64Array, ctx: MarkContext): void
  render(target: Raster, cardMask: Float64Array, ctx: MarkContext): void
}

const base = { cardInset: () => 0, carvesCard: false, carveCard: () => {} }

/** A scaled/offset stamp of the mark geometry: real shapes stamp their shape
 *  mask; free-form tiles resample the icon's OWN silhouette — the sibling a
 *  behind-mark draws is then the icon's actual outline, whatever its form. */
function stampMask(ctx: MarkContext, maskSize: number, offX: number, offY: number): Float64Array {
  if (ctx.shape !== 'None') return shapeMask(ctx.shape, ctx.size, maskSize, offX, offY)
  const size = ctx.size
  const out = new Float64Array(size * size)
  const scale = size / maskSize
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const sx = Math.round((x - offX) * scale)
      const sy = Math.round((y - offY) * scale)
      if (sx >= 0 && sy >= 0 && sx < size && sy < size) {
        out[y * size + x] = ctx.tileAlpha[sy * size + sx]
      }
    }
  }
  return out
}

/** Outside-distance (px) from a coverage field's silhouette. */
function outsideDistance(field: Float64Array, size: number): Float64Array {
  const probe = makeRaster(size)
  for (let i = 0; i < field.length; i++) probe.data[i * 4 + 3] = field[i] >= 0.5 ? 255 : 0
  return chamferDistance(probe, size, false)
}

const lerpRgba = (a: Rgba, b: Rgba, t: number): Rgba => ({
  r: Math.round(a.r + (b.r - a.r) * t),
  g: Math.round(a.g + (b.g - a.g) * t),
  b: Math.round(a.b + (b.b - a.b) * t),
  a: Math.round(a.a + (b.a - a.a) * t),
})

// ---- 投影 ShadowMark ----
// Redesigned 2026-07-09 (owner: the second layer should read as a SHADOW —
// blackish-translucent, not a solid coloured card): the tile's own silhouette
// offset down-right, blurred, neutral ink. Mark colour is deliberately
// ignored — shadows are neutral by law, and the panel hides the colour wheel
// for this style.

const shadowMark: Mark = {
  ...base,
  placement: 'behind',
  cardInset: (ctx) => Math.max(1, Math.round(ctx.size * 0.06)),
  render(target, _cardMask, ctx) {
    const size = ctx.size
    const pad = this.cardInset(ctx)
    const cardSize = size - 2 * pad
    const sil = stampMask(ctx, cardSize, pad + Math.max(1, size * 0.05), pad + Math.max(1, size * 0.06))
    const soft = boxBlur(sil, size, Math.max(1, Math.trunc(size * 0.028)))
    const ink: Rgba = { r: 8, g: 10, b: 14, a: 255 }
    for (let i = 0; i < soft.length; i++) paint(target, i, ink, soft[i] * 0.44)
  },
}

// ---- 光环 HaloMark ----
// Redesigned AGAIN 2026-07-10 (designer ruling on the owner's 歪斜 report):
// the floating hard OUTLINE band read as a dark frame, collided with Ring,
// and on asymmetric silhouettes (folder tab / Teardrop / Pebble) its uneven
// band width read as a TILT. Halo is now a TRUE outer glow: an isotropic
// distance falloff (equivalent to uniform dilate + blur) in warm white —
// no hard edge exists, so nothing can read as skewed, and Ring keeps sole
// ownership of the hard-stroke look.

const haloMark: Mark = {
  ...base,
  placement: 'behind',
  cardInset: (ctx) => Math.max(1, Math.round(ctx.size * 0.07)),
  render(target, cardMask, ctx) {
    const size = ctx.size
    const sil = ctx.shape === 'None' ? ctx.tileAlpha : cardMask
    const dist = outsideDistance(sil, size)
    const radius = Math.max(3, size * 0.1)
    const tone = ctx.markColor !== null ? fade(markRgb(ctx), 0.7) : rgbaOf(0xfffaf2, 0.7)
    for (let i = 0; i < dist.length; i++) {
      const d = dist[i]
      if (d < 0) continue
      const px = d / 3
      if (px > radius) continue
      const t = 1 - px / radius
      const a = t * t
      if (a > 0.01) paint(target, i, tone, a)
    }
  },
}

// ---- 细描边 RingMark ----

const ringStroke = (size: number) => Math.max(1, Math.round(Math.max(1.5, size * 0.03)))

const ringMark: Mark = {
  ...base,
  placement: 'behind',
  cardInset: (ctx) => ringStroke(ctx.size),
  render(target, _cardMask, ctx) {
    // Owner 2026-07-10: the thin ring defaults BLACK (a hairline frame reads
    // as chrome, not as an accent) — flipping to white on DARK tiles so the
    // delete-safety indicator never vanishes (designer adaptive amendment);
    // the coral accent only when hand-picked.
    const ring = ctx.markColor !== null ? markRgb(ctx) : isLightTile(ctx) ? fromRgbInt(0x141414) : fromRgbInt(0xf5f5f5)
    if (ctx.shape !== 'None') {
      for (let i = 0; i < ctx.tileAlpha.length; i++) paint(target, i, ring, ctx.tileAlpha[i])
      return
    }
    // Free-form: the inset trick has no visible rim, so stroke a snug band
    // around the icon's OWN silhouette instead.
    const size = ctx.size
    const dist = outsideDistance(ctx.tileAlpha, size)
    const stroke = ringStroke(size) * 1.6
    for (let i = 0; i < dist.length; i++) {
      const d = dist[i]
      if (d < 0) continue
      const a = smoothStep01((stroke - d / 3) / 0.9)
      if (a > 0.01) paint(target, i, ring, a)
    }
  },
}

// ---- 缎光角 SatinMark ----

const satinMark: Mark = {
  ...base,
  placement: 'over',
  render(target, _cardMask, ctx) {
    const size = ctx.size
    const tone = ctx.markColor !== null
      ? (isLightTile(ctx) ? mix(markRgb(ctx), fromRgbInt(0x101010), 0.62) : mix(markRgb(ctx), WHITE, 0.72))
      : (isLightTile(ctx) ? fromRgbInt(0x2a241e) : fromRgbInt(0xffffff))
    const centre = size / 2
    const dx = 0.70710678
    const dy = -0.70710678
    const length = size * 1.41421356

    const sheenAlpha = (g: number) => {
      if (g <= 0) return 0.62
      if (g <= 0.2) return 0.62 + (0.3 - 0.62) * (g / 0.2)
      if (g <= 0.46) return 0.3 * (1 - (g - 0.2) / 0.26)
      return 0
    }

    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const i = y * size + x
        const cover = ctx.tileAlpha[i]
        if (cover <= 0) continue
        const g = ((x + 0.5 - centre) * dx + (y + 0.5 - centre) * dy) / length + 0.5
        const alpha = sheenAlpha(g)
        if (alpha > 0) paint(target, i, fade(tone, alpha), cover)
      }
    }
  },
}

// ---- 珐琅光弧 ArcMark ----

const arcMark: Mark = {
  ...base,
  placement: 'over',
  render(target, _cardMask, ctx) {
    const size = ctx.size
    const arc = isLightTile(ctx)
      ? mix(markRgb(ctx), fromRgbInt(0x141414), 0.78)
      : mix(markRgb(ctx), WHITE, 0.82)
    const cx = 0.15 * size
    const cy = 0.88 * size
    let radius = 0
    for (const [px, py] of [[0, 0], [size, 0], [0, size], [size, size]]) {
      radius = Math.max(radius, Math.sqrt((px - cx) ** 2 + (py - cy) ** 2))
    }
    if (radius <= 0) radius = 1

    const glowAlpha = (d: number) => {
      if (d <= 0.2) return 1 + (0.55 - 1) * (d / 0.2)
      if (d <= 0.46) return 0.55 * (1 - (d - 0.2) / 0.26)
      return 0
    }

    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const i = y * size + x
        const cover = ctx.tileAlpha[i]
        if (cover <= 0) continue
        const d = Math.sqrt((x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2) / radius
        const alpha = glowAlpha(d)
        if (alpha > 0) paint(target, i, fade(arc, alpha), cover)
      }
    }
  },
}

// ---- 卷角 FoldMark ----

const ROOT2 = Math.SQRT2
const FOLD_START = 0.493
const CREASE_FADE = 0.02
const FOLD_BLACK: Rgba = { r: 0, g: 0, b: 0, a: 255 }

const foldDepth = (ctx: MarkContext) =>
  ctx.size * (ctx.shape === 'Apple' ? 0.26 : ctx.shape === 'Samsung' ? 0.28 : 0.3)

function foldFlapAlpha(ctx: MarkContext, c0: number, x0: number): Float64Array {
  const size = ctx.size
  const alpha = new Float64Array(size * size)
  const rr = 0.6 * c0
  for (let y = Math.trunc(x0); y < size; y++) {
    for (let x = Math.trunc(x0); x < size; x++) {
      const p = (size - (x + 0.5) + (size - (y + 0.5))) / (2 * c0)
      let a = smoothStep01((p - FOLD_START) / CREASE_FADE)
      if (a <= 0) continue
      const lx = x + 0.5 - x0
      const ly = y + 0.5 - x0
      if (lx < rr && ly < rr) {
        const d = Math.sqrt((lx - rr) ** 2 + (ly - rr) ** 2)
        a *= Math.min(1, Math.max(0, rr - d + 0.5))
      }
      alpha[y * size + x] = a
    }
  }
  return alpha
}

const foldMark: Mark = {
  ...base,
  placement: 'over',
  carvesCard: true,
  carveCard(cardMask, ctx) {
    const size = ctx.size
    const threshold = foldDepth(ctx) / ROOT2
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const proj = (size - (x + 0.5) + (size - (y + 0.5))) / ROOT2
        const removed = smoothStep01((threshold - proj) / 0.7)
        if (removed > 0) cardMask[y * size + x] *= 1 - removed
      }
    }
  },
  render(target, cardMask, ctx) {
    const size = ctx.size
    const c0 = foldDepth(ctx)
    const tone = ctx.markColor !== null
      ? (isLightTile(ctx) ? mix(markRgb(ctx), fromRgbInt(0x101010), 0.7) : mix(markRgb(ctx), WHITE, 0.78))
      : (isLightTile(ctx) ? fromRgbInt(0x3a342e) : fromRgbInt(0xf2eee6))
    const hi = mix(tone, WHITE, 0.45)
    const lo = mix(tone, FOLD_BLACK, 0.55)
    const tip = mix(tone, FOLD_BLACK, 0.76)
    const x0 = size - c0

    const flapAlpha = foldFlapAlpha(ctx, c0, x0)

    const shadow = boxBlur(
      shift(flapAlpha, size, -Math.max(1, Math.trunc(size * 0.02)), -Math.max(1, Math.trunc(size * 0.02))),
      size,
      Math.max(1, Math.trunc(size * 0.02)),
    )
    for (let i = 0; i < shadow.length; i++) {
      if (shadow[i] > 0.01 && flapAlpha[i] <= 0.01) {
        paint(target, i, FOLD_BLACK, shadow[i] * 0.3 * cardMask[i])
      }
    }

    const flapColour = (p: number): Rgba => {
      if (p <= 0.498) return lo
      if (p <= 0.57) return lerpRgba(lo, hi, (p - 0.498) / (0.57 - 0.498))
      return p <= 0.76
        ? lerpRgba(hi, tone, (p - 0.57) / (0.76 - 0.57))
        : lerpRgba(tone, tip, (p - 0.76) / (1 - 0.76))
    }

    for (let y = Math.trunc(x0); y < size; y++) {
      for (let x = Math.trunc(x0); x < size; x++) {
        const i = y * size + x
        const a = flapAlpha[i]
        if (a <= 0) continue
        const p = (size - (x + 0.5) + (size - (y + 0.5))) / (2 * c0)
        paint(target, i, flapColour(p), a * ctx.tileAlpha[i])
      }
    }
  },
}

// ---- 玻璃箭头 GlassMark ----

const glassMark: Mark = {
  ...base,
  placement: 'over',
  render(target, _cardMask, ctx) {
    const size = ctx.size
    const cs = Math.min(Math.max(16, size * 0.34), size * 0.94)
    const sx = Math.min(Math.max(0, size * 0.055), size - cs)
    const sy = Math.min(Math.max(0, size - cs - size * 0.055), size - cs)
    const cx = sx + cs / 2
    const cy = sy + cs / 2
    const seatR = cs / 2
    const lightSeat = ctx.luminance <= ADAPTIVE_THRESHOLD

    const seatBg = lightSeat ? rgbaOf(0xffffff, 0.58) : rgbaOf(0x18181c, 0.45)
    const ringLine = lightSeat ? rgbaOf(0xffffff, 0.55) : rgbaOf(0xffffff, 0.22)
    const ink = ctx.markColor !== null
      ? (lightSeat ? mix(markRgb(ctx), fromRgbInt(0x101014), 0.72) : mix(markRgb(ctx), WHITE, 0.7))
      : (lightSeat ? fromRgbInt(0x232328) : fromRgbInt(0xf4f4f1))

    const seatCov = new Float64Array(size * size)
    for (let y = Math.trunc(cy - seatR - 2); y <= cy + seatR + 2; y++) {
      if (y < 0 || y >= size) continue
      for (let x = Math.trunc(cx - seatR - 2); x <= cx + seatR + 2; x++) {
        if (x < 0 || x >= size) continue
        const dist = Math.sqrt((x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2)
        seatCov[y * size + x] = Math.min(1, Math.max(0, seatR - dist + 0.5))
      }
    }

    const blurred = backdropBlur(target, Math.max(1, Math.round(size * 0.06)))
    const td = target.data
    const bd = blurred.data
    for (let y = Math.trunc(cy - seatR - 2); y <= cy + seatR + 2; y++) {
      if (y < 0 || y >= size) continue
      for (let x = Math.trunc(cx - seatR - 2); x <= cx + seatR + 2; x++) {
        if (x < 0 || x >= size) continue
        const i = y * size + x
        const cov = seatCov[i]
        if (cov <= 0) continue
        const i4 = i * 4

        // Frost: the seat bg over the blurred backdrop, faded in by coverage.
        const frosted = overRgba(seatBg, { r: bd[i4], g: bd[i4 + 1], b: bd[i4 + 2], a: bd[i4 + 3] })
        td[i4] = Math.round(td[i4] + (frosted.r - td[i4]) * cov)
        td[i4 + 1] = Math.round(td[i4 + 1] + (frosted.g - td[i4 + 1]) * cov)
        td[i4 + 2] = Math.round(td[i4 + 2] + (frosted.b - td[i4 + 2]) * cov)
        td[i4 + 3] = Math.round(td[i4 + 3] + (frosted.a - td[i4 + 3]) * cov)

        const dist = Math.sqrt((x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2)
        const ringCov = smoothStep01((1.2 - Math.abs(dist - (seatR - 0.6))) / 1.2)
        paint(target, i, ringLine, ringCov)
      }
    }

    drawArrowGlyph(target, size, cx, cy, cs * 0.3, ink, seatCov)
  },
}

function overRgba(top: Rgba, bottom: Rgba): Rgba {
  if (top.a === 0) return bottom
  if (top.a === 255) return top
  const ta = top.a / 255
  const ba = bottom.a / 255
  const outA = ta + ba * (1 - ta)
  if (outA <= 0) return { r: 0, g: 0, b: 0, a: 0 }
  const inv = 1 / outA
  return {
    r: Math.round((top.r * ta + bottom.r * ba * (1 - ta)) * inv),
    g: Math.round((top.g * ta + bottom.g * ba * (1 - ta)) * inv),
    b: Math.round((top.b * ta + bottom.b * ba * (1 - ta)) * inv),
    a: Math.round(outA * 255),
  }
}

// ---- ArrowGlyph + classic arrow ----

/** The one NE "↗" arrow glyph (ArrowGlyph.DrawNorthEast). */
export function drawArrowGlyph(
  target: Raster, size: number, cx: number, cy: number, reach: number, ink: Rgba, clip: Float64Array,
): void {
  if (reach <= 0) return
  const tailU = -0.44
  const tailV = 0.44
  const neckU = 0.12
  const neckV = -0.12
  const tipU = 0.48
  const tipV = -0.48
  const shaftHalf = 0.135
  const headHalf = 0.28
  const perp = 0.70710678
  const headAx = neckU + perp * headHalf
  const headAy = neckV + perp * headHalf
  const headBx = neckU - perp * headHalf
  const headBy = neckV - perp * headHalf
  const soft = 1.3 / reach
  const r = Math.ceil(reach) + 2

  for (let y = Math.trunc(cy - r); y <= cy + r; y++) {
    if (y < 0 || y >= size) continue
    for (let x = Math.trunc(cx - r); x <= cx + r; x++) {
      if (x < 0 || x >= size) continue
      const i = y * size + x
      const clipCov = clip[i]
      if (clipCov <= 0) continue
      const u = (x + 0.5 - cx) / reach
      const v = (y + 0.5 - cy) / reach
      const dShaft = distToSegment(u, v, tailU, tailV, neckU, neckV)
      const cov = inTriangle(u, v, tipU, tipV, headAx, headAy, headBx, headBy)
        ? 1
        : smoothStep01((shaftHalf - dShaft) / soft)
      paint(target, i, ink, cov * clipCov)
    }
  }
}

const ARROW_PLATE: Rgba = { r: 244, g: 244, b: 241, a: 245 }
const ARROW_GLYPH: Rgba = { r: 46, g: 50, b: 56, a: 255 }

// The GENUINE Win11 shortcut-arrow badge (owner-extracted asset). When set,
// drawClassicArrow composites it exactly like C#'s system-arrow path
// (ShortcutMarkRenderer.DrawClassicArrow: box = max(8, round(size·0.65)),
// anchored bottom-left). Workers receive it at boot; the drawn approximation
// below remains only for arrow-less environments (bun tests).
let nativeArrow: Raster | null = null

export function setNativeArrowRaster(raster: Raster | null): void {
  nativeArrow = raster
}

/**
 * 经典箭头 — the real system badge when available, else the same drawn
 * approximation C# falls back to (DrawFallbackArrow).
 */
export function drawClassicArrow(target: Raster, size: number): void {
  if (nativeArrow) {
    // The owner's asset is the CROPPED badge (no transparent frame around it),
    // so it uses the fallback's footprint — ~28% of the tile at the bottom-left
    // corner — not the 0.65 full-frame scale C# applies to the shell's
    // transparent-padded overlay frame.
    const box = Math.max(14, Math.round(size * 0.28))
    drawScaled(
      nativeArrow,
      { left: 0, top: 0, right: nativeArrow.width, bottom: nativeArrow.height },
      target, size, 1, size - 1 - box, box, box,
    )
    return
  }
  const asz = Math.max(14, size * 0.28)
  const left = 1
  const top = size - 1 - asz
  const radius = Math.min(4, asz / 2)

  const plate = new Float64Array(size * size)
  for (let y = Math.trunc(top - 1); y <= top + asz + 1; y++) {
    if (y < 0 || y >= size) continue
    for (let x = Math.trunc(left - 1); x <= left + asz + 1; x++) {
      if (x < 0 || x >= size) continue
      const cov = roundedRectCoverage(x + 0.5, y + 0.5, left, top, asz, radius)
      if (cov <= 0) continue
      const i = y * size + x
      plate[i] = cov
      paint(target, i, ARROW_PLATE, cov)
    }
  }
  drawArrowGlyph(target, size, left + asz / 2, top + asz / 2, asz * 0.34, ARROW_GLYPH, plate)
}

function roundedRectCoverage(px: number, py: number, rx: number, ry: number, side: number, radius: number): number {
  const cx = rx + side / 2
  const cy = ry + side / 2
  const half = side / 2 - radius
  const qx = Math.max(Math.abs(px - cx) - half, 0)
  const qy = Math.max(Math.abs(py - cy) - half, 0)
  const d = Math.sqrt(qx * qx + qy * qy) - radius
  return Math.min(1, Math.max(0, 0.5 - d))
}

const MARKS: Record<MarkStyle, Mark> = {
  Glass: glassMark,
  Shadow: shadowMark,
  Halo: haloMark,
  Satin: satinMark,
  Arc: arcMark,
  Fold: foldMark,
  Ring: ringMark,
}

/** ShortcutMarkRenderer.Resolve — the designed mark for a style. */
export function resolveMark(style: MarkStyle): Mark {
  return MARKS[style]
}
