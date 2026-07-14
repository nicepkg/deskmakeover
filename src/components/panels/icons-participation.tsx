import * as React from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { Check, ChevronDown, RotateCcw } from 'lucide-react'
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
import type { IconKindBucket, TypePatch } from '@/bridge/types'
import { format, useT } from '@/lib/i18n'

// Participation surfaces (chief-UI/UX + owner 2026-07-09), both fed by one
// store state — no priority matrix:
//   · KeptBar   = the per-icon 「保留原样」 ledger (was the ugly exception row):
//     a pin-dot + inset chip, ghost clear, click to expand a per-item cancel list.
//   · KindTypeSection = the PERSISTENT per-bucket policy (kindPolicy), rendered as
//     the 5th axis row (chief-UI/UX + owner 2026-07-09): four glyph tiles in the
//     same PropertyRow grammar as the styling axes, but with a checkbox corner so
//     they read as MULTI-select, never pick-one. Always visible — even a count-0
//     bucket toggles, so a user can pre-opt-out a kind the desktop has none of yet.

/** The per-icon keep ledger — soft inset chip, pin dot, expandable cancel list. */
export function KeptBar() {
  const t = useT()
  // Select the STABLE items ref (a new-array selector makes zustand's
  // getSnapshot change every render → infinite loop); filter in the body.
  const items = useIcons((s) => s.items)
  const kept = React.useMemo(() => items.filter((i) => i.overrideMode !== null), [items])
  const { setOverride, clearOverrides } = useIcons.getState()
  const [open, setOpen] = React.useState(false)
  const reduced = useReducedMotion()
  if (kept.length === 0) return null

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

/** The TYPE ACCORDION v2 (ADR-0017 D5; chief-designer control spec
 *  2026-07-10) — every option is the SAME SwatchButton chip grammar as the
 *  global axes, nothing invented: each sub-axis leads with an AutoGlyph
 *  (跟随全局 anchor), the colour row IS the global Colour row minus
 *  Original, the plate row is filled colour chips. Row headers are standard
 *  list rows (beautify checkbox · glyph · name · status badge · chevron),
 *  separated by hair lines — no heavy coral frames. One row open at a time;
 *  the open row scope-highlights its icons on the canvas (dims the rest). */
export function KindTypeSection() {
  const t = useT()
  const items = useIcons((s) => s.items)
  const policy = useIcons((s) => s.state?.kindPolicy)
  const config = useIcons((s) => s.state?.config)
  const rawMonoSwatches = useIcons((s) => s.state?.monoSwatches)
  // Owner 2026-07-09: pure black/white mono dots are redundant with the 黑白
  // stop — filtered here EXACTLY like the global Subject row (§6.9 sameness).
  const monoSwatches = React.useMemo(
    () => (rawMonoSwatches ?? []).filter((v) => !['#FFFFFF', '#141414'].includes(v.toUpperCase())),
    [rawMonoSwatches],
  )
  const palette = useIcons((s) => s.state?.palette)
  const typeOverrides = useIcons((s) => s.state?.typeOverrides)
  const { setKindPolicy, setTypeOverride, setEditingBucket, resetTypeOverrides } = useIcons.getState()
  const [openBucket, setOpenBucket] = React.useState<IconKindBucket | null>(null)
  const reduced = useReducedMotion()

  // Hooks must run unconditionally (before any early return).
  const counts = React.useMemo(() => {
    const c = { App: 0, Folder: 0, File: 0, System: 0 }
    for (const i of items) {
      const b = kindBucket(i.kind)
      if (b) c[b]++
    }
    return c
  }, [items])

  // Scope feedback follows the open row; never leak a stale scope on unmount.
  React.useEffect(() => {
    setEditingBucket(openBucket)
    return () => setEditingBucket(null)
  }, [openBucket, setEditingBucket])

  if (!policy || !config) return null

  const patchType = (bucket: IconKindBucket, change: Partial<Record<keyof TypePatch, TypePatch[keyof TypePatch] | undefined>>) => {
    const current: TypePatch = { ...(typeOverrides?.[bucket]?.patch ?? {}) }
    for (const [k, v] of Object.entries(change)) {
      if (v === undefined) delete current[k as keyof TypePatch]
      else (current as Record<string, unknown>)[k] = v
    }
    setTypeOverride(bucket, { source: 'custom', patch: current })
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
                              glyph: <AutoGlyph selected={patch.shape === undefined} />,
                            },
                            ...ALL_SHAPES.map((o) => ({
                              key: o.value,
                              title: t(o.key),
                              selected: patch.shape === o.value,
                              onPick: () => patchType(b, { shape: o.value }),
                              glyph: <ShapeSwatch shape={o.value} active={patch.shape === o.value} />,
                            })),
                          ]}
                        />
                      </div>
                      {/* 颜色 — the global Colour row minus Original (DRY law):
                          AutoGlyph anchor, 满彩, 黑白, mono pair-dots, custom wheel. */}
                      {true && (
                        <div>
                          <p className="mb-1.5 text-[11px] text-t2">{t('Subject_Label')}</p>
                          <div className="flex flex-wrap items-center gap-1">
                            {/* Order (owner correction 2026-07-12): 继承 dashed-circle
                                FIRST, then the 系统默认 ⊘, then the specific styles. */}
                            <SwatchButton
                              title={t('Type_FollowGlobal')}
                              selected={patch.subject === undefined}
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
                              onClick={() => patchType(b, { subject: 'Original', tint: undefined })}
                            >
                              <NoneGlyph active={patch.subject === 'Original'} />
                            </SwatchButton>
                            <SwatchButton
                              title={t('Color_Bw')}
                              selected={patch.subject === 'BlackWhite'}
                              onClick={() => patchType(b, { subject: 'BlackWhite', tint: undefined })}
                            >
                              <BwGlyph />
                            </SwatchButton>
                            {monoSwatches.map((sw) => (
                              <SwatchButton
                                key={sw}
                                title={sw}
                                selected={patch.subject === 'Mono' && patch.tint?.toUpperCase() === sw.toUpperCase()}
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
                      )}
                      {/* 底板 — ⊘ 系统默认 · 继承 · derived · white · low-sat chips. */}
                      {true && (
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
                              onClick={() => patchType(b, { plateColor: undefined, plateFallback: undefined })}
                            >
                              <AutoGlyph selected={patch.plateColor === undefined && patch.plateFallback === undefined} />
                            </SwatchButton>
                            <SwatchButton
                              title={t('Plate_None')}
                              selected={patch.plateColor === null && patch.plateFallback === 'white'}
                              onClick={() => patchType(b, { plateColor: null, plateFallback: 'white' })}
                            >
                              <NoneGlyph active={patch.plateColor === null && patch.plateFallback === 'white'} />
                            </SwatchButton>
                            <SwatchButton
                              title={t('Plate_Auto')}
                              selected={patch.plateColor === null && patch.plateFallback !== 'white'}
                              onClick={() => patchType(b, { plateColor: null, plateFallback: 'derived' })}
                            >
                              <QuadPlateGlyph />
                            </SwatchButton>
                            <SwatchButton
                              title={t('Plate_White')}
                              selected={patch.plateColor?.toUpperCase() === '#FFFFFF'}
                              onClick={() => patchType(b, { plateColor: '#FFFFFF', plateFallback: undefined })}
                            >
                              <PairDot fg="#FFFFFF" bg="#FFFFFF" />
                            </SwatchButton>
                            {TYPE_PLATE_SWATCHES.map((hex) => (
                              <SwatchButton
                                key={hex}
                                title={hex}
                                selected={patch.plateColor?.toUpperCase() === hex.toUpperCase()}
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
                      )}
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
      </div>
    </PropertyRow>
  )
}
