import * as React from 'react'
import type { ReactNode } from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { cn } from '@/lib/utils'

/**
 * Measured clearance for the floating footer: whatever height the footer takes
 * (pills appear/disappear, locale changes, future rows), the scroller's bottom
 * padding follows — content can always scroll fully clear of the bar.
 */
export function useFooterClearance(extra = 10): {
  footerRef: (el: HTMLDivElement | null) => void
  clearance: number
} {
  const [clearance, setClearance] = React.useState(84)
  const observerRef = React.useRef<ResizeObserver | null>(null)
  const footerRef = React.useCallback((el: HTMLDivElement | null) => {
    observerRef.current?.disconnect()
    observerRef.current = null
    if (!el) return
    const measure = () => setClearance(Math.ceil(el.getBoundingClientRect().height) + extra)
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(el)
    observerRef.current = observer
  }, [extra])
  return { footerRef, clearance }
}

/**
 * THE inspector reveal grammar: conditional content unfolds with one height+fade
 * curve everywhere (更多形状, 单色色板, 高级清晰度, 分区编辑…) — never a hard cut,
 * never a second dialect. Children unmount fully when hidden, so rows carry no
 * ghost margins in the collapsed state.
 */
export function Reveal({ show, className, children }: { show: boolean; className?: string; children: ReactNode }) {
  const reduced = useReducedMotion()
  return (
    <AnimatePresence initial={false}>
      {show && (
        <motion.div
          className={cn('overflow-hidden', className)}
          initial={reduced ? false : { height: 0, opacity: 0 }}
          animate={{ height: 'auto', opacity: 1 }}
          exit={reduced ? { opacity: 0 } : { height: 0, opacity: 0 }}
          transition={{ duration: 0.18, ease: [0.33, 1, 0.68, 1] }}
        >
          {children}
        </motion.div>
      )}
    </AnimatePresence>
  )
}

/** A grouped inspector card: raised surface, hairline-divided children. */
export function InspectorCard({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <div className={cn('divide-y divide-hair overflow-hidden rounded-xl border border-hair bg-raised', className)}>
      {children}
    </div>
  )
}

/**
 * One property row. `inline` puts the control on the label's row (right-aligned);
 * otherwise the control stacks under the label (for wide controls). `sub` renders
 * an optional expansion area (tint swatches, mark gallery, 更多形状 grid).
 */
export function PropertyRow({
  label,
  inline = false,
  labelExtra,
  sub,
  className,
  children,
}: {
  label: ReactNode
  inline?: boolean
  labelExtra?: ReactNode
  sub?: ReactNode
  className?: string
  children: ReactNode
}) {
  return (
    <div className={cn('px-3 py-2', className)}>
      {inline ? (
        <div className="flex min-h-7 items-center justify-between gap-2">
          <span className="shrink-0 whitespace-nowrap text-[11px] text-t2">{label}</span>
          <div className="flex min-w-0 items-center gap-1.5">{children}</div>
        </div>
      ) : (
        <>
          <div className="mb-1.5 flex items-center justify-between gap-2">
            <span className="min-w-0 truncate whitespace-nowrap text-[11px] text-t2">{label}</span>
            {labelExtra}
          </div>
          {children}
        </>
      )}
      {/* Rendered bare: sub content (usually a <Reveal>) owns its own top spacing,
          so a collapsed sub leaves NO ghost margin under the row. */}
      {sub}
    </div>
  )
}

/** The swatch-button face, exported for triggers that can't BE a SwatchButton
 *  (e.g. a PopoverTrigger wheel) but must wear the identical axis chrome. */
export function swatchButtonClass(selected?: boolean, className?: string): string {
  return cn(
    // ring-inset: the swatch flow's fold wrapper is overflow-hidden and sits
    // FLUSH against the row's left/bottom — an outside ring gets its corner
    // clipped there (owner call 2026-07-09: "控制面板 UI 左下角被裁切").
    'flex size-7 shrink-0 items-center justify-center rounded-lg transition-all duration-150 active:scale-95',
    selected ? 'bg-wash-chip ring-1 ring-inset ring-coral/45' : 'hover:bg-raised-hov ring-1 ring-inset ring-transparent',
    className,
  )
}

/** A square visual swatch button (shapes, marks, colour dots) — Figma-style picker. */
export function SwatchButton({
  selected,
  title,
  onClick,
  onHover,
  className,
  children,
}: {
  selected?: boolean
  title: string
  onClick: () => void
  /** Hover try-on hook (spec 06 §3.2): true on enter, false on leave/pick. */
  onHover?: (hovering: boolean) => void
  className?: string
  children: ReactNode
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      aria-pressed={selected}
      onClick={() => {
        onHover?.(false)
        onClick()
      }}
      onMouseEnter={onHover ? () => onHover(true) : undefined}
      onMouseLeave={onHover ? () => onHover(false) : undefined}
      className={swatchButtonClass(selected, className)}
    >
      {children}
    </button>
  )
}

/** One option in a SwatchPicker flow. */
export type SwatchOption = {
  key: string
  title: string
  selected?: boolean
  /** Roadmap slots (coming soon): dimmed, tooltip-only, inert. */
  disabled?: boolean
  onPick?: () => void
  /** Hover try-on hook — paints the candidate live, never commits. */
  onHover?: (hovering: boolean) => void
  glyph: ReactNode
}

/** THE axis icon-picker: shape / filter / mark rows all speak this one grammar —
 *  a wrapping flow of uniform SwatchButtons; open sets grow it, never truncate. */
export function SwatchPicker({ options, className }: { options: SwatchOption[]; className?: string }) {
  return (
    <div className={cn('flex flex-wrap items-center gap-1', className)}>
      {options.map((o) =>
        o.disabled ? (
          <span
            key={o.key}
            title={o.title}
            aria-disabled="true"
            className="flex size-7 shrink-0 items-center justify-center rounded-lg opacity-40"
          >
            {o.glyph}
          </span>
        ) : (
          <SwatchButton key={o.key} title={o.title} selected={o.selected} onHover={o.onHover} onClick={o.onPick ?? (() => {})}>
            {o.glyph}
          </SwatchButton>
        ),
      )}
    </div>
  )
}

/** Quiet text action for inspector footers (还原 / 历史 / 重新合成 …). */
export function InspectorAction({
  onClick,
  active = false,
  title,
  children,
}: {
  onClick: () => void
  active?: boolean
  title?: string
  children: ReactNode
}) {
  return (
    <button
      type="button"
      title={title}
      onClick={onClick}
      className={cn(
        'whitespace-nowrap rounded-[7px] px-2 py-1 text-[11px] transition-colors active:scale-[0.98]',
        active
          ? 'bg-wash-chip text-coral-ink'
          : 'bg-chip text-t2 hover:bg-raised-hov hover:text-t1',
      )}
    >
      {children}
    </button>
  )
}

/** Icon-only quiet action (list-header verbs: add, magic-layout …) — fixed size,
 *  name in the tooltip, immune to locale length. */
export function IconAction({
  onClick,
  title,
  active = false,
  children,
  // React 19: ref rides ...rest, and Radix Slot (PopoverTrigger asChild) merges
  // its handlers into the props it hands us — spreading rest onto the button is
  // all IconAction needs to serve as a popover anchor.
  ...rest
}: {
  onClick?: React.MouseEventHandler<HTMLButtonElement>
  title: string
  active?: boolean
  children: ReactNode
} & Omit<React.ComponentPropsWithRef<'button'>, 'onClick' | 'title' | 'children'>) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      aria-pressed={active}
      onClick={onClick}
      {...rest}
      className={cn(
        'flex h-[22px] shrink-0 items-center justify-center gap-1 whitespace-nowrap rounded-[7px] px-1.5 text-[10px] leading-none transition-colors active:scale-95',
        active ? 'bg-wash-chip text-coral-ink' : 'bg-chip text-t2 hover:bg-raised-hov hover:text-t1',
      )}
    >
      {children}
    </button>
  )
}
