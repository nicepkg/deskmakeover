import { create } from 'zustand'
import { call } from '@/bridge/client'
import type { ConfigDto, KindPolicy, PresetEntryDto, PresetReadEntryDto, PresetSaveDto, TypeOverrides } from '@/bridge/types'
import { ICON_LOOK_VERSION } from '@/lib/preset-migrations'
import { parseIconLookPayload, serializeIconLook } from '@/lib/icon-look'
import { t } from '@/lib/i18n'
import { useToasts } from '@/stores/toasts'

// The user preset library (spec 09): a thin frontend view over the presets.*
// bridge verbs. Import is read → validate (the ONE TS validator via
// parseIconLook) → preview → save-per-entry; save is the only library writer.
// In the browser mock the library is in-memory and package I/O is honestly
// unavailable (needs the desktop app).

/** The applyable payload of a preset: config + typeOverrides, plus kindPolicy
 *  ONLY when the preset was exported with participation opted in (spec 09 §5 /
 *  owner decision #4). Absent kindPolicy = a style-only / community preset that
 *  must never rewrite which types participate. */
export interface PresetRecipe {
  config: ConfigDto
  typeOverrides: TypeOverrides
  kindPolicy?: KindPolicy
}

/** A library entry whose payload passed the validator, ready to render/apply. */
export interface LibraryPreset extends PresetRecipe {
  id: string
  name: string
  author: string | null
  description: string | null
  hasThumb: boolean
}

/** One package entry as shown in the import preview sheet. */
export interface ImportCandidate {
  /** Null when the entry failed (structure OR payload validation). */
  recipe: PresetRecipe | null
  name: string
  author: string | null
  thumbPngBase64: string | null
  /** Human reason when recipe is null. */
  error: string | null
  /** The raw entry (for save) — present iff recipe is present. */
  save: PresetSaveDto | null
}

/** Map one read-result entry through the ONE validator into a preview
 *  candidate. Exported pure for tests. */
export function toImportCandidate(read: PresetReadEntryDto): ImportCandidate {
  if (!read.entry) {
    return { recipe: null, name: '—', author: null, thumbPngBase64: null, error: read.error ?? 'unreadable', save: null }
  }
  const e = read.entry
  const parsed = parsePayload(e.payloadJson, e.schemaVersion)
  if (!parsed) {
    return {
      recipe: null,
      name: e.meta.name,
      author: e.meta.author,
      thumbPngBase64: read.thumbPngBase64,
      error: t('Library_InvalidEntry'),
      save: null,
    }
  }
  return {
    recipe: parsed,
    name: e.meta.name,
    author: e.meta.author,
    thumbPngBase64: read.thumbPngBase64,
    error: null,
    save: {
      id: e.id,
      presetType: 'icon',
      schemaVersion: ICON_LOOK_VERSION,
      meta: e.meta,
      // Re-serialize the VALIDATED payload — junk fields never enter the library;
      // an opt-in-exported kindPolicy is preserved (owner decision #4, codex F1).
      payloadJson: serializeIconLook(parsed),
      thumbPngBase64: read.thumbPngBase64,
    },
  }
}

/** Version-gate + migrate + strict-enum validate a package/library payload,
 *  preserving the kindPolicy presence signal. The entry carries its version in
 *  `schemaVersion`, so stamp it into the versioned envelope the parser reads. */
function parsePayload(payloadJson: string, schemaVersion: number): PresetRecipe | null {
  let raw: Record<string, unknown>
  try {
    raw = JSON.parse(payloadJson) as Record<string, unknown>
  } catch {
    return null
  }
  const payload = parseIconLookPayload(JSON.stringify({ v: schemaVersion, ...raw }))
  if (!payload) return null
  return payload.kindPolicy
    ? { config: payload.config, typeOverrides: payload.typeOverrides, kindPolicy: payload.kindPolicy }
    : { config: payload.config, typeOverrides: payload.typeOverrides }
}

function toLibraryPreset(entry: PresetEntryDto): LibraryPreset | null {
  const parsed = parsePayload(entry.payloadJson, entry.schemaVersion)
  if (!parsed) return null
  return {
    id: entry.id,
    name: entry.meta.name,
    author: entry.meta.author,
    description: entry.meta.description,
    config: parsed.config,
    typeOverrides: parsed.typeOverrides,
    kindPolicy: parsed.kindPolicy,
    hasThumb: entry.hasThumb,
  }
}

interface PresetLibraryStore {
  entries: LibraryPreset[]
  loaded: boolean
  refresh: () => Promise<void>
  /** 保存为我的风格: the current recipe → a fresh library entry. */
  saveCurrent: (name: string, recipe: { config: ConfigDto; typeOverrides: TypeOverrides }) => Promise<boolean>
  /** Read a .dmpreset into preview candidates (nothing written). */
  readPackage: (path: string) => Promise<ImportCandidate[] | null>
  /** Write the confirmed candidates into the library (import-as-copy on id collision). */
  confirmImport: (candidates: ImportCandidate[]) => Promise<number>
  deleteEntry: (id: string) => Promise<void>
  renameEntry: (id: string, name: string) => Promise<void>
  /** Export entries to destPath (from the save dialog). */
  exportTo: (destPath: string, entries: PresetSaveDto[]) => Promise<boolean>
}

// Monotonic list-refresh generation (codex #8): a slow `presets.list` must not
// publish its stale array over a delete/rename that completed while it was in flight
// (which would resurrect a deleted entry in the UI until the next refresh). Each
// refresh captures a gen and drops its result if a newer refresh or a mutation has
// since bumped it. Module scope is fine — the store is a singleton.
let listGen = 0

export const usePresetLibrary = create<PresetLibraryStore>((set, get) => ({
  entries: [],
  loaded: false,

  refresh: async () => {
    const gen = ++listGen
    try {
      const raw = await call('presets.list')
      if (gen !== listGen) return // a newer refresh or a mutation superseded us — drop the stale list
      const entries = raw.map(toLibraryPreset).filter((e): e is LibraryPreset => e !== null)
      set({ entries, loaded: true })
    } catch {
      if (gen !== listGen) return
      set({ entries: [], loaded: true })
    }
  },

  saveCurrent: async (name, recipe) => {
    try {
      const entry: PresetSaveDto = {
        id: crypto.randomUUID(),
        presetType: 'icon',
        schemaVersion: ICON_LOOK_VERSION,
        meta: { name, author: null, description: null, createdAt: new Date().toISOString() },
        payloadJson: serializeIconLook(recipe),
        thumbPngBase64: null,
      }
      await call('presets.save', { entry, overwrite: false })
      await get().refresh()
      useToasts.getState().show(t('Toast_PresetSaved'))
      return true
    } catch (e) {
      useToasts.getState().show(String(e instanceof Error ? e.message : e))
      return false
    }
  },

  readPackage: async (path) => {
    try {
      const read = await call('presets.readPackage', { path })
      if (!read.formatOk) {
        useToasts.getState().show(read.error ?? t('Library_InvalidEntry'))
        return null
      }
      return read.entries.map(toImportCandidate)
    } catch (e) {
      useToasts.getState().show(String(e instanceof Error ? e.message : e))
      return null
    }
  },

  confirmImport: async (candidates) => {
    // `existing` tracks BOTH the current library AND ids minted/saved earlier in
    // this same batch (codex F3): two package entries sharing an id both get
    // import-as-copy, instead of the second failing with `exists`.
    const existing = new Set(get().entries.map((e) => e.id))
    let imported = 0
    for (const c of candidates) {
      if (!c.save) continue
      // Import-as-copy (spec 09 §5): an id collision mints a fresh id and marks
      // the name; replacing is never silent.
      const entry = existing.has(c.save.id)
        ? {
            ...c.save,
            id: crypto.randomUUID(),
            meta: { ...c.save.meta, name: `${c.save.meta.name}${t('Library_ImportedSuffix')}` },
          }
        : c.save
      try {
        await call('presets.save', { entry, overwrite: false })
        existing.add(entry.id)
        imported++
      } catch (e) {
        useToasts.getState().show(`${entry.meta.name}: ${String(e instanceof Error ? e.message : e)}`)
      }
    }
    if (imported > 0) await get().refresh()
    return imported
  },

  deleteEntry: async (id) => {
    try {
      await call('presets.delete', { entryId: id })
      listGen++ // invalidate any refresh in flight that predates this delete
      set({ entries: get().entries.filter((e) => e.id !== id) })
    } catch (e) {
      useToasts.getState().show(String(e instanceof Error ? e.message : e))
    }
  },

  renameEntry: async (id, name) => {
    const trimmed = name.trim()
    if (!trimmed) return
    try {
      await call('presets.rename', { entryId: id, name: trimmed })
      listGen++ // invalidate any refresh in flight that predates this rename
      set({ entries: get().entries.map((e) => (e.id === id ? { ...e, name: trimmed } : e)) })
    } catch (e) {
      useToasts.getState().show(String(e instanceof Error ? e.message : e))
    }
  },

  exportTo: async (destPath, entries) => {
    try {
      const path = await call('presets.export', { destPath, entries })
      useToasts.getState().show(`${t('Toast_PresetExported')} ${path}`)
      return true
    } catch (e) {
      useToasts.getState().show(String(e instanceof Error ? e.message : e))
      return false
    }
  },
}))
