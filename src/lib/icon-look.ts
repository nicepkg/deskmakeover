import type {
  ConfigDto,
  IconKindBucket,
  IconShape,
  KindPolicy,
  TypeOverrides,
  TypePatch,
} from '@/bridge/types'
import type { IconStyleRecipe } from '@/lib/icons-assemble'
import { TYPE_PATCH_KEYS } from '@/lib/type-config'
import { ICON_LOOK_VERSION, migrateIconLook } from '@/lib/preset-migrations'

// The ONE serializer / parser / validator for icon recipes (spec 09 §1 — single
// source of truth = one type + one validator + one serializer, not one storage
// medium). The store's apply path, host styleJson parsing (via icons-assemble),
// and .dmpreset import/export ALL flow through here; a second JSON.stringify of
// a recipe anywhere else is a bug. Payloads are versioned (`v`) and migrate
// forward through lib/preset-migrations.

/** What a preset package entry carries (spec 09 §2): kindPolicy only rides when
 *  the exporter opted in (owner decision #4 — participation is a per-machine
 *  choice, not aesthetics). */
export interface IconLookPayload {
  config: ConfigDto
  typeOverrides: TypeOverrides
  kindPolicy?: KindPolicy
}

// ---- Enum whitelists (validator truth; compile-checked BOTH ways) ----------
// `satisfies` rejects foreign members; the Exclude<> probes reject omissions —
// adding a value to a bridge union without updating these lists breaks tsc.

export const ICON_SHAPES = ['None', 'Apple', 'Circle', 'Samsung', 'Tile', 'Teardrop', 'Bookmark', 'Lemon', 'Diamond', 'Flower', 'Pebble', 'Folder', 'File'] as const satisfies readonly IconShape[]
const SUBJECTS = ['Original', 'BlackWhite', 'Mono'] as const satisfies readonly ConfigDto['subject'][]
const MONO_STYLES = ['Tonal', 'Flat'] as const satisfies readonly ConfigDto['monoStyle'][]
const PLATE_BANDS = ['Vivid', 'Quiet'] as const satisfies readonly ConfigDto['plateBand'][]
const PLATE_FALLBACKS = ['derived', 'white'] as const satisfies readonly ConfigDto['plateFallback'][]
const DISTINCTIONS = ['None', 'Mark', 'Keep'] as const satisfies readonly ConfigDto['distinction'][]
const MARK_STYLES = ['Glass', 'Shadow', 'Halo', 'Satin', 'Arc', 'Fold', 'Ring', 'Comet'] as const satisfies readonly ConfigDto['markStyle'][]
const SIZES = ['Small', 'Mid', 'Big'] as const satisfies readonly ConfigDto['size'][]
const FILTERS = ['None', 'Gloss', 'Glass', 'Pixel', 'Sticker'] as const satisfies readonly ConfigDto['filter'][]
const BUCKETS = ['App', 'Folder', 'File', 'System'] as const satisfies readonly IconKindBucket[]

type MustCover<Union, List extends readonly Union[]> = Exclude<Union, List[number]> extends never ? true : never
const _shapes: MustCover<IconShape, typeof ICON_SHAPES> = true
const _subjects: MustCover<ConfigDto['subject'], typeof SUBJECTS> = true
const _monos: MustCover<ConfigDto['monoStyle'], typeof MONO_STYLES> = true
const _bands: MustCover<ConfigDto['plateBand'], typeof PLATE_BANDS> = true
const _fallbacks: MustCover<ConfigDto['plateFallback'], typeof PLATE_FALLBACKS> = true
const _dists: MustCover<ConfigDto['distinction'], typeof DISTINCTIONS> = true
const _marks: MustCover<ConfigDto['markStyle'], typeof MARK_STYLES> = true
const _sizes: MustCover<ConfigDto['size'], typeof SIZES> = true
const _filters: MustCover<ConfigDto['filter'], typeof FILTERS> = true
const _buckets: MustCover<IconKindBucket, typeof BUCKETS> = true
void _shapes; void _subjects; void _monos; void _bands; void _fallbacks; void _dists; void _marks; void _sizes; void _filters; void _buckets

const HEX_RE = /^#[0-9a-fA-F]{6}$/

const oneOf = <T extends string>(list: readonly T[], v: unknown): v is T =>
  typeof v === 'string' && (list as readonly string[]).includes(v)

/** Serialize a recipe/payload for persistence/export. Versioned since
 *  2026-07-15 — the pre-versioning styleJson carried NO `v` and would have
 *  mis-rendered silently across enum renames (the MATERIAL_MIGRATION lesson).
 *  Accepts the payload shape (kindPolicy optional): a preset/export payload
 *  omits it (owner decision #4); the host ② styleJson always carries it. */
export function serializeIconLook(payload: IconLookPayload): string {
  return JSON.stringify({
    v: ICON_LOOK_VERSION,
    config: payload.config,
    // JSON.stringify drops undefined — an omitted kindPolicy writes no key.
    kindPolicy: payload.kindPolicy,
    typeOverrides: payload.typeOverrides,
  })
}

/** Parse a persisted/imported recipe JSON: version-gate → migrate forward →
 *  normalize (strict enum whitelists). Preserves the kindPolicy PRESENCE signal
 *  (undefined = a style-only preset; present = an opt-in-exported backup, spec
 *  09 §5). Null on anything we cannot honestly render. */
export function parseIconLookPayload(json: string | null): IconLookPayload | null {
  if (!json) return null
  let raw: unknown
  try {
    raw = JSON.parse(json)
  } catch {
    return null
  }
  if (!raw || typeof raw !== 'object') return null
  const obj = raw as Record<string, unknown>
  const v = typeof obj.v === 'number' ? obj.v : 0
  const migrated = migrateIconLook(obj, v)
  if (!migrated) return null
  return normalizeIconLook(migrated)
}

/** Recipe view (host styleJson / history): absent kindPolicy reads as empty —
 *  the call sites supply their own defaults (DEFAULT_KIND_POLICY etc.). */
export function parseIconLook(json: string | null): IconStyleRecipe | null {
  const payload = parseIconLookPayload(json)
  if (!payload) return null
  return {
    config: payload.config,
    kindPolicy: (payload.kindPolicy ?? {}) as KindPolicy,
    typeOverrides: payload.typeOverrides,
  }
}

/** The ONE validator (spec 09 §4.3): strict enum whitelists per field, hex
 *  checks, patch-key whitelist. Unknown enum values REJECT the payload rather
 *  than reaching the renderer — never render garbage, never silently coerce.
 *  Used by host styleJson parsing AND package import. */
export function normalizeIconLook(raw: unknown): IconLookPayload | null {
  if (!raw || typeof raw !== 'object') return null
  const obj = raw as Record<string, unknown>
  const config = normalizeConfig(obj.config)
  if (!config) return null
  const typeOverrides = normalizeTypeOverrides(obj.typeOverrides)
  if (typeOverrides === null) return null
  const kindPolicy = normalizeKindPolicy(obj.kindPolicy)
  if (kindPolicy === null) return null
  return kindPolicy === undefined ? { config, typeOverrides } : { config, typeOverrides, kindPolicy }
}

function normalizeConfig(raw: unknown): ConfigDto | null {
  if (!raw || typeof raw !== 'object') return null
  const c = raw as Record<string, unknown>
  if (!oneOf(ICON_SHAPES, c.shape)) return null
  if (!oneOf(SUBJECTS, c.subject)) return null
  if (!oneOf(MONO_STYLES, c.monoStyle)) return null
  if (!oneOf(PLATE_BANDS, c.plateBand)) return null
  if (!oneOf(PLATE_FALLBACKS, c.plateFallback)) return null
  if (!oneOf(DISTINCTIONS, c.distinction)) return null
  if (!oneOf(MARK_STYLES, c.markStyle)) return null
  if (!oneOf(SIZES, c.size)) return null
  if (!oneOf(FILTERS, c.filter)) return null
  if (typeof c.tint !== 'string' || !HEX_RE.test(c.tint)) return null
  if (c.plateColor !== null && (typeof c.plateColor !== 'string' || !HEX_RE.test(c.plateColor))) return null
  if (c.markColor !== null && (typeof c.markColor !== 'string' || !HEX_RE.test(c.markColor))) return null
  if (c.shortcutShape !== null && c.shortcutShape !== undefined && !oneOf(ICON_SHAPES, c.shortcutShape)) return null
  return {
    shape: c.shape,
    subject: c.subject,
    monoStyle: c.monoStyle,
    plateBand: c.plateBand,
    plateFallback: c.plateFallback,
    distinction: c.distinction,
    markStyle: c.markStyle,
    size: c.size,
    filter: c.filter,
    tint: c.tint,
    plateColor: c.plateColor,
    markColor: c.markColor,
    shortcutShape: (c.shortcutShape ?? null) as ConfigDto['shortcutShape'],
  }
}

/** null = reject; a valid sparse map (possibly {}) otherwise. Unknown buckets
 *  and unknown patch keys are DROPPED (additive tolerance), but a KNOWN key
 *  with an invalid value rejects (spec 09 §3 compat rules). */
function normalizeTypeOverrides(raw: unknown): TypeOverrides | null {
  if (raw === undefined || raw === null) return {}
  if (typeof raw !== 'object') return null
  const out: TypeOverrides = {}
  for (const bucket of BUCKETS) {
    const entry = (raw as Record<string, unknown>)[bucket]
    if (entry === undefined || entry === null) continue
    if (typeof entry !== 'object') return null
    const e = entry as Record<string, unknown>
    if (e.source !== 'custom') continue // 'global' (or junk source) = follow — drop the entry
    const patch = normalizePatch(e.patch)
    if (patch === null) return null
    if (Object.keys(patch).length === 0) continue
    out[bucket] = { source: 'custom', patch }
  }
  return out
}

function normalizePatch(raw: unknown): TypePatch | null {
  if (raw === undefined || raw === null) return {}
  if (typeof raw !== 'object') return null
  const p = raw as Record<string, unknown>
  const out: TypePatch = {}
  for (const key of TYPE_PATCH_KEYS) {
    const v = p[key]
    if (v === undefined) continue
    switch (key) {
      case 'shape':
        if (!oneOf(ICON_SHAPES, v)) return null
        out.shape = v
        break
      case 'subject':
        if (!oneOf(SUBJECTS, v)) return null
        out.subject = v
        break
      case 'monoStyle':
        if (!oneOf(MONO_STYLES, v)) return null
        out.monoStyle = v
        break
      case 'plateBand':
        if (!oneOf(PLATE_BANDS, v)) return null
        out.plateBand = v
        break
      case 'plateFallback':
        if (!oneOf(PLATE_FALLBACKS, v)) return null
        out.plateFallback = v
        break
      case 'tint':
        if (typeof v !== 'string' || !HEX_RE.test(v)) return null
        out.tint = v
        break
      case 'plateColor':
        if (v !== null && (typeof v !== 'string' || !HEX_RE.test(v))) return null
        out.plateColor = v as TypePatch['plateColor']
        break
    }
  }
  return out
}

/** undefined = absent (a preset payload — fine); null = present but invalid. */
function normalizeKindPolicy(raw: unknown): KindPolicy | undefined | null {
  if (raw === undefined) return undefined
  if (raw === null || typeof raw !== 'object') return null
  const src = raw as Record<string, unknown>
  const out: Partial<Record<IconKindBucket, boolean>> = {}
  for (const bucket of BUCKETS) {
    const v = src[bucket]
    if (v === undefined) continue
    if (typeof v !== 'boolean') return null
    out[bucket] = v
  }
  return out as KindPolicy
}
