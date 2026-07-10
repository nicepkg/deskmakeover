import type {
  ConfigDto,
  IconItemDto,
  IconKind,
  IconsOpResultDto,
  IconsStateDto,
  KindPolicy,
  OverrideEntryDto,
  PresetDto,
  ScanResultDto,
  TypeOverrides,
} from './types'
import { DEFAULT_KIND_POLICY } from '@/lib/kind-policy'
import { typeOverridesEqual } from '@/lib/type-config'

// Browser-only fake desktop DATA source (icons contract v2, spec 06 §2/§5):
// the canvas wallpaper scene, the REAL icon pack (public/real-icons/, the
// asset SSoT harvested by scripts/dev/fetch-real-icons.ts) and the session state
// machine. NO styling happens here any more — the icon compositor renders
// sources exactly as it does against the Windows host, so the Mac dev loop
// shows engine truth (ADR-0015 D1 retired the old approximate tile painter).

// ---- mock wallpaper: REAL Win11 default first, drawn scene fallback ----
// The real Bloom wallpaper rides in the gitignored real pack (fetch script §4).
// Locally we cannot read the user's actual wallpaper, so the REAL default is
// the honest stand-in (owner order 2026-07-09); the canvas dawn scene remains
// only for a clone that has not fetched the fixtures.

const SCENE_W = 1920
const SCENE_H = 1080

// ---- DEV user-simulation scenarios (dev menu; owner ask 2026-07-09) ----
// messy  = the whole harvested pack (stress test — the current chaos)
// office = a tidy work desktop: documents, folders, a few work apps
// gamer  = a geek/gaming desktop: dark theme wallpaper, its own app set
// Scenario = mock DATA choice, so it lives here, not in app state; switching
// happens in the dev menu via localStorage + reload (dev/video tooling only).

export type MockScenario = 'messy' | 'office' | 'gamer'
export const SCENARIO_KEY = 'dm.dev.scenario'

export function currentScenario(): MockScenario {
  const v = localStorage.getItem(SCENARIO_KEY)
  return v === 'office' || v === 'gamer' ? v : 'messy'
}

// [M6-WIRE] Active user-profile count is host truth on Windows; in the browser
// loop it is a dev knob (localStorage + reload, like the scenario) so the
// multi-user consent gate can be exercised. Default 1 (single user).
export const USER_PROFILES_KEY = 'dm.dev.userProfiles'

function activeUserProfiles(): number {
  const v = Number(localStorage.getItem(USER_PROFILES_KEY))
  return Number.isFinite(v) && v >= 1 ? Math.floor(v) : 1
}

/** Curated item sets (real-pack ids) + video-credible label overrides. */
const SCENARIO_ITEMS: Record<Exclude<MockScenario, 'messy'>, { id: string; label?: string }[]> = {
  office: [
    { id: 'real-win-bin' },
    { id: 'real-win-thispc' },
    { id: 'real-win-folder', label: '工作' },
    { id: 'real-win-docs', label: '项目资料' },
    { id: 'real-win-down', label: '下载' },
    { id: 'real-win-pics', label: '图片' },
    { id: 'real-app-edge' },
    { id: 'real-app-winWord', label: 'Word' },
    { id: 'real-app-excel' },
    { id: 'real-app-powerpoint' },
    { id: 'real-app-outlook' },
    { id: 'real-app-teams', label: 'Teams' },
    { id: 'real-app-onenote' },
    { id: 'real-app-calculator' },
    // Loose files — every label matches its icon's ART (contact-sheet
    // verified): doc windows = .docx, ruled note = .txt, pie-chart slide =
    // .pptx, picture docs = images, floppy = backup. No .xlsx/.pdf names —
    // the pack has no spreadsheet/PDF art, and a PDF label on a printer icon
    // is exactly the confusion the owner banned.
    { id: 'real-win-67', label: '项目计划.docx' },
    { id: 'real-win-130', label: '合同定稿.docx' },
    { id: 'real-win-1278', label: '合同_副本.docx' },
    { id: 'real-win-170', label: '季度汇报.pptx' },
    { id: 'real-win-805', label: '年度总结草稿.docx' },
    { id: 'real-win-1294', label: '入职清单.docx' },
    { id: 'real-win-1569', label: '公司资料.docx' },
    { id: 'real-win-893', label: '会议纪要_0708.txt' },
    { id: 'real-win-90', label: '待办事项.txt' },
    { id: 'real-win-3', label: '新建文本文档.txt' },
    { id: 'real-win-98', label: '未读邮件.eml' },
    { id: 'real-win-106', label: '产品截图.png' },
    { id: 'real-win-1085', label: '海报终稿.jpg' },
    { id: 'real-win-1577', label: '设计稿.png' },
    { id: 'real-win-2000', label: '数据备份_0630' },
  ],
  gamer: [
    { id: 'real-win-bin' },
    { id: 'real-app-minecraft' },
    { id: 'real-app-xbox', label: 'Xbox' },
    { id: 'real-app-discord' },
    { id: 'real-app-spotify' },
    { id: 'real-app-code' },
    { id: 'real-app-terminal', label: '终端' },
    { id: 'real-app-github' },
    { id: 'real-app-edge' },
    { id: 'real-app-camera', label: 'OBS 录制' },
    { id: 'real-win-vid', label: '游戏录像' },
    { id: 'real-win-music', label: '音乐' },
    { id: 'real-win-folder3d', label: 'MOD 合集' },
    { id: 'real-win-1593', label: '击杀集锦.mp4' },
  ],
}

const SCENARIO_WALLPAPER: Record<MockScenario, string[]> = {
  messy: ['/real-icons/wallpapers/wallpaper-default.jpg'],
  office: ['/real-icons/wallpapers/wallpaper-office.jpg', '/real-icons/wallpapers/wallpaper-default.jpg'],
  gamer: ['/real-icons/wallpapers/wallpaper-gamer.jpg', '/real-icons/wallpapers/wallpaper-dark.jpg'],
}

let realWallUrl: string | null = null
let wallProbe: Promise<void> | null = null

/** Resolve the real wallpaper once; await before building any state. */
export function probeRealWallpaper(): Promise<void> {
  wallProbe ??= (async () => {
    const scenario = currentScenario()
    const dark = document.documentElement.classList.contains('dark')
    const candidates = [...SCENARIO_WALLPAPER[scenario]]
    if (scenario === 'messy' && dark) candidates.unshift('/real-icons/wallpapers/wallpaper-dark.jpg')
    for (const candidate of candidates) {
      const head = await fetch(candidate, { method: 'HEAD' }).catch(() => null)
      if (head?.ok) {
        realWallUrl = candidate
        return
      }
    }
  })()
  return wallProbe
}

let sceneUrl: string | null = null

export function mockWallpaperUrl(): string {
  if (realWallUrl) return realWallUrl
  if (sceneUrl) return sceneUrl
  const canvas = document.createElement('canvas')
  canvas.width = SCENE_W
  canvas.height = SCENE_H
  const ctx = canvas.getContext('2d')!

  const sky = ctx.createLinearGradient(0, 0, 0, SCENE_H)
  sky.addColorStop(0, '#F4E7D3')
  sky.addColorStop(0.45, '#E8C9A0')
  sky.addColorStop(0.75, '#D9A06B')
  sky.addColorStop(1, '#B97D4E')
  ctx.fillStyle = sky
  ctx.fillRect(0, 0, SCENE_W, SCENE_H)

  const sun = ctx.createRadialGradient(SCENE_W * 0.72, SCENE_H * 0.34, 40, SCENE_W * 0.72, SCENE_H * 0.34, 480)
  sun.addColorStop(0, 'rgba(255, 244, 214, 0.95)')
  sun.addColorStop(0.35, 'rgba(255, 226, 178, 0.5)')
  sun.addColorStop(1, 'rgba(255, 226, 178, 0)')
  ctx.fillStyle = sun
  ctx.fillRect(0, 0, SCENE_W, SCENE_H)

  const dune = (baseY: number, amp: number, color: string) => {
    ctx.beginPath()
    ctx.moveTo(0, baseY)
    ctx.bezierCurveTo(SCENE_W * 0.25, baseY - amp, SCENE_W * 0.45, baseY + amp * 0.6, SCENE_W * 0.62, baseY - amp * 0.2)
    ctx.bezierCurveTo(SCENE_W * 0.78, baseY - amp * 0.9, SCENE_W * 0.9, baseY + amp * 0.3, SCENE_W, baseY - amp * 0.4)
    ctx.lineTo(SCENE_W, SCENE_H)
    ctx.lineTo(0, SCENE_H)
    ctx.closePath()
    ctx.fillStyle = color
    ctx.fill()
  }
  dune(SCENE_H * 0.68, 90, '#A5713F')
  dune(SCENE_H * 0.78, 70, '#8A5A33')
  dune(SCENE_H * 0.88, 50, '#6E4526')

  const vig = ctx.createRadialGradient(SCENE_W / 2, SCENE_H / 2, SCENE_H * 0.45, SCENE_W / 2, SCENE_H / 2, SCENE_H * 0.95)
  vig.addColorStop(0, 'rgba(0,0,0,0)')
  vig.addColorStop(1, 'rgba(40,22,8,0.22)')
  ctx.fillStyle = vig
  ctx.fillRect(0, 0, SCENE_W, SCENE_H)

  sceneUrl = canvas.toDataURL('image/jpeg', 0.85)
  return sceneUrl
}

export const MOCK_PALETTE = ['#B97D4E', '#8A5A33', '#E8C9A0', '#6E4526', '#F4E7D3']
const MONO_SWATCHES = ['#FFFFFF', '#141414', '#B97D4E', '#FF6F5E', '#3FB6A8', '#D9A94E']
const MARK_SWATCHES = ['#FFFFFF', '#141414', '#FF6F5E', '#B97D4E', '#3FB6A8']

// ---- desktop items: the REAL icon pack is REQUIRED (owner order 2026-07-11:
// synthetic icons are gone from the mock desktop) ----
// public/real-icons/ is the gitignored asset SSoT (subfoldered by type),
// harvested by scripts/dev/fetch-real-icons.ts: genuine Windows system icons
// + real app icons at their native sizes. A fresh clone must run the harvest
// script once; there is no synthetic fallback.

interface RealEntry {
  file: string
  id: string
  kind: IconKind
  label: string
  /** Additional source files (Recycle Bin: [empty]). */
  extraSources: string[]
}

interface PackEntry {
  id: string
  kind: IconKind
  label: string
  sourceUrls: string[]
}

let manifestPromise: Promise<PackEntry[]> | null = null

async function loadManifest(): Promise<PackEntry[]> {
  manifestPromise ??= (async () => {
    const real = await fetch('/real-icons/manifest.json').catch(() => null)
    if (real?.ok) {
      const entries = (await real.json()) as RealEntry[]
      const all: PackEntry[] = entries.map((e) => ({
        id: e.id,
        kind: e.kind,
        label: e.label,
        sourceUrls: [
          `/real-icons/${e.file}`,
          ...(e.extraSources ?? []).map((f) => `/real-icons/${f}`),
        ],
      }))
      const scenario = currentScenario()
      if (scenario !== 'messy') {
        const byId = new Map(all.map((e) => [e.id, e]))
        const picked = SCENARIO_ITEMS[scenario]
          .map((s) => {
            const entry = byId.get(s.id)
            return entry ? { ...entry, label: s.label ?? entry.label } : null
          })
          .filter((e): e is PackEntry => e !== null)
        if (picked.length >= 3) {
          console.info(`[mock desktop] scenario ${scenario}: ${picked.length} icons`)
          return picked
        }
      }
      console.info(`[mock desktop] REAL icon pack: ${all.length} icons (scenario ${scenario})`)
      return all
    }
    console.error(
      '[mock desktop] real icon pack missing — run `bun scripts/dev/fetch-real-icons.ts` (public/real-icons/ is the asset SSoT; no synthetic fallback)',
    )
    return []
  })()
  return manifestPromise
}

function toItem(entry: PackEntry, index: number, rows: number, g: IconsStateDto['grid']): IconItemDto {
  // UWP desktop entries are ordinary .lnk files — the standard icon-location
  // write applies (owner prototype truth, 2026-07-09); only Unsupported is off.
  const styleable = entry.kind !== 'Unsupported'
  const override = session.overrides.get(entry.id)
  const col = Math.floor(index / rows)
  const row = index % rows
  return {
    id: entry.id,
    label: entry.label,
    kind: entry.kind,
    // AppxShortcut IS a shortcut (ADR-0017 bug fix): UWP desktop entries are
    // ordinary .lnk files and must wear the mark like any other shortcut.
    isShortcut: entry.kind === 'Shortcut' || entry.kind === 'UrlShortcut' || entry.kind === 'AppxShortcut',
    styleable,
    statusReason: styleable ? null : 'MOCK-HOST-REASON', // host sends localized copy; UI falls back to its own key
    x: g.inset + col * g.cellWidth,
    y: g.inset + row * g.cellHeight,
    sourceUrls: entry.sourceUrls,
    overrideMode: override?.mode ?? null,
    overrideTint: override?.tint ?? null,
  }
}

// ---- module state machine ----

// Marks default ON (owner 2026-07-10, voids the 2026-07-07 no-marks decree):
// the badge is the delete-safety indicator, so every preset ships
// Distinction.Mark with the lightweight Shadow style.
// Lineup reworked per ADR-0016 D3 (findability panel, owner 2026-07-10):
// Presets are COORDINATE BOOKMARKS in the subject × plate space (ADR-0018).
export const BASE_CONFIGS: Record<string, ConfigDto> = {
  // Preset Collection v2 (chief-designer curation, owner order 2026-07-10):
  // six distinct material worlds, six mark styles (Fold retired), dark-brown
  // folder boards banned. docs/product/preset-collection-v2.md is normative.
  // Key order = card order: the featured four (max-difference sampling -
  // colour/glass/ink/white) sit above the 更多风格 fold; glass wears Shadow
  // (the Glass mark's opaque disc read as a dark patch on frosted tiles;
  // bead redesign = logged component debt).
  spectrum: { shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'derived', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Halo', markColor: null, plateColor: null, size: 'Mid', filter: 'None' },
  glass: { shape: 'Samsung', subject: 'Original', plateBand: 'Vivid', plateFallback: 'derived', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Shadow', markColor: null, plateColor: null, size: 'Mid', filter: 'Glass' },
  ink: { shape: 'Circle', subject: 'BlackWhite', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Arc', markColor: null, plateColor: '#F4F1EA', size: 'Mid', filter: 'None' },
  white: { shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Ring', markColor: null, plateColor: '#FFFFFF', size: 'Mid', filter: 'None' },
  stationery: { shape: 'Apple', subject: 'Original', plateBand: 'Quiet', plateFallback: 'derived', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Satin', markColor: null, plateColor: null, size: 'Mid', filter: 'None' },
  pebble: { shape: 'Pebble', subject: 'Original', plateBand: 'Quiet', plateFallback: 'derived', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Shadow', markColor: null, plateColor: null, size: 'Mid', filter: 'Sticker' },
  ascast: { shape: 'Apple', subject: 'Original', plateBand: 'Vivid', plateFallback: 'white', shortcutShape: null, monoStyle: 'Tonal', tint: '#FF6F5E', distinction: 'Mark', markStyle: 'Ring', markColor: null, plateColor: null, size: 'Mid', filter: 'None' },
}

// Per-preset type ladders (Preset Collection v2) — every set ships its own.
export const PRESET_TYPE_OVERRIDES: Record<string, TypeOverrides> = {
  spectrum: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: null, plateFallback: 'derived' } },
    File: { source: 'custom', patch: { shape: 'Tile', plateColor: '#E9E2D4' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#EDEAE4' } },
  },
  stationery: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: '#EAD6A8' } },
    File: { source: 'custom', patch: { shape: 'Tile', plateColor: '#E9E2D4' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#EDEAE4' } },
  },
  glass: {
    Folder: { source: 'custom', patch: { shape: 'Samsung', plateColor: null, plateFallback: 'derived' } },
    File: { source: 'custom', patch: { shape: 'Samsung', plateColor: '#FFFFFF' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#ECECEE' } },
  },
  pebble: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: '#EAD6A8' } },
    File: { source: 'custom', patch: { shape: 'Teardrop', plateColor: '#E9E2D4' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#EAE7E0' } },
  },
  ink: {
    Folder: { source: 'custom', patch: { shape: 'Bookmark', plateColor: '#EDE8DC' } },
    File: { source: 'custom', patch: { shape: 'Tile', plateColor: '#F4F1EA' } },
    System: { source: 'custom', patch: { shape: 'Circle', plateColor: '#EEEBE4' } },
  },
  white: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: '#FFFFFF' } },
    File: { source: 'custom', patch: { shape: 'Tile', plateColor: '#FFFFFF' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#F2F2F2' } },
  },
  ascast: {
    Folder: { source: 'custom', patch: { shape: 'Folder', plateColor: null, plateFallback: 'white' } },
    File: { source: 'custom', patch: { shape: 'Tile', plateColor: '#E9E2D4' } },
    System: { source: 'custom', patch: { shape: 'Circle', subject: 'BlackWhite', plateColor: '#EDEAE4' } },
  },
}

// C# truth: desktop icon px is Small 32 · Mid 48 · Big 96 (DesktopIconSize.cs).
const ICON_PX: Record<ConfigDto['size'], number> = { Small: 32, Mid: 48, Big: 96 }

interface MockIconsSession {
  config: ConfigDto
  typeOverrides: TypeOverrides
  applied: boolean
  dirty: boolean
  working: boolean
  history: { time: string; label: string; config: ConfigDto; typeOverrides: TypeOverrides }[]
  currentHistoryIndex: number
  overrides: Map<string, { mode: 'keep' | 'tint'; tint: string | null }>
  kindPolicy: KindPolicy
  revision: number
  bakePending: Map<string, string>
  // [M6-WIRE] Machine-wide native-arrow state (ADR-0021). Apply installs the
  // transparent overlay ('hidden'); both restores lift it ('native').
  arrowOverlay: 'native' | 'hidden'
}

const session: MockIconsSession = {
  config: { ...BASE_CONFIGS.spectrum },
  typeOverrides: structuredClone(PRESET_TYPE_OVERRIDES.spectrum),
  applied: false,
  dirty: false,
  working: false,
  history: [],
  currentHistoryIndex: -1,
  overrides: new Map(),
  kindPolicy: { ...DEFAULT_KIND_POLICY },
  revision: 0,
  bakePending: new Map(),
  arrowOverlay: 'native',
}

function activePresetId(): string | null {
  for (const [id, preset] of Object.entries(BASE_CONFIGS)) {
    if (
      preset.shape === session.config.shape &&
      preset.subject === session.config.subject &&
      preset.filter === session.config.filter &&
      preset.distinction === session.config.distinction &&
      typeOverridesEqual(PRESET_TYPE_OVERRIDES[id], session.typeOverrides) &&
      (preset.shortcutShape ?? null) === (session.config.shortcutShape ?? null) &&
      preset.plateColor === session.config.plateColor &&
      preset.plateFallback === session.config.plateFallback &&
      // derived-plate presets differ by band only when the plate IS derived.
      (preset.plateColor !== null || preset.plateBand === session.config.plateBand) &&
      (preset.subject !== 'Mono' || preset.tint === session.config.tint)
    ) {
      return id
    }
  }
  return null
}

function presets(): PresetDto[] {
  return Object.entries(BASE_CONFIGS).map(([id, config]) => ({
    id,
    config: { ...config },
    typeOverrides: structuredClone(PRESET_TYPE_OVERRIDES[id] ?? {}),
  }))
}

function grid(): IconsStateDto['grid'] {
  const iconPx = ICON_PX[session.config.size]
  return {
    screenWidth: SCENE_W,
    screenHeight: SCENE_H,
    taskbarHeight: 48,
    iconPx,
    cellWidth: iconPx + 44,
    cellHeight: iconPx + 48,
    inset: 14,
    labelFontPx: 12,
  }
}

let styleableCount = 0

function state(): IconsStateDto {
  return {
    scanning: false,
    working: session.working,
    applied: session.applied,
    dirty: session.dirty,
    styleableCount,
    config: { ...session.config },
    activePresetId: activePresetId(),
    presets: presets(),
    history: session.history.map((h, index) => ({
      index,
      time: h.time,
      label: h.label,
      isCurrent: index === session.currentHistoryIndex,
      config: { ...h.config },
      typeOverrides: structuredClone(h.typeOverrides),
    })),
    palette: MOCK_PALETTE,
    monoSwatches: MONO_SWATCHES,
    markSwatches: MARK_SWATCHES,
    grid: grid(),
    wallpaperUrl: mockWallpaperUrl(),
    kindPolicy: { ...session.kindPolicy },
    typeOverrides: structuredClone(session.typeOverrides),
    arrowOverlay: session.arrowOverlay,
    activeUserProfiles: activeUserProfiles(),
  }
}

function nowLabel(): string {
  const d = new Date()
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

export async function mockIconsCall(method: string, params: unknown): Promise<unknown> {
  const p = (params ?? {}) as {
    config?: ConfigDto
    overrides?: OverrideEntryDto[]
    revision?: number
    count?: number
    items?: { id: string; sourceIndex: number; masterPng: string }[]
    label?: string
  }
  switch (method) {
    case 'icons.getState':
      return state()
    case 'icons.scan': {
      await probeRealWallpaper()
      const entries = await loadManifest()
      const g = grid()
      const rows = Math.floor((g.screenHeight - g.taskbarHeight - g.inset * 2) / g.cellHeight)
      // Positions are OBSERVED truth: computed once per scan at the CURRENT
      // icon size, never re-packed by a size-knob change (spec 06 §3.6).
      const items = entries.map((e, i) => toItem(e, i, rows, g))
      styleableCount = items.filter((i) => i.styleable).length
      session.revision += 1
      return { revision: session.revision, items, state: state() } satisfies ScanResultDto
    }
    case 'icons.setLook': {
      session.config = { ...p.config! }
      session.overrides = new Map((p.overrides ?? []).map((o) => [o.id, { mode: o.mode, tint: o.tint }]))
      const kp = (p as { kindPolicy?: KindPolicy }).kindPolicy
      if (kp) session.kindPolicy = { ...kp }
      const to = (p as { typeOverrides?: TypeOverrides }).typeOverrides
      if (to) session.typeOverrides = structuredClone(to)
      if (session.applied) session.dirty = true
      return null
    }
    case 'icons.applyBakedBegin':
      session.bakePending = new Map()
      session.working = true
      return null
    case 'icons.applyBakedChunk': {
      for (const item of p.items ?? []) session.bakePending.set(`${item.id}#${item.sourceIndex}`, item.masterPng)
      return null
    }
    case 'icons.applyBakedCommit': {
      // Keep the last bake inspectable in the browser loop (debug menu).
      ;(window as { __dmBakedIcons?: Record<string, string> }).__dmBakedIcons = Object.fromEntries(
        [...session.bakePending].map(([id, png]) => [id, `data:image/png;base64,${png}`]),
      )
      session.config = { ...p.config! }
      session.overrides = new Map((p.overrides ?? []).map((o) => [o.id, { mode: o.mode, tint: o.tint }]))
      const committedLadder = (p as { typeOverrides?: TypeOverrides }).typeOverrides
      if (committedLadder) session.typeOverrides = structuredClone(committedLadder)
      session.applied = true
      session.dirty = false
      session.working = false
      // [M6-WIRE] Apply installs the global transparent overlay: the native
      // Windows arrow is hidden machine-wide (ADR-0021).
      session.arrowOverlay = 'hidden'
      session.history.unshift({ time: nowLabel(), label: p.label ?? '自定义', config: { ...p.config! }, typeOverrides: structuredClone(session.typeOverrides) })
      session.history = session.history.slice(0, 10)
      session.currentHistoryIndex = 0
      return { state: state(), toast: null, ok: true } satisfies IconsOpResultDto
    }
    case 'icons.restore':
      session.applied = false
      session.dirty = false
      session.currentHistoryIndex = -1
      // Full restore lifts the overlay too (icons AND arrow back to native).
      session.arrowOverlay = 'native'
      return { state: state(), toast: null, ok: true } satisfies IconsOpResultDto
    case 'icons.restoreOverlay':
      // [M6-WIRE] Keep-beautification restore: only the native arrow returns;
      // the icon look (applied/history/config) is untouched.
      session.arrowOverlay = 'native'
      return { state: state(), toast: { key: 'Toast_ArrowRestored', arg: null }, ok: true } satisfies IconsOpResultDto
    case 'icons.exportCompare':
      return { state: state(), toast: null, ok: true } satisfies IconsOpResultDto
    default:
      throw new Error(`[mock desktop] unhandled method: ${method}`)
  }
}
