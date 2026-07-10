import * as React from 'react'
import { call } from '@/bridge/client'
import { WallpaperCompositor } from '@/compositor/renderer'
import type { ZoneMeta } from '@/compositor/renderer'
import { registerCompositor } from '@/compositor/registry'
import { useWallpaper } from '@/stores/wallpaper'
import type { WallpaperStateDto } from '@/bridge/types'

// Compositor lifecycle (split from wallpaper-mirror.tsx, ≤500-line law):
// create once the canvas + state exist, feed it the host-decoded source,
// register for the store's apply/bake path, tear down on unmount.

export function useWallpaperCompositor({
  canvasRef,
  compositorRef,
  state,
  setZoneMeta,
  setReady,
}: {
  canvasRef: React.RefObject<HTMLCanvasElement | null>
  compositorRef: React.MutableRefObject<WallpaperCompositor | null>
  state: WallpaperStateDto | null
  setZoneMeta: (meta: Record<string, ZoneMeta>) => void
  setReady: (ready: boolean) => void
}): void {
  React.useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !state) return
    let cancelled = false
    let instance: WallpaperCompositor | null = null
    ;(async () => {
      const source = await call('wallpaper.getSource')
      const response = await fetch(source.url)
      const bitmap = await createImageBitmap(await response.blob())
      if (cancelled) return
      instance = await WallpaperCompositor.create(
        canvas,
        { bitmap, width: bitmap.width, height: bitmap.height },
        state.grid,
        state.wallTint,
      )
      if (cancelled) {
        instance.destroy()
        return
      }
      compositorRef.current = instance
      registerCompositor(instance)
      instance.onZoneMeta(setZoneMeta)
      const currentLook = useWallpaper.getState().look
      if (currentLook) instance.update(currentLook)
      setReady(true)
    })().catch((err) => console.error('compositor init failed', err))
    return () => {
      cancelled = true
      registerCompositor(null)
      compositorRef.current?.destroy()
      compositorRef.current = null
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state?.grid.screenWidth, state?.grid.screenHeight, !!state])
}
