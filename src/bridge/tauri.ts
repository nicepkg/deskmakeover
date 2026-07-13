import { emit } from './client'

// Tauri host bridge. Under Tauri, the verbs Rust owns route here; everything
// else falls through to the mock (see client.ts). The Tauri APIs are loaded
// lazily so the browser mock loop and `bun test` never pull in @tauri-apps/api.

/** True only inside a Tauri WebView (v2 injects `__TAURI_INTERNALS__`). */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

// Settings persistence is real (Rust/rusqlite); the frameless titlebar's window
// controls drive the real window. M6-WIRE Wave A: the wallpaper verbs now route to
// the real Rust command path (schema 6 thin contract, D1) — get screen info,
// get/set (bake) wallpaper, restore snapshot. Icons + the rest stay mock until Wave B.
const HANDLED = new Set([
  'settings.get',
  'settings.set',
  'shell.minimize',
  'shell.maximize',
  'shell.restore',
  'shell.close',
  'wallpaper.getScreens',
  'wallpaper.applyBaked',
  'wallpaper.restore',
  // Icons (schema 7 thin, D1): the store assembles IconsStateDto; Rust returns raw items +
  // persisted bits. setLook is NOT here — it left the bridge (frontend draft).
  'icons.scan',
  'icons.getPersisted',
  'icons.applyBakedBegin',
  'icons.applyBakedChunk',
  'icons.applyBakedCommit',
  'icons.restore',
  'icons.restoreOverlay',
  'icons.switchVersion',
  'icons.exportCompare',
  'shell.openExternal',
  'shell.openDataFolder',
  // Diagnostics (audit #7): real host facts, not the browser `(mock)` stub.
  'diagnostics.getInfo',
])

export function tauriHandles(method: string): boolean {
  return HANDLED.has(method)
}

type OkErr<T> = { status: 'ok'; data: T } | { status: 'error'; error: string }

function unwrap<T>(result: OkErr<T>): T {
  if (result.status === 'error') throw new Error(result.error)
  return result.data
}

interface TauriApi {
  commands: typeof import('./generated')['commands']
  getCurrentWindow: typeof import('@tauri-apps/api/window')['getCurrentWindow']
}

let apiPromise: Promise<TauriApi> | null = null

function api(): Promise<TauriApi> {
  if (!apiPromise) {
    apiPromise = Promise.all([import('./generated'), import('@tauri-apps/api/window')]).then(
      ([generated, windowApi]) => ({
        commands: generated.commands,
        getCurrentWindow: windowApi.getCurrentWindow,
      }),
    )
  }
  return apiPromise
}

export async function tauriCall(method: string, params: unknown): Promise<unknown> {
  const { commands, getCurrentWindow } = await api()
  switch (method) {
    case 'settings.get':
      return unwrap(await commands.settingsGet())
    case 'diagnostics.getInfo':
      return unwrap(await commands.diagnosticsGetInfo())
    case 'settings.set':
      return unwrap(await commands.settingsSet(params as Parameters<typeof commands.settingsSet>[0]))
    // Wallpaper (schema 6 thin, D1): the store reconciles + assembles; Rust returns
    // raw screens (getScreens) / a thin op result.
    case 'wallpaper.getScreens':
      return unwrap(await commands.wallpaperGetScreens())
    case 'wallpaper.applyBaked': {
      const p = params as { monitorId: string; pngBase64: string }
      return unwrap(await commands.wallpaperApplyBaked(p.monitorId, p.pngBase64))
    }
    case 'wallpaper.restore': {
      const p = params as { monitorId: string }
      return unwrap(await commands.wallpaperRestore(p.monitorId))
    }
    // Icons (schema 7 thin, D1): map Rust's raw scan items into the store-facing IconItemDto
    // (adding the frontend-owned override slots, which the store fills from its own draft); every
    // other icon DTO is structurally identical, so it passes through.
    case 'icons.scan': {
      const scan = unwrap(await commands.iconsScan())
      return {
        revision: scan.revision,
        grid: scan.grid,
        items: scan.items.map((it) => ({ ...it, overrideMode: null, overrideTint: null })),
      }
    }
    case 'icons.getPersisted':
      return unwrap(await commands.iconsGetPersisted())
    case 'icons.applyBakedBegin': {
      const p = params as { revision: number; count: number }
      return unwrap(await commands.iconsApplyBakedBegin(p.revision, p.count))
    }
    case 'icons.applyBakedChunk': {
      const p = params as { sessionId: string; items: { id: string; sourceIndex: number; masterPng: string }[] }
      return unwrap(await commands.iconsApplyBakedChunk(p.sessionId, p.items))
    }
    case 'icons.applyBakedCommit': {
      const p = params as { sessionId: string; styleJson: string; restoreIds: string[]; label: string | null }
      return unwrap(await commands.iconsApplyBakedCommit(p.sessionId, p.styleJson, p.restoreIds, p.label))
    }
    case 'icons.restore':
      return unwrap(await commands.iconsRestore())
    case 'icons.restoreOverlay':
      return unwrap(await commands.iconsRestoreOverlay())
    case 'icons.switchVersion': {
      const p = params as { versionId: string }
      return unwrap(await commands.iconsSwitchVersion(p.versionId))
    }
    case 'icons.exportCompare': {
      const p = params as { png: string }
      return unwrap(await commands.iconsExportCompare(p.png))
    }
    case 'shell.minimize':
      await getCurrentWindow().minimize()
      return null
    case 'shell.maximize':
      await getCurrentWindow().maximize()
      return null
    case 'shell.restore':
      await getCurrentWindow().unmaximize()
      return null
    case 'shell.close':
      await getCurrentWindow().close()
      return null
    case 'shell.openExternal': {
      const { openUrl } = await import('@tauri-apps/plugin-opener')
      await openUrl((params as { url: string }).url)
      return null
    }
    case 'shell.openDataFolder': {
      const [{ openPath }, { appDataDir }] = await Promise.all([
        import('@tauri-apps/plugin-opener'),
        import('@tauri-apps/api/path'),
      ])
      await openPath(await appDataDir())
      return null
    }
    default:
      throw new Error(`[tauri bridge] unhandled method: ${method}`)
  }
}

// Keep the titlebar's maximize/restore glyph in sync with the real window state
// by feeding the bridge's own `window-state` event stream.
async function startWindowStateSync(): Promise<void> {
  const { getCurrentWindow } = await api()
  const win = getCurrentWindow()
  const push = async () => {
    try {
      emit('window-state', { maximized: await win.isMaximized() })
    } catch {
      /* window gone during teardown */
    }
  }
  await push()
  await win.onResized(() => void push())
}

if (isTauri()) {
  void startWindowStateSync()
}
