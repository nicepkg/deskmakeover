import { Image, LayoutGrid, Settings } from 'lucide-react'
import { motion, useReducedMotion } from 'motion/react'
import { cn } from '@/lib/utils'
import { useApp } from '@/stores/app'
import type { AppModule } from '@/stores/app'
import { useT } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'

// 66px module rail (spec 03): glyph-only 40×40 tiles with the localized label
// below; 设置 pinned to the bottom. Selected = coral 16% wash + accent glyph.

const items: { id: AppModule; labelKey: StringKey; Icon: typeof LayoutGrid; pinBottom?: boolean }[] = [
  { id: 'icons', labelKey: 'Rail_Icons', Icon: LayoutGrid },
  { id: 'paper', labelKey: 'Rail_Paper', Icon: Image },
  { id: 'settings', labelKey: 'Rail_Settings', Icon: Settings, pinBottom: true },
]

export function ModuleRail() {
  const t = useT()
  const module = useApp((s) => s.module)
  const setModule = useApp((s) => s.setModule)
  const reduced = useReducedMotion()

  return (
    <nav className="flex w-[66px] shrink-0 flex-col items-center gap-2.5 pb-3.5 pt-1">
      {items.map(({ id, labelKey, Icon, pinBottom }) => {
        const selected = module === id
        return (
          <button
            key={id}
            type="button"
            aria-label={t(labelKey)}
            aria-current={selected ? 'page' : undefined}
            onClick={() => setModule(id)}
            className={cn('group flex flex-col items-center gap-[3px]', pinBottom && 'mt-auto')}
          >
            <span
              className={cn(
                'relative flex size-10 items-center justify-center rounded-[13px] transition-colors duration-150',
                selected ? 'text-coral-ink' : 'text-t3 group-hover:bg-raised-hov group-hover:text-t1',
              )}
            >
              {/* ONE shared wash that SLIDES between tiles (owner call
                  2026-07-09) — scales to any number of future tabs for free. */}
              {selected && (
                <motion.span
                  layoutId="railActiveWash"
                  className="absolute inset-0 rounded-[13px] bg-wash-rail"
                  transition={reduced ? { duration: 0 } : { type: 'spring', stiffness: 520, damping: 44 }}
                />
              )}
              <Icon size={19} strokeWidth={1.6} className="relative z-10" />
            </span>
            <span className={cn('text-[9px] leading-none', selected ? 'text-coral-ink' : 'text-t3')}>
              {t(labelKey)}
            </span>
          </button>
        )
      })}
    </nav>
  )
}
