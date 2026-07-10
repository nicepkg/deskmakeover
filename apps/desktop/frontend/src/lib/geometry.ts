// Icon-shape path service — the WEB twin of IconShapeGeometry's clipFor. Only used
// for chrome (logo clip, mock icons, shape swatches); real tiles arrive pre-clipped
// from the engine, so preview==bake never depends on these paths.

/**
 * A Lamé superellipse |x|^n + |y|^n = 1 as an SVG path, sampled as a polygon over a
 * `size`×`size` box (spec 02 — sufficient fidelity). `n` selects the curvature family:
 * n=5 is the Apple continuous-corner squircle; n≈4.5 the softer 圆角方 (Squircle shape).
 */
export function superellipsePath(size: number, n: number, points = 96, inset = 0): string {
  const r = (size - 2 * inset) / 2
  const c = inset + r
  const parts: string[] = []
  for (let i = 0; i < points; i++) {
    const t = (i / points) * Math.PI * 2
    const cos = Math.cos(t)
    const sin = Math.sin(t)
    const x = c + r * Math.sign(cos) * Math.abs(cos) ** (2 / n)
    const y = c + r * Math.sign(sin) * Math.abs(sin) ** (2 / n)
    parts.push(`${i === 0 ? 'M' : 'L'}${x.toFixed(2)} ${y.toFixed(2)}`)
  }
  return `${parts.join('')}Z`
}

/**
 * The Apple continuous-corner squircle as an SVG path: quintic Lamé superellipse
 * |x|^5 + |y|^5 = 1, apparent corner ≈22.37% of width (spec 02).
 * `inset` shrinks the ink inside the box (chip breathing room).
 */
export function appleSquirclePath(size: number, inset = 0): string {
  return superellipsePath(size, 5, 96, inset)
}

/** clip-path value form, resolution-independent (objectBoundingBox-like via viewBox 0 0 1 1). */
export function appleSquircleClipPath(): string {
  return `path('${appleSquirclePath(1)}')`
}
