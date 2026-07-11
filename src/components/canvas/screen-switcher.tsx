import { motion } from 'motion/react'
import { useWallpaper } from '@/stores/wallpaper'
import { format, useT } from '@/lib/i18n'
import { arrangeTiles, shouldShowSwitcher } from '@/lib/screen-arrange'
import { cn } from '@/lib/utils'

// The screen switcher (spec 04 §B4): a floating glass pill at the canvas TOP-LEFT
// (mirrors the bottom-center CanvasToolbar's glass grammar; top-RIGHT is rejected —
// it collides with the titlebar drag region + caption buttons). Inside is a live
// mini-map of the OS "Displays arrangement": one tile per monitor, shaped to that
// screen's real aspect (orientation = shape, no icon), positioned relative to the
// others, filled with a live crop of that screen's wallpaper. Clicking a tile
// switches the active screen. Renders ONLY with ≥2 screens and never in Span mode
// (single-monitor parity: nothing renders, behaviour identical to today).

// Matches CanvasToolbar's GLASS pill so the two chrome affordances read as siblings.
const GLASS = 'bg-glass text-glass-ink ring-1 ring-glass-ring backdrop-blur-md'
// Arrangement box budget (px). Small enough to sit quietly in the corner, large
// enough that a 1080-wide portrait beside a 1920 landscape stays legible.
const MAX_W = 140
const MAX_H = 84

export function ScreenSwitcher() {
  const t = useT()
  const state = useWallpaper((s) => s.state)
  const activeScreenId = useWallpaper((s) => s.activeScreenId)
  const selectScreen = useWallpaper((s) => s.selectScreen)

  const screens = state?.screens ?? []
  if (!shouldShowSwitcher(screens.length, state?.spanActive ?? false)) return null

  const { tiles, width, height } = arrangeTiles(screens, MAX_W, MAX_H)

  return (
    <div className={cn('absolute left-2.5 top-2.5 z-10 rounded-[12px] p-2', GLASS)}>
      <div className="relative" style={{ width, height }}>
        {tiles.map((tile) => {
          const screen = screens[tile.index]
          const selected = screen.monitorId === activeScreenId
          const dynamic = screen.slideshowActive || !screen.hasReadableSource
          const n = tile.index + 1
          return (
            <motion.button
              key={screen.monitorId}
              type="button"
              title={format(t('Paper_SelectScreen'), n)}
              aria-label={format(t('Paper_SelectScreen'), n)}
              aria-pressed={selected}
              onClick={() => selectScreen(screen.monitorId)}
              whileTap={{ scale: 0.95 }}
              className={cn(
                'absolute overflow-hidden rounded-[4px] ring-1 transition-shadow',
                selected ? 'ring-2 ring-coral' : 'ring-glass-ring hover:ring-t3/50',
              )}
              style={{ left: tile.left, top: tile.top, width: tile.width, height: tile.height }}
            >
              {screen.source?.url ? (
                <img
                  src={screen.source.url}
                  alt=""
                  draggable={false}
                  className="absolute inset-0 size-full object-cover"
                />
              ) : (
                <span className="absolute inset-0 bg-chip" />
              )}
              <span className="absolute left-0.5 top-0.5 flex min-w-[11px] items-center justify-center rounded-[3px] bg-black/45 px-0.5 text-[8px] font-semibold leading-[13px] text-white tabular-nums">
                {n}
              </span>
              {tile.isPrimary && (
                <span
                  title={t('Paper_ScreenPrimary')}
                  className="absolute right-0.5 top-0.5 size-1.5 rounded-full bg-coral ring-1 ring-white/70"
                />
              )}
              {dynamic && (
                <span className="absolute bottom-0.5 left-0.5 rounded-[3px] bg-amber-wash px-1 text-[8px] font-medium leading-[13px] text-amber">
                  {t('Paper_DynamicChip')}
                </span>
              )}
            </motion.button>
          )
        })}
      </div>
    </div>
  )
}
