import * as React from 'react'
import { pickFitMode } from '@/lib/canvas-view'
import type { CanvasView } from '@/lib/canvas-view'
import type { WallpaperStateDto } from '@/bridge/types'

// The screen-switch canvas transition (spec 04 §A2). Two coupled concerns behind
// one hook so the mirror stays lean:
//   • FIT — re-pick the fit mode for the active screen's aspect on every switch (and
//     on an orientation flip: grid dims change). Portrait opens fit-height,
//     landscape/ultrawide fit-all. Fires once on mount to seed the initial fit.
//   • DIP — a brief opacity dip that masks the aspect/source change while the
//     compositor repaints the new screen. Returns `dip`; the mirror hides the
//     composed canvas (showing the new screen's plain wallpaper) while it is true.

export function useScreenSwitchTransition({
  view,
  state,
  activeScreenId,
}: {
  view: CanvasView
  state: WallpaperStateDto | null
  activeScreenId: string | null
}): boolean {
  const [dip, setDip] = React.useState(false)

  React.useEffect(() => {
    if (!state) return
    view.reset(pickFitMode(state.grid.screenWidth, state.grid.screenHeight))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeScreenId, state?.grid.screenWidth, state?.grid.screenHeight])

  // Dip only on a real screen→screen switch, not the initial null→first-screen seed.
  const prevScreen = React.useRef<string | null>(null)
  React.useEffect(() => {
    const prev = prevScreen.current
    prevScreen.current = activeScreenId
    if (!prev || !activeScreenId || prev === activeScreenId) return
    setDip(true)
    const timer = setTimeout(() => setDip(false), 140)
    return () => clearTimeout(timer)
  }, [activeScreenId])

  return dip
}
