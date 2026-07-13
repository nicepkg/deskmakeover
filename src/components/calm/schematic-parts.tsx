import type { ReactNode } from 'react'
import { motion, useReducedMotion } from 'motion/react'
import type { Variants } from 'motion/react'
import { noiseExit } from '@/lib/motion'
import type { CalmRowState } from '@/lib/calm/states'
import type { SchematicRegion } from '@/lib/calm/schematic-map'

// Shared schematic vocabulary (viz panel 2026-07-13): ONE coral highlight per
// frame marks the operation area; noise elements inside it exit when the row is
// honestly settled. Everything derives from CalmRowState — the schematic is a
// VIEW of the state machine and can never run ahead of verification.

export type HighlightVisual = 'armed' | 'working' | 'done' | 'awaiting' | 'muted'

export function highlightVisual(state: CalmRowState): HighlightVisual {
  switch (state) {
    case 'pending':
      return 'working'
    case 'verified':
    case 'confirmedOff':
    case 'userAttested':
      return 'done'
    case 'setAwaiting':
      return 'awaiting'
    case 'quiet':
    case 'external':
    case 'unsupported':
    case 'managed':
      return 'muted'
    default:
      return 'armed' // pushing / reverted / unknown / needsReconfirm
  }
}

/** States whose noise has honestly LEFT the surface (verified write, or a guided
 *  outcome the user completed). setAwaiting dims in place — never removed early. */
const NOISE_GONE: ReadonlySet<CalmRowState> = new Set(['verified', 'confirmedOff', 'userAttested', 'quiet'])

const noiseFade: Variants = {
  present: { opacity: 1 },
  quiet: { opacity: 0, transition: { duration: 0.3 } },
}

/** Wraps a scene's noise elements; exits via noiseExit when the state says gone.
 *  `delay` staggers batch apply (bloomStaggerMs × row index, set by the caller). */
export function NoiseGroup({ state, delay = 0, children }: { state: CalmRowState; delay?: number; children: ReactNode }) {
  const reduced = useReducedMotion()
  const gone = NOISE_GONE.has(state)
  const dimmed = state === 'setAwaiting'
  return (
    <motion.g
      initial={false}
      animate={gone ? 'quiet' : 'present'}
      variants={reduced ? noiseFade : noiseExit}
      transition={{ delay }}
      style={{ transformBox: 'fill-box', transformOrigin: 'center', opacity: dimmed ? 0.35 : undefined }}
    >
      {children}
    </motion.g>
  )
}

/** The single coral highlight marking the operation area. Pulse dot while armed
 *  (hover-linked via .calm-pulse), coral-ink check once done, dashed while the
 *  effect waits for sign-in. Never a security palette. */
export function Highlight({ region, visual }: { region: SchematicRegion; visual: HighlightVisual }) {
  const { x, y, w, h, rx = 3 } = region
  if (visual === 'muted') {
    return <rect x={x} y={y} width={w} height={h} rx={rx} fill="none" stroke="var(--hair-strong)" strokeWidth="1" />
  }
  const done = visual === 'done'
  const awaiting = visual === 'awaiting'
  return (
    <g>
      <rect
        x={x}
        y={y}
        width={w}
        height={h}
        rx={rx}
        fill={done || awaiting ? 'none' : 'var(--coral)'}
        fillOpacity={done || awaiting ? 0 : 0.12}
        className={done || awaiting ? undefined : 'calm-wash'}
        stroke={done || awaiting ? 'var(--coral-ink)' : 'var(--coral)'}
        strokeWidth={done || awaiting ? 1 : 1.5}
        strokeDasharray={awaiting ? '3 2' : undefined}
      />
      {visual === 'armed' && (
        <circle cx={x + w - 1} cy={y + 1} r="1.8" fill="var(--coral)" className="calm-pulse" />
      )}
      {visual === 'working' && (
        <motion.rect
          x={x}
          y={y - 1}
          width={12}
          height={1.5}
          rx={0.75}
          fill="var(--coral)"
          initial={{ x: x - 6 }}
          animate={{ x: x + w - 6 }}
          transition={{ duration: 1.1, repeat: Infinity, repeatType: 'mirror', ease: 'easeInOut' }}
        />
      )}
      {done && (
        <g transform={`translate(${x + w - 3.5}, ${y + 3.5})`}>
          <circle r="3.4" fill="var(--coral-ink)" />
          <path d="M -1.5 0 L -0.4 1.2 L 1.6 -1.1" fill="none" stroke="white" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round" />
        </g>
      )}
    </g>
  )
}
