import type {
  AppInfoDto,
  FontChoiceDto,
  LookDto,
  SettingsDto,
  WallpaperOpDto,
  WallpaperStateDto,
} from './types'
import { BRIDGE_SCHEMA_VERSION } from './types'
import { mockIconsCall, mockWallpaperUrl, probeRealWallpaper } from './mock-desktop'

// Browser-only development fallback: lets `bun run dev` render every surface in a
// normal browser (design iteration) without the WebView2 host. Never shipped paths —
// the hosted app always talks to the real bridge.

const settings: SettingsDto = {
  theme: 'System',
  language: 'System',
  keepNewIconsStyled: false,
  wallpaperCoachShown: false,
}

// Mock wallpaper state — a plausible 1920×1080 desktop grid so the paper module and
// the store tests exercise real geometry without the Windows host. wallTint is warm
// on purpose (blue/violet accents are banned, and this file is colour-scanned).
const wallpaperLook: LookDto = {
  zones: [],
  // angleDeg 0 = scrim from the TOP — mirrors ClarityConfig's engine default.
  clarity: { level: 'Off', gradient: 'Linear', angleDeg: 0, dimOverride: null, tone: 'Dark', customScrim: null },
}

let wallpaperState: WallpaperStateDto = {
  look: wallpaperLook,
  hasBackup: false,
  working: false,
  dirty: false,
  pale: false,
  fingerprintMismatch: false,
  wallTint: '#7A6E62',
  grid: {
    screenWidth: 1920,
    screenHeight: 1080,
    taskbarHeight: 48,
    iconPx: 48,
    cellWidth: 92,
    cellHeight: 92,
    inset: 14,
    columns: 20,
    rows: 11,
  },
  originalUrl: null,
}

// The browser fallback lazily attaches the canvas-generated scene so the wallpaper
// module has an image to show (the composed shared-buffer frames are host-only).
function withScene(state: WallpaperStateDto): WallpaperStateDto {
  return state.originalUrl ? state : { ...state, originalUrl: mockWallpaperUrl() }
}

const mockFonts: FontChoiceDto[] = [
  { display: '__bundled__', family: null },
  { display: 'Segoe UI', family: 'Segoe UI' },
  { display: 'Microsoft YaHei UI', family: 'Microsoft YaHei UI' },
]

const appInfo: AppInfoDto = {
  schemaVersion: BRIDGE_SCHEMA_VERSION,
  version: '0.0.0',
  productNameZh: '桌面美颜',
  productNameEn: 'DeskMakeover',
  effectiveDark: typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches,
  links: {
    repo: 'https://github.com/nicepkg/deskmakeover',
    releases: 'https://github.com/nicepkg/deskmakeover/releases',
    issues: 'https://github.com/nicepkg/deskmakeover/issues',
    email: '2214962083@qq.com',
    homepage: 'https://github.com/nicepkg/deskmakeover', // owner: homepage IS the repo
    githubProfile: 'https://github.com/2214962083',
    x: 'https://x.com/jinmingyang666',
    bilibili: 'https://space.bilibili.com/83540912',
    douyin: 'https://www.douyin.com/user/MS4wLjABAAAAAHGEUOQlkdfgHzzs88wWgKWwl2wyEcRYvodqmwfvK_k',
  },
  changelogZh: [
    { version: '未发布 · 开发预览', items: ['全新 v3 视觉语言：浅色优先、内置字体', '画布工具条重做，触控板手势打磨', '应用前确认与完成引导'] },
  ],
  changelogEn: [
    { version: 'Unreleased · Preview', items: ['New v3 visual language: light-first, bundled fonts', 'Reworked canvas toolbar, trackpad gestures', 'Apply consent & completion flow'] },
  ],
}

export async function mockCall(method: string, params: unknown): Promise<unknown> {
  if (method.startsWith('icons.')) return mockIconsCall(method, params)
  switch (method) {
    case 'app.getInfo':
      return appInfo
    case 'diagnostics.getInfo':
      return {
        osVersion: `${navigator.platform || 'browser'} (mock)`,
        dotnetVersion: '.NET 8.0 (mock)',
        webview2Version: 'browser (mock)',
        arch: 'x64 (mock)',
        hostLogTail: [],
      }
    case 'settings.get':
      return { ...settings }
    case 'settings.set':
      Object.assign(settings, params as Partial<SettingsDto>)
      return { ...settings }
    case 'wallpaper.getState':
      await probeRealWallpaper()
      return withScene({ ...wallpaperState })
    case 'wallpaper.getSource': {
      await probeRealWallpaper()
      const url = withScene(wallpaperState).originalUrl!
      return { url, width: wallpaperState.grid.screenWidth, height: wallpaperState.grid.screenHeight }
    }
    case 'wallpaper.setLook': {
      const look = (params as { look: LookDto }).look
      wallpaperState = {
        ...wallpaperState,
        look,
        dirty: look.zones.length > 0 || look.clarity.level !== 'Off',
      }
      return null
    }
    case 'wallpaper.applyBaked': {
      // Keep the baked PNG inspectable in the browser loop (debug menu can open it).
      const { pngBase64, look } = params as { pngBase64: string; look: LookDto }
      ;(window as { __dmBakedPng?: string }).__dmBakedPng = `data:image/png;base64,${pngBase64}`
      wallpaperState = { ...wallpaperState, look, hasBackup: true, dirty: false, working: false }
      return { state: withScene({ ...wallpaperState }), toast: null, ok: true } satisfies WallpaperOpDto
    }
    case 'wallpaper.restore':
      wallpaperState = { ...wallpaperState, hasBackup: false, dirty: false, working: false }
      return { state: withScene({ ...wallpaperState }), toast: null, ok: true } satisfies WallpaperOpDto
    case 'fonts.list':
      return mockFonts.map((f) => ({ ...f }))
    case 'shell.openExternal':
      window.open((params as { url: string }).url, '_blank')
      return null
    case 'shell.minimize':
    case 'shell.maximize':
    case 'shell.restore':
    case 'shell.close':
    case 'shell.openDataFolder':
      console.info(`[mock bridge] ${method}`)
      return null
    default:
      throw new Error(`[mock bridge] unhandled method: ${method}`)
  }
}
