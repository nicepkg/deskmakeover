import * as React from 'react'
import { AnimatePresence, useAnimationControls, useReducedMotion } from 'motion/react'
import { useApp } from '@/stores/app'
import { useWallpaper } from '@/stores/wallpaper'
import { makeZone } from '@/stores/wallpaper'
import { useT } from '@/lib/i18n'
import { useCanvasView } from '@/lib/canvas-view'
import { useDevicePixelRatio } from '@/lib/use-dpr'
import { cellOf, createFromDrag, magnetizeMove, moveZone, nudgeZone, overlapRegions, resizeZone } from '@/lib/zone-math'
import type { HandleId, MagnetGuide, ZoneRect } from '@/lib/zone-math'
import type { WallpaperCompositor } from '@/compositor/renderer'
import { MATERIAL_TITLE_DEFAULT } from '@/compositor/material'
import { EMOJI_PAGES, EmojiPicker } from '@/components/panels/wallpaper-panel-popovers'
import { MenuRow } from './canvas-menu'
import { TaskbarStrip } from './taskbar-strip'
import type { ZoneMeta } from '@/compositor/renderer'
import { useWallpaperCompositor } from './use-wallpaper-compositor'
import { orientationOfGrid, projectPreset } from '@/lib/zone-presets'
import { cn } from '@/lib/utils'
import { ScanShimmer } from './scan-shimmer'
import { ApplyWave } from './apply-wave'
import { CanvasProgress, CanvasToolbar } from './canvas-toolbar'
import { MagnetGuideLines, OverlapWash, RubberBand, ZoneView } from './zone-layer'
import { PaperEmptyState } from './paper-empty'
import { PaperCoach } from './paper-coach'
import { ScreenSwitcher } from './screen-switcher'
import { useScreenSwitchTransition } from './use-screen-switch-transition'
import { useDropImport } from './use-drop-import'

// The wallpaper mirror (spec 04 v2.0, ADR-0014): the client COMPOSITOR paints the
// composed preview (source + 壁纸压暗 + Adaptive Frost zones) on a WebGL canvas at
// viewport resolution — every look mutation repaints on the next frame, so during
// create/move/resize the MATERIAL tracks the pointer with the outline (nothing
// teleports). The editor chrome lives on top in desktop-pixel space. Fit-all by
// default; Ctrl+wheel zooms at the pointer, space/middle-drag pans, left-drag on
// empty canvas DRAWS a zone.

export function WallpaperMirror() {
  const t = useT()
  const state = useWallpaper((s) => s.state)
  const look = useWallpaper((s) => s.look)
  const activeScreenId = useWallpaper((s) => s.activeScreenId)
  const selected = useWallpaper((s) => s.selected)
  const comparing = useWallpaper((s) => s.comparing)
  const applying = useWallpaper((s) => s.applying)
  const applyWave = useWallpaper((s) => s.applyWave)
  const sourceUrl = useWallpaper((s) => s.sourceUrl)
  const canUndo = useWallpaper((s) => s.canUndo)
  const canRedo = useWallpaper((s) => s.canRedo)
  const reduced = useReducedMotion()
  const { mutateZone, addZone, duplicateZone, removeZone, applyToAllZones, select, setComparing, beginInteraction, endInteraction, undo, redo } =
    useWallpaper.getState()

  const canvasRef = React.useRef<HTMLCanvasElement>(null)
  const hostRef = React.useRef<HTMLDivElement | null>(null)
  const observerRef = React.useRef<ResizeObserver | null>(null)
  const compositorRef = React.useRef<WallpaperCompositor | null>(null)
  const [viewport, setViewport] = React.useState({ w: 0, h: 0 })
  const [zoom, setZoomState] = React.useState(1)
  const setZoom = React.useCallback((z: number) => setZoomState(Math.min(3, Math.max(0.2, z))), [])
  const [ready, setReady] = React.useState(false)
  const [loadError, setLoadError] = React.useState(false)
  const [rubber, setRubber] = React.useState<{ sx: number; sy: number; ex: number; ey: number } | null>(null)
  const [rename, setRename] = React.useState<{ id: string; value: string } | null>(null)
  const [zoneMenu, setZoneMenu] = React.useState<{ id: string; x: number; y: number } | null>(null)
  const [guides, setGuides] = React.useState<MagnetGuide[]>([])
  const [overlaps, setOverlaps] = React.useState<ZoneRect[]>([])
  const [zoneMeta, setZoneMeta] = React.useState<Record<string, ZoneMeta>>({})
  const { dropActive, dropHandlers } = useDropImport()

  const gesture = React.useRef<{
    kind: 'move' | 'resize'
    id: string
    handle?: HandleId
    startCx: number
    startCy: number
    origin: ZoneRect
    moved: boolean
  } | null>(null)
  const interaction = React.useRef<'pan' | 'create' | 'gesture' | null>(null)
  const zoneSnap = useAnimationControls()

  // Callback ref: the host div mounts AFTER state loads, so a mount-time effect
  // would observe nothing (bug class: effect ran against the early-return render).
  const attachHost = React.useCallback((el: HTMLDivElement | null) => {
    hostRef.current = el
    observerRef.current?.disconnect()
    observerRef.current = null
    if (el) {
      setViewport({ w: el.clientWidth, h: el.clientHeight })
      const observer = new ResizeObserver(() => setViewport({ w: el.clientWidth, h: el.clientHeight }))
      observer.observe(el)
      observerRef.current = observer
    }
  }, [])

  const hostMergedRef = React.useCallback(
    (el: HTMLDivElement | null) => {
      attachHost(el)
      view.wheelRef(el)
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [attachHost],
  )

  const view = useCanvasView({
    contentW: state?.grid.screenWidth ?? 1,
    contentH: state?.grid.screenHeight ?? 1,
    viewport,
    zoom,
    setZoom,
    initialFitMode: 'all',
    center: 'xy',
  })

  useWallpaperCompositor({ canvasRef, compositorRef, state, setZoneMeta, setReady, setLoadError })

  // On a screen switch: re-fit for the new aspect + a brief opacity dip that masks
  // the change (§A2). `dip` hides the composed canvas while the new source repaints.
  const dip = useScreenSwitchTransition({ view, state, activeScreenId })

  // Backing-store resolution follows the view zoom (never above native). `dpr` is
  // a real dependency: a different-DPI monitor or an OS scale change re-arms it
  // without any resize event (wv2-render audit 2026-07-15 §4).
  const dpr = useDevicePixelRatio()
  React.useEffect(() => {
    compositorRef.current?.setRenderScale(view.scale * dpr)
  }, [view.scale, dpr])

  // WebView2 can restore from minimize/tray with a stale (blank) frame — a pure
  // repaint bug on the compositor side (WebView2Feedback #5171 class). A dirty
  // invalidate on focus/visibility is a free belt: no-op when nothing changed.
  React.useEffect(() => {
    const kick = () => compositorRef.current?.invalidate()
    window.addEventListener('focus', kick)
    document.addEventListener('visibilitychange', kick)
    return () => {
      window.removeEventListener('focus', kick)
      document.removeEventListener('visibilitychange', kick)
    }
  }, [])

  // The chip under an open rename editor hides (the DOM input replaces it).
  React.useEffect(() => {
    compositorRef.current?.setRenamingZone(rename?.id ?? null)
  }, [rename?.id])

  // Keyboard: nudge (0.5 cell) + Delete + undo/redo. Backspace is NOT delete
  // (spec 04 §3.5). Ctrl/Cmd+Z / +Shift+Z are wired app-wide in App.tsx.
  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Modules stay mounted app-wide now — this handler must only act while
      // the PAPER module is the visible one (Delete in icons must not reach it).
      if (useApp.getState().module !== 'paper') return
      const s = useWallpaper.getState()
      if (s.selected === null || !s.state || e.target instanceof HTMLInputElement) return
      const { columns, rows } = s.state.grid
      const arrows: Record<string, [number, number]> = {
        ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1],
      }
      if (e.key in arrows) {
        const [dx, dy] = arrows[e.key]
        s.mutateZone(s.selected, (z) => ({ ...z, ...nudgeZone(z, dx, dy, columns, rows) }))
        e.preventDefault()
      } else if (e.key === 'Delete') {
        s.removeZone(s.selected)
        e.preventDefault()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  React.useEffect(() => () => endInteraction(), [endInteraction])

  if (!state || !look) {
    return (
      <div className="relative mb-3 ml-1.5 mr-1 mt-1 flex-1 overflow-hidden rounded-[14px] bg-canvas-stage shadow-elev-1 ring-1 ring-inset ring-glass-ring">
        <ScanShimmer />
      </div>
    )
  }

  const { grid } = state
  const panelInset = Math.min(16, Math.max(8, grid.cellWidth * 0.12))
  const ghostSize = grid.iconPx * 0.82

  const toCell = (e: { clientX: number; clientY: number }) => {
    const rect = hostRef.current!.getBoundingClientRect()
    const dx = (e.clientX - rect.left - view.offset.x - view.pan.x) / view.scale
    const dy = (e.clientY - rect.top - view.offset.y - view.pan.y) / view.scale
    return cellOf(dx, dy, grid.cellWidth, grid.cellHeight, grid.inset)
  }

  const liveSpan = rubber
    ? createFromDrag({ cx: rubber.sx, cy: rubber.sy }, { cx: rubber.ex, cy: rubber.ey }, grid.columns, grid.rows)
    : null

  const commitRename = () => {
    setRename((r) => {
      if (r) {
        const value = r.value.trim()
        if (value) mutateZone(r.id, (z) => ({ ...z, title: value }))
      }
      return null
    })
  }
  const cancelRename = () => setRename(null)

  const startRename = (id: string) => {
    select(id)
    // Fresh store read: right after addZone the RENDER look is one commit
    // stale and the new id is missing (codex review m1 — blank rename input).
    const zones = useWallpaper.getState().look?.zones ?? []
    setRename({ id, value: zones.find((z) => z.id === id)?.title ?? '' })
  }

  /** Right-click a zone (verified prior no-op) → select it + open the context
   *  menu at the cursor, host coords, clamped inside the viewport. Suppressed
   *  mid-gesture (spec 04 §3 round 3). */
  const openZoneMenu = (e: React.MouseEvent, id: string) => {
    if (interaction.current) return
    select(id)
    const host = hostRef.current!.getBoundingClientRect()
    setZoneMenu({
      id,
      x: Math.min(e.clientX - host.left, Math.max(8, host.width - 200)),
      y: Math.min(e.clientY - host.top, Math.max(8, host.height - 250)),
    })
  }

  const startZoneGesture = (e: React.PointerEvent, id: string, kind: 'move' | 'resize', handle?: HandleId) => {
    e.stopPropagation()
    setZoneMenu(null)
    if (e.button !== 0) return
    if (rename && rename.id !== id) commitRename()
    const { cx, cy } = toCell(e)
    let zone = look.zones.find((z) => z.id === id)
    if (!zone) return
    beginInteraction() // coalesce the whole drag (incl. an Alt-dup) into one undo step
    // Alt-drag duplicates, then the gesture moves the COPY (spec 04 §3).
    if (kind === 'move' && e.altKey) {
      const copyId = duplicateZone(id, { cellX: zone.cellX, cellY: zone.cellY })
      if (copyId) {
        id = copyId
        zone = useWallpaper.getState().look!.zones.find((z) => z.id === copyId)!
      }
    }
    const origin: ZoneRect = { cellX: zone.cellX, cellY: zone.cellY, cellsWide: zone.cellsWide, cellsTall: zone.cellsTall }
    gesture.current = { kind, id, handle, startCx: cx, startCy: cy, origin, moved: false }
    select(id)
    interaction.current = 'gesture'
    hostRef.current!.setPointerCapture(e.pointerId)
  }

  const onHostPointerDown = (e: React.PointerEvent) => {
    // An open context menu absorbs the first click-away (no accidental rubber band).
    if (zoneMenu) {
      setZoneMenu(null)
      return
    }
    if ((e.target as HTMLElement).closest('[data-zone]')) return
    const host = e.currentTarget as HTMLElement
    if (e.button === 1 || (e.button === 0 && comparing)) {
      view.beginPan(e)
      interaction.current = 'pan'
      host.setPointerCapture(e.pointerId)
      e.preventDefault()
      return
    }
    if (e.button !== 0) return
    if (rename) commitRename()
    const { cx, cy } = toCell(e)
    setRubber({ sx: cx, sy: cy, ex: cx, ey: cy })
    interaction.current = 'create'
    host.setPointerCapture(e.pointerId)
  }

  const onHostPointerMove = (e: React.PointerEvent) => {
    if (interaction.current === 'pan') {
      view.dragPan(e)
      return
    }
    if (interaction.current === 'create' && rubber && e.buttons & 1) {
      const { cx, cy } = toCell(e)
      setRubber({ ...rubber, ex: cx, ey: cy })
      // The forming MATERIAL tracks the drag (spec 04 §3) — not just an outline.
      const span = createFromDrag({ cx: rubber.sx, cy: rubber.sy }, { cx, cy }, grid.columns, grid.rows)
      compositorRef.current?.setProvisional(span)
      return
    }
    if (interaction.current === 'gesture' && gesture.current && e.buttons & 1) {
      const g = gesture.current
      const { cx, cy } = toCell(e)
      const dx = cx - g.startCx
      const dy = cy - g.startCy
      const others = look.zones
        .filter((z) => z.id !== g.id)
        .map((z) => ({ cellX: z.cellX, cellY: z.cellY, cellsWide: z.cellsWide, cellsTall: z.cellsTall }))
      let next: ZoneRect
      if (g.kind === 'move') {
        // Magnetism runs on the RAW rect (pre half-snap; a 0.35-cell window can
        // never fire after 0.5-step quantisation); unclaimed axes half-snap.
        const raw: ZoneRect = {
          ...g.origin,
          cellX: Math.min(Math.max(g.origin.cellX + dx, 0), Math.max(0, grid.columns - g.origin.cellsWide)),
          cellY: Math.min(Math.max(g.origin.cellY + dy, 0), Math.max(0, grid.rows - g.origin.cellsTall)),
        }
        const magnet = magnetizeMove(raw, others, grid.columns, grid.rows)
        const snapped = moveZone(g.origin, dx, dy, grid.columns, grid.rows)
        next = {
          ...raw,
          cellX: magnet.fired.x ? magnet.rect.cellX : snapped.cellX,
          cellY: magnet.fired.y ? magnet.rect.cellY : snapped.cellY,
        }
        setGuides(magnet.fired.x || magnet.fired.y ? magnet.guides : [])
      } else {
        next = resizeZone(g.origin, g.handle!, dx, dy, grid.columns, grid.rows)
      }
      setOverlaps(overlapRegions(next, others))
      g.moved = true
      mutateZone(g.id, (z) => ({ ...z, ...next }))
    }
  }

  const onHostPointerUp = () => {
    if (interaction.current === 'pan') {
      view.endPan()
    } else if (interaction.current === 'create' && rubber) {
      compositorRef.current?.setProvisional(null)
      const span = createFromDrag({ cx: rubber.sx, cy: rubber.sy }, { cx: rubber.ex, cy: rubber.ey }, grid.columns, grid.rows)
      const dragged = Math.abs(rubber.ex - rubber.sx) > 0.4 || Math.abs(rubber.ey - rubber.sy) > 0.4
      if (dragged) {
        const zone = makeZone({ ...span, title: t('Zone_DefaultTitle') })
        addZone(zone)
        startRename(zone.id) // create → rename immediately (spec 04 §3)
      }
      // A plain empty-canvas click PRESERVES selection; deselect is Esc only.
      setRubber(null)
    } else if (interaction.current === 'gesture') {
      // Snap-pulse rides the RELEASE commit only (spec 04 §3, was per-half-cell).
      if (gesture.current?.moved && !reduced) void zoneSnap.start('pulse')
      endInteraction()
      gesture.current = null
    }
    interaction.current = null
    setGuides([])
    setOverlaps([])
  }

  const resetGesture = () => {
    if (interaction.current === 'pan') view.endPan()
    endInteraction()
    compositorRef.current?.setProvisional(null)
    gesture.current = null
    interaction.current = null
    setRubber(null)
    setGuides([])
    setOverlaps([])
  }

  // The wallpaper's on-screen rect (screen space) — the empty-state card and
  // the drop overlay bind to the WALLPAPER, never the letterbox.
  const wallRect = {
    left: view.offset.x + view.pan.x,
    top: view.offset.y + view.pan.y,
    width: grid.screenWidth * view.scale,
    height: grid.screenHeight * view.scale,
  }

  return (
    <div
      data-toast-anchor
      className="relative mb-3 ml-1.5 mr-1 mt-1 min-w-0 flex-1 overflow-hidden rounded-[14px] bg-canvas-stage shadow-elev-1 ring-1 ring-inset ring-glass-ring"
    >
      <div
        ref={hostMergedRef}
        className={cn('absolute inset-0', comparing ? 'cursor-grab active:cursor-grabbing' : 'cursor-crosshair')}
        onPointerDown={onHostPointerDown}
        onPointerMove={onHostPointerMove}
        onPointerUp={onHostPointerUp}
        onPointerCancel={resetGesture}
        onLostPointerCapture={resetGesture}
        onContextMenu={(e) => e.preventDefault()} // parity with the icons canvas — never the OS menu
        {...dropHandlers}
      >
        {/* Desktop space */}
        <div
          className="absolute"
          style={{
            width: grid.screenWidth,
            height: grid.screenHeight,
            transform: `translate(${view.offset.x + view.pan.x}px, ${view.offset.y + view.pan.y}px) scale(${view.scale})`,
            transformOrigin: '0 0',
            willChange: 'transform',
          }}
        >
          {/* Composed preview — the compositor's WebGL canvas (viewport-res backing,
              stretched into desktop space). Hidden while comparing. Keyed on the grid
              dims so a different-aspect screen switch mounts a FRESH canvas (the
              compositor re-inits on the same dims trigger) instead of rebinding pixi
              to a canvas whose WebGL context was just destroyed. Same-dims switches
              keep the canvas and swap only the source. */}
          <canvas
            key={`${grid.screenWidth}x${grid.screenHeight}`}
            ref={canvasRef}
            className={cn(
              'absolute inset-0 transition-opacity',
              dip ? 'duration-[120ms]' : 'duration-[180ms]',
              comparing || !ready || dip ? 'opacity-0' : 'opacity-100',
            )}
            style={{ width: grid.screenWidth, height: grid.screenHeight }}
          />
          {(comparing || !ready || dip) && (sourceUrl ?? state.originalUrl) && (
            <img
              src={sourceUrl ?? state.originalUrl!}
              alt=""
              className="absolute inset-0 size-full object-cover"
              draggable={false}
            />
          )}

          {/* Edit chrome is hidden ENTIRELY while comparing (spec 04 §3.5) */}
          {!comparing && (
            <>
              <OverlapWash regions={overlaps} grid={grid} />
              <MagnetGuideLines guides={guides} grid={grid} scale={view.scale} />
              <AnimatePresence initial={false}>
              {look.zones.map((z) => (
                <ZoneView
                  key={z.id}
                  zone={z}
                  grid={grid}
                  scale={view.scale}
                  isSelected={selected === z.id}
                  ghostSize={ghostSize}
                  panelInset={panelInset}
                  ink={zoneMeta[z.id]?.tone === 'Dark' ? '#F4EFEA' : '#2A2622'}
                  titleOverhang={zoneMeta[z.id]?.overhang ?? true}
                  reserveFirstRow={zoneMeta[z.id]?.reserveFirstRow ?? false}
                  snapControls={zoneSnap}
                  renaming={rename?.id === z.id}
                  renameValue={rename?.id === z.id ? rename.value : ''}
                  onMoveDown={(ev, id) => startZoneGesture(ev, id, 'move')}
                  onResizeDown={(ev, id, h) => startZoneGesture(ev, id, 'resize', h)}
                  onMenu={openZoneMenu}
                  onTitleDoubleClick={startRename}
                  onRenameChange={(value) => setRename((r) => (r ? { ...r, value } : r))}
                  onRenameCommit={commitRename}
                  onRenameCancel={cancelRename}
                />
              ))}
              </AnimatePresence>
              {liveSpan && <RubberBand span={liveSpan} grid={grid} />}
            </>
          )}

          {/* 分区落版 wave — the apply signature moment (spec 04 §4.3). */}
          <ApplyWave wave={applyWave} look={look} grid={grid} />

          {/* The SAME simulated taskbar as the icons mirror (owner order
              2026-07-09: one desktop, both modules). Pure scenery here —
              pointer-events-none so zone gestures pass straight through. */}
          <div className="pointer-events-none absolute inset-0">
            <TaskbarStrip height={grid.taskbarHeight} />
          </div>
        </div>

        {/* Zone context menu (spec 04 §3 round 3) — host coords, icons-canvas
            menu dialect. Delete is red with NO confirm: removeZone already
            ships the 已删除·撤销 toast. */}
        {zoneMenu && (() => {
          const mz = look.zones.find((z) => z.id === zoneMenu.id)
          if (!mz) return null
          const hidden = mz.titleStyle === 'None'
          return (
            <div
              className="absolute z-20 w-[188px] rounded-xl border border-hair bg-popover p-1.5 shadow-2xl"
              style={{ left: zoneMenu.x, top: zoneMenu.y }}
              onClick={(e) => e.stopPropagation()}
              onPointerDown={(e) => e.stopPropagation()}
              onContextMenu={(e) => e.preventDefault()}
            >
              <p className="truncate px-2 py-1 text-[11px] font-semibold text-t1">
                {mz.emoji ? `${mz.emoji} ${mz.title}` : mz.title}
              </p>
              <MenuRow
                onClick={() => {
                  setZoneMenu(null)
                  startRename(mz.id)
                }}
              >
                {t('Zone_MenuRename')}
              </MenuRow>
              {/* Quick emoji strip (icons menu's inline-swatch grammar) + the
                  FULL shared EmojiPicker at the end — every emoji, custom
                  input and 无 stay reachable from the menu (codex r3 P3). */}
              <div className="flex items-center gap-0.5 px-1.5 pb-1 pt-0.5">
                {EMOJI_PAGES[0].emojis.slice(0, 5).map((em) => (
                  <button
                    key={em}
                    type="button"
                    onClick={() => {
                      mutateZone(mz.id, (z) => ({ ...z, emoji: z.emoji === em ? null : em }))
                      setZoneMenu(null)
                    }}
                    className={cn(
                      'flex h-6 w-6 items-center justify-center rounded-[6px] text-[13px] hover:bg-raised-hov',
                      mz.emoji === em && 'bg-wash-chip',
                    )}
                  >
                    {em}
                  </button>
                ))}
                <EmojiPicker
                  value={mz.emoji}
                  noneLabel={t('Zone_EmojiNone')}
                  onPick={(emoji) => {
                    mutateZone(mz.id, (z) => ({ ...z, emoji }))
                    setZoneMenu(null)
                  }}
                />
              </div>
              <MenuRow
                onClick={() => {
                  mutateZone(mz.id, (z) => ({
                    ...z,
                    titleStyle: hidden ? MATERIAL_TITLE_DEFAULT[z.material] : 'None',
                  }))
                  setZoneMenu(null)
                }}
              >
                {t(hidden ? 'Zone_MenuShowTitle' : 'Zone_MenuHideTitle')}
              </MenuRow>
              <MenuRow
                onClick={() => {
                  // Offset the copy one cell down-right (clamped) so it lands visible.
                  const copyId = duplicateZone(mz.id, {
                    cellX: Math.max(0, Math.min(mz.cellX + 1, grid.columns - mz.cellsWide)),
                    cellY: Math.max(0, Math.min(mz.cellY + 1, grid.rows - mz.cellsTall)),
                  })
                  if (copyId) select(copyId)
                  setZoneMenu(null)
                }}
              >
                {t('Zone_MenuDuplicate')}
              </MenuRow>
              {look.zones.length > 1 && (
                <MenuRow
                  onClick={() => {
                    applyToAllZones({
                      tone: mz.tone,
                      material: mz.material,
                      titleStyle: mz.titleStyle,
                      shadow: mz.shadow,
                      fillOpacity: mz.fillOpacity,
                      cornerRadius: mz.cornerRadius,
                      titleSize: mz.titleSize,
                      fontFamily: mz.fontFamily,
                    })
                    setZoneMenu(null)
                  }}
                >
                  {t('Zone_ApplyAll')}
                </MenuRow>
              )}
              <div className="my-1 border-t border-hair" />
              <MenuRow
                danger
                onClick={() => {
                  setZoneMenu(null)
                  removeZone(mz.id)
                }}
              >
                {t('Zone_MenuDelete')}
              </MenuRow>
            </div>
          )
        })()}

        {/* Empty state = the preset gallery on YOUR wallpaper, bound to the
            wallpaper's screen rect and NEVER before the first composed frame
            (the old dashed frame flashed on refresh for ignoring both). */}
        <PaperEmptyState
          show={ready && look.zones.length === 0 && !rubber && !comparing}
          wallpaperUrl={sourceUrl ?? state.originalUrl}
          rect={wallRect}
          orientation={orientationOfGrid(grid)}
          onPreset={(preset) => {
            const s = useWallpaper.getState()
            if (s.state) s.replaceZones(projectPreset(preset, s.state.grid))
          }}
          onImport={() => useWallpaper.getState().importSourceViaPicker()}
        />

        {/* OS file drag-drop = import (IA spec): coral ring + glass hint over
            the wallpaper rect while an image hovers. */}
        {dropActive && (
          <div className="pointer-events-none absolute z-20 grid place-items-center" style={wallRect}>
            <div className="absolute inset-0 rounded-[10px] ring-2 ring-inset ring-coral/80" />
            <p className="rounded-full bg-glass px-4 py-2 text-body font-medium text-glass-ink shadow-elev-1 ring-1 ring-glass-ring backdrop-blur-md">
              {t('Paper_DropHint')}
            </p>
          </div>
        )}

        {!ready && !loadError && <ScanShimmer />}
      </div>

      {/* Multi-monitor switcher — floating glass pill, top-left (§B4). Renders
          nothing with a single screen or in Span mode (single-monitor parity). */}
      <ScreenSwitcher />

      <CanvasProgress active={applying} />
      <CanvasToolbar
        compareTip={t('Compare_Idle')}
        comparing={comparing}
        onCompareDown={() => setComparing(true)}
        onCompareUp={() => setComparing(false)}
        zoomPercent={Math.round(zoom * 100)}
        zoomMinPercent={Math.ceil(view.minZoom * 100)}
        zoomMaxPercent={Math.floor(view.maxZoom * 100)}
        onZoomPercent={(pct) => setZoom(Math.min(view.maxZoom, Math.max(view.minZoom, pct / 100)))}
        onFitLabel={t('Zoom_FitAll_Tip')}
        fitTip={t('Zoom_FitAll_Tip')}
        onFit={() => view.reset('all')}
        undoTip={t('History_Undo')}
        redoTip={t('History_Redo')}
        canUndo={canUndo}
        canRedo={canRedo}
        onUndo={undo}
        onRedo={redo}
      />

      <PaperCoach />
    </div>
  )
}
