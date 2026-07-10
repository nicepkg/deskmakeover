import { create } from 'zustand'
import { call } from '@/bridge/client'
import type { FontChoiceDto, LookDto, WallpaperOpDto, WallpaperStateDto, ZoneDto } from '@/bridge/types'
import { getCompositor } from '@/compositor/registry'
import { format, t } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'
import { useToasts } from '@/stores/toasts'

// Wallpaper-module state (spec 04 v2.0, ADR-0014): the WEB owns the working look
// AND its rendering — the client compositor repaints on every look change (no
// host round-trip, no debounce). The host is only asked to persist the look and,
// on apply, to write the baked PNG. apply/restore run ONLY from explicit user
// clicks (owner gate).
//
// Reversibility: a session undo/redo stack over `look` snapshots. Continuous
// canvas gestures (move / resize) coalesce into ONE snapshot via
// begin/endInteraction so a single drag is one undo step, not one-per-frame.

const HISTORY_LIMIT = 100
/** Debounce for persisting the look to the host (pure persistence, not render). */
const PERSIST_DEBOUNCE_MS = 400

interface WallpaperState {
  loaded: boolean
  state: WallpaperStateDto | null
  look: LookDto | null
  selected: string | null
  comparing: boolean
  /** Baking the native-resolution PNG during apply. */
  applying: boolean
  /** Increments on apply success — the canvas plays the 分区落版 wave. */
  applyWave: number
  fonts: FontChoiceDto[]

  past: LookDto[]
  future: LookDto[]
  canUndo: boolean
  canRedo: boolean

  load: () => Promise<void>
  mutateLook: (change: (look: LookDto) => LookDto, coalesce?: string) => void
  mutateZone: (id: string, change: (zone: ZoneDto) => ZoneDto, coalesce?: string) => void
  addZone: (zone: ZoneDto) => void
  duplicateZone: (id: string, rect: Pick<ZoneDto, 'cellX' | 'cellY'>) => string | null
  removeZone: (id: string) => void
  select: (id: string | null) => void
  /** Imported source name (壁纸导入); null = the user's current desktop wallpaper. */
  sourceName: string | null
  /** Object URL of the imported source (compare view + preset thumbs); null = originalUrl. */
  sourceUrl: string | null
  importSource: (file: File) => Promise<boolean>
  /** Opens the native file picker → importSource (shared by header/empty/drop). */
  importSourceViaPicker: () => void
  resetSource: () => Promise<void>
  /** Bake + save locally; resolves the saved filename, null on failure. */
  exportImage: () => Promise<string | null>
  applyToAllZones: (patch: Partial<Pick<ZoneDto, 'tone' | 'material' | 'titleStyle' | 'shadow' | 'fillOpacity' | 'cornerRadius' | 'titleSize' | 'fontFamily'>>) => void
  replaceZones: (zones: ZoneDto[]) => void
  beginInteraction: () => void
  endInteraction: () => void
  undo: () => void
  redo: () => void
  /** Resolves true only when THIS apply succeeded (gates the DoneCard). */
  apply: () => Promise<boolean>
  restore: () => Promise<void>
  setComparing: (comparing: boolean) => void
  loadFonts: () => Promise<void>
}

/** New-zone factory: stable id + Adaptive Frost defaults (spec 04 §2.4). */
export function makeZone(partial: Partial<ZoneDto> & Pick<ZoneDto, 'cellX' | 'cellY' | 'cellsWide' | 'cellsTall' | 'title'>): ZoneDto {
  return {
    id: crypto.randomUUID(),
    emoji: null,
    accent: null,
    tone: 'Auto',
    material: 'Frost',
    titleStyle: 'Chip',
    shadow: false,
    fillOpacity: null,
    cornerRadius: 20,
    titleSize: 'M',
    fontFamily: null,
    ...partial,
  }
}

let persistTimer: ReturnType<typeof setTimeout> | null = null
let interactionOpen = false
let snapshotTakenThisGesture = false
// Continuous PANEL inputs (typing a title, dragging a slider) coalesce like
// canvas gestures: same coalesce key within the window = one undo step
// (codex review M4: per-keystroke snapshots burned the 100-entry history).
let lastCoalesceKey: string | null = null
let lastCoalesceAt = 0
const COALESCE_WINDOW_MS = 1200

function toastOf(dto: WallpaperOpDto): void {
  if (dto.toast) {
    useToasts.getState().show(t(dto.toast.key as StringKey), dto.ok ? 'info' : 'warn')
  }
}

function clampSelected(selected: string | null, look: LookDto): string | null {
  if (selected === null) return null
  return look.zones.some((z) => z.id === selected) ? selected : null
}

export const useWallpaper = create<WallpaperState>((set, get) => {
  /** Repaint now (client compositor) + debounce-persist the look to the host. */
  const commit = (look: LookDto) => {
    const state = get().state
    const dirty = look.zones.length > 0 || look.clarity.level !== 'Off'
    set({ look, state: state ? { ...state, dirty } : state })
    getCompositor()?.update(look)
    if (persistTimer) clearTimeout(persistTimer)
    persistTimer = setTimeout(() => {
      persistTimer = null
      void call('wallpaper.setLook', { look: get().look! }).catch(() => {})
    }, PERSIST_DEBOUNCE_MS)
  }

  const snapshot = () => {
    const look = get().look
    if (!look) return
    const past = [...get().past, structuredClone(look)]
    if (past.length > HISTORY_LIMIT) past.shift()
    set({ past, future: [], canUndo: true, canRedo: false })
  }

  const maybeSnapshot = (coalesce?: string) => {
    if (interactionOpen) {
      if (!snapshotTakenThisGesture) {
        snapshot()
        snapshotTakenThisGesture = true
      }
      return
    }
    const now = Date.now()
    if (coalesce && coalesce === lastCoalesceKey && now - lastCoalesceAt < COALESCE_WINDOW_MS) {
      lastCoalesceAt = now // keep the run alive while the user keeps editing
      return
    }
    snapshot()
    lastCoalesceKey = coalesce ?? null
    lastCoalesceAt = now
  }

  return {
    loaded: false,
    state: null,
    look: null,
    selected: null,
    comparing: false,
    applying: false,
    applyWave: 0,
    fonts: [],
    past: [],
    future: [],
    canUndo: false,
    canRedo: false,

    load: async () => {
      if (get().loaded) return
      set({ loaded: true })
      const state = await call('wallpaper.getState')
      set({ state, look: state.look })
    },

    mutateLook: (change, coalesce) => {
      const look = get().look
      if (!look) return
      maybeSnapshot(coalesce)
      commit(change(look))
    },

    mutateZone: (id, change, coalesce) => {
      const look = get().look
      if (!look) return
      const index = look.zones.findIndex((z) => z.id === id)
      if (index < 0) return
      maybeSnapshot(coalesce ? `${coalesce}:${id}` : undefined)
      commit({ ...look, zones: look.zones.map((z, i) => (i === index ? change(z) : z)) })
    },

    addZone: (zone) => {
      const look = get().look
      if (!look) return
      maybeSnapshot()
      commit({ ...look, zones: [...look.zones, zone] })
      set({ selected: zone.id })
    },

    duplicateZone: (id, rect) => {
      const look = get().look
      const source = look?.zones.find((z) => z.id === id)
      if (!look || !source) return null
      maybeSnapshot()
      const copy: ZoneDto = {
        ...structuredClone(source),
        id: crypto.randomUUID(),
        title: `${source.title} ${t('Zone_CopySuffix')}`,
        ...rect,
      }
      commit({ ...look, zones: [...look.zones, copy] })
      set({ selected: copy.id })
      return copy.id
    },

    removeZone: (id) => {
      const look = get().look
      const victim = look?.zones.find((z) => z.id === id)
      if (!look || !victim) return
      maybeSnapshot()
      // Delete clears selection deliberately (spec 04 §3.5) — not a side effect.
      commit({ ...look, zones: look.zones.filter((z) => z.id !== id) })
      set({ selected: null })
      // Aesthetics-first users won't guess Ctrl+Z — hand them the undo (spec 04 §3).
      useToasts.getState().show(format(t('Zone_DeletedToast'), victim.title), 'info', {
        label: t('History_Undo'),
        run: () => get().undo(),
      })
    },

    select: (id) => set({ selected: id }),

    sourceName: null,
    sourceUrl: null,

    /** 导入壁纸: design on a picked image instead of the current desktop
     *  wallpaper. Purely client-side — the source never crosses the bridge;
     *  apply bakes it into the PNG like any other source. */
    importSource: async (file) => {
      const compositor = getCompositor()
      if (!compositor) return false
      try {
        const bitmap = await createImageBitmap(file)
        const prev = get().sourceUrl
        compositor.setSource({ bitmap, width: bitmap.width, height: bitmap.height })
        const state = get().state
        set({
          sourceName: file.name,
          sourceUrl: URL.createObjectURL(file),
          // A new source IS a change worth applying/exporting, even with 0 zones.
          state: state ? { ...state, dirty: true } : state,
        })
        if (prev) URL.revokeObjectURL(prev)
        return true
      } catch {
        return false
      }
    },

    importSourceViaPicker: () => {
      const input = document.createElement('input')
      input.type = 'file'
      input.accept = 'image/*'
      input.onchange = async () => {
        const file = input.files?.[0]
        if (!file) return
        const ok = await get().importSource(file)
        // Success needs no toast — the wallpaper visibly changes (预览即所得).
        if (!ok) useToasts.getState().show(t('Toast_ImportFailed'), 'warn')
      }
      input.click()
    },

    /** Back to the user's current desktop wallpaper as the design source. */
    resetSource: async () => {
      const compositor = getCompositor()
      if (!compositor) return
      const source = await call('wallpaper.getSource')
      const response = await fetch(source.url)
      const bitmap = await createImageBitmap(await response.blob())
      compositor.setSource({ bitmap, width: bitmap.width, height: bitmap.height })
      const prev = get().sourceUrl
      const state = get().state
      const look = get().look
      const dirty = !!look && (look.zones.length > 0 || look.clarity.level !== 'Off')
      set({ sourceName: null, sourceUrl: null, state: state ? { ...state, dirty } : state })
      if (prev) URL.revokeObjectURL(prev)
    },

    /** 导出壁纸: bake the current look at native res and save the PNG locally
     *  (a plain image — the receiver needs no app). Never touches the desktop.
     *  Browser/mock saves via download; the host gets a native Save dialog (F8,
     *  bridge `wallpaper.exportPng`). */
    exportImage: async () => {
      const compositor = getCompositor()
      if (!compositor) return null
      try {
        const blob = await compositor.bake()
        const filename = `桌面壁纸_${new Date().toISOString().slice(0, 10)}.png`
        const url = URL.createObjectURL(blob)
        const a = document.createElement('a')
        a.href = url
        a.download = filename
        a.click()
        setTimeout(() => URL.revokeObjectURL(url), 10_000)
        return filename
      } catch {
        return null
      }
    },

    // The ONLY mass-edit path: explicit 应用到全部分区 (spec 04 §2.4).
    applyToAllZones: (patch) => {
      const look = get().look
      if (!look || look.zones.length === 0) return
      maybeSnapshot()
      commit({ ...look, zones: look.zones.map((z) => ({ ...z, ...patch })) })
    },

    /** Preset gallery / recommended layout entry — replaces every zone. */
    replaceZones: (zones) => {
      const look = get().look
      if (!look) return
      maybeSnapshot()
      commit({ ...look, zones })
      set({ selected: null })
    },

    beginInteraction: () => {
      interactionOpen = true
      snapshotTakenThisGesture = false
    },
    endInteraction: () => {
      interactionOpen = false
      snapshotTakenThisGesture = false
    },

    undo: () => {
      const { past, look } = get()
      if (past.length === 0 || !look) return
      const previous = past[past.length - 1]
      const newPast = past.slice(0, -1)
      set({
        past: newPast,
        future: [structuredClone(look), ...get().future],
        canUndo: newPast.length > 0,
        canRedo: true,
        selected: clampSelected(get().selected, previous),
      })
      commit(previous)
    },

    redo: () => {
      const { future, look } = get()
      if (future.length === 0 || !look) return
      const next = future[0]
      const newFuture = future.slice(1)
      set({
        past: [...get().past, structuredClone(look)],
        future: newFuture,
        canUndo: true,
        canRedo: newFuture.length > 0,
        selected: clampSelected(get().selected, next),
      })
      commit(next)
    },

    apply: async () => {
      const { look, applying } = get()
      const compositor = getCompositor()
      if (!look || !compositor || applying) return false
      set({ applying: true })
      // Guarantee the CTA's applying shimmer plays at least one calm beat even
      // when the mock write returns instantly (mirrors CanvasProgress' linger).
      const started = Date.now()
      try {
        const blob = await compositor.bake()
        const pngBase64 = await blobToBase64(blob)
        const result = await call('wallpaper.applyBaked', { pngBase64, look })
        const wait = 550 - (Date.now() - started)
        if (wait > 0) await new Promise((r) => setTimeout(r, wait))
        set({ state: { ...result.state, look: get().look ?? result.state.look } })
        toastOf(result)
        if (result.ok) set({ applyWave: get().applyWave + 1 })
        return result.ok
      } catch {
        return false
      } finally {
        set({ applying: false })
      }
    },

    restore: async () => {
      const result = await call('wallpaper.restore')
      const look = get().look
      // The draft look survives a desktop restore, so dirty derives from IT.
      const dirty = !!look && (look.zones.length > 0 || look.clarity.level !== 'Off')
      set({ state: { ...result.state, look: look ?? result.state.look, dirty } })
      toastOf(result)
    },

    setComparing: (comparing) => set({ comparing }),

    loadFonts: async () => {
      if (get().fonts.length === 0) {
        set({ fonts: await call('fonts.list') })
      }
    },
  }
})

async function blobToBase64(blob: Blob): Promise<string> {
  const buf = new Uint8Array(await blob.arrayBuffer())
  let binary = ''
  const CHUNK = 0x8000
  for (let i = 0; i < buf.length; i += CHUNK) {
    binary += String.fromCharCode(...buf.subarray(i, i + CHUNK))
  }
  return btoa(binary)
}
