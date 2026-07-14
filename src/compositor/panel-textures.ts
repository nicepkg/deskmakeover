import { Texture } from 'pixi.js'

// Procedural panel textures (spec 04 §4.1 round 3): tiny Canvas2D tiles,
// repeat-tiled at PIXEL scale so grain/line density is zone-size independent
// and bake parity holds (same texture at k=1). Each tile encodes its own
// per-pixel alpha; zone-node draws it as a second fill pass over the body.
//
//   noise — Paper 素笺 grain: monochrome random, drawn at a low master alpha.
//   flute — Fluted 棱纹玻璃: vertical soft-cosine ribs (bright ridge line,
//           dark valley) that slice the blurred wallpaper into light bands.
//   brush — Brushed 拉丝金属: dense horizontal streaks (2px period, low
//           contrast, per-pixel jitter) reading as anisotropic brushing.

export type PanelTextureKind = 'noise' | 'flute' | 'brush'

const cache = new Map<PanelTextureKind, Texture>()

/** Brushed sheen — NOT a repeat tile: one soft ~20° white light band baked
 *  into a 256² canvas at peak alpha 1 (Canvas2D gradients honour alpha
 *  reliably); zone-node stretches it over the panel via textureSpace 'local'
 *  and scales it with the paint's sheen alpha. */
let sheenTex: Texture | null = null
export function sheenTexture(): Texture {
  if (sheenTex) return sheenTex
  const size = 256
  const canvas = document.createElement('canvas')
  canvas.width = size
  canvas.height = size
  const ctx2 = canvas.getContext('2d')!
  const g = ctx2.createLinearGradient(0, size * 0.1, size * 0.94, size * 0.44)
  g.addColorStop(0.08, 'rgba(255,255,255,0)')
  g.addColorStop(0.35, 'rgba(255,255,255,1)')
  g.addColorStop(0.62, 'rgba(255,255,255,0)')
  ctx2.fillStyle = g
  ctx2.fillRect(0, 0, size, size)
  sheenTex = Texture.from(canvas)
  return sheenTex
}

/** Fluted rib period in px (soft cosine: ridge center bright → valley dark).
 *  Owner-calmed 2026-07-15: zones are ICON CONTAINERS — the first cut (12px
 *  ribs at α0.12) dazzled under icons ("眼花缭乱"). Wide flutes at whisper
 *  alpha keep the light-band innovation while the panel stays a quiet stage. */
const FLUTE_PERIOD = 28
const FLUTE_RIDGE_ALPHA = 0.065
const FLUTE_VALLEY_ALPHA = 0.032

export function panelTexture(kind: PanelTextureKind): Texture {
  const hit = cache.get(kind)
  if (hit) return hit
  const canvas = document.createElement('canvas')
  const ctx2 = canvas.getContext('2d')!
  if (kind === 'noise') {
    const size = 128
    canvas.width = size
    canvas.height = size
    const img = ctx2.createImageData(size, size)
    for (let i = 0; i < img.data.length; i += 4) {
      const v = Math.floor(Math.random() * 256)
      img.data[i] = v
      img.data[i + 1] = v
      img.data[i + 2] = v
      img.data[i + 3] = 255
    }
    ctx2.putImageData(img, 0, 0)
  } else if (kind === 'flute') {
    canvas.width = FLUTE_PERIOD
    canvas.height = 8
    const img = ctx2.createImageData(FLUTE_PERIOD, 8)
    for (let x = 0; x < FLUTE_PERIOD; x++) {
      // cos profile across one rib: +1 at the ridge center, -1 in the valley.
      const c = Math.cos((x / FLUTE_PERIOD) * Math.PI * 2)
      const white = c > 0
      const a = Math.round((white ? c * FLUTE_RIDGE_ALPHA : -c * FLUTE_VALLEY_ALPHA) * 255)
      const v = white ? 255 : 0
      for (let y = 0; y < 8; y++) {
        const i = (y * FLUTE_PERIOD + x) * 4
        img.data[i] = v
        img.data[i + 1] = v
        img.data[i + 2] = v
        img.data[i + 3] = a
      }
    }
    ctx2.putImageData(img, 0, 0)
  } else {
    // brush: 128×2 — row 0 a light streak, row 1 a dark one, alpha jittered
    // along x so the tiling reads as brushed streaks, not printed stripes.
    const w = 128
    canvas.width = w
    canvas.height = 2
    const img = ctx2.createImageData(w, 2)
    for (let x = 0; x < w; x++) {
      const jitter = () => Math.round((0.03 + Math.random() * 0.04) * 255)
      let i = x * 4 // row 0: white
      img.data[i] = 255
      img.data[i + 1] = 255
      img.data[i + 2] = 255
      img.data[i + 3] = jitter()
      i = (w + x) * 4 // row 1: black
      img.data[i] = 0
      img.data[i + 1] = 0
      img.data[i + 2] = 0
      img.data[i + 3] = jitter()
    }
    ctx2.putImageData(img, 0, 0)
  }
  const tex = Texture.from(canvas)
  tex.source.addressMode = 'repeat'
  cache.set(kind, tex)
  return tex
}
