import { motion, useReducedMotion } from 'motion/react'
import { useApp } from '@/stores/app'
import { useT } from '@/lib/i18n'

// One-shot coach mark (spec 04 §1 / §3.5): the first time the wallpaper module is
// opened it explains the mental model (zones are painted底板, icons don't auto-move,
// the original is backed up), then persists `wallpaperCoachShown` via settings so it
// never returns. Rendered inside the paper mirror; 知道了 writes the flag through the
// bridge (mock in browser-dev, the real host in production).
export function PaperCoach() {
  const t = useT()
  const settings = useApp((s) => s.settings)
  const reduced = useReducedMotion()

  if (!settings || settings.wallpaperCoachShown) return null

  const dismiss = () => void useApp.getState().updateSettings({ wallpaperCoachShown: true })

  return (
    <motion.div
      className="absolute inset-0 z-50 grid place-items-center bg-black/45 p-6 backdrop-blur-[2px]"
      initial={{ opacity: reduced ? 1 : 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: reduced ? 0 : 0.2 }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <motion.div
        role="dialog"
        aria-modal="true"
        className="w-full max-w-[340px] rounded-2xl bg-glass p-5 text-glass-ink shadow-elev-2 ring-1 ring-glass-ring backdrop-blur-xl"
        initial={{ scale: reduced ? 1 : 0.96, opacity: reduced ? 1 : 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ duration: reduced ? 0 : 0.22, ease: [0.33, 1, 0.68, 1] }}
      >
        <p className="text-body leading-relaxed">{t('Paper_Coach')}</p>
        <div className="mt-4 flex justify-end">
          <button
            type="button"
            onClick={dismiss}
            className="rounded-[10px] bg-coral px-4 py-2 text-body font-semibold text-cta-ink shadow-elev-cta transition-transform active:scale-[0.98]"
          >
            {t('Paper_CoachOk')}
          </button>
        </div>
      </motion.div>
    </motion.div>
  )
}
