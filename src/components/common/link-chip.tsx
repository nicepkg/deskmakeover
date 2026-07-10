import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

// Quiet action chips (还原 / 上一版 / 历史 / 对比图 — spec 02: 6×11, radius 9, 12px).

export function LinkChip({
  onClick,
  disabled = false,
  active = false,
  className,
  children,
}: {
  onClick?: () => void
  disabled?: boolean
  active?: boolean
  className?: string
  children: ReactNode
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={cn(
        'rounded-[9px] px-[11px] py-1.5 text-xs leading-none transition-colors duration-150',
        active ? 'bg-wash-chip text-coral-ink' : 'text-t2 hover:bg-chip hover:text-t1',
        disabled && 'cursor-default opacity-45 hover:bg-transparent hover:text-t2',
        className,
      )}
    >
      {children}
    </button>
  )
}
