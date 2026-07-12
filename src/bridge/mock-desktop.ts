import type {
  IconItemDto,
  IconKind,
  IconOpResultDto,
  IconPersistedDto,
  IconScanDto,
  LookVersionDto,
} from './types'
import { iconGrid } from '@/lib/icons-assemble'
import { overlayRestoreResult, type OverlayOutcome } from '@/lib/arrow-overlay'

// Browser-only fake DESKTOP DATA source (icons contract v2, schema 7, D1): the mock is now THIN —
// it produces the raw scan items + the persisted ②③/native bits, exactly like the real Rust host.
// The presets/palette/swatches/grid/assembly moved to `lib/icons-assemble` (the single frontend
// assembly both the mock and the real bridge feed); NOTHING styles here — the compositor renders
// sources identically against the Windows host, so the Mac dev loop shows engine truth.

// ---- mock wallpaper scene (kept: consumed by mock-wallpaper.ts) ----
const SCENE_W = 1920
const SCENE_H = 1080

// ---- DEV user-simulation scenarios (dev menu; owner ask 2026-07-09) ----
export type MockScenario = 'messy' | 'office' | 'gamer'
export const SCENARIO_KEY = 'dm.dev.scenario'

export function currentScenario(): MockScenario {
  const v = localStorage.getItem(SCENARIO_KEY)
  return v === 'office' || v === 'gamer' ? v : 'messy'
}

// [M6-WIRE] Active user-profile count is host truth on Windows; in the browser loop a dev knob.
export const USER_PROFILES_KEY = 'dm.dev.userProfiles'

function activeUserProfiles(): number {
  const v = Number(localStorage.getItem(USER_PROFILES_KEY))
  return Number.isFinite(v) && v >= 1 ? Math.floor(v) : 1
}

// [M6-WIRE] The real dm-elevated RestoreOverlay verb resolves Applied|Declined|Failed; a dev knob
// injects the outcome so the declined/failed paths are exercisable.
export const RESTORE_OUTCOME_KEY = 'dm.dev.restoreOutcome'

function restoreOutcome(): OverlayOutcome {
  const v = localStorage.getItem(RESTORE_OUTCOME_KEY)
  return v === 'declined' || v === 'failed' ? v : 'applied'
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

// ---- desktop items: the REAL icon pack is REQUIRED (owner order 2026-07-11) ----
interface RealEntry {
  file: string
  id: string
  kind: IconKind
  label: string
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

// Positions are OBSERVED truth (spec 06 §3.6): the mock lays them out on a fixed-size grid, the
// stand-in for the real IFolderView2 layout. The frontend's grid drives rendering (D1).
const MOCK_GRID = iconGrid('Mid')

function toItem(entry: PackEntry, index: number, rows: number): IconItemDto {
  // UWP desktop entries are ordinary .lnk files — only Unsupported is off (owner prototype truth).
  const styleable = entry.kind !== 'Unsupported'
  const col = Math.floor(index / rows)
  const row = index % rows
  return {
    id: entry.id,
    label: entry.label,
    kind: entry.kind,
    isShortcut: entry.kind === 'Shortcut' || entry.kind === 'UrlShortcut' || entry.kind === 'AppxShortcut',
    styleable,
    statusReason: styleable ? null : 'MOCK-HOST-REASON',
    x: MOCK_GRID.inset + col * MOCK_GRID.cellWidth,
    y: MOCK_GRID.inset + row * MOCK_GRID.cellHeight,
    sourceUrls: entry.sourceUrls,
    // Per-icon overrides are frontend DRAFT state (schema 7) — a scan starts them empty; the store
    // fills them from its own draft.
    overrideMode: null,
    overrideTint: null,
  }
}

// ---- thin session (schema 7): only the platform/persisted truth the mock owns ----
const HISTORY_CAP = 10

interface MockIconsSession {
  applied: boolean
  arrowOverlay: 'native' | 'hidden'
  /** Store ② — the last-Applied recipe JSON, or null before any Apply. */
  savedStyleJson: string | null
  /** Store ③ — up to 10 saved looks, newest-first. */
  history: LookVersionDto[]
  revision: number
  bakePending: Map<string, string>
  styleableCount: number
  /** The current apply session token (mirrors the real host's R3-Block 1 guard). */
  sessionId: string
}

let mockSessionCounter = 0

const session: MockIconsSession = {
  applied: false,
  arrowOverlay: 'native',
  savedStyleJson: null,
  history: [],
  revision: 0,
  bakePending: new Map(),
  styleableCount: 0,
  sessionId: '0',
}

function persisted(): IconPersistedDto {
  return {
    savedStyleJson: session.savedStyleJson,
    history: session.history.map((v) => ({ ...v })),
    applied: session.applied,
    arrowOverlay: session.arrowOverlay,
    activeUserProfiles: activeUserProfiles(),
  }
}

function nowSeconds(): number {
  return Math.floor(Date.now() / 1000)
}

let lookCounter = 0

/** Pushes a look (dedup-before-cap, mirroring the real LookHistoryStore). */
function pushHistory(styleJson: string, label: string | null): void {
  const head = session.history[0]
  if (head && head.styleJson === styleJson) {
    head.createdAt = nowSeconds()
    return
  }
  session.history.unshift({ id: `mock-look-${++lookCounter}`, createdAt: nowSeconds(), label, pinned: false, styleJson })
  session.history = session.history.slice(0, HISTORY_CAP)
}

export async function mockIconsCall(method: string, params: unknown): Promise<unknown> {
  const p = (params ?? {}) as {
    revision?: number
    count?: number
    items?: { id: string; sourceIndex: number; masterPng: string }[]
    styleJson?: string
    label?: string | null
    sessionId?: string
  }
  switch (method) {
    case 'icons.getPersisted':
      return persisted()
    case 'icons.scan': {
      await probeRealWallpaper()
      const entries = await loadManifest()
      const rows = Math.floor((MOCK_GRID.screenHeight - MOCK_GRID.taskbarHeight - MOCK_GRID.inset * 2) / MOCK_GRID.cellHeight)
      const items = entries.map((e, i) => toItem(e, i, rows))
      session.styleableCount = items.filter((i) => i.styleable).length
      session.revision += 1
      // Observed grid metrics (the frontend assembles its grid from these). The mock's fake desktop
      // is a 1080p work area matching the item layout.
      return {
        revision: session.revision,
        items,
        grid: { screenWidth: MOCK_GRID.screenWidth, screenHeight: MOCK_GRID.screenHeight, taskbarHeight: MOCK_GRID.taskbarHeight },
      } satisfies IconScanDto
    }
    case 'icons.applyBakedBegin':
      session.bakePending = new Map()
      session.sessionId = String(++mockSessionCounter)
      return session.sessionId
    case 'icons.applyBakedChunk': {
      if (p.sessionId !== session.sessionId) throw new Error('mock: stale apply session token')
      for (const item of p.items ?? []) session.bakePending.set(`${item.id}#${item.sourceIndex}`, item.masterPng)
      return null
    }
    case 'icons.applyBakedCommit': {
      // A stale token (a newer Begin superseded this apply) → ok:false, never mutate (R3-Block 1).
      if (p.sessionId !== session.sessionId) {
        return { ok: false, toast: { key: 'Toast_ApplySuperseded', arg: null }, persisted: persisted() } satisfies IconOpResultDto
      }
      // Keep the last bake inspectable in the browser loop (debug menu).
      ;(window as { __dmBakedIcons?: Record<string, string> }).__dmBakedIcons = Object.fromEntries(
        [...session.bakePending].map(([id, png]) => [id, `data:image/png;base64,${png}`]),
      )
      const styleJson = p.styleJson ?? '{}'
      session.savedStyleJson = styleJson
      session.applied = true
      // Apply installs the global transparent overlay: the native arrow is hidden (ADR-0021).
      session.arrowOverlay = 'hidden'
      pushHistory(styleJson, p.label ?? '自定义')
      return { ok: true, toast: null, persisted: persisted() } satisfies IconOpResultDto
    }
    case 'icons.restore':
      session.applied = false
      session.savedStyleJson = null
      // Full restore lifts the overlay too (icons AND arrow back to native).
      session.arrowOverlay = 'native'
      return { ok: true, toast: null, persisted: persisted() } satisfies IconOpResultDto
    case 'icons.restoreOverlay': {
      // Keep-beautification restore: only the arrow overlay moves; the icon look is untouched.
      const res = overlayRestoreResult(restoreOutcome())
      session.arrowOverlay = res.arrowOverlay
      return { ok: res.ok, toast: { key: res.toastKey, arg: null }, persisted: persisted() } satisfies IconOpResultDto
    }
    case 'icons.exportCompare':
      // The before/after compare sheet is not yet implemented — honest ok:false (matches the real
      // host), never a phantom success (codex Major 5 / R2-Major 2).
      return { ok: false, toast: { key: 'Toast_CompareFailed', arg: null }, persisted: persisted() } satisfies IconOpResultDto
    default:
      throw new Error(`[mock desktop] unhandled method: ${method}`)
  }
}
