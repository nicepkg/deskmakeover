// Tier B — the style matrix. A representative subset of sources rendered across
// every compositor axis, so the M5 Rust port gets a golden for each lane/branch
// (shapes, subjects, plate stops, marks, filters, shortcut badge) independent of
// the desktop-global hue spread (Tier A/C cover that). Sources span the whole
// classification space; the pick is data-driven (scan of all 120) and validated
// at runtime against the live profile so pack drift fails loudly.

import type { ConfigDto, FilterStyle, IconShape, MarkStyle } from '@/bridge/types'
import { BASE_CONFIGS } from '@/bridge/mock-desktop'
import type { OracleSource } from './desktop-session'
import { PRESET_IDS } from './desktop-session'

/** A selected Tier B source + the classification category it fills (recorded in the
 *  manifest). Data-driven over the real pack — no hardcoded ids to drift. */
export interface TierBPick {
  id: string
  category: string
  rationale: string
}

/** Live-profile view of a source used to bucket it into a Tier B category. */
export interface SourceProfileView {
  kind: string
  cornerSymmetric: boolean
  seed: string | null
  matchesCircle: boolean
  bucket: string | null
}

/** Category quotas spanning the compositor's classification space, in priority order.
 *  Each source is claimed by the first category whose predicate it satisfies (manifest
 *  order), so the set is deterministic and every category gets distinct representatives. */
interface Category {
  name: string
  rationale: string
  take: number
  match: (s: OracleSource, p: SourceProfileView) => boolean
}

const CATEGORIES: Category[] = [
  { name: 'ownBoard', rationale: 'own-background board → composeFromPlate / derived-plate lane', take: 6, match: (_s, p) => p.kind === 'ownBoard' },
  { name: 'fullSquare', rationale: 'full-bleed opaque art → clip-only fullSquare lane', take: 2, match: (_s, p) => p.kind === 'fullSquare' },
  { name: 'badge/circle', rationale: 'circle-matching silhouette (matchesCircle inscribe gate)', take: 2, match: (_s, p) => p.matchesCircle },
  { name: 'system/recyclebin', rationale: 'RecycleBin — fixed-plate System type-ladder target', take: 1, match: (s) => s.kind === 'RecycleBin' },
  { name: 'system/other', rationale: 'SystemIcon (This PC / Network / Control Panel)', take: 2, match: (s) => s.kind === 'SystemIcon' },
  { name: 'appx-shortcut', rationale: 'UWP AppxShortcut — styleable shortcut, wears the mark', take: 2, match: (s) => s.kind === 'AppxShortcut' },
  { name: 'file/dog-ear', rationale: 'document page, corner-asymmetric — must NOT anchor a board', take: 3, match: (_s, p) => p.bucket === 'File' && !p.cornerSymmetric },
  { name: 'file/symmetric', rationale: 'file-bucket, corner-symmetric', take: 2, match: (_s, p) => p.bucket === 'File' },
  { name: 'folder', rationale: 'folder-bucket artwork', take: 3, match: (_s, p) => p.bucket === 'Folder' },
  { name: 'app/seed-null', rationale: 'colourless App source → App-accent fallback seed', take: 3, match: (_s, p) => p.bucket === 'App' && p.seed === null },
  { name: 'app/bare', rationale: 'transparent-edge App logo (bare) → contrast-plate lane', take: 4, match: (_s, p) => p.bucket === 'App' && p.kind === 'bare' },
]

/** The canonical Tier B default config (spectrum base): one axis varies per
 *  sweep, the rest hold here. Renders receive a RESOLVED config directly
 *  (no desktop type-ladder / hue-spread — that is Tier A/C territory). */
export const TIER_B_DEFAULT: ConfigDto = { ...BASE_CONFIGS.spectrum, distinction: 'Mark', markStyle: 'Halo', markColor: null }

const ALL_SHAPES: IconShape[] = ['Apple', 'Circle', 'Samsung', 'None', 'Bookmark', 'Lemon', 'Tile', 'Teardrop', 'Diamond', 'Flower', 'Pebble', 'Folder']
const ALL_MARKS: MarkStyle[] = ['Glass', 'Shadow', 'Halo', 'Satin', 'Arc', 'Fold', 'Ring']
const ALL_FILTERS: FilterStyle[] = ['None', 'Gloss', 'Glass', 'Pixel', 'Sticker']

/** One rendered cell: a name, the resolved config, the shortcut flag, and the
 *  peek flag (showOriginal → raw artwork + arrow lane). */
export interface StyleCell {
  group: string
  name: string
  config: ConfigDto
  isShortcut: boolean
  showOriginal: boolean
}

/** The full per-source cell matrix. Deterministic ordering (group then axis). */
export function styleCells(): StyleCell[] {
  const d = TIER_B_DEFAULT
  const cells: StyleCell[] = []
  const add = (group: string, name: string, over: Partial<ConfigDto>, isShortcut = false, showOriginal = false) =>
    cells.push({ group, name, config: { ...d, ...over }, isShortcut, showOriginal })

  // 7 presets (global base config, no type-ladder) — plate/shape/subject/filter.
  for (const id of PRESET_IDS) cells.push({ group: 'preset', name: id, config: { ...BASE_CONFIGS[id] }, isShortcut: false, showOriginal: false })

  // Each shape (holding default otherwise).
  for (const shape of ALL_SHAPES) add('shape', shape, { shape })

  // Subject axis: Original / BW / Mono×(Tonal|Flat).
  add('subject', 'Original', { subject: 'Original' })
  add('subject', 'BlackWhite', { subject: 'BlackWhite' })
  add('subject', 'Mono-Tonal', { subject: 'Mono', monoStyle: 'Tonal' })
  add('subject', 'Mono-Flat', { subject: 'Mono', monoStyle: 'Flat' })

  // Plate stops: derived Vivid/Quiet, white fallback, fixed white, fixed swatch.
  add('plate', 'derived-vivid', { plateFallback: 'derived', plateBand: 'Vivid', plateColor: null })
  add('plate', 'derived-quiet', { plateFallback: 'derived', plateBand: 'Quiet', plateColor: null })
  add('plate', 'white-fallback', { plateFallback: 'white', plateColor: null })
  add('plate', 'fixed-white', { plateColor: '#FFFFFF' })
  add('plate', 'fixed-swatch', { plateColor: '#E9E2D4' })

  // Distinction axis (shortcut): Keep draws the classic arrow badge on the
  // styled tile (the real badge, not the vector fallback); None draws nothing.
  add('distinction', 'keep', { distinction: 'Keep' }, true)
  add('distinction', 'none', { distinction: 'None' }, true)

  // Mark styles (shortcut + Mark) incl. default markColor and one explicit.
  for (const markStyle of ALL_MARKS) add('mark', markStyle, { distinction: 'Mark', markStyle, markColor: null }, true)
  add('mark', 'Ring-explicit', { distinction: 'Mark', markStyle: 'Ring', markColor: '#00A040' }, true)

  // Filters incl. the liquid Glass.
  for (const filter of ALL_FILTERS) add('filter', filter, { filter })

  // Shortcut badge on/off (renderTile's isShortcut branch: Mark badge present).
  add('shortcut', 'badge-off', { distinction: 'Mark', markStyle: 'Halo' }, false)
  add('shortcut', 'badge-on', { distinction: 'Mark', markStyle: 'Halo' }, true)

  // Peek / keep path (showOriginal → raw artwork; shortcut adds classic arrow).
  add('original', 'plain', {}, false, true)
  add('original', 'shortcut', {}, true, true)

  return cells
}

/** Data-driven Tier B selection over the loaded (real) pack: walk each category in
 *  priority order and claim up to its quota of not-yet-picked sources (manifest order),
 *  classifying each by its LIVE profile. Deterministic → capture-twice stable. A category
 *  that finds zero sources fails loudly (a compositor class vanished from the pack). */
export function selectTierBSources(
  sources: OracleSource[],
  profileOf: (s: OracleSource) => SourceProfileView,
): { source: OracleSource; pick: TierBPick }[] {
  const view = new Map(sources.map((s) => [s.id, profileOf(s)]))
  const claimed = new Set<string>()
  const out: { source: OracleSource; pick: TierBPick }[] = []
  for (const cat of CATEGORIES) {
    let n = 0
    for (const s of sources) {
      if (n >= cat.take) break
      if (claimed.has(s.id)) continue
      if (!cat.match(s, view.get(s.id)!)) continue
      claimed.add(s.id)
      out.push({ source: s, pick: { id: s.id, category: cat.name, rationale: cat.rationale } })
      n++
    }
    if (n === 0) throw new Error(`Tier B category '${cat.name}' matched no source — a compositor class is missing from the pack`)
  }
  return out
}
