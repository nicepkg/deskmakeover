// Zone editing math in HALF-CELL units — a 1:1 port of DesktopCanvasView.Zones.cs
// (owner-tuned: half-cell snapping precision, exclusive-edge resize, min 2×2 cells).
// Pure functions; bun tests mirror the C# fixtures.

export interface ZoneRect {
  cellX: number
  cellY: number
  cellsWide: number
  cellsTall: number
}

export const MIN_CELLS = 2

export const halfFloor = (v: number): number => Math.floor(v * 2) / 2
export const halfCeil = (v: number): number => Math.ceil(v * 2) / 2
export const halfRound = (v: number): number => Math.round(v * 2) / 2

const clamp = (v: number, min: number, max: number): number => Math.min(Math.max(v, min), max)

/** Pixel point → fractional cell coordinates (origin-inset space). */
export function cellOf(px: number, py: number, cellW: number, cellH: number, origin: number) {
  return { cx: (px - origin) / cellW, cy: (py - origin) / cellH }
}

/** Rubber-band create: both corners snap to the half-cell grid, min 2×2, clamped. */
export function createFromDrag(
  start: { cx: number; cy: number },
  end: { cx: number; cy: number },
  cols: number,
  rows: number,
): ZoneRect {
  const x0 = clamp(halfFloor(Math.min(start.cx, end.cx)), 0, Math.max(0, cols - MIN_CELLS))
  const y0 = clamp(halfFloor(Math.min(start.cy, end.cy)), 0, Math.max(0, rows - MIN_CELLS))
  const x1 = clamp(halfCeil(Math.max(start.cx, end.cx)), 0, cols)
  const y1 = clamp(halfCeil(Math.max(start.cy, end.cy)), 0, rows)
  return {
    cellX: x0,
    cellY: y0,
    cellsWide: clamp(Math.max(MIN_CELLS, x1 - x0), MIN_CELLS, cols - x0),
    cellsTall: clamp(Math.max(MIN_CELLS, y1 - y0), MIN_CELLS, rows - y0),
  }
}

/** Move: half-cell snap, clamped inside the grid. */
export function moveZone(zone: ZoneRect, dxCells: number, dyCells: number, cols: number, rows: number): ZoneRect {
  return {
    ...zone,
    cellX: clamp(halfRound(zone.cellX + dxCells), 0, Math.max(0, cols - zone.cellsWide)),
    cellY: clamp(halfRound(zone.cellY + dyCells), 0, Math.max(0, rows - zone.cellsTall)),
  }
}

export type HandleId = 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw'

/** Resize by handle with exclusive-edge math: the OPPOSITE edge never moves. */
export function resizeZone(
  zone: ZoneRect,
  handle: HandleId,
  dxCells: number,
  dyCells: number,
  cols: number,
  rows: number,
): ZoneRect {
  let { cellX: x, cellY: y, cellsWide: w, cellsTall: h } = zone
  const right = x + w
  const bottom = y + h

  if (handle.includes('e')) {
    w = clamp(halfRound(w + dxCells), MIN_CELLS, cols - x)
  }
  if (handle.includes('s')) {
    h = clamp(halfRound(h + dyCells), MIN_CELLS, rows - y)
  }
  if (handle.includes('w')) {
    x = clamp(halfRound(x + dxCells), 0, right - MIN_CELLS)
    w = right - x
  }
  if (handle.includes('n')) {
    y = clamp(halfRound(y + dyCells), 0, bottom - MIN_CELLS)
    h = bottom - y
  }

  return { cellX: x, cellY: y, cellsWide: w, cellsTall: h }
}

/** Arrow-key nudge (0.5 cells). */
export function nudgeZone(zone: ZoneRect, dx: number, dy: number, cols: number, rows: number): ZoneRect {
  return moveZone(zone, dx * 0.5, dy * 0.5, cols, rows)
}

/** Do two cell rects overlap (open intervals — touching edges do NOT count)? */
export function rectsOverlap(a: ZoneRect, b: ZoneRect): boolean {
  return (
    a.cellX < b.cellX + b.cellsWide &&
    a.cellX + a.cellsWide > b.cellX &&
    a.cellY < b.cellY + b.cellsTall &&
    a.cellY + a.cellsTall > b.cellY
  )
}

/**
 * Placement for [+ 添加分区] (spec 04 §2.5): scan the cell grid (row-major, 1-cell
 * steps inside a half-cell margin) for a `wide`×`tall` area that overlaps no existing
 * zone — so a new zone never stacks on the previous one at a fixed origin. When the
 * grid is full, cascade a half-cell offset from the origin (still deterministic).
 */
export function firstFreeArea(
  grid: { columns: number; rows: number },
  zones: ZoneRect[],
  wide = 6,
  tall = 4,
): ZoneRect {
  const cols = grid.columns
  const rows = grid.rows
  const w = clamp(wide, MIN_CELLS, cols)
  const h = clamp(tall, MIN_CELLS, rows)
  const margin = 0.5

  for (let y = margin; y + h <= rows; y += 1) {
    for (let x = margin; x + w <= cols; x += 1) {
      const cand: ZoneRect = { cellX: x, cellY: y, cellsWide: w, cellsTall: h }
      if (!zones.some((z) => rectsOverlap(cand, z))) return cand
    }
  }

  // Grid full: cascade so consecutive adds never perfectly overlap, clamped in-bounds.
  const step = (zones.length % 8) * 0.5
  return {
    cellX: clamp(margin + step, 0, Math.max(0, cols - w)),
    cellY: clamp(margin + step, 0, Math.max(0, rows - h)),
    cellsWide: w,
    cellsTall: h,
  }
}

// ---- Neighbour magnetism + guides (spec 04 v2.0 §3) -------------------------

export const MAGNET_CELLS = 0.35

export interface MagnetGuide {
  axis: 'x' | 'y'
  /** The aligned coordinate in cell space. */
  at: number
  /** Span of the guide along the other axis (cell space), covering both zones. */
  from: number
  to: number
}

export interface MagnetResult {
  rect: ZoneRect
  guides: MagnetGuide[]
  fired: { x: boolean; y: boolean }
}

/**
 * Edge magnetism against neighbour zones (≤ MAGNET_CELLS): same-edge alignment
 * (left↔left, right↔right, top↔top, bottom↔bottom) and adjacent tiling
 * (my left↔their right, my right↔their left, …). Runs on the RAW (unsnapped)
 * move rect — the caller half-snaps whichever axis the magnet did not claim
 * (the half-grid step is 0.5, so a post-snap magnet with a 0.35 window could
 * never fire). Neighbour edges live on the half grid, so a magnetized axis
 * stays half-grid consistent.
 */
export function magnetizeMove(rect: ZoneRect, others: ZoneRect[], cols: number, rows: number): MagnetResult {
  const best = { dx: 0, dy: 0 }
  let bestDx = MAGNET_CELLS + 1
  let bestDy = MAGNET_CELLS + 1
  const right = rect.cellX + rect.cellsWide
  const bottom = rect.cellY + rect.cellsTall

  for (const o of others) {
    const oRight = o.cellX + o.cellsWide
    const oBottom = o.cellY + o.cellsTall
    // Candidate x-alignments: [myEdge, targetCoord]
    const xCands: [number, number][] = [
      [rect.cellX, o.cellX], [right, oRight], [rect.cellX, oRight], [right, o.cellX],
    ]
    for (const [edge, target] of xCands) {
      const d = target - edge
      if (Math.abs(d) <= MAGNET_CELLS && Math.abs(d) < Math.abs(bestDx)) {
        bestDx = d
        best.dx = d
      }
    }
    const yCands: [number, number][] = [
      [rect.cellY, o.cellY], [bottom, oBottom], [rect.cellY, oBottom], [bottom, o.cellY],
    ]
    for (const [edge, target] of yCands) {
      const d = target - edge
      if (Math.abs(d) <= MAGNET_CELLS && Math.abs(d) < Math.abs(bestDy)) {
        bestDy = d
        best.dy = d
      }
    }
  }

  const fired = { x: Math.abs(bestDx) <= MAGNET_CELLS, y: Math.abs(bestDy) <= MAGNET_CELLS }
  const snapped: ZoneRect = {
    ...rect,
    cellX: Math.min(Math.max(rect.cellX + (fired.x ? best.dx : 0), 0), Math.max(0, cols - rect.cellsWide)),
    cellY: Math.min(Math.max(rect.cellY + (fired.y ? best.dy : 0), 0), Math.max(0, rows - rect.cellsTall)),
  }
  return { rect: snapped, guides: alignmentGuides(snapped, others), fired }
}

/** Which edges of `rect` currently align with a neighbour → span guide lines. */
export function alignmentGuides(rect: ZoneRect, others: ZoneRect[]): MagnetGuide[] {
  const guides: MagnetGuide[] = []
  const right = rect.cellX + rect.cellsWide
  const bottom = rect.cellY + rect.cellsTall
  const eps = 0.01
  for (const o of others) {
    const oRight = o.cellX + o.cellsWide
    const oBottom = o.cellY + o.cellsTall
    for (const x of [rect.cellX, right]) {
      if (Math.abs(x - o.cellX) < eps || Math.abs(x - oRight) < eps) {
        guides.push({ axis: 'x', at: x, from: Math.min(rect.cellY, o.cellY), to: Math.max(bottom, oBottom) })
      }
    }
    for (const y of [rect.cellY, bottom]) {
      if (Math.abs(y - o.cellY) < eps || Math.abs(y - oBottom) < eps) {
        guides.push({ axis: 'y', at: y, from: Math.min(rect.cellX, o.cellX), to: Math.max(right, oRight) })
      }
    }
  }
  return guides
}

/** Intersections of `rect` with other zones (warn-wash regions, cell space). */
export function overlapRegions(rect: ZoneRect, others: ZoneRect[]): ZoneRect[] {
  const out: ZoneRect[] = []
  for (const o of others) {
    if (!rectsOverlap(rect, o)) continue
    const x = Math.max(rect.cellX, o.cellX)
    const y = Math.max(rect.cellY, o.cellY)
    out.push({
      cellX: x,
      cellY: y,
      cellsWide: Math.min(rect.cellX + rect.cellsWide, o.cellX + o.cellsWide) - x,
      cellsTall: Math.min(rect.cellY + rect.cellsTall, o.cellY + o.cellsTall) - y,
    })
  }
  return out
}

/** Clamp a persisted zone into the current grid (环境变化 fallback). */
export function clampZone(zone: ZoneRect, cols: number, rows: number): ZoneRect {
  const w = clamp(zone.cellsWide, MIN_CELLS, cols)
  const h = clamp(zone.cellsTall, MIN_CELLS, rows)
  return {
    cellsWide: w,
    cellsTall: h,
    cellX: clamp(zone.cellX, 0, Math.max(0, cols - w)),
    cellY: clamp(zone.cellY, 0, Math.max(0, rows - h)),
  }
}

/**
 * Ghost slot cells (editor-only, never baked): INTEGER global-grid cells whose
 * full cell lies inside the zone body. Spec 04 v2.0: the title chip OVERHANGS
 * the panel top, so row 1 is usable icon space — but when the chip falls back
 * to the IN-PANEL lane (zone flush to the screen top / stacked under a
 * neighbour) the chip occupies row 1's headroom, so `reserveFirstRow` shifts
 * the spread down one row (owner call 2026-07-09). The spread is a FULL first
 * row + half second row (3..12).
 */
export function ghostCells(zone: ZoneRect, reserveFirstRow = false): { col: number; row: number }[] {
  const firstCol = Math.ceil(zone.cellX)
  const lastCol = Math.floor(zone.cellX + zone.cellsWide) - 1
  const firstRow = Math.ceil(zone.cellY) + (reserveFirstRow ? 1 : 0)
  const lastRow = Math.floor(zone.cellY + zone.cellsTall) - 1
  const cols = lastCol - firstCol + 1
  const rows = lastRow - firstRow + 1
  if (cols <= 0 || rows <= 0) return []

  const target = Math.min(Math.max(3, Math.ceil(cols * 1.5)), 12, cols * rows)
  const out: { col: number; row: number }[] = []
  for (let row = firstRow; row <= lastRow && out.length < target; row++) {
    for (let col = firstCol; col <= lastCol && out.length < target; col++) {
      out.push({ col, row })
    }
  }
  return out
}
