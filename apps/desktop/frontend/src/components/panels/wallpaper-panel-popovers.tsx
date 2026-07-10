import * as React from 'react'
import { ChevronDown, LayoutTemplate } from 'lucide-react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { ZONE_PRESETS } from '@/lib/zone-presets'
import { useT } from '@/lib/i18n'
import type { ZoneMaterial, ZoneTitleStyle } from '@/bridge/types'
import type { StringKey } from '@/lib/i18n'
import { cn } from '@/lib/utils'

// The 壁纸 panel's pickers & swatch minis (split from wallpaper-panel.tsx,
// ≤500-line law): font popover, preset gallery popover, emoji pager, material
// and title-style swatches. Colour controls live in
// components/common/color-controls.tsx (the single colour-dialect home).

// Font picker — bundled handwritten face(s) first, then the system list (spec 04 §2.5).
export function FontPopover({
  open,
  setOpen,
  fonts,
  current,
  display,
  onLoad,
  onPick,
}: {
  open: boolean
  setOpen: (open: boolean) => void
  fonts: { display: string; family: string | null }[]
  current: string | null
  display: string
  onLoad: () => void
  onPick: (family: string | null) => void
}) {
  const t = useT()
  const bundled = fonts.filter((f) => f.family === null)
  const system = fonts.filter((f) => f.family !== null)
  const option = (f: { display: string; family: string | null }) => (
    <button
      key={f.family ?? '__bundled__'}
      type="button"
      onClick={() => onPick(f.family)}
      className={cn(
        'flex h-7 w-full items-center truncate rounded-[7px] px-2 text-left text-[12px] hover:bg-raised-hov',
        current === f.family ? 'text-coral-ink' : 'text-t1',
      )}
      style={f.family ? { fontFamily: f.family } : undefined}
    >
      {f.display === '__bundled__' ? t('TitleFont_Default') : f.display}
    </button>
  )
  return (
    <Popover open={open} onOpenChange={(o) => { setOpen(o); if (o) onLoad() }}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex h-7 w-full items-center justify-between rounded-[8px] border border-hair bg-chip px-2 text-[12px] text-t1"
        >
          <span className="truncate">{display}</span>
          <ChevronDown size={11} className="shrink-0 text-t3" />
        </button>
      </PopoverTrigger>
      <PopoverContent side="bottom" align="start" sideOffset={4} className="max-h-64 w-[var(--radix-popover-trigger-width)] min-w-0 gap-0 overflow-y-auto rounded-[10px] p-1 shadow-elev-2">
        {bundled.map(option)}
        {bundled.length > 0 && system.length > 0 && <div className="my-1 border-t border-hair" />}
        {system.map(option)}
      </PopoverContent>
    </Popover>
  )
}

/** Curated emoji rosters for zone labels: one OBJECTS page (categorize by
 *  thing) + one FACES page (categorize by mood — owner call 2026-07-09); the
 *  custom input row hosts the SYSTEM emoji panel (Win + .) for everything
 *  else — never a bundled third-party picker. */
const EMOJI_PAGES = [
  {
    key: 'Zone_EmojiTabObjects' as const,
    emojis: [
      '🚀', '💼', '📁', '🔥', '📥', '🗃️', '🎮', '🎵', '🖼️', '📚', '🧰', '⭐',
      '💻', '🎬', '🎨', '📷', '✈️', '🏠', '❤️', '🌙', '☕', '🛒', '🎯', '⚽',
    ],
  },
  {
    key: 'Zone_EmojiTabFaces' as const,
    emojis: [
      '😀', '😄', '😊', '😎', '🤓', '🥳', '🤩', '😍', '🥰', '😌', '😴', '🤔',
      '😅', '😂', '🥲', '😭', '😤', '🤯', '😱', '🙃', '😇', '🫡', '👻', '🤖',
    ],
  },
]

/** The 无 dialect's slash-circle, sized for the emoji grid (axis chips keep
 *  the 25px keyline; this is a different, denser context). */
function NoneMini({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 20 20" fill="none" aria-hidden>
      <circle cx="10" cy="10" r="7.25" stroke="currentColor" strokeWidth="1.5" />
      <line x1="4.9" y1="4.9" x2="15.1" y2="15.1" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  )
}

/** First grapheme of a string (emoji are multi-codepoint: ZWJ, skin tones). */
function firstGrapheme(value: string): string | null {
  const trimmed = value.trim()
  if (!trimmed) return null
  const seg = new Intl.Segmenter(undefined, { granularity: 'grapheme' })
  for (const s of seg.segment(trimmed)) return s.segment
  return null
}

export const MATERIALS: ZoneMaterial[] = ['Frost', 'Luminous', 'Solid', 'Halo', 'Outline']
export const MATERIAL_KEYS: Record<ZoneMaterial, StringKey> = {
  Frost: 'Material_Frost',
  Luminous: 'Material_Luminous',
  Solid: 'Material_Solid',
  Halo: 'Material_Halo',
  Outline: 'Material_Outline',
}
export const TITLE_STYLE_KEYS: Record<ZoneTitleStyle, StringKey> = {
  Chip: 'TitleStyle_Chip',
  Bare: 'TitleStyle_Bare',
  Tab: 'TitleStyle_Tab',
  Bar: 'TitleStyle_Bar',
}

/** Mini previews of the five finishes (DOM approximations of the recipes —
 *  the compositor stays the pixel truth; these only need to be evocative). */
export function MaterialSwatch({
  material,
  title,
  selected,
  onClick,
}: {
  material: ZoneMaterial
  title: string
  selected: boolean
  onClick: () => void
}) {
  const face: React.CSSProperties =
    material === 'Frost'
      ? { background: 'rgba(255,255,255,0.62)', boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.8), inset 0 0 0 1px rgba(0,0,0,0.10)' }
      : material === 'Luminous'
        ? { background: 'linear-gradient(180deg, rgba(255,255,255,0.9), rgba(255,255,255,0.55))', boxShadow: 'inset 0 1.5px 0 rgba(255,255,255,0.95), inset 0 0 0 1px rgba(0,0,0,0.08)' }
        : material === 'Solid'
          ? { background: 'rgba(252,251,249,0.97)', boxShadow: 'inset 0 1px 0 #FFFFFF, inset 0 0 0 1px rgba(0,0,0,0.10)' }
          : material === 'Halo'
            ? { background: 'radial-gradient(circle at 50% 45%, rgba(255,255,255,0.85) 20%, rgba(255,255,255,0.0) 75%)' }
            : { boxShadow: 'inset 0 0 0 1.5px rgba(90,80,70,0.75)' }
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      aria-pressed={selected}
      onClick={onClick}
      className={cn(
        'flex size-7 items-center justify-center rounded-[8px] ring-1 transition-shadow',
        selected ? 'ring-coral bg-wash-chip' : 'ring-hair hover:ring-t3/50',
      )}
    >
      <span className="block size-4 rounded-[5px]" style={face} />
    </button>
  )
}

/** Tiny glyphs sketching each title style inside a 16px panel outline. */
export function TitleStyleSwatch({
  style,
  title,
  selected,
  onClick,
}: {
  style: ZoneTitleStyle
  title: string
  selected: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      aria-pressed={selected}
      onClick={onClick}
      className={cn(
        'flex size-6 items-center justify-center rounded-[7px] transition-colors',
        selected ? 'bg-wash-chip text-coral-ink' : 'text-t3 hover:bg-raised-hov hover:text-t1',
      )}
    >
      <svg width={16} height={16} viewBox="0 0 16 16" aria-hidden>
        <rect x={1.5} y={4.5} width={13} height={10} rx={2.5} fill="none" stroke="currentColor" strokeOpacity={0.55} />
        {style === 'Chip' && <rect x={3} y={2.5} width={7} height={4} rx={2} fill="currentColor" />}
        {style === 'Bare' && (
          <>
            <circle cx={4.4} cy={4.5} r={1.5} fill="currentColor" />
            <rect x={6.8} y={3.6} width={6} height={1.8} rx={0.9} fill="currentColor" />
          </>
        )}
        {style === 'Tab' && <path d="M3 4.5 v-1.5 a1.5 1.5 0 0 1 1.5 -1.5 h4 a1.5 1.5 0 0 1 1.5 1.5 v1.5 z" fill="currentColor" />}
        {style === 'Bar' && (
          <>
            <path d="M1.5 7.5 h13" stroke="currentColor" strokeWidth={1.2} />
            <rect x={3} y={5.4} width={5.5} height={1.4} rx={0.7} fill="currentColor" />
          </>
        )}
      </svg>
    </button>
  )
}

/** Preset gallery popover: each layout drawn ON the user's wallpaper thumbnail
 *  (choice, not prediction — spec 04 §2.3). Accent-tinted panels echo the
 *  Adaptive material at micro scale. */
export function PresetPopover({ wallpaperUrl, onPick }: { wallpaperUrl: string | null; onPick: (id: string) => void }) {
  const t = useT()
  const [open, setOpen] = React.useState(false)
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          title={t('Preset_Gallery')}
          className="flex size-6 items-center justify-center rounded-[7px] text-t3 transition-colors hover:bg-raised-hov hover:text-t1"
        >
          <LayoutTemplate size={13} />
        </button>
      </PopoverTrigger>
      <PopoverContent side="bottom" align="end" className="w-[252px] rounded-[12px] p-2">
        <p className="mb-1.5 px-1 text-[11px] font-medium text-t1">{t('Preset_Gallery')}</p>
        <div className="grid grid-cols-2 gap-1.5">
          {ZONE_PRESETS.map((preset) => (
            <button
              key={preset.id}
              type="button"
              onClick={() => {
                setOpen(false)
                onPick(preset.id)
              }}
              className="group overflow-hidden rounded-[9px] ring-1 ring-hair transition-shadow hover:ring-coral/60"
            >
              <span className="relative block aspect-video w-full overflow-hidden bg-canvas-stage">
                {wallpaperUrl && (
                  <img src={wallpaperUrl} alt="" className="absolute inset-0 size-full object-cover" draggable={false} />
                )}
                {preset.zones.map((z, i) => (
                  <span
                    key={i}
                    className="absolute rounded-[3px]"
                    style={{
                      left: `${z.x * 100}%`,
                      top: `${z.y * 100}%`,
                      width: `${z.w * 100}%`,
                      height: `${z.h * 100}%`,
                      background: 'rgba(255,255,255,0.42)',
                      boxShadow: `inset 0 0 0 1px rgba(0,0,0,0.12), inset 0 1.5px 0 ${z.accent}`,
                    }}
                  />
                ))}
              </span>
              <span className="block truncate px-1.5 py-1 text-left text-caption text-t2 group-hover:text-t1">
                {t(preset.nameKey)}
              </span>
            </button>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  )
}

/** Emoji picker: 无 wears the slash-circle (the app-wide none dialect), two
 *  curated pages (objects / faces) are the fast path, and a free-input row
 *  receives the SYSTEM emoji panel (Win + .) or a paste — any grapheme wins. */
export function EmojiPicker({
  value,
  noneLabel,
  onPick,
}: {
  value: string | null
  noneLabel: string
  onPick: (emoji: string | null) => void
}) {
  const t = useT()
  const [open, setOpen] = React.useState(false)
  const [custom, setCustom] = React.useState('')
  const valuePage = value !== null ? EMOJI_PAGES.findIndex((p) => p.emojis.includes(value)) : 0
  const [page, setPage] = React.useState(Math.max(0, valuePage))

  const commitCustom = (raw: string) => {
    const grapheme = firstGrapheme(raw)
    if (!grapheme) return
    onPick(grapheme)
    setCustom('')
    setOpen(false)
  }

  const inRoster = value !== null && EMOJI_PAGES.some((p) => p.emojis.includes(value))

  return (
    <Popover
      open={open}
      onOpenChange={(o) => {
        setOpen(o)
        if (o) setPage(Math.max(0, valuePage)) // land on the page holding the current pick
      }}
    >
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex h-6 min-w-9 items-center justify-center rounded-[7px] bg-chip px-1.5 text-[13px] leading-none text-t3 transition-colors hover:bg-raised-hov"
          aria-label={value ?? noneLabel}
        >
          {value ?? <NoneMini />}
        </button>
      </PopoverTrigger>
      <PopoverContent side="bottom" align="start" className="w-[196px] rounded-[12px] p-1.5">
        {/* Page tabs + the always-reachable 无 (outside the pages). */}
        <div className="mb-1 flex items-center gap-1">
          {EMOJI_PAGES.map((p, i) => (
            <button
              key={p.key}
              type="button"
              onClick={() => setPage(i)}
              className={cn(
                'rounded-[6px] px-1.5 py-0.5 text-[11px] transition-colors',
                page === i ? 'bg-wash-chip text-coral-ink' : 'text-t3 hover:text-t1',
              )}
            >
              {t(p.key)}
            </button>
          ))}
          <button
            type="button"
            aria-label={noneLabel}
            title={noneLabel}
            onClick={() => {
              onPick(null)
              setOpen(false)
            }}
            className={cn(
              'ml-auto flex size-6 items-center justify-center rounded-[6px] text-t3 transition-colors hover:bg-raised-hov',
              value === null && 'bg-wash-chip text-coral-ink',
            )}
          >
            <NoneMini />
          </button>
        </div>
        <div className="grid grid-cols-6 gap-0.5">
          {EMOJI_PAGES[page].emojis.map((emoji) => (
            <button
              key={emoji}
              type="button"
              onClick={() => {
                onPick(emoji)
                setOpen(false)
              }}
              className={cn(
                'flex h-7 items-center justify-center rounded-[6px] text-[15px] transition-colors hover:bg-raised-hov',
                value === emoji && 'bg-wash-chip',
              )}
            >
              {emoji}
            </button>
          ))}
        </div>
        {/* Custom slot: the OS emoji panel is the "rich picker" — we host it
            instead of bundling a third-party one. */}
        <div className="mt-1.5 border-t border-hair pt-1.5">
          <div className="flex items-center gap-1.5">
            <input
              value={custom}
              onChange={(e) => {
                setCustom(e.currentTarget.value)
                commitCustom(e.currentTarget.value)
              }}
              onPaste={(e) => {
                e.preventDefault()
                commitCustom(e.clipboardData.getData('text'))
              }}
              placeholder={t('Zone_EmojiCustom')}
              className="h-6 min-w-0 flex-1 rounded-[6px] border border-hair bg-chip px-1.5 text-[12px] text-t1 outline-none placeholder:text-t3/70 focus:border-coral/60"
            />
            {value !== null && !inRoster && (
              <span className="flex h-6 w-7 shrink-0 items-center justify-center rounded-[6px] bg-wash-chip text-[13px]">{value}</span>
            )}
          </div>
          <p className="mt-1 px-0.5 text-[10px] leading-snug text-t3/80">{t('Zone_EmojiOsHint')}</p>
        </div>
      </PopoverContent>
    </Popover>
  )
}
