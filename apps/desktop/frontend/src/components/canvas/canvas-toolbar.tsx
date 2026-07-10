import type { ReactNode } from 'react'
import * as React from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { Eye, Maximize2, Redo2, RotateCw, Undo2 } from 'lucide-react'
import { cn } from '@/lib/utils'

// THE canvas toolbar (both mirrors): an icon-only micro pill — compare · zoom% ·
// fit · refresh. 28px tall, ~130px wide; names live in tooltips; zooming's main
// path is Ctrl+wheel (the percentage tap resets). Screen space is the user's.

const GLASS = 'bg-glass text-glass-ink ring-1 ring-glass-ring backdrop-blur-md'

export function CanvasToolbar({
  compareTip,
  comparing,
  onCompareDown,
  onCompareUp,
  zoomPercent,
  zoomMinPercent,
  zoomMaxPercent,
  onZoomPercent,
  onFitLabel,
  fitTip,
  onFit,
  refreshTip,
  onRefresh,
  undoTip,
  redoTip,
  canUndo,
  canRedo,
  onUndo,
  onRedo,
}: {
  compareTip: string
  comparing: boolean
  onCompareDown: () => void
  onCompareUp: () => void
  zoomPercent: number
  zoomMinPercent: number
  zoomMaxPercent: number
  onZoomPercent: (percent: number) => void
  onFitLabel: string
  fitTip: string
  onFit: () => void
  refreshTip?: string
  onRefresh?: () => void
  undoTip?: string
  redoTip?: string
  canUndo?: boolean
  canRedo?: boolean
  onUndo?: () => void
  onRedo?: () => void
}) {
  const [zoomOpen, setZoomOpen] = React.useState(false)
  return (
    <div className="absolute inset-x-0 bottom-2.5 z-10 flex items-center justify-center">
      <div className={cn('relative flex h-7 items-center gap-px rounded-full px-1', GLASS)}>
        {/* Zoom slider flyout — anchored to the PILL, both ends flush with it
            (owner call 2026-07-09: the readout-centred popup sat misaligned). */}
        <AnimatePresence>
          {zoomOpen && (
            <div
              className="absolute inset-x-0 bottom-full pb-2"
              onMouseEnter={() => setZoomOpen(true)}
              onMouseLeave={() => setZoomOpen(false)}
            >
              <motion.div
                className={cn('rounded-[10px] px-3 py-2', GLASS)}
                initial={{ opacity: 0, y: 4, scale: 0.97 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: 4, scale: 0.97 }}
                transition={{ duration: 0.12, ease: [0.33, 1, 0.68, 1] }}
              >
                <input
                  type="range"
                  className="dm-slider w-full"
                  min={zoomMinPercent}
                  max={zoomMaxPercent}
                  step={1}
                  value={Math.min(zoomMaxPercent, Math.max(zoomMinPercent, zoomPercent))}
                  onChange={(e) => onZoomPercent(Number(e.currentTarget.value))}
                  aria-label={fitTip}
                />
                <div className="flex justify-between text-[10px] leading-none text-glass-ink/60">
                  <span>{zoomMinPercent}%</span>
                  <span>{zoomMaxPercent}%</span>
                </div>
              </motion.div>
            </div>
          )}
        </AnimatePresence>
        <button
          type="button"
          title={compareTip}
          aria-label={compareTip}
          aria-pressed={comparing}
          onPointerDown={onCompareDown}
          onPointerUp={onCompareUp}
          onPointerLeave={onCompareUp}
          className={cn(
            'flex size-[22px] items-center justify-center rounded-full transition-colors active:scale-95',
            comparing ? 'bg-coral/90 text-cta-ink' : 'text-glass-ink/75 hover:bg-glass-ink/10 hover:text-glass-ink',
          )}
        >
          <Eye size={12} />
        </button>
        <button
          type="button"
          title={onFitLabel}
          onClick={onFit}
          onMouseEnter={() => setZoomOpen(true)}
          onMouseLeave={() => setZoomOpen(false)}
          className="flex h-[22px] min-w-[38px] items-center justify-center px-1 text-[11px] leading-none tabular-nums text-glass-ink/85 transition-colors hover:text-glass-ink"
        >
          {zoomPercent}%
        </button>
        <ToolButton label={fitTip} onClick={onFit}>
          <Maximize2 size={11} />
        </ToolButton>
        {onRefresh && (
          <ToolButton label={refreshTip ?? ''} onClick={onRefresh}>
            <RotateCw size={11} />
          </ToolButton>
        )}
        {onUndo && (
          <ToolButton label={undoTip ?? ''} onClick={onUndo} disabled={!canUndo}>
            <Undo2 size={11} />
          </ToolButton>
        )}
        {onRedo && (
          <ToolButton label={redoTip ?? ''} onClick={onRedo} disabled={!canRedo}>
            <Redo2 size={11} />
          </ToolButton>
        )}
      </div>
    </div>
  )
}

function ToolButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string
  onClick?: () => void
  disabled?: boolean
  children: ReactNode
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'flex size-[22px] items-center justify-center rounded-full text-glass-ink/75 transition-colors',
        disabled ? 'opacity-35' : 'hover:bg-glass-ink/10 hover:text-glass-ink active:scale-95',
      )}
    >
      {children}
    </button>
  )
}

/**
 * Recompute feedback that respects fast work (owner call 2026-07-08): a 1.5px
 * coral light-line sweeping along the canvas TOP edge. It never touches the
 * pixels the user is judging, and a minimum-visible window lets even an 80ms
 * recompute play one full, calm sweep instead of a subliminal flicker.
 */
export function CanvasProgress({ active }: { active: boolean }) {
  const reduced = useReducedMotion()
  const [visible, setVisible] = React.useState(false)
  const since = React.useRef(0)

  React.useEffect(() => {
    if (active) {
      since.current = Date.now()
      setVisible(true)
      return
    }
    const elapsed = Date.now() - since.current
    const linger = Math.max(0, 400 - elapsed)
    const timer = setTimeout(() => setVisible(false), linger)
    return () => clearTimeout(timer)
  }, [active])

  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          aria-hidden
          className="absolute inset-x-3 top-0 z-10 h-[1.5px] overflow-hidden"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.15 }}
        >
          {reduced ? (
            <div className="h-full w-full bg-coral/60" />
          ) : (
            <motion.div
              className="h-full w-1/3 rounded-full bg-gradient-to-r from-transparent via-coral to-transparent"
              initial={{ x: '-110%' }}
              animate={{ x: '410%' }}
              transition={{ duration: 0.8, repeat: Infinity, ease: 'easeInOut' }}
            />
          )}
        </motion.div>
      )}
    </AnimatePresence>
  )
}
