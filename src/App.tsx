import { useEffect, useState } from 'react'
import { TitleBar } from '@/components/shell/title-bar'
import { ModuleRail } from '@/components/shell/module-rail'
import { SettingsPage } from '@/components/panels/settings-page'
import { IconsPanel } from '@/components/panels/icons-panel'
import { IconsMirror } from '@/components/canvas/icons-mirror'
import { WallpaperPanel } from '@/components/panels/wallpaper-panel'
import { WallpaperMirror } from '@/components/canvas/wallpaper-mirror'
import { ModuleLayout } from '@/components/shell/module-layout'
import { useWallpaper } from '@/stores/wallpaper'
import { ToastHost } from '@/components/common/toast-host'
import { ChangelogDialog, shouldAutoShowChangelog } from '@/components/common/changelog-dialog'
import { WelcomeGate, welcomePending } from '@/components/shell/welcome-gate'
import { CrashProbe } from '@/components/shell/crash-gate'
import { ComponentGallery } from '@/components/debug/component-gallery'
import { useApp } from '@/stores/app'
import type { AppModule } from '@/stores/app'
import { useIcons } from '@/stores/icons'
import { useCalm } from '@/stores/calm'
import { CalmPage } from '@/components/panels/calm-page'

export default function App() {
  if (new URLSearchParams(window.location.search).get('debug') === 'components') {
    return <ComponentGallery />
  }
  return <Shell />
}

function Shell() {
  const boot = useApp((s) => s.boot)
  const booted = useApp((s) => s.booted)
  const module = useApp((s) => s.module)
  const setModule = useApp((s) => s.setModule)
  const info = useApp((s) => s.info)
  const [updateLogOpen, setUpdateLogOpen] = useState(false)
  const [welcomeOpen, setWelcomeOpen] = useState(welcomePending)

  // Post-update ritual (owner model): the changelog auto-opens exactly ONCE after
  // a new version is installed — never on first install, never again after.
  useEffect(() => {
    if (booted && info && shouldAutoShowChangelog(info.version)) setUpdateLogOpen(true)
  }, [booted, info])

  useEffect(() => {
    void boot()
  }, [boot])

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      // Ctrl+1/2/3/4 = module switch (spec 03; 清爽 = Ctrl+3, 设置 stays last).
      if (e.ctrlKey && !e.altKey && !e.shiftKey && ['1', '2', '3', '4'].includes(e.key)) {
        const target: AppModule = e.key === '1' ? 'icons' : e.key === '2' ? 'paper' : e.key === '3' ? 'calm' : 'settings'
        setModule(target)
        e.preventDefault()
        return
      }

      // Ctrl+Z / Ctrl+Shift+Z (+ Ctrl+Y) = undo/redo for the active module's history
      // (wallpaper when in `paper`; spec 04 §3.5). Never hijack native text undo.
      const key = e.key.toLowerCase()
      if (
        e.ctrlKey && !e.altKey && (key === 'z' || key === 'y') &&
        !(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)
      ) {
        if (useApp.getState().module === 'paper') {
          const wallpaper = useWallpaper.getState()
          if (key === 'y' || e.shiftKey) wallpaper.redo()
          else wallpaper.undo()
          e.preventDefault()
        }
        return
      }

      // Esc deselects the selected wallpaper zone FIRST (spec 04 §3.5). A single Esc
      // with a zone selected only deselects — it does NOT also close the compact panel
      // overlay in the same press. With no zone selected, fall through to that behavior.
      if (
        e.key === 'Escape' &&
        useApp.getState().module === 'paper' &&
        useWallpaper.getState().selected !== null
      ) {
        useWallpaper.getState().select(null)
        e.preventDefault()
        return
      }

      // Hold Space = 对比原样, GLOBALLY (owner call, spec 01) — by design it is NOT stolen
      // by a focused button, because this UI is button-dense and a just-clicked swatch/chip
      // keeps focus; if Space activated that button instead, the compare gesture would fail
      // exactly when the user reaches for it. Buttons stay keyboard-activatable via ENTER, so
      // nothing is stranded. Only text entry (space is a real character) is excluded.
      if (
        e.code === 'Space' &&
        !e.repeat &&
        !(e.target instanceof HTMLInputElement) &&
        !(e.target instanceof HTMLTextAreaElement)
      ) {
        useIcons.getState().setComparing(true)
        useWallpaper.getState().setComparing(true)
        e.preventDefault()
      }
    }
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.code === 'Space') {
        useIcons.getState().setComparing(false)
        useWallpaper.getState().setComparing(false)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('keyup', onKeyUp)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('keyup', onKeyUp)
    }
  }, [setModule])

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <TitleBar />
      <div className="relative flex min-h-0 flex-1">
        <ModuleRail />
        {/* Module switch is INSTANT and modules stay MOUNTED (owner calls
            2026-07-09): unmounting tore down the wallpaper compositor every
            switch. Hiding uses VISIBILITY, not display:none — display:none
            zeroed the hidden module's viewport, so its first re-shown frame
            painted with garbage fit state (canvas at scale 0.1 top-left for
            1-2 frames = the flash, caught on a rAF frame recorder). visibility
            keeps layout alive: observers hold real sizes, nothing settles. */}
        <div className="relative min-w-0 flex-1">
          {booted && (
            <>
              {(
                [
                  ['settings', <SettingsPage key="s" />],
                  ['icons', <IconsModule key="i" />],
                  ['paper', <PaperModule key="p" />],
                  ['calm', <CalmModule key="c" />],
                ] as const
              ).map(([id, node]) => (
                <div
                  key={id}
                  aria-hidden={module !== id}
                  inert={module !== id || undefined}
                  className={
                    module === id
                      ? 'absolute inset-0 flex min-w-0'
                      : 'pointer-events-none invisible absolute inset-0 flex min-w-0'
                  }
                >
                  {node}
                </div>
              ))}
            </>
          )}
        </div>
        {/* First-run doorway: covers everything below the title bar (the window
            stays movable/closable) and exits into the already-booted app. */}
        <WelcomeGate open={welcomeOpen} onDone={() => setWelcomeOpen(false)} />
      </div>
      <ToastHost />
      <CrashProbe />
      <ChangelogDialog open={updateLogOpen} onOpenChange={setUpdateLogOpen} />
    </div>
  )
}

function IconsModule() {
  const scan = useIcons((s) => s.scan)
  useEffect(() => {
    void scan()
  }, [scan])
  return <ModuleLayout inspector={<IconsPanel />} mirror={<IconsMirror />} />
}

function PaperModule() {
  const load = useWallpaper((s) => s.load)
  useEffect(() => {
    void load()
  }, [load])
  return <ModuleLayout inspector={<WallpaperPanel />} mirror={<WallpaperMirror />} />
}

function CalmModule() {
  const probe = useCalm((s) => s.probe)
  useEffect(() => {
    void probe()
  }, [probe])
  return <CalmPage />
}
