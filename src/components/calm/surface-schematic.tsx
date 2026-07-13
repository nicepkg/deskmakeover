import { CALM_CATALOG, controlById, type CalmControl, type CalmControlId } from '@/lib/calm/catalog'
import type { CalmRowState } from '@/lib/calm/states'
import { FRAME_H, FRAME_W, SCHEMATICS } from '@/lib/calm/schematic-map'
import { bloomStaggerMs } from '@/lib/motion'
import { Highlight, NoiseGroup, highlightVisual } from './schematic-parts'
import { SceneLayers } from './scenes'
import { cn } from '@/lib/utils'

// The row schematic (viz panel 2026-07-13): a 104×64 mini screen answering
// 关的是哪里 at a glance — abstract wireframe + ONE coral highlight on the
// operation area. The hero variant is the honest establishing shot: the start
// panel open over the taskbar (they DO coexist), lighting exactly the starter
// controls so the pin count story stays truthful.

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

// Hero establishing shot: 192×112 — desktop, the REAL Win11 taskbar order
// (weather far left · centered start→search→taskview cluster · tray right),
// start panel open with its Recommended band; the three starter-slice regions
// marked and animating on apply.
const HERO_STARTERS: CalmControlId[] = ['start.recommendations', 'taskbar.search', 'taskbar.taskview']
const HERO_REGIONS: Record<string, { x: number; y: number; w: number; h: number; rx: number }> = {
  'start.recommendations': { x: 52, y: 60, w: 88, h: 25, rx: 3 },
  'taskbar.search': { x: 61, y: 91, w: 42, h: 16, rx: 6 },
  'taskbar.taskview': { x: 104, y: 91, w: 15, h: 16, rx: 4 },
}

export function HeroSchematic({ rows, className }: { rows: Record<CalmControlId, CalmRowState>; className?: string }) {
  const stagger = (i: number) => (i * bloomStaggerMs) / 1000
  const stateOf = (id: CalmControlId) => rows[id]
  return (
    <svg viewBox="0 0 192 112" className={cn('block shrink-0', className)} role="img" aria-hidden>
      {/* screen + desktop */}
      <rect x="0.5" y="0.5" width="191" height="111" rx="10" fill="var(--raised)" stroke="var(--hair)" />
      <rect x="8" y="10" width="176" height="66" rx="5" fill="var(--chip)" opacity="0.4" />
      {/* start panel, open over the taskbar (an honest coexistence) */}
      <rect x="48" y="16" width="96" height="72" rx="7" fill="var(--raised)" stroke="var(--hair)" />
      <rect x="56" y="21" width="80" height="6" rx="3" fill="var(--chip)" />
      {[31, 41].map((y) =>
        [56, 70, 84, 98, 112, 126].map((x) => (
          <rect key={`${x}-${y}`} x={x} y={y} width="9" height="7" rx="1.8" fill="var(--t3)" opacity="0.3" />
        )),
      )}
      <NoiseGroup state={stateOf('start.recommendations')} delay={stagger(0)}>
        <rect x="56" y="63" width="26" height="3.5" rx="1.75" fill="var(--t3)" opacity="0.35" />
        {[69, 76.5].map((y) =>
          [56, 100].map((x) => (
            <g key={`${x}-${y}`}>
              <rect x={x} y={y} width="38" height="6" rx="2" fill="var(--chip)" />
              <circle cx={x + 3.5} cy={y + 3} r="1.7" fill="var(--t3)" opacity="0.35" />
            </g>
          )),
        )}
      </NoiseGroup>
      {/* taskbar: weather far left · centered cluster · tray right */}
      <rect x="6" y="90" width="180" height="18" rx="5" fill="var(--chip)" />
      <g>
        <rect x="11" y="93.5" width="24" height="11" rx="4" fill="var(--raised)" />
        <circle cx="18" cy="99" r="3" fill="var(--amber)" opacity="0.85" />
        <rect x="23" y="97.5" width="9" height="3" rx="1.5" fill="var(--t3)" opacity="0.4" />
      </g>
      {/* start squares (flat 2×2) */}
      <g fill="var(--t3)" opacity="0.55">
        <rect x="50" y="95" width="3.6" height="3.6" rx="0.6" />
        <rect x="54.4" y="95" width="3.6" height="3.6" rx="0.6" />
        <rect x="50" y="99.4" width="3.6" height="3.6" rx="0.6" />
        <rect x="54.4" y="99.4" width="3.6" height="3.6" rx="0.6" />
      </g>
      <NoiseGroup state={stateOf('taskbar.search')} delay={stagger(1)}>
        <rect x="63" y="93" width="38" height="12" rx="6" fill="var(--raised)" stroke="var(--hair)" />
        <circle cx="70" cy="99" r="2.4" fill="none" stroke="var(--t3)" strokeWidth="1.2" opacity="0.55" />
        <line x1="72" y1="101" x2="74" y2="103" stroke="var(--t3)" strokeWidth="1.2" strokeLinecap="round" opacity="0.55" />
        <rect x="78" y="97.5" width="18" height="3" rx="1.5" fill="var(--t3)" opacity="0.35" />
      </NoiseGroup>
      <NoiseGroup state={stateOf('taskbar.taskview')} delay={stagger(2)}>
        <g fill="none" stroke="var(--t3)" strokeWidth="1.2" opacity="0.55">
          <rect x="107" y="94.5" width="6.5" height="6.5" rx="1.4" />
          <rect x="110" y="97.5" width="6.5" height="6.5" rx="1.4" fill="var(--chip)" />
        </g>
      </NoiseGroup>
      {[124, 136].map((x) => (
        <rect key={x} x={x} y={95.5} width="8" height="8" rx="2" fill="var(--t3)" opacity="0.3" />
      ))}
      {/* tray: hidden-icons caret · status · clock */}
      <path d="M 158 100.5 L 160.2 98 L 162.4 100.5" fill="none" stroke="var(--t3)" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" opacity="0.6" />
      <rect x="166" y="95.5" width="7" height="8" rx="1.6" fill="var(--t3)" opacity="0.3" />
      <rect x="176" y="95.5" width="5" height="8" rx="1.4" fill="var(--t3)" opacity="0.35" />
      {HERO_STARTERS.map((id) => (
        <Highlight key={id} region={HERO_REGIONS[id]} visual={highlightVisual(rows[id])} />
      ))}
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
