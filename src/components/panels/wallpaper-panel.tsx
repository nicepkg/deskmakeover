import * as React from 'react'
import { CtaButton } from '@/components/common/cta-button'
import { Confetti, useCelebration } from '@/components/common/confetti'
import { ConfirmSheet, DoneCard } from '@/components/common/ceremony'
import { usePaperHero } from '@/lib/hero'
import { DmSlider } from '@/components/common/dm-slider'
import { IconAction, InspectorCard, PropertyRow, Reveal, useFooterClearance } from '@/components/common/inspector'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { ArchiveRestore, ChevronDown, Download, ImageUp, Layers, MousePointerClick, Plus, RotateCcw, X } from 'lucide-react'
import { Segmented } from '@/components/common/segmented'
import { ToggleSwitch } from '@/components/common/toggle-switch'
import { ZoneList } from '@/components/panels/wallpaper-zone-list'
import { EmojiPicker, FontPopover, MATERIALS, MATERIAL_KEYS, MaterialSwatch, PresetPopover, TITLE_STYLE_KEYS, TitleStyleSwatch } from '@/components/panels/wallpaper-panel-popovers'
import { WallpaperDimCard } from '@/components/panels/wallpaper-dim-card'
import { WallpaperScreenNotices } from '@/components/panels/wallpaper-screen-notices'
import { ColorSwatchDot, PalettePopover } from '@/components/common/color-controls'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { call } from '@/bridge/client'
import { useToasts } from '@/stores/toasts'
import { useWallpaper, makeZone } from '@/stores/wallpaper'
import {
  ACCENT_PALETTE,
  CORNER_MAX,
  CORNER_MIN,
  MATERIAL_RADIUS_DEFAULT,
  MATERIAL_TITLE_DEFAULT,
  allowedTitleStyles,
  resolveAccent,
} from '@/compositor/material'
import { ZONE_PRESETS, projectPreset } from '@/lib/zone-presets'
import { activeScreenFacts } from '@/lib/screen-arrange'
import { firstFreeArea } from '@/lib/zone-math'
import { format, useT } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'
import type { TitleSize, ZoneTone } from '@/bridge/types'
import { cn } from '@/lib/utils'

// The 壁纸 INSPECTOR (spec 04 v2.0, ADR-0014 + round-2 style sets): 壁纸压暗
// leads (the beauty story), 分区 follows as the power capability. The edit-scope
// law is per-zone: every property edits the SELECTED zone under a visible
// 正在编辑 header; mass edit is one explicit 应用到全部分区. Style axes surface
// first (材质 · 强调色 · 标题样式); granular dials live in the 高级 fold.


export function WallpaperPanel() {
  const t = useT()
  const { phase, statusText, ctaText } = usePaperHero()
  const state = useWallpaper((s) => s.state)
  const look = useWallpaper((s) => s.look)
  const selected = useWallpaper((s) => s.selected)
  const fonts = useWallpaper((s) => s.fonts)
  const sourceName = useWallpaper((s) => s.sourceName)
  const sourceUrl = useWallpaper((s) => s.sourceUrl)
  const {
    mutateZone, addZone, removeZone, select, applyToAllZones, replaceZones,
    apply, restore, loadFonts, importSourceViaPicker, resetSource, exportImage,
  } = useWallpaper.getState()
  const [advZone, setAdvZone] = React.useState(false)
  const [fontOpen, setFontOpen] = React.useState(false)
  const [presetPick, setPresetPick] = React.useState<string | null>(null)
  const [restoreOpen, setRestoreOpen] = React.useState(false)
  const [doneOpen, setDoneOpen] = React.useState(false)
  // First destructive apply over a live (slideshow/dynamic) wallpaper is confirmed
  // once, then remembered per screen for the session (§A4).
  const [dynamicConfirmOpen, setDynamicConfirmOpen] = React.useState(false)
  const [confirmedScreens, setConfirmedScreens] = React.useState<Set<string>>(new Set())
  const { footerRef, clearance } = useFooterClearance()
  const reduced = useReducedMotion()
  const { celebrateKey, celebrate } = useCelebration('wallpaper')

  // Active-screen facts for the CTA rename + dynamic-apply gate (§B5/A4). The
  // per-screen header + dynamic banners render from the same facts in
  // WallpaperScreenNotices. A single-monitor host yields multiScreen === false.
  const { activeIndex, multiScreen, liveWallpaper } = activeScreenFacts(state)
  const needsDynamicConfirm = !!state && liveWallpaper && !confirmedScreens.has(state.activeScreenId)
  // The CTA names its target so an apply can never silently hit the wrong monitor
  // (§B5). Only the actionable phases rename; working/synced keep their own copy.
  const ctaLabel =
    multiScreen && activeIndex >= 0 && (phase === 'ready' || phase === 'dirty')
      ? format(t('Paper_Cta_ApplyScreen'), activeIndex + 1)
      : ctaText

  const doApply = async () => {
    // Gate the DoneCard on THIS apply's result — hasBackup stays true from any
    // earlier success and would celebrate a failed apply (codex review M6).
    const ok = await apply()
    if (!ok) return
    // Same celebration as the icons apply (DRY): confetti on the FIRST successful
    // wallpaper apply of each launch, then the DoneCard.
    celebrate()
    setDoneOpen(true)
  }

  const runApply = async () => {
    if (needsDynamicConfirm) {
      setDynamicConfirmOpen(true)
      return
    }
    await doApply()
  }

  // 导出图片 never touches the desktop — it saves the composed PNG locally.
  const runExport = async () => {
    const filename = await exportImage()
    const show = useToasts.getState().show
    if (filename) show(format(t('Toast_PaperExported'), filename), 'success')
    else show(t('Toast_PaperExportFailed'), 'warn')
  }

  if (!state || !look) return <aside className="w-[280px] shrink-0" />

  const zone = selected !== null ? look.zones.find((z) => z.id === selected) : undefined
  const zoneIndex = zone ? look.zones.findIndex((z) => z.id === zone.id) : -1
  const opacityPercent = Math.round((zone?.fillOpacity ?? (zone?.material === 'Outline' ? 0.05 : 0.6)) * 100)

  const addDefaultZone = () => {
    const rect = firstFreeArea(state.grid, look.zones, 6, 4)
    const created = makeZone({ ...rect, title: t('Zone_DefaultTitle') })
    addZone(created)
  }

  const applyPreset = (presetId: string) => {
    const preset = ZONE_PRESETS.find((p) => p.id === presetId)
    if (!preset) return
    replaceZones(projectPreset(preset, state.grid))
  }

  const requestPreset = (presetId: string) => {
    if (look.zones.length > 0) setPresetPick(presetId)
    else applyPreset(presetId)
  }

  // The Zones verbs (预设布局 · 添加) — inline when the list is empty, header
  // cluster once zones exist.
  const zoneActions = (
    <span className="flex shrink-0 gap-1">
      <PresetPopover wallpaperUrl={sourceUrl ?? state.originalUrl} onPick={requestPreset} />
      <IconAction title={t('Paper_AddZone')} onClick={addDefaultZone}>
        <Plus size={14} />
      </IconAction>
    </span>
  )

  const applyAll = () => {
    if (!zone) return
    applyToAllZones({
      tone: zone.tone,
      material: zone.material,
      titleStyle: zone.titleStyle,
      shadow: zone.shadow,
      fillOpacity: zone.fillOpacity,
      cornerRadius: zone.cornerRadius,
      titleSize: zone.titleSize,
      fontFamily: zone.fontFamily,
    })
  }

  const fontDisplay = zone?.fontFamily
    ? fonts.find((f) => f.family === zone.fontFamily)?.display ?? zone.fontFamily
    : t('TitleFont_Default')

  return (
    <aside className="relative flex w-[280px] shrink-0 flex-col gap-2.5 pl-1 pr-3 pt-1">
      <div style={{ paddingBottom: clearance }} className="scrollbar-none flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto [&>*]:shrink-0">
      {/* Session bar: source-in lives here (IA 2026-07-09). Default = status
          text + quiet import entry; imported = source chip + cancel. Only the
          NON-default state gets labelled. Lives INSIDE the scroller — it rides
          along with the panel (owner call 2026-07-09), never floats. */}
      <div className="flex items-center gap-1 px-0.5">
        {sourceName === null ? (
          <p className="min-w-0 flex-1 truncate text-[11px] text-t3/90">{statusText}</p>
        ) : (
          <button
            type="button"
            title={t('Paper_Import_Tip')}
            onClick={() => importSourceViaPicker()}
            className="flex min-w-0 flex-1 items-center gap-1.5 rounded-[7px] px-1 py-0.5 text-left transition-colors hover:bg-raised-hov"
          >
            {sourceUrl && <img src={sourceUrl} alt="" className="size-4 shrink-0 rounded-[4px] object-cover" />}
            <span className="min-w-0 truncate text-[11px] text-t2">{t('Paper_SourceImported')}</span>
          </button>
        )}
        {sourceName === null ? (
          <button
            type="button"
            aria-label={t('Paper_Import')}
            title={t('Paper_Import_Tip')}
            onClick={() => importSourceViaPicker()}
            className="flex size-4 shrink-0 items-center justify-center rounded-full text-t3/80 ring-1 ring-hair transition-colors hover:bg-raised-hov hover:text-t1"
          >
            <ImageUp size={10} />
          </button>
        ) : (
          <button
            type="button"
            aria-label={t('Paper_ImportCancel')}
            title={t('Paper_ImportCancel_Tip')}
            onClick={() => void resetSource()}
            className="flex size-4 shrink-0 items-center justify-center rounded-full text-t3/80 ring-1 ring-hair transition-colors hover:bg-raised-hov hover:text-t1"
          >
            <X size={10} />
          </button>
        )}
        <Popover>
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label={t('PaperHow_Title')}
              className="flex size-4 shrink-0 items-center justify-center rounded-full text-[10px] leading-none text-t3/80 ring-1 ring-hair transition-colors hover:bg-raised-hov hover:text-t1"
            >
              ?
            </button>
          </PopoverTrigger>
          <PopoverContent side="bottom" align="end" className="w-[252px] rounded-[12px] p-3">
            <p className="mb-2 text-[11px] font-medium text-t1">{t('PaperHow_Title')}</p>
            <div className="space-y-2">
              {(
                [
                  [Layers, 'PaperHow_Zones'],
                  [MousePointerClick, 'PaperHow_Icons'],
                  [ArchiveRestore, 'PaperHow_Backup'],
                ] as [typeof Layers, StringKey][]
              ).map(([Icon, key]) => (
                <div key={key} className="flex items-start gap-2">
                  <Icon size={12} className="mt-px shrink-0 text-t3" />
                  <p className="min-w-0 text-caption leading-snug text-t2">{t(key)}</p>
                </div>
              ))}
            </div>
          </PopoverContent>
        </Popover>
      </div>

        {/* Per-screen header + Span note + dynamic-wallpaper banners (§B5/A4/B6). */}
        <WallpaperScreenNotices />

        <Reveal show={state.fingerprintMismatch}>
          <button
            type="button"
            onClick={() => void call('wallpaper.getState').then((s) => useWallpaper.setState({ state: s }))}
            className="w-full rounded-[10px] bg-amber-wash px-3 py-2 text-left text-caption text-amber"
          >
            {t('Paper_Mismatch')} · {t('Paper_Regenerate')}
          </button>
        </Reveal>

        <WallpaperDimCard />

        {/* 分区 — the power capability */}
        <InspectorCard>
          <PropertyRow
            label={t('Paper_Zones')}
            inline={look.zones.length === 0}
            labelExtra={look.zones.length > 0 ? zoneActions : undefined}
          >
            {look.zones.length === 0 ? (
              zoneActions
            ) : (
              <ZoneList
                zones={look.zones}
                selected={selected}
                onSelect={select}
                onRename={(id, value) => mutateZone(id, (z) => ({ ...z, title: value }), 'title')}
                onDelete={removeZone}
              />
            )}
          </PropertyRow>

          {/* Edit scope: per-zone controls under a visible 正在编辑 header; with no
              selection they collapse to one hint line. */}
          {zone ? (
            /* ONE persistent block across zone switches (owner call 2026-07-09):
               remounting killed control-level motion. The block stays mounted;
               each control animates its own value change (segmented thumbs
               slide, toggle knobs travel, selection rings hand over) and only
               the header NAME crossfades — state morphs, nothing re-renders. */
            <motion.div
              key="zone-edit"
              initial={reduced ? false : { opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.15 }}
              className="divide-y divide-hair"
            >
              {/* Style axes surface first (IA §5): 材质 · 强调色 · 标题样式;
                  granular dials fold into ONE 高级 reveal (same grammar as
                  clarity's advanced fold) — the default card gets SHORTER while
                  exposing two new style axes. */}
              <PropertyRow
                label={
                  /* The animated name lives in the NORMAL label slot so its
                     rhythm matches the sibling headers (Accent / Title style);
                     popLayout lifts only the EXITING copy out of flow. */
                  <AnimatePresence mode="popLayout" initial={false}>
                    <motion.span
                      key={zone.id}
                      initial={reduced ? false : { opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                      transition={{ duration: 0.12, ease: [0.33, 1, 0.68, 1] }}
                      className="inline-block text-coral-ink"
                    >
                      {format(t('Zone_EditingHeader'), zone.title)}
                    </motion.span>
                  </AnimatePresence>
                }
              >
                <div className="flex items-center gap-1">
                  {MATERIALS.map((m) => (
                    <MaterialSwatch
                      key={m}
                      material={m}
                      title={t(MATERIAL_KEYS[m])}
                      selected={zone.material === m}
                      onClick={() =>
                        // Designer pairing: each material lands with its tuned
                        // title style + radius; both stay user-overridable after.
                        mutateZone(zone.id, (z) => ({
                          ...z,
                          material: m,
                          titleStyle: MATERIAL_TITLE_DEFAULT[m],
                          cornerRadius: MATERIAL_RADIUS_DEFAULT[m],
                        }))
                      }
                    />
                  ))}
                </div>
              </PropertyRow>

              <PropertyRow label={t('Zone_Accent')}>
                <div className="flex items-center gap-1.5">
                  {ACCENT_PALETTE.map((hex) => (
                    <ColorSwatchDot
                      key={hex}
                      color={hex}
                      selected={resolveAccent(zone, zoneIndex) === hex}
                      onClick={() => mutateZone(zone.id, (z) => ({ ...z, accent: hex }))}
                    />
                  ))}
                  <PalettePopover
                    value={zone.accent ?? ACCENT_PALETTE[0]}
                    active={!!zone.accent && !ACCENT_PALETTE.includes(zone.accent as (typeof ACCENT_PALETTE)[number])}
                    wallpaper={[state.wallTint]}
                    onPick={(hex) => mutateZone(zone.id, (z) => ({ ...z, accent: hex }))}
                  />
                </div>
              </PropertyRow>

              <PropertyRow
                label={t('Zone_TitleStyle')}
                sub={
                  <div className="mt-2 flex items-center justify-between gap-2">
                    <EmojiPicker
                      value={zone.emoji}
                      noneLabel={t('Zone_EmojiNone')}
                      onPick={(emoji) => mutateZone(zone.id, (z) => ({ ...z, emoji }))}
                    />
                    <Segmented
                      size="sm"
                      value={zone.titleSize}
                      options={(
                        [
                          ['Size_Small', 'S'],
                          ['Size_Mid', 'M'],
                          ['Size_Big', 'L'],
                        ] as [StringKey, TitleSize][]
                      ).map(([key, value]) => ({ value, label: t(key) }))}
                      onChange={(titleSize) => mutateZone(zone.id, (z) => ({ ...z, titleSize }))}
                    />
                  </div>
                }
              >
                <div className="flex items-center gap-1">
                  {allowedTitleStyles(zone.material).map((style) => (
                    <TitleStyleSwatch
                      key={style}
                      style={style}
                      title={t(TITLE_STYLE_KEYS[style])}
                      selected={zone.titleStyle === style}
                      onClick={() => mutateZone(zone.id, (z) => ({ ...z, titleStyle: style }))}
                    />
                  ))}
                </div>
              </PropertyRow>

              {/* The WHOLE row is the fold trigger (owner call 2026-07-09) —
                  a label with only its chevron clickable is an interaction
                  dead zone. */}
              <div className="px-3 py-2">
                <button
                  type="button"
                  onClick={() => setAdvZone((v) => !v)}
                  aria-expanded={advZone}
                  className={cn(
                    'flex min-h-7 w-full items-center justify-between gap-2 rounded-md transition-colors',
                    advZone ? 'text-coral-ink' : 'text-t2 hover:text-t1',
                  )}
                >
                  <span className="text-[11px]">{t('Paper_Advanced')}</span>
                  <ChevronDown
                    size={12}
                    className={cn('shrink-0 transition-transform duration-150', advZone ? 'rotate-180' : 'text-t3')}
                  />
                </button>
                <Reveal show={advZone}>
                    <div className="space-y-2.5 pt-2">
                      <div className="flex items-center justify-between gap-2">
                        <Segmented
                          size="sm"
                          value={zone.tone}
                          options={(
                            [
                              ['Zone_ToneAuto', 'Auto'],
                              ['Zone_ToneLight', 'Light'],
                              ['Zone_ToneDark', 'Dark'],
                            ] as [StringKey, ZoneTone][]
                          ).map(([key, value]) => ({ value, label: t(key) }))}
                          onChange={(tone) => mutateZone(zone.id, (z) => ({ ...z, tone }))}
                        />
                      </div>
                      <div>
                        <p className="mb-1 text-caption text-t3">{`${t('Paper_FillOpacity')} · ${opacityPercent}%`}</p>
                        <DmSlider
                          value={opacityPercent}
                          min={3}
                          max={95}
                          onChange={(v) => mutateZone(zone.id, (z) => ({ ...z, fillOpacity: v / 100 }), 'opacity')}
                          aria-label={t('Paper_FillOpacity')}
                        />
                      </div>
                      <div>
                        <p className="mb-1 text-caption text-t3">{`${t('Paper_Corner')} · ${Math.round(zone.cornerRadius)}px`}</p>
                        <DmSlider
                          value={Math.round(zone.cornerRadius)}
                          min={CORNER_MIN}
                          max={CORNER_MAX}
                          onChange={(v) => mutateZone(zone.id, (z) => ({ ...z, cornerRadius: v }), 'corner')}
                          aria-label={t('Paper_Corner')}
                        />
                      </div>
                      {(zone.material === 'Frost' || zone.material === 'Luminous' || zone.material === 'Solid') && (
                        <div className="flex items-center justify-between">
                          <span className="text-caption text-t3">{t('Zone_Shadow')}</span>
                          <ToggleSwitch
                            checked={zone.shadow}
                            onChange={(shadow) => mutateZone(zone.id, (z) => ({ ...z, shadow }))}
                            label={t('Zone_Shadow')}
                          />
                        </div>
                      )}
                      <FontPopover
                        open={fontOpen}
                        setOpen={setFontOpen}
                        fonts={fonts}
                        current={zone.fontFamily}
                        display={fontDisplay === '__bundled__' ? t('TitleFont_Default') : fontDisplay}
                        onLoad={() => void loadFonts()}
                        onPick={(family) => {
                          mutateZone(zone.id, (z) => ({ ...z, fontFamily: family }))
                          setFontOpen(false)
                        }}
                      />
                    </div>
                </Reveal>
              </div>

              {look.zones.length > 1 && (
                <div className="px-0 pt-2.5">
                  <button
                    type="button"
                    onClick={applyAll}
                    className="w-full rounded-[9px] bg-chip py-1.5 text-caption text-t2 transition-colors hover:bg-raised-hov hover:text-t1"
                  >
                    {t('Zone_ApplyAll')}
                  </button>
                </div>
              )}
            </motion.div>
          ) : (
            look.zones.length > 0 && (
              <motion.div
                key="zone-hint"
                initial={reduced ? false : { opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ duration: 0.15 }}
              >
                <PropertyRow label={t('Zone_EditingNone')}>
                  <p className="text-caption text-t3">{t('Zone_SelectHint')}</p>
                </PropertyRow>
              </motion.div>
            )
          )}
        </InspectorCard>
      </div>

      <div
        ref={footerRef}
        className="absolute inset-x-0 bottom-0 z-20 bg-gradient-to-t from-background from-65% via-background/55 to-transparent"
      >
        <div className="flex flex-col gap-1.5 pb-3 pl-1 pr-3 pt-5">
          {/* Secondary link slot: result-out actions (restore / export) — quiet
              peers beside the sacred CTA, shown only when meaningful. */}
          <Reveal show={state.hasBackup || state.dirty}>
            <div className="flex items-center gap-1">
              {state.hasBackup && (
                <IconAction title={t('Paper_Restore')} onClick={() => setRestoreOpen(true)}>
                  <RotateCcw size={11} />
                  {t('Paper_Restore')}
                </IconAction>
              )}
              {state.dirty && (
                <IconAction title={t('Paper_Export_Tip')} onClick={() => void runExport()}>
                  <Download size={11} />
                  {t('Paper_Export')}
                </IconAction>
              )}
            </div>
          </Reveal>
          <CtaButton phase={phase} onClick={() => void runApply()}>
            {ctaLabel}
          </CtaButton>
        </div>
      </div>

      <ConfirmSheet
        open={presetPick !== null}
        title={t('Paper_ReplaceConfirm')}
        confirmLabel={t('Preset_Apply')}
        cancelLabel={t('ConsentCancel')}
        onConfirm={() => {
          const id = presetPick
          setPresetPick(null)
          if (id) applyPreset(id)
        }}
        onCancel={() => setPresetPick(null)}
      />
      <ConfirmSheet
        open={restoreOpen}
        title={t('RestoreConfirm')}
        confirmLabel={t('Paper_Restore')}
        cancelLabel={t('ConsentCancel')}
        destructive
        onConfirm={() => {
          setRestoreOpen(false)
          void restore()
        }}
        onCancel={() => setRestoreOpen(false)}
      />
      <ConfirmSheet
        open={dynamicConfirmOpen}
        title={t('Paper_DynamicReplaceConfirm')}
        confirmLabel={t('Paper_DynamicReplaceCta')}
        cancelLabel={t('ConsentCancel')}
        destructive
        onConfirm={() => {
          setDynamicConfirmOpen(false)
          if (state) setConfirmedScreens((prev) => new Set(prev).add(state.activeScreenId))
          void doApply()
        }}
        onCancel={() => setDynamicConfirmOpen(false)}
      />
      <DoneCard
        open={doneOpen}
        onClose={() => setDoneOpen(false)}
        note={t('Done_LastStep')}
        ctaLabel={t('Done_GoOrganize')}
      />

      {/* First-of-launch confetti — same shared celebration as the icons apply. */}
      <Confetti fireKey={celebrateKey} />
    </aside>
  )
}
