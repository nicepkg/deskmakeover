import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import type { ScreenOrientation } from '@/bridge/types'
import { useT } from '@/lib/i18n'
import { presetsForOrientation } from '@/lib/zone-presets'
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
  orientation,
  onPreset,
  onImport,
}: {
  show: boolean
  wallpaperUrl: string | null
  /** The wallpaper's on-screen rect (screen space, already zoom/pan-projected). */
  rect: { left: number; top: number; width: number; height: number }
  /** The active screen's shape — picks which preset set + thumbnail aspect to show. */
  orientation: ScreenOrientation
  onPreset: (preset: ZonePreset) => void
  onImport: () => void
}) {
  const t = useT()
  const reduced = useReducedMotion()
  const presets = presetsForOrientation(orientation)
  const isPortrait = orientation === 'portrait'
  // Portrait thumbnails keep the true 9:16 aspect but are HEIGHT-capped + centred
  // so two rows don't blow the popup past the canvas (owner 2026-07-12: 弹窗高度
  // 没适配好). Landscape stays width-filling 16:9.
  const thumbClass = isPortrait ? 'aspect-[9/16] h-[136px] mx-auto' : 'aspect-video w-full'
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
            className="pointer-events-auto flex max-h-[86%] w-[318px] max-w-[86%] flex-col rounded-[16px] bg-glass p-3.5 text-glass-ink shadow-elev-2 ring-1 ring-glass-ring backdrop-blur-md"
            onPointerDown={(e) => e.stopPropagation()}
          >
            <p className="px-0.5 text-body font-medium">{t('Paper_EmptyLead')}</p>
            <div className="mt-2.5 grid grid-cols-2 gap-1.5 overflow-y-auto">
              {presets.map((preset) => (
                <button
                  key={preset.id}
                  type="button"
                  onClick={() => onPreset(preset)}
                  className="group overflow-hidden rounded-[10px]"
                >
                  <span className={`relative block ${thumbClass} overflow-hidden bg-black/20`}>
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
                  <span className={`block truncate px-1.5 py-1 text-caption text-glass-ink/85 group-hover:text-glass-ink ${isPortrait ? 'text-center' : 'text-left'}`}>
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
