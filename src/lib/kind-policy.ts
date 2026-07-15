import type { IconKind, IconKindBucket, KindPolicy } from '@/bridge/types'
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
    case 'Shortcut':
    case 'UrlShortcut':
    case 'AppxShortcut':
    // Bare .exe launchers are PROGRAMS to the user (ADR-0017 D1) — they
    // bucket with App, never with documents. Mechanism semantics: shortcuts
    // stay here regardless of target (owner override of the panel's 2:1).
    case 'ExecutableFile':
    // System virtual items (Recycle Bin / This PC / Network / Control Panel)
    // are PROGRAMS in the user's mind (owner merge 2026-07-16) — same bucket,
    // same policy switch, same type ladder as every other launcher.
    case 'RecycleBin':
    case 'SystemIcon':
      return 'App'
    case 'Folder':
      return 'Folder'
    case 'RegularFile':
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
