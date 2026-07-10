import { create } from 'zustand'
import { call } from '@/bridge/client'
import type { ConfigDto, IconItemDto, IconsOpResultDto, IconsStateDto, IconKindBucket, KindPolicy, OverrideEntryDto, TypeOverrideEntry, TypeOverrides } from '@/bridge/types'
import { kindBucket, kindParticipates } from '@/lib/kind-policy'
import { appAccentSeed, resolveTypeConfig, typeHasFixedPlate, typeOverridesEqual } from '@/lib/type-config'
import { getIconCompositor } from '@/icon-compositor/icon-renderer'
import type { RenderOpts } from '@/icon-compositor/compose'
import type { SpreadEntry } from '@/icon-compositor/hue-spread'
import { computeHueSpread } from '@/icon-compositor/hue-spread'
import { format, t } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'
import { useToasts } from '@/stores/toasts'

// Icons-module state (icons contract v2, spec 06): the web compositor renders
// every preview locally — NOTHING crosses the bridge per edit. `setLook`
// persists config+overrides (400ms debounce); apply bakes 256px masters and
// streams them in chunks. Undo: one step per discrete pick; wheel drags
// coalesce per gesture (spec 06 §3.3). apply/restore run ONLY from explicit
// user clicks (owner gate).

const PERSIST_DEBOUNCE_MS = 400
const APPLY_CHUNK = 20

interface LookSnapshot {
  config: ConfigDto
  overrides: Map<string, { mode: 'keep' | 'tint'; tint: string | null }>
  kindPolicy: KindPolicy
  typeOverrides: TypeOverrides
}

interface IconsState {
  loaded: boolean
  state: IconsStateDto | null
  items: IconItemDto[]
  revision: number
  comparing: boolean
  zoom: number
  waveKind: 'bloom' | 'settle' | null
  waveStamp: number
  /** Hover try-on candidate — paints the whole desktop, never commits (spec 06 §3.2). */
  hoverConfig: ConfigDto | null
  /** The hovered candidate's type ladder (preset try-on carries its FULL
   *  look — config alone previewed the wrong plates, owner bug 2026-07-10);
   *  null = keep the committed ladder. */
  hoverTypeOverrides: TypeOverrides | null
  /** Bumped whenever tiles must re-render (source loaded / rescan / zoom settle). */
  renderTick: number
  /** First-paint gate: false until every dispatched tile has landed, so the
   *  desktop never shows wallpaper-with-blank-icons. Sticky true after the first
   *  quiesce (a later rescan re-renders in place, no re-veil). */
  ready: boolean
  applyProgress: { done: number; total: number } | null
  canUndo: boolean
  canRedo: boolean

  scan: () => Promise<void>
  rescan: () => Promise<void>
  mutate: (change: Partial<ConfigDto>) => void
  beginGesture: () => void
  endGesture: () => void
  hover: (change: Partial<ConfigDto> | null, typeOverrides?: TypeOverrides) => void
  selectPreset: (presetId: string) => void
  setOverride: (id: string, mode: 'keep' | 'tint' | 'follow', tint?: string) => void
  clearOverrides: () => void
  setKindPolicy: (bucket: IconKindBucket, styled: boolean) => void
  /** Per-type style source (ADR-0017): null/global clears the bucket's patch. */
  setTypeOverride: (bucket: IconKindBucket, entry: TypeOverrideEntry | null) => void
  /** One-click wipe of EVERY type's style override (owner 2026-07-10) —
   *  one undo step; kindPolicy (participation) is untouched. */
  resetTypeOverrides: () => void
  /** The type row currently expanded in the panel — the canvas scope-highlights
   *  this bucket's icons and dims the rest (ADR-0017 D5 hard requirement). */
  editingBucket: IconKindBucket | null
  setEditingBucket: (bucket: IconKindBucket | null) => void
  undo: () => void
  redo: () => void
  apply: () => Promise<boolean>
  restore: () => Promise<void>
  stageVersion: (index: number) => void
  exportCompare: () => Promise<void>
  setComparing: (comparing: boolean) => void
  setZoom: (zoom: number) => void
}

let persistTimer: ReturnType<typeof setTimeout> | null = null
let zoomSettleTimer: ReturnType<typeof setTimeout> | null = null
let undoStack: LookSnapshot[] = []
let redoStack: LookSnapshot[] = []
let gestureDepth = 0
let sourcesLoading = 0
let readyTimer: ReturnType<typeof setTimeout> | null = null

/** Await the tile-source load wave (rescan/first load) fully settling. */
async function sourcesSettled(): Promise<void> {
  while (sourcesLoading > 0) {
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
}
/** id -> hue-spread-adjusted Field seed hex (ADR-0016 D1; designer item 3). */
let fieldSeeds = new Map<string, string>()
/** id -> kind bucket (Field kind families + affordances + kindShapes, D2). */
let kindBuckets = new Map<string, 'App' | 'Folder' | 'File' | 'System' | null>()

/** Device-resolution tile render size (same clamp discipline as v1). */
export function displaySize(state: IconsStateDto | null, zoom: number): number {
  const iconPx = state?.grid.iconPx ?? 48
  const raw = Math.ceil(iconPx * zoom * window.devicePixelRatio)
  return Math.min(256, Math.max(24, Math.ceil(raw / 4) * 4))
}

/** The config a tile actually renders with. Cascade (spec 06 §6): styleable:false
 *  > per-icon override > kindPolicy default. A per-icon override always wins over
 *  the type policy — an icon the user styled individually stays styled even if its
 *  whole bucket is opted out. */
export function effectiveTileConfig(
  item: IconItemDto,
  config: ConfigDto,
  policy: KindPolicy,
  typeOverrides?: TypeOverrides,
): { config: ConfigDto; showOriginal: boolean } {
  if (!item.styleable || item.overrideMode === 'keep') return { config, showOriginal: true }
  // ADR-0017 composition invariant: style = resolve(bucket) + shortcut layer.
  // The type ladder resolves FIRST; the opt-in uniform shortcut shape rides on
  // top of it; per-icon overrides stay the highest styling authority below
  // styleable/keep.
  const typed = resolveTypeConfig(config, typeOverrides, kindBucket(item.kind))
  const resolved =
    item.isShortcut && typed.shortcutShape ? { ...typed, shape: typed.shortcutShape } : typed
  if (item.overrideMode === 'tint') {
    return { config: { ...resolved, subject: 'Mono', tint: item.overrideTint ?? resolved.tint }, showOriginal: false }
  }
  if (!kindParticipates(item.kind, policy)) return { config: resolved, showOriginal: true }
  return { config: resolved, showOriginal: false }
}

/** Compositor source key: the primary source keeps the plain item id (shared
 *  with the preview cache); secondary sources (Recycle Bin full) get `#i`. */
function bakeSourceId(id: string, sourceIndex: number): string {
  return sourceIndex === 0 ? id : `${id}#${sourceIndex}`
}

/** Per-icon Field inputs for renders AND bakes of this item (WYSIWYG: both
 *  paths read the same spread map; bakes key by the ITEM id so every source
 *  of an item shares one plate). */
export function fieldRenderOpts(id: string): RenderOpts {
  return { fieldSeed: fieldSeeds.get(id) ?? null, kindBucket: kindBuckets.get(id) ?? null }
}

function overridesOf(items: IconItemDto[]): OverrideEntryDto[] {
  return items
    .filter((i) => i.overrideMode !== null)
    .map((i) => ({ id: i.id, mode: i.overrideMode as 'keep' | 'tint', tint: i.overrideTint }))
}

function toastOf(dto: IconsOpResultDto): void {
  if (dto.toast) {
    const text = dto.toast.arg
      ? format(t(dto.toast.key as StringKey), dto.toast.arg)
      : t(dto.toast.key as StringKey)
    useToasts.getState().show(text, dto.ok ? 'info' : 'warn')
  }
}

const PRESET_NAME_KEYS: Record<string, StringKey> = {
  spectrum: 'Preset_spectrum',
  stationery: 'Preset_stationery',
  glass: 'Preset_glass',
  pebble: 'Preset_pebble',
  ink: 'Preset_ink',
  white: 'Preset_white',
  ascast: 'Preset_ascast',
}

/**
 * Client-side active-preset derivation (v2: config is WEB truth — the host
 * only refreshes its own activePresetId on scan/apply, far too late for the
 * selection highlight). Matching rule mirrors the engine: shape + colorMode +
 * filter + distinction, tint only when Mono.
 */
export function activePresetIdOf(state: IconsStateDto): string | null {
  const c = state.config
  for (const p of state.presets) {
    if (
      p.config.shape === c.shape &&
      p.config.subject === c.subject &&
      p.config.filter === c.filter &&
      p.config.distinction === c.distinction &&
      typeOverridesEqual(p.typeOverrides, state.typeOverrides) &&
      (p.config.shortcutShape ?? null) === (c.shortcutShape ?? null) &&
      (p.config.plateColor ?? null) === (c.plateColor ?? null) &&
      p.config.plateFallback === c.plateFallback &&
      (p.config.plateColor !== null || p.config.plateBand === c.plateBand) &&
      (p.config.subject !== 'Mono' || p.config.monoStyle === c.monoStyle) &&
      (p.config.subject !== 'Mono' || p.config.tint.toUpperCase() === c.tint.toUpperCase())
    ) {
      return p.id
    }
  }
  return null
}

function lookLabel(state: IconsStateDto): string {
  const id = activePresetIdOf(state)
  if (id && PRESET_NAME_KEYS[id]) return t(PRESET_NAME_KEYS[id])
  return `${t('History_Custom')} · ${t(('Shape_' + state.config.shape) as StringKey)}`
}

export const useIcons = create<IconsState>((set, get) => {
  const snapshot = (): LookSnapshot => {
    const s = get()
    return {
      config: { ...s.state!.config },
      overrides: new Map(
        s.items.filter((i) => i.overrideMode !== null).map((i) => [i.id, { mode: i.overrideMode as 'keep' | 'tint', tint: i.overrideTint }]),
      ),
      kindPolicy: { ...s.state!.kindPolicy },
      typeOverrides: structuredClone(s.state!.typeOverrides),
    }
  }

  const restoreSnapshot = (snap: LookSnapshot) => {
    const s = get()
    if (!s.state) return
    set({
      state: { ...s.state, config: { ...snap.config }, kindPolicy: { ...snap.kindPolicy }, typeOverrides: structuredClone(snap.typeOverrides) },
      items: s.items.map((i) => {
        const o = snap.overrides.get(i.id)
        return { ...i, overrideMode: o?.mode ?? null, overrideTint: o?.tint ?? null }
      }),
      canUndo: undoStack.length > 0,
      canRedo: redoStack.length > 0,
    })
    schedulePersist()
  }

  const pushUndo = () => {
    if (gestureDepth > 0) return // gesture already snapshotted at pointer-down
    undoStack.push(snapshot())
    if (undoStack.length > 60) undoStack.shift()
    redoStack = []
    set({ canUndo: true, canRedo: false })
  }

  const schedulePersist = () => {
    if (persistTimer) clearTimeout(persistTimer)
    persistTimer = setTimeout(() => {
      persistTimer = null
      const s = get()
      if (!s.state) return
      void call('icons.setLook', { config: s.state.config, overrides: overridesOf(s.items), kindPolicy: s.state.kindPolicy, typeOverrides: s.state.typeOverrides })
    }, PERSIST_DEBOUNCE_MS)
  }

  const markDirty = (state: IconsStateDto): IconsStateDto =>
    state.applied ? { ...state, dirty: true } : state

  /** Once every source of the scan settled, run the deterministic hue-spread
   *  pass; if any plate moved versus the raw first paint, repaint once. */
  const recomputeHueSpread = () => {
    const compositor = getIconCompositor()
    const st = get().state
    // Pool membership (ADR-0017 D6): only icons whose RESOLVED colorMode is
    // Field and whose type pins no plate colour — a fixed plate is its own hue
    // authority, and demoted (Mono/BW) types have no hue to protect. The pool
    // shrinking just gives survivors more room.
    const entries: SpreadEntry[] = get()
      .items.filter((item) => {
        if (!st) return true
        const bucket = kindBuckets.get(item.id) ?? null
        if (typeHasFixedPlate(st.typeOverrides, bucket)) return false
        const r = resolveTypeConfig(st.config, st.typeOverrides, bucket)
        // Pool = derived-plate participants (ADR-0018): Original subject ×
        // null plate × 满彩 fallback. 本色 (white fallback) derives nothing.
        return r.subject === 'Original' && r.plateColor === null && r.plateFallback !== 'white'
      })
      .map((item) => ({
      id: item.id,
      artKey: item.sourceUrls[0] ?? item.id,
      seed: compositor.seedOf(item.id) ?? null,
    }))
    const next = computeHueSpread(entries)
    // Owner special case (ADR-0017): App-bucket icons whose artwork is
    // colourless (no rim seed) take a rotating brand-accent seed instead of
    // the neutral plate — a grey .exe must not sink into the file band.
    if (st) {
      for (const item of get().items) {
        const bucket = kindBuckets.get(item.id) ?? null
        if (bucket !== 'App' || next.has(item.id)) continue
        if (typeHasFixedPlate(st.typeOverrides, bucket)) continue
        const r = resolveTypeConfig(st.config, st.typeOverrides, bucket)
        if (r.subject !== 'Original' || r.plateColor !== null || r.plateFallback === 'white') continue
        if (compositor.seedOf(item.id) !== null) continue
        next.set(item.id, appAccentSeed(item.id))
      }
    }
    let changed = next.size !== fieldSeeds.size
    if (!changed) {
      for (const [k, v] of next) {
        if (fieldSeeds.get(k) !== v) {
          changed = true
          break
        }
      }
    }
    if (!changed) return
    fieldSeeds = next
    compositor.invalidateAll()
    set((s) => ({ renderTick: s.renderTick + 1 }))
  }

  const loadSources = (items: IconItemDto[]) => {
    const compositor = getIconCompositor()
    // The pool pings this rAF-coalesced listener whenever a tile bitmap lands;
    // one renderTick bump re-pulls every visible tile from the cache. It also
    // drives the first-paint gate: 130ms after the LAST land with the pool idle
    // and all sources decoded, the desktop is fully painted → lift the veil. The
    // debounce absorbs the re-pull→re-dispatch ripple so `ready` never flips early.
    compositor.setOnReady(() => {
      set((s) => ({ renderTick: s.renderTick + 1 }))
      if (get().ready) return
      if (readyTimer) clearTimeout(readyTimer)
      readyTimer = setTimeout(() => {
        readyTimer = null
        if (sourcesLoading !== 0 || compositor.pendingCount() !== 0) return
        // Late source retries (per-tile loadSource calls bypass the counter)
        // still land here on the next idle — keep the spread honest (codex #6).
        recomputeHueSpread()
        // Re-check idle after two frames: absorbs the render-lands → React-blits
        // lag (so no tile is still blank when the veil lifts) AND rejects a false
        // idle in a lull between waves — if work resumed, a later onReady retries.
        requestAnimationFrame(() =>
          requestAnimationFrame(() => {
            if (get().ready || sourcesLoading !== 0 || compositor.pendingCount() !== 0) return
            // First fully-painted frame of THIS launch — the desktop is already
            // visible (no veil); just mark ready. No first-paint bloom/wand.
            set({ ready: true })
          }),
        )
      }, 130)
    })
    for (const item of items) {
      const url = item.sourceUrls[0]
      if (!url || compositor.hasSource(item.id, url)) continue
      sourcesLoading++
      void compositor
        .loadSource(item.id, url)
        .catch(() => {})
        .finally(() => {
          sourcesLoading--
          if (sourcesLoading === 0) recomputeHueSpread()
          set((s) => ({ renderTick: s.renderTick + 1 }))
        })
    }
    if (sourcesLoading === 0) recomputeHueSpread() // all sources were cached
  }

  const adoptScan = (result: { revision: number; items: IconItemDto[]; state: IconsStateDto }) => {
    if (result.revision < get().revision) return
    kindBuckets = new Map(result.items.map((i) => [i.id, kindBucket(i.kind)]))
    getIconCompositor().invalidateAll()
    set({
      items: result.items,
      state: result.state,
      revision: result.revision,
      renderTick: get().renderTick + 1,
    })
    loadSources(result.items)
  }

  return {
    loaded: false,
    state: null,
    items: [],
    revision: 0,
    comparing: false,
    zoom: 1,
    waveKind: null,
    waveStamp: 0,
    hoverConfig: null,
    hoverTypeOverrides: null,
    editingBucket: null,
    renderTick: 0,
    ready: false,
    applyProgress: null,
    canUndo: false,
    canRedo: false,

    scan: async () => {
      if (get().loaded) return
      set({ loaded: true })
      adoptScan(await call('icons.scan'))
      // The first paint is gated by `ready` (loadSources): the desktop stays
      // veiled until EVERY tile has landed, so there is never a wallpaper-with-
      // blank-icons window. That first reveal (once per launch) sweeps the coral
      // wand + a per-tile bloom (ADR-0013 D8, reworked from the old comparing-hold
      // which showed originals that hadn't rendered yet).
      // Safety: never strand the veil if the pool never pings (e.g. 0 renderable
      // tiles) — force the reveal after a ceiling well past a full-desktop paint.
      setTimeout(() => {
        if (!get().ready) set({ ready: true })
      }, 2500)
    },

    rescan: async () => {
      adoptScan(await call('icons.scan'))
      useToasts.getState().show(t('Toast_Refreshed'))
    },

    // One undo step per discrete pick; repaint is local and immediate.
    mutate: (change) => {
      const s = get()
      if (!s.state) return
      pushUndo()
      set({ state: markDirty({ ...s.state, config: { ...s.state.config, ...change } }), hoverConfig: null })
      schedulePersist()
      // Pool membership follows the resolved configs (cheap; invalidates only
      // when the seed map actually moves).
      if ('colorMode' in change || 'plateColor' in change) recomputeHueSpread()
    },

    // Wheel/continuous gestures: ONE undo step per pointer-down→up (spec 06 §3.3).
    beginGesture: () => {
      if (gestureDepth === 0) {
        undoStack.push(snapshot())
        if (undoStack.length > 60) undoStack.shift()
        redoStack = []
        set({ canUndo: true, canRedo: false })
      }
      gestureDepth++
    },
    endGesture: () => {
      gestureDepth = Math.max(0, gestureDepth - 1)
    },

    // Hover try-on: repaint only — never history, never persistence.
    hover: (change, typeOverrides) => {
      const s = get()
      if (!s.state) return
      set({
        hoverConfig: change ? { ...s.state.config, ...change } : null,
        hoverTypeOverrides: change && typeOverrides !== undefined ? typeOverrides : null,
      })
    },

    selectPreset: (presetId) => {
      const s = get()
      const preset = s.state?.presets.find((p) => p.id === presetId)
      if (!preset || !s.state) return
      pushUndo()
      set({
        state: markDirty({ ...s.state, config: { ...preset.config }, typeOverrides: structuredClone(preset.typeOverrides) }),
        hoverConfig: null,
      })
      schedulePersist()
      recomputeHueSpread()
    },

    setOverride: (id, mode, tint) => {
      const s = get()
      pushUndo()
      set({
        items: s.items.map((i) =>
          i.id === id
            ? { ...i, overrideMode: mode === 'follow' ? null : mode, overrideTint: mode === 'tint' ? (tint ?? null) : null }
            : i,
        ),
      })
      schedulePersist()
    },

    clearOverrides: () => {
      const s = get()
      if (!s.items.some((i) => i.overrideMode !== null)) return
      pushUndo()
      set({ items: s.items.map((i) => ({ ...i, overrideMode: null, overrideTint: null })) })
      schedulePersist()
      useToasts.getState().show(t('Toast_ExceptionsCleared'))
    },

    // Type participation (spec 06 §6): ONE bucket switch governs manual apply AND
    // the future background auto-format. Persistent — survives when the desktop
    // has no icons of that kind. Per-icon overrides still win (cascade).
    setKindPolicy: (bucket, styled) => {
      const s = get()
      if (!s.state || s.state.kindPolicy[bucket] === styled) return
      pushUndo()
      set({ state: markDirty({ ...s.state, kindPolicy: { ...s.state.kindPolicy, [bucket]: styled } }) })
      schedulePersist()
    },

    setTypeOverride: (bucket, entry) => {
      const s = get()
      if (!s.state) return
      pushUndo()
      const next: TypeOverrides = { ...s.state.typeOverrides }
      if (!entry || entry.source === 'global' || !entry.patch || Object.keys(entry.patch).length === 0) {
        delete next[bucket]
      } else {
        next[bucket] = entry
      }
      set({ state: markDirty({ ...s.state, typeOverrides: next }), hoverConfig: null })
      schedulePersist()
      recomputeHueSpread()
    },

    resetTypeOverrides: () => {
      const s = get()
      if (!s.state || Object.keys(s.state.typeOverrides).length === 0) return
      pushUndo()
      set({ state: markDirty({ ...s.state, typeOverrides: {} }), hoverConfig: null })
      schedulePersist()
      recomputeHueSpread()
    },

    setEditingBucket: (bucket) => set({ editingBucket: bucket }),

    undo: () => {
      const prev = undoStack.pop()
      if (!prev) return
      redoStack.push(snapshot())
      restoreSnapshot(prev)
    },

    redo: () => {
      const next = redoStack.pop()
      if (!next) return
      undoStack.push(snapshot())
      restoreSnapshot(next)
    },

    // Bake 256 masters and stream in chunks. Apply is RESTORE-FIRST on the
    // host (spec 06 §2): 「保留原样」items need no master — the restore step
    // returns them to their originals. Multi-source items (Recycle Bin) bake
    // one master PER SOURCE (empty + full).
    apply: async () => {
      const s = get()
      if (!s.state) return false
      const compositor = getIconCompositor()
      const config = s.state.config
      const policy = s.state.kindPolicy
      const typeOverrides = s.state.typeOverrides
      // Bake only tiles that are actually beautified — styleable, not per-icon
      // kept, and their bucket participates (kindPolicy). showOriginal tiles are
      // RESTORE-FIRST, no master.
      const bakeSet = s.items.filter((i) => !effectiveTileConfig(i, config, policy, typeOverrides).showOriginal)
      const jobs = bakeSet.flatMap((item) =>
        item.sourceUrls.map((url, sourceIndex) => ({ item, url, sourceIndex })),
      )
      set({ state: { ...s.state, working: true }, applyProgress: { done: 0, total: jobs.length } })
      try {
        // Every source must be decoded BEFORE the count is advertised — an
        // apply during the initial load must never under-deliver (codex M5).
        await Promise.all(jobs.map((j) => compositor.loadSource(bakeSourceId(j.item.id, j.sourceIndex), j.url)))
        // Field WYSIWYG barrier (codex #3): the hue spread must be FINAL and
        // FROZEN for the whole bake — wait out any in-flight tile-source wave,
        // recompute once, then snapshot every item's opts so a mid-bake spread
        // change can never split the desktop across two colour worlds.
        await sourcesSettled()
        recomputeHueSpread()
        const frozenOpts = new Map(s.items.map((i) => [i.id, fieldRenderOpts(i.id)]))
        await call('icons.applyBakedBegin', { revision: s.revision, count: jobs.length })
        for (let at = 0; at < jobs.length; at += APPLY_CHUNK) {
          const chunk = jobs.slice(at, at + APPLY_CHUNK)
          const rendered: { id: string; sourceIndex: number; masterPng: string }[] = []
          for (const j of chunk) {
            const eff = effectiveTileConfig(j.item, config, policy, typeOverrides)
            const png = await compositor.bakeMasterPng(bakeSourceId(j.item.id, j.sourceIndex), eff.config, j.item.isShortcut, frozenOpts.get(j.item.id))
            if (!png) throw new Error(`master missing: ${j.item.id}#${j.sourceIndex}`)
            rendered.push({ id: j.item.id, sourceIndex: j.sourceIndex, masterPng: png })
          }
          await call('icons.applyBakedChunk', { items: rendered })
          set({ applyProgress: { done: Math.min(at + chunk.length, jobs.length), total: jobs.length } })
          // Yield a frame so the progress UI paints between chunks.
          await new Promise((resolve) => requestAnimationFrame(resolve))
        }
        const result = await call('icons.applyBakedCommit', {
          config,
          typeOverrides,
          overrides: overridesOf(s.items),
          label: lookLabel(s.state),
        })
        set({ state: result.state, applyProgress: null })
        toastOf(result)
        if (result.ok) set({ waveKind: 'bloom', waveStamp: Date.now() })
        return result.ok
      } catch {
        const cur = get()
        if (cur.state) set({ state: { ...cur.state, working: false }, applyProgress: null })
        useToasts.getState().show(t('Toast_ApplyFailed'), 'warn')
        return false
      }
    },

    restore: async () => {
      const s = get()
      if (!s.state) return
      set({ state: { ...s.state, working: true } })
      try {
        const result = await call('icons.restore')
        set({ state: result.state })
        toastOf(result)
        if (result.ok) set({ waveKind: 'settle', waveStamp: Date.now() })
      } catch {
        const cur = get()
        if (cur.state) set({ state: { ...cur.state, working: false } })
        useToasts.getState().show(t('Toast_ApplyFailed'), 'warn')
      }
    },

    // 回到此版 stages the entry's config; the panel then runs the SAME
    // ceremonied apply as any other real-desktop crossing (spec 06 §3.7).
    stageVersion: (index) => {
      const s = get()
      const entry = s.state?.history.find((h) => h.index === index)
      if (!entry || !s.state) return
      pushUndo()
      set({
        state: markDirty({ ...s.state, config: { ...entry.config }, typeOverrides: structuredClone(entry.typeOverrides) }),
        hoverConfig: null,
      })
      schedulePersist()
      recomputeHueSpread()
    },

    exportCompare: async () => {
      const result = await call('icons.exportCompare')
      set({ state: result.state })
      toastOf(result)
    },

    setComparing: (comparing) => set({ comparing }),

    setZoom: (zoom) => {
      const clamped = Math.min(3, Math.max(0.2, zoom))
      const previous = displaySize(get().state, get().zoom)
      set({ zoom: clamped })
      // The canvas scales existing bitmaps during the gesture (free); the crisp
      // re-render at the new device resolution fires after the gesture settles.
      const next = displaySize(get().state, clamped)
      if (zoomSettleTimer) clearTimeout(zoomSettleTimer)
      if (get().items.length > 0 && next !== previous) {
        zoomSettleTimer = setTimeout(() => {
          zoomSettleTimer = null
          set((s) => ({ renderTick: s.renderTick + 1 }))
        }, 250)
      }
    },
  }
})

/** Test seam: reset module-level history/timers between tests. */
export function resetIconsHistoryForTests(): void {
  undoStack = []
  redoStack = []
  gestureDepth = 0
  if (persistTimer) clearTimeout(persistTimer)
  persistTimer = null
}

/** True while any icon source is still decoding (drives the scan shimmer). */
export function iconSourcesLoading(): boolean {
  return sourcesLoading > 0
}
