import type { ReactNode } from 'react'
import type { CalmControlId } from '@/lib/calm/catalog'
import type { CalmRowState } from '@/lib/calm/states'
import type { CalmScene } from '@/lib/calm/schematic-map'
import { NoiseGroup } from './schematic-parts'

// The nine mini-screen scenes, redrawn against REAL Windows 11 layout research
// (win11-layout-scout 2026-07-13): weather/widgets pinned FAR LEFT, the centered
// start→search→taskview cluster, search flyout news in the RIGHT column,
// notification calendar at the BOTTOM, lock-screen status cards TOP-LEFT, the
// Settings promo cards in the grid below the device header. Icon glyphs are our
// own simple geometry (flat 2×2 start squares, magnifier, stacked task-view
// rects, ^ tray chevron, weather pill) — evocative, never extracted brand art.
// The row's noise elements wrap in NoiseGroup so an honest settle empties the
// marked region. Scenes stay wireframes: placeholder ink, never fake screenshots.

type SceneProps = { control: CalmControlId; state: CalmRowState; delay?: number }

const INK = 'var(--t3)'

function Placeholder({ x, y, w, h, o = 0.3, rx = 1.5 }: { x: number; y: number; w: number; h: number; o?: number; rx?: number }) {
  return <rect x={x} y={y} width={w} height={h} rx={rx} fill={INK} opacity={o} />
}

/** Bottom anchor bar for popup/window scenes — "this floats above your taskbar". */
function TaskbarAnchor() {
  return <rect x="4" y="57" width="96" height="5" rx="2" fill="var(--chip)" />
}

// ---- own-geometry mini glyphs (scout's shape descriptions) ----

/** Flat 2×2 window panes with a cross gutter (Win11 start, upright, never skewed). */
function StartSquares({ x, y, s = 6 }: { x: number; y: number; s?: number }) {
  const p = s / 2 - 0.4
  return (
    <g fill={INK} opacity="0.55">
      <rect x={x} y={y} width={p} height={p} rx="0.4" />
      <rect x={x + p + 0.8} y={y} width={p} height={p} rx="0.4" />
      <rect x={x} y={y + p + 0.8} width={p} height={p} rx="0.4" />
      <rect x={x + p + 0.8} y={y + p + 0.8} width={p} height={p} rx="0.4" />
    </g>
  )
}

function Magnifier({ x, y, r = 2 }: { x: number; y: number; r?: number }) {
  return (
    <g stroke={INK} strokeWidth="1" fill="none" opacity="0.55" strokeLinecap="round">
      <circle cx={x} cy={y} r={r} />
      <line x1={x + r * 0.75} y1={y + r * 0.75} x2={x + r * 1.7} y2={y + r * 1.7} />
    </g>
  )
}

/** Two overlapping rounded rects — Win11 task view. */
function TaskViewGlyph({ x, y }: { x: number; y: number }) {
  return (
    <g fill="none" stroke={INK} strokeWidth="1" opacity="0.55">
      <rect x={x} y={y} width="4.6" height="4.6" rx="1" />
      <rect x={x + 2} y={y + 2} width="4.6" height="4.6" rx="1" fill="var(--chip)" />
    </g>
  )
}

/** The tray "show hidden icons" caret. */
function ChevronUp({ x, y }: { x: number; y: number }) {
  return (
    <path
      d={`M ${x - 1.8} ${y + 1.2} L ${x} ${y - 1} L ${x + 1.8} ${y + 1.2}`}
      fill="none"
      stroke={INK}
      strokeWidth="1.1"
      strokeLinecap="round"
      strokeLinejoin="round"
      opacity="0.6"
    />
  )
}

/** The far-left live weather pill (amber sun + temperature dash). */
function WeatherPill({ x, y }: { x: number; y: number }) {
  return (
    <g>
      <rect x={x} y={y} width="15" height="9" rx="3" fill="var(--chip)" />
      <circle cx={x + 4.5} cy={y + 4.5} r="2.1" fill="var(--amber)" opacity="0.85" />
      <Placeholder x={x + 8} y={y + 3.5} w={5} h={2} o={0.45} rx={1} />
    </g>
  )
}

/** The Win11 taskbar strip drawn once, reused by every taskbar-rooted scene. */
function TaskbarStrip({ control, state, delay }: SceneProps) {
  const search = (
    <g>
      <rect x="36" y="47.5" width="26" height="9" rx="4.5" fill="var(--raised)" stroke="var(--hair)" />
      <Magnifier x={41.5} y={52} r={1.8} />
      <Placeholder x={46} y={51} w={13} h={2} o={0.35} rx={1} />
    </g>
  )
  const taskview = <TaskViewGlyph x={66} y={48.5} />
  return (
    <>
      <rect x="4" y="44" width="96" height="16" rx="4" fill="var(--chip)" />
      {/* far left: the live weather / widgets entry */}
      <WeatherPill x={6} y={47.5} />
      {/* centered cluster: start → search → task view → pinned */}
      <StartSquares x={27} y={48.5} />
      {control === 'taskbar.search' ? <NoiseGroup state={state} delay={delay}>{search}</NoiseGroup> : search}
      {control === 'taskbar.taskview' ? <NoiseGroup state={state} delay={delay}>{taskview}</NoiseGroup> : taskview}
      {[75, 81].map((x) => (
        <Placeholder key={x} x={x} y={49} w={4.5} h={5.5} o={0.35} rx={1.2} />
      ))}
      {/* right: hidden-icons caret · status pills · clock */}
      <ChevronUp x={88.5} y={52} />
      <Placeholder x={91.5} y={49.5} w={4} h={5} o={0.3} rx={1} />
      <Placeholder x={96.5} y={49.5} w={2.5} h={5} o={0.35} rx={0.8} />
    </>
  )
}

function TaskbarScene(props: SceneProps) {
  return (
    <>
      <rect x="6" y="8" width="92" height="30" rx="4" fill="var(--chip)" opacity="0.4" />
      <TaskbarStrip {...props} />
    </>
  )
}

/** Search flyout: full-width input + tabs; LEFT = top apps + recent; the news /
 *  trending "quick searches" live in the RIGHT ~40% column (the noise). */
function SearchPanelScene({ state, delay }: SceneProps) {
  return (
    <>
      <TaskbarAnchor />
      <rect x="14" y="4" width="76" height="50" rx="6" fill="var(--raised)" stroke="var(--hair)" />
      <rect x="19" y="8" width="66" height="6" rx="3" fill="var(--chip)" />
      <Magnifier x={23} y={11} r={1.5} />
      {[19, 28, 37].map((x) => (
        <Placeholder key={x} x={x} y={16.5} w={7} h={2} o={0.25} rx={1} />
      ))}
      {/* left column: top apps grid + recent rows */}
      {[22, 30].map((y) => [19, 27, 35, 43].map((x) => <Placeholder key={`${x}-${y}`} x={x} y={y} w={6} h={6} o={0.3} rx={1.5} />))}
      {[40, 45, 50].map((y) => (
        <Placeholder key={y} x={19} y={y} w={32} h={3} o={0.22} rx={1.5} />
      ))}
      {/* right column: quick searches / trending news — the noise */}
      <NoiseGroup state={state} delay={delay}>
        <rect x="57" y="20" width="28" height="32" rx="3" fill="var(--chip)" />
        <Placeholder x={60} y={23} w={16} h={2.2} o={0.45} rx={1} />
        <rect x="60" y="27.5" width="22" height="9" rx="2" fill={INK} opacity="0.18" />
        <Placeholder x={60} y={39} w={20} h={2} o={0.35} rx={1} />
        <Placeholder x={60} y={43} w={17} h={2} o={0.3} rx={1} />
        <Placeholder x={60} y={47} w={19} h={2} o={0.25} rx={1} />
      </NoiseGroup>
    </>
  )
}

/** Start panel: pinned 6-wide grid on the top two thirds, the Recommended band
 *  (the noise) on the bottom third, user/power footer. */
function StartScene({ state, delay }: SceneProps) {
  return (
    <>
      <TaskbarAnchor />
      <rect x="22" y="4" width="60" height="50" rx="6" fill="var(--raised)" stroke="var(--hair)" />
      <rect x="27" y="8" width="50" height="5" rx="2.5" fill="var(--chip)" />
      {[16, 23].map((y) =>
        [27, 36, 45, 54, 63, 72].map((x) => <Placeholder key={`${x}-${y}`} x={x} y={y} w={5.5} h={5} o={0.3} rx={1.2} />),
      )}
      <NoiseGroup state={state} delay={delay}>
        <Placeholder x={27} y={31.5} w={14} h={2.2} o={0.4} rx={1} />
        {[35.5, 41].map((y) =>
          [27, 52].map((x) => (
            <g key={`${x}-${y}`}>
              <rect x={x} y={y} width="23" height="4.5" rx="1.5" fill="var(--chip)" />
              <circle cx={x + 2.5} cy={y + 2.2} r="1.3" fill={INK} opacity="0.35" />
            </g>
          )),
        )}
      </NoiseGroup>
      <circle cx="30" cy="50" r="1.8" fill={INK} opacity="0.35" />
      <Placeholder x={33.5} y={49} w={9} h={2} o={0.3} rx={1} />
      <Placeholder x={73} y={48.5} w={3.5} h={3.5} o={0.35} rx={1} />
    </>
  )
}

/** Notification center: slides from the right, cards on top (the suggestion card
 *  is the noise), the month calendar at the BOTTOM. */
function NotifScene({ state, delay }: SceneProps) {
  return (
    <>
      <TaskbarAnchor />
      <rect x="56" y="4" width="42" height="52" rx="6" fill="var(--raised)" stroke="var(--hair)" />
      <Placeholder x={60} y={8} w={12} h={2.2} o={0.4} rx={1} />
      <rect x="60" y="12.5" width="34" height="7" rx="2" fill="var(--chip)" />
      <NoiseGroup state={state} delay={delay}>
        <rect x="60" y="22" width="34" height="9" rx="2" fill="var(--chip)" />
        <circle cx="64" cy="26.5" r="2" fill={INK} opacity="0.4" />
        <Placeholder x={68} y={24} w={18} h={1.8} o={0.4} rx={0.9} />
        <Placeholder x={68} y={27.5} w={12} h={1.8} o={0.3} rx={0.9} />
      </NoiseGroup>
      <rect x="60" y="33.5" width="34" height="6" rx="2" fill="var(--chip)" />
      {/* month calendar at the bottom */}
      <rect x="60" y="42" width="34" height="11" rx="2" fill="var(--chip)" />
      {[45, 48.5].map((y) => [63, 68, 73, 78, 83, 88].map((x) => <Placeholder key={`${x}-${y}`} x={x} y={y} w={3} h={2.2} o={0.25} rx={0.6} />))}
    </>
  )
}

/** Settings home: left nav, device header on top, the promo card in the
 *  interactive card grid below the header (the noise). */
function SettingsScene({ state, delay }: SceneProps) {
  return (
    <>
      <TaskbarAnchor />
      <rect x="8" y="4" width="88" height="50" rx="5" fill="var(--raised)" stroke="var(--hair)" />
      <rect x="12" y="9" width="17" height="40" rx="3" fill="var(--chip)" opacity="0.7" />
      {[13, 19, 25, 31].map((y) => (
        <Placeholder key={y} x={14} y={y} w={13} h={2.2} rx={1} />
      ))}
      {/* device header: desktop thumbnail + name lines */}
      <rect x="33" y="9" width="18" height="11" rx="2" fill="var(--chip)" />
      <Placeholder x={54} y={11} w={22} h={2.4} o={0.4} rx={1} />
      <Placeholder x={54} y={15.5} w={16} h={2} o={0.3} rx={1} />
      {/* interactive card grid; the promo/suggestion card is the noise */}
      <rect x="33" y="24" width="29" height="12" rx="2.5" fill="var(--chip)" />
      <NoiseGroup state={state} delay={delay}>
        <rect x="65" y="24" width="29" height="12" rx="2.5" fill="var(--chip)" />
        <circle cx="70" cy="29" r="2" fill={INK} opacity="0.4" />
        <Placeholder x={74} y={26.5} w={16} h={2} o={0.4} rx={1} />
        <Placeholder x={74} y={30} w={12} h={2} o={0.3} rx={1} />
      </NoiseGroup>
      <rect x="33" y="39" width="29" height="12" rx="2.5" fill="var(--chip)" />
      <rect x="65" y="39" width="29" height="12" rx="2.5" fill="var(--chip)" />
    </>
  )
}

/** File Explorer: command bar, address bar, and the OneDrive/Office promo banner
 *  right below the address bar, above the file list (the noise). */
function ExplorerScene({ state, delay }: SceneProps) {
  return (
    <>
      <TaskbarAnchor />
      <rect x="8" y="4" width="88" height="50" rx="5" fill="var(--raised)" stroke="var(--hair)" />
      {[12, 20, 28].map((x) => (
        <Placeholder key={x} x={x} y={7.5} w={6} h={2.5} o={0.3} rx={1.2} />
      ))}
      <rect x="12" y="12.5" width="82" height="5" rx="2" fill="var(--chip)" />
      <NoiseGroup state={state} delay={delay}>
        <rect x="26" y="20" width="68" height="7" rx="2" fill="var(--chip)" />
        <circle cx="30.5" cy="23.5" r="1.8" fill={INK} opacity="0.4" />
        <Placeholder x={34} y={22.5} w={34} h={2} o={0.4} rx={1} />
        <Placeholder x={84} y={22} w={7} h={3} o={0.4} rx={1.5} />
      </NoiseGroup>
      <rect x="12" y="20" width="11" height="30" rx="2.5" fill="var(--chip)" opacity="0.7" />
      {[31, 39].map((y) => [26, 44, 62, 80].map((x) => <Placeholder key={`${x}-${y}`} x={x} y={y} w={14} h={5} o={0.25} rx={2} />))}
      <Placeholder x={26} y={47} w={60} h={3} o={0.2} rx={1.5} />
    </>
  )
}

/** SCOOBE full-screen takeover: centered title + toggle list + the primary
 *  continue button; the whole interruption is the noise. */
function SystemFullScene({ state, delay }: SceneProps) {
  return (
    <NoiseGroup state={state} delay={delay}>
      <rect x="3" y="3" width="98" height="58" rx="6" fill="var(--chip)" />
      <circle cx="52" cy="15" r="4.5" fill={INK} opacity="0.25" />
      <Placeholder x={34} y={23} w={36} h={3} o={0.45} rx={1.5} />
      <Placeholder x={40} y={28.5} w={24} h={2} o={0.3} rx={1} />
      {[34, 54].map((x) =>
        [34, 40].map((y) => (
          <g key={`${x}-${y}`}>
            <rect x={x} y={y} width="16" height="4" rx="2" fill="var(--raised)" opacity="0.9" />
            <circle cx={x + 13.5} cy={y + 2} r="1.2" fill={INK} opacity="0.35" />
          </g>
        )),
      )}
      <Placeholder x={34} y={49} w={8} h={2} o={0.3} rx={1} />
      <rect x="56" y="47.5" width="14" height="5" rx="2.5" fill="var(--coral)" opacity="0.7" />
    </NoiseGroup>
  )
}

/** Widgets board: slides from the LEFT — search on top, gear at the top-right,
 *  pinned weather/calendar cards, then the MSN news feed below (the noise). */
function WidgetsScene({ state, delay }: SceneProps) {
  return (
    <>
      <TaskbarAnchor />
      <rect x="6" y="4" width="56" height="50" rx="5" fill="var(--raised)" stroke="var(--hair)" />
      <rect x="10" y="8" width="34" height="5" rx="2.5" fill="var(--chip)" />
      <circle cx="56" cy="10.5" r="2" fill={INK} opacity="0.35" />
      {/* pinned cards: weather + calendar */}
      <rect x="10" y="16" width="24" height="12" rx="2.5" fill="var(--chip)" />
      <circle cx="15" cy="21" r="2.4" fill="var(--amber)" opacity="0.85" />
      <Placeholder x={19} y={19} w={10} h={2} o={0.4} rx={1} />
      <rect x="37" y="16" width="21" height="12" rx="2.5" fill="var(--chip)" />
      <Placeholder x={40} y={19} w={13} h={2} o={0.35} rx={1} />
      {/* the news feed below — the noise */}
      <NoiseGroup state={state} delay={delay}>
        {[31, 42].map((y) =>
          [10, 35].map((x) => (
            <g key={`${x}-${y}`}>
              <rect x={x} y={y} width="23" height="9" rx="2" fill="var(--chip)" />
              <rect x={x + 1.5} y={y + 1.5} width="7" height="6" rx="1" fill={INK} opacity="0.18" />
              <Placeholder x={x + 10} y={y + 2} w={11} h={1.8} o={0.4} rx={0.9} />
              <Placeholder x={x + 10} y={y + 5} w={8} h={1.8} o={0.3} rx={0.9} />
            </g>
          )),
        )}
      </NoiseGroup>
    </>
  )
}

/** Lock screen: the status widget cards live at the TOP-LEFT (22H2+), the big
 *  clock centered in the upper half, app status dots at the bottom. */
function LockScene({ state, delay }: SceneProps) {
  return (
    <>
      <rect x="2" y="2" width="100" height="60" rx="7" fill="var(--chip)" opacity="0.8" />
      <NoiseGroup state={state} delay={delay}>
        {[6, 24, 42].map((x, i) => (
          <g key={x}>
            <rect x={x} y={6} width="16" height="9" rx="2" fill="var(--raised)" opacity="0.9" />
            {i === 0 && <circle cx={x + 3.5} cy={10.5} r="1.8" fill="var(--amber)" opacity="0.85" />}
            <Placeholder x={x + (i === 0 ? 7 : 3)} y={9.5} w={i === 0 ? 6 : 10} h={2} o={0.4} rx={1} />
          </g>
        ))}
      </NoiseGroup>
      <Placeholder x={38} y={22} w={28} h={9} o={0.5} rx={2} />
      <Placeholder x={43} y={34} w={18} h={2.5} o={0.35} rx={1.2} />
      {[46, 52, 58].map((x) => (
        <Placeholder key={x} x={x} y={52} w={3.5} h={3.5} o={0.3} rx={1} />
      ))}
    </>
  )
}

const SCENES: Record<CalmScene, (p: SceneProps) => ReactNode> = {
  taskbar: TaskbarScene,
  start: StartScene,
  searchPanel: SearchPanelScene,
  notif: NotifScene,
  settings: SettingsScene,
  systemFull: SystemFullScene,
  explorer: ExplorerScene,
  widgets: WidgetsScene,
  lock: LockScene,
}

export function SceneLayers({ scene, ...props }: SceneProps & { scene: CalmScene }) {
  const Scene = SCENES[scene]
  return <Scene {...props} />
}
