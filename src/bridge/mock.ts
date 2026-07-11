import type {
  AppInfoDto,
  FontChoiceDto,
  SettingsDto,
} from './types'
import { BRIDGE_SCHEMA_VERSION } from './types'
import { mockIconsCall } from './mock-desktop'
import { mockWallpaperCall } from './mock-wallpaper'

// Browser-only development fallback: lets `bun run dev` render every surface in a
// normal browser (design iteration) without the WebView2 host. Never shipped paths —
// the hosted app always talks to the real bridge.

const settings: SettingsDto = {
  theme: 'System',
  language: 'System',
  keepNewIconsStyled: false,
  wallpaperCoachShown: false,
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
  if (method.startsWith('wallpaper.')) return mockWallpaperCall(method, params)
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
