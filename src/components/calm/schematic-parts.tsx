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
 *  (hover-linked via .calm-pulse), dashed while the effect waits for sign-in.
 *  Once DONE the highlight disappears entirely (owner 2026-07-13): the honest
 *  after-state is the clean reflowed surface, not a ghost outline — the row's
 *  ✓已生效 chip carries the receipt. Never a security palette. */
export function Highlight({ region, visual }: { region: SchematicRegion; visual: HighlightVisual }) {
  const { x, y, w, h, rx = 3 } = region
  if (visual === 'done') return null
  if (visual === 'muted') {
    return <rect x={x} y={y} width={w} height={h} rx={rx} fill="none" stroke="var(--hair-strong)" strokeWidth="1" />
  }
  const awaiting = visual === 'awaiting'
  return (
    <g>
      <rect
        x={x}
        y={y}
        width={w}
        height={h}
        rx={rx}
        fill={awaiting ? 'none' : 'var(--coral)'}
        fillOpacity={awaiting ? 0 : 0.12}
        className={awaiting ? undefined : 'calm-wash'}
        stroke={awaiting ? 'var(--coral-ink)' : 'var(--coral)'}
        strokeWidth={awaiting ? 1 : 1.5}
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
    </g>
  )
}

/** Trailing content that REFLOWS into the removed element's place (owner
 *  2026-07-13: no empty sockets — the surface compacts exactly like the real
 *  desktop does). Wrap the siblings that sit after the noise and give the shift. */
export function ReflowGroup({
  gone,
  dx = 0,
  dy = 0,
  children,
}: {
  gone: boolean
  dx?: number
  dy?: number
  children: ReactNode
}) {
  const reduced = useReducedMotion()
  return (
    <motion.g
      initial={false}
      animate={{ x: gone ? dx : 0, y: gone ? dy : 0 }}
      transition={reduced ? { duration: 0 } : { duration: 0.35, ease: [0.33, 1, 0.68, 1], delay: 0.18 }}
    >
      {children}
    </motion.g>
  )
}

/** Shared: has this row's noise honestly left the surface? (view helper) */
export function noiseGone(state: CalmRowState): boolean {
  return NOISE_GONE.has(state)
}
