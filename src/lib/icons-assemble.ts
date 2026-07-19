// The frontend's icon-state ASSEMBLY (D1-thin boundary, owner ruling 2026-07-12): Rust returns
// raw scan items + the persisted ②③/native bits; the FRONTEND owns presets, palette, swatches,
// the grid, activePresetId, and stitches them into the `IconsStateDto` the store + UI consume.
// This is the single assembly path both bridge backends feed (the real Tauri host and the browser
// mock), so mock and production render from ONE source of truth — the same move wallpaper made in
// A3f. Pure: no bridge, no session state; the store supplies the live draft + items.

import type {
  ConfigDto,
  GridDto,
  GridMetricsDto,
  HistoryEntryDto,
  IconItemDto,
  IconsStateDto,
  KindPolicy,
  LookVersionDto,
  PresetDto,
  TypeOverrides,
} from '@/bridge/types'
import { DEFAULT_KIND_POLICY } from '@/lib/kind-policy'
import { parseIconLook } from '@/lib/icon-look'
import { typeOverridesEqual } from '@/lib/type-config'

/** The three global knobs a saved appearance recipe carries (store ②③, spec 07 §8.2). */
export interface IconStyleRecipe {
  config: ConfigDto
  kindPolicy: KindPolicy
  typeOverrides: TypeOverrides
}

// ---- Preset collection v3 (OWNER-CURATED, hand-tuned exports 2026-07-16) ----
// The owner built these nine on the live canvas and exported them as .dmpreset
// packages; the values below are those recipes VERBATIM — do not "improve" them.
// Replaces the chief-designer v2 lineup (spectrum/stationery/glass/pebble/ink/
// white/ascast — retired, git remembers). Key order = card order.
// The 蓝图 mono ink #0F4F93 + 釉光 plates #DDE6F2 are desktop CONTENT the user
// picks, not app-chrome accent (banned-colors carries the reviewed exemption).
export const BASE_CONFIGS: Record<string, ConfigDto> = {
  squircle: { shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Ring', markColor: null, plateColor: null, size: 'Mid', filter: 'None' },
  porthole: { shape: 'Circle', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Ring', markColor: null, plateColor: null, size: 'Mid', filter: 'None' },
  pixel: { shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Comet', markColor: null, plateColor: null, size: 'Mid', filter: 'Pixel' },
  creek: { shape: 'Pebble', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Shadow', markColor: null, plateColor: null, size: 'Mid', filter: 'None' },
  scrapbook: { shape: 'Samsung', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Fold', markColor: null, plateColor: null, size: 'Mid', filter: 'Sticker' },
  gleam: { shape: 'None', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Comet', markColor: null, plateColor: null, size: 'Mid', filter: 'Glass' },
  diecut: { shape: 'None', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Comet', markColor: null, plateColor: null, size: 'Mid', filter: 'Sticker' },
  blueprint: { shape: 'Samsung', subject: 'Mono', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#0F4F93', distinction: 'Mark', markStyle: 'Shadow', markColor: null, plateColor: null, size: 'Mid', filter: 'None' },
  glaze: { shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Comet', markColor: '#FFFFFF', plateColor: null, size: 'Mid', filter: 'Gloss' },
}

// Per-preset type ladders (collection v3) — exactly as the owner exported them.
// diecut + blueprint deliberately ship NONE (the global look covers every type).
export const PRESET_TYPE_OVERRIDES: Record<string, TypeOverrides> = {
  squircle: {
    Folder: { source: 'custom', patch: { shape: 'Folder' } },
    File: { source: 'custom', patch: { shape: 'File' } },
  },
  porthole: {
    Folder: { source: 'custom', patch: { shape: 'Folder' } },
    File: { source: 'custom', patch: { shape: 'Apple' } },
  },
  pixel: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: '#E7E7E5' } },
    File: { source: 'custom', patch: { shape: 'File', plateColor: '#E7E7E5' } },
  },
  creek: {
    Folder: { source: 'custom', patch: { shape: 'Samsung', plateColor: null, plateFallback: 'derived' } },
    File: { source: 'custom', patch: { shape: 'Samsung' } },
  },
  scrapbook: {
    Folder: { source: 'custom', patch: { shape: 'Folder' } },
    File: { source: 'custom', patch: { shape: 'Pebble', plateColor: '#E7E7E5' } },
  },
  gleam: {
    Folder: { source: 'custom', patch: { shape: 'Folder' } },
    File: { source: 'custom', patch: { shape: 'File' } },
  },
  glaze: {
    Folder: { source: 'custom', patch: { plateColor: '#DDE6F2', plateFallback: 'derived' } },
    File: { source: 'custom', patch: { plateColor: '#DDE6F2' } },
  },
}

/** The factory default look (方圆/Squircle — the owner's flagship) — used when no saved-style (②) exists yet. */
export const DEFAULT_PRESET_ID = 'squircle'

/** The SYSTEM-DEFAULT baseline (owner order 2026-07-15): selecting 系统默认 RESETS the draft to
 *  THIS config — the values every panel row's ⊘ already advertises while the card is active
 *  (shape ⊘, 原彩, plate ⊘ = null+white, filter ⊘, native arrow Keep, uniform shortcut shape off).
 *  Before this, 系统默认 was only a display lens over the PRESERVED previous-preset draft, so the
 *  first follow-up edit resurrected that whole preset with one key changed — the panel's lit ⊘s
 *  were lying about the draft. Now draft and panel agree, and each subsequent edit moves exactly
 *  one axis. Latent axes (tint/monoStyle/plateBand/markStyle) keep factory values — they only take
 *  effect once their mode is picked. */
export const SYSTEM_DEFAULT_CONFIG: ConfigDto = {
  shape: 'None',
  subject: 'Original',
  plateBand: 'Vivid',
  plateFallback: 'white',
  shortcutShape: null,
  monoStyle: 'Tonal',
  tint: '#FF6F5E',
  distinction: 'Keep',
  markStyle: 'Shadow',
  markColor: null,
  plateColor: null,
  size: 'Mid',
  filter: 'None',
}

export const ICON_PALETTE = ['#B97D4E', '#8A5A33', '#E8C9A0', '#6E4526', '#F4E7D3']
export const MONO_SWATCHES = ['#FFFFFF', '#141414', '#B97D4E', '#FF6F5E', '#3FB6A8', '#D9A94E']
export const MARK_SWATCHES = ['#FFFFFF', '#141414', '#FF6F5E', '#B97D4E', '#3FB6A8']

// Desktop icon px is Small 32 · Mid 48 · Big 96 (C# DesktopIconSize.cs).
const ICON_PX: Record<ConfigDto['size'], number> = { Small: 32, Mid: 48, Big: 96 }
/** Fallback metrics before the first scan lands (the store always feeds real scan metrics after). */
const DEFAULT_METRICS: GridMetricsDto = {
  screenWidth: 1920,
  screenHeight: 1080,
  taskbarHeight: 48,
  cellWidth: null,
  cellHeight: null,
  iconPx: null,
}

/** Presets as DATA — the web renders each mini with the live renderer. */
export function iconPresets(): PresetDto[] {
  return Object.entries(BASE_CONFIGS).map(([id, config]) => ({
    id,
    config: { ...config },
    typeOverrides: structuredClone(PRESET_TYPE_OVERRIDES[id] ?? {}),
  }))
}

/** The desktop grid for an icon size, built from the OBSERVED platform metrics (D1: dims are
 *  platform truth from the scan). When the scan carries the TRUE snap-cell pitch + icon size
 *  (IFolderView GetSpacing/GetViewModeAndIconSize), the cell derives from those — a mirror tile
 *  centers its glyph inside `cellWidth`, so a fabricated wider cell shifted every icon right of
 *  where Windows draws it (owner report 2026-07-16: the preview's left padding read too large).
 *  For a size OTHER than the observed one we keep the observed ABSOLUTE gutter (`cell − icon`)
 *  and re-add it around the new icon — NOT a proportional scale: Windows does not promise the
 *  gutter scales with icon size, and a custom `IconSpacing` would break that assumption (codex
 *  P2). The `iconPx + 44/48` constants remain ONLY as the no-observation fallback (browser mock
 *  / failed shell walk). */
export function iconGrid(size: ConfigDto['size'], metrics: GridMetricsDto = DEFAULT_METRICS): GridDto {
  const iconPx = ICON_PX[size]
  const observed =
    metrics.cellWidth != null && metrics.cellHeight != null && metrics.iconPx != null && metrics.iconPx > 0
      ? { w: metrics.cellWidth, h: metrics.cellHeight, px: metrics.iconPx }
      : null
  return {
    screenWidth: metrics.screenWidth,
    screenHeight: metrics.screenHeight,
    taskbarHeight: metrics.taskbarHeight,
    iconPx,
    // Preserve the observed gutter around the (possibly different) preview icon; clamp so a
    // freak reading can never make the cell narrower than the icon.
    cellWidth: observed ? Math.max(iconPx, iconPx + (observed.w - observed.px)) : iconPx + 44,
    cellHeight: observed ? Math.max(iconPx, iconPx + (observed.h - observed.px)) : iconPx + 48,
    inset: 14,
    labelFontPx: 12,
  }
}

/** The factory-default recipe (方圆/squircle) — the store's initial draft when ② is empty. */
export function defaultRecipe(kindPolicy: KindPolicy): IconStyleRecipe {
  return {
    config: { ...BASE_CONFIGS[DEFAULT_PRESET_ID] },
    kindPolicy: { ...kindPolicy },
    typeOverrides: structuredClone(PRESET_TYPE_OVERRIDES[DEFAULT_PRESET_ID] ?? {}),
  }
}

/** Parses a persisted recipe JSON (styleJson) into its three knobs, or `null` if
 *  malformed. Delegates to the ONE parser (lib/icon-look, spec 09 §1): version
 *  gate → migration chain → strict enum validation. A payload from a NEWER app
 *  (unknown version/enums) parses null — callers fall back to factory default
 *  rather than rendering garbage. */
export function parseRecipe(styleJson: string | null): IconStyleRecipe | null {
  return parseIconLook(styleJson)
}

/** Maps store-③ look-history entries into the `HistoryEntryDto`s the panel renders, carrying the
 *  FULL recipe (config + kindPolicy + typeOverrides) so 回到此版 restores the participation policy
 *  too. `isCurrent` is determined by ② (`savedStyleJson`), NOT position: after a reset ② is null, so
 *  NO entry is current (codex Major 2). */
export function parseHistory(history: LookVersionDto[], savedStyleJson: string | null): HistoryEntryDto[] {
  return history.map((v, index) => {
    const recipe = parseRecipe(v.styleJson)
    return {
      index,
      time: v.label ?? formatTime(v.createdAt),
      label: v.label ?? '自定义',
      isCurrent: savedStyleJson !== null && v.styleJson === savedStyleJson,
      config: recipe?.config ?? { ...BASE_CONFIGS[DEFAULT_PRESET_ID] },
      kindPolicy: recipe?.kindPolicy ?? { ...DEFAULT_KIND_POLICY },
      typeOverrides: recipe?.typeOverrides ?? {},
    }
  })
}

function formatTime(unixSeconds: number | null): string {
  // A legacy history row can carry a null timestamp (codex R2 B-8); render no time rather than the
  // 1970 epoch `new Date(null)` would produce. The label ('自定义') still identifies the entry.
  if (unixSeconds == null) return ''
  const d = new Date(unixSeconds * 1000)
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

/** The ONE preset-identity rule (ADR-0018 bookmark identity): does the draft
 *  coordinate match this recipe? Shared by the built-in `activePresetIdOf` and
 *  the user preset library's selection highlight (spec 09) — the two matchers
 *  can never drift. */
export function recipeMatchesDraft(
  preset: ConfigDto,
  presetOverrides: TypeOverrides | undefined,
  config: ConfigDto,
  typeOverrides: TypeOverrides,
): boolean {
  return (
    preset.shape === config.shape &&
    preset.subject === config.subject &&
    preset.filter === config.filter &&
    preset.distinction === config.distinction &&
    typeOverridesEqual(presetOverrides, typeOverrides) &&
    (preset.shortcutShape ?? null) === (config.shortcutShape ?? null) &&
    (preset.plateColor ?? null) === (config.plateColor ?? null) &&
    preset.plateFallback === config.plateFallback &&
    (preset.plateColor !== null || preset.plateBand === config.plateBand) &&
    (preset.subject !== 'Mono' || preset.monoStyle === config.monoStyle) &&
    (preset.subject !== 'Mono' || preset.tint.toUpperCase() === config.tint.toUpperCase())
  )
}

/** Which BUILT-IN preset (if any) the current draft matches. The store's
 *  `activePresetIdOf(state)` delegates here so the selection highlight and the
 *  assembled `activePresetId` never drift. */
export function activePresetIdOf(config: ConfigDto, typeOverrides: TypeOverrides): string | null {
  for (const [id, preset] of Object.entries(BASE_CONFIGS)) {
    if (recipeMatchesDraft(preset, PRESET_TYPE_OVERRIDES[id], config, typeOverrides)) return id
  }
  return null
}

/** The persisted + native bits Rust reports (mirrors `IconPersistedDto`, parsed by the store). */
export interface PersistedIcons {
  history: HistoryEntryDto[]
  applied: boolean
  arrowOverlay: 'native' | 'hidden'
  /** Schema 10 (optional here: older callers/tests may omit it; assemble defaults false). */
  overlayStale?: boolean
  activeUserProfiles: number
}

/** Assembles the full `IconsStateDto` the store consumes from the live draft + scan items + the
 *  persisted/native bits + the frontend's own presets/palette/grid. Session flags
 *  (`scanning`/`working`/`dirty`) default false — the store owns them on top of this. */
export function assembleIconsState(args: {
  draft: IconStyleRecipe
  items: IconItemDto[]
  persisted: PersistedIcons
  wallpaperUrl: string | null
  gridMetrics?: GridMetricsDto
}): IconsStateDto {
  const { draft, items, persisted, wallpaperUrl, gridMetrics } = args
  return {
    scanning: false,
    working: false,
    applied: persisted.applied,
    overlayStale: persisted.overlayStale ?? false,
    dirty: false,
    styleableCount: items.filter((i) => i.styleable).length,
    config: { ...draft.config },
    activePresetId: activePresetIdOf(draft.config, draft.typeOverrides),
    presets: iconPresets(),
    history: persisted.history,
    palette: ICON_PALETTE,
    monoSwatches: MONO_SWATCHES,
    markSwatches: MARK_SWATCHES,
    grid: iconGrid(draft.config.size, gridMetrics),
    wallpaperUrl,
    kindPolicy: { ...draft.kindPolicy },
    typeOverrides: structuredClone(draft.typeOverrides),
    arrowOverlay: persisted.arrowOverlay,
    activeUserProfiles: persisted.activeUserProfiles,
  }
}
