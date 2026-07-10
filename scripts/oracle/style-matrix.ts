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

/** Each pick names the classification cell it fills — recorded in the manifest.
 *  Verified at runtime: the live profile must still match `expect`. */
export interface TierBPick {
  id: string
  category: string
  rationale: string
  /** A predicate string documenting the classification asserted (see verify). */
  expect: { kind?: 'fullSquare' | 'ownBoard' | 'bare'; bucket?: string; cornerSymmetric?: boolean; seedNull?: boolean; matchesCircle?: boolean }
}

/** ~24 sources spanning every classification category present in the 120-icon
 *  synthetic pack (all eight the phase brief lists ARE present; none fabricated). */
export const TIER_B_SELECTION: TierBPick[] = [
  { id: 'mock-017', category: 'ownBoard/deep-rim', rationale: 'deep indigo rim #320C9C — dark board, derived plate pushes light', expect: { kind: 'ownBoard' } },
  { id: 'mock-019', category: 'ownBoard/deep-rim', rationale: 'deep navy rim #232B7E — second dark-rim board', expect: { kind: 'ownBoard' } },
  { id: 'mock-026', category: 'ownBoard/yellow-rim', rationale: 'warm amber rim #EDB252 — yellow board (folder-domain hue on an App)', expect: { kind: 'ownBoard' } },
  { id: 'mock-035', category: 'ownBoard/yellow-rim', rationale: 'saturated yellow rim #DCBA19', expect: { kind: 'ownBoard' } },
  { id: 'mock-069', category: 'ownBoard/pale-yellow-rim', rationale: 'pale yellow rim #E2D472 — light board edge', expect: { kind: 'ownBoard' } },
  { id: 'mock-025', category: 'badge/circle', rationale: 'circle-matching silhouette on a board (matchesCircle) — sphere-in-ring class', expect: { kind: 'ownBoard', matchesCircle: true } },
  { id: 'mock-097', category: 'badge/circle', rationale: 'magenta circle badge (matchesCircle), seed #DE1F5C', expect: { matchesCircle: true } },
  { id: 'mock-054', category: 'fullSquare', rationale: 'full-bleed busy art — fullSquare clip-only lane', expect: { kind: 'fullSquare' } },
  { id: 'mock-055', category: 'fullSquare', rationale: 'second full-bleed source', expect: { kind: 'fullSquare' } },
  { id: 'mock-000', category: 'bare/colourful', rationale: 'transparent irregular colourful logo (seed #DDB268), App/Shortcut', expect: { kind: 'bare', bucket: 'App' } },
  { id: 'mock-005', category: 'bare/dark-subject', rationale: 'dark bare subject (L≈0.21) — dark-art contrast plate', expect: { kind: 'bare' } },
  { id: 'mock-021', category: 'bare/pale-subject', rationale: 'near-white bare artwork (L≈0.97) — pale-class ring halo', expect: { kind: 'bare' } },
  { id: 'mock-040', category: 'bare/generic', rationale: 'mid-lightness colourful bare logo', expect: { kind: 'bare' } },
  { id: 'mock-104', category: 'folder/blue', rationale: 'folder, blue rim #5CA2D4', expect: { bucket: 'Folder' } },
  { id: 'mock-106', category: 'folder/gold', rationale: 'folder, gold rim #D3B55B', expect: { bucket: 'Folder' } },
  { id: 'mock-113', category: 'folder/colourless', rationale: 'folder with no-hue rim (neutral board)', expect: { bucket: 'Folder', seedNull: true } },
  { id: 'mock-114', category: 'file/dog-ear', rationale: 'document page (.xlsx), bare + corner-asymmetric (dog-ear) — must NOT anchor a board', expect: { bucket: 'File', kind: 'bare', cornerSymmetric: false } },
  { id: 'mock-117', category: 'file/dog-ear', rationale: 'document page (.pdf), dog-ear class', expect: { bucket: 'File', cornerSymmetric: false } },
  { id: 'mock-119', category: 'file/dog-ear', rationale: 'document page (.docx), dog-ear class', expect: { bucket: 'File', cornerSymmetric: false } },
  { id: 'mock-011', category: 'app/colourless-exe', rationale: 'grey .exe launcher, seed null → App-accent fallback seed', expect: { bucket: 'App', seedNull: true } },
  { id: 'mock-045', category: 'app/colourless-exe', rationale: 'second colourless .exe', expect: { bucket: 'App', seedNull: true } },
  { id: 'mock-004', category: 'system/recyclebin', rationale: 'RecycleBin (System bucket) — the fixed-plate type ladder target', expect: { bucket: 'System' } },
  { id: 'mock-003', category: 'appx-shortcut', rationale: 'UWP AppxShortcut — styleable shortcut, wears the mark', expect: { bucket: 'App' } },
  { id: 'mock-008', category: 'url-shortcut', rationale: 'UrlShortcut', expect: { bucket: 'App' } },
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

/** Resolve the selection against the loaded pack; assert each pick still
 *  classifies as documented (pack drift → loud failure, never a silent skip). */
export function selectTierBSources(
  sources: OracleSource[],
  profileOf: (s: OracleSource) => { kind: string; cornerSymmetric: boolean; seed: string | null; matchesCircle: boolean; bucket: string | null },
): { source: OracleSource; pick: TierBPick }[] {
  const byId = new Map(sources.map((s) => [s.id, s]))
  return TIER_B_SELECTION.map((pick) => {
    const source = byId.get(pick.id)
    if (!source) throw new Error(`Tier B pick ${pick.id} missing from the mock pack`)
    const p = profileOf(source)
    const e = pick.expect
    const fail = (why: string) => {
      throw new Error(`Tier B pick ${pick.id} (${pick.category}) drifted: ${why}`)
    }
    if (e.kind && p.kind !== e.kind) fail(`kind ${p.kind} != ${e.kind}`)
    if (e.bucket && p.bucket !== e.bucket) fail(`bucket ${p.bucket} != ${e.bucket}`)
    if (e.cornerSymmetric !== undefined && p.cornerSymmetric !== e.cornerSymmetric) fail(`cornerSymmetric ${p.cornerSymmetric}`)
    if (e.seedNull !== undefined && (p.seed === null) !== e.seedNull) fail(`seedNull ${p.seed === null}`)
    if (e.matchesCircle !== undefined && p.matchesCircle !== e.matchesCircle) fail(`matchesCircle ${p.matchesCircle}`)
    return { source, pick }
  })
}
