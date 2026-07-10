import type { ReactNode } from 'react'
import { useApp } from '@/stores/app'
import { cn } from '@/lib/utils'

// Module layout (spec 02 v3 rebuild): CANVAS-FIRST. The mirror owns the stage; the
// controls live in a slim right-hand inspector that is ALWAYS visible — the old
// compact hamburger-overlay (which hid the product's controls behind a click) is
// deleted. Below the compact breakpoint the inspector narrows instead of hiding;
// the layout stays workable down to the 1024×700 window floor.

export function ModuleLayout({
  inspector,
  mirror,
}: {
  inspector: ReactNode
  mirror: ReactNode
}) {
  const compact = useApp((s) => s.compact)
  return (
    <div className="flex min-w-0 flex-1">
      <div className="flex min-w-0 flex-1">{mirror}</div>
      <div className={cn('flex shrink-0', compact && '[&>aside]:w-[248px]')}>{inspector}</div>
    </div>
  )
}
