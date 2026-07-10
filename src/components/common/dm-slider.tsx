import * as React from 'react'
import { cn } from '@/lib/utils'

// Product slider — coral progress on a hairline track, 14px knob. Built on the
// native range input (one dependency fewer than radix for a 1-D control) with the
// same keyboard semantics; the track paints via a gradient stop at the value.

export function DmSlider({
  value,
  min = 0,
  max = 100,
  step = 1,
  onChange,
  onCommit,
  disabled = false,
  className,
  'aria-label': ariaLabel,
}: {
  value: number
  min?: number
  max?: number
  step?: number
  onChange: (value: number) => void
  onCommit?: (value: number) => void
  disabled?: boolean
  className?: string
  'aria-label'?: string
}) {
  const percent = max > min ? ((value - min) / (max - min)) * 100 : 0
  const commit = () => onCommit?.(value)
  return (
    <input
      type="range"
      aria-label={ariaLabel}
      min={min}
      max={max}
      step={step}
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(Number(e.currentTarget.value))}
      onPointerUp={commit}
      onKeyUp={(e) => {
        if (e.key.startsWith('Arrow') || e.key === 'Home' || e.key === 'End') commit()
      }}
      className={cn('dm-slider w-full', disabled && 'opacity-50', className)}
      style={{ '--dm-slider-fill': `${percent}%` } as React.CSSProperties}
    />
  )
}
