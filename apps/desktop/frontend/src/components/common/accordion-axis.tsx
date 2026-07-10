import type { ReactNode } from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { ChevronDown, Minus, Plus } from 'lucide-react'
import { collapse } from '@/lib/motion'
import { cn } from '@/lib/utils'

// The 自定义 accordion grammar shared by every panel (spec 02): 44px row with a
// left label, right-aligned live summary, and a chevron that rotates 180°.
// Rows separate with hairline top borders; content collapses with height motion.

export function AccordionAxis({
  title,
  summary,
  open,
  onToggle,
  badge,
  first = false,
  children,
}: {
  title: string
  summary?: ReactNode
  open: boolean
  onToggle: () => void
  badge?: ReactNode
  first?: boolean
  children: ReactNode
}) {
  const reduced = useReducedMotion()
  return (
    <div className={cn(!first && 'border-t border-hair')}>
      <button
        type="button"
        aria-expanded={open}
        onClick={onToggle}
        className="flex h-11 w-full items-center justify-between text-left"
      >
        <span className="flex items-center gap-1.5 text-body text-t2">
          {title}
          {badge}
        </span>
        <span className="flex items-center gap-1.5">
          {summary !== undefined && (
            <span className="max-w-[170px] truncate text-body text-t1">{summary}</span>
          )}
          <motion.span
            animate={{ rotate: open ? 180 : 0 }}
            transition={{ duration: reduced ? 0 : 0.2 }}
            className="text-t3"
          >
            <ChevronDown size={13} strokeWidth={2} />
          </motion.span>
        </span>
      </button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            variants={reduced ? undefined : collapse}
            initial="hidden"
            animate="visible"
            exit="exit"
            className="overflow-hidden"
          >
            <div className="pb-3">{children}</div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}

/** The ＋/− expand-all affordance that sits beside a section label. */
export function ExpandAllToggle({
  allOpen,
  onToggle,
  label,
}: {
  allOpen: boolean
  onToggle: () => void
  label?: string
}) {
  return (
    <button
      type="button"
      aria-label={label ?? (allOpen ? 'collapse all' : 'expand all')}
      onClick={onToggle}
      className="flex size-5 items-center justify-center rounded-md text-t3 transition-colors hover:bg-raised-hov hover:text-t1"
    >
      {allOpen ? <Minus size={12} /> : <Plus size={12} />}
    </button>
  )
}
