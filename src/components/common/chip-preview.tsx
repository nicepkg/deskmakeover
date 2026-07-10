import { useId } from 'react'
import type { ReactNode } from 'react'
import type { IconKindBucket, IconShape, MarkStyle } from '@/bridge/types'
const winNativeArrow = '/win-native-arrow.png'
import { clipPathFor } from '@/lib/shape-paths'
import { cn } from '@/lib/utils'

// Live chip previews (spec 02 §Geometry) — presentational only, monochrome via
// currentColor, coral on active; chrome, never WYSIWYG.
//
// ONE KEYLINE for every axis glyph (owner law: identical size across the
// shape/filter/mark rows, 无 and roadmap slots included): art is authored on a
// 20px grid with a 16px motif extent, and every glyph renders at GLYPH px —
// the vector scales as one block, so the ink extent is GLYPH×0.8 everywhere.
// No optical exceptions — the owner chose mathematical uniformity over
// icon-grid optics; sized up 20→25 on the owner's legibility call (2026-07-09).

/** Rendered canvas for every axis glyph; ink extent = GLYPH × 0.8 (= 20px).
 *  MUST stay = shape-paths SWATCH ÷ 0.8 — the solid ShapeSwatch box and the
 *  SVG ink extent are the same keyline. */
const GLYPH = 25

/** 10px filled colour dot for 配色 chips. */
export function ColorDot({ color, className }: { color: string; className?: string }) {
  return (
    <span
      aria-hidden="true"
      className={cn('size-2.5 shrink-0 rounded-full ring-1 ring-black/10', className)}
      style={{ background: color }}
    />
  )
}

/**
 * THE 「无」 glyph — one dialect for every axis's none option (shape/filter/mark),
 * always first in its row: a slash-circle, the design-tool "no fill" convention.
 * Never dashed — dashed already means 自动 in the colour dialect (AutoDot).
 */
export function NoneGlyph({
  active,
  className,
}: {
  active?: boolean
  className?: string
}) {
  return (
    <span aria-hidden="true" className={cn('inline-flex shrink-0', active ? 'text-coral' : 'text-t2', className)}>
      <svg width={GLYPH} height={GLYPH} viewBox="0 0 20 20" fill="none">
        <circle cx="10" cy="10" r="7.25" stroke="currentColor" strokeWidth="1.5" />
        <line x1="4.9" y1="4.9" x2="15.1" y2="15.1" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    </span>
  )
}

/** Concentric fg/bg pair swatch (owner grammar 2026-07-09): the OUTER disc is
 *  the background/plate colour, the INNER dot the subject colour — the swatch
 *  IS a preview of the pairing. Literal colours, never tinted by selection;
 *  the SwatchButton wash carries the selected state. */
/** Quick perceived lightness of a #RRGGBB hex (0..1) — swatch-ring weighting
 *  only, no colour science needed at 20px. */
function hexLightness(hex: string): number {
  const v = parseInt(hex.replace('#', ''), 16)
  return (0.299 * ((v >> 16) & 255) + 0.587 * ((v >> 8) & 255) + 0.114 * (v & 255)) / 255
}

export function PairDot({ fg, bg, className }: { fg: string; bg: string; className?: string }) {
  // Light-on-light pairs (the white mono swatch) read as EMPTY rings on a
  // white panel (designer must-fix 2026-07-10) — the ring firms up and the
  // inner dot gains its own hairline so the chip always reads solid.
  const light = hexLightness(bg) > 0.82
  const fgLight = hexLightness(fg) > 0.82
  return (
    <span aria-hidden="true" className={cn('inline-flex shrink-0 text-t2', className)}>
      <svg width={GLYPH} height={GLYPH} viewBox="0 0 20 20" fill="none">
        <circle cx="10" cy="10" r="8" fill={bg} />
        <circle cx="10" cy="10" r="4.4" fill={fg} />
        {fgLight && <circle cx="10" cy="10" r="4.15" stroke="currentColor" strokeWidth="0.9" opacity="0.5" />}
        <circle cx="10" cy="10" r="7.75" stroke="currentColor" strokeWidth="1" opacity={light ? 0.55 : 0.35} />
      </svg>
    </span>
  )
}

/** 黑白 — spoken in the concentric pair grammar (owner call 2026-07-09: no
 *  half/half DISC), but the inner SUBJECT dot is split black|white so the
 *  chip cannot be mistaken for the pure-black mono swatch (designer
 *  must-fix 2026-07-10: the black/white/black triple was unreadable). */
export function BwGlyph({ className }: { className?: string }) {
  return (
    <span aria-hidden="true" className={cn('inline-flex shrink-0 text-t2', className)}>
      <svg width={GLYPH} height={GLYPH} viewBox="0 0 20 20" fill="none">
        <circle cx="10" cy="10" r="8" fill="#FFFFFF" />
        <path d="M 10 5.6 A 4.4 4.4 0 0 0 10 14.4 Z" fill="#141414" />
        <path d="M 10 5.6 A 4.4 4.4 0 0 1 10 14.4 Z" fill="#F5F5F3" />
        <circle cx="10" cy="10" r="4.4" stroke="currentColor" strokeWidth="0.9" opacity="0.45" />
        <circle cx="10" cy="10" r="7.75" stroke="currentColor" strokeWidth="1" opacity="0.45" />
      </svg>
    </span>
  )
}

/** 随图标 (ADR-0018 plate axis first stop) — a four-quadrant derived board:
 *  four REAL harmony-band plate tones with hairline seams, reading as "one
 *  plate, four different derivations" — an algorithm, not a colour. Band-
 *  aware: Quiet swaps to the pastel envelope. Must never be confusable
 *  with the 原彩 trio dots (three floating dots vs one quartered board). */
export function QuadPlateGlyph({ band = 'Vivid', className }: { band?: 'Vivid' | 'Quiet'; className?: string }) {
  // FIELD_SLOTS-true tones (L0.87 C~0.1 / L0.91 C~0.05): coral/amber/teal/lavender.
  const q = band === 'Quiet'
    ? ['#F6E3DE', '#F3EBD9', '#DFEDE9', '#E3EDDD']
    : ['#F6C9BE', '#EFD9AE', '#BFE3DA', '#CBE2C2']
  return (
    <span aria-hidden="true" className={cn('inline-flex shrink-0 text-t2', className)}>
      <svg width={GLYPH} height={GLYPH} viewBox="0 0 20 20" fill="none">
        <clipPath id="quadplate-clip"><rect x="1.5" y="1.5" width="17" height="17" rx="5" /></clipPath>
        <g clipPath="url(#quadplate-clip)">
          <rect x="1.5" y="1.5" width="8.5" height="8.5" fill={q[0]} />
          <rect x="10" y="1.5" width="8.5" height="8.5" fill={q[1]} />
          <rect x="1.5" y="10" width="8.5" height="8.5" fill={q[2]} />
          <rect x="10" y="10" width="8.5" height="8.5" fill={q[3]} />
          <path d="M10 1.5 V18.5 M1.5 10 H18.5" stroke="#FFFFFF" strokeWidth="1" opacity="0.9" />
        </g>
        <rect x="1.5" y="1.5" width="17" height="17" rx="5" stroke="currentColor" strokeWidth="1" opacity="0.35" />
      </svg>
    </span>
  )
}

/** 本色 (ADR-0018 plate fifth stop): anchors-else-white — half an anchored
 *  brand board, half the white fallback, split by a hairline diagonal. */
export function FaithfulGlyph({ className }: { className?: string }) {
  return (
    <span aria-hidden="true" className={cn('inline-flex shrink-0 text-t2', className)}>
      <svg width={GLYPH} height={GLYPH} viewBox="0 0 20 20" fill="none">
        <clipPath id="faithful-clip"><rect x="1.5" y="1.5" width="17" height="17" rx="5" /></clipPath>
        <g clipPath="url(#faithful-clip)">
          <rect x="1.5" y="1.5" width="17" height="17" fill="#FFFFFF" />
          <path d="M1.5 1.5 H18.5 L1.5 18.5 Z" fill="#E8836F" />
          <rect x="4.2" y="4.2" width="5.2" height="5.2" rx="1.6" fill="#FFFFFF" opacity="0.92" />
        </g>
        <rect x="1.5" y="1.5" width="17" height="17" rx="5" stroke="currentColor" strokeWidth="1" opacity="0.4" />
      </svg>
    </span>
  )
}

/** 满彩 Field (ADR-0016 default) — a trio of hue dots: every icon keeps its
 *  own colour on a coloured plate. Dots reuse brand-approved hues only
 *  (coral / 湖水 teal / 琥珀 amber — no banned families). */
export function FieldGlyph({ className }: { className?: string }) {
  return (
    <span aria-hidden="true" className={cn('inline-flex shrink-0 text-t2', className)}>
      <svg width={GLYPH} height={GLYPH} viewBox="0 0 20 20" fill="none">
        <circle cx="10" cy="6.2" r="3.6" fill="#FF6F5E" />
        <circle cx="6" cy="13.2" r="3.6" fill="#3FB6A8" />
        <circle cx="14" cy="13.2" r="3.6" fill="#D9A94E" />
        <circle cx="10" cy="10" r="9.2" stroke="currentColor" strokeWidth="1" opacity="0.25" />
      </svg>
    </span>
  )
}

/** 20px box (= SWATCH) clipped to `shape`'s silhouette for 外形 chips (keyline:
 *  solid block); 无 wears the slash-circle. */
export function ShapeSwatch({
  shape,
  active,
  className,
}: {
  shape: IconShape
  active?: boolean
  className?: string
}) {
  if (shape === 'None') return <NoneGlyph active={active} className={className} />
  return (
    <span
      aria-hidden="true"
      className={cn('size-5 shrink-0', active ? 'bg-coral' : 'bg-t2', className)}
      style={{ clipPath: clipPathFor(shape) }}
    />
  )
}

// 8-bit heart for the 像素 tile — a pixel-ART motif (jagged silhouette, visible
// cells), not a mosaic checkerboard. 7 cols on the 16px keyline extent.
const PIXEL_HEART = ['.XX.XX.', 'XXXXXXX', 'XXXXXXX', '.XXXXX.', '..XXX..', '...X...']
const PX_PITCH = 2.32
const PX_CELL = 2.05

/** 18px schematic preview of a filter effect for the 滤镜 swatch row.
 *  'Gloss' is the coming-soon roadmap slot (owner-kept look, engine later). */
export function FilterSwatch({
  filter,
  active,
  className,
}: {
  filter: 'None' | 'Gloss' | 'Glass' | 'Pixel' | 'Sticker'
  active?: boolean
  className?: string
}) {
  if (filter === 'None') return <NoneGlyph active={active} className={className} />
  const tone = active ? 'text-coral' : 'text-t2'
  return (
    <span aria-hidden="true" className={cn('inline-flex shrink-0', tone, className)}>
      {/* 20px authoring grid, motifs at the 16px extent (2..18); rendered at GLYPH. */}
      <svg width={GLYPH} height={GLYPH} viewBox="0 0 20 20" fill="none">
        {filter === 'Gloss' && (
          <>
            <rect x="2" y="2" width="16" height="16" rx="4.4" fill="currentColor" opacity="0.85" />
            <path d="M2 8.9 Q10 4.3 18 7.7 L18 6.6 Q18 2 13.4 2 L6.6 2 Q2 2 2 6.6 Z" fill="#FFFFFF" opacity="0.55" />
          </>
        )}
        {filter === 'Glass' && (
          <>
            <rect x="2" y="2" width="16" height="16" rx="4.4" fill="currentColor" opacity="0.26" />
            <rect x="2.6" y="2.6" width="14.8" height="14.8" rx="3.8" stroke="currentColor" strokeWidth="1.2" opacity="0.55" />
            <path d="M5.7 14.8 L12.3 5 M9.8 15.3 L14.8 7.9" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" opacity="0.6" />
          </>
        )}
        {filter === 'Pixel' &&
          PIXEL_HEART.flatMap((row, r) =>
            [...row].map((cell, col) =>
              cell === 'X' ? (
                <rect
                  key={`${r}-${col}`}
                  x={2 + col * PX_PITCH}
                  y={3.18 + r * PX_PITCH}
                  width={PX_CELL}
                  height={PX_CELL}
                  fill="currentColor"
                />
              ) : null,
            ),
          )}
        {filter === 'Sticker' && (
          <>
            <rect x="2.6" y="2.6" width="14.8" height="14.8" rx="5" stroke="currentColor" strokeWidth="1.2" opacity="0.5" />
            <rect x="5" y="5" width="10" height="10" rx="3" fill="currentColor" opacity="0.9" />
          </>
        )}
      </svg>
    </span>
  )
}

/**
 * The GENUINE Win11 shortcut-arrow badge (owner-extracted, corners cut to
 * true transparency), previewed in the SAME grammar as every other mark: a
 * quiet tile silhouette with the badge sitting at its bottom-left — exactly
 * where the desktop puts it. One component serves the swatch (22px), the
 * gate sheet (88px) and the welcome survey; the tile renderer bakes the same
 * asset, so the preview IS the outcome.
 */
export function WinArrowGlyph({
  active,
  className,
  size = GLYPH,
  realistic = false,
}: {
  active?: boolean
  className?: string
  size?: number
  /** True desktop proportions (badge ≈28% of the tile, engine footprint) —
   *  for LARGE previews like the gate sheet. Small chips keep the enlarged
   *  read (a to-scale badge is illegible at 22px). */
  realistic?: boolean
}) {
  const badge = realistic ? 4.6 : 8.8
  return (
    <span aria-hidden="true" className={cn('inline-flex shrink-0', active ? 'text-coral' : 'text-t2', className)}>
      <svg width={size} height={size} viewBox="1 1 20 20">
        <rect x="3" y="3" width="16" height="16" rx="3.5" fill="currentColor" opacity="0.18" />
        <image href={winNativeArrow} x="3.2" y={18.8 - badge} width={badge} height={badge} />
      </svg>
    </span>
  )
}

/** 22px schematic render of a shortcut-mark style for 标识 chips. */
export function MarkGlyph({
  mark,
  active,
  className,
}: {
  mark: MarkStyle
  active?: boolean
  className?: string
}) {
  const uid = useId()
  return (
    <span
      aria-hidden="true"
      className={cn('inline-flex shrink-0', active ? 'text-coral' : 'text-t2', className)}
    >
      {/* Drawn on 22 with a 1px quiet margin; the cropped viewBox lands the
          motif on the shared authoring keyline, rendered at GLYPH. */}
      <svg width={GLYPH} height={GLYPH} viewBox="1 1 20 20" fill="none">
        {markBody(mark, uid)}
      </svg>
    </span>
  )
}

// Each style renders a distinct, recognizable silhouette in the current text colour.
function markBody(mark: MarkStyle, uid: string): ReactNode {
  switch (mark) {
    case 'Shadow': // 投影 — a square floating on its translucent drop shadow
      return (
        <>
          <rect x="6.5" y="6.5" width="12" height="12" rx="3" fill="currentColor" opacity="0.32" />
          <rect x="3" y="3" width="12" height="12" rx="3" fill="currentColor" />
        </>
      )
    case 'Halo': // 光环 — ONE glowing block: concentric fading rings, no
      // nested frame (designer 2026-07-10: three non-concentric rects under
      // the selection ring read as skew; selected state = glyph + coral ring
      // = two concentric layers only).
      return (
        <>
          <rect x="3" y="3" width="14" height="14" rx="5.5" fill="currentColor" opacity="0.12" />
          <rect x="4.5" y="4.5" width="11" height="11" rx="4" fill="currentColor" opacity="0.25" />
          <rect x="6" y="6" width="8" height="8" rx="2.5" fill="currentColor" />
        </>
      )
    case 'Satin': // 缎光角 — a square with a diagonal sheen from bottom-left
      return (
        <>
          <defs>
            <linearGradient id={`${uid}-satin`} x1="3" y1="19" x2="17" y2="7" gradientUnits="userSpaceOnUse">
              <stop offset="0" stopColor="currentColor" stopOpacity="0.9" />
              <stop offset="0.46" stopColor="currentColor" stopOpacity="0.14" />
              <stop offset="1" stopColor="currentColor" stopOpacity="0" />
            </linearGradient>
          </defs>
          <rect x="3" y="3" width="16" height="16" rx="3.5" fill="currentColor" opacity="0.18" />
          <rect x="3" y="3" width="16" height="16" rx="3.5" fill={`url(#${uid}-satin)`} />
        </>
      )
    case 'Arc': // 珐琅光弧 — a square with a corner radial glow dot
      return (
        <>
          <defs>
            <radialGradient id={`${uid}-arc`} cx="0.2" cy="0.82" r="0.7">
              <stop offset="0" stopColor="currentColor" stopOpacity="0.95" />
              <stop offset="0.55" stopColor="currentColor" stopOpacity="0.12" />
              <stop offset="1" stopColor="currentColor" stopOpacity="0" />
            </radialGradient>
          </defs>
          <rect x="3" y="3" width="16" height="16" rx="3.5" fill="currentColor" opacity="0.18" />
          <rect x="3" y="3" width="16" height="16" rx="3.5" fill={`url(#${uid}-arc)`} />
        </>
      )
    case 'Fold': // 卷角 — a square with a dog-ear fold at bottom-right
      return (
        <>
          <path
            d="M6 3 H16 Q19 3 19 6 V13 L13 19 H6 Q3 19 3 16 V6 Q3 3 6 3 Z"
            fill="currentColor"
          />
          <path d="M13 19 L13 14 Q13 13 14 13 L19 13 Z" fill="currentColor" opacity="0.4" />
        </>
      )
    case 'Ring': // 细描边 — a rounded-square ring
      return (
        <rect x="4" y="4" width="14" height="14" rx="3.5" fill="none" stroke="currentColor" strokeWidth="2.2" />
      )
    case 'Glass': // 玻璃 — a translucent frosted plate with a highlight streak
      return (
        <>
          <rect x="3" y="3" width="16" height="16" rx="3.5" fill="currentColor" opacity="0.22" />
          <path d="M6 13 L13 6 L17 6 L6 17 Z" fill="currentColor" opacity="0.3" />
        </>
      )
    default: {
      const _exhaustive: never = mark
      return _exhaustive
    }
  }
}

/** Type-bucket glyph for the 参与美化的类型 row — same 20px keyline as every axis
 *  glyph. `muted` desaturates it for the excluded (unchecked) state. App = a
 *  launcher tile grid, Folder = tabbed folder, File = dog-eared page, System = a
 *  cog. Monochrome via currentColor, so the row inherits the panel's ink. */
export function KindGlyph({
  bucket,
  muted,
  size = GLYPH,
  className,
}: {
  bucket: IconKindBucket
  muted?: boolean
  size?: number
  className?: string
}) {
  return (
    <span aria-hidden="true" className={cn('inline-flex shrink-0', muted ? 'text-t3/50' : 'text-t2', className)}>
      <svg width={size} height={size} viewBox="0 0 20 20" fill="none">
        {kindBody(bucket)}
      </svg>
    </span>
  )
}

function kindBody(bucket: IconKindBucket): ReactNode {
  switch (bucket) {
    case 'App': // 2×2 launcher tile grid
      return (
        <>
          <rect x="3.2" y="3.2" width="6" height="6" rx="1.6" fill="currentColor" />
          <rect x="10.8" y="3.2" width="6" height="6" rx="1.6" fill="currentColor" />
          <rect x="3.2" y="10.8" width="6" height="6" rx="1.6" fill="currentColor" />
          <rect x="10.8" y="10.8" width="6" height="6" rx="1.6" fill="currentColor" />
        </>
      )
    case 'Folder': // tabbed folder silhouette
      return (
        <path
          d="M2.6 6.4 Q2.6 4.9 4.1 4.9 H7.9 L9.6 6.6 H15.9 Q17.4 6.6 17.4 8.1 V13.9 Q17.4 15.4 15.9 15.4 H4.1 Q2.6 15.4 2.6 13.9 Z"
          fill="currentColor"
        />
      )
    case 'File': // page with the fold at the BOTTOM-right — the top-right corner
      // stays clean so the participation ✓ badge never masks the file's identity.
      return (
        <>
          <path
            d="M6.2 2.6 H13.9 A1.5 1.5 0 0 1 15.4 4.1 V12.4 L10.8 17.4 H6.2 A1.5 1.5 0 0 1 4.7 15.9 V4.1 A1.5 1.5 0 0 1 6.2 2.6 Z"
            fill="currentColor"
          />
          <path d="M10.8 17.4 V13.9 A1.5 1.5 0 0 1 12.3 12.4 H15.4 Z" fill="currentColor" opacity="0.4" />
        </>
      )
    case 'System': // a cog — ring body (centre hole) with eight teeth
      return (
        <>
          {[0, 45, 90, 135, 180, 225, 270, 315].map((a) => (
            <rect key={a} x="9.1" y="1.9" width="1.8" height="3.5" rx="0.6" fill="currentColor" transform={`rotate(${a} 10 10)`} />
          ))}
          <circle cx="10" cy="10" r="4.6" fill="none" stroke="currentColor" strokeWidth="2.6" />
        </>
      )
    default: {
      const _exhaustive: never = bucket
      return _exhaustive
    }
  }
}
