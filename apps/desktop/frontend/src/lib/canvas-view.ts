import * as React from 'react'

// One shared zoom/pan viewport model for BOTH desktop-mirror canvases (spec 04 §3.5
// "one canvas navigation model"): Ctrl+wheel zooms at the pointer, drag pans, and
// clampPan keeps content inside the stage. The hook owns fit-mode + pan; `zoom` is
// caller-controlled so each mirror wires its own zoom source — the icons store
// re-renders tiles at the new device resolution on zoom, while the paper canvas keeps
// zoom in local state. Geometry (offset + clampPan + wheel focal math) is a verbatim
// extraction of the logic that used to live inline in icons-mirror.tsx.

// 'height' = the screen's full HEIGHT fills the viewport (content pinned LEFT when
// it overflows horizontally — many users cluster icons on the left); 'width' = the
// full WIDTH fills (centered vertically); 'all' = the whole screen letterboxed.
export type FitMode = 'height' | 'width' | 'all'

/** Which axes to auto-center when content is smaller than the viewport. Icons pins
 *  content to the top (`x`, its historical behaviour); the paper canvas judges the
 *  whole desktop so it centers both axes (`xy`). */
export type CenterAxes = 'x' | 'xy'

export interface Viewport {
  w: number
  h: number
}

export interface Vec2 {
  x: number
  y: number
}

interface CanvasViewOptions {
  contentW: number
  contentH: number
  viewport: Viewport
  zoom: number
  setZoom: (zoom: number) => void
  initialFitMode?: FitMode
  center?: CenterAxes
  minZoom?: number
  maxZoom?: number
  /** CSS selector for children that must not start a pan (e.g. `[data-tile]`). */
  panIgnoreSelector?: string
}

export interface CanvasView {
  scale: number
  /** System-computed zoom bounds: the floor keeps the content's MAJOR axis filling
   *  the viewport (landscape content never narrower than the stage, portrait never
   *  shorter), capped at 1 so the default fit view is always legal. */
  minZoom: number
  maxZoom: number
  pan: Vec2
  offset: Vec2
  fitMode: FitMode
  setFitMode: (mode: FitMode) => void
  /** Attach to the canvas host: binds a native non-passive wheel listener. */
  wheelRef: (el: HTMLElement | null) => void
  /** Bundled left-drag pan for canvases whose empty space pans (the icons mirror). */
  panHandlers: {
    onPointerDown: (e: React.PointerEvent) => void
    onPointerMove: (e: React.PointerEvent) => void
    onPointerUp: (e: React.PointerEvent) => void
  }
  /** Imperative pan for canvases that multiplex the left button (the paper mirror
   *  reserves left-drag for zone creation, so it pans only on space/middle-drag). */
  beginPan: (e: { clientX: number; clientY: number }) => void
  dragPan: (e: { clientX: number; clientY: number }) => void
  endPan: () => void
  isPanning: () => boolean
  reset: (fitMode?: FitMode) => void
}

const DEFAULT_MIN_ZOOM = 0.2
const DEFAULT_MAX_ZOOM = 3

export function useCanvasView({
  contentW,
  contentH,
  viewport,
  zoom,
  setZoom,
  initialFitMode = 'height',
  center = 'x',
  minZoom = DEFAULT_MIN_ZOOM,
  maxZoom = DEFAULT_MAX_ZOOM,
  panIgnoreSelector,
}: CanvasViewOptions): CanvasView {
  const [fitMode, setFitMode] = React.useState<FitMode>(initialFitMode)
  const [rawPan, setPan] = React.useState<Vec2>({ x: 0, y: 0 })
  const dragStart = React.useRef<{ x: number; y: number; panX: number; panY: number } | null>(null)

  const baseFit =
    fitMode === 'height'
      ? viewport.h / contentH
      : fitMode === 'width'
        ? viewport.w / contentW
        : Math.min(viewport.w / contentW, viewport.h / contentH)
  const scale = (Number.isFinite(baseFit) && baseFit > 0 ? baseFit : 0.1) * zoom

  // Fill-axis floor (owner rule): landscape content may never scale below
  // viewport-width coverage; portrait content never below viewport-height coverage.
  const fillFloor =
    contentW >= contentH
      ? viewport.w / (contentW * (baseFit || 1))
      : viewport.h / (contentH * (baseFit || 1))
  const effMinZoom = Number.isFinite(fillFloor) && fillFloor > 0 ? Math.max(minZoom, Math.min(fillFloor, 1)) : minZoom
  const effMaxZoom = Math.max(maxZoom, effMinZoom)

  const contentPxW = contentW * scale
  const contentPxH = contentH * scale
  const offset: Vec2 = {
    x: Math.max(0, (viewport.w - contentPxW) / 2),
    y: center === 'xy' ? Math.max(0, (viewport.h - contentPxH) / 2) : 0,
  }

  // `pan` is the deviation from the offset-anchored position; an axis that already
  // fits (content <= viewport) locks to 0 so `offset` alone centers/pins it (never a
  // stray drift past the centered bounds). When content overflows, pan spans
  // [viewport - contentPx, 0].
  const clampAxis = (v: number, contentPx: number, viewportPx: number) =>
    Math.min(0, Math.max(v, Math.min(0, viewportPx - contentPx)))
  const clampPan = (p: Vec2): Vec2 => ({
    x: clampAxis(p.x, contentPxW, viewport.w),
    y: clampAxis(p.y, contentPxH, viewport.h),
  })
  // The APPLIED pan is clamped at render time: zooming out via slider/buttons/pinch
  // can never leave stale pan behind — the long edge re-pins to the stage, always.
  const pan = clampPan(rawPan)

  const wheelZoomAt = (px: number, py: number, factor: number) => {
    const nextZoom = Math.min(effMaxZoom, Math.max(effMinZoom, zoom * factor))
    if (nextZoom === zoom) return
    // Zoom AT the pointer: keep the content point under the cursor fixed. The content's
    // screen position is `offset + pan + point*scale`, and `offset` itself recenters as
    // scale changes — so the focal math must include offset, not just pan (else zoom
    // drifts whenever the view is centered / fit-all). Reduces to the plain
    // `px - (px-pan)*ratio` form when offset stays 0 (the zoomed-in case).
    const newScale = (Number.isFinite(baseFit) && baseFit > 0 ? baseFit : 0.1) * nextZoom
    const newContentPxW = contentW * newScale
    const newContentPxH = contentH * newScale
    const newOffX = Math.max(0, (viewport.w - newContentPxW) / 2)
    const newOffY = center === 'xy' ? Math.max(0, (viewport.h - newContentPxH) / 2) : 0
    const cx = (px - offset.x - pan.x) / scale
    const cy = (py - offset.y - pan.y) / scale
    setPan({
      x: clampAxis(px - newOffX - cx * newScale, newContentPxW, viewport.w),
      y: clampAxis(py - newOffY - cy * newScale, newContentPxH, viewport.h),
    })
    setZoom(nextZoom)
  }

  // Native, NON-passive wheel binding: React's synthetic wheel is passive, so
  // preventDefault() silently fails and trackpad pinch bubbles into browser page
  // zoom. Bound once; the handler reads live state through a ref. Pinch/Ctrl+wheel
  // zooms continuously (exp of delta — trackpad-smooth); plain two-finger scroll
  // PANS the canvas (Figma grammar).
  const live = React.useRef<{ zoomAt: typeof wheelZoomAt; panBy: (dx: number, dy: number) => void }>(null!)
  live.current = {
    zoomAt: wheelZoomAt,
    panBy: (dx, dy) => setPan(clampPan({ x: pan.x - dx, y: pan.y - dy })),
  }
  const wheelEl = React.useRef<HTMLElement | null>(null)
  const wheelRef = React.useCallback((el: HTMLElement | null) => {
    if (wheelEl.current === el) return
    if (wheelEl.current) wheelEl.current.removeEventListener('wheel', onNativeWheel.current)
    wheelEl.current = el
    if (el) el.addEventListener('wheel', onNativeWheel.current, { passive: false })
  }, [])
  const onNativeWheel = React.useRef((e: WheelEvent) => {
    e.preventDefault()
    const el = wheelEl.current
    if (!el) return
    if (e.ctrlKey) {
      const rect = el.getBoundingClientRect()
      live.current.zoomAt(e.clientX - rect.left, e.clientY - rect.top, Math.exp(-e.deltaY * 0.01))
    } else {
      live.current.panBy(e.deltaX, e.deltaY)
    }
  })
  React.useEffect(() => () => {
    if (wheelEl.current) wheelEl.current.removeEventListener('wheel', onNativeWheel.current)
  }, [])

  const beginPan = (e: { clientX: number; clientY: number }) => {
    dragStart.current = { x: e.clientX, y: e.clientY, panX: pan.x, panY: pan.y }
  }
  const dragPan = (e: { clientX: number; clientY: number }) => {
    const d = dragStart.current
    if (!d) return
    setPan(clampPan({ x: d.panX + (e.clientX - d.x), y: d.panY + (e.clientY - d.y) }))
  }
  const endPan = () => {
    dragStart.current = null
  }

  const panHandlers = {
    onPointerDown: (e: React.PointerEvent) => {
      if (e.button !== 0 || (panIgnoreSelector && (e.target as HTMLElement).closest(panIgnoreSelector))) return
      beginPan(e)
      ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    },
    onPointerMove: (e: React.PointerEvent) => {
      if (!(e.buttons & 1)) return
      dragPan(e)
    },
    onPointerUp: () => endPan(),
  }

  const reset = (mode?: FitMode) => {
    if (mode) setFitMode(mode)
    setZoom(1)
    setPan({ x: 0, y: 0 })
  }

  return {
    scale,
    minZoom: effMinZoom,
    maxZoom: effMaxZoom,
    pan,
    offset,
    fitMode,
    setFitMode,
    wheelRef,
    panHandlers,
    beginPan,
    dragPan,
    endPan,
    isPanning: () => dragStart.current !== null,
    reset,
  }
}
