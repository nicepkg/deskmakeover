import * as React from 'react'
import { motion, useAnimationControls, useReducedMotion } from 'motion/react'
import type { WallpaperGridInfoDto, ZoneDto } from '@/bridge/types'
import { ghostCells } from '@/lib/zone-math'
import type { HandleId, MagnetGuide, ZoneRect } from '@/lib/zone-math'
import { snapPulse } from '@/lib/motion'
import { appleSquirclePath } from '@/lib/geometry'

// Editor CHROME for the wallpaper zone editor (spec 04 v2.0 §3). The zone
// MATERIAL (frost/fill/chip/title) is painted by the compositor underneath —
// these pieces render only what never bakes: selection, handles, ghost icons,
// the rename editor and the create rubber-band. Everything lives in
// DESKTOP-PIXEL space (the parent applies zoom/pan), so screen-constant sizes
// divide by `scale`.

type SnapControls = ReturnType<typeof useAnimationControls>

const HANDLES: HandleId[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']
const CORNER_HANDLES: HandleId[] = ['nw', 'ne', 'se', 'sw']

const cursorFor = (h: HandleId): string =>
  h === 'n' || h === 's'
    ? 'ns-resize'
    : h === 'e' || h === 'w'
      ? 'ew-resize'
      : h === 'ne' || h === 'sw'
        ? 'nesw-resize'
        : 'nwse-resize'

/** Alignment guides: ONLY the edges currently magnet-snapped light up (spec 04
 *  v2.0 §3 — the full-grid overlay is gone). Coral dashed core + a 1px white
 *  companion so the line reads on dark art too. */
export function MagnetGuideLines({
  guides,
  grid,
  scale,
}: {
  guides: MagnetGuide[]
  grid: WallpaperGridInfoDto
  scale: number
}) {
  if (guides.length === 0) return null
  const w = 1 / scale
  return (
    <svg className="pointer-events-none absolute inset-0 size-full" aria-hidden>
      {guides.map((g, i) => {
        const at = grid.inset + g.at * (g.axis === 'x' ? grid.cellWidth : grid.cellHeight)
        const from = grid.inset + g.from * (g.axis === 'x' ? grid.cellHeight : grid.cellWidth)
        const to = grid.inset + g.to * (g.axis === 'x' ? grid.cellHeight : grid.cellWidth)
        const [x1, y1, x2, y2] = g.axis === 'x' ? [at, from, at, to] : [from, at, to, at]
        return (
          <g key={i}>
            <line x1={x1} y1={y1} x2={x2} y2={y2} stroke="rgba(255,255,255,0.85)" strokeWidth={w * 2.5} />
            <line x1={x1} y1={y1} x2={x2} y2={y2} stroke="#FF6F5E" strokeWidth={w} strokeDasharray={`${5 * w} ${4 * w}`} />
          </g>
        )
      })}
    </svg>
  )
}

/** Overlap warn-wash: overlapping regions wear a coral wash during the gesture
 *  (allowed but discouraged — spec 04 v2.0 §3). */
export function OverlapWash({ regions, grid }: { regions: ZoneRect[]; grid: WallpaperGridInfoDto }) {
  if (regions.length === 0) return null
  return (
    <>
      {regions.map((r, i) => (
        <div
          key={i}
          className="pointer-events-none absolute bg-coral/[0.12] ring-1 ring-inset ring-coral/30"
          style={{
            left: grid.inset + r.cellX * grid.cellWidth,
            top: grid.inset + r.cellY * grid.cellHeight,
            width: r.cellsWide * grid.cellWidth,
            height: r.cellsTall * grid.cellHeight,
          }}
        />
      ))}
    </>
  )
}

interface ZoneViewProps {
  zone: ZoneDto
  grid: WallpaperGridInfoDto
  scale: number
  isSelected: boolean
  ghostSize: number
  panelInset: number
  /** Ghost/label ink derived from the zone's RESOLVED material tone (compositor
   *  ZoneMeta): light panel → near-black, dark panel → near-white. */
  ink: string
  /** Title rides the gutter lane above the panel top (rename band follows it). */
  titleOverhang: boolean
  /** Ghost slots skip the zone's first row (in-panel title / header bar). */
  reserveFirstRow: boolean
  snapControls: SnapControls
  renaming: boolean
  renameValue: string
  onMoveDown: (e: React.PointerEvent, id: string) => void
  onResizeDown: (e: React.PointerEvent, id: string, handle: HandleId) => void
  onTitleDoubleClick: (id: string) => void
  onRenameChange: (value: string) => void
  onRenameCommit: () => void
  onRenameCancel: () => void
}

/**
 * Ghost SLOT (designer spec 2026-07-09): a drawn landing slot — outlined
 * squircle + ONE abstract mark + a label baseline — never a fake real icon.
 * All alphas are baked per element (no blanket group opacity); everything
 * derives from `currentColor` (the panel-tone ink). One svg per tile.
 */
const GhostSlot = React.memo(function GhostSlot({
  size,
  hair,
  glyph,
  labelW,
  alpha,
  rise,
  delay,
  reduced,
}: {
  size: number
  hair: number
  glyph: number
  labelW: number
  alpha: number
  rise: number
  delay: number
  reduced: boolean
}) {
  const labelH = Math.max(hair * 2, size * 0.055)
  const gap = size * 0.14
  const height = size + gap + labelH
  return (
    <motion.svg
      width={size}
      height={height}
      viewBox={`0 0 ${size} ${height}`}
      className="pointer-events-none"
      initial={reduced ? { opacity: 0 } : { opacity: 0, y: rise, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={reduced ? { duration: 0.12 } : { duration: 0.22, ease: [0.33, 1, 0.68, 1], delay }}
      aria-hidden
    >
      <path
        d={appleSquirclePath(size)}
        fill="currentColor"
        fillOpacity={0.05 * alpha}
        stroke="currentColor"
        strokeOpacity={0.22 * alpha}
        strokeWidth={hair}
      />
      {glyph === 0 && <circle cx={size / 2} cy={size / 2} r={size * 0.13} fill="currentColor" fillOpacity={0.3 * alpha} />}
      {glyph === 1 && (
        <rect
          x={size * 0.29}
          y={size * 0.44}
          width={size * 0.42}
          height={size * 0.12}
          rx={size * 0.06}
          fill="currentColor"
          fillOpacity={0.26 * alpha}
        />
      )}
      {glyph === 2 && (
        <rect
          x={size * 0.35}
          y={size * 0.35}
          width={size * 0.3}
          height={size * 0.3}
          rx={size * 0.084}
          fill="currentColor"
          fillOpacity={0.28 * alpha}
        />
      )}
      <rect
        x={(size - size * labelW) / 2}
        y={size + gap}
        width={size * labelW}
        height={labelH}
        rx={labelH / 2}
        fill="currentColor"
        fillOpacity={0.3 * alpha}
      />
    </motion.svg>
  )
})

export const ZoneView = React.memo(function ZoneView({
  zone,
  grid,
  scale,
  isSelected,
  ghostSize,
  panelInset,
  ink,
  titleOverhang,
  reserveFirstRow,
  snapControls,
  renaming,
  renameValue,
  onMoveDown,
  onResizeDown,
  onTitleDoubleClick,
  onRenameChange,
  onRenameCommit,
  onRenameCancel,
}: ZoneViewProps) {
  const reduced = useReducedMotion() ?? false
  const r = {
    left: grid.inset + zone.cellX * grid.cellWidth,
    top: grid.inset + zone.cellY * grid.cellHeight,
    width: zone.cellsWide * grid.cellWidth,
    height: zone.cellsTall * grid.cellHeight,
  }
  // Corner-only handles under 5-cell zones (Figma grammar, spec 04 §3).
  const handles = Math.min(zone.cellsWide, zone.cellsTall) >= 5 ? HANDLES : CORNER_HANDLES

  const ghosts = ghostCells(
    { cellX: zone.cellX, cellY: zone.cellY, cellsWide: zone.cellsWide, cellsTall: zone.cellsTall },
    reserveFirstRow, // in-panel title lane / header bar → row 1 reserved
  )
  const firstGhostRow = ghosts[0]?.row ?? 0
  const hair = 1 / scale

  return (
    <motion.div
      data-zone
      className="absolute cursor-move"
      style={{ left: r.left, top: r.top, width: r.width, height: r.height }}
      variants={snapPulse}
      initial={false}
      animate={isSelected ? snapControls : undefined}
      exit={
        // Delete exit (spec 04 §3): 140ms, in step with the compositor's
        // material alpha fade — the two layers leave together.
        reduced
          ? { opacity: 0, transition: { duration: 0.14 } }
          : { opacity: 0, scale: 0.94, transition: { duration: 0.14, ease: [0.4, 0, 1, 1] } }
      }
      onPointerDown={(e) => onMoveDown(e, zone.id)}
    >
      {/* Ghost landing SLOTS (editor-only, never baked): keyed by INDEX so a
          drag repositions without remounting (no animation replay mid-gesture);
          alpha ramps down per row to whisper "more fits below". */}
      <span className="pointer-events-none" style={{ color: ink }}>
        {ghosts.map(({ col, row }, i) => (
          <span
            key={i}
            className="absolute flex justify-center"
            style={{
              left: grid.inset + col * grid.cellWidth - r.left,
              top: grid.inset + row * grid.cellHeight - r.top + 6,
              width: grid.cellWidth,
            }}
          >
            <GhostSlot
              size={ghostSize}
              hair={hair}
              glyph={(col * 2 + row) % 3}
              labelW={(col + row) % 2 ? 0.55 : 0.42}
              alpha={Math.max(0.5, 0.82 ** (row - firstGhostRow))}
              rise={4 / scale}
              delay={reduced ? 0 : i * 0.026}
              reduced={reduced}
            />
          </span>
        ))}
      </span>

      {/* Rename affordance: the chip lane — overhang or in-panel, matching where
          the compositor put the chip. Double-click edits in place; the DOM input
          replaces the compositor chip while open. */}
      <div
        data-zone
        className="absolute"
        style={{
          left: panelInset,
          right: panelInset,
          top: titleOverhang ? -grid.cellHeight * 0.25 : 0,
          height: grid.cellHeight * 0.66,
        }}
        onDoubleClick={(e) => {
          e.stopPropagation()
          onTitleDoubleClick(zone.id)
        }}
      >
        {renaming && (
          <input
            autoFocus
            value={renameValue}
            onChange={(e) => onRenameChange(e.currentTarget.value)}
            onPointerDown={(e) => e.stopPropagation()}
            onFocus={(e) => e.currentTarget.select()}
            onBlur={onRenameCommit}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                onRenameCommit()
              } else if (e.key === 'Escape') {
                e.preventDefault()
                onRenameCancel()
              }
              e.stopPropagation()
            }}
            className="absolute rounded-[8px] bg-black/60 px-2.5 text-white outline-none ring-1 ring-coral"
            style={{
              left: zone.cornerRadius * 0.5 + 14 - panelInset,
              top: titleOverhang ? grid.cellHeight * 0.25 - 14 : 8,
              fontSize: Math.min(22, Math.max(15, grid.cellHeight * 0.2)),
              height: 30,
              minWidth: 120,
            }}
          />
        )}
      </div>

      {isSelected && (
        <>
          {/* Double-stroke selection: coral core + white halo (readable on any art). */}
          <div
            className="pointer-events-none absolute border-[1.5px] border-coral/95"
            style={{
              inset: 0,
              borderRadius: Math.max(0, zone.cornerRadius),
              boxShadow: '0 0 0 1px rgba(255,255,255,0.9), inset 0 0 0 1px rgba(255,255,255,0.35)',
            }}
          />
          {handles.map((h) => {
            const hit = 20 / scale
            const half = hit / 2
            const pos: React.CSSProperties = {}
            if (h.includes('n')) pos.top = -half
            if (h.includes('s')) pos.bottom = -half
            if (h.includes('w')) pos.left = -half
            if (h.includes('e')) pos.right = -half
            if (h === 'n' || h === 's') {
              pos.left = '50%'
              pos.marginLeft = -half
            }
            if (h === 'e' || h === 'w') {
              pos.top = '50%'
              pos.marginTop = -half
            }
            return (
              <span
                key={h}
                data-zone
                onPointerDown={(e) => onResizeDown(e, zone.id, h)}
                className="absolute grid place-items-center"
                style={{ ...pos, width: hit, height: hit, cursor: cursorFor(h) }}
              >
                {/* White-core rounded square + coral ring + soft shadow (spec 04 §3). */}
                <span
                  className="block bg-white"
                  style={{
                    width: 10 / scale,
                    height: 10 / scale,
                    borderRadius: 3 / scale,
                    boxShadow: `0 0 0 ${1.25 / scale}px #FF6F5E, 0 ${1 / scale}px ${3 / scale}px rgba(0,0,0,0.25)`,
                  }}
                />
              </span>
            )
          })}
        </>
      )}
    </motion.div>
  )
})

/** The live rubber-band during create — the SNAPPED span. The forming material is
 *  painted by the compositor via a provisional zone; this outline signals "drawing". */
export function RubberBand({ span, grid }: { span: ZoneRect; grid: WallpaperGridInfoDto }) {
  const width = span.cellsWide * grid.cellWidth
  const height = span.cellsTall * grid.cellHeight
  return (
    <div
      className="pointer-events-none absolute border border-dashed border-coral/80"
      style={{
        left: grid.inset + span.cellX * grid.cellWidth,
        top: grid.inset + span.cellY * grid.cellHeight,
        width,
        height,
      }}
    >
      <span className="absolute -bottom-6 right-0 rounded-[6px] bg-black/60 px-1.5 py-0.5 font-mono text-[11px] text-white">
        {span.cellsWide} × {span.cellsTall}
      </span>
    </div>
  )
}

