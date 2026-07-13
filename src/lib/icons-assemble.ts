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
import { typeOverridesEqual } from '@/lib/type-config'

/** The three global knobs a saved appearance recipe carries (store ②③, spec 07 §8.2). */
export interface IconStyleRecipe {
  config: ConfigDto
  kindPolicy: KindPolicy
  typeOverrides: TypeOverrides
}

// ---- Preset collection v2 (chief-designer curation, owner order 2026-07-10) ----
// Coordinate bookmarks in the subject × plate space (ADR-0018); docs/product/preset-collection-v2.md
// is normative. Key order = card order (featured four above the 更多风格 fold).
export const BASE_CONFIGS: Record<string, ConfigDto> = {
  spectrum: { shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'derived', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Halo', markColor: null, plateColor: null, size: 'Mid', filter: 'None' },
  glass: { shape: 'Samsung', subject: 'Original', plateBand: 'Vivid', plateFallback: 'derived', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Shadow', markColor: null, plateColor: null, size: 'Mid', filter: 'Glass' },
  ink: { shape: 'Circle', subject: 'BlackWhite', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Arc', markColor: null, plateColor: '#F4F1EA', size: 'Mid', filter: 'None' },
  white: { shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Ring', markColor: null, plateColor: '#FFFFFF', size: 'Mid', filter: 'None' },
  stationery: { shape: 'Apple', subject: 'Original', plateBand: 'Quiet', plateFallback: 'derived', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Satin', markColor: null, plateColor: null, size: 'Mid', filter: 'None' },
  pebble: { shape: 'Pebble', subject: 'Original', plateBand: 'Quiet', plateFallback: 'derived', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Shadow', markColor: null, plateColor: null, size: 'Mid', filter: 'Sticker' },
  ascast: { shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Ring', markColor: null, plateColor: null, size: 'Mid', filter: 'None' },
}

// Per-preset type ladders (Preset Collection v2) — every set ships its own.
export const PRESET_TYPE_OVERRIDES: Record<string, TypeOverrides> = {
  spectrum: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: null, plateFallback: 'derived' } },
    File: { source: 'custom', patch: { shape: 'Tile', plateColor: '#E9E2D4' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#EDEAE4' } },
  },
  stationery: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: '#EAD6A8' } },
    File: { source: 'custom', patch: { shape: 'Tile', plateColor: '#E9E2D4' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#EDEAE4' } },
  },
  glass: {
    Folder: { source: 'custom', patch: { shape: 'Samsung', plateColor: null, plateFallback: 'derived' } },
    File: { source: 'custom', patch: { shape: 'Samsung', plateColor: '#FFFFFF' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#ECECEE' } },
  },
  pebble: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: '#EAD6A8' } },
    File: { source: 'custom', patch: { shape: 'Teardrop', plateColor: '#E9E2D4' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#EAE7E0' } },
  },
  ink: {
    Folder: { source: 'custom', patch: { shape: 'Bookmark', plateColor: '#EDE8DC' } },
    File: { source: 'custom', patch: { shape: 'Tile', plateColor: '#F4F1EA' } },
    System: { source: 'custom', patch: { shape: 'Circle', plateColor: '#EEEBE4' } },
  },
  white: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: '#FFFFFF' } },
    File: { source: 'custom', patch: { shape: 'Tile', plateColor: '#FFFFFF' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#F2F2F2' } },
  },
  ascast: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: null, plateFallback: 'white' } },
    File: { source: 'custom', patch: { shape: 'Tile', plateColor: '#E9E2D4' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#EDEAE4' } },
  },
}

/** The factory default look (the spectrum preset) — used when no saved-style (②) exists yet. */
export const DEFAULT_PRESET_ID = 'spectrum'

export const ICON_PALETTE = ['#B97D4E', '#8A5A33', '#E8C9A0', '#6E4526', '#F4E7D3']
export const MONO_SWATCHES = ['#FFFFFF', '#141414', '#B97D4E', '#FF6F5E', '#3FB6A8', '#D9A94E']
export const MARK_SWATCHES = ['#FFFFFF', '#141414', '#FF6F5E', '#B97D4E', '#3FB6A8']

// Desktop icon px is Small 32 · Mid 48 · Big 96 (C# DesktopIconSize.cs).
const ICON_PX: Record<ConfigDto['size'], number> = { Small: 32, Mid: 48, Big: 96 }
/** Fallback metrics before the first scan lands (the store always feeds real scan metrics after). */
const DEFAULT_METRICS: GridMetricsDto = { screenWidth: 1920, screenHeight: 1080, taskbarHeight: 48 }

/** Presets as DATA — the web renders each mini with the live renderer. */
export function iconPresets(): PresetDto[] {
  return Object.entries(BASE_CONFIGS).map(([id, config]) => ({
    id,
    config: { ...config },
    typeOverrides: structuredClone(PRESET_TYPE_OVERRIDES[id] ?? {}),
  }))
}

/** The desktop grid for an icon size, built from the OBSERVED platform metrics (D1: dims are
 *  platform truth from the scan; iconPx + cell padding are the frontend rendering concern). */
export function iconGrid(size: ConfigDto['size'], metrics: GridMetricsDto = DEFAULT_METRICS): GridDto {
  const iconPx = ICON_PX[size]
  return {
    screenWidth: metrics.screenWidth,
    screenHeight: metrics.screenHeight,
    taskbarHeight: metrics.taskbarHeight,
    iconPx,
    cellWidth: iconPx + 44,
    cellHeight: iconPx + 48,
    inset: 14,
    labelFontPx: 12,
  }
}

/** The factory-default recipe (spectrum) — the store's initial draft when ② is empty. */
export function defaultRecipe(kindPolicy: KindPolicy): IconStyleRecipe {
  return {
    config: { ...BASE_CONFIGS[DEFAULT_PRESET_ID] },
    kindPolicy: { ...kindPolicy },
    typeOverrides: structuredClone(PRESET_TYPE_OVERRIDES[DEFAULT_PRESET_ID] ?? {}),
  }
}

/** Parses a persisted recipe JSON (styleJson) into its three knobs, or `null` if malformed. */
export function parseRecipe(styleJson: string | null): IconStyleRecipe | null {
  if (!styleJson) return null
  try {
    const v = JSON.parse(styleJson) as Partial<IconStyleRecipe>
    if (!v || typeof v.config !== 'object' || v.config === null) return null
    return {
      config: v.config as ConfigDto,
      kindPolicy: (v.kindPolicy ?? {}) as KindPolicy,
      typeOverrides: (v.typeOverrides ?? {}) as TypeOverrides,
    }
  } catch {
    return null
  }
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

/** Which preset (if any) the current draft coordinate matches (ADR-0018 bookmark identity). The
 *  SINGLE matching rule — the store's `activePresetIdOf(state)` delegates here so the selection
 *  highlight and the assembled `activePresetId` never drift. */
export function activePresetIdOf(config: ConfigDto, typeOverrides: TypeOverrides): string | null {
  for (const [id, preset] of Object.entries(BASE_CONFIGS)) {
    if (
      preset.shape === config.shape &&
      preset.subject === config.subject &&
      preset.filter === config.filter &&
      preset.distinction === config.distinction &&
      typeOverridesEqual(PRESET_TYPE_OVERRIDES[id], typeOverrides) &&
      (preset.shortcutShape ?? null) === (config.shortcutShape ?? null) &&
      (preset.plateColor ?? null) === (config.plateColor ?? null) &&
      preset.plateFallback === config.plateFallback &&
      (preset.plateColor !== null || preset.plateBand === config.plateBand) &&
      (preset.subject !== 'Mono' || preset.monoStyle === config.monoStyle) &&
      (preset.subject !== 'Mono' || preset.tint.toUpperCase() === config.tint.toUpperCase())
    ) {
      return id
    }
  }
  return null
}

/** The persisted + native bits Rust reports (mirrors `IconPersistedDto`, parsed by the store). */
export interface PersistedIcons {
  history: HistoryEntryDto[]
  applied: boolean
  arrowOverlay: 'native' | 'hidden'
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
