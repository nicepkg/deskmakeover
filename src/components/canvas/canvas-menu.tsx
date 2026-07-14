import * as React from 'react'
import { cn } from '@/lib/utils'

// The canvas context-menu dialect shared by BOTH desktop mirrors (icons +
// wallpaper): one 188px rounded-xl surface, quiet rows, hover wash. Extracted
// from icons-mirror (spec 06 §3.8) when the wallpaper canvas gained its zone
// menu (spec 04 §3 round 3) — one menu grammar, one home.

export function MenuRow({
  checked = false,
  danger = false,
  onClick,
  children,
}: {
  checked?: boolean
  /** Destructive verb (delete): red ink, same quiet surface. */
  danger?: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'flex w-full items-center justify-between rounded-lg px-2 py-[7px] text-left text-xs hover:bg-raised-hov',
        danger ? 'text-red-500' : 'text-t1',
      )}
    >
      {children}
      {checked && <span className="text-coral-ink">✓</span>}
    </button>
  )
}
