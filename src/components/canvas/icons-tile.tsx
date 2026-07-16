import * as React from 'react'
import { motion, useReducedMotion } from 'motion/react'
import type { ConfigDto, GridDto, IconItemDto } from '@/bridge/types'
import { effectiveTileConfig, useIcons, fieldRenderOpts } from '@/stores/icons'
import { DEFAULT_KIND_POLICY, kindBucket } from '@/lib/kind-policy'
import { getIconCompositor, tileStyleKey } from '@/icon-compositor/icon-renderer'

// One desktop tile: a compositor-rendered bitmap + the Windows-faithful label.
// Pixel math runs in the worker POOL; this component only pulls the cached
// ImageBitmap (getTile dispatches a shard render on miss) and drawImage()s it.
// The store bumps renderTick when the pool lands new tiles (rAF-coalesced),
// which re-runs the effect and blits the fresh bitmap.

/** Pulls (or requests) one tile bitmap into a canvas element. */
function useTileBitmap(
  item: IconItemDto,
  config: ConfigDto,
  showOriginal: boolean,
  renderSize: number,
  renderTick: number,
  waveStamp: number,
): React.RefObject<HTMLCanvasElement | null> {
  const ref = React.useRef<HTMLCanvasElement>(null)
  const kindPolicy = useIcons((s) => s.state?.kindPolicy) ?? DEFAULT_KIND_POLICY
  const typeOverrides = useIcons((s) => s.hoverTypeOverrides ?? s.state?.typeOverrides)
  const eff = effectiveTileConfig(item, config, kindPolicy, typeOverrides)
  const original = showOriginal || eff.showOriginal
  const styleKey = tileStyleKey(eff.config, item.isShortcut, original, renderSize)
  const sourceUrl = item.sourceUrls[0] ?? ''

  // useLayoutEffect (not useEffect): a bloom/reveal replays by REMOUNTING the tile
  // (key={waveStamp}), handing us a fresh blank canvas — blitting in a layout
  // effect paints it BEFORE the frame shows, so it never flashes empty. CRITICAL:
  // `waveStamp` MUST be a dep — the remount swaps the canvas ELEMENT, so the effect
  // has to re-run to blit the new one; without it, an already-cached tile that
  // remounts (apply re-bloom, reveal) stays permanently blank (the "点击美化后
  // 图标消失" bug — the old comparing-hold masked it by also flipping the styleKey).
  React.useLayoutEffect(() => {
    const compositor = getIconCompositor()
    if (!sourceUrl) {
      // A degraded item (no extractable source — codex icons2-🟡10) must not keep the previous
      // render's pixels: clear to a blank cell so a tile that DEGRADED after once rendering shows
      // empty, not a stale ghost of its old artwork.
      const el = ref.current
      if (el) el.getContext('2d')?.clearRect(0, 0, el.width, el.height)
      return
    }
    if (!compositor.hasSource(item.id, sourceUrl)) {
      // The store's loader owns sources + renderTick; failures are its problem.
      compositor.loadSource(item.id, sourceUrl).catch(() => {})
      return
    }
    const image = compositor.getTile(item.id, eff.config, item.isShortcut, original, renderSize, fieldRenderOpts(item.id))
    const el = ref.current
    if (!el || !image) return // pool render dispatched — next renderTick blits it
    const w = (image as ImageBitmap).width ?? renderSize
    const h = (image as ImageBitmap).height ?? renderSize
    if (el.width !== w || el.height !== h) {
      el.width = w
      el.height = h
    }
    const ctx = el.getContext('2d')!
    ctx.clearRect(0, 0, el.width, el.height)
    ctx.drawImage(image, 0, 0)
    // styleKey covers every config axis the render depends on; waveStamp covers the
    // remount that swaps the canvas element.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [item.id, sourceUrl, styleKey, renderTick, waveStamp])

  return ref
}

export const IconTile = React.memo(function IconTile({
  item,
  grid,
  config,
  showOriginal,
  renderSize,
  renderTick,
  waveKind,
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
  waveKind: 'bloom' | 'settle' | null
  waveStamp: number
  unstyleableTip: string | null
  peekTip: string
  onMenu: (x: number, y: number) => void
}) {
  const [peeking, setPeeking] = React.useState(false)
  const reduced = useReducedMotion()
  // Scope feedback (ADR-0017 D5 hard requirement): while a type row is being
  // edited, icons OUTSIDE that type dim so the edit's reach is unmistakable.
  // Compare/peek SUSPENDS the veil (owner 2026-07-16): hold-to-compare means
  // "show me the true original" — originals under a 0.28-opacity ghost read
  // as the eye button doing nothing.
  const editingBucket = useIcons((s) => s.editingBucket)
  const dimmed = !showOriginal && !peeking && editingBucket !== null && kindBucket(item.kind) !== editingBucket
  const canvasRef = useTileBitmap(item, config, showOriginal || peeking, renderSize, renderTick, waveStamp)
  const lineHeight = Math.ceil(grid.labelFontPx * 1.35)

  // Bloom/settle sweep left→right by tile POSITION (not index): bounded by the
  // screen width regardless of icon count — the old index*42ms ran 5s+ across a
  // full desktop. bloom scale eased 0.88→0.92 (the sweep carries the drama; 100
  // tiles scaling hard at once is noise).
  const sweep = grid.screenWidth > 0 ? Math.min(1, Math.max(0, item.x / grid.screenWidth)) : 0
  const wave =
    reduced
      ? null
      : waveKind === 'bloom'
      ? { initial: { scale: 0.92, filter: 'brightness(1.25) saturate(1.15)' }, animate: { scale: 1, filter: 'brightness(1) saturate(1)' }, transition: { duration: 0.5, ease: [0.34, 1.4, 0.4, 1] as const, delay: sweep * 0.55 } }
        : waveKind === 'settle'
          ? { initial: { scale: 1.06, opacity: 0.35 }, animate: { scale: 1, opacity: 1 }, transition: { duration: 0.7, ease: [0.33, 1, 0.68, 1] as const, delay: sweep * 0.45 } }
          : null

  return (
    <motion.div
      key={waveStamp}
      data-tile
      title={unstyleableTip ?? (item.styleable ? peekTip : undefined)}
      className="group absolute flex select-none flex-col items-center rounded-[10px] pt-1.5 hover:bg-white/[.10]"
      style={{
        left: item.x,
        top: item.y,
        width: grid.cellWidth,
        height: grid.cellHeight,
        opacity: dimmed ? 0.28 : undefined,
        filter: dimmed ? 'saturate(0.4)' : undefined,
        transition: 'opacity 180ms ease, filter 180ms ease',
      }}
      initial={wave?.initial ?? false}
      animate={wave?.animate ?? {}}
      transition={wave?.transition}
      onPointerDown={(e) => {
        if (e.button === 0 && item.styleable) setPeeking(true)
      }}
      onPointerUp={() => setPeeking(false)}
      onPointerLeave={() => setPeeking(false)}
      onContextMenu={(e) => {
        e.preventDefault()
        e.stopPropagation()
        if (!item.styleable) return // no fake affordances on un-editable items (spec 06 §3.5)
        const host = (e.currentTarget.closest('.cursor-grab') as HTMLElement).getBoundingClientRect()
        onMenu(e.clientX - host.left, e.clientY - host.top)
      }}
    >
      {/* Hover ⋯ affordance — styleable items only; opens the keep/follow/tint menu. */}
      {item.styleable && (
        <button
          type="button"
          aria-label={item.label}
          aria-haspopup="menu"
          onPointerDown={(e) => e.stopPropagation()}
          onClick={(e) => {
            e.preventDefault()
            e.stopPropagation()
            const host = (e.currentTarget.closest('.cursor-grab') as HTMLElement).getBoundingClientRect()
            const btn = e.currentTarget.getBoundingClientRect()
            onMenu(btn.left - host.left, btn.bottom - host.top)
          }}
          className="absolute right-1 top-1 z-10 hidden size-5 items-center justify-center rounded-full bg-glass text-cardtitle leading-none text-glass-ink ring-1 ring-glass-ring backdrop-blur-md transition-colors group-hover:flex hover:bg-raised-hov"
        >
          ⋯
        </button>
      )}

      {/* Exception badge — a pinned override is VISIBLE state (spec 06 §3.4). */}
      {item.overrideMode !== null && (
        <span
          className="pointer-events-none absolute left-1 top-1 z-10 size-2 rounded-full bg-coral ring-1 ring-white/70"
          aria-hidden
        />
      )}

      <canvas
        ref={canvasRef}
        style={{ width: grid.iconPx, height: grid.iconPx }}
        className="pointer-events-none"
        aria-hidden
      />
      <span
        className="mt-1 overflow-hidden text-center text-[#F2F2F0]"
        style={{
          fontFamily: 'var(--font-os-mirror)',
          fontSize: grid.labelFontPx,
          lineHeight: `${lineHeight}px`,
          maxHeight: lineHeight * 2,
          maxWidth: grid.cellWidth - 6,
          // Double shadow (spec 06 §4.5): crisp contact + soft halo so labels
          // survive light wallpapers — the real Win11 treatment.
          textShadow: '0 1px 2px rgba(0,0,0,.55), 0 0 6px rgba(0,0,0,.35)',
          display: '-webkit-box',
          WebkitLineClamp: 2,
          WebkitBoxOrient: 'vertical',
        }}
      >
        {item.label}
      </span>
    </motion.div>
  )
})
