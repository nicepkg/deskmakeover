import type { ReactNode } from 'react'
import { RotateCcw } from 'lucide-react'
import { useT } from '@/lib/i18n'

// Shared version-history strip (chief-UI/UX call 2026-07-09): NOT a modal — it
// expands in place inside the 280px panel so the canvas the user compares
// against stays visible. Each row leads with a live THUMBNAIL of that version
// (versions that differ only by tint/filter are indistinguishable as text).
// Generic over the entry type so the wallpaper module reuses it by injecting
// its own `renderThumb`. Newest first; `onGoTo` runs the module's apply
// ceremony (the ceremony IS the confirmation — no extra dialog).

export interface HistoryStripItem {
  key: string | number
  /** Short timestamp, e.g. "14:32". */
  time: string
  /** Human label, e.g. "自定 · 苹果 · 单色". */
  label: string
  isCurrent: boolean
}

export function HistoryStrip<T extends HistoryStripItem>({
  items,
  renderThumb,
  onGoTo,
  onBackToInitial,
  disabled = false,
}: {
  /** Newest first. */
  items: T[]
  renderThumb: (item: T) => ReactNode
  onGoTo: (item: T) => void
  onBackToInitial?: () => void
  /** True while a host crossing is in flight — the version-jump buttons run the
   *  module's apply/restore ceremony, so they must be inert until it lands. */
  disabled?: boolean
}) {
  const t = useT()
  return (
    <div className="rounded-xl border border-hair bg-raised p-2.5">
      <p className="mb-2 px-0.5 text-caption text-t3">{t('History_Header')}</p>
      <div className="space-y-1">
        {items.map((item) => (
          <div key={item.key} className="flex items-center gap-2">
            <span className="grid size-7 shrink-0 place-items-center overflow-hidden rounded-[8px] bg-chip ring-1 ring-hair">
              {renderThumb(item)}
            </span>
            <span className="min-w-0 flex-1 leading-tight">
              <span className="block truncate text-[12px] text-t1">{item.label}</span>
              <span className="block text-caption tabular-nums text-t3">{item.time}</span>
            </span>
            {item.isCurrent ? (
              <span className="shrink-0 rounded-full bg-teal-wash px-1.5 py-0.5 text-caption text-teal">
                {t('History_Current')}
              </span>
            ) : (
              <button
                type="button"
                disabled={disabled}
                className="shrink-0 text-caption text-coral-ink transition-colors hover:underline disabled:cursor-not-allowed disabled:text-t3 disabled:no-underline"
                onClick={() => onGoTo(item)}
              >
                {t('History_GoTo')}
              </button>
            )}
          </div>
        ))}
        {onBackToInitial && (
          <div className="flex items-center gap-2 border-t border-hair pt-2">
            <span className="grid size-7 shrink-0 place-items-center rounded-[8px] bg-chip/60 text-t3">
              <RotateCcw size={13} />
            </span>
            <span className="min-w-0 flex-1 leading-tight">
              <span className="block truncate text-[12px] text-t2">{t('History_Initial')}</span>
              <span className="block text-caption text-t3">{t('History_InitialDesc')}</span>
            </span>
            <button
              type="button"
              disabled={disabled}
              className="shrink-0 text-caption text-coral-ink transition-colors hover:underline disabled:cursor-not-allowed disabled:text-t3 disabled:no-underline"
              onClick={onBackToInitial}
            >
              {t('History_BackToInitial')}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
