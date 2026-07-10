// Headless replica of the icon store's per-desktop RenderSession (stores/icons.ts
// recomputeHueSpread + effectiveTileConfig) for the oracle corpus. Bun has no
// Worker/canvas, so the browser path (render.worker decode + IconCompositor
// seed/spread) cannot run — this module reproduces its DETERMINISTIC parts:
// decode-time seed (iconProfile.subjectRimColour), cross-icon hue spread, the
// App-accent fallback, and the resolved per-item config + opts. Every algorithm
// primitive is imported from the real code (no re-implementation); only the
// session orchestration is mirrored, and the Rust RenderSession must reproduce
// exactly what this emits (Tier C session dumps pin the seeds).
//
// The corpus is captured over the REAL icon pack (public/real-icons/, the committed
// dev-fixture SSoT, ADR-0015 D9) — the same artwork the mock desktop renders. Sources
// arrive at native sizes / colour types and are normalized to 256² (source-decode).

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import type { ConfigDto, IconItemDto, IconKind, IconKindBucket, TypeOverrides } from '@/bridge/types'
import { BASE_CONFIGS, PRESET_TYPE_OVERRIDES } from '@/bridge/mock-desktop'
import { DEFAULT_KIND_POLICY, kindBucket } from '@/lib/kind-policy'
import { appAccentSeed, resolveTypeConfig, typeHasFixedPlate } from '@/lib/type-config'
import { computeHueSpread } from '@/icon-compositor/hue-spread'
import type { SpreadEntry } from '@/icon-compositor/hue-spread'
import { iconProfile } from '@/icon-compositor/profile'
import type { Raster } from '@/icon-compositor/raster'
import { effectiveTileConfig } from '@/stores/icons'
import { decodeSourceImage } from './source-decode'

/** Preset card order (mock-desktop BASE_CONFIGS key order); spectrum is the
 *  factory default and Tier A's look. */
export const PRESET_IDS = ['spectrum', 'glass', 'ink', 'white', 'stationery', 'pebble', 'ascast'] as const
export type PresetId = (typeof PRESET_IDS)[number]

// The real-icon pack: the committed dev-fixture SSoT (Microsoft system + brand art,
// ADR-0015 D9 — lives in the repo, stripped from every ship).
export const SOURCE_ICONS_DIR = 'public/real-icons'
// Source identity / hue-spread artKey prefix (IconItemDto.sourceUrls[0]); unique per
// subfolder-relative file, pinned throughout the committed corpus.
const SOURCE_ICONS_URL = '/real-icons'

/** One real-pack manifest entry. `kind` is already a resolved IconKind (no translation
 *  layer); `extraSources` (e.g. the empty Recycle Bin) are the codec's paired-ICO concern,
 *  not separate render sources. */
interface RealEntry {
  file: string
  id: string
  kind: IconKind
  label: string
  extraSources: string[]
}

/** A source's identity + raw bytes, WITHOUT the (expensive) decode/segment —
 *  enough for the whole-set hash. */
export interface SourceMeta {
  id: string
  file: string
  /** Absolute path of the referenced PNG (never copied into the corpus). */
  path: string
  label: string
  kind: IconKind
  bucket: IconKindBucket | null
  isShortcut: boolean
  /** Matches the app's IconItemDto.sourceUrls[0] — the hue-spread artKey. */
  sourceUrl: string
  /** Raw source bytes (for the manifest sha256; not re-emitted). */
  bytes: Uint8Array
}

/** One decoded desktop source, with the decode-time seed the store derives. */
export interface OracleSource extends SourceMeta {
  raster: Raster
  /** iconProfile(raster).subjectRimColour as uppercase hex, or null (no-hue
   *  tail). Identical to IconCompositor.seedOf on the sync path. */
  seed: string | null
}

function seedOf(raster: Raster): string | null {
  const colour = iconProfile(raster).subjectRimColour
  if (!colour) return null
  const h = (v: number) => v.toString(16).padStart(2, '0')
  return `#${h(colour.r)}${h(colour.g)}${h(colour.b)}`.toUpperCase()
}

/** Read the committed real pack's identity + bytes, in manifest order, WITHOUT decoding
 *  (cheap — enough for setHash + selective decode). Kinds are already resolved IconKinds. */
export function readSourceMetas(rootDir: string): SourceMeta[] {
  const dir = join(rootDir, SOURCE_ICONS_DIR)
  const entries = JSON.parse(readFileSync(join(dir, 'manifest.json'), 'utf8')) as RealEntry[]
  return entries.map((e) => ({
    id: e.id,
    file: e.file,
    path: join(dir, e.file),
    label: e.label,
    kind: e.kind,
    bucket: kindBucket(e.kind),
    isShortcut: e.kind === 'Shortcut' || e.kind === 'UrlShortcut' || e.kind === 'AppxShortcut',
    sourceUrl: `${SOURCE_ICONS_URL}/${e.file}`,
    bytes: new Uint8Array(readFileSync(join(dir, e.file))),
  }))
}

/** Decode one source raster (normalized to 256²) + derive its hue-spread seed. */
export function decodeSource(meta: SourceMeta): OracleSource {
  const raster = decodeSourceImage(meta.path, meta.bytes)
  return { ...meta, raster, seed: seedOf(raster) }
}

/** Decode the whole pack (capture + full verify need every source). */
export function loadSources(rootDir: string): OracleSource[] {
  return readSourceMetas(rootDir).map(decodeSource)
}

/** A fresh-scan IconItemDto (no per-icon overrides) for effectiveTileConfig. */
function asItem(s: OracleSource): IconItemDto {
  return {
    id: s.id,
    label: s.label,
    kind: s.kind,
    isShortcut: s.isShortcut,
    styleable: s.kind !== 'Unsupported',
    statusReason: null,
    x: 0,
    y: 0,
    sourceUrls: [s.sourceUrl],
    overrideMode: null,
    overrideTint: null,
  }
}

export interface LookConfig {
  id: PresetId
  config: ConfigDto
  typeOverrides: TypeOverrides
}

export function lookOf(id: PresetId): LookConfig {
  return { id, config: { ...BASE_CONFIGS[id] }, typeOverrides: structuredClone(PRESET_TYPE_OVERRIDES[id] ?? {}) }
}

/** Cross-icon hue spread for a look — a faithful mirror of stores/icons.ts
 *  recomputeHueSpread: pool = derived-plate participants whose type pins no
 *  fixed plate, then the App-accent fallback for colourless App sources. */
export function computeFieldSeeds(sources: OracleSource[], look: LookConfig): Map<string, string> {
  const { config, typeOverrides } = look
  const isDerivedParticipant = (bucket: IconKindBucket | null): boolean => {
    if (typeHasFixedPlate(typeOverrides, bucket)) return false
    const r = resolveTypeConfig(config, typeOverrides, bucket)
    return r.subject === 'Original' && r.plateColor === null && r.plateFallback !== 'white'
  }
  const entries: SpreadEntry[] = sources
    .filter((s) => isDerivedParticipant(s.bucket))
    .map((s) => ({ id: s.id, artKey: s.sourceUrl, seed: s.seed }))
  const next = computeHueSpread(entries)
  for (const s of sources) {
    if (s.bucket !== 'App' || next.has(s.id)) continue
    if (!isDerivedParticipant(s.bucket)) continue
    if (s.seed !== null) continue
    next.set(s.id, appAccentSeed(s.id))
  }
  return next
}

/** Everything a look needs to render + dump, per source. */
export interface ResolvedCell {
  source: OracleSource
  config: ConfigDto
  showOriginal: boolean
  fieldSeed: string | null
  poolMember: boolean
}

export interface ResolvedLook {
  look: LookConfig
  fieldSeeds: Map<string, string>
  cells: ResolvedCell[]
}

/** Resolve a full desktop under one look: per-item config (type ladder +
 *  shortcut layer via the store's effectiveTileConfig) + hue-spread opts. */
export function resolveLook(sources: OracleSource[], look: LookConfig): ResolvedLook {
  const fieldSeeds = computeFieldSeeds(sources, look)
  const cells = sources.map((source) => {
    const eff = effectiveTileConfig(asItem(source), look.config, DEFAULT_KIND_POLICY, look.typeOverrides)
    return {
      source,
      config: eff.config,
      showOriginal: eff.showOriginal,
      fieldSeed: fieldSeeds.get(source.id) ?? null,
      poolMember: fieldSeeds.has(source.id),
    }
  })
  return { look, fieldSeeds, cells }
}

/** Field render opts for a source under a resolved look (mirror of the store's
 *  fieldRenderOpts: item-keyed seed + bucket, shared by preview and bake). */
export function optsOf(cell: ResolvedCell): { fieldSeed: string | null; kindBucket: IconKindBucket | null } {
  return { fieldSeed: cell.fieldSeed, kindBucket: cell.source.bucket }
}
