import * as React from 'react'
import { ChevronDown, LayoutTemplate } from 'lucide-react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { presetsForOrientation } from '@/lib/zone-presets'
import { useT } from '@/lib/i18n'
import type { ScreenOrientation, ZoneMaterial, ZoneTitleStyle } from '@/bridge/types'
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
        'flex h-7 w-full items-center gap-2 rounded-[7px] px-2 text-left text-[12px] hover:bg-raised-hov',
        current === f.family ? 'text-coral-ink' : 'text-t1',
      )}
    >
      {/* The NAME always renders in the UI font — never in the face itself, or
          symbol / decorative / CJK-less system fonts turn their own name into 乱码
          (queryLocalFonts returns the whole installed set). */}
      <span className="min-w-0 flex-1 truncate">
        {f.display === '__bundled__' ? t('TitleFont_Default') : f.display}
      </span>
      {/* A compact right-aligned specimen in the actual face — 'Ag' renders in
          virtually every font (Latin lives even in CJK faces), so it previews the
          shape without risking the name's legibility. */}
      {f.family && (
        <span aria-hidden className="shrink-0 text-[13px] leading-none text-t3" style={{ fontFamily: f.family }}>
          Ag
        </span>
      )}
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
export const EMOJI_PAGES = [
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

export const MATERIALS: ZoneMaterial[] = ['Frost', 'LiquidGlass', 'Fluted', 'Paper', 'Brushed', 'Outline']
export const MATERIAL_KEYS: Record<ZoneMaterial, StringKey> = {
  Frost: 'Material_Frost',
  LiquidGlass: 'Material_LiquidGlass',
  Fluted: 'Material_Fluted',
  Paper: 'Material_Paper',
  Brushed: 'Material_Brushed',
  Outline: 'Material_Outline',
}
export const TITLE_STYLE_KEYS: Record<ZoneTitleStyle, StringKey> = {
  None: 'TitleStyle_None',
  Etched: 'TitleStyle_Etched',
  Chip: 'TitleStyle_Chip',
  Bare: 'TitleStyle_Bare',
  Bar: 'TitleStyle_Bar',
}

/** Per-finish panel approximation drawn OVER the live wallpaper crop (the
 *  compositor stays the pixel truth; backdrop-filter gives Frost/Glaze a REAL
 *  blur of the wallpaper behind — 所见即所得, round 3 WYSIWYG pickers). */
function materialFace(material: ZoneMaterial): React.CSSProperties {
  switch (material) {
    case 'Frost':
      return {
        background: 'rgba(255,255,255,0.55)',
        backdropFilter: 'blur(3px)',
        boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.8), inset 0 0 0 1px rgba(0,0,0,0.08)',
      }
    case 'LiquidGlass':
      return {
        background:
          'linear-gradient(135deg, rgba(255,255,255,0.30) 0%, rgba(255,255,255,0.04) 45%, rgba(255,255,255,0.02) 62%, rgba(255,255,255,0.22) 100%)',
        backdropFilter: 'blur(0.5px)',
        boxShadow: 'inset 0 1px 1px rgba(255,255,255,0.9), inset 0 -1px 1px rgba(0,0,0,0.25), inset 0 0 0 1px rgba(255,255,255,0.35)',
      }
    case 'Fluted':
      return {
        // Vertical ribs sliced into the blur — repeating light bands.
        background:
          'repeating-linear-gradient(90deg, rgba(255,255,255,0.32) 0 1px, rgba(255,255,255,0.62) 2px 3px, rgba(0,0,0,0.05) 4.5px 5px, rgba(255,255,255,0.32) 6px)',
        backdropFilter: 'blur(3px)',
        boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.75), inset 0 0 0 1px rgba(0,0,0,0.07)',
      }
    case 'Paper':
      return {
        background: '#F4EFE4',
        boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.9), inset 0 -1px 0 rgba(0,0,0,0.16), inset 0 0 0 1px rgba(0,0,0,0.06)',
      }
    case 'Brushed':
      return {
        // Warm-graphite plate: fine horizontal streaks + one diagonal sheen.
        background:
          'linear-gradient(110deg, rgba(255,255,255,0) 25%, rgba(255,255,255,0.34) 45%, rgba(255,255,255,0) 65%), repeating-linear-gradient(0deg, #C9C4BC 0 1px, #BFBAB2 1px 2px)',
        boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.8), inset 0 -1px 0 rgba(0,0,0,0.18), inset 0 0 0 1px rgba(0,0,0,0.08)',
      }
    default: // Outline
      return { boxShadow: 'inset 0 0 0 1.5px rgba(70,60,50,0.85)' }
  }
}

/** WYSIWYG material tile: the finish rendered over a crop of the USER'S
 *  wallpaper (preset-popover on-wallpaper pattern), not a swatch in a vacuum. */
export function MaterialSwatch({
  material,
  title,
  selected,
  wallpaperUrl,
  onClick,
}: {
  material: ZoneMaterial
  title: string
  selected: boolean
  wallpaperUrl: string | null
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
        'relative size-10 shrink-0 overflow-hidden rounded-[9px] bg-canvas-stage transition-shadow',
        selected ? 'ring-2 ring-coral' : 'ring-1 ring-hair hover:ring-t3/50',
      )}
    >
      {wallpaperUrl && (
        <img src={wallpaperUrl} alt="" className="absolute inset-0 size-full object-cover" draggable={false} />
      )}
      <span className="absolute inset-[5px] rounded-[6px]" style={materialFace(material)} />
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
        'flex size-10 items-center justify-center rounded-[9px] transition-colors',
        selected ? 'bg-wash-chip text-coral-ink ring-1 ring-coral/50' : 'text-t3 ring-1 ring-hair hover:bg-raised-hov hover:text-t1',
      )}
    >
      <svg width={18} height={18} viewBox="0 0 16 16" aria-hidden>
        {style === 'None' ? (
          /* 无 wears the app-wide slash-circle dialect — hiding the title is a
             first-class answer to "how is this zone labeled" (round 3). */
          <>
            <circle cx={8} cy={8} r={5.4} stroke="currentColor" strokeWidth={1.3} fill="none" />
            <line x1={4.2} y1={4.2} x2={11.8} y2={11.8} stroke="currentColor" strokeWidth={1.3} strokeLinecap="round" />
          </>
        ) : (
          <>
            <rect x={1.5} y={4.5} width={13} height={10} rx={2.5} fill="none" stroke="currentColor" strokeOpacity={0.55} />
            {style === 'Etched' && (
              /* Frosted lozenge ETCHED into the panel: outline body + a light
                 top edge (no filled accent block — glass language). */
              <>
                <rect x={3} y={2.5} width={7} height={4} rx={2} fill="none" stroke="currentColor" strokeWidth={1.1} />
                <path d="M4.6 3.4 h3.8" stroke="currentColor" strokeWidth={0.9} strokeOpacity={0.6} strokeLinecap="round" />
              </>
            )}
            {style === 'Chip' && <rect x={3} y={2.5} width={7} height={4} rx={2} fill="currentColor" />}
            {style === 'Bare' && (
              <>
                <circle cx={4.4} cy={4.5} r={1.5} fill="currentColor" />
                <rect x={6.8} y={3.6} width={6} height={1.8} rx={0.9} fill="currentColor" />
              </>
            )}
            {style === 'Bar' && (
              <>
                {/* Editorial header: a header-weight title + a full-width hairline
                    seam (title-bar baseline). No colour band, no dot. */}
                <rect x={3.5} y={5.7} width={7} height={2} rx={1} fill="currentColor" />
                <path d="M1.5 9.5 h13" stroke="currentColor" strokeWidth={0.8} strokeOpacity={0.5} />
              </>
            )}
          </>
        )}
      </svg>
    </button>
  )
}

/** Preset gallery popover: each layout drawn ON the user's wallpaper thumbnail
 *  (choice, not prediction — spec 04 §2.3). Accent-tinted panels echo the
 *  Adaptive material at micro scale. */
export function PresetPopover({
  wallpaperUrl,
  orientation,
  onPick,
}: {
  wallpaperUrl: string | null
  /** Active screen shape — shows only its matching preset set (owner 2026-07-12). */
  orientation: ScreenOrientation
  onPick: (id: string) => void
}) {
  const t = useT()
  const [open, setOpen] = React.useState(false)
  const presets = presetsForOrientation(orientation)
  const isPortrait = orientation === 'portrait'
  // Portrait thumbs: true 9:16 but height-capped + centred so the popover stays
  // compact (owner 2026-07-12: 弹窗高度没适配好).
  const thumbClass = isPortrait ? 'aspect-[9/16] h-[128px] mx-auto' : 'aspect-video w-full'
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
      <PopoverContent side="bottom" align="end" className="max-h-[78vh] w-[252px] overflow-y-auto rounded-[12px] p-2">
        <p className="mb-1.5 px-1 text-[11px] font-medium text-t1">{t('Preset_Gallery')}</p>
        <div className="grid grid-cols-2 gap-1.5">
          {presets.map((preset) => (
            <button
              key={preset.id}
              type="button"
              onClick={() => {
                setOpen(false)
                onPick(preset.id)
              }}
              className="group overflow-hidden rounded-[9px]"
            >
              <span className={`relative block ${thumbClass} overflow-hidden bg-canvas-stage`}>
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
              <span className={`block truncate px-1.5 py-1 text-caption text-t2 group-hover:text-t1 ${isPortrait ? 'text-center' : 'text-left'}`}>
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
