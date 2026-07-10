import type { ZoneTone } from '@/bridge/types'
import { rgbToOklch } from './oklch'

// Per-zone wallpaper sampling (spec 04 §4.1). The source is downsampled ONCE
// into a small luminance/colour buffer; each zone then averages its covered
// region in OKLab-ish terms. Tone decisions carry hysteresis per zone id so a
// drag across a luminance boundary doesn't strobe light/dark.

export const TONE_THRESHOLD = 0.55
export const TONE_HYSTERESIS = 0.05
/** Downsample target for the sampling buffer (long edge). */
export const SAMPLE_LONG_EDGE = 128

export interface RegionSample {
  /** Mean OKLCH lightness 0..1. */
  l: number
  /** Mean chroma. */
  c: number
  /** Circular-mean hue in degrees. */
  h: number
}

export interface SampleBuffer {
  width: number
  height: number
  /** Per-pixel {l,c,hx,hy} packed as 4 floats. */
  data: Float32Array
}

/** Build the small sampling buffer from raw RGBA (any resolution). */
export function buildSampleBuffer(rgba: Uint8ClampedArray, width: number, height: number): SampleBuffer {
  const scale = SAMPLE_LONG_EDGE / Math.max(width, height)
  const w = Math.max(1, Math.round(width * scale))
  const h = Math.max(1, Math.round(height * scale))
  const data = new Float32Array(w * h * 4)
  for (let y = 0; y < h; y++) {
    const sy = Math.min(height - 1, Math.round(((y + 0.5) / h) * height))
    for (let x = 0; x < w; x++) {
      const sx = Math.min(width - 1, Math.round(((x + 0.5) / w) * width))
      const si = (sy * width + sx) * 4
      const { l, c, h: hue } = rgbToOklch(rgba[si] / 255, rgba[si + 1] / 255, rgba[si + 2] / 255)
      const rad = (hue * Math.PI) / 180
      const di = (y * w + x) * 4
      data[di] = l
      data[di + 1] = c
      data[di + 2] = Math.cos(rad) * c // chroma-weighted hue vector
      data[di + 3] = Math.sin(rad) * c
    }
  }
  return { width: w, height: h, data }
}

/** Average a normalized region (0..1 rect in source space) of the buffer. */
export function sampleRegion(
  buf: SampleBuffer,
  nx: number,
  ny: number,
  nw: number,
  nh: number,
): RegionSample {
  const x0 = Math.max(0, Math.floor(nx * buf.width))
  const y0 = Math.max(0, Math.floor(ny * buf.height))
  const x1 = Math.min(buf.width, Math.ceil((nx + nw) * buf.width))
  const y1 = Math.min(buf.height, Math.ceil((ny + nh) * buf.height))
  let l = 0
  let c = 0
  let hx = 0
  let hy = 0
  let n = 0
  for (let y = y0; y < y1; y++) {
    for (let x = x0; x < x1; x++) {
      const i = (y * buf.width + x) * 4
      l += buf.data[i]
      c += buf.data[i + 1]
      hx += buf.data[i + 2]
      hy += buf.data[i + 3]
      n++
    }
  }
  if (n === 0) return { l: 0.5, c: 0, h: 0 }
  let h = (Math.atan2(hy / n, hx / n) * 180) / Math.PI
  if (h < 0) h += 360
  return { l: l / n, c: c / n, h }
}

/**
 * Tone decision with hysteresis: `previous` is the zone's last resolved tone
 * ('Light' | 'Dark' | null). Only flips when lightness crosses the threshold by
 * more than the hysteresis band.
 */
export function resolveTone(
  tone: ZoneTone,
  sample: RegionSample,
  previous: 'Light' | 'Dark' | null,
): 'Light' | 'Dark' {
  if (tone !== 'Auto') return tone
  if (previous === null) return sample.l >= TONE_THRESHOLD ? 'Light' : 'Dark'
  if (previous === 'Light' && sample.l < TONE_THRESHOLD - TONE_HYSTERESIS) return 'Dark'
  if (previous === 'Dark' && sample.l >= TONE_THRESHOLD + TONE_HYSTERESIS) return 'Light'
  return previous
}
