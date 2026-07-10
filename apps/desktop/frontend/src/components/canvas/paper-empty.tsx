import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { useT } from '@/lib/i18n'
import { ZONE_PRESETS } from '@/lib/zone-presets'
import type { ZonePreset } from '@/lib/zone-presets'

// Wallpaper empty state (spec 04 §2.3, IA redesign 2026-07-09): the preset
// gallery IS the empty state — curated layouts drawn on the user's own
// wallpaper, inside a quiet glass card bound to the WALLPAPER rect (never the
// letterbox; never before the compositor's first frame — the old giant dashed
// frame flashed on refresh precisely because it ignored both). Drawing by hand
// and importing stay one line each.

export function PaperEmptyState({
  show,
  wallpaperUrl,
  rect,
  onPreset,
  onImport,
}: {
  show: boolean
  wallpaperUrl: string | null
  /** The wallpaper's on-screen rect (screen space, already zoom/pan-projected). */
  rect: { left: number; top: number; width: number; height: number }
  onPreset: (preset: ZonePreset) => void
  onImport: () => void
}) {
  const t = useT()
  const reduced = useReducedMotion()
  return (
    <AnimatePresence>
      {show && (
        <motion.div
          className="pointer-events-none absolute z-10 grid place-items-center"
          style={rect}
          initial={reduced ? { opacity: 0 } : { opacity: 0, scale: 0.985, y: 4 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={reduced ? { opacity: 0 } : { opacity: 0, scale: 0.98, y: 4 }}
          transition={{ duration: 0.18, ease: [0.33, 1, 0.68, 1] }}
        >
          {/* pointerdown must NOT bubble into the canvas host — its create-zone
              gesture takes pointer capture and swallows the ensuing click (the
              "preset cards don't react" bug). */}
          <div
            className="pointer-events-auto w-[318px] max-w-[86%] rounded-[16px] bg-glass p-3.5 text-glass-ink shadow-elev-2 ring-1 ring-glass-ring backdrop-blur-md"
            onPointerDown={(e) => e.stopPropagation()}
          >
            <p className="px-0.5 text-body font-medium">{t('Paper_EmptyLead')}</p>
            <div className="mt-2.5 grid grid-cols-2 gap-1.5">
              {ZONE_PRESETS.map((preset) => (
                <button
                  key={preset.id}
                  type="button"
                  onClick={() => onPreset(preset)}
                  className="group overflow-hidden rounded-[10px] ring-1 ring-glass-ring transition-shadow hover:ring-coral/70"
                >
                  <span className="relative block aspect-video w-full overflow-hidden bg-black/20">
                    {wallpaperUrl && (
                      <img src={wallpaperUrl} alt="" className="absolute inset-0 size-full object-cover" draggable={false} />
                    )}
                    {preset.zones.map((z, i) => (
                      <span
                        key={i}
                        className="absolute rounded-[3px]"
                        style={{
                          left: `${z.x * 100}%`,
                          top: `${z.y * 100}%`,
                          width: `${z.w * 100}%`,
                          height: `${z.h * 100}%`,
                          background: 'rgba(255,255,255,0.42)',
                          boxShadow: `inset 0 0 0 1px rgba(0,0,0,0.12), inset 0 1.5px 0 ${z.accent}`,
                        }}
                      />
                    ))}
                  </span>
                  <span className="block truncate px-1.5 py-1 text-left text-caption text-glass-ink/85 group-hover:text-glass-ink">
                    {t(preset.nameKey)}
                  </span>
                </button>
              ))}
            </div>
            <p className="mt-2.5 px-0.5 text-caption leading-snug text-glass-ink/70">{t('Paper_EmptyDrawHint')}</p>
            <button
              type="button"
              onClick={onImport}
              className="mt-1 px-0.5 text-caption text-glass-ink/70 underline decoration-glass-ink/30 underline-offset-2 transition-colors hover:text-glass-ink"
            >
              {t('Paper_EmptyImportHint')}
            </button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
