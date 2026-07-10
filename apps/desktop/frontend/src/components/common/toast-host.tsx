import * as React from 'react'
import { AnimatePresence, motion } from 'motion/react'
import { pop } from '@/lib/motion'
import { useToasts } from '@/stores/toasts'

// Toast pills anchored to the CANVAS STAGE (owner call 2026-07-09): the user's
// eyes live on the preview, and the canvas toolbar is centered within it — a
// window-centered toast sits visibly off that axis and reads as broken. When a
// `[data-toast-anchor]` stage exists, toasts center on IT and hover just above
// its toolbar; pages without a stage (settings) fall back to window-center.

/** Gap above the stage's bottom edge: toolbar (bottom 10 + h 28) + 8px air. */
const ABOVE_TOOLBAR = 54

function useStageAnchor(active: boolean): React.CSSProperties {
  const [style, setStyle] = React.useState<React.CSSProperties>({})
  React.useLayoutEffect(() => {
    if (!active) return
    const measure = () => {
      const el = document.querySelector('[data-toast-anchor]')
      if (!el) {
        setStyle({ left: 0, right: 0, bottom: 62 })
        return
      }
      const r = el.getBoundingClientRect()
      setStyle({ left: r.left, width: r.width, bottom: window.innerHeight - r.bottom + ABOVE_TOOLBAR })
    }
    measure()
    window.addEventListener('resize', measure)
    return () => window.removeEventListener('resize', measure)
  }, [active])
  return style
}

export function ToastHost() {
  const toasts = useToasts((s) => s.toasts)
  const style = useStageAnchor(toasts.length > 0)
  return (
    <div className="pointer-events-none fixed z-50 flex flex-col items-center gap-2" style={style}>
      <AnimatePresence>
        {toasts.map((t) => (
          <motion.div
            key={t.id}
            variants={pop}
            initial="hidden"
            animate="visible"
            exit="exit"
            className="pointer-events-auto flex items-center gap-2 rounded-[11px] bg-[rgba(22,22,26,0.88)] px-4 py-2 text-[12.5px] text-[#F4F4F2] shadow-lg backdrop-blur-md"
          >
            {t.text}
            {t.action && (
              <button
                type="button"
                onClick={() => {
                  t.action!.run()
                  useToasts.getState().dismiss(t.id)
                }}
                className="-my-0.5 rounded-md px-1.5 py-0.5 font-medium text-coral transition-colors hover:bg-white/10"
              >
                {t.action.label}
              </button>
            )}
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  )
}
