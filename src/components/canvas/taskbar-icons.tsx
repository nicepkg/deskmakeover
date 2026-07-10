import * as React from 'react'

// Colorful pinned-app art for the simulated Win11 taskbar (owner mockup
// 2026-07-09: flag / search / gear / folder / browser orb / store / music).
// ALL of it is our own vector art — evocative of the real desktop, no
// extracted Microsoft or brand assets, so it ships. This file is the
// simulated-OS layer (spec 02 rule 5): banned-colors.test.ts exempts its hex
// scan the same way taskbar-strip.tsx is exempt from the cool-gray ban —
// these colours depict Windows itself, never our app chrome.

type G = { children: React.ReactNode }

function Svg({ children }: G) {
  return (
    <svg viewBox="0 0 24 24" width={24} height={24} aria-hidden="true">
      {children}
    </svg>
  )
}

/** The Windows flag: four blue panes, subtle top-light gradient. */
export function FlagIcon() {
  return (
    <Svg>
      <defs>
        <linearGradient id="tbFlag" x1="0" y1="0" x2="0.4" y2="1">
          <stop offset="0" stopColor="#4CC2FF" />
          <stop offset="1" stopColor="#0067C0" />
        </linearGradient>
      </defs>
      <g transform="skewX(-2)">
        <rect x="3.4" y="3.4" width="8.1" height="8.1" rx="0.9" fill="url(#tbFlag)" />
        <rect x="12.7" y="3.4" width="8.1" height="8.1" rx="0.9" fill="url(#tbFlag)" />
        <rect x="3.4" y="12.7" width="8.1" height="8.1" rx="0.9" fill="url(#tbFlag)" />
        <rect x="12.7" y="12.7" width="8.1" height="8.1" rx="0.9" fill="url(#tbFlag)" />
      </g>
    </Svg>
  )
}

/** Search: a bold magnifier outline (currentColor — flips with the bar theme). */
export function SearchIcon() {
  return (
    <Svg>
      <circle cx="10.5" cy="10.5" r="6.2" fill="none" stroke="currentColor" strokeWidth="2.1" />
      <line x1="15.2" y1="15.2" x2="20.4" y2="20.4" stroke="currentColor" strokeWidth="2.1" strokeLinecap="round" />
    </Svg>
  )
}

/** Settings: a gray gear with a punched centre. */
export function GearIcon() {
  const teeth: React.ReactNode[] = []
  for (let i = 0; i < 8; i++) {
    teeth.push(
      <rect
        key={i}
        x="10.7"
        y="1.6"
        width="2.6"
        height="4.4"
        rx="1.1"
        fill="#8E959D"
        transform={`rotate(${i * 45} 12 12)`}
      />,
    )
  }
  return (
    <Svg>
      <defs>
        <radialGradient id="tbGear" cx="0.35" cy="0.3" r="0.9">
          <stop offset="0" stopColor="#B9BfC7" />
          <stop offset="1" stopColor="#7C838B" />
        </radialGradient>
      </defs>
      {teeth}
      <circle cx="12" cy="12" r="7.4" fill="url(#tbGear)" />
      <circle cx="12" cy="12" r="3.1" fill="#3A3F45" />
    </Svg>
  )
}

/** Explorer: the yellow Win11 folder (dark back panel, light front, teal clip). */
export function FolderIcon() {
  return (
    <Svg>
      <path
        d="M2.6 6.2c0-.9.7-1.6 1.6-1.6h4.6l2 2h10c.9 0 1.6.7 1.6 1.6v1H2.6V6.2Z"
        fill="#E8A33E"
      />
      <rect x="2.6" y="8.2" width="18.8" height="11.2" rx="1.6" fill="#FFC94A" />
      <path d="M2.6 9.8c0-.9.7-1.6 1.6-1.6h16.6c.5 0 .9.2 1.2.5l-1 1.9H2.6V9.8Z" fill="#FFD97E" />
      <rect x="8.2" y="15.6" width="7.6" height="3.8" rx="1.1" fill="#49B8C4" />
    </Svg>
  )
}

/** Browser: a swirled orb (teal→blue sweep around an aqua core). */
export function BrowserOrbIcon() {
  return (
    <Svg>
      <defs>
        <linearGradient id="tbOrbA" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#35D0C2" />
          <stop offset="0.55" stopColor="#2B7CD3" />
          <stop offset="1" stopColor="#1B4FA0" />
        </linearGradient>
        <linearGradient id="tbOrbB" x1="0" y1="1" x2="1" y2="0">
          <stop offset="0" stopColor="#8DE8B7" />
          <stop offset="1" stopColor="#35C1D0" />
        </linearGradient>
      </defs>
      <circle cx="12" cy="12" r="9.4" fill="url(#tbOrbA)" />
      <path
        d="M4.4 14.6c.6-4.4 4-7.2 8-7.2 3.4 0 6.2 1.9 7.2 4.7-1.5-1.1-3.4-1.7-5.5-1.4-3.6.4-6.3 3-6.1 6.2.1 1 .4 1.9 1 2.6-2.7-.9-4.8-2.7-4.6-4.9Z"
        fill="url(#tbOrbB)"
      />
      <circle cx="13.4" cy="13.2" r="4.1" fill="#EAF6FF" />
      <circle cx="13.4" cy="13.2" r="2.5" fill="#2B7CD3" />
    </Svg>
  )
}

/** Store: a blue bag, white window, four colored panes. */
export function StoreIcon() {
  return (
    <Svg>
      <defs>
        <linearGradient id="tbStore" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#2FA3E8" />
          <stop offset="1" stopColor="#1B6FC4" />
        </linearGradient>
      </defs>
      <path d="M8.6 7.4V6a3.4 3.4 0 0 1 6.8 0v1.4h-1.8V6a1.6 1.6 0 0 0-3.2 0v1.4H8.6Z" fill="#1B6FC4" />
      <path d="M4.4 7.4h15.2l-.9 12a1.6 1.6 0 0 1-1.6 1.5H6.9a1.6 1.6 0 0 1-1.6-1.5l-.9-12Z" fill="url(#tbStore)" />
      <rect x="7.6" y="10.4" width="8.8" height="7" rx="1" fill="#F4F8FC" />
      <rect x="8.7" y="11.5" width="3.1" height="2.3" fill="#E4573D" />
      <rect x="12.2" y="11.5" width="3.1" height="2.3" fill="#7DBA45" />
      <rect x="8.7" y="14.2" width="3.1" height="2.3" fill="#3B94D8" />
      <rect x="12.2" y="14.2" width="3.1" height="2.3" fill="#F0B73F" />
    </Svg>
  )
}

/** Music: a green disc with three dark sound arcs. */
export function MusicDiscIcon() {
  return (
    <Svg>
      <circle cx="12" cy="12" r="9.4" fill="#1ED760" />
      <path d="M6.7 9.6c3.8-1.1 8-.7 11 1.1" fill="none" stroke="#10131A" strokeWidth="1.9" strokeLinecap="round" />
      <path d="M7.2 12.8c3.1-.9 6.7-.5 9.2 1" fill="none" stroke="#10131A" strokeWidth="1.7" strokeLinecap="round" />
      <path d="M7.8 15.8c2.4-.7 5.2-.4 7.2.8" fill="none" stroke="#10131A" strokeWidth="1.5" strokeLinecap="round" />
    </Svg>
  )
}

/** The pinned row, in the owner's mockup order. `realSrc` is the pack-relative
 *  path of the REAL extracted icon inside the committed SSoT (/real-icons/);
 *  the drawn node is only the fallback when the pack is missing. */
export const TASKBAR_PINNED: { key: string; realSrc: string; node: React.ReactNode }[] = [
  { key: 'start', realSrc: 'apps/app-home.png', node: <FlagIcon /> },
  { key: 'search', realSrc: 'apps/app-search.png', node: <SearchIcon /> },
  { key: 'settings', realSrc: 'apps/app-settings.png', node: <GearIcon /> },
  { key: 'explorer', realSrc: 'folders/win-folder.png', node: <FolderIcon /> },
  { key: 'browser', realSrc: 'apps/app-edge.png', node: <BrowserOrbIcon /> },
  { key: 'store', realSrc: 'apps/app-store.png', node: <StoreIcon /> },
  { key: 'music', realSrc: 'apps/app-spotify.png', node: <MusicDiscIcon /> },
]
