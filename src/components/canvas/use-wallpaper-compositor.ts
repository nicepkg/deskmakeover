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
  // Bumped on `webglcontextrestored` to force a full compositor rebuild (§A1).
  const [recoverNonce, setRecoverNonce] = React.useState(0)

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

    // WebGL context loss (pitfalls doc §A1): a GPU driver update, a TDR reset, or a
    // laptop dGPU/iGPU switch destroys the context and the pixi canvas goes blank
    // FOREVER — nothing re-inits it. `preventDefault` on `lost` is REQUIRED or the
    // browser never fires `restored`; while lost we fall back to the plain wallpaper
    // <img> (the loadError path) instead of a blank canvas, and on `restored` we bump
    // the nonce to tear down the dead compositor and rebuild a fresh one. Attached
    // synchronously (before the async create) so a loss mid-init is caught too, and
    // torn down with the effect so it re-binds to each recreated canvas.
    const onContextLost = (e: Event) => {
      e.preventDefault()
      console.error('wallpaper compositor: WebGL context lost — showing the plain wallpaper until restore')
      setReady(false)
      setLoadError(true)
    }
    const onContextRestored = () => {
      console.info('wallpaper compositor: WebGL context restored — rebuilding the preview')
      setRecoverNonce((n) => n + 1)
    }
    canvas.addEventListener('webglcontextlost', onContextLost)
    canvas.addEventListener('webglcontextrestored', onContextRestored)
    ;(async () => {
      const source = await resolveActiveSource()
      if (cancelled) {
        // The compositor never took ownership of this bitmap — close it here or the
        // decoded full-res wallpaper leaks in GPU-backed memory (codex #6). Rapid
        // monitor/dimension changes and WebGL-recovery rebuilds hit this path often.
        source.bitmap.close()
        return
      }
      instance = await WallpaperCompositor.create(canvas, source, state.grid, state.wallTint)
      if (cancelled) {
        instance.destroy()
        return
      }
      compositorRef.current = instance
      registerCompositor(instance)
      instance.onZoneMeta(setZoneMeta)
      // The grid may have re-recorded (regridScreens) DURING the async create —
      // adopt the store's current lattice so the first paint is never stale.
      const currentGrid = useWallpaper.getState().state?.grid
      if (currentGrid) instance.setGrid(currentGrid)
      const currentLook = useWallpaper.getState().look
      if (currentLook) instance.update(currentLook)
      loadedScreenRef.current = useWallpaper.getState().activeScreenId
      // Mirror the loaded screen into the store so apply() can gate on it (codex #5).
      useWallpaper.setState({ loadedScreenId: loadedScreenRef.current })
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
      canvas.removeEventListener('webglcontextlost', onContextLost)
      canvas.removeEventListener('webglcontextrestored', onContextRestored)
      registerCompositor(null)
      compositorRef.current?.destroy()
      compositorRef.current = null
      loadedScreenRef.current = null
      // A torn-down / mid-recreate compositor holds no screen — apply() must refuse until reload.
      useWallpaper.setState({ loadedScreenId: null })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state?.grid.screenWidth, state?.grid.screenHeight, !!state, recoverNonce])

  // Same-dims grid LATTICE refresh (icon scan → regridScreens re-records the true
  // cell pitch after the compositor was created): push it into the live instance so
  // zone panels track the same lattice the DOM overlay renders on. Without this the
  // panels stayed on the boot fallback lattice and the selection ring drifted off
  // them (owner report 2026-07-16). Dims changes recreate above instead.
  React.useEffect(() => {
    if (state?.grid) compositorRef.current?.setGrid(state.grid)
  }, [state?.grid])

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
      // The swap landed — the compositor now holds this screen; apply() may proceed (codex #5).
      useWallpaper.setState({ loadedScreenId: activeScreenId })
      const look = useWallpaper.getState().look
      if (look) compositor.update(look)
    })().catch((err) => console.error('source swap failed', err))
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeScreenId])
}
