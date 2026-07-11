import * as React from 'react'
import { CtaButton } from '@/components/common/cta-button'
import { Confetti, useCelebration } from '@/components/common/confetti'
import { ConfirmSheet, DoneCard } from '@/components/common/ceremony'
import { usePaperHero } from '@/lib/hero'
import { IconAction, Reveal, useFooterClearance } from '@/components/common/inspector'
import { ArchiveRestore, Download, ImageUp, Layers, MousePointerClick, RotateCcw, X } from 'lucide-react'
import { WallpaperDimCard } from '@/components/panels/wallpaper-dim-card'
import { WallpaperScreenNotices } from '@/components/panels/wallpaper-screen-notices'
import { WallpaperZoneInspector } from '@/components/panels/wallpaper-zone-inspector'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useToasts } from '@/stores/toasts'
import { useWallpaper } from '@/stores/wallpaper'
import { activeScreenFacts } from '@/lib/screen-arrange'
import { format, useT } from '@/lib/i18n'
import type { StringKey } from '@/lib/i18n'

// The 壁纸 panel (spec 04 v2.0, ADR-0014): session bar (source import) + per-screen
// notices + 壁纸压暗 (the beauty story) + the 分区 inspector (WallpaperZoneInspector,
// split out for the ≤500 law) + the apply/restore/export footer ceremony. The CTA
// names its target screen and the first destructive apply over a live wallpaper is
// confirmed once (§B5/A4).

export function WallpaperPanel() {
  const t = useT()
  const { phase, statusText, ctaText } = usePaperHero()
  const state = useWallpaper((s) => s.state)
  const look = useWallpaper((s) => s.look)
  const sourceName = useWallpaper((s) => s.sourceName)
  const sourceUrl = useWallpaper((s) => s.sourceUrl)
  const { apply, restore, importSourceViaPicker, resetSource, exportImage } = useWallpaper.getState()
  const [restoreOpen, setRestoreOpen] = React.useState(false)
  const [doneOpen, setDoneOpen] = React.useState(false)
  // First destructive apply over a live (slideshow/dynamic) wallpaper is confirmed
  // once, then remembered per screen for the session (§A4).
  const [dynamicConfirmOpen, setDynamicConfirmOpen] = React.useState(false)
  const [confirmedScreens, setConfirmedScreens] = React.useState<Set<string>>(new Set())
  const { footerRef, clearance } = useFooterClearance()
  const { celebrateKey, celebrate } = useCelebration('wallpaper')

  // Active-screen facts for the CTA rename + dynamic-apply gate (§B5/A4). The
  // per-screen header + dynamic banners render from the same facts in
  // WallpaperScreenNotices. A single-monitor host yields multiScreen === false.
  const { activeIndex, multiScreen, liveWallpaper } = activeScreenFacts(state)
  const needsDynamicConfirm = !!state && liveWallpaper && !confirmedScreens.has(state.activeScreenId)
  // The CTA names its target so an apply can never silently hit the wrong monitor
  // (§B5). Only the actionable phases rename; working/synced keep their own copy.
  const ctaLabel =
    multiScreen && activeIndex >= 0 && (phase === 'ready' || phase === 'dirty')
      ? format(t('Paper_Cta_ApplyScreen'), activeIndex + 1)
      : ctaText

  const doApply = async () => {
    // Gate the DoneCard on THIS apply's result — hasBackup stays true from any
    // earlier success and would celebrate a failed apply (codex review M6).
    const ok = await apply()
    if (!ok) return
    // Same celebration as the icons apply (DRY): confetti on the FIRST successful
    // wallpaper apply of each launch, then the DoneCard.
    celebrate()
    setDoneOpen(true)
  }

  const runApply = async () => {
    if (needsDynamicConfirm) {
      setDynamicConfirmOpen(true)
      return
    }
    await doApply()
  }

  // 导出图片 never touches the desktop — it saves the composed PNG locally.
  const runExport = async () => {
    const filename = await exportImage()
    const show = useToasts.getState().show
    if (filename) show(format(t('Toast_PaperExported'), filename), 'success')
    else show(t('Toast_PaperExportFailed'), 'warn')
  }

  if (!state || !look) return <aside className="w-[280px] shrink-0" />

  return (
    <aside className="relative flex w-[280px] shrink-0 flex-col gap-2.5 pl-1 pr-3 pt-1">
      <div style={{ paddingBottom: clearance }} className="scrollbar-none flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto [&>*]:shrink-0">
      {/* Session bar: source-in lives here (IA 2026-07-09). Default = status
          text + quiet import entry; imported = source chip + cancel. Only the
          NON-default state gets labelled. Lives INSIDE the scroller — it rides
          along with the panel (owner call 2026-07-09), never floats. */}
      <div className="flex items-center gap-1 px-0.5">
        {sourceName === null ? (
          <p className="min-w-0 flex-1 truncate text-[11px] text-t3/90">{statusText}</p>
        ) : (
          <button
            type="button"
            title={t('Paper_Import_Tip')}
            onClick={() => importSourceViaPicker()}
            className="flex min-w-0 flex-1 items-center gap-1.5 rounded-[7px] px-1 py-0.5 text-left transition-colors hover:bg-raised-hov"
          >
            {sourceUrl && <img src={sourceUrl} alt="" className="size-4 shrink-0 rounded-[4px] object-cover" />}
            <span className="min-w-0 truncate text-[11px] text-t2">{t('Paper_SourceImported')}</span>
          </button>
        )}
        {sourceName === null ? (
          <button
            type="button"
            aria-label={t('Paper_Import')}
            title={t('Paper_Import_Tip')}
            onClick={() => importSourceViaPicker()}
            className="flex size-4 shrink-0 items-center justify-center rounded-full text-t3/80 ring-1 ring-hair transition-colors hover:bg-raised-hov hover:text-t1"
          >
            <ImageUp size={10} />
          </button>
        ) : (
          <button
            type="button"
            aria-label={t('Paper_ImportCancel')}
            title={t('Paper_ImportCancel_Tip')}
            onClick={() => void resetSource()}
            className="flex size-4 shrink-0 items-center justify-center rounded-full text-t3/80 ring-1 ring-hair transition-colors hover:bg-raised-hov hover:text-t1"
          >
            <X size={10} />
          </button>
        )}
        <Popover>
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label={t('PaperHow_Title')}
              className="flex size-4 shrink-0 items-center justify-center rounded-full text-[10px] leading-none text-t3/80 ring-1 ring-hair transition-colors hover:bg-raised-hov hover:text-t1"
            >
              ?
            </button>
          </PopoverTrigger>
          <PopoverContent side="bottom" align="end" className="w-[252px] rounded-[12px] p-3">
            <p className="mb-2 text-[11px] font-medium text-t1">{t('PaperHow_Title')}</p>
            <div className="space-y-2">
              {(
                [
                  [Layers, 'PaperHow_Zones'],
                  [MousePointerClick, 'PaperHow_Icons'],
                  [ArchiveRestore, 'PaperHow_Backup'],
                ] as [typeof Layers, StringKey][]
              ).map(([Icon, key]) => (
                <div key={key} className="flex items-start gap-2">
                  <Icon size={12} className="mt-px shrink-0 text-t3" />
                  <p className="min-w-0 text-caption leading-snug text-t2">{t(key)}</p>
                </div>
              ))}
            </div>
          </PopoverContent>
        </Popover>
      </div>

        {/* Per-screen header + Span note + dynamic-wallpaper banners (§B5/A4/B6). */}
        <WallpaperScreenNotices />

        <Reveal show={state.fingerprintMismatch}>
          <button
            type="button"
            onClick={() => void useWallpaper.getState().refresh()}
            className="w-full rounded-[10px] bg-amber-wash px-3 py-2 text-left text-caption text-amber"
          >
            {t('Paper_Mismatch')} · {t('Paper_Regenerate')}
          </button>
        </Reveal>

        <WallpaperDimCard />

        {/* 分区 — the power capability (split to its own file for the ≤500 law). */}
        <WallpaperZoneInspector />
      </div>

      <div
        ref={footerRef}
        className="absolute inset-x-0 bottom-0 z-20 bg-gradient-to-t from-background from-65% via-background/55 to-transparent"
      >
        <div className="flex flex-col gap-1.5 pb-3 pl-1 pr-3 pt-5">
          {/* Secondary link slot: result-out actions (restore / export) — quiet
              peers beside the sacred CTA, shown only when meaningful. */}
          <Reveal show={state.hasBackup || state.dirty}>
            <div className="flex items-center gap-1">
              {state.hasBackup && (
                <IconAction title={t('Paper_Restore')} onClick={() => setRestoreOpen(true)}>
                  <RotateCcw size={11} />
                  {t('Paper_Restore')}
                </IconAction>
              )}
              {state.dirty && (
                <IconAction title={t('Paper_Export_Tip')} onClick={() => void runExport()}>
                  <Download size={11} />
                  {t('Paper_Export')}
                </IconAction>
              )}
            </div>
          </Reveal>
          <CtaButton phase={phase} onClick={() => void runApply()}>
            {ctaLabel}
          </CtaButton>
        </div>
      </div>

      <ConfirmSheet
        open={restoreOpen}
        title={t('RestoreConfirm')}
        confirmLabel={t('Paper_Restore')}
        cancelLabel={t('ConsentCancel')}
        destructive
        onConfirm={() => {
          setRestoreOpen(false)
          void restore()
        }}
        onCancel={() => setRestoreOpen(false)}
      />
      <ConfirmSheet
        open={dynamicConfirmOpen}
        title={t('Paper_DynamicReplaceConfirm')}
        confirmLabel={t('Paper_DynamicReplaceCta')}
        cancelLabel={t('ConsentCancel')}
        destructive
        onConfirm={() => {
          setDynamicConfirmOpen(false)
          if (state) setConfirmedScreens((prev) => new Set(prev).add(state.activeScreenId))
          void doApply()
        }}
        onCancel={() => setDynamicConfirmOpen(false)}
      />
      <DoneCard
        open={doneOpen}
        onClose={() => setDoneOpen(false)}
        note={t('Done_LastStep')}
        ctaLabel={t('Done_GoOrganize')}
      />

      {/* First-of-launch confetti — same shared celebration as the icons apply. */}
      <Confetti fireKey={celebrateKey} />
    </aside>
  )
}
