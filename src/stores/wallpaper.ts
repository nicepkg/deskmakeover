import { create } from 'zustand'
import { call } from '@/bridge/client'
import type { FontChoiceDto, LookDto, WallpaperResultDto, WallpaperSourceDto, WallpaperStateDto, ZoneDto } from '@/bridge/types'
import { decodeWallpaperResult, decodeWallpaperScreens } from '@/bridge/wallpaper-decode'
import { type ScreenLook, mergeScreenMap } from '@/lib/monitor-reconcile'
import { assembleWallpaperState, loadPersistedLooks, lookDirty, savePersistedLook } from '@/lib/wallpaper-assemble'
import { activeScreenSourceUrl } from '@/lib/screen-arrange'
import { getCompositor } from '@/compositor/registry'
import { format, t } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'
import { useToasts } from '@/stores/toasts'

// Wallpaper-module state (spec 04 v2.0, ADR-0014; multi-monitor §B2). The WEB owns
// the working look + its rendering; the host only persists per-monitor + bakes the
// PNG on apply (apply/restore only from explicit clicks). Truth = `screens:
// Record<monitorId, ScreenLook>` + `activeScreenId`, each screen owning its own draft
// + selection + undo. Top-level fields MIRROR the active screen (single-monitor parity).

const HISTORY_LIMIT = 100
/** Debounce for persisting the look to the host (pure persistence, not render). */
const PERSIST_DEBOUNCE_MS = 400

interface WallpaperState {
  loaded: boolean
  state: WallpaperStateDto | null
  /** Per-monitor runtime state (source of truth). */
  screens: Record<string, ScreenLook>
  /** The monitor currently being edited; the top-level mirror follows it. */
  activeScreenId: string | null
  /** Which screen's source the compositor currently HOLDS (mirrored by the compositor hook). On a
   *  same-dims screen switch the source swap is async, so this lags `activeScreenId` for a beat;
   *  `apply()` gates on the two matching so a mid-swap apply can't bake screen A's pixels and submit
   *  them under screen B's monitorId (codex #5). Null before the first load / during a recreate. */
  loadedScreenId: string | null

  // ---- active-screen mirror (UI back-compat + single-monitor parity) ----
  look: LookDto | null
  selected: string | null
  /** Imported source name (壁纸导入); null = the current desktop wallpaper. */
  sourceName: string | null
  /** Object URL of the imported source (compare view + preset thumbs); null = originalUrl. */
  sourceUrl: string | null
  past: LookDto[]
  future: LookDto[]
  canUndo: boolean
  canRedo: boolean

  comparing: boolean
  /** Baking the native-resolution PNG during apply. */
  applying: boolean
  /** Increments on apply success — the canvas plays the 分区落版 wave. */
  applyWave: number
  fonts: FontChoiceDto[]

  load: () => Promise<void>
  /** Re-fetch getScreens + re-assemble (preserving drafts) — the fingerprint-mismatch
   *  regenerate affordance and any external re-sync. */
  refresh: () => Promise<void>
  /** Switch the active monitor; the active-screen mirror + compositor follow it. */
  selectScreen: (monitorId: string) => void
  mutateLook: (change: (look: LookDto) => LookDto, coalesce?: string) => void
  mutateZone: (id: string, change: (zone: ZoneDto) => ZoneDto, coalesce?: string) => void
  addZone: (zone: ZoneDto) => void
  duplicateZone: (id: string, rect: Pick<ZoneDto, 'cellX' | 'cellY'>) => string | null
  removeZone: (id: string) => void
  select: (id: string | null) => void
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
// A cold `load()` in flight — so `loaded` can latch only on SUCCESS (see load())
// without a concurrent second call double-fetching in the gap before it settles.
let loadInFlight = false
// Continuous PANEL inputs (typing a title, dragging a slider) coalesce like
// canvas gestures: same coalesce key within the window = one undo step
// (codex review M4: per-keystroke snapshots burned the 100-entry history).
let lastCoalesceKey: string | null = null
let lastCoalesceAt = 0
const COALESCE_WINDOW_MS = 1200

function toastOf(dto: WallpaperResultDto): void {
  if (dto.toast) {
    useToasts.getState().show(t(dto.toast.key as StringKey), dto.ok ? 'info' : 'warn')
  }
}

function clampSelected(selected: string | null, look: LookDto): string | null {
  if (selected === null) return null
  return look.zones.some((z) => z.id === selected) ? selected : null
}

/** Dirty if ANY screen carries a non-empty draft (single screen → old behavior). */
function anyScreenDirty(screens: Record<string, ScreenLook>): boolean {
  return Object.values(screens).some((s) => lookDirty(s.look))
}

/** Project a ScreenLook to the top-level mirror fields UI consumers read (the one
 *  place the active-screen → top-level mapping lives). */
function mirror(s: ScreenLook) {
  return {
    look: s.look,
    selected: s.selected,
    sourceName: s.sourceName,
    sourceUrl: s.sourceUrl,
    past: s.past,
    future: s.future,
    canUndo: s.past.length > 0,
    canRedo: s.future.length > 0,
  }
}

export const useWallpaper = create<WallpaperState>((set, get) => {
  /** Read the active screen's runtime record (null before load / no monitor). */
  const active = (): { id: string; screen: ScreenLook } | null => {
    const { activeScreenId, screens } = get()
    if (!activeScreenId) return null
    const screen = screens[activeScreenId]
    return screen ? { id: activeScreenId, screen } : null
  }

  /** The single funnel: write a partial into the active screen's record AND
   *  project the mirrored fields to the top level (keeps both coherent). */
  const setActive = (partial: Partial<ScreenLook>): void => {
    const a = active()
    if (!a) return
    const next: ScreenLook = { ...a.screen, ...partial }
    set({ screens: { ...get().screens, [a.id]: next }, ...mirror(next) })
  }

  /** Repaint now + debounce-persist the active screen's look, keyed by monitorId
   *  (captured so a mid-debounce screen switch still persists the RIGHT monitor).
   *  `extra` merges more active fields (e.g. the new selection) in one write. */
  const commit = (look: LookDto, extra?: Partial<ScreenLook>): void => {
    const a = active()
    if (!a) return
    setActive({ look, ...extra })
    const screens = get().screens
    const state = get().state
    if (state) {
      set({
        state: {
          ...state,
          dirty: anyScreenDirty(screens),
          look,
          screens: state.screens.map((s) => (s.monitorId === a.id ? { ...s, look } : s)),
        },
      })
    }
    getCompositor()?.update(look)
    // Persist the draft to localStorage (D1: setLook left the bridge), keyed by
    // monitorId + captured with its bounds fingerprint (both captured so a mid-debounce
    // screen switch still persists the RIGHT monitor). A screen with no known bounds
    // (the 0-monitor virtual screen) is session-only — nothing to reconcile back to.
    if (persistTimer) clearTimeout(persistTimer)
    const monitorId = a.id
    const bounds = get().state?.screens.find((s) => s.monitorId === monitorId)?.bounds
    persistTimer = setTimeout(() => {
      persistTimer = null
      if (bounds) savePersistedLook(monitorId, look, bounds)
    }, PERSIST_DEBOUNCE_MS)
  }

  const snapshot = (): void => {
    const a = active()
    if (!a) return
    const past = [...a.screen.past, structuredClone(a.screen.look)]
    if (past.length > HISTORY_LIMIT) past.shift()
    setActive({ past, future: [] })
  }

  const maybeSnapshot = (coalesce?: string): void => {
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

  /** Re-fetch the thin getScreens payload and re-assemble the store state, preserving
   *  every in-progress per-screen draft + undo stack (mergeScreenMap keeps live
   *  ScreenLook refs). `hasBackup` now rides getScreens itself (the host's durable
   *  snapshot truth), so it is correct on cold start AND after every mutating op. */
  const syncFromHost = async (): Promise<void> => {
    const dto = decodeWallpaperScreens(await call('wallpaper.getScreens'))
    const persisted = loadPersistedLooks()
    const state = assembleWallpaperState(dto, persisted, get().screens, {
      prevActiveId: get().activeScreenId,
    })
    let screens = mergeScreenMap(get().screens, state.screens)
    let activeId = state.activeScreenId
    // 0-monitor host: keep the store from crash-emptying with one virtual screen from
    // the top-level mirror (spec 04 §B6; the virtual-screen UI is Task 2).
    if (Object.keys(screens).length === 0) {
      activeId = activeId || 'virtual-screen'
      screens = {
        [activeId]: { look: state.look, source: null, sourceName: null, sourceUrl: null, selected: null, past: [], future: [] },
      }
    }
    set({ state, screens, activeScreenId: activeId, ...mirror(screens[activeId]) })
  }

  return {
    loaded: false,
    state: null,
    screens: {},
    activeScreenId: null,
    loadedScreenId: null,
    look: null,
    selected: null,
    sourceName: null,
    sourceUrl: null,
    past: [],
    future: [],
    canUndo: false,
    canRedo: false,
    comparing: false,
    applying: false,
    applyWave: 0,
    fonts: [],

    load: async () => {
      // Latch `loaded` only AFTER a successful sync (codex #4): a transient first-run
      // failure — COM init, a temporary adapter error — must stay retryable, not
      // permanently no-op the wallpaper page for the process lifetime. The in-flight
      // guard keeps a concurrent second call (StrictMode double-mount) from double-fetching.
      if (get().loaded || loadInFlight) return
      loadInFlight = true
      try {
        // Cold load: getScreens now reports whether a durable snapshot persists, so the
        // whole-desktop restore affordance is correct across a restart (schema 6 D1).
        await syncFromHost()
        set({ loaded: true })
      } finally {
        loadInFlight = false
      }
    },

    refresh: async () => {
      await syncFromHost()
    },

    selectScreen: (monitorId) => {
      const { screens, activeScreenId, state } = get()
      const target = screens[monitorId]
      if (!target || monitorId === activeScreenId) return
      // Cross-screen coalescing must not bleed: a slider run on screen A cannot
      // coalesce into screen B's history.
      lastCoalesceKey = null
      const dtoScreen = state?.screens.find((s) => s.monitorId === monitorId)
      set({
        activeScreenId: monitorId,
        ...mirror(target),
        state:
          state && dtoScreen
            ? { ...state, activeScreenId: monitorId, look: target.look, grid: dtoScreen.grid, originalUrl: dtoScreen.source?.url ?? null }
            : state,
      })
      getCompositor()?.update(target.look)
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
      commit({ ...look, zones: [...look.zones, zone] }, { selected: zone.id })
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
      commit({ ...look, zones: [...look.zones, copy] }, { selected: copy.id })
      return copy.id
    },

    removeZone: (id) => {
      const look = get().look
      const victim = look?.zones.find((z) => z.id === id)
      if (!look || !victim) return
      maybeSnapshot()
      // Delete clears selection deliberately (spec 04 §3.5) — not a side effect.
      commit({ ...look, zones: look.zones.filter((z) => z.id !== id) }, { selected: null })
      // Aesthetics-first users won't guess Ctrl+Z — hand them the undo (spec 04 §3).
      useToasts.getState().show(format(t('Zone_DeletedToast'), victim.title), 'info', {
        label: t('History_Undo'),
        run: () => get().undo(),
      })
    },

    select: (id) => setActive({ selected: id }),

    /** 导入壁纸: design on a picked image (client-side; apply bakes it in). Per-screen. */
    importSource: async (file) => {
      const compositor = getCompositor()
      if (!compositor) return false
      try {
        const bitmap = await createImageBitmap(file)
        const prev = get().sourceUrl
        compositor.setSource({ bitmap, width: bitmap.width, height: bitmap.height })
        // A new source IS a change worth applying/exporting, even with 0 zones.
        setActive({ sourceName: file.name, sourceUrl: URL.createObjectURL(file) })
        const state = get().state
        if (state) set({ state: { ...state, dirty: true } })
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

    /** Drop the import → the ACTIVE screen's OWN desktop wallpaper, resolved from the
     *  per-screen store (activeScreenSourceUrl), NOT getSource (would hit the wrong monitor). */
    resetSource: async () => {
      const compositor = getCompositor()
      if (!compositor) return
      const url = activeScreenSourceUrl(get().screens, get().activeScreenId)
      if (url) {
        const response = await fetch(url)
        const bitmap = await createImageBitmap(await response.blob())
        compositor.setSource({ bitmap, width: bitmap.width, height: bitmap.height })
      }
      const prev = get().sourceUrl
      setActive({ sourceName: null, sourceUrl: null })
      const state = get().state
      if (state) set({ state: { ...state, dirty: anyScreenDirty(get().screens) } })
      if (prev) URL.revokeObjectURL(prev)
    },

    /** 导出壁纸: bake the current look at native res and save the PNG locally
     *  (a plain image — the receiver needs no app). Never touches the desktop. */
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
      commit({ ...look, zones }, { selected: null })
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
      const a = active()
      if (!a || a.screen.past.length === 0) return
      const previous = a.screen.past[a.screen.past.length - 1]
      const newPast = a.screen.past.slice(0, -1)
      const newFuture = [structuredClone(a.screen.look), ...a.screen.future]
      commit(previous, { past: newPast, future: newFuture, selected: clampSelected(a.screen.selected, previous) })
    },

    redo: () => {
      const a = active()
      if (!a || a.screen.future.length === 0) return
      const next = a.screen.future[0]
      const newFuture = a.screen.future.slice(1)
      const newPast = [...a.screen.past, structuredClone(a.screen.look)]
      commit(next, { past: newPast, future: newFuture, selected: clampSelected(a.screen.selected, next) })
    },

    apply: async () => {
      const { look, applying, activeScreenId, loadedScreenId } = get()
      const compositor = getCompositor()
      if (!look || !compositor || applying || !activeScreenId) return false
      // The compositor's loaded source must be the ACTIVE screen's, or a bake started mid
      // screen-switch (same-dims swaps load asynchronously) would submit screen A's pixels under
      // screen B's monitorId (codex #5). Refuse for the beat it takes the swap to land, rather
      // than apply the wrong monitor — the seamless visual switch itself is unaffected.
      if (loadedScreenId !== activeScreenId) {
        useToasts.getState().show(t('Paper_ApplyScreenSyncing'), 'warn')
        return false
      }
      set({ applying: true })
      // Keep the CTA shimmer for one calm beat even when the mock returns instantly.
      const started = Date.now()
      try {
        const blob = await compositor.bake()
        const pngBase64 = await blobToBase64(blob)
        // Thin result (schema 6): host bakes the desktop + reports hasBackup, NO state.
        const result = decodeWallpaperResult(await call('wallpaper.applyBaked', { monitorId: activeScreenId, pngBase64 }))
        const wait = 550 - (Date.now() - started)
        if (wait > 0) await new Promise((r) => setTimeout(r, wait))
        // Re-fetch getScreens + re-assemble; the live per-screen drafts survive (they
        // are kept by reference), and the re-fetched getScreens carries the now-true
        // hasBackup (the apply just captured the snapshot). Then fire the wave.
        if (result.ok) {
          await syncFromHost()
          set({ applyWave: get().applyWave + 1 })
        }
        toastOf(result)
        return result.ok
      } catch {
        return false
      } finally {
        set({ applying: false })
      }
    },

    restore: async () => {
      // Whole-desktop restore reverts the pre-first-apply snapshot (§B5); draft looks
      // survive, so dirty re-derives from them on the getScreens re-assemble.
      const result = decodeWallpaperResult(await call('wallpaper.restore', { monitorId: 'all' }))
      await syncFromHost()
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

/** Test seam: single-screen store seed (parity with the pre-multi-monitor tests). */
export function singleScreenSeed(monitorId: string, look: LookDto, source: WallpaperSourceDto | null = null): {
  screens: Record<string, ScreenLook>
  activeScreenId: string
} {
  return { screens: { [monitorId]: { look, source, sourceName: null, sourceUrl: null, selected: null, past: [], future: [] } }, activeScreenId: monitorId }
}

export type { ScreenLook }
