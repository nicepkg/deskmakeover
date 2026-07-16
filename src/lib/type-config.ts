import type { ConfigDto, IconKindBucket, TypeOverrides, TypePatch } from '@/bridge/types'

// The per-type resolve chain (ADR-0017 D2): one pure merge turns the global
// base config + a bucket's sparse patch into the config a tile renders with.
// Everything downstream (preview, styleKey, bake) consumes RESOLVED configs —
// no stage re-derives type styling.

/** The only ConfigDto keys a type may override (spec 06 §6.5 envelope).
 *  Filter and Original-mode are global-only by law. */
export const TYPE_PATCH_KEYS = ['shape', 'subject', 'tint', 'plateBand', 'monoStyle', 'plateColor', 'plateFallback'] as const

/** The config a bucket's icons render with: base for followers, base+patch
 *  for custom types. Bucketless icons (Unsupported) always take base. */
export function resolveTypeConfig(
  base: ConfigDto,
  overrides: TypeOverrides | undefined,
  bucket: IconKindBucket | null,
): ConfigDto {
  if (!bucket) return base
  const entry = overrides?.[bucket]
  if (!entry || entry.source !== 'custom' || !entry.patch) return base
  const patch: TypePatch = entry.patch
  const merged: ConfigDto = { ...base }
  if (patch.shape !== undefined) merged.shape = patch.shape
  if (patch.subject !== undefined) merged.subject = patch.subject
  if (patch.tint !== undefined) merged.tint = patch.tint
  if (patch.plateBand !== undefined) merged.plateBand = patch.plateBand
  if (patch.monoStyle !== undefined) merged.monoStyle = patch.monoStyle
  if (patch.plateColor !== undefined) merged.plateColor = patch.plateColor
  if (patch.plateFallback !== undefined) merged.plateFallback = patch.plateFallback
  return merged
}

/** True when the bucket carries a live custom patch. */
export function typeIsCustom(overrides: TypeOverrides | undefined, bucket: IconKindBucket): boolean {
  const entry = overrides?.[bucket]
  return !!entry && entry.source === 'custom' && !!entry.patch && Object.keys(entry.patch).length > 0
}

/** True when the bucket's custom patch asserts its own SHAPE. Shape precedence
 *  (owner 2026-07-16, supersedes the unconditional swap): type patch shape >
 *  uniform shortcut shape > global shape — a per-type shape is the more
 *  specific user assertion, so the opt-in shortcut uniform yields to it. */
export function typeAssertsShape(overrides: TypeOverrides | undefined, bucket: IconKindBucket | null): boolean {
  if (!bucket) return false
  const entry = overrides?.[bucket]
  return !!entry && entry.source === 'custom' && entry.patch?.shape !== undefined
}

/** A type that PINS its plate colour exits the hue-spread pool — one hue
 *  authority per plate (ADR-0017 D3): the pool never fights a fixed plate. */
export function typeHasFixedPlate(overrides: TypeOverrides | undefined, bucket: IconKindBucket | null): boolean {
  if (!bucket) return false
  const entry = overrides?.[bucket]
  return !!entry && entry.source === 'custom' && entry.patch?.plateColor != null
}

/** Deep-enough equality for preset matching (sparse maps, small patches). */
export function typeOverridesEqual(a: TypeOverrides | undefined, b: TypeOverrides | undefined): boolean {
  const buckets: IconKindBucket[] = ['App', 'Folder', 'File']
  for (const k of buckets) {
    const ea = a?.[k]
    const eb = b?.[k]
    const customA = !!ea && ea.source === 'custom'
    const customB = !!eb && eb.source === 'custom'
    if (customA !== customB) return false
    if (!customA) continue
    const pa = ea!.patch ?? {}
    const pb = eb!.patch ?? {}
    for (const key of TYPE_PATCH_KEYS) {
      if ((pa[key] ?? undefined) !== (pb[key] ?? undefined)) return false
    }
  }
  return true
}

/** Brand accents rotated onto COLOURLESS App-bucket sources (owner special
 *  case 2026-07-10, ADR-0017): a grey .exe/tool icon must not take the
 *  neutral plate and sink into the file band — programs stay the loudest
 *  layer. Deterministic per id; other buckets keep the pure-neutral law. */
/** Coral + teal ONLY: amber's DARK variant (#5f491a for a light glyph)
 *  lands inside the Folder band's gold domain (#65470D) — a program must
 *  never dress like a folder (designer re-acceptance must-fix). */
export const APP_ACCENT_SEEDS = ['#FF6F5E', '#3FB6A8'] as const

export function appAccentSeed(id: string): string {
  let h = 0
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0
  return APP_ACCENT_SEEDS[h % APP_ACCENT_SEEDS.length]
}
