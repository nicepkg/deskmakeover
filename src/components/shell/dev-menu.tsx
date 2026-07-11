import * as React from 'react'
import { Bomb, FlaskConical, RotateCcw } from 'lucide-react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'

// DEV-ONLY debug menu (owner ask): title-bar flask, left of the keymap icon.
// Resets the app's persisted first-run/ritual states so any onboarding flow can
// be replayed without hand-editing localStorage. Vite strips the whole component
// from production builds (import.meta.env.DEV guard) — so labels are hardcoded
// ENGLISH on purpose (owner call 2026-07-09: open-source collaborators): the
// engineering lingua franca, and no i18n keys for the resx reconciliation to see.

const RESETS: { label: string; keys: string[] }[] = [
  { label: 'Welcome gate (first run)', keys: ['dm.welcome.done'] },
  // Clear BOTH consent bits: legacy (`dm.consent.icons`) and the v2 machine-wide
  // arrow disclosure (`dm.consent.icons.v2`). Missing v2 here left the consent
  // sheet un-replayable once v2 was set (review new-P3).
  { label: 'Apply consent sheet', keys: ['dm.consent.icons', 'dm.consent.icons.v2'] },
  { label: 'Changelog seen flag', keys: ['dm.changelog.seen'] },
]
// NOTE: the reveal wand + per-tile bloom now replay on EVERY launch, and the
// apply celebration is in-memory (first apply per launch) — both auto-reset on
// reload, so neither needs a localStorage reset here.

// The 60s arrow-gate penance torments USERS by design — not developers.
// DEV-only override, read by ceremony.tsx exclusively under import.meta.env.DEV;
// production is hard-wired to 60 no matter what this key says.
export const ARROW_GATE_DEV_KEY = 'dm.dev.arrowGateSeconds'
const GATE_CHOICES = [3, 10, 60] as const

// User-simulation scenarios (mock data source; dev + video demos): switching
// swaps the mock desktop's item set + wallpaper, then reloads for a clean
// scan. Browser-mock only — the Windows host always mirrors the real desktop.
const SCENARIOS: { value: 'messy' | 'office' | 'gamer'; label: string }[] = [
  { value: 'messy', label: 'Messy' },
  { value: 'office', label: 'Office' },
  { value: 'gamer', label: 'Gamer' },
]

export function DevMenu() {
  const [, bump] = React.useReducer((n: number) => n + 1, 0)
  if (!import.meta.env.DEV) return null

  const clearAll = () => {
    for (const key of Object.keys(localStorage)) {
      if (key.startsWith('dm.')) localStorage.removeItem(key)
    }
    location.reload()
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Developer options"
          title="Developer options (dev builds only)"
          className="app-no-drag flex size-8 items-center justify-center rounded-lg text-t3 transition-colors duration-100 hover:bg-raised-hov hover:text-t1"
        >
          <FlaskConical size={15} strokeWidth={1.75} />
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" sideOffset={6} className="w-[248px] gap-0 rounded-[14px] p-0">
        <p className="border-b border-hair px-4 py-2.5 text-body font-medium text-t1">Developer options</p>
        <div className="p-2">
          {RESETS.map((r) => {
            const set = r.keys.some((k) => localStorage.getItem(k) !== null)
            return (
              <div key={r.label} className="flex h-[30px] items-center justify-between gap-2 px-2">
                <span className="flex min-w-0 items-center gap-1.5 text-[12px] text-t1">
                  <span
                    aria-hidden
                    className={set ? 'size-1.5 shrink-0 rounded-full bg-teal' : 'size-1.5 shrink-0 rounded-full bg-t3/30'}
                  />
                  <span className="truncate">{r.label}</span>
                </span>
                <button
                  type="button"
                  disabled={!set}
                  onClick={() => {
                    r.keys.forEach((k) => localStorage.removeItem(k))
                    bump()
                  }}
                  className="shrink-0 rounded-[7px] bg-chip px-2 py-1 text-[11px] text-t2 transition-colors enabled:hover:bg-raised-hov enabled:hover:text-t1 disabled:opacity-40"
                >
                  Reset
                </button>
              </div>
            )
          })}
        </div>
        {/* Three-option rows STACK: label on its own line above a full-width
            segmented button row (the inline layout overflowed the 248px popover
            — owner call 2026-07-09). Buttons share the width equally. */}
        <div className="space-y-2.5 border-t border-hair p-2">
          <div className="px-2">
            <p className="mb-1.5 text-[12px] text-t1">User scenario</p>
            <div className="flex gap-1">
              {SCENARIOS.map((s) => {
                const current = localStorage.getItem('dm.dev.scenario') ?? 'messy'
                const selected = current === s.value
                return (
                  <button
                    key={s.value}
                    type="button"
                    onClick={() => {
                      if (s.value === 'messy') localStorage.removeItem('dm.dev.scenario')
                      else localStorage.setItem('dm.dev.scenario', s.value)
                      location.reload()
                    }}
                    className={
                      selected
                        ? 'flex-1 rounded-[7px] bg-coral py-1 text-[11px] font-medium text-cta-ink'
                        : 'flex-1 rounded-[7px] bg-chip py-1 text-[11px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1'
                    }
                  >
                    {s.label}
                  </button>
                )
              })}
            </div>
          </div>
          <div className="px-2">
            <p className="mb-1.5 text-[12px] text-t1">Arrow gate wait</p>
            <div className="flex gap-1">
              {GATE_CHOICES.map((s) => {
                const current = Number(localStorage.getItem(ARROW_GATE_DEV_KEY) ?? 60)
                const selected = current === s
                return (
                  <button
                    key={s}
                    type="button"
                    onClick={() => {
                      if (s === 60) localStorage.removeItem(ARROW_GATE_DEV_KEY)
                      else localStorage.setItem(ARROW_GATE_DEV_KEY, String(s))
                      bump()
                    }}
                    className={
                      selected
                        ? 'flex-1 rounded-[7px] bg-coral py-1 text-[11px] font-medium text-cta-ink'
                        : 'flex-1 rounded-[7px] bg-chip py-1 text-[11px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1'
                    }
                  >
                    {s}s
                  </button>
                )
              })}
            </div>
          </div>
        </div>
        <div className="border-t border-hair p-2">
          <button
            type="button"
            onClick={() => window.dispatchEvent(new Event('dm-test-crash'))}
            className="mb-1 flex w-full items-center justify-center gap-1.5 rounded-[9px] bg-chip py-1.5 text-[12px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1"
          >
            <Bomb size={12} />
            Trigger test crash
          </button>
          <button
            type="button"
            onClick={clearAll}
            className="flex w-full items-center justify-center gap-1.5 rounded-[9px] bg-chip py-1.5 text-[12px] text-t2 transition-colors hover:bg-destructive hover:text-white"
          >
            <RotateCcw size={12} />
            Reset all & reload
          </button>
        </div>
      </PopoverContent>
    </Popover>
  )
}
