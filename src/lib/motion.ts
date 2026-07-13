import type { Transition, Variants } from 'motion/react'

// Named motion grammar (spec 02 · Motion) — one place, every surface reuses it.
// Reduced motion: components pass `useReducedMotion()` into these helpers where
// the degradation is not a plain crossfade.

export const easeOutSoft: Transition = { duration: 0.22, ease: [0.33, 1, 0.68, 1] }
export const popTransition: Transition = { duration: 0.16, ease: [0.33, 1, 0.68, 1] }

/** Menus, pickers, dialogs, toasts. */
export const pop: Variants = {
  hidden: { opacity: 0, scale: 0.95 },
  visible: { opacity: 1, scale: 1, transition: popTransition },
  exit: { opacity: 0, scale: 0.97, transition: { duration: 0.12 } },
}

/** Cards entering (history, checklists). */
export const rise: Variants = {
  hidden: { opacity: 0, y: 10 },
  visible: { opacity: 1, y: 0, transition: { duration: 0.3, ease: [0.33, 1, 0.68, 1] } },
  exit: { opacity: 0, y: 6, transition: { duration: 0.15 } },
}

/** Compact panel / page transitions. */
export const slide: Variants = {
  hidden: { opacity: 0, x: 36 },
  visible: { opacity: 1, x: 0, transition: easeOutSoft },
  exit: { opacity: 0, x: 24, transition: { duration: 0.15 } },
}

/** Apply wave — per-tile, staggered by the caller (42ms/tile). */
export const bloom: Variants = {
  hidden: { scale: 0.88, filter: 'brightness(1.35) saturate(1.3)' },
  visible: {
    scale: 1,
    filter: 'brightness(1) saturate(1)',
    transition: { duration: 0.6, ease: [0.34, 1.4, 0.4, 1] },
  },
}

/** Restore exhale — per-tile, 24ms stagger. */
export const settle: Variants = {
  hidden: { scale: 1.06, opacity: 0.35 },
  visible: { scale: 1, opacity: 1, transition: { duration: 0.8, ease: 'easeOut' } },
}

export const bloomStaggerMs = 42
export const settleStaggerMs = 24

/** 清爽 schematic — a noise element leaving the screen = 「变安静」 made literal.
 *  SVG scaleY needs transformBox:'fill-box' set at the usage site; reduced-motion
 *  callers animate opacity only. */
export const noiseExit: Variants = {
  present: { opacity: 1, scaleY: 1 },
  quiet: { opacity: 0, scaleY: 0, transition: { duration: 0.3, ease: [0.33, 1, 0.68, 1] } },
}

/**
 * Snap pulse (spec 04 §3 / §3.5) — a zone pops scale 1.02→1.0 in 80ms when its
 * snapped cell changes during create / move / resize. Driven imperatively via
 * `useAnimationControls().start('pulse')`; callers must skip it under
 * `useReducedMotion()`.
 */
export const snapPulseTransition: Transition = { duration: 0.08, ease: [0.33, 1, 0.68, 1] }

export const snapPulse: Variants = {
  rest: { scale: 1 },
  pulse: { scale: [1.02, 1], transition: snapPulseTransition },
}

/** Collapse/expand for accordion content. */
export const collapse: Variants = {
  hidden: { height: 0, opacity: 0 },
  visible: { height: 'auto', opacity: 1, transition: { duration: 0.2, ease: [0.33, 1, 0.68, 1] } },
  exit: { height: 0, opacity: 0, transition: { duration: 0.16 } },
}
