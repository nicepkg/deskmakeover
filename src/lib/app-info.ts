import type { AppInfoDto } from '@/bridge/types'
import { BRIDGE_SCHEMA_VERSION } from '@/bridge/types'

// The app's identity blob (links, product names, changelog) is FRONTEND content —
// single-sourced here so the browser mock and the Tauri host serve the same facts.
// Only `version` is host truth: the Tauri bridge overrides it with the real app
// version (the mock's 0.0.0 leaking into the About card was the owner's 2026-07-16
// "版本 0.0.0" report).
export const appInfo: AppInfoDto = {
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
