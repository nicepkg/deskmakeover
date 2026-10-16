import type { Surface } from '@/bridge/types'
import { useT } from '@/lib/i18n'
import { cn } from '@/lib/utils'

// The canvas surface switcher (ADR-0024, fork draft): OUR chrome in a small
// glass pill above the toolbar — 桌面 | 任务栏 | 开始菜单 (Escritorio | Barra de
// tareas | Menú Inicio). Picking a non-desktop chip swaps the canvas to that
// surface's mock view (the strip pins enlarged / the Start panel); the scenery
// itself stays inert — only tiles gain behavior (spec 06 §4 amended).

const GLASS = 'bg-glass text-glass-ink ring-1 ring-glass-ring backdrop-blur-md'

export function SurfaceChips({ view, onChange }: { view: Surface; onChange: (view: Surface) => void }) {
  const t = useT()
  const chips: { key: Surface; label: string }[] = [
    { key: 'desktop', label: t('Surface_Desktop') },
    { key: 'taskbar', label: t('Surface_Taskbar') },
    { key: 'startMenu', label: t('Surface_StartMenu') },
  ]
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-11 z-10 flex items-center justify-center">
      <div className={cn('pointer-events-auto flex items-center gap-0.5 rounded-full p-0.5 text-[11px]', GLASS)}>
        {chips.map((c) => (
          <button
            key={c.key}
            type="button"
            aria-pressed={view === c.key}
            onClick={() => onChange(c.key)}
            className={cn(
              'rounded-full px-2.5 py-0.5 leading-5 transition-colors',
              view === c.key ? 'bg-coral/90 text-cta-ink' : 'text-glass-ink/75 hover:bg-glass-ink/10 hover:text-glass-ink',
            )}
          >
            {c.label}
          </button>
        ))}
      </div>
    </div>
  )
}
