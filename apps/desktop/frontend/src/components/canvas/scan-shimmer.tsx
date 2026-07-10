import { motion } from 'motion/react'

// Shared scanning skeleton for the mirror canvases (spec 04 §3.5 "load feedback").
// Four pulsing tiles over a dimmed stage — the same cue the icons canvas shows while
// the desktop is being read, reused verbatim by the paper canvas' first compose.
export function ScanShimmer() {
  return (
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
  )
}
