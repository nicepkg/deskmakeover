import type { ReactNode } from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { cn } from '@/lib/utils'

// The hero CTA (spec 01 state machine · spec 02 v3): solid accent marks the moment
// of transformation. `working` proves liveness with an indeterminate shimmer sweep;
// phase text crossfades instead of hard-swapping; `synced` lands with a ✓ pop.
// bg-primary resolves to --coral-ink on light (large-solid register) and --coral
// on dark.

export type HeroPhase = 'scanning' | 'ready' | 'working' | 'dirty' | 'synced'

const phaseStyles: Record<HeroPhase, string> = {
  scanning: 'bg-chip text-t3 cursor-default',
  ready: 'bg-primary text-primary-foreground shadow-elev-cta hover:brightness-105 active:scale-[0.98]',
  working: 'bg-primary/80 text-primary-foreground/90 cursor-default',
  dirty: 'bg-primary text-primary-foreground shadow-elev-cta hover:brightness-105 active:scale-[0.98]',
  synced: 'bg-teal-solid text-teal cursor-default',
}

export function CtaButton({
  phase,
  onClick,
  compact = false,
  className,
  children,
}: {
  phase: HeroPhase
  onClick?: () => void
  compact?: boolean
  className?: string
  children: ReactNode
}) {
  const interactive = phase === 'ready' || phase === 'dirty'
  const reduced = useReducedMotion()
  // Some locales bake a ✓ into the synced string — the animated glyph below is the
  // single source of checkmarks, so strip any leading one from the text.
  const label = typeof children === 'string' ? children.replace(/^[✓\s]+/, '') : children
  return (
    <button
      type="button"
      disabled={!interactive}
      onClick={interactive ? onClick : undefined}
      className={cn(
        'relative w-full overflow-hidden font-medium transition-all duration-150',
        compact ? 'h-[32px] rounded-[9px] text-body' : 'h-10 rounded-[10px] text-[13px]',
        phaseStyles[phase],
        className,
      )}
    >
      {/* Indeterminate liveness sweep while the engine works */}
      {phase === 'working' && !reduced && (
        <motion.span
          aria-hidden
          className="absolute inset-y-0 w-1/3 bg-gradient-to-r from-transparent via-white/25 to-transparent"
          initial={{ x: '-120%' }}
          animate={{ x: '420%' }}
          transition={{ duration: 1.3, repeat: Infinity, ease: 'linear' }}
        />
      )}
      <AnimatePresence mode="popLayout" initial={false}>
        <motion.span
          key={String(label)}
          className="relative z-10 inline-flex items-center justify-center gap-1"
          initial={reduced ? { opacity: 0 } : { opacity: 0, y: 5 }}
          animate={{ opacity: 1, y: 0 }}
          exit={reduced ? { opacity: 0 } : { opacity: 0, y: -5 }}
          transition={{ duration: 0.12, ease: [0.33, 1, 0.68, 1] }}
        >
          {phase === 'synced' && (
            <motion.span
              initial={reduced ? false : { scale: 0.6, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              transition={{ duration: 0.18, ease: [0.34, 1.4, 0.4, 1] }}
            >
              ✓
            </motion.span>
          )}
          {label}
        </motion.span>
      </AnimatePresence>
    </button>
  )
}
