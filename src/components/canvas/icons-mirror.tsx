import * as React from 'react'
import { motion } from 'motion/react'
import { useCanvasView } from '@/lib/canvas-view'
import { displaySize, useIcons } from '@/stores/icons'
import { format, useT } from '@/lib/i18n'
import type { IconItemDto } from '@/bridge/types'
import { BUCKET_NAME_KEY, kindBucket } from '@/lib/kind-policy'
import { cn } from '@/lib/utils'
import { TaskbarStrip } from './taskbar-strip'
import { CanvasProgress, CanvasToolbar } from './canvas-toolbar'
import { MenuRow } from './canvas-menu'
import { IconTile } from './icons-tile'

// The desktop mirror (spec 06): an equal-scale replica of the real screen —
// real wallpaper, compositor-rendered tiles at OBSERVED positions, decorative
// taskbar. Every knob repaints locally in the same frame; hover try-on paints
// the candidate config across the whole desktop.

type Menu =
  | { kind: 'tile'; item: IconItemDto; x: number; y: number }
  | { kind: 'canvas'; x: number; y: number }

export function IconsMirror() {
  const t = useT()
  const state = useIcons((s) => s.state)
  const scanExhausted = useIcons((s) => s.scanExhausted)
  const items = useIcons((s) => s.items)
  const comparing = useIcons((s) => s.comparing)
  // System Default (A1): a bare look paints every tile original — the same
  // show-original path as hold-to-compare — but persistently. A hover try-on
  // (hoverConfig) still previews over it, so browsing other presets works; and
  // hoveringBare is the symmetric try-on for the System Default card itself
  // (owner 2026-07-12: hovering it must preview like every other style card).
  const bareLook = useIcons((s) => s.bareLook)
  const hoverConfig = useIcons((s) => s.hoverConfig)
  const hoveringBare = useIcons((s) => s.hoveringBare)
  const renderTick = useIcons((s) => s.renderTick)
  const applyProgress = useIcons((s) => s.applyProgress)
  const canUndo = useIcons((s) => s.canUndo)
  const canRedo = useIcons((s) => s.canRedo)
  const zoom = useIcons((s) => s.zoom)
  const waveKind = useIcons((s) => s.waveKind)
  const waveStamp = useIcons((s) => s.waveStamp)
  const { setComparing, setZoom, rescan, setOverride, undo, redo } = useIcons.getState()

  const observerRef = React.useRef<ResizeObserver | null>(null)
  const [viewport, setViewport] = React.useState({ w: 0, h: 0 })
  const [menu, setMenu] = React.useState<Menu | null>(null)

  const attachViewport = React.useCallback((el: HTMLDivElement | null) => {
    observerRef.current?.disconnect()
    observerRef.current = null
    if (el) {
      setViewport({ w: el.clientWidth, h: el.clientHeight })
      const observer = new ResizeObserver(() => setViewport({ w: el.clientWidth, h: el.clientHeight }))
      observer.observe(el)
      observerRef.current = observer
    }
  }, [])

  const hostRef = React.useCallback(
    (el: HTMLDivElement | null) => {
      attachViewport(el)
      view.wheelRef(el)
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [attachViewport],
  )

  const view = useCanvasView({
    contentW: state?.grid.screenWidth ?? 1,
    contentH: state?.grid.screenHeight ?? 1,
    viewport,
    zoom,
    setZoom,
    initialFitMode: 'height',
    center: 'xy',
    panIgnoreSelector: '[data-tile]',
  })

  // Pre-scan the mirror is a quiet placeholder. Once the scan retry budget is
  // spent (review P2-2) it surfaces a manual re-read entry so a permanent bridge
  // failure never strands the user with no way out but a restart.
  if (!state)
    return (
      <div className="mb-3 ml-1.5 mr-1 mt-1 flex flex-1 items-center justify-center rounded-[14px] bg-canvas-stage">
        {scanExhausted && (
          <div className="flex flex-col items-center gap-3 text-center">
            <p className="text-[13px] text-t2">{t('Canvas_ScanFailed')}</p>
            <button
              type="button"
              onClick={() => void useIcons.getState().retryScan()}
              className="rounded-[9px] bg-chip px-3.5 py-1.5 text-[12px] text-t1 transition-colors hover:bg-raised-hov"
            >
              {t('Canvas_ScanRetry')}
            </button>
          </div>
        )}
      </div>
    )

  const { grid } = state
  const activeConfig = hoverConfig ?? state.config
  const renderSize = displaySize(state, view.scale)

  // Fit toggle (owner 2026-07-09): one button flips between 满宽 (full width,
  // centered) and 满高·靠左 (full height, pinned to the LEFT — where most users
  // cluster icons). reset('height') already yields that left-anchored full-height
  // view (offset.x pins to 0 and pan resets to 0 when the screen overflows
  // horizontally); reset('width') fills the width. From any custom zoom the first
  // click lands on 满宽, the next flips to 满高, and so on.
  const fitZoomed = Math.abs(zoom - 1) > 0.001
  const atWidth = !fitZoomed && view.fitMode === 'width'
  const toggleFit = () => view.reset(atWidth ? 'height' : 'width')
  const fitTipText = atWidth ? t('Zoom_FitHeight_Tip') : t('Zoom_FitWidth_Tip')

  const openCanvasMenu = (e: React.MouseEvent) => {
    e.preventDefault()
    const host = (e.currentTarget as HTMLElement).getBoundingClientRect()
    setMenu({ kind: 'canvas', x: e.clientX - host.left, y: e.clientY - host.top })
  }

  return (
    <div
      data-toast-anchor
      className="relative mb-3 ml-1.5 mr-1 mt-1 min-w-0 flex-1 overflow-hidden rounded-[14px] bg-canvas-stage shadow-elev-1 ring-1 ring-inset ring-glass-ring"
      onContextMenu={(e) => e.preventDefault()}
    >
      <div
        ref={hostRef}
        className="absolute inset-0 cursor-grab active:cursor-grabbing"
        {...view.panHandlers}
        onClick={() => setMenu(null)}
        onContextMenu={openCanvasMenu}
      >
        {/* Desktop space (real screen px, scaled) — shown immediately. Icons blit
            before paint (useLayoutEffect) as their sources land; no gray veil. */}
        <div
          className="absolute left-0 top-0"
          style={{
            width: grid.screenWidth,
            height: grid.screenHeight,
            transform: `translate(${view.offset.x + view.pan.x}px, ${view.offset.y + view.pan.y}px) scale(${view.scale})`,
            transformOrigin: '0 0',
            willChange: 'transform',
          }}
        >
          {state.wallpaperUrl && (
            <img src={state.wallpaperUrl} alt="" className="absolute inset-0 size-full object-cover" draggable={false} />
          )}

          {/* Icon tiles — compositor canvases, positions are OBSERVED truth */}
          <div className="absolute inset-0">
            {items.map((item) => (
              <IconTile
                key={item.id}
                item={item}
                grid={grid}
                config={activeConfig}
                showOriginal={comparing || hoveringBare || (bareLook && !hoverConfig)}
                renderSize={renderSize}
                renderTick={renderTick}
                waveKind={waveKind}
                waveStamp={waveStamp}
                unstyleableTip={item.styleable ? null : (item.statusReason !== 'MOCK-HOST-REASON' && item.statusReason) || t('Icons_Unstyleable')}
                peekTip={t('Icons_PeekHint')}
                onMenu={(x, y) => setMenu({ kind: 'tile', item, x, y })}
              />
            ))}
          </div>

          <TaskbarStrip height={grid.taskbarHeight} />
        </div>

        {/* Scanning shimmer */}
        {state.scanning && (
          <div className="absolute inset-0 grid place-items-center bg-black/30">
            <div className="flex gap-3">
              {[0, 1, 2, 3].map((i) => (
                <motion.span
                  key={i}
                  className="size-10 rounded-[12px] bg-white/10"
                  animate={{ opacity: [0.25, 0.7, 0.25] }}
                  transition={{ duration: 1.3, repeat: Infinity, delay: i * 0.15 }}
                />
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Apply progress: shell writes dominate, keep the user informed */}
      {applyProgress && (
        <div className="absolute right-3 top-3 rounded-[9px] bg-black/60 px-2.5 py-1 text-[11px] text-white/80 backdrop-blur">
          {t('Icons_ApplyProgress').replace('{0}', String(applyProgress.done)).replace('{1}', String(applyProgress.total))}
        </div>
      )}

      <CanvasProgress active={!!applyProgress} />
      <CanvasToolbar
        undoTip={t('History_Undo')}
        redoTip={t('History_Redo')}
        canUndo={canUndo}
        canRedo={canRedo}
        onUndo={undo}
        onRedo={redo}
        compareTip={t('Compare_Idle')}
        comparing={comparing}
        onCompareDown={() => setComparing(true)}
        onCompareUp={() => setComparing(false)}
        zoomPercent={Math.round(zoom * 100)}
        zoomMinPercent={Math.ceil(view.minZoom * 100)}
        zoomMaxPercent={Math.floor(view.maxZoom * 100)}
        onZoomPercent={(pct) => setZoom(Math.min(view.maxZoom, Math.max(view.minZoom, pct / 100)))}
        onFitLabel={fitTipText}
        fitTip={fitTipText}
        onFit={toggleFit}
        refreshTip={t('Canvas_Refresh_Tip')}
        onRefresh={() => void rescan()}
      />

      {menu?.kind === 'tile' && (
        <TileMenu
          item={menu.item}
          x={Math.min(menu.x, viewport.w - 200)}
          y={Math.min(menu.y, viewport.h - 190)}
          monoSwatches={state.monoSwatches}
          onClose={() => setMenu(null)}
          onOverride={(mode, tint) => {
            setOverride(menu.item.id, mode, tint)
            setMenu(null)
          }}
        />
      )}

      {/* Empty-canvas menu — owned verbs ONLY (spec 06 §3.8): refresh. Icon size
          was removed (owner 2026-07-09: the user sets it on their real desktop,
          not here); Windows' Sort/auto-arrange verbs are permanently out. */}
      {menu?.kind === 'canvas' && (
        <div
          className="absolute z-20 w-[188px] rounded-xl border border-hair bg-popover p-1.5 shadow-2xl"
          style={{ left: Math.min(menu.x, viewport.w - 200), top: Math.min(menu.y, viewport.h - 80) }}
        >
          <MenuRow
            onClick={() => {
              void rescan()
              setMenu(null)
            }}
          >
            {t('Icons_MenuRefresh')}
          </MenuRow>
        </div>
      )}
    </div>
  )
}

function TileMenu({
  item,
  x,
  y,
  monoSwatches,
  onClose,
  onOverride,
}: {
  item: IconItemDto
  x: number
  y: number
  monoSwatches: string[]
  onClose: () => void
  onOverride: (mode: 'keep' | 'tint' | 'follow', tint?: string) => void
}) {
  const t = useT()
  return (
    <div
      className="absolute z-20 w-[188px] rounded-xl border border-hair bg-popover p-1.5 shadow-2xl"
      style={{ left: x, top: y }}
      onClick={(e) => e.stopPropagation()}
    >
      <p className="truncate px-2 py-1 text-[11px] font-semibold text-t1">{item.label}</p>
      <MenuRow
        checked={item.overrideMode === 'keep'}
        onClick={() => onOverride(item.overrideMode === 'keep' ? 'follow' : 'keep')}
      >
        {t('Menu_Keep')}
      </MenuRow>
      <MenuRow onClick={() => onOverride('follow')}>{t('Menu_Follow')}</MenuRow>
      {(() => {
        // Batch shortcut for the persistent type policy (same store state as the
        // panel's type section): flip this icon's WHOLE bucket in one step.
        const bucket = kindBucket(item.kind)
        if (!bucket) return null
        const on = useIcons.getState().state?.kindPolicy[bucket] ?? true
        const name = t(BUCKET_NAME_KEY[bucket])
        return (
          <MenuRow
            onClick={() => {
              useIcons.getState().setKindPolicy(bucket, !on)
              onClose()
            }}
          >
            {format(t(on ? 'Icons_KeepAllKind' : 'Icons_ReincludeKind'), name)}
          </MenuRow>
        )
      })()}
      <p className="px-2 pb-1 pt-1.5 text-[10.5px] text-t3">{t('Menu_TintHeader')}</p>
      <div className="flex gap-1.5 px-2 pb-1.5">
        {monoSwatches.slice(0, 6).map((s) => (
          <button
            key={s}
            type="button"
            aria-label={s}
            className={cn(
              'size-[18px] rounded-full border border-hair hover:scale-110',
              item.overrideTint?.toUpperCase() === s.toUpperCase() && 'ring-2 ring-coral',
            )}
            style={{ background: s }}
            onClick={() => onOverride('tint', s)}
          />
        ))}
      </div>
      <button type="button" className="hidden" onClick={onClose} aria-hidden />
    </div>
  )
}

