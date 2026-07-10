import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { ColorPickerPanel } from '@/components/common/color-picker'
import { useT } from '@/lib/i18n'
import { cn } from '@/lib/utils'

// THE single home for colour-choice controls (DRY law): every surface that lets
// the user pick a colour speaks this one dialect — an 18px dot family:
//   · ColorSwatchDot — a concrete colour
//   · AutoDot        — "automatic" (dashed hollow circle, industry convention)
//   · PalettePopover — the colour-wheel entry opening the full 调色盘
//   · SwatchStrip    — swatches + wheel, composed
// Fixed-size glyphs, tooltips for names: no locale can wrap or overflow them.

export const QUICK_SWATCHES = ['#FFFFFF', '#141414', '#FF6F5E', '#3FB6A8', '#D9A94E', '#E4574D']

export function ColorSwatchDot({
  color,
  selected,
  onClick,
  label,
}: {
  color: string
  selected?: boolean
  onClick: () => void
  label?: string
}) {
  return (
    <button
      type="button"
      title={label ?? color}
      aria-label={label ?? color}
      onClick={onClick}
      className={cn(
        'size-[18px] shrink-0 rounded-full ring-1 ring-black/10 transition-transform hover:scale-110 active:scale-95',
        selected && 'ring-2 ring-coral ring-offset-1 ring-offset-raised',
      )}
      style={{ background: color }}
    />
  )
}

export function AutoDot({
  selected,
  onClick,
  label,
}: {
  selected: boolean
  onClick: () => void
  label: string
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      className={cn(
        'size-[18px] shrink-0 rounded-full border-[1.5px] border-dashed border-t3 transition-transform hover:scale-110 active:scale-95',
        selected && 'border-solid border-coral ring-2 ring-coral ring-offset-1 ring-offset-raised',
      )}
    />
  )
}

/** AutoDot's face without the button — for slotting INSIDE a SwatchButton
 *  chip (the type accordion's 跟随全局 anchor); AutoDot itself stays the
 *  standalone-dot form. Same dashed-circle dialect, one source of look. */
export function AutoGlyph({ selected }: { selected: boolean }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'block size-[18px] shrink-0 rounded-full border-[1.5px] border-dashed border-t3',
        selected && 'border-solid border-coral',
      )}
    />
  )
}

/** The colour-wheel glyph itself — THE face every colour-picker entry wears
 *  (PalettePopover, 标识配色, …): conic ring, inner dot = current pick. */
export function WheelRing({ value, active, size = 22 }: { value: string; active: boolean; size?: number }) {
  return (
    <span
      aria-hidden="true"
      className="flex shrink-0 items-center justify-center rounded-full"
      style={{
        width: size,
        height: size,
        background: 'conic-gradient(#E5484D, #D9A94E, #3FA65C, #3FB6A8, #8A8F98, #E4574D, #E5484D)',
      }}
    >
      <span
        className="rounded-full ring-1 ring-black/15"
        style={{ width: size - 10, height: size - 10, background: active ? value : '#FFFFFF' }}
      />
    </span>
  )
}

/** The 调色盘 entry — a colour-wheel dot (fixed glyph, name in the tooltip). */
export function PalettePopover({
  value,
  active,
  wallpaper,
  onPick,
  align = 'start',
}: {
  value: string
  active: boolean
  wallpaper: string[]
  onPick: (hex: string) => void
  align?: 'start' | 'end'
}) {
  const t = useT()
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          title={t('Palette_Button')}
          aria-label={t('Palette_Button')}
          className={cn(
            'shrink-0 rounded-full transition-transform hover:scale-110 active:scale-95',
            active && 'ring-2 ring-coral ring-offset-1 ring-offset-raised',
          )}
        >
          <WheelRing value={value} active={active} />
        </button>
      </PopoverTrigger>
      <PopoverContent side="bottom" align={align} className="w-auto rounded-[14px] p-3">
        <ColorPickerPanel value={value} onChange={onPick} wallpaperSwatches={wallpaper} quickSwatches={QUICK_SWATCHES} />
      </PopoverContent>
    </Popover>
  )
}

/** Preset swatches + the wheel, in one wrapping flow. */
export function SwatchStrip({
  swatches,
  active,
  wallpaper,
  onPick,
}: {
  swatches: string[]
  active: string
  wallpaper: string[]
  onPick: (hex: string) => void
}) {
  const isPreset = swatches.some((s) => s.toUpperCase() === active.toUpperCase())
  return (
    <div className="flex flex-wrap items-center gap-1">
      {swatches.map((s) => (
        <ColorSwatchDot
          key={s}
          color={s}
          selected={active.toUpperCase() === s.toUpperCase()}
          onClick={() => onPick(s)}
        />
      ))}
      <PalettePopover value={active || QUICK_SWATCHES[2]} active={!!active && !isPreset} wallpaper={wallpaper} onPick={onPick} />
    </div>
  )
}
