import * as React from 'react'

/** Live `window.devicePixelRatio`. A DPR change without a CSS-viewport change —
 *  dragging the window to a different-DPI monitor, an OS scale/text-size change —
 *  fires NO resize event and no ResizeObserver, so anything that sized a backing
 *  store off the old DPR keeps rendering blurry until an unrelated re-render
 *  (wv2-render audit 2026-07-15 §4). The standard signal is a one-shot
 *  `matchMedia('(resolution: …dppx)')` listener re-armed after every change. */
export function useDevicePixelRatio(): number {
  const [dpr, setDpr] = React.useState(() => window.devicePixelRatio || 1)
  React.useEffect(() => {
    const query = window.matchMedia(`(resolution: ${dpr}dppx)`)
    const onChange = () => setDpr(window.devicePixelRatio || 1)
    query.addEventListener('change', onChange)
    return () => query.removeEventListener('change', onChange)
  }, [dpr])
  return dpr
}
