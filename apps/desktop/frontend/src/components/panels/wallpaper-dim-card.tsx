import * as React from 'react'
import { AngleDial } from '@/components/common/angle-dial'
import { ChevronDown } from 'lucide-react'
import { DmSlider } from '@/components/common/dm-slider'
import { InspectorCard, PropertyRow, Reveal } from '@/components/common/inspector'
import { Segmented } from '@/components/common/segmented'
import { ColorSwatchDot, PalettePopover } from '@/components/common/color-controls'
import { SelectPopover } from '@/components/common/select-popover'
import { useWallpaper } from '@/stores/wallpaper'
import { useT } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'
import type { ClarityLevel } from '@/bridge/types'
import { cn } from '@/lib/utils'

// 壁纸压暗 card (split from wallpaper-panel.tsx, ≤500-line law): the hero
// capability (ADR-0013 D7) — gradient dim segmented + the 高级 fold (strength,
// direction dial-synced dropdown, dim colour). Self-contained: reads the store.

// Gradient direction presets — CLOCKWISE, the engine contract verbatim
// (0°=top · 90°=right · 180°=bottom · 270°=left), which is also the dial's
// clock face. Dropdown and dial are two views of ONE angle.
const GRADIENT_DIRS: { value: string; angle: number; key: StringKey }[] = [
  { value: 'Top', angle: 0, key: 'Gradient_Top' },
  { value: 'Right', angle: 90, key: 'Gradient_Right' },
  { value: 'Bottom', angle: 180, key: 'Gradient_Bottom' },
  { value: 'Left', angle: 270, key: 'Gradient_Left' },
]

export function WallpaperDimCard() {
  const t = useT()
  const state = useWallpaper((s) => s.state)
  const look = useWallpaper((s) => s.look)
  const { mutateLook } = useWallpaper.getState()
  const [advClarity, setAdvClarity] = React.useState(false)
  const [dirOpen, setDirOpen] = React.useState(false)

  if (!state || !look) return null
  const clarity = look.clarity

  return (
    <InspectorCard>
      <PropertyRow
        label={t('Paper_Clarity')}
        sub={
          <Reveal show={clarity.level !== 'Off'}>
            <div className="flex items-center justify-between pt-2">
              {state.pale && (
                <span className="rounded-full bg-amber-wash px-1.5 py-0.5 text-caption text-amber">
                  {t('Paper_PaleHint')}
                </span>
              )}
              <button
                type="button"
                onClick={() => setAdvClarity((v) => !v)}
                className={cn(
                  'ml-auto flex items-center gap-0.5 whitespace-nowrap rounded-md px-1.5 py-0.5 text-[11px] transition-colors',
                  advClarity ? 'bg-wash-chip text-coral-ink' : 'text-t3 hover:text-t1',
                )}
              >
                {t('Paper_Advanced')}
                <ChevronDown size={11} className={cn('transition-transform duration-150', advClarity && 'rotate-180')} />
              </button>
            </div>
          </Reveal>
        }
      >
        <Segmented
          size="sm"
          value={clarity.level}
          onChange={(level: ClarityLevel) => mutateLook((l) => ({ ...l, clarity: { ...l.clarity, level } }))}
          options={[
            { value: 'Off', label: t('Clarity_Off') },
            { value: 'Soft', label: t('Clarity_Soft') },
            { value: 'Strong', label: t('Clarity_Strong') },
          ]}
        />
      </PropertyRow>

      <Reveal show={clarity.level !== 'Off' && advClarity}>
        <div className="divide-y divide-hair">
          <PropertyRow label={`${t('Paper_Dim')} · ${Math.round((clarity.dimOverride ?? (clarity.level === 'Soft' ? 0.12 : 0.22)) * 100)}%`}>
            <DmSlider
              value={Math.round((clarity.dimOverride ?? (clarity.level === 'Soft' ? 0.12 : 0.22)) * 100)}
              min={0}
              max={100}
              onChange={(v) => mutateLook((l) => ({ ...l, clarity: { ...l.clarity, dimOverride: v / 100 } }), 'dim')}
              aria-label={t('Paper_Dim')}
            />
          </PropertyRow>
          <PropertyRow
            label={t('Paper_Gradient')}
            inline
            sub={
              <Reveal show={clarity.gradient === 'Linear'}>
                <div className="flex items-center gap-3 pt-2">
                  <AngleDial
                    value={Math.round(clarity.angleDeg)}
                    onChange={(deg) => mutateLook((l) => ({ ...l, clarity: { ...l.clarity, angleDeg: deg } }), 'angle')}
                  />
                  <span className="text-[12px] tabular-nums text-t1">{Math.round(clarity.angleDeg)}°</span>
                </div>
              </Reveal>
            }
          >
            <div className="w-[55px]">
              <SelectPopover
                compact
                open={dirOpen}
                setOpen={setDirOpen}
                value={
                  clarity.gradient === 'Vignette'
                    ? 'Vignette'
                    : GRADIENT_DIRS.find((d) => d.angle === ((Math.round(clarity.angleDeg) % 360) + 360) % 360)?.value ?? 'Custom'
                }
                options={[
                  ...GRADIENT_DIRS.map((d) => ({ value: d.value, label: t(d.key) })),
                  { value: 'Vignette', label: t('Gradient_Vignette') },
                  { value: 'Custom', label: t('Gradient_Custom') },
                ]}
                onPick={(dir) => {
                  const preset = GRADIENT_DIRS.find((d) => d.value === dir)
                  mutateLook((l) => ({
                    ...l,
                    clarity: {
                      ...l.clarity,
                      gradient: dir === 'Vignette' ? 'Vignette' : 'Linear',
                      angleDeg: preset ? preset.angle : l.clarity.angleDeg,
                    },
                  }))
                }}
              />
            </div>
          </PropertyRow>
          <PropertyRow label={t('Paper_ScrimLabel')}>
            <div className="flex items-center gap-1.5">
              <ColorSwatchDot
                color="#14171C"
                label={t('Scrim_Dark')}
                selected={clarity.tone === 'Dark'}
                onClick={() => mutateLook((l) => ({ ...l, clarity: { ...l.clarity, tone: 'Dark' } }))}
              />
              <ColorSwatchDot
                color="#F6F5F2"
                label={t('Scrim_Light')}
                selected={clarity.tone === 'Light'}
                onClick={() => mutateLook((l) => ({ ...l, clarity: { ...l.clarity, tone: 'Light' } }))}
              />
              <ColorSwatchDot
                color={state.wallTint}
                label={t('Scrim_Tint')}
                selected={clarity.tone === 'Tint'}
                onClick={() => mutateLook((l) => ({ ...l, clarity: { ...l.clarity, tone: 'Tint' } }))}
              />
              <PalettePopover
                value={clarity.customScrim ?? '#101418'}
                active={clarity.tone === 'Custom'}
                wallpaper={[state.wallTint]}
                onPick={(hex) =>
                  mutateLook((l) => ({ ...l, clarity: { ...l.clarity, tone: 'Custom', customScrim: hex } }))
                }
              />
            </div>
          </PropertyRow>
        </div>
      </Reveal>
    </InspectorCard>
  )
}
