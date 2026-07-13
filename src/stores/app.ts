import { create } from 'zustand'
import { call, on } from '@/bridge/client'
import { BRIDGE_SCHEMA_VERSION } from '@/bridge/types'
import type { AppInfoDto, SettingsDto } from '@/bridge/types'
import { useI18n } from '@/lib/i18n'
import { useToasts } from '@/stores/toasts'

// App-level session state: module routing, window state, settings, app info.
// One boot() wires the bridge; settings changes flow bridge → store → i18n/theme.

export type AppModule = 'icons' | 'paper' | 'calm' | 'settings'

interface AppState {
  booted: boolean
  module: AppModule
  maximized: boolean
  osDark: boolean
  compact: boolean
  panelOpen: boolean
  info: AppInfoDto | null
  settings: SettingsDto | null
  setModule: (module: AppModule) => void
  setPanelOpen: (open: boolean) => void
  updateSettings: (patch: Partial<SettingsDto>) => Promise<void>
  boot: () => Promise<void>
}

/** Below this width the control panel becomes a left slide-in overlay (spec 01). */
const COMPACT_BREAKPOINT = 1100

function applyTheme(settings: SettingsDto | null, osDark: boolean) {
  const theme = settings?.theme ?? 'System'
  const dark = theme === 'Dark' || (theme === 'System' && osDark)
  document.documentElement.className = dark ? 'dark' : 'light'
}

export const useApp = create<AppState>((set, get) => ({
  booted: false,
  module: 'icons',
  maximized: false,
  osDark: true,
  compact: false,
  panelOpen: false,
  info: null,
  settings: null,

  setModule: (module) => set({ module, panelOpen: false }),

  setPanelOpen: (panelOpen) => set({ panelOpen }),

  updateSettings: async (patch) => {
    const settings = await call('settings.set', patch)
    set({ settings })
    useI18n.getState().setPreference(settings.language)
    applyTheme(settings, get().osDark)
  },

  boot: async () => {
    if (get().booted) return
    set({ booted: true, compact: window.innerWidth < COMPACT_BREAKPOINT })

    window.addEventListener('resize', () =>
      set({ compact: window.innerWidth < COMPACT_BREAKPOINT }))

    on('window-state', ({ maximized }) => set({ maximized }))
    on('settings-changed', (settings) => {
      set({ settings })
      useI18n.getState().setPreference(settings.language)
      applyTheme(settings, get().osDark)
    })
    on('os-theme-changed', ({ dark }) => {
      set({ osDark: dark })
      applyTheme(get().settings, dark)
    })

    try {
      const [info, settings] = await Promise.all([call('app.getInfo'), call('settings.get')])
      if (info.schemaVersion !== BRIDGE_SCHEMA_VERSION) {
        // Loud drift failure beats silent mis-deserialization (spec 05 §3).
        console.error(`bridge schema mismatch: host=${info.schemaVersion} web=${BRIDGE_SCHEMA_VERSION}`)
        useToasts.getState().show(`bridge schema mismatch (host ${info.schemaVersion} / web ${BRIDGE_SCHEMA_VERSION})`, 'warn')
      }
      set({ info, settings, osDark: info.effectiveDark })
      useI18n.getState().setPreference(settings.language)
      applyTheme(settings, info.effectiveDark)
    } catch (err) {
      // A failed handshake must be retryable — otherwise the app is stuck blank.
      console.error('boot handshake failed', err)
      set({ booted: false })
    }
  },
}))
