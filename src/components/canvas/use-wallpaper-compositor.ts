import * as React from 'react'
import { WallpaperCompositor } from '@/compositor/renderer'
import type { CompositorSource, ZoneMeta } from '@/compositor/renderer'
import { registerCompositor } from '@/compositor/registry'
import { useWallpaper } from '@/stores/wallpaper'
import { useToasts } from '@/stores/toasts'
import { t } from '@/lib/i18n'
import type { WallpaperStateDto } from '@/bridge/types'

// Compositor lifecycle + the PER-SCREEN SOURCE SEAM (spec 04 §B2/B4). Split from
// wallpaper-mirror.tsx (≤500-line law). The compositor is a SINGLE instance: on a
// screen switch the store swaps the LOOK (selectScreen → compositor.update) but NOT
// the source bitmap, and the compositor re-inits ONLY when the grid dims change.
// So switching between two SAME-dimension screens would leave the WRONG wallpaper on
// the canvas. This hook owns the source truth:
//   • it (re)creates on grid-dims change, loading the ACTIVE screen's source, and
//   • it swaps the source into the live compositor on any activeScreenId change,
//     reading the active screen's source from the store mirror (no bridge round-trip
//     — wallpaper.getSource returns the HOST's active screen, which the client-only
//     selectScreen never syncs, so it would fetch the wrong monitor's wallpaper).

/** Resolve the ACTIVE screen's design source from the store mirror: an imported
 *  image wins, else the screen's desktop wallpaper. A screen with no readable
 *  source (third-party dynamic/video wallpaper, §A4) gets a neutral branded fill so
 *  the canvas never shows the PREVIOUS screen's wallpaper by mistake. */
async function resolveActiveSource(): Promise<CompositorSource> {
  const s = useWallpaper.getState()
  const url = s.sourceUrl ?? s.state?.originalUrl ?? null
  if (url) {
    const response = await fetch(url)
    const bitmap = await createImageBitmap(await response.blob())
    return { bitmap, width: bitmap.width, height: bitmap.height }
  }
  return flatFillSource(s.state?.wallTint ?? '#888888')
}

/** A tiny flat-colour bitmap (the sprite stretches it to grid dims) — the
 *  "awaiting import" backdrop for an unreadable dynamic-wallpaper screen. */
async function flatFillSource(tint: string): Promise<CompositorSource> {
  const canvas = document.createElement('canvas')
  canvas.width = 8
  canvas.height = 8
  const ctx = canvas.getContext('2d')!
  ctx.fillStyle = tint
  ctx.fillRect(0, 0, 8, 8)
  const bitmap = await createImageBitmap(canvas)
  return { bitmap, width: 8, height: 8 }
}

export function useWallpaperCompositor({
  canvasRef,
  compositorRef,
  state,
  setZoneMeta,
  setReady,
  setLoadError,
}: {
  canvasRef: React.RefObject<HTMLCanvasElement | null>
  compositorRef: React.MutableRefObject<WallpaperCompositor | null>
  state: WallpaperStateDto | null
  setZoneMeta: (meta: Record<string, ZoneMeta>) => void
  setReady: (ready: boolean) => void
  /** Compositor init/source-load failed: the mirror clears the loading shimmer and
   *  falls back to the plain wallpaper `<img>` instead of hanging on it forever. */
  setLoadError: (v: boolean) => void
}): void {
  const activeScreenId = useWallpaper((s) => s.activeScreenId)
  // Which screen's source the live compositor currently holds — the seam guard so a
  // same-dims switch swaps the source while a co-fired recreate never double-loads.
  const loadedScreenRef = React.useRef<string | null>(null)

  // (Re)create on grid-dims change; load the active screen's source at birth.
  React.useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !state) return
    // A different-dimension screen switch tears down + recreates: hide the stale
    // canvas until the new compositor paints (the mirror shows the new screen's
    // plain wallpaper meanwhile). A same-dims switch never re-runs this effect.
    setReady(false)
    setLoadError(false)
    let cancelled = false
    let instance: WallpaperCompositor | null = null
    ;(async () => {
      const source = await resolveActiveSource()
      if (cancelled) return
      instance = await WallpaperCompositor.create(canvas, source, state.grid, state.wallTint)
      if (cancelled) {
        instance.destroy()
        return
      }
      compositorRef.current = instance
      registerCompositor(instance)
      instance.onZoneMeta(setZoneMeta)
      const currentLook = useWallpaper.getState().look
      if (currentLook) instance.update(currentLook)
      loadedScreenRef.current = useWallpaper.getState().activeScreenId
      setReady(true)
    })().catch((err) => {
      if (cancelled) return
      // Never leave the loading shimmer stuck: clear it, fall back to the plain
      // wallpaper, and tell the user (owner report: 壁纸 page stuck loading).
      console.error('compositor init failed', err)
      setLoadError(true)
      useToasts.getState().show(t('Paper_PreviewFailed'), 'warn')
    })
    return () => {
      cancelled = true
      registerCompositor(null)
      compositorRef.current?.destroy()
      compositorRef.current = null
      loadedScreenRef.current = null
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state?.grid.screenWidth, state?.grid.screenHeight, !!state])

  // Swap the source on a SAME-dimension screen switch (a different-dimension switch
  // recreates above and that recreate already loads the right source). The
  // loadedScreenRef guard makes both paths idempotent: mid-recreate compositorRef is
  // null, so this no-ops and the recreate owns the load; steady-state it owns the swap.
  React.useEffect(() => {
    const compositor = compositorRef.current
    if (!compositor || !activeScreenId || loadedScreenRef.current === activeScreenId) return
    let cancelled = false
    ;(async () => {
      const source = await resolveActiveSource()
      if (cancelled || compositorRef.current !== compositor) return
      compositor.setSource(source)
      loadedScreenRef.current = activeScreenId
      const look = useWallpaper.getState().look
      if (look) compositor.update(look)
    })().catch((err) => console.error('source swap failed', err))
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeScreenId])
}
