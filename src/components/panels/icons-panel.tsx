import * as React from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { IconAction, InspectorCard, PropertyRow, Reveal, SwatchButton, SwatchPicker, swatchButtonClass, useFooterClearance } from '@/components/common/inspector'
import type { SwatchOption } from '@/components/common/inspector'
import { ChevronDown, History, RotateCcw } from 'lucide-react'
import { AutoDot, ColorSwatchDot, QUICK_SWATCHES, WheelRing } from '@/components/common/color-controls'
import { BwGlyph, FaithfulGlyph, FieldGlyph, FilterSwatch, MarkGlyph, NoneGlyph, PairDot, QuadPlateGlyph, ShapeSwatch, WinArrowGlyph } from '@/components/common/chip-preview'
import { ColorPickerPanel } from '@/components/common/color-picker'
import { HistoryStrip } from '@/components/common/history-strip'
import { Confetti, useCelebration } from '@/components/common/confetti'
import { KeptBar, KindTypeSection } from '@/components/panels/icons-participation'
import { CURATED_SHAPES, MORE_SHAPES, TYPE_PLATE_SWATCHES, paleOf } from '@/components/panels/icon-axis-options'
import { CtaButton } from '@/components/common/cta-button'
import { ArrowGateSheet, ConfirmSheet, ConsentSheet, DoneCard } from '@/components/common/ceremony'
import { Segmented } from '@/components/common/segmented'
import { activePresetIdOf, fieldRenderOpts, resumeStatusKey, useIcons } from '@/stores/icons'
import { getIconCompositor } from '@/icon-compositor/icon-renderer'
import { useIconsHero } from '@/lib/hero'
import { format, useT } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'
import type { ConfigDto, FilterStyle, IconShape, MarkStyle, TypeOverrides } from '@/bridge/types'
import { consentSatisfied, showDoneArrowNote } from '@/lib/arrow-overlay'
import { cn } from '@/lib/utils'

// The icons INSPECTOR (spec 02 v3): height is scarce, and axes GROW over time —
// so every axis follows one future-proof grammar:
//   · open sets (shapes, filters, marks — more coming) = a wrapping flow of visual
//     swatches / compact chips, with a 「更多」 fold once the set outgrows one row.
//     Wrapping never truncates; new options just extend the flow.
//   · closed sets (≤3 fixed options: colour mode) = a full-width segmented
//     with the label on its own line — label and control never fight for width.
// Presets are a single-column list (names always fully visible; future presets
// extend downward). CTA docks at the bottom, always visible.

// Owner law: every axis's 「无」 sits FIRST, wearing the shared slash-circle glyph.
const FILTERS: { value: FilterStyle; key: StringKey }[] = [
  { value: 'None', key: 'Filter_None' }, { value: 'Gloss', key: 'Filter_Gloss' },
  { value: 'Glass', key: 'Filter_Glass' },
  { value: 'Pixel', key: 'Filter_Pixel' }, { value: 'Sticker', key: 'Filter_Sticker' },
]
const MARKS: { value: MarkStyle; key: StringKey }[] = [
  { value: 'Shadow', key: 'Mark_Shadow' }, { value: 'Halo', key: 'Mark_Halo' }, { value: 'Satin', key: 'Mark_Satin' },
  { value: 'Arc', key: 'Mark_Arc' }, { value: 'Fold', key: 'Mark_Fold' }, { value: 'Ring', key: 'Mark_Ring' },
]
export const PRESET_NAME: Record<string, { name: StringKey; desc: StringKey }> = {
  spectrum: { name: 'Preset_spectrum', desc: 'Preset_spectrum_Desc' },
  stationery: { name: 'Preset_stationery', desc: 'Preset_stationery_Desc' },
  glass: { name: 'Preset_glass', desc: 'Preset_glass_Desc' },
  pebble: { name: 'Preset_pebble', desc: 'Preset_pebble_Desc' },
  ink: { name: 'Preset_ink', desc: 'Preset_ink_Desc' },
  white: { name: 'Preset_white', desc: 'Preset_white_Desc' },
  ascast: { name: 'Preset_ascast', desc: 'Preset_ascast_Desc' },
}

export function IconsPanel() {
  const t = useT()
  const { state, phase, statusText, ctaText } = useIconsHero()
  const bareLook = useIcons((s) => s.bareLook)
  const { mutate, selectPreset, selectSystemDefault, apply, restore, stageVersion, hover, hoverBare } = useIcons.getState()
  const [moreShapes, setMoreShapes] = React.useState(false)
  const [morePresets, setMorePresets] = React.useState(false)
  // Fold-animation hover freeze (owner diagnosis 2026-07-10: hovering a card
  // MID-EXPANSION fires the full-desktop try-on render inside the animation
  // frames — that was the jank). While the fold animates, preset hovers are
  // ignored; clicks still work.
  const foldAnimating = React.useRef(false)
  const foldAnimTimer = React.useRef<ReturnType<typeof setTimeout> | null>(null)
  const armFoldFreeze = () => {
    foldAnimating.current = true
    if (foldAnimTimer.current) clearTimeout(foldAnimTimer.current)
    foldAnimTimer.current = setTimeout(() => {
      foldAnimating.current = false
      foldAnimTimer.current = null
    }, 450)
  }
  const [moreShortcutShapes, setMoreShortcutShapes] = React.useState(false)
  const [historyOpen, setHistoryOpen] = React.useState(false)
  const [consentOpen, setConsentOpen] = React.useState(false)
  const [arrowGateOpen, setArrowGateOpen] = React.useState(false)
  const [restoreOpen, setRestoreOpen] = React.useState(false)
  const [doneOpen, setDoneOpen] = React.useState(false)
  // Milestone celebration (owner decree 2026-07-10): a full-window confetti burst
  // from the two bottom corners (shared useCelebration — same as wallpaper apply,
  // first success of each launch) PLUS a coral ripple from the CTA.
  const ctaWrapRef = React.useRef<HTMLDivElement>(null)
  const { celebrateKey, celebrate } = useCelebration('icons')
  const [ripple, setRipple] = React.useState<{ key: number; cx: number; cy: number } | null>(null)
  const { footerRef, clearance } = useFooterClearance()

  // The apply ceremony (D9): first apply shows the consent sheet once (persisted),
  // every successful apply lands the completion card — the real change happens
  // BEHIND this window, the doorway says so.
  const runApply = async () => {
    const ok = await apply()
    // Gate the completion card on THIS attempt's result — not the persisted
    // `applied` flag, which stays true from an earlier apply and would pop a
    // false DoneCard (and its "arrow is now hidden" line) after a failed or
    // overlay-declined re-apply (review P2-1). The DoneCard's arrow line reads
    // the live overlay state, so it never claims a hide that did not happen.
    if (!ok) return
    // First successful apply of this launch → celebrate (confetti + CTA ripple).
    const fired = celebrate()
    if (fired && !reduced) {
      const r = ctaWrapRef.current?.getBoundingClientRect()
      if (r) setRipple({ key: Date.now(), cx: r.left + r.width / 2, cy: r.top + r.height / 2 })
    }
    const beat = reduced ? 0 : 220
    window.setTimeout(() => setDoneOpen(true), beat)
  }
  // First-run consent gate (owner disposition #3): v2 consent carries the
  // machine-wide arrow disclosure and is required to skip. A legacy (pre-
  // disclosure) consent grandfathers single-user machines only — a machine with
  // more than one active profile must (re)see the non-skippable disclosure
  // (review P2-3), even if the user consented before this build.
  const consentOk = () =>
    consentSatisfied({
      v2: localStorage.getItem('dm.consent.icons.v2') === '1',
      legacy: localStorage.getItem('dm.consent.icons') === '1',
      profiles: useIcons.getState().state?.activeUserProfiles ?? 1,
    })
  const onCta = () => {
    // System Default's crossing is a RESTORE, never an apply (A1). If the desktop
    // is still styled, offer the existing restore confirm; if it is already bare,
    // there is nothing to cross.
    if (bareLook) {
      if (state?.applied) setRestoreOpen(true)
      return
    }
    if (!(phase === 'ready' || phase === 'dirty')) return
    if (!consentOk()) setConsentOpen(true)
    else void runApply()
  }
  // 回到此版 = stage that version's config, then the SAME ceremonied crossing
  // as apply — no silent desktop writes (spec 06 §3.7).
  const goVersion = (index: number) => {
    stageVersion(index)
    if (!consentOk()) setConsentOpen(true)
    else void runApply()
  }
  const reduced = useReducedMotion()

  // Hover try-on INTENT debounce (owner call 2026-07-09): a fast sweep across
  // options must not render each one — the pointer has to REST ~90ms before a
  // candidate paints. Leaving reverts instantly; clicks never wait. Hooks live
  // ABOVE the loading early-return (hook-order law).
  const hoverTimer = React.useRef<ReturnType<typeof setTimeout> | null>(null)
  const tryOn = React.useCallback(
    (change: Partial<ConfigDto>, typeOverrides?: TypeOverrides) => (hovering: boolean) => {
      if (hoverTimer.current) clearTimeout(hoverTimer.current)
      if (hovering) {
        hoverTimer.current = setTimeout(() => {
          hoverTimer.current = null
          hover(change, typeOverrides)
        }, 90)
      } else {
        hoverTimer.current = null
        hover(null)
      }
    },
    [hover],
  )
  // The System Default card's try-on (owner 2026-07-12: hovering it must preview
  // like every other style card). Shares hoverTimer with tryOn — one pending
  // hover at a time, same 90ms feel; the preview channel is hoveringBare.
  const bareTryOn = React.useCallback(
    (hovering: boolean) => {
      if (hoverTimer.current) clearTimeout(hoverTimer.current)
      if (hovering) {
        hoverTimer.current = setTimeout(() => {
          hoverTimer.current = null
          hoverBare(true)
        }, 90)
      } else {
        hoverTimer.current = null
        hoverBare(false)
      }
    },
    [hoverBare],
  )

  // Continuous colour drags (picker area / hue strip) fire per pointer move;
  // 124 tile re-renders cannot keep that pace (owner call 2026-07-09: 务必加入
  // debounce). Leading+trailing throttle: instant first paint, ~7 fps while
  // dragging, and the release value always lands.
  const throttledMarkColor = useThrottledChange((markColor: string | null) => mutate({ markColor }))

  if (!state) return <aside className="w-[280px] shrink-0" />

  const c = state.config
  // A3 resume + A1 bare: an honest status line (a resumed draft reads "resumed",
  // an un-applied one never reads "applied") and a CTA that restores rather than
  // applies while the System-Default look is active. Scanning/working defer to
  // the shared hero copy.
  const statusLine =
    phase === 'scanning' || phase === 'working'
      ? statusText
      : format(t(resumeStatusKey(state.applied, state.dirty, bareLook)), state.styleableCount)
  const ctaPhase = bareLook ? (state.applied ? 'dirty' : 'synced') : phase
  const ctaLabel = bareLook ? (state.applied ? t('Cta_RestoreDefault') : t('Cta_Synced')) : ctaText
  const shapeInMore = MORE_SHAPES.some((s) => s.value === c.shape)
  const shortcutShapeInMore = MORE_SHAPES.some((s) => s.value === c.shortcutShape)
  // Pure black/white are redundant with the 黑白 option (owner call 2026-07-09):
  // hidden from the dot row whatever the bridge sends; still reachable via 调色盘.
  const monoDots = state.monoSwatches.filter((s) => !['#FFFFFF', '#141414'].includes(s.toUpperCase()))
  const monoCustomTint =
    c.subject === 'Mono' && !monoDots.some((s) => s.toUpperCase() === c.tint.toUpperCase())
  const customPlate =
    c.plateColor !== null &&
    c.plateColor.toUpperCase() !== '#FFFFFF' &&
    !TYPE_PLATE_SWATCHES.some((h) => h.toUpperCase() === c.plateColor!.toUpperCase())

  // Shared SwatchPicker option builders (curated + 更多 rows, the filter flow).
  const shapeOption = (s: { value: IconShape; key: StringKey }): SwatchOption => ({
    key: s.value,
    title: t(s.key),
    selected: c.shape === s.value,
    onPick: () => mutate({ shape: s.value }),
    onHover: tryOn({ shape: s.value }),
    glyph: <ShapeSwatch shape={s.value} active={c.shape === s.value} />,
  })
  const shortcutShapeOption = (s: { value: IconShape; key: StringKey }): SwatchOption => ({
    key: s.value,
    title: t(s.key),
    selected: c.shortcutShape === s.value,
    onPick: () => mutate({ shortcutShape: s.value }),
    onHover: tryOn({ shortcutShape: s.value }),
    glyph: <ShapeSwatch shape={s.value} active={c.shortcutShape === s.value} />,
  })
  const filterOption = (o: { value: FilterStyle; key: StringKey }): SwatchOption => ({
    key: o.value,
    title: t(o.key),
    selected: c.filter === o.value,
    onPick: () => mutate({ filter: o.value }),
    onHover: tryOn({ filter: o.value }),
    glyph: <FilterSwatch filter={o.value} active={c.filter === o.value} />,
  })

  return (
    <aside className="@container relative flex w-[280px] shrink-0 flex-col gap-2.5 pl-1 pr-3 pt-1">
      {/* -mt-px/pt-px: 1px of clip headroom inside the scroller (zero net layout
          shift) so the preset cards' hover lift never shears their top border
          against the overflow edge. The status line lives INSIDE the scroller —
          it rides along with the panel (owner call 2026-07-09), never floats. */}
      <div style={{ paddingBottom: clearance }} className="scrollbar-none -mt-px flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto pt-px [&>*]:shrink-0">
        <p className="truncate px-0.5 text-[11px] text-t3/90" title={statusLine}>{statusLine}</p>
        {/* 风格 presets — style-card grid (filter-deck grammar): a visual thumbnail
            leads, the name sits on one line below, the description lives in the
            tooltip. Two columns grow downward forever — 4 presets or 40, same UI. */}
        {/* Preset fold (owner height complaint + designer ruling): the featured
            four max-difference cards show by default; a counting ghost row
            (「更多风格 +N」, never a bare More) expands IN PLACE — positions of
            the visible cards never reflow. Auto-open when the active preset
            hides behind the fold. */}
        {(() => {
          const activeHidden = state.presets.slice(3).some((p) => activePresetIdOf(state) === p.id)
          const foldOpen = morePresets || activeHidden
          const renderPresetCard = (p: (typeof state.presets)[number]) => {
            const meta = PRESET_NAME[p.id]
            // v2: derived client-side — the host no longer refreshes per edit. While
            // the bare look is active (A1), NO style preset is selected — System
            // Default owns the highlight even though config still matches a preset.
            const selected = !bareLook && activePresetIdOf(state) === p.id
            const presetHover = tryOn(p.config, p.typeOverrides)
            return (
              <button
                key={p.id}
                type="button"
                aria-pressed={selected}
                title={meta ? `${t(meta.name)} · ${t(meta.desc)}` : p.id}
                onClick={() => {
                  presetHover(false)
                  selectPreset(p.id)
                }}
                onMouseEnter={() => {
                  if (foldAnimating.current) return
                  presetHover(true)
                }}
                onMouseLeave={() => presetHover(false)}
                className={cn(
                  // Native feel (owner): hover changes COLOUR only — never size or
                  // position. No lift, no scale, no translate.
                  'group relative flex w-full flex-col overflow-hidden rounded-[10px] border bg-raised text-left transition-colors duration-150',
                  selected ? 'border-coral/50' : 'border-hair-strong hover:border-hair-strong',
                )}
              >
                <span
                  className={cn(
                    'flex h-11 w-full items-center justify-center gap-1 transition-colors',
                    selected ? 'bg-wash-preset' : 'bg-chip/60 group-hover:bg-chip',
                  )}
                >
                  <PresetMinis config={p.config} />
                </span>
                <span
                  className={cn(
                    'w-full truncate whitespace-nowrap px-2 py-1.5 text-center text-[11px] font-medium [word-break:keep-all]',
                    selected ? 'text-coral-ink' : 'text-t1',
                  )}
                >
                  {meta ? t(meta.name) : p.id}
                </span>
                {selected && (
                  <motion.span
                    initial={reduced ? false : { scale: 0.4, opacity: 0 }}
                    animate={{ scale: 1, opacity: 1 }}
                    transition={{ type: 'spring', stiffness: 520, damping: 26 }}
                    className="absolute right-1 top-1 flex size-3.5 items-center justify-center rounded-full bg-coral text-[9px] leading-none text-cta-ink"
                  >
                    ✓
                  </motion.span>
                )}
              </button>
            )
          }
          // System Default (A1) as the FIRST card in the style grid (owner: not a
          // separate row). Same card grammar as a style preset — a reset thumbnail
          // leads, name below — but selecting it shows the bare desktop in the preview
          // with NO host write (the CTA restores). No hover geometry (native feel).
          const systemDefaultCard = (
            <button
              key="system-default"
              type="button"
              aria-pressed={bareLook}
              title={`${t('Preset_SystemDefault')} · ${t('Preset_SystemDefault_Desc')}`}
              onClick={() => {
                bareTryOn(false)
                hover(null)
                selectSystemDefault()
              }}
              onMouseEnter={() => {
                if (foldAnimating.current) return
                bareTryOn(true)
              }}
              onMouseLeave={() => bareTryOn(false)}
              className={cn(
                'group relative flex w-full flex-col overflow-hidden rounded-[10px] border bg-raised text-left transition-colors duration-150',
                bareLook ? 'border-coral/50' : 'border-hair-strong',
              )}
            >
              <span
                className={cn(
                  'flex h-11 w-full items-center justify-center gap-1 transition-colors',
                  bareLook ? 'bg-wash-preset' : 'bg-chip/60 group-hover:bg-chip',
                )}
              >
                {/* The bare/original icons WITH the native shortcut arrow (owner):
                    same 3-mini grammar as every style card, so System Default reads
                    as the same category — its content is the ugly desktop the reset
                    returns to. */}
                <PresetMinis bare config={state.config} />
              </span>
              <span
                className={cn(
                  'w-full truncate whitespace-nowrap px-2 py-1.5 text-center text-[11px] font-medium',
                  bareLook ? 'text-coral-ink' : 'text-t1',
                )}
              >
                {t('Preset_SystemDefault')}
              </span>
              {bareLook && (
                <motion.span
                  initial={reduced ? false : { scale: 0.4, opacity: 0 }}
                  animate={{ scale: 1, opacity: 1 }}
                  transition={{ type: 'spring', stiffness: 520, damping: 26 }}
                  className="absolute right-1 top-1 flex size-3.5 items-center justify-center rounded-full bg-coral text-[9px] leading-none text-cta-ink"
                >
                  ✓
                </motion.span>
              )}
            </button>
          )
          return (
            <div>
              <div className="grid grid-cols-2 gap-1.5">
                {systemDefaultCard}
                {state.presets.slice(0, 3).map(renderPresetCard)}
              </div>
              {/* The fold EXPANDS, never pops (owner) — and stays SMOOTH
                  (owner jank report): the hidden cards are ALWAYS MOUNTED so
                  their live-renderer minis paint once at load, never inside
                  the animation frames; expansion animates height while the
                  cards ride transform+opacity (compositor-only). inert seals
                  the collapsed cards from focus/clicks. */}
              <motion.div
                className="overflow-hidden"
                initial={false}
                animate={{ height: foldOpen ? 'auto' : 0, opacity: foldOpen ? 1 : 0 }}
                transition={reduced ? { duration: 0 } : { duration: 0.22, ease: [0.33, 1, 0.68, 1] }}
                aria-hidden={!foldOpen}
                {...(foldOpen ? {} : { inert: '' as never })}
              >
                <div className="grid grid-cols-2 gap-1.5 pt-1.5">
                  {state.presets.slice(3).map((p, i) => (
                    <motion.div
                      key={p.id}
                      className="flex"
                      initial={false}
                      animate={foldOpen ? { opacity: 1, y: 0 } : { opacity: 0, y: 8 }}
                      transition={
                        reduced
                          ? { duration: 0 }
                          : foldOpen
                            ? { duration: 0.24, ease: [0.33, 1, 0.68, 1], delay: 0.04 + i * 0.045 }
                            : { duration: 0.12 }
                      }
                    >
                      {renderPresetCard(p)}
                    </motion.div>
                  ))}
                </div>
              </motion.div>
            </div>
          )
        })()}
        {state.presets.length > 4 && !state.presets.slice(4).some((p) => activePresetIdOf(state) === p.id) && (
          <button
            type="button"
            onClick={() => {
              armFoldFreeze()
              setMorePresets((v) => !v)
            }}
            className="mx-auto mt-0.5 flex items-center gap-0.5 rounded px-2 py-0.5 text-[11px] text-coral-ink transition-colors hover:text-coral"
          >
            {morePresets ? t('Preset_Collapse') : format(t('Preset_MoreN'), state.presets.length - 4)}
            <ChevronDown size={12} className={cn('transition-transform duration-200', morePresets && 'rotate-180')} />
          </button>
        )}

        {/* 自定义 — one grouped card, one grammar per axis kind */}
        <InspectorCard>
          {/* OPEN SET: shapes — curated swatches + 更多 fold, wrapping flow */}
          <PropertyRow
            label={t('Axis_Shape')}
            labelExtra={
              <button
                type="button"
                onClick={() => setMoreShapes((v) => !v)}
                className={cn(
                  'flex items-center gap-0.5 whitespace-nowrap rounded-md px-1.5 py-0.5 text-[11px] transition-colors',
                  moreShapes || shapeInMore ? 'bg-wash-chip text-coral-ink' : 'text-t3 hover:text-t1',
                )}
              >
                {t('Shape_More')}
                <ChevronDown size={11} className={cn('shrink-0 transition-transform duration-150', moreShapes && 'rotate-180')} />
              </button>
            }
          >
            {/* Shape swatches stay one visual block (owner 2026-07-10): the
                更多 fold expands DIRECTLY under the curated row; the kind
                switch always sits below the full swatch field. */}
            <SwatchPicker options={CURATED_SHAPES.map(shapeOption)} />
            <Reveal show={moreShapes}>
              <SwatchPicker className="pt-2" options={MORE_SHAPES.map(shapeOption)} />
            </Reveal>
            {/* Per-type shapes moved to the TYPE accordion (ADR-0017 D5 — the
                One-shape/By-type segmented is gone by owner order). This row
                edits the GLOBAL base shape; a ghost note appears while the
                uniform shortcut shape overrides it for shortcuts. */}
            {c.shortcutShape && (
              <p className="mt-1.5 text-[11px] text-t3">{t('Shortcut_ShapeGhost')}</p>
            )}
          </PropertyRow>

          {/* 主体 Subject axis (ADR-0018): how the ARTWORK renders — 原彩 /
              黑白 / 单色 dots / custom wheel. No modes: the plate lives on
              its own row below. */}
          <PropertyRow label={t('Subject_Label')}>
            <div className="flex flex-wrap items-center gap-1">
              <SwatchButton
                title={t('Subject_Orig')}
                selected={c.subject === 'Original'}
                onHover={tryOn({ subject: 'Original' })}
                onClick={() => mutate({ subject: 'Original' })}
              >
                <FieldGlyph />
              </SwatchButton>
              <SwatchButton
                title={t('Color_Bw')}
                selected={c.subject === 'BlackWhite'}
                onHover={tryOn({ subject: 'BlackWhite' })}
                onClick={() => mutate({ subject: 'BlackWhite' })}
              >
                <BwGlyph />
              </SwatchButton>
              {monoDots.map((s) => (
                <SwatchButton
                  key={s}
                  title={s}
                  selected={c.subject === 'Mono' && c.tint.toUpperCase() === s.toUpperCase()}
                  onHover={tryOn({ subject: 'Mono', tint: s })}
                  onClick={() => mutate({ subject: 'Mono', tint: s })}
                >
                  {/* Concentric pair (owner grammar): inner = subject tint,
                      outer = the plate that tint derives (ramp light end). */}
                  <PairDot fg={s} bg={paleOf(s)} />
                </SwatchButton>
              ))}
              <Popover>
                <PopoverTrigger asChild>
                  <button
                    type="button"
                    title={t('Palette_Button')}
                    aria-label={t('Palette_Button')}
                    className={swatchButtonClass(monoCustomTint)}
                  >
                    <WheelRing size={20} value={c.tint} active={monoCustomTint} />
                  </button>
                </PopoverTrigger>
                <PopoverContent side="bottom" align="end" className="w-auto rounded-[14px] p-3">
                  {/* Single-purpose picker (ADR-0018: the fg/bg dual-tab is dead). */}
                  <ColorPickerPanel
                    value={c.tint}
                    onChange={(tint) => mutate({ subject: 'Mono', tint })}
                    wallpaperSwatches={state.palette}
                    quickSwatches={QUICK_SWATCHES}
                  />
                </PopoverContent>
              </Popover>
            </div>
            {/* 单色 depth: 渐层 = tonal ramp; 纯平 = 极致单色 flat subject. */}
            <Reveal show={c.subject === 'Mono'}>
              <Segmented
                size="sm"
                className="mt-2"
                value={c.monoStyle}
                options={[
                  { value: 'Tonal', label: t('Mono_Tonal') },
                  { value: 'Flat', label: t('Mono_Flat') },
                ]}
                onChange={(monoStyle) => mutate({ monoStyle })}
              />
            </Reveal>
          </PropertyRow>

          {/* 底板 Plate axis (ADR-0018): 随图标(derived) FIRST — the
              algorithm is the soul, never buried; then 本色(anchors-else-
              white), 白, the bounded swatches, the free wheel. ALWAYS
              visible; needs a container, so shape=None DISABLES (never
              hides — hiding was the recolour dead-end's root). */}
          <PropertyRow label={t('Plate_Label')}>
            <div
              className={cn(
                'flex flex-wrap items-center gap-1',
                c.shape === 'None' && 'pointer-events-none opacity-40',
              )}
              aria-disabled={c.shape === 'None'}
            >
              <SwatchButton
                title={t('Plate_Auto')}
                selected={c.plateColor === null && c.plateFallback === 'derived'}
                onHover={tryOn({ plateColor: null, plateFallback: 'derived' })}
                onClick={() => mutate({ plateColor: null, plateFallback: 'derived' })}
              >
                <QuadPlateGlyph band={c.plateBand} />
              </SwatchButton>
              <SwatchButton
                title={t('Plate_Faithful')}
                selected={c.plateColor === null && c.plateFallback === 'white'}
                onHover={tryOn({ plateColor: null, plateFallback: 'white' })}
                onClick={() => mutate({ plateColor: null, plateFallback: 'white' })}
              >
                <FaithfulGlyph />
              </SwatchButton>
              <SwatchButton
                title={t('Plate_White')}
                selected={c.plateColor?.toUpperCase() === '#FFFFFF'}
                onHover={tryOn({ plateColor: '#FFFFFF' })}
                onClick={() => mutate({ plateColor: '#FFFFFF' })}
              >
                <PairDot fg="#FFFFFF" bg="#FFFFFF" />
              </SwatchButton>
              {TYPE_PLATE_SWATCHES.map((hex) => (
                <SwatchButton
                  key={hex}
                  title={hex}
                  selected={c.plateColor?.toUpperCase() === hex.toUpperCase()}
                  onHover={tryOn({ plateColor: hex })}
                  onClick={() => mutate({ plateColor: hex })}
                >
                  <span className="block size-5 rounded-md ring-1 ring-hair" style={{ background: hex }} />
                </SwatchButton>
              ))}
              <Popover>
                <PopoverTrigger asChild>
                  <button
                    type="button"
                    title={t('Palette_Button')}
                    aria-label={t('Palette_Button')}
                    className={swatchButtonClass(customPlate)}
                  >
                    <WheelRing size={20} value={c.plateColor ?? '#FFFFFF'} active={customPlate} />
                  </button>
                </PopoverTrigger>
                <PopoverContent side="bottom" align="end" className="w-auto rounded-[14px] p-3">
                  <ColorPickerPanel
                    value={c.plateColor ?? '#FFFFFF'}
                    onChange={(plateColor) => mutate({ plateColor })}
                    wallpaperSwatches={state.palette}
                    quickSwatches={QUICK_SWATCHES}
                  />
                </PopoverContent>
              </Popover>
            </div>
            {c.shape === 'None' && (
              <p className="mt-1.5 text-[11px] text-t3">{t('Plate_NeedShape')}</p>
            )}
            {/* Derived-plate depth: only meaningful for 原彩 × 随图标. */}
            <Reveal show={c.subject === 'Original' && c.plateColor === null && c.plateFallback === 'derived'}>
              <Segmented
                size="sm"
                className="mt-2"
                value={c.plateBand}
                options={[
                  { value: 'Vivid', label: t('Field_Vivid') },
                  { value: 'Quiet', label: t('Field_Quiet') },
                ]}
                onChange={(plateBand) => mutate({ plateBand })}
              />
            </Reveal>
          </PropertyRow>

          {/* OPEN SET: filters — visual effect tiles, wrapping flow (future filters
              extend it). 光泽 went live 2026-07-09 (engine gloss in filters.ts). */}
          <PropertyRow label={t('Axis_Filter')}>
            <SwatchPicker options={FILTERS.map(filterOption)} />
          </PropertyRow>

          {/* OPEN SET: shortcut marks — ONE flat swatch flow, no mode segmented
              (owner call): 无 first, then the native Windows arrow (blue on white,
              deliberately recognizable — and deliberately not tempting), then the
              beautified mark styles. The colour wheel lives in the header's right
              slot (the same corner every row keeps its auxiliary control in),
              appearing once a mark is active. */}
          <PropertyRow
            label={t('Axis_Dist')}
            labelExtra={
              c.distinction === 'Mark' && c.markStyle !== 'Shadow' ? (
                <motion.span
                  initial={reduced ? false : { scale: 0.5, opacity: 0 }}
                  animate={{ scale: 1, opacity: 1 }}
                  transition={{ type: 'spring', stiffness: 520, damping: 26 }}
                  className="flex"
                >
                  <AuxColorDot
                    value={c.markColor}
                    swatches={state.markSwatches}
                    wallpaper={state.palette}
                    autoLabel={t('MarkColor_Auto')}
                    label={t('MarkColor_Label')}
                    onPick={throttledMarkColor}
                  />
                </motion.span>
              ) : undefined
            }
          >
            <SwatchPicker
              options={[
                {
                  key: 'none',
                  title: t('Dist_None'),
                  selected: c.distinction === 'None',
                  onPick: () => mutate({ distinction: 'None' }),
                  onHover: tryOn({ distinction: 'None' }),
                  glyph: <NoneGlyph active={c.distinction === 'None'} />,
                },
                ...MARKS.map((m) => ({
                  key: m.value,
                  title: t(m.key),
                  selected: c.distinction === 'Mark' && c.markStyle === m.value,
                  onPick: () => mutate({ distinction: 'Mark', markStyle: m.value }),
                  onHover: tryOn({ distinction: 'Mark', markStyle: m.value }),
                  glyph: <MarkGlyph mark={m.value} active={c.distinction === 'Mark' && c.markStyle === m.value} />,
                })),
                // LAST, behind the gate (owner decree): picking the native arrow
                // opens a sixty-second penance sheet before it takes effect. It
                // flows naturally at the row's end like any other option.
                {
                  key: 'keep',
                  title: t('Dist_Keep'),
                  selected: c.distinction === 'Keep',
                  onPick: () => {
                    if (c.distinction !== 'Keep') setArrowGateOpen(true)
                  },
                  // Try-on works here too — seeing the arrow everywhere IS the argument.
                  onHover: tryOn({ distinction: 'Keep' }),
                  glyph: <WinArrowGlyph active={c.distinction === 'Keep'} />,
                },
              ]}
            />
            {/* The native-arrow disclosure lives ONLY in the first-beautify consent
                dialog + Settings (owner 2026-07-11): the native arrow always goes
                transparent on apply regardless of the mark choice, so a permanent
                inline hint here just nags. Removed. */}
          </PropertyRow>

          {/* Uniform shortcut shape (ADR-0017 D5; control spec 2026-07-10):
              a RADIO row in the Shape-axis grammar - 无 first (= shortcuts
              keep their type's shape), curated chips, 更多 fold. No toggle,
              no collapse: the None chip IS the off state. */}
          <PropertyRow
            label={t('Shortcut_UniformShape')}
            labelExtra={
              <button
                type="button"
                onClick={() => setMoreShortcutShapes((v) => !v)}
                className={cn(
                  'flex items-center gap-0.5 whitespace-nowrap rounded-md px-1.5 py-0.5 text-[11px] transition-colors',
                  moreShortcutShapes || shortcutShapeInMore ? 'bg-wash-chip text-coral-ink' : 'text-t3 hover:text-t1',
                )}
              >
                {t('Shape_More')}
                <ChevronDown size={11} className={cn('shrink-0 transition-transform duration-150', moreShortcutShapes && 'rotate-180')} />
              </button>
            }
          >
            <SwatchPicker
              options={[
                {
                  key: 'none',
                  title: t('Shape_None'),
                  selected: c.shortcutShape === null,
                  onPick: () => mutate({ shortcutShape: null }),
                  onHover: tryOn({ shortcutShape: null }),
                  glyph: <NoneGlyph active={c.shortcutShape === null} />,
                },
                ...CURATED_SHAPES.filter((o) => o.value !== 'None').map(shortcutShapeOption),
              ]}
            />
            <Reveal show={moreShortcutShapes}>
              <SwatchPicker className="pt-2" options={MORE_SHAPES.map(shortcutShapeOption)} />
            </Reveal>
          </PropertyRow>

          {/* The persistent per-type participation policy as a 5th axis row
              (spec 06 §6), then the per-icon keep ledger (spec 06 §3.4) — both
              one store state, no matrix. */}
          <KindTypeSection />
          <KeptBar />
        </InspectorCard>

        {/* History card (on demand) */}
        <AnimatePresence>
          {historyOpen && state.history.length > 0 && (
            <motion.div
              initial={reduced ? false : { opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.18 }}
            >
              <HistoryStrip
                items={[...state.history].reverse().map((h) => ({
                  key: h.index,
                  time: h.time,
                  label: h.label,
                  isCurrent: h.isCurrent,
                  config: h.config,
                  index: h.index,
                }))}
                renderThumb={(h) => <IconVersionThumb config={h.config} />}
                onGoTo={(h) => goVersion(h.index)}
                onBackToInitial={() => void restore()}
                disabled={state.working}
              />
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* Floating footer (owner-tuned): a frosted gradient backing — OPAQUE at the
          very bottom, fading upward to transparent with the blur masked out in
          step — so passing content calms down before it reaches the controls
          instead of jittering between them. */}
      <div
        ref={footerRef}
        className="absolute inset-x-0 bottom-0 z-20 bg-gradient-to-t from-background from-65% via-background/55 to-transparent"
      >
        <div className="flex flex-col gap-1.5 pb-3 pl-1 pr-3 pt-5">
        {state.applied && (
          <div className="flex items-center gap-1">
            <IconAction title={t('Link_Restore_Tip')} onClick={() => setRestoreOpen(true)}>
              <RotateCcw size={11} />
              {t('Link_Restore')}
            </IconAction>
            {state.history.length > 0 && (
              <IconAction
                title={t('Link_History_Tip')}
                active={historyOpen}
                onClick={() => setHistoryOpen((v) => !v)}
              >
                <History size={11} />
                {t('Link_History')} {state.history.length}
              </IconAction>
            )}
          </div>
        )}
          <div ref={ctaWrapRef}>
            <CtaButton phase={ctaPhase} onClick={onCta}>
              {ctaLabel}
            </CtaButton>
          </div>
        </div>
      </div>

      {/* First-apply confetti cannons — full-window, ABOVE the DoneCard. */}
      <Confetti fireKey={celebrateKey} />

      {/* First-apply coral ripple — a bright ring + wash bursting from the CTA into
          the preview (fixed so it overflows the panel), self-removing. */}
      {ripple && (
        <motion.div
          key={ripple.key}
          aria-hidden
          className="pointer-events-none fixed z-[99] rounded-full"
          style={{
            left: ripple.cx,
            top: ripple.cy,
            width: 560,
            height: 560,
            marginLeft: -280,
            marginTop: -280,
            background:
              'radial-gradient(circle, rgba(255,111,94,0.40) 0%, rgba(255,111,94,0.16) 46%, transparent 68%)',
            border: '2px solid rgba(255,111,94,0.55)',
          }}
          initial={{ scale: 0.22, opacity: 0.9 }}
          animate={{ scale: 1, opacity: 0 }}
          transition={{ duration: 0.62, ease: [0.22, 1, 0.36, 1] }}
          onAnimationComplete={() => setRipple(null)}
        />
      )}

      <ConsentSheet
        open={consentOpen}
        count={state.styleableCount}
        multiUser={state.activeUserProfiles > 1}
        onAgree={() => {
          // v2 records that the machine-wide arrow disclosure was shown; keep
          // the legacy bit for any other reader.
          localStorage.setItem('dm.consent.icons.v2', '1')
          localStorage.setItem('dm.consent.icons', '1')
          setConsentOpen(false)
          void runApply()
        }}
        onCancel={() => setConsentOpen(false)}
      />
      <ConfirmSheet
        open={restoreOpen}
        title={t('RestoreConfirm')}
        confirmLabel={t('Link_Restore')}
        cancelLabel={t('ConsentCancel')}
        destructive
        onConfirm={() => {
          setRestoreOpen(false)
          void restore()
        }}
        onCancel={() => setRestoreOpen(false)}
      />
      <ArrowGateSheet
        open={arrowGateOpen}
        onConfirm={() => {
          setArrowGateOpen(false)
          mutate({ distinction: 'Keep' })
        }}
        onCancel={() => setArrowGateOpen(false)}
      />
      {/* DoneCard reinforcement (panel record 2026-07-11): the arrow line only
          appears when the overlay is ACTUALLY hidden now — a failed or overlay-
          declined apply never claims it (review P2-1). */}
      <DoneCard
        open={doneOpen}
        note={showDoneArrowNote(state.arrowOverlay) ? t('DoneArrow') : undefined}
        onClose={() => setDoneOpen(false)}
      />
    </aside>
  )
}

/** Preset thumbnails rendered by the LIVE compositor on the user's own icons
 *  (v2: no more host-rendered miniUrls) — badge-free by construction, since
 *  every preset ships Distinction.None (owner decree). */
function PresetMinis({ config, bare = false }: { config: ConfigDto; bare?: boolean }) {
  const items = useIcons((s) => s.items)
  const renderTick = useIcons((s) => s.renderTick)
  const samples = React.useMemo(
    () => items.filter((i) => i.styleable && i.kind !== 'RecycleBin').slice(0, 3),
    [items],
  )
  return (
    <>
      {samples.map((item) => (
        <PresetMiniCanvas key={item.id} itemId={item.id} sourceUrl={item.sourceUrls[0] ?? ''} config={config} renderTick={renderTick} bare={bare} />
      ))}
    </>
  )
}

function PresetMiniCanvas({
  itemId,
  sourceUrl,
  config,
  renderTick,
  bare = false,
}: {
  itemId: string
  sourceUrl: string
  config: ConfigDto
  renderTick: number
  /** System Default preview: the ORIGINAL unmodified icon WITH the native Windows
   *  shortcut arrow (is_shortcut + show_original) — the ugly bare desktop the reset
   *  returns to. The tile renderer bakes the real arrow asset, so the mini IS the
   *  outcome. */
  bare?: boolean
}) {
  const ref = React.useRef<HTMLCanvasElement>(null)
  React.useEffect(() => {
    const compositor = getIconCompositor()
    if (!sourceUrl || !compositor.hasSource(itemId, sourceUrl)) return // renderTick re-fires once loaded
    const image = compositor.getTile(itemId, config, bare, bare, 44, fieldRenderOpts(itemId))
    const el = ref.current
    if (!el || !image) return // pool render dispatched — next renderTick blits
    el.width = 44
    el.height = 44
    el.getContext('2d')!.drawImage(image, 0, 0)
  }, [itemId, sourceUrl, config, renderTick])
  return <canvas ref={ref} className="size-[22px] drop-shadow-sm" aria-hidden />
}

/** A 24px live preview of a saved version — one representative icon styled with
 *  that version's config (history strip thumbnail). */
function IconVersionThumb({ config }: { config: ConfigDto }) {
  const items = useIcons((s) => s.items)
  const renderTick = useIcons((s) => s.renderTick)
  const sample = React.useMemo(
    () => items.find((i) => i.styleable && i.kind !== 'RecycleBin' && i.sourceUrls[0]),
    [items],
  )
  if (!sample) return null
  return (
    <PresetMiniCanvas
      itemId={sample.id}
      sourceUrl={sample.sourceUrls[0] ?? ''}
      config={config}
      renderTick={renderTick}
    />
  )
}

/** 标识配色 — the house colour-wheel ring (same face as every 调色盘 entry),
 *  living in the row header's auxiliary slot; auto = white centre, a pick
 *  fills the centre. */
/** Leading+trailing throttle for continuous colour drags: the first value
 *  paints instantly, intermediate values collapse to one per window, and the
 *  release value always lands. */
function useThrottledChange<T>(fn: (v: T) => void, ms = 140): (v: T) => void {
  const fnRef = React.useRef(fn)
  fnRef.current = fn
  const last = React.useRef(0)
  const timer = React.useRef<ReturnType<typeof setTimeout> | null>(null)
  const pending = React.useRef<{ v: T } | null>(null)
  React.useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current)
  }, [])
  return React.useCallback(
    (v: T) => {
      pending.current = { v }
      const fire = () => {
        timer.current = null
        last.current = Date.now()
        if (pending.current) {
          fnRef.current(pending.current.v)
          pending.current = null
        }
      }
      const wait = last.current + ms - Date.now()
      if (wait <= 0) fire()
      else if (!timer.current) timer.current = setTimeout(fire, wait)
    },
    [ms],
  )
}

/** The Auto plate colour a mono tint produces (the ramp's light end) as hex —
 *  what the engine paints when plateColor = null, so the swatch previews truth. */

/** Row-corner auxiliary colour wheel (the mark colour keeps it). */
function AuxColorDot({
  value,
  swatches,
  wallpaper,
  autoLabel,
  label,
  onPick,
}: {
  value: string | null
  swatches: string[]
  wallpaper: string[]
  autoLabel: string
  label: string
  onPick: (hex: string | null) => void
}) {
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          title={label}
          aria-label={label}
          className={cn(
            'shrink-0 rounded-full transition-transform hover:scale-110 active:scale-95',
            value && 'ring-2 ring-coral ring-offset-1 ring-offset-raised',
          )}
        >
          <WheelRing size={18} value={value ?? '#FFFFFF'} active={!!value} />
        </button>
      </PopoverTrigger>
      <PopoverContent side="bottom" align="end" className="w-auto rounded-[14px] p-3">
        <div className="mb-2 flex flex-wrap items-center gap-1.5">
          <AutoDot selected={value === null} onClick={() => onPick(null)} label={autoLabel} />
          {swatches.map((s) => (
            <ColorSwatchDot key={s} color={s} selected={value?.toUpperCase() === s.toUpperCase()} onClick={() => onPick(s)} />
          ))}
        </div>
        <ColorPickerPanel
          value={value ?? '#FFFFFF'}
          onChange={onPick}
          wallpaperSwatches={wallpaper}
          quickSwatches={QUICK_SWATCHES}
        />
      </PopoverContent>
    </Popover>
  )
}
