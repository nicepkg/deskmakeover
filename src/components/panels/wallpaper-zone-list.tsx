import { X } from 'lucide-react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import type { ZoneDto } from '@/bridge/types'
import { resolveAccent } from '@/compositor/material'
import { useT } from '@/lib/i18n'
import { cn } from '@/lib/utils'

// Zone rows in layer-list grammar (Figma/Finder): quiet transparent rows that
// wash on hover, wash+ink when selected. The leading swatch wears the zone's
// ACCENT (the categorization signal, spec 04 v2.0); emoji rides the title text.
// Titles are visible-editable; ✕ shows on hover only. Grid units never shown.

// Every row settles to this height (enter/exit animate from 0). It is the SINGLE
// source for both the row height and the active wash's per-index offset, so the
// wash always lands dead-center on its row.
const ROW_H = 32

function AccentSwatch({ accent, outline }: { accent: string; outline: boolean }) {
  return (
    <span
      aria-hidden="true"
      className="size-4 shrink-0 rounded-[5px]"
      style={
        outline
          ? { boxShadow: `inset 0 0 0 1.5px ${accent}` }
          : { background: `${accent}2E`, boxShadow: `inset 0 0 0 1px ${accent}66` }
      }
    >
      <span className="ml-[3px] mt-[3px] block size-1.5 rounded-full" style={{ background: accent }} />
    </span>
  )
}

export function ZoneList({
  zones,
  selected,
  onSelect,
  onRename,
  onDelete,
}: {
  zones: ZoneDto[]
  selected: string | null
  onSelect: (id: string) => void
  onRename: (id: string, title: string) => void
  onDelete: (id: string) => void
}) {
  const t = useT()
  const reduced = useReducedMotion()
  if (zones.length === 0) return null
  const activeIndex = selected !== null ? zones.findIndex((z) => z.id === selected) : -1
  return (
    <div className="relative mt-1.5 -mx-1.5">
      {/* Active wash — ONE persistent, container-level layer that SLIDES between
          rows by translateY (y = index × ROW_H), NOT a per-row layoutId
          projection. This is the same lesson the segmented thumb learned (see
          components/common/segmented.tsx §thumb): a layoutId wash mounted inside
          each row was CLIPPED by that row's own `overflow-hidden` — motion
          projected it up from the target row toward the old row, but every frame
          above the row's top edge was cut off, so instead of a background that
          glides up/down between zones you saw the highlight vanish and re-grow in
          place (frame-verified 2026-07-10: washTop swept 194→290 while clipTop
          stayed 290 → the whole travel was clipped). A single element translated
          in the non-clipping container rides selection AND row add/remove
          instantly and can never be clipped.
          ⛔ Do NOT move this wash back inside the rows / do NOT give it a layoutId. */}
      {activeIndex >= 0 && (
        <motion.div
          aria-hidden
          className="pointer-events-none absolute inset-x-0 top-0 rounded-[8px] bg-wash-chip"
          style={{ height: ROW_H }}
          initial={false}
          animate={{ y: activeIndex * ROW_H }}
          transition={reduced ? { duration: 0 } : { type: 'spring', stiffness: 520, damping: 44 }}
        />
      )}
      <AnimatePresence initial={false}>
      {zones.map((z, i) => (
        <motion.div
          key={z.id}
          layout={!reduced}
          initial={reduced ? false : { height: 0, opacity: 0 }}
          animate={{ height: ROW_H, opacity: 1 }}
          exit={reduced ? { opacity: 0, transition: { duration: 0.14 } } : { height: 0, opacity: 0, transition: { duration: 0.14 } }}
          transition={{ duration: 0.18, ease: [0.33, 1, 0.68, 1] }}
          role="button"
          tabIndex={0}
          onClick={() => onSelect(z.id)}
          onKeyDown={(e) => e.key === 'Enter' && onSelect(z.id)}
          className={cn(
            // `relative` keeps the row (and its content) painting ABOVE the
            // container wash; `overflow-hidden` clips the height enter/exit only.
            'group relative flex items-center gap-2 overflow-hidden rounded-[8px] px-1.5 transition-colors',
            selected !== z.id && 'hover:bg-raised-hov',
          )}
        >
          <AccentSwatch accent={resolveAccent(z, i)} outline={z.material === 'Outline'} />
          {z.emoji && <span className="shrink-0 text-[13px] leading-none">{z.emoji}</span>}
          <input
            value={z.title}
            onChange={(e) => onRename(z.id, e.currentTarget.value)}
            onFocus={() => onSelect(z.id)}
            onClick={(e) => e.stopPropagation()}
            className={cn(
              'min-w-0 flex-1 border-b border-transparent bg-transparent text-[12px] outline-none transition-colors',
              'hover:border-t3/40 focus:border-coral/70',
              selected === z.id ? 'text-coral-ink' : 'text-t1',
            )}
            aria-label={t('Zone_DefaultTitle')}
          />
          <button
            type="button"
            aria-label="delete zone"
            onClick={(e) => {
              e.stopPropagation()
              onDelete(z.id)
            }}
            className="hidden shrink-0 rounded p-0.5 text-t3 transition-colors hover:text-t1 group-hover:block"
          >
            <X size={12} />
          </button>
        </motion.div>
      ))}
      </AnimatePresence>
    </div>
  )
}
