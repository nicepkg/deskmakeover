import * as React from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { Check, ChevronDown, Folder, LayoutGrid, Monitor, PanelBottom, RotateCcw } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { PropertyRow, SwatchButton, SwatchPicker, swatchButtonClass } from '@/components/common/inspector'
import { BwGlyph, KindGlyph, NoneGlyph, PairDot, QuadPlateGlyph, ShapeSwatch } from '@/components/common/chip-preview'
import { AutoGlyph, QUICK_SWATCHES, WheelRing } from '@/components/common/color-controls'
import { ColorPickerPanel } from '@/components/common/color-picker'
import { ALL_SHAPES, TYPE_PLATE_SWATCHES, paleOf } from '@/components/panels/icon-axis-options'
import { useIcons } from '@/stores/icons'
import { BUCKET_NAME_KEY, KIND_BUCKETS, kindBucket } from '@/lib/kind-policy'
import { typeIsCustom } from '@/lib/type-config'
import type { ConfigDto, IconKindBucket, Surfaces, TypeOverrides, TypePatch } from '@/bridge/types'
import { format, useT, type StringKey } from '@/lib/i18n'

// Participation surfaces (chief-UI/UX + owner 2026-07-09), both fed by one
// store state — no priority matrix:
//   · KeptBar   = the per-icon 「保留原样」 ledger (was the ugly exception row):
//     a pin-dot + inset chip, ghost clear, click to expand a per-item cancel list.
//   · KindTypeSection = the PERSISTENT per-bucket policy (kindPolicy), rendered as
//     the 5th axis row (chief-UI/UX + owner 2026-07-09): four glyph tiles in the
//     same PropertyRow grammar as the styling axes, but with a checkbox corner so
//     they read as MULTI-select, never pick-one. Always visible — even a count-0
//     bucket toggles, so a user can pre-opt-out a kind the desktop has none of yet.
//     ADR-0024: the same row grammar grows a SURFACES group (live counts; the
//     desktop row is the disabled always-on anchor) — the second participation
//     axis, orthogonal to kindPolicy.

/** The toggleable surface legs (the desktop is never a leg — always on). */
const SURFACE_LEGS = ['taskbar', 'startMenu', 'explorer'] as const satisfies readonly (keyof Surfaces)[]

/** Per-leg label + icon (the row grammar stays identical across legs). */
const SURFACE_LEG_META: Record<keyof Surfaces, { label: StringKey; Icon: typeof PanelBottom }> = {
  taskbar: { label: 'Surface_Taskbar', Icon: PanelBottom },
  startMenu: { label: 'Surface_StartMenu', Icon: LayoutGrid },
  explorer: { label: 'Surface_Explorer', Icon: Folder },
}

/** The per-icon keep ledger — soft inset chip, pin dot, expandable cancel list. */
export function KeptBar() {
  const t = useT()
  // Select the STABLE items ref (a new-array selector makes zustand's
  // getSnapshot change every render → infinite loop); filter in the body.
  const items = useIcons((s) => s.items)
  // Lens projection (spec 06 §3.13): while 系统默认 is active the whole panel
  // reads system-default — the keep ledger is suppressed (draft preserved).
  const bareLook = useIcons((s) => s.bareLook)
  const kept = React.useMemo(() => items.filter((i) => i.overrideMode !== null), [items])
  const { setOverride, clearOverrides } = useIcons.getState()
  const [open, setOpen] = React.useState(false)
  const reduced = useReducedMotion()
  if (kept.length === 0 || bareLook) return null

  return (
    <div className="rounded-[10px] bg-wash-chip px-2.5 py-2">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className="flex min-w-0 flex-1 items-center gap-1.5 text-[11px] text-t2 transition-colors hover:text-t1"
        >
          <span className="size-2 shrink-0 rounded-full bg-coral ring-1 ring-white/70" aria-hidden />
          <span className="truncate">{format(t('Icons_KeptCount'), kept.length)}</span>
          <ChevronDown size={12} className={open ? 'rotate-180 transition-transform' : 'transition-transform'} />
        </button>
        <button
          type="button"
          onClick={() => {
            clearOverrides()
            setOpen(false)
          }}
          className="shrink-0 rounded-[6px] px-1.5 py-0.5 text-[11px] text-t3 transition-colors hover:bg-raised-hov hover:text-t1"
        >
          {t('Icons_ClearKept')}
        </button>
      </div>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            className="overflow-hidden"
            initial={reduced ? false : { height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={reduced ? { opacity: 0 } : { height: 0, opacity: 0 }}
            transition={{ duration: 0.16, ease: [0.33, 1, 0.68, 1] }}
          >
            <div className="mt-1.5 space-y-0.5 border-t border-hair/60 pt-1.5">
              {kept.map((i) => (
                <div key={i.id} className="flex items-center gap-2 text-[11px]">
                  <span className="min-w-0 flex-1 truncate text-t2">{i.label}</span>
                  <button
                    type="button"
                    onClick={() => setOverride(i.id, 'follow')}
                    className="shrink-0 text-coral-ink transition-colors hover:underline"
                  >
                    {t('Icons_ReincludeCancel')}
                  </button>
                </div>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}

/** The panel's hover try-on channel (icons-panel `tryOn`): the debounced
 *  candidate painter the type chips share with the global axes, so hovering a
 *  per-type option previews on the canvas exactly like a global chip does
 *  (owner 2026-07-16: the type rows previously previewed nothing — 割裂). */
export type TypeTryOn = (change: Partial<ConfigDto>, typeOverrides?: TypeOverrides) => (hovering: boolean) => void

/** The TYPE ACCORDION v2 (ADR-0017 D5; chief-designer control spec
 *  2026-07-10) — every option is the SAME SwatchButton chip grammar as the
 *  global axes, nothing invented: each sub-axis leads with an AutoGlyph
 *  (跟随全局 anchor), the colour row IS the global Colour row minus
 *  Original, the plate row is filled colour chips. Row headers are standard
 *  list rows (beautify checkbox · glyph · name · status badge · chevron),
 *  separated by hair lines — no heavy coral frames. One row open at a time;
 *  the open row scope-highlights its icons on the canvas (dims the rest). */
export function KindTypeSection({ tryOn }: { tryOn: TypeTryOn }) {
  const t = useT()
  const items = useIcons((s) => s.items)
  const policy = useIcons((s) => s.state?.kindPolicy)
  const surfaces = useIcons((s) => s.state?.surfaces)
  const config = useIcons((s) => s.state?.config)
  const rawMonoSwatches = useIcons((s) => s.state?.monoSwatches)
  // Owner 2026-07-09: pure black/white mono dots are redundant with the 黑白
  // stop — filtered here EXACTLY like the global Subject row (§6.9 sameness).
  const monoSwatches = React.useMemo(
    () => (rawMonoSwatches ?? []).filter((v) => !['#FFFFFF', '#141414'].includes(v.toUpperCase())),
    [rawMonoSwatches],
  )
  const palette = useIcons((s) => s.state?.palette)
  const rawTypeOverrides = useIcons((s) => s.state?.typeOverrides)
  // Lens projection (spec 06 §3.13, item-6 rule extended to this section):
  // while 系统默认 (bareLook) is active, the DISPLAY projects every bucket to
  // 跟随全局 — badges, sub-axis anchors, and the 全部重置/↺ affordances all
  // read follow-global. The underlying draft typeOverrides are preserved and
  // resurface losslessly when the lens lifts. The participation checkboxes are
  // the deliberate exception: kindPolicy is orthogonal to the style lens and
  // keeps rendering its real persisted state.
  const bareLook = useIcons((s) => s.bareLook)
  const typeOverrides = bareLook ? undefined : rawTypeOverrides
  const { setKindPolicy, setSurface, setTypeOverride, setEditingBucket, resetTypeOverrides } = useIcons.getState()
  const [openBucket, setOpenBucket] = React.useState<IconKindBucket | null>(null)
  const reduced = useReducedMotion()

  // Hooks must run unconditionally (before any early return).
  const counts = React.useMemo(() => {
    const c = { App: 0, Folder: 0, File: 0 }
    for (const i of items) {
      const b = kindBucket(i.kind)
      if (b) c[b]++
    }
    return c
  }, [items])

  // ADR-0024: live per-surface counts — what the scan actually found on each
  // write surface (the honest number behind each checkbox).
  const surfaceCounts = React.useMemo(() => {
    const c = { taskbar: 0, startMenu: 0, explorer: 0 }
    for (const i of items) {
      if (i.surface === 'taskbar') c.taskbar++
      else if (i.surface === 'startMenu') c.startMenu++
      else if (i.surface === 'explorer') c.explorer++
    }
    return c
  }, [items])

  // Scope feedback follows the open row; never leak a stale scope on unmount.
  React.useEffect(() => {
    setEditingBucket(openBucket)
    return () => setEditingBucket(null)
  }, [openBucket, setEditingBucket])

  if (!policy || !surfaces || !config) return null

  type PatchChange = Partial<Record<keyof TypePatch, TypePatch[keyof TypePatch] | undefined>>

  // Merge onto the REAL draft patch, never the lens projection: a value-
  // asserting edit lifts the lens and applies onto the preserved draft
  // (spec 06 §3.13) — exactly like the main axes' mutate() onto config.
  const mergedPatch = (bucket: IconKindBucket, change: PatchChange): TypePatch => {
    const current: TypePatch = { ...(rawTypeOverrides?.[bucket]?.patch ?? {}) }
    for (const [k, v] of Object.entries(change)) {
      if (v === undefined) delete current[k as keyof TypePatch]
      else (current as Record<string, unknown>)[k] = v
    }
    return current
  }

  const patchType = (bucket: IconKindBucket, change: PatchChange) => {
    setTypeOverride(bucket, { source: 'custom', patch: mergedPatch(bucket, change) })
  }

  // Hover try-on for a type chip: the candidate is the FULL ladder a click
  // would commit (merged patch in place, empty patch = entry removed), painted
  // through the shared debounced channel. `{}` keeps the base config as-is —
  // only the hovered bucket's icons change on the canvas.
  const tryType = (bucket: IconKindBucket, change: PatchChange): ((hovering: boolean) => void) => {
    const patch = mergedPatch(bucket, change)
    const ladder: TypeOverrides = { ...rawTypeOverrides }
    if (Object.keys(patch).length === 0) delete ladder[bucket]
    else ladder[bucket] = { source: 'custom', patch }
    return tryOn({}, ladder)
  }

  const anyCustom = KIND_BUCKETS.some((b) => typeIsCustom(typeOverrides, b))

  return (
    <PropertyRow
      label={t('Axis_Kind')}
      labelExtra={
        anyCustom ? (
          <button
            type="button"
            onClick={() => resetTypeOverrides()}
            className="-mr-1.5 flex items-center gap-1 whitespace-nowrap rounded-md px-1.5 py-0.5 text-[11px] text-t3 transition-colors hover:text-coral-ink"
          >
            {t('Type_ResetAll')}
            <RotateCcw size={11} className="shrink-0" />
          </button>
        ) : undefined
      }
    >
      <div className="divide-y divide-hair/60">
        {KIND_BUCKETS.map((b) => {
          const on = policy[b]
          const name = t(BUCKET_NAME_KEY[b])
          const custom = typeIsCustom(typeOverrides, b)
          const patch: TypePatch = typeOverrides?.[b]?.patch ?? {}
          const open = openBucket === b
          const monoCustom =
            patch.subject === 'Mono' && !!patch.tint && !monoSwatches.some((m) => m.toUpperCase() === patch.tint!.toUpperCase())
          const plateCustom =
            !!patch.plateColor &&
            patch.plateColor.toUpperCase() !== '#FFFFFF' &&
            !TYPE_PLATE_SWATCHES.some((h) => h.toUpperCase() === patch.plateColor!.toUpperCase())
          return (
            <div key={b} className="py-0.5">
              {/* Header: standard list row — checkbox(参与美化) · glyph · name ·
                  status badge · chevron. Subtle ring only while open. */}
              <div
                className={cn(
                  // Align the type-row content with the swatch rows above (owner 2026-07-14):
                  // the row keeps its rounded hover/ring via a negative margin, but its CONTENT
                  // lines up with the PropertyRow px-3 edge instead of an extra 1.5 inset.
                  'flex items-center gap-2 rounded-lg -mx-1.5 px-1.5 py-1.5',
                  open && 'ring-1 ring-coral/35',
                )}
              >
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={on}
                  aria-label={name}
                  title={t('Axis_Kind')}
                  onClick={() => setKindPolicy(b, !on)}
                  className="shrink-0 active:scale-95"
                >
                  <span
                    className={cn(
                      'flex size-3.5 items-center justify-center rounded-[4px] border transition-colors',
                      on ? 'border-coral bg-coral text-cta-ink' : 'border-hair-strong bg-raised',
                    )}
                  >
                    {on && <Check size={9} strokeWidth={3} />}
                  </span>
                </button>
                <button
                  type="button"
                  aria-expanded={open}
                  onClick={() => setOpenBucket(open ? null : b)}
                  className="flex min-w-0 flex-1 items-center gap-1.5 text-[11px]"
                  title={`${name} · ${counts[b]}`}
                >
                  <KindGlyph bucket={b} muted={!on} size={17} className="shrink-0" />
                  <span className={cn('min-w-0 truncate text-left', on ? 'text-t1' : 'text-t3 line-through')}>{name}</span>
                  <span className="flex-1" />
                  <div className="flex shrink-0 items-center gap-1">
                    {custom ? (
                      <span className="rounded bg-wash-chip px-1 text-[9px] leading-4 text-coral-ink">{t('Type_Custom')}</span>
                    ) : (
                      <span className="text-[10px] text-t3">{t('Type_FollowGlobal')}</span>
                    )}
                    <ChevronDown
                      size={12}
                      className={cn('text-t3 transition-transform duration-200', open && 'rotate-180')}
                    />
                  </div>
                </button>
              </div>
              <AnimatePresence initial={false}>
                {open && (
                  <motion.div
                    className="overflow-hidden"
                    initial={reduced ? false : { height: 0, opacity: 0 }}
                    animate={{ height: 'auto', opacity: 1 }}
                    exit={reduced ? { opacity: 0 } : { height: 0, opacity: 0 }}
                    transition={{ duration: 0.16, ease: [0.33, 1, 0.68, 1] }}
                  >
                    <div className="space-y-2.5 px-1.5 pb-2 pt-1.5">
                      {/* 形状 — AutoGlyph anchor first, then the shared shape chips. */}
                      <div>
                        <p className="mb-1.5 text-[11px] text-t2">{t('Axis_Shape')}</p>
                        <SwatchPicker
                          options={[
                            {
                              key: 'auto',
                              title: t('Type_FollowGlobal'),
                              selected: patch.shape === undefined,
                              onPick: () => patchType(b, { shape: undefined }),
                              onHover: tryType(b, { shape: undefined }),
                              glyph: <AutoGlyph selected={patch.shape === undefined} />,
                            },
                            ...ALL_SHAPES.map((o) => ({
                              key: o.value,
                              title: t(o.key),
                              selected: patch.shape === o.value,
                              onPick: () => patchType(b, { shape: o.value }),
                              onHover: tryType(b, { shape: o.value }),
                              glyph: <ShapeSwatch shape={o.value} active={patch.shape === o.value} />,
                            })),
                          ]}
                        />
                      </div>
                      {/* 颜色 — the global Colour row minus Original (DRY law):
                          AutoGlyph anchor, 满彩, 黑白, mono pair-dots, custom wheel. */}
                      <div>
                        <p className="mb-1.5 text-[11px] text-t2">{t('Subject_Label')}</p>
                        <div className="flex flex-wrap items-center gap-1">
                          {/* Order (owner correction 2026-07-12): 继承 dashed-circle
                              FIRST, then the 系统默认 ⊘, then the specific styles. */}
                          <SwatchButton
                            title={t('Type_FollowGlobal')}
                            selected={patch.subject === undefined}
                            onHover={tryType(b, { subject: undefined, tint: undefined })}
                            onClick={() => patchType(b, { subject: undefined, tint: undefined })}
                          >
                            <AutoGlyph selected={patch.subject === undefined} />
                          </SwatchButton>
                          {/* ⊘ 系统默认 = 原彩: the type keeps its own colours
                              regardless of the global subject (mirrors the main
                              Subject row's ⊘). */}
                          <SwatchButton
                            title={t('Subject_Orig')}
                            selected={patch.subject === 'Original'}
                            onHover={tryType(b, { subject: 'Original', tint: undefined })}
                            onClick={() => patchType(b, { subject: 'Original', tint: undefined })}
                          >
                            <NoneGlyph active={patch.subject === 'Original'} />
                          </SwatchButton>
                          <SwatchButton
                            title={t('Color_Bw')}
                            selected={patch.subject === 'BlackWhite'}
                            onHover={tryType(b, { subject: 'BlackWhite', tint: undefined })}
                            onClick={() => patchType(b, { subject: 'BlackWhite', tint: undefined })}
                          >
                            <BwGlyph />
                          </SwatchButton>
                          {monoSwatches.map((sw) => (
                            <SwatchButton
                              key={sw}
                              title={sw}
                              selected={patch.subject === 'Mono' && patch.tint?.toUpperCase() === sw.toUpperCase()}
                              onHover={tryType(b, { subject: 'Mono', tint: sw })}
                              onClick={() => patchType(b, { subject: 'Mono', tint: sw })}
                            >
                              <PairDot fg={sw} bg={paleOf(sw)} />
                            </SwatchButton>
                          ))}
                          <Popover>
                            <PopoverTrigger asChild>
                              <button
                                type="button"
                                title={t('Palette_Button')}
                                aria-label={t('Palette_Button')}
                                className={swatchButtonClass(monoCustom)}
                              >
                                <WheelRing size={20} value={patch.tint ?? config.tint} active={monoCustom} />
                              </button>
                            </PopoverTrigger>
                            <PopoverContent side="left" align="start" className="w-auto rounded-[14px] p-3">
                              <ColorPickerPanel
                                value={patch.tint ?? config.tint}
                                onChange={(hex) => patchType(b, { subject: 'Mono', tint: hex })}
                                wallpaperSwatches={palette ?? []}
                                quickSwatches={QUICK_SWATCHES}
                              />
                            </PopoverContent>
                          </Popover>
                        </div>
                      </div>
                      {/* 底板 — ⊘ 系统默认 · 继承 · derived · white · low-sat chips. */}
                      <div>
                        <p className="mb-1.5 text-[11px] text-t2">{t('Type_Plate')}</p>
                        <div className="flex flex-wrap items-center gap-1">
                          {/* Order (owner correction 2026-07-12): 继承 dashed-circle
                              FIRST, then the 系统默认 ⊘, then the specific plates.
                              The ⊘ = 本色 (null + white): the per-type plate model
                              has no config distinct from 本色 for "no plate", so the
                              ⊘ IS that state (mirrors the main Plate row's ⊘). */}
                          <SwatchButton
                            title={t('Type_FollowGlobal')}
                            selected={patch.plateColor === undefined && patch.plateFallback === undefined}
                            onHover={tryType(b, { plateColor: undefined, plateFallback: undefined })}
                            onClick={() => patchType(b, { plateColor: undefined, plateFallback: undefined })}
                          >
                            <AutoGlyph selected={patch.plateColor === undefined && patch.plateFallback === undefined} />
                          </SwatchButton>
                          <SwatchButton
                            title={t('Plate_None')}
                            selected={patch.plateColor === null && patch.plateFallback === 'white'}
                            onHover={tryType(b, { plateColor: null, plateFallback: 'white' })}
                            onClick={() => patchType(b, { plateColor: null, plateFallback: 'white' })}
                          >
                            <NoneGlyph active={patch.plateColor === null && patch.plateFallback === 'white'} />
                          </SwatchButton>
                          <SwatchButton
                            title={t('Plate_Auto')}
                            selected={patch.plateColor === null && patch.plateFallback !== 'white'}
                            onHover={tryType(b, { plateColor: null, plateFallback: 'derived' })}
                            onClick={() => patchType(b, { plateColor: null, plateFallback: 'derived' })}
                          >
                            <QuadPlateGlyph />
                          </SwatchButton>
                          <SwatchButton
                            title={t('Plate_White')}
                            selected={patch.plateColor?.toUpperCase() === '#FFFFFF'}
                            onHover={tryType(b, { plateColor: '#FFFFFF', plateFallback: undefined })}
                            onClick={() => patchType(b, { plateColor: '#FFFFFF', plateFallback: undefined })}
                          >
                            <PairDot fg="#FFFFFF" bg="#FFFFFF" />
                          </SwatchButton>
                          {TYPE_PLATE_SWATCHES.map((hex) => (
                            <SwatchButton
                              key={hex}
                              title={hex}
                              selected={patch.plateColor?.toUpperCase() === hex.toUpperCase()}
                              onHover={tryType(b, { plateColor: hex })}
                              onClick={() => patchType(b, { plateColor: hex })}
                            >
                              <span className="block size-5 rounded-md ring-1 ring-hair" style={{ background: hex }} />
                            </SwatchButton>
                          ))}
                          {/* Custom plate colour — the free wheel, matching the main plate row
                              and the per-type subject wheel (owner 2026-07-14). */}
                          <Popover>
                            <PopoverTrigger asChild>
                              <button
                                type="button"
                                title={t('Palette_Button')}
                                aria-label={t('Palette_Button')}
                                className={swatchButtonClass(plateCustom)}
                              >
                                <WheelRing size={20} value={patch.plateColor ?? config.plateColor ?? '#FFFFFF'} active={plateCustom} />
                              </button>
                            </PopoverTrigger>
                            <PopoverContent side="left" align="start" className="w-auto rounded-[14px] p-3">
                              <ColorPickerPanel
                                value={patch.plateColor ?? config.plateColor ?? '#FFFFFF'}
                                onChange={(hex) => patchType(b, { plateColor: hex })}
                                wallpaperSwatches={palette ?? []}
                                quickSwatches={QUICK_SWATCHES}
                              />
                            </PopoverContent>
                          </Popover>
                        </div>
                      </div>
                      {custom && (
                        <div className="flex justify-end">
                          <button
                            type="button"
                            onClick={() => setTypeOverride(b, null)}
                            className="text-[11px] text-coral-ink transition-colors hover:underline"
                          >
                            ↺ {t('Type_ResetFollow')}
                          </button>
                        </div>
                      )}
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          )
        })}
        {/* ADR-0024 SURFACES group — the second participation axis in the SAME
            row grammar, with live scan counts. The desktop row is a disabled
            always-on anchor so "why can't I turn the desktop off?" answers
            itself; each leg flips ONE surface (one undo step) and disabling a
            surface auto-restores it on the next apply (RESTORE-FIRST). */}
        <div className="py-0.5">
          <p className="px-1.5 pb-1 pt-1 text-[10.5px] text-t3">{t('Axis_Surface')}</p>
          <div className="-mx-1.5 flex items-center gap-2 rounded-lg px-1.5 py-1.5 opacity-60" title={t('Surface_DesktopTip')}>
            <span aria-hidden className="shrink-0">
              <span className="flex size-3.5 items-center justify-center rounded-[4px] border border-hair-strong bg-raised">
                <Check size={9} strokeWidth={3} />
              </span>
            </span>
            <Monitor size={17} className="shrink-0 text-t2" />
            <span className="min-w-0 flex-1 truncate text-[11px] text-t1">{t('Surface_Desktop')}</span>
            <span className="shrink-0 text-[10px] text-t3">{t('Surface_Always')}</span>
          </div>
          {SURFACE_LEGS.map((leg) => {
            const on = surfaces[leg]
            const { label, Icon } = SURFACE_LEG_META[leg]
            const name = t(label)
            return (
              <div key={leg} className="-mx-1.5 flex items-center gap-2 rounded-lg px-1.5 py-1.5">
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={on}
                  aria-label={name}
                  title={name}
                  onClick={() => setSurface(leg, !on)}
                  className="shrink-0 active:scale-95"
                >
                  <span
                    className={cn(
                      'flex size-3.5 items-center justify-center rounded-[4px] border transition-colors',
                      on ? 'border-coral bg-coral text-cta-ink' : 'border-hair-strong bg-raised',
                    )}
                  >
                    {on && <Check size={9} strokeWidth={3} />}
                  </span>
                </button>
                <Icon size={17} className={cn('shrink-0', on ? 'text-t2' : 'text-t3')} />
                <span className={cn('min-w-0 flex-1 truncate text-[11px]', on ? 'text-t1' : 'text-t3 line-through')}>
                  {name}
                </span>
                <span className="shrink-0 text-[10px] text-t3">{surfaceCounts[leg]}</span>
              </div>
            )
          })}
        </div>
      </div>
    </PropertyRow>
  )
}
