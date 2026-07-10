// Minimal HSV/hex conversions for the 调色盘 (no color library — DRY beats deps here).

export interface Hsv {
  h: number // 0-360
  s: number // 0-1
  v: number // 0-1
}

export function normalizeHex(input: string): string | null {
  let s = input.trim().replace(/^#/, '')
  if (/^[0-9a-fA-F]{3}$/.test(s)) {
    s = s.split('').map((c) => c + c).join('')
  }
  return /^[0-9a-fA-F]{6}$/.test(s) ? `#${s.toUpperCase()}` : null
}

export function hexToHsv(hex: string): Hsv {
  const n = normalizeHex(hex) ?? '#000000'
  const r = parseInt(n.slice(1, 3), 16) / 255
  const g = parseInt(n.slice(3, 5), 16) / 255
  const b = parseInt(n.slice(5, 7), 16) / 255
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const d = max - min
  let h = 0
  if (d > 0) {
    if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) * 60
    else if (max === g) h = ((b - r) / d + 2) * 60
    else h = ((r - g) / d + 4) * 60
  }
  return { h, s: max === 0 ? 0 : d / max, v: max }
}

export function hsvToHex({ h, s, v }: Hsv): string {
  const f = (n: number) => {
    const k = (n + h / 60) % 6
    const c = v - v * s * Math.max(0, Math.min(k, 4 - k, 1))
    return Math.round(c * 255)
      .toString(16)
      .padStart(2, '0')
  }
  return `#${f(5)}${f(3)}${f(1)}`.toUpperCase()
}

/** Perceived luminance 0-1 (prototype's ink law input). */
export function luminance(hex: string): number {
  const n = normalizeHex(hex) ?? '#000000'
  const r = parseInt(n.slice(1, 3), 16)
  const g = parseInt(n.slice(3, 5), 16)
  const b = parseInt(n.slice(5, 7), 16)
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255
}

/** Channel-wise sRGB mix; `t` is the weight of `b` (0 → a, 1 → b). Clamped. */
export function mix(a: string, b: string, t: number): string {
  const na = normalizeHex(a) ?? '#000000'
  const nb = normalizeHex(b) ?? '#000000'
  const w = Math.min(1, Math.max(0, t))
  const chan = (i: number) => {
    const va = parseInt(na.slice(1 + i * 2, 3 + i * 2), 16)
    const vb = parseInt(nb.slice(1 + i * 2, 3 + i * 2), 16)
    return Math.round(va + (vb - va) * w)
      .toString(16)
      .padStart(2, '0')
  }
  return `#${chan(0)}${chan(1)}${chan(2)}`.toUpperCase()
}
