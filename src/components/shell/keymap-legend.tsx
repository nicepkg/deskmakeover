import { Keyboard } from 'lucide-react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useT } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'

// Title-bar keymap affordance (spec 02/03): a quiet keyboard-icon button opening
// the shortcut legend — the non-hold discoverability path for the app's gesture
// set. Grouped BY PAGE (owner call): 通用 first, then each module's own gestures,
// so a reader instantly sees what works where. Popover + --elev-2.

const SECTIONS: { title: StringKey; rows: { action: StringKey; keys: StringKey }[] }[] = [
  {
    title: 'KeymapSec_General',
    rows: [
      { action: 'Keymap_Compare', keys: 'Keymap_CompareKey' },
      { action: 'Keymap_Modules', keys: 'Keymap_ModulesKey' },
      { action: 'Keymap_Zoom', keys: 'Keymap_ZoomKey' },
    ],
  },
  {
    title: 'KeymapSec_Icons',
    rows: [{ action: 'Keymap_Pan', keys: 'Keymap_PanIconsKey' }],
  },
  {
    title: 'KeymapSec_Paper',
    rows: [
      { action: 'Keymap_NewZone', keys: 'Keymap_NewZoneKey' },
      { action: 'Keymap_DeleteZone', keys: 'Keymap_DeleteZoneKey' },
      { action: 'Keymap_Deselect', keys: 'Keymap_DeselectKey' },
      { action: 'Keymap_Undo', keys: 'Keymap_UndoKey' },
      { action: 'Keymap_Redo', keys: 'Keymap_RedoKey' },
      { action: 'Keymap_Pan', keys: 'Keymap_PanPaperKey' },
    ],
  },
]

export function KeymapLegend() {
  const t = useT()
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={t('Keymap_Open')}
          title={t('Keymap_Open')}
          className="app-no-drag flex size-8 items-center justify-center rounded-lg text-t2 transition-colors duration-100 hover:bg-raised-hov hover:text-t1"
        >
          <Keyboard size={16} strokeWidth={1.75} />
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" sideOffset={6} className="w-[272px] gap-0 rounded-[14px] p-0">
        <p className="border-b border-hair px-4 py-2.5 text-body font-medium text-t1">
          {t('Keymap_Title')}
        </p>
        <div className="divide-y divide-hair pb-1">
          {SECTIONS.map((sec) => (
            <section key={sec.title} className="px-2 pb-1.5 pt-2">
              <p className="px-2 pb-0.5 text-[10px] font-medium tracking-[0.08em] text-t3">
                {t(sec.title)}
              </p>
              {sec.rows.map((row) => (
                <div key={`${row.action}-${row.keys}`} className="flex h-[30px] items-center justify-between gap-3 px-2">
                  <span className="whitespace-nowrap text-[12px] text-t1">{t(row.action)}</span>
                  <kbd className="shrink-0 whitespace-nowrap rounded-[6px] border border-hair bg-chip px-1.5 py-[3px] font-sans text-[10.5px] font-medium leading-none text-t2">
                    {t(row.keys)}
                  </kbd>
                </div>
              ))}
            </section>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  )
}
