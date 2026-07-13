import * as React from 'react'
import { useReducedMotion } from 'motion/react'
import confetti from 'canvas-confetti'

// Celebration confetti — the industry-standard canvas-confetti library
// (catdad/canvas-confetti, MIT), the "fire from the two sides" recipe adapted to
// the two bottom CORNERS of the screen (owner: 屏幕的两个角), with a long ribbon
// shape (owner: 飘丝带). Multicolour BY DESIGN — a reviewed exception to the
// coral-only accent rule (tests/banned-colors.test.ts exempts this file). Fires
// once each time `fireKey` turns non-zero; full-window canvas at z-100.

const COLORS = ['#FF6F5E', '#FFC24B', '#4C8DFF', '#3ECF8E', '#FF5CA8', '#A78BFA', '#26CCFF', '#FF3B30', '#FFD93B']

// ONE celebration per app launch, across ALL modules (spec 02 §Ceremony "the
// ONE-per-launch celebration confetti"; spec 08 §4 "the launch's first module
// success"). In-memory — resets on restart, so every launch gets exactly one
// confetti moment: whichever module lands the first successful apply.
let launchCelebrated = false

/** The pure once-per-launch gate (unit-testable): the FIRST claim of a launch
 *  wins; every later claim, from ANY module, returns false. */
export function claimCelebration(): boolean {
  if (launchCelebrated) return false
  launchCelebrated = true
  return true
}

/** Test-only: clear the launch ledger between test cases. */
export function resetCelebrationLedger() {
  launchCelebrated = false
}

/**
 * DRY celebration trigger, shared by the icons, wallpaper AND calm applies.
 * Returns a `celebrateKey` to feed <Confetti/>, and `celebrate()` to call on a
 * successful apply — gated by `claimCelebration`; the boolean says whether it
 * actually fired this call (so the caller can pair a ripple / stand-in toast).
 */
export function useCelebration(): { celebrateKey: number; celebrate: () => boolean } {
  const [celebrateKey, setCelebrateKey] = React.useState(0)
  const celebrate = React.useCallback(() => {
    if (!claimCelebration()) return false
    setCelebrateKey(Date.now())
    return true
  }, [])
  return { celebrateKey, celebrate }
}

/** Floating-ribbon confetti overlay, above the DoneCard (z-100). */
export function Confetti({ fireKey }: { fireKey: number }) {
  const ref = React.useRef<HTMLCanvasElement>(null)
  const reduced = useReducedMotion()

  React.useEffect(() => {
    if (!fireKey || reduced) return
    const canvas = ref.current
    if (!canvas) return

    // Size the drawing buffer to the element (canvas-confetti draws in canvas.width
    // space; a bare canvas otherwise stays 300×150). resize:false — we own the size.
    canvas.width = canvas.clientWidth
    canvas.height = canvas.clientHeight
    const shoot = confetti.create(canvas, { resize: false, useWorker: false })
    // A long streamer ribbon (飘丝带) — canvas-confetti flutters every shape, so it
    // twists as it drifts down. Mostly ribbons, a few rounds for variety.
    const ribbon = confetti.shapeFromPath({ path: 'M0 0 L2.6 0 L2.6 14 L0 14 Z' })
    const base = { colors: COLORS, shapes: [ribbon, ribbon, ribbon, 'circle' as const], scalar: 1.45, ticks: 340, gravity: 1, decay: 0.92 }

    // Two cannons, one at EACH bottom corner (origin y:1), angled up-and-inward.
    const fireCorners = (particleCount: number, startVelocity: number, spread: number) => {
      shoot({ ...base, particleCount, startVelocity, spread, angle: 60, origin: { x: 0, y: 1 } })
      shoot({ ...base, particleCount, startVelocity, spread, angle: 120, origin: { x: 1, y: 1 } })
    }

    // A dense burst train (canvas-confetti's "realistic" pattern) keeps the air full.
    fireCorners(90, 62, 55)
    const timers = [
      window.setTimeout(() => fireCorners(75, 72, 70), 150),
      window.setTimeout(() => fireCorners(55, 55, 100), 320),
      window.setTimeout(() => fireCorners(40, 48, 120), 480),
    ]

    return () => {
      timers.forEach(clearTimeout)
      shoot.reset()
    }
  }, [fireKey, reduced])

  // Explicit viewport size: a bare <canvas> is inline with a 300×150 intrinsic
  // size, so `inset-0` alone won't stretch it — and canvas-confetti sizes its
  // drawing buffer from clientWidth/Height. block + w/h-screen forces full-window.
  return (
    <canvas
      ref={ref}
      className="pointer-events-none fixed inset-0 z-[100] block h-screen w-screen"
      aria-hidden
    />
  )
}
