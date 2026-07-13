import { CALM_CATALOG, controlById, type CalmControl, type CalmControlId } from '@/lib/calm/catalog'
import type { CalmRowState } from '@/lib/calm/states'
import { FRAME_H, FRAME_W, SCHEMATICS } from '@/lib/calm/schematic-map'
import { bloomStaggerMs } from '@/lib/motion'
import { Highlight, highlightVisual } from './schematic-parts'
import { SceneLayers } from './scenes'
import { cn } from '@/lib/utils'

// The row schematic (viz panel 2026-07-13): a 104×64 mini screen answering
// 关的是哪里 at a glance — abstract wireframe + ONE coral highlight on the
// operation area. There is deliberately NO hero establishing image (owner
// 2026-07-13): a fixed picture would hard-code the starter count and lie the
// moment exclusions change; the per-row schematics carry all the visuals.

export function SurfaceSchematic({
  control,
  state,
  delay = 0,
  className,
}: {
  control: CalmControl
  state: CalmRowState
  /** Batch-apply stagger in seconds (bloomStaggerMs × row index / 1000). */
  delay?: number
  className?: string
}) {
  const spec = SCHEMATICS[control.id]
  return (
    <svg
      viewBox={`0 0 ${FRAME_W} ${FRAME_H}`}
      className={cn('block shrink-0', className)}
      role="img"
      aria-hidden
    >
      <rect x="0.5" y="0.5" width={FRAME_W - 1} height={FRAME_H - 1} rx="8" fill="var(--raised)" stroke="var(--hair)" />
      <SceneLayers scene={spec.scene} control={control.id} state={state} delay={delay} />
      <Highlight region={spec.region} visual={highlightVisual(state)} />
    </svg>
  )
}

/** Batch stagger index of a one-click row (view-side cascade; store stays instant). */
export function applyStaggerDelay(id: CalmControlId, oneClickIds: CalmControlId[]): number {
  const i = oneClickIds.indexOf(id)
  return i <= 0 ? 0 : (i * bloomStaggerMs) / 1000
}

/** Dev sanity: every catalog id has a schematic spec (drift guard for new rows). */
export function assertSchematicCoverage(): string[] {
  return CALM_CATALOG.filter((c) => !SCHEMATICS[c.id]).map((c) => controlById(c.id).id)
}
