import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { Reveal } from '@/components/common/inspector'
import { useWallpaper } from '@/stores/wallpaper'
import { activeScreenFacts } from '@/lib/screen-arrange'
import { format, useT } from '@/lib/i18n'

// The multi-screen panel notices (spec 04 §B5/A4/B6). Grouped out of the (already
// large) wallpaper panel so all the per-screen status lives in one cohesive place:
//   • per-screen 正在编辑 · 屏幕 N（竖屏） header — crossfades on switch so the
//     target monitor is never ambiguous;
//   • Span note — one image spans every monitor, the switcher is hidden (§B6);
//   • dynamic-wallpaper banners — non-blocking amber, NEVER red: an unreadable
//     source routes to the import CTA, a Windows slideshow warns rotation stops.
// A single-monitor host yields multiScreen === false ⇒ nothing renders (parity).

export function WallpaperScreenNotices() {
  const t = useT()
  const reduced = useReducedMotion()
  const state = useWallpaper((s) => s.state)
  const sourceUrl = useWallpaper((s) => s.sourceUrl)
  const importSourceViaPicker = useWallpaper((s) => s.importSourceViaPicker)

  if (!state) return null
  const { activeScreen, activeIndex, multiScreen, noReadableSource, slideshowActive } = activeScreenFacts(state)

  return (
    <>
      {multiScreen && activeScreen && (
        <div className="flex min-h-[16px] items-center px-0.5">
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.p
              key={state.activeScreenId}
              initial={reduced ? false : { opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.12, ease: [0.33, 1, 0.68, 1] }}
              className="text-[11px] font-medium text-coral-ink"
            >
              {format(t('Paper_EditingScreen'), activeIndex + 1)}
              <span className="text-t3">
                （{t(activeScreen.orientation === 'portrait' ? 'Paper_Portrait' : 'Paper_Landscape')}）
              </span>
            </motion.p>
          </AnimatePresence>
        </div>
      )}

      <Reveal show={state.spanActive}>
        <p className="rounded-[10px] bg-chip px-3 py-2 text-caption text-t2">{t('Paper_SpanNote')}</p>
      </Reveal>

      <Reveal show={noReadableSource && !sourceUrl}>
        <button
          type="button"
          onClick={() => importSourceViaPicker()}
          className="w-full rounded-[10px] bg-amber-wash px-3 py-2 text-left text-caption text-amber"
        >
          {t('Paper_DynamicWallpaper')} · {t('Paper_DynamicImport')}
        </button>
      </Reveal>
      <Reveal show={slideshowActive}>
        <p className="rounded-[10px] bg-amber-wash px-3 py-2 text-caption text-amber">{t('Paper_SlideshowWarn')}</p>
      </Reveal>
    </>
  )
}
