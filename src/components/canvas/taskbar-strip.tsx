import * as React from 'react'
import { BatteryFull, ChevronUp, Volume2, Wifi } from 'lucide-react'
import { TASKBAR_PINNED } from './taskbar-icons'

// Decorative Win11-style taskbar strip that completes the desktop-mirror illusion
// (spec 06 §4). This is SIMULATED OS chrome, never the user's real taskbar: neutral
// glyphs (own Fluent-flavored art, no Microsoft assets, no brand logos), zero
// interactivity, no flyouts. The acrylic follows the APP theme — not the wallpaper —
// through the `dark:` variant. OS-mirror colour exemption: the OS-blue Start flag and
// the active-app indicator are allowed here; the coral accent never is. banned-
// colors.test.ts allowlists this one file from the cool-gray ban and whitelists the
// OS blues #4CC2FF / #0067C0 (spec 06 §4's lighter top-pane blue is realised with
// the already-exempt light OS blue, so no new exemption entry was needed).

// No running-indicator pills (owner order 2026-07-09): the pinned row is pure
// scenery — icons only, hover wash only.
function PinnedCell({ children }: { children: React.ReactNode }) {
  return (
    <span className="relative grid size-10 place-items-center rounded-[5px] text-black/80 hover:bg-black/[.05] dark:text-white/85 dark:hover:bg-white/[.08]">
      {children}
    </span>
  )
}

/** REAL extracted icon (dev fixture pack) with the drawn-vector fallback for
 *  environments that have not run scripts/dev/fetch-real-icons.ts. */
function PinnedIcon({ src, fallback }: { src: string; fallback: React.ReactNode }) {
  const [failed, setFailed] = React.useState(false)
  if (failed) return <>{fallback}</>
  return (
    <img
      src={`/real-icons/${src}`}
      alt=""
      width={26}
      height={26}
      draggable={false}
      onError={() => setFailed(true)}
    />
  )
}

function useClock() {
  const [now, setNow] = React.useState(() => new Date())
  React.useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 30_000)
    return () => clearInterval(timer)
  }, [])
  return now
}

export function TaskbarStrip({ height }: { height: number }) {
  const now = useClock()
  const hh = String(now.getHours()).padStart(2, '0')
  const mm = String(now.getMinutes()).padStart(2, '0')
  const date = `${now.getFullYear()}/${now.getMonth() + 1}/${now.getDate()}`

  return (
    <div
      className="absolute inset-x-0 bottom-0 border-t border-t-[rgba(255,255,255,0.06)] bg-[rgba(243,243,243,0.82)] backdrop-blur-[20px] backdrop-saturate-[1.8] dark:bg-[rgba(32,32,34,0.72)] dark:backdrop-saturate-[1.6]"
      style={{ height, fontFamily: 'var(--font-os-mirror)' }}
    >
      {/* Pinned row — centered on the whole bar (Win11 center alignment).
          REAL extracted icons from the dev fixture pack (owner order
          2026-07-09); the drawn vectors in taskbar-icons.tsx are only the
          fixture-less fallback. */}
      <div className="absolute inset-y-0 left-1/2 flex -translate-x-1/2 items-center gap-1">
        {TASKBAR_PINNED.map((g) => (
          <PinnedCell key={g.key}>
            <PinnedIcon src={g.realSrc} fallback={g.node} />
          </PinnedCell>
        ))}
      </div>

      {/* Tray cluster — right edge */}
      <div className="absolute inset-y-0 right-0 flex items-center gap-1.5 text-black/60 dark:text-white/70">
        <ChevronUp size={12} />
        <span className="flex items-center gap-1.5 rounded-md px-2 py-1 hover:bg-black/[.05] dark:hover:bg-white/[.08]">
          <Wifi size={16} />
          <Volume2 size={16} />
          <BatteryFull size={16} />
        </span>
        <span className="flex flex-col items-end text-[11px] leading-[1.15] tabular-nums">
          <span>
            {hh}:{mm}
          </span>
          <span>{date}</span>
        </span>
        {/* show-desktop sliver: a thin hover target at the far corner */}
        <span className="h-full w-[6px] hover:border-l hover:border-l-black/10 dark:hover:border-l-white/10" />
      </div>
    </div>
  )
}
