import type {
  FontChoiceDto,
  PresetEntryDto,
  PresetSaveDto,
  SettingsDto,
} from './types'
import { appInfo } from '@/lib/app-info'
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

// Curated fallback for hosts without the Local Font Access API (WKWebView / older
// runtimes / a denied permission). `__bundled__` (family null) is always first — it's
// the app's own default title font.
const mockFonts: FontChoiceDto[] = [
  { display: '__bundled__', family: null },
  { display: 'Segoe UI', family: 'Segoe UI' },
  { display: 'Microsoft YaHei UI', family: 'Microsoft YaHei UI' },
]

/** The real installed-font list via the web Local Font Access API (`queryLocalFonts`,
 *  available in WebView2/Chromium; absent in WKWebView/Safari). Wallpaper zone titles
 *  render purely in the web layer, so this is a web capability, not a Rust command.
 *  Falls back to the curated list when the API is missing or the permission is denied. */
async function listFonts(): Promise<FontChoiceDto[]> {
  const query = (window as unknown as { queryLocalFonts?: () => Promise<Array<{ family: string }>> })
    .queryLocalFonts
  if (typeof query !== 'function') return mockFonts.map((f) => ({ ...f }))
  try {
    const fonts = await query()
    const families = [...new Set(fonts.map((f) => f.family))].sort((a, b) => a.localeCompare(b))
    if (families.length === 0) return mockFonts.map((f) => ({ ...f }))
    return [{ display: '__bundled__', family: null }, ...families.map((family) => ({ display: family, family }))]
  } catch {
    // Permission denied or the API threw → the curated list keeps the picker usable.
    return mockFonts.map((f) => ({ ...f }))
  }
}

// In-memory preset library (spec 09 in the browser loop): same verbs, same
// copy/overwrite semantics, no fs. readPackage/export are honest stubs — the
// browser cannot touch local .dmpreset files; those paths need the Tauri host.
const mockPresetLibrary = new Map<string, PresetEntryDto>()

function mockPresetsCall(method: string, params: unknown): unknown {
  switch (method) {
    case 'presets.list':
      return [...mockPresetLibrary.values()]
    case 'presets.save': {
      const p = params as { entry: PresetSaveDto; overwrite: boolean }
      if (mockPresetLibrary.has(p.entry.id) && !p.overwrite) throw new Error('exists')
      const entry: PresetEntryDto = {
        id: p.entry.id,
        presetType: p.entry.presetType,
        schemaVersion: p.entry.schemaVersion,
        meta: { ...p.entry.meta },
        payloadJson: p.entry.payloadJson,
        hasThumb: false, // no dmpreset:// server in the browser loop
      }
      mockPresetLibrary.set(entry.id, entry)
      return entry
    }
    case 'presets.delete':
      mockPresetLibrary.delete((params as { entryId: string }).entryId)
      return null
    case 'presets.rename': {
      const p = params as { entryId: string; name: string }
      const entry = mockPresetLibrary.get(p.entryId)
      if (!entry) throw new Error('not found')
      entry.meta = { ...entry.meta, name: p.name }
      return entry
    }
    case 'presets.readPackage':
      return { formatOk: false, entries: [], error: 'mock bridge: package files need the desktop app' }
    case 'presets.export':
      throw new Error('mock bridge: export needs the desktop app')
    default:
      throw new Error(`[mock bridge] unhandled method: ${method}`)
  }
}

export async function mockCall(method: string, params: unknown): Promise<unknown> {
  if (method.startsWith('icons.')) return mockIconsCall(method, params)
  if (method.startsWith('wallpaper.')) return mockWallpaperCall(method, params)
  if (method.startsWith('presets.')) return mockPresetsCall(method, params)
  switch (method) {
    case 'app.getInfo':
      return appInfo
    case 'diagnostics.getInfo':
      return {
        osVersion: `${navigator.platform || 'browser'} (mock)`,
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
      return listFonts()
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
