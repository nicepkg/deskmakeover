// Headless replica of the icon store's per-desktop RenderSession (stores/icons.ts
// recomputeHueSpread + effectiveTileConfig) for the M0b oracle corpus. Bun has
// no Worker/canvas, so the browser path (render.worker decode + IconCompositor
// seed/spread) cannot run — this module reproduces its DETERMINISTIC parts:
// decode-time seed (iconProfile.subjectRimColour), cross-icon hue spread, the
// App-accent fallback, and the resolved per-item config + opts. Every algorithm
// primitive is imported from the real code (no re-implementation); only the
// session orchestration is mirrored, and the Rust RenderSession must reproduce
// exactly what this emits (Tier C session dumps pin the seeds).

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import type { ConfigDto, IconItemDto, IconKind, IconKindBucket, TypeOverrides } from '@/bridge/types'
import { BASE_CONFIGS, KIND_MAP, PRESET_TYPE_OVERRIDES } from '@/bridge/mock-desktop'
import { DEFAULT_KIND_POLICY, kindBucket } from '@/lib/kind-policy'
import { appAccentSeed, resolveTypeConfig, typeHasFixedPlate } from '@/lib/type-config'
import { computeHueSpread } from '@/icon-compositor/hue-spread'
import type { SpreadEntry } from '@/icon-compositor/hue-spread'
import { iconProfile } from '@/icon-compositor/profile'
import type { Raster } from '@/icon-compositor/raster'
import { effectiveTileConfig } from '@/stores/icons'
import { decodePng } from './png-codec'

/** Preset card order (mock-desktop BASE_CONFIGS key order); spectrum is the
 *  factory default and Tier A's look. */
export const PRESET_IDS = ['spectrum', 'glass', 'ink', 'white', 'stationery', 'pebble', 'ascast'] as const
export type PresetId = (typeof PRESET_IDS)[number]

// Synthetic parity fixtures (deterministic, committed — the oracle corpus is
// anchored to them; the mock DESKTOP itself uses public/real-icons only).
export const MOCK_ICONS_DIR = 'testdata/icons/source-pack'
// LEGACY IDENTITY PREFIX, not a served URL: sourceUrl is the hue-spread
// artKey pinned throughout the committed corpus — do not rename.
const MOCK_ICONS_URL = '/mock-icons'

interface SyntheticEntry {
  file: string
  id: string
  kind: keyof typeof KIND_MAP
  label: string
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

/** Read the committed synthetic mock pack's identity + bytes, in manifest
 *  order, WITHOUT decoding (cheap — enough for setHash + selective decode). */
export function readSourceMetas(rootDir: string): SourceMeta[] {
  const dir = join(rootDir, MOCK_ICONS_DIR)
  const entries = JSON.parse(readFileSync(join(dir, 'manifest.json'), 'utf8')) as SyntheticEntry[]
  return entries.map((e) => {
    const kind = KIND_MAP[e.kind]
    return {
      id: e.id,
      file: e.file,
      path: join(dir, e.file),
      label: e.label,
      kind,
      bucket: kindBucket(kind),
      isShortcut: kind === 'Shortcut' || kind === 'UrlShortcut' || kind === 'AppxShortcut',
      sourceUrl: `${MOCK_ICONS_URL}/${e.file}`,
      bytes: new Uint8Array(readFileSync(join(dir, e.file))),
    }
  })
}

/** Decode one source raster + derive its hue-spread seed. */
export function decodeSource(meta: SourceMeta): OracleSource {
  const raster = decodePng(meta.bytes)
  return { ...meta, raster, seed: seedOf(raster) }
}

/** Decode the whole pack (capture + full verify need every source). */
export function loadMockSources(rootDir: string): OracleSource[] {
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
