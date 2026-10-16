import * as React from 'react'
import type { ConfigDto, GridDto, IconItemDto } from '@/bridge/types'
import { useT } from '@/lib/i18n'
import { cn } from '@/lib/utils'
import { TileGlyph } from './icons-tile'

// The Start-panel mock view (ADR-0024, spec 06 §4 amended — fork draft): OUR
// chrome inside the mirror canvas, a Win11-flavoured acrylic panel listing the
// REAL scanned Start menu shortcuts grouped by their parent folder. Tiles render
// through the SAME renderer as the desktop grid (hover try-on, press-to-peek,
// owned-verb menu, exception badges — tile behavior is uniform across surfaces).
// D7 preserved: the switch to this view is the canvas toolbar's chip; the
// scenery underneath never gains interactivity.

/** One compact Start tile: the styled glyph + label, peek + the owned menu. */
function StartTile({
  item,
  grid,
  config,
  showOriginal,
  renderSize,
  renderTick,
  waveStamp,
  unstyleableTip,
  peekTip,
  onMenu,
}: {
  item: IconItemDto
  grid: GridDto
  config: ConfigDto
  showOriginal: boolean
  renderSize: number
  renderTick: number
  waveStamp: number
  unstyleableTip: string | null
  peekTip: string
  onMenu: (item: IconItemDto, x: number, y: number) => void
}) {
  const [peeking, setPeeking] = React.useState(false)
  const lineHeight = Math.ceil(grid.labelFontPx * 1.35)
  const menuAt = (e: React.MouseEvent, x: number, y: number) => {
    const host = (e.currentTarget.closest('.cursor-grab') as HTMLElement).getBoundingClientRect()
    onMenu(item, x - host.left, y - host.top)
  }
  return (
    <div
      data-tile
      title={unstyleableTip ?? (item.styleable ? peekTip : undefined)}
      className="group relative flex select-none flex-col items-center rounded-[10px] px-1 pb-1.5 pt-1 hover:bg-black/[.05] dark:hover:bg-white/[.08]"
      style={{ width: grid.cellWidth }}
      onPointerDown={(e) => {
        if (e.button === 0 && item.styleable) setPeeking(true)
      }}
      onPointerUp={() => setPeeking(false)}
      onPointerLeave={() => setPeeking(false)}
      onContextMenu={(e) => {
        e.preventDefault()
        e.stopPropagation()
        if (!item.styleable) return // no fake affordances on un-editable items (spec 06 §3.5)
        menuAt(e, e.clientX, e.clientY)
      }}
    >
      {item.styleable && (
        <button
          type="button"
          aria-label={item.label}
          aria-haspopup="menu"
          onPointerDown={(e) => e.stopPropagation()}
          onClick={(e) => {
            e.preventDefault()
            e.stopPropagation()
            const btn = e.currentTarget.getBoundingClientRect()
            menuAt(e, btn.left, btn.bottom)
          }}
          className="absolute right-1 top-1 z-10 hidden size-5 items-center justify-center rounded-full bg-glass text-cardtitle leading-none text-glass-ink ring-1 ring-glass-ring backdrop-blur-md transition-colors group-hover:flex hover:bg-raised-hov"
        >
          ⋯
        </button>
      )}
      {item.overrideMode !== null && (
        <span
          className="pointer-events-none absolute left-1 top-1 z-10 size-2 rounded-full bg-coral ring-1 ring-white/70"
          aria-hidden
        />
      )}
      <TileGlyph
        item={item}
        config={config}
        showOriginal={showOriginal || peeking}
        renderSize={renderSize}
        renderTick={renderTick}
        waveStamp={waveStamp}
        size={grid.iconPx}
      />
      <span
        className={cn(
          'mt-1 w-full overflow-hidden text-center text-[#F2F2F0]',
          item.styleable ? '' : 'opacity-80',
        )}
        style={{
          fontFamily: 'var(--font-os-mirror)',
          fontSize: grid.labelFontPx,
          lineHeight: `${lineHeight}px`,
          maxHeight: lineHeight * 2,
          maxWidth: grid.cellWidth - 6,
          textShadow: '0 1px 2px rgba(0,0,0,.55), 0 0 6px rgba(0,0,0,.35)',
          display: '-webkit-box',
          WebkitLineClamp: 2,
          WebkitBoxOrient: 'vertical',
        }}
      >
        {item.label}
      </span>
    </div>
  )
}

/** The panel itself: grouped sections (root first, then alphabetical parent). */
export function StartPanel({
  items,
  grid,
  config,
  showOriginal,
  renderSize,
  renderTick,
  waveStamp,
  unstyleableTip,
  peekTip,
  onMenu,
}: {
  items: IconItemDto[]
  grid: GridDto
  config: ConfigDto
  showOriginal: boolean
  renderSize: number
  renderTick: number
  waveStamp: number
  unstyleableTip: (item: IconItemDto) => string | null
  peekTip: string
  onMenu: (item: IconItemDto, x: number, y: number) => void
}) {
  const t = useT()
  const groups = React.useMemo(() => {
    const map = new Map<string, IconItemDto[]>()
    for (const i of items) {
      const list = map.get(i.parent) ?? []
      list.push(i)
      map.set(i.parent, list)
    }
    // Root group first, then alphabetical — folders read top-to-bottom.
    return [...map.entries()].sort(([a], [b]) => (a === '' ? -1 : b === '' ? 1 : a.localeCompare(b)))
  }, [items])

  return (
    <div
      className="absolute left-1/2 top-[12%] z-10 flex max-h-[74%] w-[min(860px,74%)] -translate-x-1/2 flex-col overflow-y-auto rounded-2xl border border-white/10 bg-[rgba(243,243,243,0.86)] p-5 shadow-elev-3 backdrop-blur-[24px] backdrop-saturate-[1.6] dark:bg-[rgba(32,32,34,0.80)] dark:border-white/[.08]"
      style={{ fontFamily: 'var(--font-os-mirror)' }}
    >
      {groups.length === 0 ? (
        <p className="py-10 text-center text-[12px] text-black/55 dark:text-white/55">{t('Start_Empty')}</p>
      ) : (
        groups.map(([parent, list]) => (
          <section key={parent === '' ? '__root__' : parent} className="mb-4 shrink-0 last:mb-0">
            <p className="mb-1.5 text-[12px] font-semibold text-black/70 dark:text-white/75">
              {parent === '' ? t('Start_Pinned') : parent.replace(/\\/g, ' \\ ')}
            </p>
            <div className="flex flex-wrap gap-0.5">
              {list.map((item) => (
                <StartTile
                  key={item.id}
                  item={item}
                  grid={grid}
                  config={config}
                  showOriginal={showOriginal}
                  renderSize={renderSize}
                  renderTick={renderTick}
                  waveStamp={waveStamp}
                  unstyleableTip={unstyleableTip(item)}
                  peekTip={peekTip}
                  onMenu={onMenu}
                />
              ))}
            </div>
          </section>
        ))
      )}
    </div>
  )
}

/** The taskbar-surface view (ADR-0024): the real pins enlarged into a centered
 *  row on the wallpaper — same renderer, same verbs as every other tile. */
export function TaskbarPinsView({
  items,
  grid,
  config,
  showOriginal,
  renderSize,
  renderTick,
  waveStamp,
  unstyleableTip,
  peekTip,
  onMenu,
}: {
  items: IconItemDto[]
  grid: GridDto
  config: ConfigDto
  showOriginal: boolean
  renderSize: number
  renderTick: number
  waveStamp: number
  unstyleableTip: (item: IconItemDto) => string | null
  peekTip: string
  onMenu: (item: IconItemDto, x: number, y: number) => void
}) {
  const t = useT()
  return (
    <div className="absolute inset-x-0 top-[38%] z-10 flex items-center justify-center px-8">
      {items.length === 0 ? (
        <p
          className="rounded-xl bg-black/45 px-4 py-2.5 text-[12px] text-white/75 backdrop-blur"
          style={{ fontFamily: 'var(--font-os-mirror)' }}
        >
          {t('Taskbar_NoPins')}
        </p>
      ) : (
        <div className="flex flex-wrap items-start justify-center gap-1">
          {items.map((item) => (
            <StartTile
              key={item.id}
              item={item}
              grid={grid}
              config={config}
              showOriginal={showOriginal}
              renderSize={renderSize}
              renderTick={renderTick}
              waveStamp={waveStamp}
              unstyleableTip={unstyleableTip(item)}
              peekTip={peekTip}
              onMenu={onMenu}
            />
          ))}
        </div>
      )}
    </div>
  )
}
