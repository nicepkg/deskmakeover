import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

// Choice chip (spec 02): padding 6×10, radius 9, 12px; selected = coral 17% wash
// + accent-ink text at weight 600. Saturation is an event — never a solid fill.

export function Chip({
  selected = false,
  onClick,
  className,
  children,
  title,
  leading,
}: {
  selected?: boolean
  onClick?: () => void
  className?: string
  children: ReactNode
  title?: string
  /** Optional live preview (spec 02: 14px shape clip · 10px colour dot · 22px mark). */
  leading?: ReactNode
}) {
  return (
    <button
      type="button"
      title={title}
      aria-pressed={selected}
      onClick={onClick}
      className={cn(
        'inline-flex items-center gap-1.5 rounded-[9px] border border-hair px-2.5 py-1.5',
        'whitespace-nowrap text-[12px] leading-none transition-colors duration-150',
        selected
          ? 'bg-wash-chip font-semibold text-coral-ink border-coral/25'
          : 'bg-chip text-t2 hover:bg-raised-hov hover:text-t1',
        className,
      )}
    >
      {leading}
      {children}
    </button>
  )
}

export function ChipRow({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cn('flex flex-wrap gap-1.5', className)}>{children}</div>
}
