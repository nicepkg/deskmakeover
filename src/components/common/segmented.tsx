import * as React from 'react'
import { motion, useReducedMotion } from 'motion/react'
import { cn } from '@/lib/utils'

// Adaptive segmented control (spec 02 v3): an inset track with a SLIDING white
// thumb — and a locale-proof fallback. Labels are MEASURED against the space each
// equal segment actually gets (canvas measureText in the control's own font); if
// any language's label would overflow its segment, the whole control degrades to a
// wrapping pill group that shares the same selected-state language (raised white
// pill). No locale can ever truncate or overflow this control.

let measureCtx: CanvasRenderingContext2D | null = null

function textWidth(label: string, font: string): number {
  measureCtx ??= document.createElement('canvas').getContext('2d')
  if (!measureCtx) return 0
  measureCtx.font = font
  return measureCtx.measureText(label).width
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  size = 'md',
  className,
}: {
  value: T
  options: { value: T; label: string }[]
  onChange: (value: T) => void
  size?: 'sm' | 'md'
  className?: string
}) {
  const reduced = useReducedMotion()
  const hostRef = React.useRef<HTMLDivElement>(null)
  const [wrap, setWrap] = React.useState(false)

  const labelsKey = options.map((o) => o.label).join('')
  const segPaddingX = size === 'sm' ? 12 : 20 // px-2 / px-2.5 both sides

  React.useLayoutEffect(() => {
    const host = hostRef.current
    if (!host) return
    const check = () => {
      const style = getComputedStyle(host)
      const font = `${size === 'sm' ? 11 : 13}px ${style.fontFamily}`
      const inner = host.clientWidth - 6 // track p-[3px]
      if (inner <= 0) return
      const per = inner / options.length - segPaddingX
      setWrap(options.some((o) => textWidth(o.label, font) > per))
    }
    check()
    const observer = new ResizeObserver(check)
    observer.observe(host)
    return () => observer.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [labelsKey, size, options.length])

  if (wrap) {
    return (
      <div ref={hostRef} role="radiogroup" className={cn('flex w-full flex-wrap gap-1', className)}>
        {options.map((o) => {
          const selected = o.value === value
          return (
            <button
              key={o.value}
              type="button"
              role="radio"
              aria-checked={selected}
              onClick={() => onChange(o.value)}
              className={cn(
                'whitespace-nowrap rounded-[8px] leading-none transition-colors duration-150 active:scale-[0.98]',
                size === 'sm' ? 'h-[22px] px-2 text-[11px]' : 'h-7 px-3 text-body',
                selected
                  ? 'bg-raised text-t1 shadow-elev-1 ring-1 ring-hair dark:bg-raised-hov'
                  : 'bg-chip text-t2 hover:text-t1',
              )}
            >
              {o.label}
            </button>
          )
        })}
      </div>
    )
  }

  // The thumb is a single track-relative element animated with translateX only —
  // never a layoutId projection. Projection animates in viewport space, so any
  // panel-height change elsewhere made the thumb drift vertically inside the
  // track while the track itself jumped. A transform in track space slides
  // horizontally on selection and rides layout shifts instantly.
  const activeIndex = options.findIndex((o) => o.value === value)

  return (
    <div
      ref={hostRef}
      role="radiogroup"
      className={cn(
        'relative grid w-full max-w-[360px] auto-cols-fr grid-flow-col rounded-[9px] bg-chip p-[3px]',
        className,
      )}
    >
      {activeIndex >= 0 && (
        <motion.span
          aria-hidden
          className="absolute inset-y-[3px] left-[3px] rounded-[7px] bg-raised shadow-elev-1 ring-1 ring-hair dark:bg-raised-hov"
          style={{ width: `calc((100% - 6px) / ${options.length})` }}
          initial={false}
          animate={{ x: `${activeIndex * 100}%` }}
          transition={reduced ? { duration: 0 } : { type: 'spring', stiffness: 520, damping: 44 }}
        />
      )}
      {options.map((o) => {
        const selected = o.value === value
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => onChange(o.value)}
            className={cn(
              'relative rounded-[7px] leading-none transition-colors duration-150 active:scale-[0.98]',
              size === 'sm' ? 'h-[22px] px-1.5 text-[11px]' : 'h-7 px-2.5 text-body',
              selected ? 'text-t1' : 'text-t2 hover:text-t1',
            )}
          >
            <span className="relative z-10 whitespace-nowrap">{o.label}</span>
          </button>
        )
      })}
    </div>
  )
}
