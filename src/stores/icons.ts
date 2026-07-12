import { create } from 'zustand'
import { call } from '@/bridge/client'
import type { ConfigDto, GridMetricsDto, IconItemDto, IconOpResultDto, IconPersistedDto, IconScanDto, IconsStateDto, IconKindBucket, KindPolicy, TypeOverrideEntry, TypeOverrides } from '@/bridge/types'
import { DEFAULT_KIND_POLICY, kindBucket, kindParticipates } from '@/lib/kind-policy'
import { appAccentSeed, resolveTypeConfig, typeHasFixedPlate } from '@/lib/type-config'
import { activePresetIdOf as activePresetIdOfRecipe, assembleIconsState, defaultRecipe, parseHistory, parseRecipe, type IconStyleRecipe } from '@/lib/icons-assemble'
import { useWallpaper } from '@/stores/wallpaper'
import { getIconCompositor } from '@/icon-compositor/icon-renderer'
import type { RenderOpts } from '@/icon-wasm/protocol'
import type { SpreadEntry } from '@/icon-wasm/hue-spread'
import { computeHueSpread } from '@/icon-wasm/hue-spread'
import { format, t } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'
import { useToasts } from '@/stores/toasts'

// Icons-module state (icons contract v2, spec 06): the web compositor renders
// every preview locally — NOTHING crosses the bridge per edit. `setLook`
// persists config+overrides (400ms debounce); apply bakes 256px masters and
// streams them in chunks. Undo: one step per discrete pick; wheel drags
// coalesce per gesture (spec 06 §3.3). apply/restore run ONLY from explicit
// user clicks (owner gate).

const APPLY_CHUNK = 20

// System-Default (bare) draft persistence (A3). The host settings store persists
// the config/overrides draft ([M6-WIRE]); the bare-look intent is NOT a host
// concept (no ConfigDto represents it — see selectSystemDefault), so it rides the
// SAME client layer as the consent flags. Persisting it makes a relaunch resume
// the last bare selection (with the applied/not-applied status line), and it is
// the ONLY place a "system default" resume can live — the host can't distinguish
// "un-styled + spectrum draft" from "un-styled + system-default intent". Guarded
// so a non-browser / privacy-mode env degrades to session-only, never crashes.
const BARE_LOOK_KEY = 'dm.icons.bareLook'
export function readBareLook(): boolean {
  try {
    return typeof localStorage !== 'undefined' && localStorage.getItem(BARE_LOOK_KEY) === '1'
  } catch {
    return false
  }
}
export function persistBareLook(value: boolean): void {
  try {
    if (typeof localStorage === 'undefined') return
    if (value) localStorage.setItem(BARE_LOOK_KEY, '1')
    else localStorage.removeItem(BARE_LOOK_KEY)
  } catch {
    // Storage unavailable (SSR / privacy mode): the intent stays session-only.
  }
}

interface LookSnapshot {
  config: ConfigDto
  overrides: Map<string, { mode: 'keep' | 'tint'; tint: string | null }>
  kindPolicy: KindPolicy
  typeOverrides: TypeOverrides
  /** The System-Default reset intent rides history too (undo un-selects it). */
  bareLook: boolean
}

interface IconsState {
  loaded: boolean
  state: IconsStateDto | null
  items: IconItemDto[]
  revision: number
  comparing: boolean
  /** System-Default reset (A1): the working design is "bare" — every icon shows
   *  its original art in the preview (the mirror's show-original path). Purely a
   *  design intent; selecting it never writes to the host. The real desktop only
   *  changes at the explicit CTA crossing, which for a bare look is a RESTORE. */
  bareLook: boolean
  zoom: number
  waveKind: 'bloom' | 'settle' | null
  waveStamp: number
  /** Hover try-on candidate — paints the whole desktop, never commits (spec 06 §3.2). */
  hoverConfig: ConfigDto | null
  /** The hovered candidate's type ladder (preset try-on carries its FULL
   *  look — config alone previewed the wrong plates, owner bug 2026-07-10);
   *  null = keep the committed ladder. */
  hoverTypeOverrides: TypeOverrides | null
  /** Hover try-on of the BARE desktop (the System Default card, owner 2026-07-12:
   *  hovering it must preview like every other style card): paints original icons
   *  + the native arrow, never commits — symmetric with hoverConfig. */
  hoveringBare: boolean
  /** Bumped whenever tiles must re-render (source loaded / rescan / zoom settle). */
  renderTick: number
  /** First-paint gate: false until every dispatched tile has landed, so the
   *  desktop never shows wallpaper-with-blank-icons. Sticky true after the first
   *  quiesce (a later rescan re-renders in place, no re-veil). */
  ready: boolean
  applyProgress: { done: number; total: number } | null
  canUndo: boolean
  canRedo: boolean
  /** In-flight guard for the elevated overlay restore (single-flight; a slow
   *  UAC round-trip must not allow a duplicate restore or clobber newer edits). */
  overlayRestoring: boolean
  /** True once the scan retry budget is spent (review P2-2): auto-retry has
   *  given up, so the empty desktop surfaces a manual "re-read" entry instead of
   *  spinning forever. Cleared by a successful scan or an explicit retry. */
  scanExhausted: boolean

  scan: () => Promise<void>
  rescan: () => Promise<void>
  /** Manual scan recovery (review P2-2): resets the backoff budget and re-reads
   *  the desktop after auto-retry has been exhausted. */
  retryScan: () => void
  mutate: (change: Partial<ConfigDto>) => void
  beginGesture: () => void
  endGesture: () => void
  hover: (change: Partial<ConfigDto> | null, typeOverrides?: TypeOverrides) => void
  /** Bare-desktop hover try-on (System Default card) — repaint only, no commit. */
  hoverBare: (on: boolean) => void
  selectPreset: (presetId: string) => void
  /** System Default (A1): reset the working design to bare (client-only, no host
   *  write). Highlighted when active; the CTA then restores rather than applies. */
  selectSystemDefault: () => void
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
  /** Keep-beautification arrow restore (panel record 2026-07-11): brings the
   *  native shortcut arrow back without undoing the icon look. */
  restoreOverlay: () => Promise<void>
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
/** Pending retry for a failed scan (review P2-2), exponential backoff. */
let scanRetryTimer: ReturnType<typeof setTimeout> | null = null
let scanRetryBaseMs = 2000
let scanRetryCount = 0
/** Max total scan attempts (initial + retries) before giving up (review P2-2). */
export const SCAN_RETRY_MAX = 5
export const SCAN_RETRY_CAP_MS = 30000

/** Exponential backoff for the Nth failed scan attempt, capped. Pure so the
 *  interval ladder + cap are unit-testable without real timers (review P2-2). */
export function scanRetryDelayMs(attempt: number, baseMs: number, capMs: number): number {
  return Math.min(baseMs * 2 ** (attempt - 1), capMs)
}

// Unified concurrency discipline (review P2-4). EVERY writer that establishes
// authoritative host state (arrow overlay, profile count, or a wholesale state
// replacement — scan/rescan, apply, full restore, keep-beautification restore,
// export) claims a monotonic generation token at its START and, when its
// response lands, writes state ONLY if no LATER-STARTED writer exists
// (`isCurrentGen`). Ordering by START, not arrival, kills both failure modes at
// once: a late response can neither overwrite a newer writer's truth (P2-4a) nor
// be falsely dropped by an older writer that merely landed first (P2-4b). This
// applies UNIFORMLY — modal writers (apply / full restore) are NOT exempt: the
// canvas refresh + Settings export/arrow-restore stay reachable while `working`,
// so a modal writer's late commit must gate on its generation like any other.
//
// [M6-STORE-TXN] Deferred to M6 host wiring, out of scope for arrow-restore: the
// SYNCHRONOUS look/item writers (undo/redo, mutate, presets, overrides, kind/
// type policy, history) are NOT generation-aware, and a superseded apply still
// dispatches its host effect (applyBakedCommit) even though its state write is
// dropped. Both are store-level concurrency debt to be unified as one proper
// store transaction when the real elevated contract lands — not bolted onto the
// mock at 3am. Only the async host-truth writers are gated here.
let stateGen = 0
function nextGen(): number {
  return ++stateGen
}
function isCurrentGen(gen: number): boolean {
  return gen === stateGen
}

// Bridge indirection: production always uses the real `call`; store-behaviour
// tests inject a controllable bridge to drive deterministic / out-of-order
// responses (single-flight, cross-op race, scan retry) without a DOM mock.
let bridge: typeof call = call

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
/** The last scan's OBSERVED grid metrics, reused so op-result reassembly uses platform truth (not
 *  the fabricated default) between scans (codex Major 5). */
let lastGridMetrics: GridMetricsDto | undefined

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

function toastOf(dto: IconOpResultDto): void {
  if (dto.toast) {
    const text = dto.toast.arg
      ? format(t(dto.toast.key as StringKey), dto.toast.arg)
      : t(dto.toast.key as StringKey)
    useToasts.getState().show(text, dto.ok ? 'info' : 'warn')
  }
}

/** The desktop wallpaper for the icons preview mirror — the active screen's decoded source, read
 *  from the wallpaper store (both the mock and the real bridge populate it). Null before the
 *  wallpaper module loads (the mirror simply shows no bed until then). */
function currentWallpaperUrl(): string | null {
  return useWallpaper.getState().state?.originalUrl ?? null
}

/** Maps the thin `IconPersistedDto` into the assembly's persisted shape (parses history recipes;
 *  `isCurrent` keys off ② so a reset leaves nothing current). */
function persistedForAssembly(p: IconPersistedDto) {
  return {
    history: parseHistory(p.history, p.savedStyleJson),
    applied: p.applied,
    arrowOverlay: p.arrowOverlay,
    activeUserProfiles: p.activeUserProfiles,
  }
}

/** Derives the initial draft (config/kindPolicy/typeOverrides) from the saved-style ② (or the
 *  factory default when no Apply has run) — the store's resume source on a fresh scan. */
function draftFromPersisted(p: IconPersistedDto): IconStyleRecipe {
  return parseRecipe(p.savedStyleJson) ?? defaultRecipe(DEFAULT_KIND_POLICY)
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
  // Delegates to the single matching rule in lib/icons-assemble so the selection highlight and the
  // assembled `activePresetId` field never drift (D1: presets are frontend data).
  return activePresetIdOfRecipe(state.config, state.typeOverrides)
}

/**
 * Resume-aware status line (A3), consistent with the wallpaper module's grammar.
 * On relaunch the last draft is already rehydrated (config/overrides persist via
 * the host settings store — [M6-WIRE]); this maps the SAME applied/dirty signals
 * every module reads to an honest status: a draft that matches the live desktop
 * reads "resumed", a draft that differs NEVER silently reads as "applied". Pure
 * (no store access) so the mapping is unit-testable without a DOM. Scanning/
 * working are handled by the hero upstream and never reach here.
 */
export function resumeStatusKey(applied: boolean, dirty: boolean, bareLook: boolean): StringKey {
  // A bare (System Default) look has its own two-state line: nothing to resume.
  if (bareLook) return applied ? 'Icons_BareDirtyStatus' : 'Icons_BareStatus'
  // Un-applied (fresh OR a resumed-but-never-applied draft): honest "ready", never "applied".
  if (!applied) return 'Hero_ReadyStatus'
  // Applied + a newer draft on top → last unapplied draft; applied + clean → resumed.
  return dirty ? 'Hero_UnappliedStatus' : 'Hero_ResumedStatus'
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
      bareLook: s.bareLook,
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
      bareLook: snap.bareLook,
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
    // The bare-look intent is the only client-durable draft bit — write it immediately (not
    // debounced) so a fast close never drops it. The config/override/kindPolicy/typeOverrides DRAFT
    // does NOT persist here (D1 / spec 07 §8.2: setLook left the bridge; a draft is not intent). It
    // is frontend session state, resumed from ② (the last global Apply) on the next scan — so a
    // never-applied tweak is intentionally not durable, only an Applied look is.
    persistBareLook(get().bareLook)
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

  /** The store's live draft (config/kindPolicy/typeOverrides), or null before the first scan. */
  const currentDraft = (): IconStyleRecipe | null => {
    const s = get().state
    return s ? { config: s.config, kindPolicy: s.kindPolicy, typeOverrides: s.typeOverrides } : null
  }

  /** Fetches the raw scan + the persisted ②③/native bits (thin, D1) in one round-trip pair. */
  const fetchScan = async (): Promise<{ scan: IconScanDto; persisted: IconPersistedDto }> => {
    const [scan, persisted] = await Promise.all([bridge('icons.scan'), bridge('icons.getPersisted')])
    return { scan, persisted }
  }

  /** Assembles + adopts a scan under `draft` (D1: the frontend owns presets/palette/grid/assembly).
   *  `keepOverrides` re-applies live per-icon overrides across a manual rescan (codex Major 3). */
  const adoptScan = (
    scan: IconScanDto,
    persisted: IconPersistedDto,
    draft: IconStyleRecipe,
    keepOverrides?: Map<string, { mode: 'keep' | 'tint'; tint: string | null }>,
  ) => {
    if (scan.revision < get().revision) return
    // Any successful full load clears the scan-recovery state (review P2-2 item 4):
    // retry budget + pending backoff timer + the exhausted flag, so a rescan (or
    // manual retry) heals a prior exhaustion.
    scanRetryCount = 0
    if (scanRetryTimer !== null) {
      clearTimeout(scanRetryTimer)
      scanRetryTimer = null
    }
    // Scan items arrive with empty overrides (D1: overrides are frontend draft); carry the live ones
    // forward on a manual rescan so a refresh never silently drops the user's per-icon choices.
    const items = keepOverrides
      ? scan.items.map((it) => {
          const o = keepOverrides.get(it.id)
          return o ? { ...it, overrideMode: o.mode, overrideTint: o.tint } : it
        })
      : scan.items
    kindBuckets = new Map(items.map((i) => [i.id, kindBucket(i.kind)]))
    lastGridMetrics = scan.grid
    getIconCompositor().invalidateAll()
    set({
      items,
      state: assembleIconsState({
        draft,
        items,
        persisted: persistedForAssembly(persisted),
        wallpaperUrl: currentWallpaperUrl(),
        gridMetrics: scan.grid,
      }),
      revision: scan.revision,
      renderTick: get().renderTick + 1,
      scanExhausted: false,
    })
    loadSources(items)
  }

  return {
    loaded: false,
    state: null,
    items: [],
    revision: 0,
    comparing: false,
    bareLook: false,
    zoom: 1,
    waveKind: null,
    waveStamp: 0,
    hoverConfig: null,
    hoverTypeOverrides: null,
    hoveringBare: false,
    editingBucket: null,
    renderTick: 0,
    ready: false,
    applyProgress: null,
    canUndo: false,
    canRedo: false,
    overlayRestoring: false,
    scanExhausted: false,

    scan: async () => {
      if (get().loaded) return
      set({ loaded: true })
      // Claim a generation so a stale response from an older op can't overwrite
      // this scan, and vice-versa (unified concurrency, review P2-4).
      const gen = nextGen()
      // A failed scan must be RETRYABLE and must not strand consumers: `state`
      // stays null (→ Settings shows the "checking" status, never a false
      // "Windows default"), and `loaded` resets so a later trigger re-scans
      // (mirrors the boot() handshake). Previously the await was uncaught, so a
      // scan failure left loaded=true + state=null forever (review P2-2).
      let fetched
      try {
        fetched = await fetchScan()
      } catch (err) {
        console.error('icons.scan failed', err)
        // SELF-RECOVER with EXPONENTIAL BACKOFF + a TERMINAL state (review P2-2):
        // the sole scan trigger is App.tsx's one-shot mount effect, so resetting
        // `loaded` alone stranded the user in "checking". Auto-retry heals a
        // transient failure without a restart, but a PERMANENT failure must not
        // spin forever (log spam) — after SCAN_RETRY_MAX attempts it gives up and
        // surfaces a manual re-read entry (icons-mirror).
        // Gen-guard the FAILURE path BEFORE touching shared state (review item 2):
        // a superseded failed scan must not flip `loaded` back to false after a
        // newer scan already succeeded, nor schedule a retry that would claim a
        // fresh generation and drop the newer op's response.
        if (!isCurrentGen(gen)) return
        set({ loaded: false })
        scanRetryCount++
        if (scanRetryCount < SCAN_RETRY_MAX) {
          const delay = scanRetryDelayMs(scanRetryCount, scanRetryBaseMs, SCAN_RETRY_CAP_MS)
          if (scanRetryTimer === null) {
            scanRetryTimer = setTimeout(() => {
              scanRetryTimer = null
              // The retry belongs to THIS failed scan's generation: if a newer op
              // superseded it since scheduling, discard the retry rather than claim
              // a fresh gen that would invalidate that newer op (review timer gen).
              if (!isCurrentGen(gen)) return
              void get().scan()
            }, delay)
          }
        } else {
          set({ scanExhausted: true })
        }
        return
      }
      // Superseded by a newer op between dispatch and arrival (review item 1):
      // don't clobber the newer truth, and DON'T strand at loaded:true/state:null
      // with no recovery — re-open the load gate so a later trigger can re-read.
      if (!isCurrentGen(gen)) {
        set({ loaded: false })
        return
      }
      // Scan succeeded — adoptScan clears the recovery budget + timer + exhausted. The initial
      // draft resumes from ② (the last global Apply), or the factory default if none.
      adoptScan(fetched.scan, fetched.persisted, draftFromPersisted(fetched.persisted))
      // A3 resume: rehydrate the last bare-look intent BEFORE first paint, so a
      // relaunch whose last selection was System Default opens bare (not the
      // rehydrated config). Only the initial scan resumes it — a manual rescan
      // must not resurrect a bare intent the user has since left.
      set({ bareLook: readBareLook() })
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

    retryScan: () => {
      scanRetryCount = 0
      if (scanRetryTimer !== null) {
        clearTimeout(scanRetryTimer)
        scanRetryTimer = null
      }
      set({ scanExhausted: false, loaded: false })
      void get().scan()
    },

    rescan: async () => {
      const gen = nextGen()
      const fetched = await fetchScan()
      // A rescan superseded by a later op (e.g. an arrow restore started after it)
      // must not apply its now-stale full state over the newer truth (P2-4b).
      if (!isCurrentGen(gen)) return
      // Preserve the user's live draft AND per-icon overrides across a manual refresh (only the
      // initial scan resumes ② + starts overrides empty).
      const keepOverrides = new Map(
        get()
          .items.filter((i) => i.overrideMode !== null)
          .map((i) => [i.id, { mode: i.overrideMode as 'keep' | 'tint', tint: i.overrideTint }]),
      )
      adoptScan(fetched.scan, fetched.persisted, currentDraft() ?? draftFromPersisted(fetched.persisted), keepOverrides)
      useToasts.getState().show(t('Toast_Refreshed'))
    },

    // One undo step per discrete pick; repaint is local and immediate.
    mutate: (change) => {
      const s = get()
      if (!s.state) return
      pushUndo()
      set({ state: markDirty({ ...s.state, config: { ...s.state.config, ...change } }), hoverConfig: null, bareLook: false })
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
        set({ canUndo: true, canRedo: false, bareLook: false })
        persistBareLook(false) // a slider/wheel drag is a real edit — leave bare
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

    hoverBare: (on) => {
      if (get().hoveringBare !== on) set({ hoveringBare: on })
    },

    selectPreset: (presetId) => {
      const s = get()
      const preset = s.state?.presets.find((p) => p.id === presetId)
      if (!preset || !s.state) return
      pushUndo()
      set({
        state: markDirty({ ...s.state, config: { ...preset.config }, typeOverrides: structuredClone(preset.typeOverrides) }),
        hoverConfig: null,
        bareLook: false,
      })
      schedulePersist()
      recomputeHueSpread()
    },

    // System Default (A1): a RESET, not a style. It flips the working design to
    // "bare" — the mirror's show-original path paints every icon untouched — with
    // NO host write (no apply, no restore) at selection. `bareLook` is the design
    // intent that lights the card and turns the CTA into a restore crossing. It is
    // undoable (rides the snapshot) and any real look edit clears it.
    selectSystemDefault: () => {
      const s = get()
      if (!s.state || s.bareLook) return
      pushUndo()
      set({ bareLook: true, hoverConfig: null })
      persistBareLook(true) // resume the bare selection on the next launch (A3)
    },

    setOverride: (id, mode, tint) => {
      const s = get()
      if (!s.state) return
      pushUndo()
      set({
        // A per-icon override IS a draft change: mark dirty after an apply so the CTA offers a
        // re-apply instead of staying falsely "synced" (codex Major 3).
        state: markDirty(s.state),
        items: s.items.map((i) =>
          i.id === id
            ? { ...i, overrideMode: mode === 'follow' ? null : mode, overrideTint: mode === 'tint' ? (tint ?? null) : null }
            : i,
        ),
        bareLook: false,
      })
      schedulePersist()
    },

    clearOverrides: () => {
      const s = get()
      if (!s.state || !s.items.some((i) => i.overrideMode !== null)) return
      pushUndo()
      set({
        state: markDirty(s.state),
        items: s.items.map((i) => ({ ...i, overrideMode: null, overrideTint: null })),
      })
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
      set({ state: markDirty({ ...s.state, kindPolicy: { ...s.state.kindPolicy, [bucket]: styled } }), bareLook: false })
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
      set({ state: markDirty({ ...s.state, typeOverrides: next }), hoverConfig: null, bareLook: false })
      schedulePersist()
      recomputeHueSpread()
    },

    resetTypeOverrides: () => {
      const s = get()
      if (!s.state || Object.keys(s.state.typeOverrides).length === 0) return
      pushUndo()
      set({ state: markDirty({ ...s.state, typeOverrides: {} }), hoverConfig: null, bareLook: false })
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
      // System Default is a reset, never an apply (A1): a bare look can NEVER bake
      // beautified icons onto the desktop. Its CTA crossing is a restore, routed by
      // the panel; this store invariant guarantees no stray apply slips through.
      if (s.bareLook) return false
      // Claim a generation so any writer already in flight is superseded, AND so
      // this apply's own late commit self-drops if a newer writer (canvas refresh
      // / Settings restore, both reachable while `working`) started after it
      // (P2-4, uniform gating — modal is NOT exempt).
      const gen = nextGen()
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
      // 「保留原样」 / kindPolicy-excluded styleable items send NO master — but a currently-styled one
      // must be REVERTED to its original, not left (spec 06 §2). Rust reverts the tracked subset.
      const restoreIds = s.items
        .filter((i) => i.styleable && effectiveTileConfig(i, config, policy, typeOverrides).showOriginal)
        .map((i) => i.id)
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
        await bridge('icons.applyBakedBegin', { revision: s.revision, count: jobs.length })
        for (let at = 0; at < jobs.length; at += APPLY_CHUNK) {
          const chunk = jobs.slice(at, at + APPLY_CHUNK)
          const rendered: { id: string; sourceIndex: number; masterPng: string }[] = []
          for (const j of chunk) {
            const eff = effectiveTileConfig(j.item, config, policy, typeOverrides)
            const png = await compositor.bakeMasterPng(bakeSourceId(j.item.id, j.sourceIndex), eff.config, j.item.isShortcut, frozenOpts.get(j.item.id))
            if (!png) throw new Error(`master missing: ${j.item.id}#${j.sourceIndex}`)
            rendered.push({ id: j.item.id, sourceIndex: j.sourceIndex, masterPng: png })
          }
          await bridge('icons.applyBakedChunk', { items: rendered })
          set({ applyProgress: { done: Math.min(at + chunk.length, jobs.length), total: jobs.length } })
          // Yield a frame so the progress UI paints between chunks.
          await new Promise((resolve) => requestAnimationFrame(resolve))
        }
        // The full recipe rides as an opaque JSON string (Rust persists it as ②③); a tint override
        // is baked into its master. `restoreIds` carries the 「保留原样」 items so Rust reverts any
        // that are currently styled (spec 06 §2). The styleJson is the three global knobs (spec 07 §8.2).
        const styleJson = JSON.stringify({ config, kindPolicy: policy, typeOverrides })
        const result = await bridge('icons.applyBakedCommit', { styleJson, restoreIds, label: lookLabel(s.state) })
        // Superseded while baking (a newer writer landed first): drop this stale
        // commit — never write the outdated state or announce success over the
        // newer writer's truth (P2-4). Clear the progress veil AND release
        // `working` on the current state: a superseding delta-merge writer (arrow
        // restore) leaves apply's optimistic working:true in place, which would
        // otherwise lock the buttons/veil (review item 1 — modal working hang).
        if (!isCurrentGen(gen)) {
          const cur = get().state
          set(cur ? { state: { ...cur, working: false }, applyProgress: null } : { applyProgress: null })
          return false
        }
        // The applied recipe IS the new draft; assemble the fresh state from it + the persisted ②③.
        const applied: IconStyleRecipe = { config, kindPolicy: policy, typeOverrides }
        set({
          state: assembleIconsState({
            draft: applied,
            items: get().items,
            persisted: persistedForAssembly(result.persisted),
            wallpaperUrl: currentWallpaperUrl(),
            gridMetrics: lastGridMetrics,
          }),
          applyProgress: null,
        })
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
      // Claim a generation to supersede any writer in flight, and to self-drop
      // this restore's own late response if a newer writer (canvas refresh /
      // Settings arrow-restore, reachable while `working`) started after it
      // (P2-4, uniform gating — modal is NOT exempt).
      const gen = nextGen()
      set({ state: { ...s.state, working: true } })
      try {
        const result = await bridge('icons.restore')
        // Superseded: the newer writer's truth stands. Still release `working` on
        // the current state — a delta-merge superseder (arrow restore) leaves this
        // restore's optimistic working:true in place, locking the UI (item 1).
        if (!isCurrentGen(gen)) {
          const cur = get().state
          if (cur) set({ state: { ...cur, working: false } })
          return
        }
        // A full restore clears ② + reverts the desktop; keep the user's live draft (they can
        // re-apply) — only `applied`/arrow flip, sourced from the fresh persisted state.
        set({
          state: assembleIconsState({
            draft: currentDraft() ?? draftFromPersisted(result.persisted),
            items: get().items,
            persisted: persistedForAssembly(result.persisted),
            wallpaperUrl: currentWallpaperUrl(),
            gridMetrics: lastGridMetrics,
          }),
        })
        toastOf(result)
        if (result.ok) set({ waveKind: 'settle', waveStamp: Date.now() })
      } catch {
        const cur = get()
        if (cur.state) set({ state: { ...cur.state, working: false } })
        useToasts.getState().show(t('Toast_ApplyFailed'), 'warn')
      }
    },

    // 恢复系统箭头 (Settings): moves the arrow overlay only, keeping the icon
    // look. Single-flight (a slow UAC round-trip must not double-fire), merges
    // only the overlay delta (never clobbers newer config edits with a stale
    // DTO), and reports a restore-specific failure — not "apply failed".
    restoreOverlay: async () => {
      const s = get()
      if (!s.state || s.overlayRestoring) return
      set({ overlayRestoring: true })
      // Claim a generation at START (initiation order, not arrival order): this
      // restore applies its result only if no LATER-STARTED authoritative op
      // exists. A later apply/full-restore/rescan supersedes it (no stale
      // overwrite); an EARLIER op that merely lands afterwards does NOT supersede
      // it, so a legitimately-newer restore is never falsely dropped (P2-4a/b).
      const gen = nextGen()
      try {
        const result = await bridge('icons.restoreOverlay')
        if (!isCurrentGen(gen)) return
        // Delta-merge only the arrow/profile fields so a concurrent config edit
        // (this op holds no `working` flag) is never clobbered by a stale DTO.
        const cur = get().state
        if (cur) {
          set({
            state: {
              ...cur,
              arrowOverlay: result.persisted.arrowOverlay,
              activeUserProfiles: result.persisted.activeUserProfiles,
            },
          })
        }
        toastOf(result)
      } catch {
        // A superseded restore that also rejected stays silent — the newer op's
        // result already stands; only a still-current failure toasts.
        if (!isCurrentGen(gen)) return
        useToasts.getState().show(t('Toast_RestoreArrowFailed'), 'warn')
      } finally {
        set({ overlayRestoring: false })
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
        // 回到此版 restores the FULL recipe — config + kindPolicy (participation) + typeOverrides —
        // so a history look never applies A's visuals with B's participation policy (codex Major 2).
        state: markDirty({ ...s.state, config: { ...entry.config }, kindPolicy: { ...entry.kindPolicy }, typeOverrides: structuredClone(entry.typeOverrides) }),
        hoverConfig: null,
        bareLook: false,
      })
      schedulePersist()
      recomputeHueSpread()
    },

    exportCompare: async () => {
      // Export renders a before/after compare sheet — a pure side effect that never changes the
      // visible state (D1: the thin op returns only the unchanged persisted bits), so it just
      // toasts. No generation guard needed: there is no state to clobber.
      const result = await bridge('icons.exportCompare')
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

/** Test seam: reset module-level history/timers between tests. Also heals any
 *  leaked bridge/generation/retry override so state can't bleed across files. */
export function resetIconsHistoryForTests(): void {
  undoStack = []
  redoStack = []
  gestureDepth = 0
  if (persistTimer) clearTimeout(persistTimer)
  persistTimer = null
  if (scanRetryTimer) clearTimeout(scanRetryTimer)
  scanRetryTimer = null
  scanRetryBaseMs = 2000
  scanRetryCount = 0
  stateGen = 0
  bridge = call
  persistBareLook(false) // clear the client bare-look flag so it can't bleed across tests
}

/** Test seam: inject a controllable bridge (null restores the real `call`). */
export function __setBridgeForTests(fn: typeof call | null): void {
  bridge = fn ?? call
}

/** Test seam: shorten the failed-scan retry base delay so recovery is observable. */
export function __setScanRetryMsForTests(ms: number): void {
  scanRetryBaseMs = ms
}

/** True while any icon source is still decoding (drives the scan shimmer). */
export function iconSourcesLoading(): boolean {
  return sourcesLoading > 0
}
