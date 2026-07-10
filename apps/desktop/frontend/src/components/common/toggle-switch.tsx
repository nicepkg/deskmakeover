import { motion, useReducedMotion } from 'motion/react'
import { cn } from '@/lib/utils'

// Toggle (spec 02): 32×19, knob 15, radius 10; on = coral, off = neutral 35%.

export function ToggleSwitch({
  checked,
  onChange,
  label,
  className,
}: {
  checked: boolean
  onChange: (checked: boolean) => void
  label?: string
  className?: string
}) {
  const reduced = useReducedMotion()
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={cn(
        'relative h-[19px] w-8 shrink-0 rounded-[10px] transition-colors duration-200',
        checked ? 'bg-coral' : 'bg-[rgba(128,128,128,0.35)]',
        className,
      )}
    >
      {/* translateX, not layout projection: projection tracks viewport position,
          so page-level layout shifts made the knob drift inside the track. */}
      <motion.span
        initial={false}
        animate={{ x: checked ? 13 : 0 }}
        transition={reduced ? { duration: 0 } : { type: 'spring', stiffness: 600, damping: 38 }}
        className="absolute left-0.5 top-0.5 size-[15px] rounded-full bg-white shadow-sm"
      />
    </button>
  )
}
