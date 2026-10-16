import type { IconKind, IconKindBucket, KindPolicy, Surface, Surfaces } from '@/bridge/types'
import type { StringKey } from '@/lib/i18n'

// The participation-policy bucketing (chief-UI/UX + owner 2026-07-09): three
// user-facing buckets over the engineering IconKind taxonomy — kind names like
// AppxShortcut / RegularFile are jargon, buckets speak the user's language.
// The former System bucket merged into App (owner 2026-07-16): Recycle Bin /
// This PC read as programs to the user; a fourth split was taxonomy jargon.
// Unsupported has no bucket (never styleable → never governed).

export const KIND_BUCKETS: IconKindBucket[] = ['App', 'Folder', 'File']

/** Every bucket participates by default — the app beautifies everything until
 *  the user opts a bucket out. */
export const DEFAULT_KIND_POLICY: KindPolicy = { App: true, Folder: true, File: true }

/** The bucket an icon kind belongs to, or null for Unsupported (ungoverned). */
export function kindBucket(kind: IconKind): IconKindBucket | null {
  switch (kind) {
    // Kind carries TARGET semantics (owner 2026-07-16, supersedes ADR-0017 D1's
    // "regardless of target"): the scan resolves a .lnk's target and classifies
    // by it — a shortcut to a folder arrives kind=Folder + isShortcut=true and
    // walks the Folder ladder/policy while keeping the shortcut mark layer.
    // A bare 'Shortcut' kind = unresolvable target (broken link) → App fallback.
    case 'Shortcut':
    case 'UrlShortcut':
    case 'AppxShortcut':
    // Bare .exe launchers are PROGRAMS to the user (ADR-0017 D1) — they
    // bucket with App, never with documents.
    case 'ExecutableFile':
    // System virtual items (Recycle Bin / This PC / Network / Control Panel)
    // are PROGRAMS in the user's mind (owner merge 2026-07-16) — same bucket,
    // same policy switch, same type ladder as every other launcher.
    case 'RecycleBin':
    case 'SystemIcon':
      return 'App'
    case 'Folder':
    // A fixed drive is an Explorer container the user browses — it rides the
    // Folder bucket: same policy switch, same explorer consent (ADR-0004 D7).
    case 'Drive':
      return 'Folder'
    case 'RegularFile':
    // A file TYPE is a DOCUMENT's icon (an extension association, ADR-0004 D7
    // amendment 2026-10-16) — it rides the File bucket, so flipping File OFF
    // disables every association icon while folders/drives stay styled. Same
    // explorer surface/consent as its siblings.
    case 'FileType':
      return 'File'
    default:
      return null
  }
}

/** i18n label key per bucket (singular/plural handled by the caller's copy). */
export const BUCKET_NAME_KEY: Record<IconKindBucket, StringKey> = {
  App: 'KindBucket_App',
  Folder: 'KindBucket_Folder',
  File: 'KindBucket_File',
}

/** True when the icon participates in beautify under the policy (before per-icon
 *  overrides — those cascade above this in effectiveTileConfig). Unsupported and
 *  bucketless kinds are governed by styleable, not the policy, so they pass. */
export function kindParticipates(kind: IconKind, policy: KindPolicy): boolean {
  const bucket = kindBucket(kind)
  return bucket === null ? true : policy[bucket]
}

// ---- Surface participation (ADR-0024) — the second participation axis --------

/** Every shell surface participates BY DEFAULT (owner decision 2026-10-12,
 *  amending ADR-0024's original all-OFF opt-in): the app's promise is
 *  system-wide restyling, so a fresh recipe styles the taskbar and Start menu
 *  too — the SAME participate-by-default rule as kindPolicy. Opting a surface
 *  OUT is explicit (and disabling one auto-restores it on the next apply,
 *  RESTORE-FIRST). */
export const DEFAULT_SURFACES: Surfaces = { taskbar: true, startMenu: true, explorer: true }

/** The `surfaces` legs — a CLOSED whitelist (G11): only these keys exist, any
 *  other key in a `surfaces` object is dropped on read (additive tolerance,
 *  mirroring unknown kind buckets). */
const SURFACE_KEYS = ['taskbar', 'startMenu', 'explorer'] as const satisfies readonly (keyof Surfaces)[]

/** Whether an item on `surface` participates under `surfaces`. The desktop is
 *  NEVER gated here — it is the app's core promise; kindPolicy governs it alone. */
export function surfaceParticipates(surface: Surface, surfaces: Surfaces): boolean {
  switch (surface) {
    case 'desktop':
      return true
    case 'taskbar':
      return surfaces.taskbar
    case 'startMenu':
      return surfaces.startMenu
    case 'explorer':
      return surfaces.explorer
  }
}

/** G11 normalizer for a recipe's `surfaces` leg: undefined → undefined (absent
 *  — a style-only preset stays clean); null/non-object → null (REJECT the
 *  recipe, never render garbage); unknown keys dropped; a KNOWN key with a
 *  non-boolean value rejects the whole leg. Missing known legs take
 *  [`DEFAULT_SURFACES`] (participate — the owner's default, 2026-10-12). */
export function normalizeSurfaces(raw: unknown): Surfaces | undefined | null {
  if (raw === undefined) return undefined
  if (raw === null || typeof raw !== 'object') return null
  const src = raw as Record<string, unknown>
  const out: Surfaces = { ...DEFAULT_SURFACES }
  for (const key of SURFACE_KEYS) {
    const v = src[key]
    if (v === undefined) continue
    if (typeof v !== 'boolean') return null
    out[key] = v
  }
  return out
}
