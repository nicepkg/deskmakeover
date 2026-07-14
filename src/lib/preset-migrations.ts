// Payload migrations (spec 09 §3): ONE ordered, pure, idempotent chain per
// payload type, driven by the payload's own schema version. Import-from-package
// and load-from-disk call the SAME functions — migration logic lives once.
// History: MATERIAL_MIGRATION started life as a version-less in-place hack in
// wallpaper-assemble (2026-07-15 round 3); it graduates here as the wallpaper
// chain's first citizens.

/** Current icon recipe schema version — bump WITH a new migration step. */
export const ICON_LOOK_VERSION = 1

type RawObject = Record<string, unknown>
type Migration = (raw: RawObject) => RawObject

/** Keyed by FROM-version: `ICON_MIGRATIONS[n]` lifts a v(n) payload to v(n+1). */
const ICON_MIGRATIONS: Record<number, Migration> = {
  // v0 → v1: the pre-versioning styleJson ({config, kindPolicy?, typeOverrides?})
  // is structurally v1 — stamping the version is most of the step. Dev-era
  // configs predating ADR-0018's two-axis fields are backfilled with the launch
  // defaults so a pre-versioning saved style survives instead of falling to
  // factory default under the strict validator.
  0: (raw) => {
    const config = raw.config
    if (config && typeof config === 'object') {
      const c = config as RawObject
      c.monoStyle ??= 'Tonal'
      c.plateBand ??= 'Vivid'
      c.plateFallback ??= 'derived'
      c.plateColor ??= null
      c.markColor ??= null
      c.shortcutShape ??= null
    }
    return raw
  },
}

/** Lift a raw icon payload from `from` to ICON_LOOK_VERSION. Returns null when
 *  the version is unknown (newer than this build, or negative) — fail closed,
 *  never guess at fields we cannot understand (spec 09 §3). */
export function migrateIconLook(raw: RawObject, from: number): RawObject | null {
  if (!Number.isInteger(from) || from < 0 || from > ICON_LOOK_VERSION) return null
  let out = raw
  for (let v = from; v < ICON_LOOK_VERSION; v++) {
    const step = ICON_MIGRATIONS[v]
    if (!step) return null
    out = step(out)
  }
  return out
}

// ---- Wallpaper look migrations (round-3 lineup, 2026-07-15) ----------------

/** Retired finishes → their heirs (one-way, silent; owner cuts included:
 *  Glaze→Fluted, Float→Brushed; Halo re-aimed at Frost). */
export const WALLPAPER_MATERIAL_MIGRATION: Record<string, string> = {
  Luminous: 'Frost',
  Solid: 'Paper',
  Halo: 'Frost',
  Glaze: 'Fluted',
  Float: 'Brushed',
}

/** Retired title styles → their heirs (Tab was a folder skeuomorph). */
export const WALLPAPER_TITLE_MIGRATION: Record<string, string> = {
  Tab: 'Chip',
}

/** Migrate one zone's enum fields in place; returns true when anything moved.
 *  Persisted entries re-save in new terms on the next write. */
export function migrateWallpaperZoneEnums(zone: { material: string; titleStyle: string }): boolean {
  let moved = false
  const material = WALLPAPER_MATERIAL_MIGRATION[zone.material]
  if (material) {
    zone.material = material
    moved = true
  }
  const title = WALLPAPER_TITLE_MIGRATION[zone.titleStyle]
  if (title) {
    zone.titleStyle = title
    moved = true
  }
  return moved
}
