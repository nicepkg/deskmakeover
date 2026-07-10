import * as React from 'react'
import { cn } from '@/lib/utils'

// Photoshop-style rotary angle picker (owner call — never a linear slider for
// angles). 0° = top, clockwise; drag anywhere on the face, Shift snaps to 15°.

export function AngleDial({
  value,
  onChange,
  onCommit,
  size = 44,
  className,
}: {
  value: number
  onChange: (deg: number) => void
  onCommit?: (deg: number) => void
  size?: number
  className?: string
}) {
  const ref = React.useRef<SVGSVGElement>(null)

  const angleFrom = (e: { clientX: number; clientY: number; shiftKey: boolean }) => {
    const rect = ref.current!.getBoundingClientRect()
    const dx = e.clientX - (rect.left + rect.width / 2)
    const dy = e.clientY - (rect.top + rect.height / 2)
    let deg = (Math.atan2(dx, -dy) * 180) / Math.PI
    deg = (deg + 360) % 360
    if (e.shiftKey) deg = Math.round(deg / 15) * 15 % 360
    return Math.round(deg)
  }

  const onPointerDown = (e: React.PointerEvent<SVGSVGElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId)
    onChange(angleFrom(e))
  }
  const onPointerMove = (e: React.PointerEvent<SVGSVGElement>) => {
    if (e.buttons & 1) onChange(angleFrom(e))
  }
  const onPointerUp = (e: React.PointerEvent<SVGSVGElement>) => {
    onCommit?.(angleFrom(e))
  }

  const c = size / 2
  const rOuter = c - 1.5
  const rad = ((value - 90) * Math.PI) / 180
  const nx = c + Math.cos(rad) * (rOuter - 6)
  const ny = c + Math.sin(rad) * (rOuter - 6)

  return (
    <svg
      ref={ref}
      role="slider"
      aria-label="angle"
      aria-valuenow={value}
      aria-valuemin={0}
      aria-valuemax={359}
      tabIndex={0}
      width={size}
      height={size}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onKeyDown={(e) => {
        const step = e.shiftKey ? 15 : 1
        if (e.key === 'ArrowUp' || e.key === 'ArrowRight') {
          onChange((value + step) % 360)
          e.preventDefault()
        } else if (e.key === 'ArrowDown' || e.key === 'ArrowLeft') {
          onChange((value - step + 360) % 360)
          e.preventDefault()
        }
      }}
      className={cn('cursor-pointer touch-none select-none outline-none focus-visible:drop-shadow-[0_0_3px_var(--coral)]', className)}
    >
      <circle cx={c} cy={c} r={rOuter} className="fill-raised stroke-hair" strokeWidth="1.5" />
      {[0, 90, 180, 270].map((t) => {
        const tr = ((t - 90) * Math.PI) / 180
        return (
          <line
            key={t}
            x1={c + Math.cos(tr) * (rOuter - 3.5)}
            y1={c + Math.sin(tr) * (rOuter - 3.5)}
            x2={c + Math.cos(tr) * (rOuter - 1)}
            y2={c + Math.sin(tr) * (rOuter - 1)}
            className="stroke-t3"
            strokeWidth="1.2"
          />
        )
      })}
      <line x1={c} y1={c} x2={nx} y2={ny} className="stroke-coral" strokeWidth="2" strokeLinecap="round" />
      <circle cx={c} cy={c} r="2.4" className="fill-coral" />
    </svg>
  )
}
