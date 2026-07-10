import * as React from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import type { LookDto, WallpaperGridInfoDto } from '@/bridge/types'

// 「分区落版」 wave (spec 04 §4.3): the wallpaper module's signature apply
// moment. A coral light-line sweeps the desktop while each zone blooms
// (scale + brightness) in reading order — the ceremony that says "baked into
// the wallpaper", and a curtain over the bake/SetWallpaper latency.
// Reduced motion: one calm 120ms brightness pulse, no sweep.

const BLOOM_EASE = [0.34, 1.4, 0.4, 1] as const

export function ApplyWave({
  wave,
  look,
  grid,
}: {
  /** Increments per successful apply; 0 = never. */
  wave: number
  look: LookDto
  grid: WallpaperGridInfoDto
}) {
  const reduced = useReducedMotion()
  const [playing, setPlaying] = React.useState<number | null>(null)

  React.useEffect(() => {
    if (wave === 0) return
    setPlaying(wave)
    const timer = setTimeout(() => setPlaying(null), reduced ? 260 : 1400)
    return () => clearTimeout(timer)
  }, [wave, reduced])

  // Reading order (top-left first) for the stagger.
  const ordered = [...look.zones].sort((a, b) => a.cellY - b.cellY || a.cellX - b.cellX)

  return (
    <AnimatePresence>
      {playing !== null && (
        <motion.div
          key={playing}
          className="pointer-events-none absolute inset-0"
          initial={{ opacity: 1 }}
          exit={{ opacity: 0, transition: { duration: 0.15 } }}
          aria-hidden
        >
          {!reduced && (
            <motion.div
              className="absolute inset-y-0 w-[12%]"
              style={{
                background:
                  'linear-gradient(90deg, transparent, rgba(255,111,94,0.22) 45%, rgba(255,255,255,0.30) 50%, rgba(255,111,94,0.22) 55%, transparent)',
              }}
              initial={{ x: '-110%' }}
              animate={{ x: `${grid.screenWidth}px` }}
              transition={{ duration: 0.3, ease: 'easeInOut' }}
            />
          )}
          {ordered.map((z, i) => {
            const rect = {
              left: grid.inset + z.cellX * grid.cellWidth,
              top: grid.inset + z.cellY * grid.cellHeight,
              width: z.cellsWide * grid.cellWidth,
              height: z.cellsTall * grid.cellHeight,
            }
            return (
              <motion.div
                key={z.id}
                className="absolute bg-white"
                style={{ ...rect, borderRadius: z.cornerRadius }}
                initial={{ opacity: 0, scale: reduced ? 1 : 0.97 }}
                animate={{ opacity: [0, reduced ? 0.25 : 0.35, 0], scale: 1 }}
                transition={
                  reduced
                    ? { duration: 0.12 }
                    : { duration: 0.48, delay: 0.18 + i * 0.06, ease: BLOOM_EASE }
                }
              />
            )
          })}
        </motion.div>
      )}
    </AnimatePresence>
  )
}
