import * as React from 'react'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { ChevronDown, Plus } from 'lucide-react'
import { ConfirmSheet } from '@/components/common/ceremony'
import { DmSlider } from '@/components/common/dm-slider'
import { IconAction, InspectorCard, PropertyRow, Reveal } from '@/components/common/inspector'
import { Segmented } from '@/components/common/segmented'
import { ToggleSwitch } from '@/components/common/toggle-switch'
import { ZoneList } from '@/components/panels/wallpaper-zone-list'
import { FontPopover, MATERIALS, MATERIAL_KEYS, MaterialSwatch, PresetPopover, TITLE_STYLE_KEYS, TitleStyleSwatch } from '@/components/panels/wallpaper-panel-popovers'
import { ColorSwatchDot, PalettePopover } from '@/components/common/color-controls'
import { useWallpaper, makeZone } from '@/stores/wallpaper'
import {
  ACCENT_PALETTE,
  CORNER_MAX,
  CORNER_MIN,
  MATERIAL_RADIUS_DEFAULT,
  MATERIAL_TITLE_DEFAULT,
  OPACITY_DEFAULTS,
  allowedTitleStyles,
  resolveAccent,
} from '@/compositor/material'
import { ZONE_PRESETS, orientationOfGrid, projectPreset } from '@/lib/zone-presets'
import { firstFreeArea } from '@/lib/zone-math'
import { format, useT } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'
import type { TitleSize, ZoneTone } from '@/bridge/types'
import { cn } from '@/lib/utils'

// The 分区 (zone) inspector — the power capability of the wallpaper panel (spec 04
// v2.0, ADR-0014 + round-2 style sets). Split out of wallpaper-panel.tsx for the
// ≤500-line law; PURE STRUCTURE MOVE, zero behaviour change. The edit-scope law is
// per-zone: every property edits the SELECTED zone under a visible 正在编辑 header;
// mass edit is one explicit 应用到全部分区. Style axes surface first (材质 · 强调色 ·
// 标题样式); granular dials live in the 高级 fold. The preset-replace confirm rides
// with the PresetPopover it belongs to (the sheet is a fixed overlay, so its DOM
// position is irrelevant).

export function WallpaperZoneInspector() {
  const t = useT()
  const reduced = useReducedMotion()
  const state = useWallpaper((s) => s.state)
  const look = useWallpaper((s) => s.look)
  const selected = useWallpaper((s) => s.selected)
  const fonts = useWallpaper((s) => s.fonts)
  const sourceUrl = useWallpaper((s) => s.sourceUrl)
  const { mutateZone, addZone, removeZone, select, applyToAllZones, replaceZones, loadFonts } = useWallpaper.getState()
  const [advZone, setAdvZone] = React.useState(false)
  const [fontOpen, setFontOpen] = React.useState(false)
  const [presetPick, setPresetPick] = React.useState<string | null>(null)

  if (!state || !look) return null

  const zone = selected !== null ? look.zones.find((z) => z.id === selected) : undefined
  const zoneIndex = zone ? look.zones.findIndex((z) => z.id === zone.id) : -1
  // Display truth for the null (untouched) sentinel = the material's real
  // default, not a hardcoded 60% (glass defaults to 0 = pure refraction).
  const opacityPercent = zone
    ? Math.round((zone.fillOpacity ?? OPACITY_DEFAULTS[zone.material][zone.tone === 'Dark' ? 'Dark' : 'Light']) * 100)
    : 0

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
      <PresetPopover wallpaperUrl={sourceUrl ?? state.originalUrl} orientation={orientationOfGrid(state.grid)} onPick={requestPreset} />
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
    <>
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
              onEmoji={(id, emoji) => mutateZone(id, (z) => ({ ...z, emoji }))}
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
              <div className="flex flex-col gap-1.5">
              {/* gap-0.5: six ≥40px hit areas must fit the 280px inspector. */}
              <div className="flex items-center gap-0.5">
                {MATERIALS.map((m) => (
                  <MaterialSwatch
                    key={m}
                    material={m}
                    title={t(MATERIAL_KEYS[m])}
                    selected={zone.material === m}
                    wallpaperUrl={sourceUrl ?? state.originalUrl}
                    onClick={() =>
                      // Switch semantics (round 3, owner 2026-07-15): an axis the
                      // user never touched (still at the OUTGOING material's
                      // default; fillOpacity's untouched sentinel is null) adopts
                      // the new material's tuned default; a touched axis keeps
                      // the user's value — except an illegal titleStyle, which
                      // falls back to the new material's default.
                      mutateZone(zone.id, (z) => {
                        const keptTitle =
                          z.titleStyle !== MATERIAL_TITLE_DEFAULT[z.material] &&
                          allowedTitleStyles(m).includes(z.titleStyle)
                        return {
                          ...z,
                          material: m,
                          titleStyle: keptTitle ? z.titleStyle : MATERIAL_TITLE_DEFAULT[m],
                          cornerRadius:
                            z.cornerRadius === MATERIAL_RADIUS_DEFAULT[z.material]
                              ? MATERIAL_RADIUS_DEFAULT[m]
                              : z.cornerRadius,
                        }
                      })
                    }
                  />
                ))}
              </div>
              {/* Persistent name caption — the tile shows the LOOK, this names
                  it (识别优于回忆; hover tooltips alone hid the vocabulary). */}
              <p className="text-caption leading-none text-t3">{t(MATERIAL_KEYS[zone.material])}</p>
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
                /* Size only applies to a VISIBLE title — selecting 无 collapses
                   it (clear cause and effect). Emoji moved beside the title
                   text in the zone list (round 3 — one label, one place). */
                zone.titleStyle !== 'None' ? (
                  <div className="mt-2 flex items-center justify-end gap-2">
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
                ) : undefined
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
                        min={0}
                        max={100}
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
                        step={2}
                        onChange={(v) => mutateZone(zone.id, (z) => ({ ...z, cornerRadius: v }), 'corner')}
                        aria-label={t('Paper_Corner')}
                      />
                    </div>
                    {(zone.material === 'Frost' || zone.material === 'Fluted' || zone.material === 'Paper' || zone.material === 'Brushed' || zone.material === 'LiquidGlass') && (
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
    </>
  )
}
