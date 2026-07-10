import type { ClarityDto, WallpaperGridInfoDto } from '@/bridge/types'

// 壁纸压暗 layer (spec 04 §2.2, semantics carried over from 1.0): a gradient
// scrim toward a chosen tone that makes icon labels readable on pale wallpapers.
// Rendered as one small Canvas2D gradient the compositor stretches — cheap,
// resolution-independent, and identical logic at preview and bake scales.

const LEVEL_DIM: Record<string, number> = { Off: 0, Soft: 0.12, Strong: 0.22 }
/** Canvas authoring size (stretched by the sprite; gradients scale losslessly). */
const W = 512
const H = 288

export function scrimColor(clarity: ClarityDto, wallTint: string): string {
  switch (clarity.tone) {
    case 'Light':
      return '#FFFFFF'
    case 'Tint':
      return wallTint
    case 'Custom':
      return clarity.customScrim ?? '#101217'
    default:
      return '#101217'
  }
}

export function clarityDim(clarity: ClarityDto): number {
  if (clarity.level === 'Off') return 0
  return clarity.dimOverride ?? LEVEL_DIM[clarity.level] ?? 0
}

/** Build the scrim canvas, or null when clarity is off. */
export function clarityCanvas(
  clarity: ClarityDto,
  grid: WallpaperGridInfoDto,
  wallTint: string,
): HTMLCanvasElement | null {
  const dim = clarityDim(clarity)
  if (dim <= 0.001) return null

  const canvas = document.createElement('canvas')
  canvas.width = W
  canvas.height = H
  const ctx = canvas.getContext('2d')!
  const color = scrimColor(clarity, wallTint)

  if (clarity.gradient === 'Vignette') {
    // Vignette closes in from the corners (smoothstep 0.55→1 of center distance).
    const g = ctx.createRadialGradient(W / 2, H / 2, Math.min(W, H) * 0.32, W / 2, H / 2, Math.hypot(W, H) / 2)
    g.addColorStop(0, `${color}00`)
    g.addColorStop(0.55, `${color}00`)
    g.addColorStop(1, hexWithAlpha(color, dim))
    ctx.fillStyle = g
    ctx.fillRect(0, 0, W, H)
  } else {
    // Linear sweep from the scrimmed edge: 0°=top · 90°=right · 180°=bottom ·
    // 270°=left (engine-contract clockwise mapping, dial-verified in v1).
    const rad = (clarity.angleDeg * Math.PI) / 180
    const ux = Math.sin(rad)
    const uy = -Math.cos(rad)
    const cx = W / 2
    const cy = H / 2
    const len = (Math.abs(ux) * W + Math.abs(uy) * H) / 2
    const g = ctx.createLinearGradient(cx + ux * len, cy + uy * len, cx - ux * len * 0.2, cy - uy * len * 0.2)
    g.addColorStop(0, hexWithAlpha(color, dim))
    g.addColorStop(1, `${color}00`)
    ctx.fillStyle = g
    ctx.fillRect(0, 0, W, H)
  }

  void grid // grid reserved for the label-halo pass (F8 parity work)
  return canvas
}

function hexWithAlpha(hex: string, alpha: number): string {
  const a = Math.round(Math.min(1, Math.max(0, alpha)) * 255)
    .toString(16)
    .padStart(2, '0')
  return `${hex}${a}`
}
