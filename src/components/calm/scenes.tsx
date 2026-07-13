import type { ReactNode } from 'react'
import type { CalmControlId } from '@/lib/calm/catalog'
import type { CalmRowState } from '@/lib/calm/states'
import type { CalmScene } from '@/lib/calm/schematic-map'
import { NoiseGroup, ReflowGroup, ShrinkRect, noiseGone } from './schematic-parts'

// The nine mini-screen scenes, drawn AGAINST DOWNLOADED REAL SCREENSHOTS
// (win11-screenshot-hunter 2026-07-13, scratchpad/win11-shots/ — taskbar
// close-ups tb-left/tb-right, widgets-old-vs-new, search-before-after,
// explorer-backup-banner, lockscreen-hero, scoobe-hero, settings-home,
// notif-collapsed-calendar). Pixel truths encoded here:
//   · taskbar: weather pill FAR LEFT (icon + two text lines) · centered cluster =
//     BLUE flat start squares → dark search capsule (magnifier left, colourful
//     daily-highlight icon right) → task-view stack → colourful pinned apps ·
//     tray = ^ caret · status pills · two-line clock.
//   · widgets board: anchored LEFT (~60% width); inside it the personal cards sit
//     in the left third and the MSN news/ads feed fills the RIGHT two thirds.
//   · search flyout: narrow Suggested list left (~30%), main area right (~70%)
//     with a promo hero card, news rows and trending pills — the right side is
//     the noise.
//   · explorer: no full-width banner — a cloud breadcrumb chip in the address bar
//     plus a bubble card right below it.
//   · lock screen: big clock top-centre, the status/weather cards CENTRED below.
// #0067C0/#4CC2FF are the reviewed OS-authentic mirror hexes (banned-colors
// exception set) — they depict Windows itself, never our chrome.

type SceneProps = { control: CalmControlId; state: CalmRowState; delay?: number }

const INK = 'var(--t3)'
const WIN_BLUE = '#0067C0'
const WIN_BLUE_LIGHT = '#4CC2FF'

function Placeholder({ x, y, w, h, o = 0.3, rx = 1.5 }: { x: number; y: number; w: number; h: number; o?: number; rx?: number }) {
  return <rect x={x} y={y} width={w} height={h} rx={rx} fill={INK} opacity={o} />
}

/** Bottom anchor bar for popup/window scenes — "this floats above your taskbar". */
function TaskbarAnchor() {
  return <rect x="4" y="57" width="96" height="5" rx="2" fill="var(--chip)" />
}

// ---- glyphs traced from the real close-ups ----

/** Flat 2×2 Windows-blue panes (tb-left.png: upright, even gutter). */
function StartSquares({ x, y, s = 6.4 }: { x: number; y: number; s?: number }) {
  const p = s / 2 - 0.5
  return (
    <g fill={WIN_BLUE_LIGHT}>
      <rect x={x} y={y} width={p} height={p} rx="0.3" />
      <rect x={x + p + 1} y={y} width={p} height={p} rx="0.3" />
      <rect x={x} y={y + p + 1} width={p} height={p} rx="0.3" />
      <rect x={x + p + 1} y={y + p + 1} width={p} height={p} rx="0.3" />
    </g>
  )
}

function Magnifier({ x, y, r = 1.8 }: { x: number; y: number; r?: number }) {
  return (
    <g stroke={INK} strokeWidth="1" fill="none" opacity="0.55" strokeLinecap="round">
      <circle cx={x} cy={y} r={r} />
      <line x1={x + r * 0.75} y1={y + r * 0.75} x2={x + r * 1.6} y2={y + r * 1.6} />
    </g>
  )
}

/** Task view, per the owner's pixel description of the real button (2026-07-13):
 *  a SOLID dark-grey square at the bottom, with a SEMI-TRANSPARENT WHITE square
 *  overlapping its top-right corner — the overlap lightens through. */
function TaskViewGlyph({ x, y }: { x: number; y: number }) {
  return (
    <g>
      <rect x={x} y={y + 2.2} width="5" height="5" rx="1.1" fill={INK} opacity="0.6" />
      <rect
        x={x + 2.4}
        y={y}
        width="5"
        height="5"
        rx="1.1"
        fill="white"
        fillOpacity="0.72"
        stroke={INK}
        strokeOpacity="0.28"
        strokeWidth="0.5"
      />
    </g>
  )
}

/** Tray "show hidden icons" caret. */
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

/** Far-left live weather: icon + TWO stacked text lines (82°F / Clear). */
function WeatherEntry({ x, y }: { x: number; y: number }) {
  return (
    <g>
      <circle cx={x + 2.6} cy={y + 4.5} r="2.4" fill="var(--amber)" opacity="0.85" />
      <Placeholder x={x + 6.5} y={y + 1.5} w={7} h={2} o={0.45} rx={1} />
      <Placeholder x={x + 6.5} y={y + 5.5} w={5} h={2} o={0.3} rx={1} />
    </g>
  )
}

/** Colourful pinned-app dots (yellow folder + neutral/teal companions). */
function PinnedApps({ x, y }: { x: number; y: number }) {
  return (
    <g>
      <rect x={x} y={y} width="5" height="5" rx="1.2" fill="#FFC94A" />
      <rect x={x + 7} y={y} width="5" height="5" rx="1.2" fill="var(--raised)" stroke="var(--hair)" />
      <rect x={x + 14} y={y} width="5" height="5" rx="1.2" fill="#3FB6A8" opacity="0.75" />
    </g>
  )
}

/** The Win11 taskbar drawn from tb-left/tb-right (shared by taskbar scenes).
 *  When this row's element is verified-gone, the cluster REFLOWS left exactly
 *  like the real taskbar compacts — no empty socket is ever left behind. */
function TaskbarStrip({ control, state, delay }: SceneProps) {
  const searchGone = control === 'taskbar.search' && noiseGone(state)
  const taskviewGone = control === 'taskbar.taskview' && noiseGone(state)
  const search = (
    <g>
      <rect x="34" y="47.5" width="26" height="9" rx="4.5" fill="var(--raised)" stroke="var(--hair)" />
      <Magnifier x={39} y={52} />
      <Placeholder x={43.5} y={51} w={9} h={2} o={0.35} rx={1} />
      {/* the colourful daily search-highlight icon at the capsule's right end */}
      <circle cx="56" cy="52" r="1.9" fill="var(--amber)" opacity="0.9" />
      <circle cx="56.7" cy="51.4" r="0.7" fill={WIN_BLUE_LIGHT} />
    </g>
  )
  const taskview = <TaskViewGlyph x={63.5} y={48.5} />
  return (
    <>
      <rect x="4" y="44" width="96" height="16" rx="4" fill="var(--chip)" />
      {/* far left: live weather entry (icon + two text lines) */}
      {control === 'taskbar.widgetsButton' ? (
        <NoiseGroup state={state} delay={delay}>
          <WeatherEntry x={6} y={47.5} />
        </NoiseGroup>
      ) : (
        <WeatherEntry x={6} y={47.5} />
      )}
      {/* centered cluster: blue start → search capsule → task view → pinned apps */}
      <StartSquares x={26} y={48.5} />
      {control === 'taskbar.search' ? <NoiseGroup state={state} delay={delay}>{search}</NoiseGroup> : search}
      <ReflowGroup gone={searchGone} dx={-28}>
        {control === 'taskbar.taskview' ? <NoiseGroup state={state} delay={delay}>{taskview}</NoiseGroup> : taskview}
        <ReflowGroup gone={taskviewGone} dx={-10.5}>
          <PinnedApps x={73} y={49.5} />
        </ReflowGroup>
      </ReflowGroup>
      {/* right: hidden-icons caret · status pills · two-line clock */}
      {control === 'tray.entries' ? (
        <NoiseGroup state={state} delay={delay}>
          <ChevronUp x={91.5} y={52} />
        </NoiseGroup>
      ) : (
        <ChevronUp x={91.5} y={52} />
      )}
      <Placeholder x={94} y={48.5} w={4.5} h={3} o={0.35} rx={1} />
      <Placeholder x={94} y={53} w={4.5} h={3} o={0.3} rx={1} />
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

/** Search flyout (search-before-after.png): Suggested list on the LEFT ~30%,
 *  the main area right — promo hero card, news rows, trending pills = the noise. */
function SearchPanelScene({ state, delay }: SceneProps) {
  const gone = noiseGone(state)
  return (
    <>
      <TaskbarAnchor />
      {/* the flyout NARROWS to hug the Suggested list once the promo side is
          gone — search becomes the compact launcher it should have been */}
      <ShrinkRect gone={gone} x={10} y={4} w={84} h={50} goneW={32} rx={6} fill="var(--raised)" stroke="var(--hair)" />
      <ShrinkRect gone={gone} x={14} y={8} w={76} h={6} goneW={24} rx={3} fill="var(--chip)" />
      <Magnifier x={18} y={11} r={1.5} />
      {/* left: the Suggested app/recents list */}
      {[18, 24, 30, 36, 42, 48].map((y) => (
        <g key={y}>
          <rect x="14" y={y} width="4" height="4" rx="1" fill={INK} opacity="0.3" />
          <Placeholder x={20} y={y + 1} w={16} h={2} o={0.25} rx={1} />
        </g>
      ))}
      {/* right main area: promo hero + news rows + trending pills — the noise */}
      <NoiseGroup state={state} delay={delay}>
        <rect x="41" y="18" width="30" height="15" rx="2.5" fill={INK} opacity="0.18" />
        <Placeholder x={44} y={21} w={14} h={2.4} o={0.45} rx={1} />
        {[19.5, 25, 30.5].map((y) => (
          <g key={y}>
            <rect x="74" y={y} width="16" height="4" rx="1.5" fill="var(--chip)" />
          </g>
        ))}
        <Placeholder x={41} y={37} w={17} h={2.2} o={0.4} rx={1} />
        {[41, 46].map((y) =>
          [41, 58, 75].map((x) => (
            <rect key={`${x}-${y}`} x={x} y={y} width="15" height="3.5" rx="1.75" fill="var(--chip)" />
          )),
        )}
      </NoiseGroup>
    </>
  )
}

/** Start panel (start-old-classic.png): pinned 6-wide grid, the slim Recommended
 *  item rows below, avatar left / power right in the footer. The copy promises
 *  「你自己的常用文件保留」 so the header + YOUR-files row stay neutral; only the
 *  app-promotion row is the noise, and the footer compacts into the freed space
 *  (acceptance P1a/P1b 2026-07-13 — no hollow socket mid-panel). */
function StartScene({ state, delay }: SceneProps) {
  return (
    <>
      <TaskbarAnchor />
      <rect x="22" y="4" width="60" height="50" rx="6" fill="var(--raised)" stroke="var(--hair)" />
      <rect x="27" y="8" width="50" height="5" rx="2.5" fill="var(--chip)" />
      {[16, 23].map((y) =>
        [27, 36, 45, 54, 63, 72].map((x) => <Placeholder key={`${x}-${y}`} x={x} y={y} w={5.5} h={5} o={0.3} rx={1.2} />),
      )}
      {/* Recommended header + your recent-files row — kept, per the copy promise */}
      <Placeholder x={27} y={32} w={14} h={2.2} o={0.4} rx={1} />
      {[27, 52].map((x) => (
        <g key={x}>
          <rect x={x} y={36.5} width="3.5" height="3.5" rx="1" fill={INK} opacity="0.35" />
          <Placeholder x={x + 5.5} y={37.3} w={17} h={1.8} o={0.28} rx={0.9} />
        </g>
      ))}
      {/* the app-promotion row is the noise */}
      <NoiseGroup state={state} delay={delay}>
        {[27, 52].map((x) => (
          <g key={x}>
            <rect x={x} y={41.5} width="3.5" height="3.5" rx="1" fill={INK} opacity="0.35" />
            <Placeholder x={x + 5.5} y={42.3} w={17} h={1.8} o={0.28} rx={0.9} />
          </g>
        ))}
      </NoiseGroup>
      {/* footer compacts up into the freed row */}
      <ReflowGroup gone={noiseGone(state)} dy={-5}>
        <circle cx="30" cy="50" r="1.8" fill={INK} opacity="0.35" />
        <Placeholder x={33.5} y={49} w={9} h={2} o={0.3} rx={1} />
        <Placeholder x={73} y={48.5} w={3.5} h={3.5} o={0.35} rx={1} />
      </ReflowGroup>
    </>
  )
}

/** Notification center (notif-collapsed-calendar.jpg): right-edge flyout,
 *  notification cards on TOP (the suggestion card is the noise), clock/calendar
 *  card at the BOTTOM. */
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
      {/* the stack below the suggestion card compacts up once it is gone */}
      <ReflowGroup gone={noiseGone(state)} dy={-11.5}>
        <rect x="60" y="33.5" width="34" height="6" rx="2" fill="var(--chip)" />
        {/* clock + month calendar at the bottom */}
        <rect x="60" y="42" width="34" height="11" rx="2" fill="var(--chip)" />
        <Placeholder x={63} y={44.5} w={10} h={3} o={0.45} rx={1} />
        {[48.5].map((y) => [63, 68, 73, 78, 83, 88].map((x) => <Placeholder key={`${x}-${y}`} x={x} y={y} w={3} h={2.2} o={0.25} rx={0.6} />))}
      </ReflowGroup>
    </>
  )
}

/** Settings home (settings-home.jpg): left nav, device header, card grid below —
 *  the promo/suggestion cards live in the RIGHT column (the noise). */
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
      {/* card grid; the promo/suggestion cards = the right column (the noise).
          Once gone, the remaining cards REFLOW through the grid like the real
          settings home — the lower-left card slides into the freed top slot. */}
      <rect x="33" y="24" width="29" height="12" rx="2.5" fill="var(--chip)" />
      <ReflowGroup gone={noiseGone(state)} dx={32} dy={-15}>
        <rect x="33" y="39" width="29" height="12" rx="2.5" fill="var(--chip)" />
      </ReflowGroup>
      <NoiseGroup state={state} delay={delay}>
        <rect x="65" y="24" width="29" height="12" rx="2.5" fill="var(--chip)" />
        <circle cx="70" cy="29" r="2" fill={INK} opacity="0.4" />
        <Placeholder x={74} y={26.5} w={16} h={2} o={0.4} rx={1} />
        <Placeholder x={74} y={30} w={12} h={2} o={0.3} rx={1} />
        <rect x="65" y="39" width="29" height="12" rx="2.5" fill="var(--chip)" />
        <Placeholder x={69} y={42} w={18} h={2} o={0.4} rx={1} />
        <Placeholder x={69} y={45.5} w={10} h={2.5} o={0.45} rx={1.2} />
      </NoiseGroup>
    </>
  )
}

/** File Explorer (explorer-backup-banner.jpg): NOT a full-width banner — a cloud
 *  breadcrumb chip in the address bar plus the bubble card right below it. */
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
        {/* the Start OneDrive breadcrumb chip inside the address bar… */}
        <rect x="14" y="13" width="14" height="4" rx="2" fill={INK} opacity="0.22" />
        <circle cx="17.5" cy="15" r="1.3" fill={INK} opacity="0.5" />
        {/* …and the bubble card hanging right below it */}
        <rect x="26" y="20" width="44" height="16" rx="3" fill={INK} opacity="0.15" />
        <Placeholder x={30} y={23} w={30} h={2} o={0.45} rx={1} />
        <Placeholder x={30} y={26.5} w={24} h={2} o={0.3} rx={1} />
        <rect x="30" y="30.5" width="14" height="3.5" rx="1.75" fill={INK} opacity="0.45" />
      </NoiseGroup>
      <rect x="12" y="20" width="11" height="30" rx="2.5" fill="var(--chip)" opacity="0.7" />
      {/* the file grid slides up into the freed space */}
      <ReflowGroup gone={noiseGone(state)} dy={-16}>
        {[40, 47].map((y) => [26, 44, 62, 80].map((x) => <Placeholder key={`${x}-${y}`} x={x} y={y} w={14} h={5} o={0.25} rx={2} />))}
      </ReflowGroup>
    </>
  )
}

/** SCOOBE (scoobe-hero.webp): full-screen takeover — floating app icons left,
 *  title + feature rows centre-right, the primary Continue bottom-right. The
 *  calm desktop sits underneath, revealed when the takeover exits. */
function SystemFullScene({ state, delay }: SceneProps) {
  return (
    <>
      <rect x="6" y="8" width="92" height="42" rx="4" fill="var(--chip)" opacity="0.4" />
      <TaskbarAnchor />
      <NoiseGroup state={state} delay={delay}>
      <rect x="3" y="3" width="98" height="58" rx="6" fill="var(--chip)" />
      {/* floating decorative app icons on the left */}
      {[
        [12, 14], [20, 26], [10, 38], [18, 48],
      ].map(([x, y]) => (
        <rect key={`${x}-${y}`} x={x} y={y} width="6" height="6" rx="1.6" fill={INK} opacity="0.25" />
      ))}
      {/* title + feature rows, centre-right */}
      <Placeholder x={36} y={10} w={40} h={3.2} o={0.45} rx={1.5} />
      <Placeholder x={36} y={16} w={28} h={2} o={0.3} rx={1} />
      {[23, 31, 39].map((y) => (
        <g key={y}>
          <rect x="36" y={y} width="5" height="5" rx="1.4" fill={INK} opacity="0.3" />
          <Placeholder x={44} y={y + 0.5} w={26} h={1.8} o={0.4} rx={0.9} />
          <Placeholder x={44} y={y + 3.2} w={20} h={1.5} o={0.25} rx={0.75} />
        </g>
      ))}
      {/* remind-me link + the primary Continue, bottom-right */}
      <Placeholder x={58} y={51} w={14} h={2} o={0.3} rx={1} />
      <rect x="76" y="49" width="16" height="5.5" rx="2.5" fill={WIN_BLUE} opacity="0.85" />
      </NoiseGroup>
    </>
  )
}

/** Widgets board (widgets-old-vs-new.png): anchored to the LEFT edge (~60%
 *  width); personal cards fill the board's left third, the MSN news/ads feed
 *  fills the board's RIGHT two thirds — the feed is the noise. */
function WidgetsScene({ state, delay }: SceneProps) {
  const gone = noiseGone(state)
  return (
    <>
      <TaskbarAnchor />
      {/* the board itself SLIMS to hug your cards once the feed is gone; the
          desktop grows into the freed space (acceptance P2 — no phantom width) */}
      <ShrinkRect gone={gone} x={4} y={4} w={62} h={50} goneW={25} rx={5} fill="var(--raised)" stroke="var(--hair)" />
      {/* left third: personal widget cards (weather + phone/watchlist) */}
      <rect x="8" y="9" width="17" height="16" rx="2.5" fill="var(--chip)" />
      <circle cx="12.5" cy="13.5" r="2.2" fill="var(--amber)" opacity="0.85" />
      <Placeholder x={16} y={12.5} w={7} h={2} o={0.4} rx={1} />
      <rect x={10} y={18} width="13" height="5" rx="1.5" fill={INK} opacity="0.18" />
      <rect x="8" y="28" width="17" height="10" rx="2.5" fill="var(--chip)" />
      <rect x="8" y="41" width="17" height="9" rx="2.5" fill="var(--chip)" />
      {/* right two thirds: the news/ads feed — the noise */}
      <NoiseGroup state={state} delay={delay}>
        <rect x="28" y="9" width="34" height="17" rx="2.5" fill={INK} opacity="0.18" />
        <Placeholder x={31} y={20} w={22} h={2.4} o={0.5} rx={1} />
        {[29, 40].map((y) =>
          [28, 46].map((x) => (
            <g key={`${x}-${y}`}>
              <rect x={x} y={y} width="16" height="9" rx="2" fill="var(--chip)" />
              <Placeholder x={x + 1.5} y={y + 1.5} w={13} h={1.6} o={0.35} rx={0.8} />
              <Placeholder x={x + 1.5} y={y + 4.2} w={9} h={1.6} o={0.25} rx={0.8} />
            </g>
          )),
        )}
      </NoiseGroup>
      {/* desktop sliver on the right — the board does NOT span the screen */}
      <ShrinkRect gone={gone} x={70} y={10} w={28} h={40} goneX={33} goneW={65} rx={3} fill="var(--chip)" opacity={0.35} />
    </>
  )
}

/** Lock screen (lockscreen-hero.webp): big clock top-centre, date under it, the
 *  weather/status cards CENTRED below the clock (the noise). */
function LockScene({ state, delay }: SceneProps) {
  return (
    <>
      <rect x="2" y="2" width="100" height="60" rx="7" fill="var(--chip)" opacity="0.8" />
      <Placeholder x={38} y={10} w={28} h={9} o={0.5} rx={2} />
      <Placeholder x={43} y={22} w={18} h={2.5} o={0.35} rx={1.2} />
      <NoiseGroup state={state} delay={delay}>
        {[26, 44, 62].map((x, i) => (
          <g key={x}>
            <rect x={x} y={31} width="16" height="10" rx="2" fill="var(--raised)" opacity="0.9" />
            {i === 0 && <circle cx={x + 3.5} cy={36} r="1.8" fill="var(--amber)" opacity="0.85" />}
            <Placeholder x={x + (i === 0 ? 7 : 3)} y={34.8} w={i === 0 ? 6 : 10} h={2} o={0.4} rx={1} />
          </g>
        ))}
      </NoiseGroup>
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
