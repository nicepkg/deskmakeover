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
  'wallpaper.getSource',
  'wallpaper.applyBaked',
  'wallpaper.restore',
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
    case 'settings.set':
      return unwrap(await commands.settingsSet(params as Parameters<typeof commands.settingsSet>[0]))
    // Wallpaper (schema 6 thin, D1): the store reconciles + assembles; Rust returns
    // raw screens (getScreens) / the active source (getSource) / a thin op result.
    case 'wallpaper.getScreens':
      return unwrap(await commands.wallpaperGetScreens())
    case 'wallpaper.getSource':
      return unwrap(await commands.wallpaperGetSource())
    case 'wallpaper.applyBaked': {
      const p = params as { monitorId: string; pngBase64: string }
      return unwrap(await commands.wallpaperApplyBaked(p.monitorId, p.pngBase64))
    }
    case 'wallpaper.restore': {
      const p = params as { monitorId: string }
      return unwrap(await commands.wallpaperRestore(p.monitorId))
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
