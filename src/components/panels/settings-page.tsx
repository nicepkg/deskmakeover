import * as React from 'react'
import type { ReactNode } from 'react'
import { Bug, CircleHelp, ClipboardCopy, Download, FolderOpen, ImageDown, Mail, MessageSquare, RotateCcw, ScrollText, Tv, Undo2 } from 'lucide-react'
import { call } from '@/bridge/client'
const appIcon = '/app-icon.svg'
import { InspectorCard } from '@/components/common/inspector'
import { ConfirmSheet } from '@/components/common/ceremony'
import { FullPage } from '@/components/shell/full-page'
import { arrowRowView } from '@/lib/arrow-overlay'
import { Segmented } from '@/components/common/segmented'
import { ToggleSwitch } from '@/components/common/toggle-switch'
import { useApp } from '@/stores/app'
import { useIcons } from '@/stores/icons'
import { usePresetLibrary } from '@/stores/preset-library'
import { ChangelogDialog } from '@/components/common/changelog-dialog'
import { format, useT } from '@/lib/i18n'
import { buildReport, copyText, issueUrl, mailtoUrl } from '@/lib/diagnostics'
import { useToasts } from '@/stores/toasts'
import type { StringKey } from '@/lib/i18n'
import { cn } from '@/lib/utils'

// 设置 (spec 02 v3): ONE grouped inset card per column — never a scatter of
// look-alike cardlets. Right = the working settings as hairline rows. Left =
// identity: trust facts are a quiet dotted TEXT line (statements), links are
// text links — only real push-buttons wear a chip fill, so the text/button
// hierarchy reads at a glance.
//
// Scale: this is a full PAGE, not a 280px inspector — it uses page-scale type
// (13px labels, md controls, 54px rows), one notch up from the inspector
// dialect, matching macOS System Settings density instead of shrinking to
// side-panel sizes that read miniature on a maximized window.

// The M7 resident (spec 07) now consumes `keepNewIconsStyled` — the tray loop polls it every
// heartbeat, so this switch and the tray ☑自动整理新图标 are ONE state. (The 2026-07-10 hide was
// for the pre-resident era when nothing consumed the flag.)
const SHOW_KEEP_UP = true

const TRUST_CHIPS: StringKey[] = [
  'About_Chip_Local',
  'About_Chip_NoAccount',
  'About_Chip_NoTelemetry',
  'About_Chip_Reversible',
  'About_Chip_OpenSource',
]

export function SettingsPage() {
  const t = useT()
  const compact = useApp((s) => s.compact)
  const [logOpen, setLogOpen] = React.useState(false)
  const info = useApp((s) => s.info)
  const settings = useApp((s) => s.settings)
  const updateSettings = useApp((s) => s.updateSettings)
  // Native shortcut-arrow state (panel record 2026-07-11): the status text here
  // is the authority. Undefined = state not yet resolved (scan pending/failed) →
  // "checking", NEVER a false "Windows default" that would also strip the
  // restore action (review P2-2). Only a confirmed 'hidden' offers the action.
  const arrowOverlay = useIcons((s) => s.state?.arrowOverlay)
  const overlayRestoring = useIcons((s) => s.overlayRestoring)
  const arrowRow = arrowRowView(arrowOverlay)
  const [arrowRestoreOpen, setArrowRestoreOpen] = React.useState(false)
  // Help/FAQ safety net (panel record): 「小箭头不见了？」 scrolls the arrow row
  // into view and pulses it — the deep-link target for a user who thinks Windows
  // broke and cannot attribute it.
  const arrowRowRef = React.useRef<HTMLDivElement>(null)
  const [arrowHighlight, setArrowHighlight] = React.useState(false)
  const revealArrowRow = () => {
    arrowRowRef.current?.scrollIntoView({ block: 'center', behavior: 'smooth' })
    setArrowHighlight(true)
    window.setTimeout(() => setArrowHighlight(false), 1600)
  }

  // 恢复系统原始外观 (spec 07 §13 level 4 — Settings › Advanced): the destructive vertical exit,
  // behind the §13.2 confirmation. Also the tray 「恢复系统原始外观…」 deep-link target.
  const iconsWorking = useIcons((s) => s.state?.working ?? false)
  const styledCount = useIcons((s) => s.state?.styleableCount ?? 0)
  const savedLooks = usePresetLibrary((s) => s.entries.length)
  const [resetOpen, setResetOpen] = React.useState(false)
  const resetRowRef = React.useRef<HTMLDivElement>(null)
  const [resetHighlight, setResetHighlight] = React.useState(false)
  const deepLink = useApp((s) => s.deepLink)
  React.useEffect(() => {
    // The §13.2 dialog quotes "你保存的 M 个外观方案" — make M live, not a stale 0.
    void usePresetLibrary.getState().refresh()
  }, [])
  React.useEffect(() => {
    if (deepLink !== 'reset') return
    useApp.getState().consumeDeepLink()
    // Routing only (spec §13: the tray item ROUTES here) — reveal + pulse, never auto-open the
    // confirmation, so a stray tray click cannot walk toward a destructive action by itself.
    resetRowRef.current?.scrollIntoView({ block: 'center', behavior: 'smooth' })
    setResetHighlight(true)
    window.setTimeout(() => setResetHighlight(false), 1600)
  }, [deepLink])

  if (!settings || !info) return null

  // Every external open reports failure honestly — the opener plugin rejects out-of-scope
  // urls/paths inside Rust, and a swallowed rejection reads as a dead button (owner report
  // 2026-07-16: "设置里的链接点了没反应").
  const toast = useToasts.getState().show
  const openExternal = (url: string) =>
    void call('shell.openExternal', { url }).catch(() => toast(t('Toast_OpenLinkFailed'), 'warn'))
  const openDataFolder = () =>
    void call('shell.openDataFolder').catch(() => toast(t('Toast_OpenLinkFailed'), 'warn'))
  const copyDiagnostics = async () => copyText(await buildReport())

  return (
    <FullPage title={t('Panel_SettingsTitle')}>
        <div className={compact ? 'flex flex-col gap-4' : 'flex gap-6'}>
          {/* Identity — one card: brand, trust facts as text, links as text links */}
          <aside className={cn('@container', !compact && 'w-[300px] shrink-0')}>
            <InspectorCard>
              <div className="@[560px]:grid @[560px]:grid-cols-[1.15fr_1fr] @[560px]:divide-x @[560px]:divide-hair">
                {/* Product region: brand · trust · product links (stacked) */}
                <div className="divide-y divide-hair">
                  <div className="px-5 py-4.5">
                    <div className="flex items-center gap-3">
                      <img src={appIcon} alt="" className="size-11 shrink-0" />
                      <div className="min-w-0">
                        <h2 className="text-cardtitle font-medium leading-none text-t1">
                          {t('AppTitle')}
                          <span className="ml-1.5 text-[11px] font-normal tabular-nums text-t3/60">
                            {format(t('About_VersionFormat'), info.version)}
                          </span>
                        </h2>
                        <p className="mt-1.5 text-[12px] text-t2">{t('About_Slogan')}</p>
                      </div>
                    </div>
                    <p className="mt-3 text-[11.5px] leading-relaxed text-t3/60">
                      {TRUST_CHIPS.map((k) => t(k)).join(' · ')}
                    </p>
                  </div>
                  <div className="flex flex-col items-start gap-2 px-5 py-4">
                    <TextLink icon={<BrandGlyph brand="github" />} onClick={() => openExternal(info.links.repo)}>
                      {t('About_RepoUrl')}
                    </TextLink>
                    <TextLink icon={<Download size={12} />} onClick={() => openExternal(info.links.releases)}>
                      {t('About_CheckUpdate')}
                    </TextLink>
                    <TextLink icon={<MessageSquare size={12} />} onClick={() => openExternal(`${info.links.issues.replace(/\/+$/, '')}/new`)}>
                      {t('Settings_Feedback')}
                    </TextLink>
                    <TextLink icon={<ScrollText size={12} />} onClick={() => setLogOpen(true)}>
                      {t('About_Changelog')}
                    </TextLink>
                    <TextLink icon={<CircleHelp size={12} />} onClick={revealArrowRow}>
                      {t('Settings_ArrowFaq')}
                    </TextLink>
                  </div>
                </div>
                {/* Community region: the owner's channels as quiet list rows */}
                {/* Icon-INK keyline (owner call): the glyph inside the 24px chip is
                    inset 6px, so container 6px + row 8px + 6px = the glyph itself
                    lands on the same 20px line as the product links' icons above. */}
                <div className="flex flex-col justify-center gap-1 border-t border-hair px-1.5 py-3 @[560px]:border-t-0">
                  <SocialRow icon={<BrandGlyph brand="github" />} name={t('About_Link_GitHub')} handle="@2214962083" onClick={() => openExternal(info.links.githubProfile)} />
                  <SocialRow icon={<BrandGlyph brand="x" />} name={t('About_Link_X')} handle="@jinmingyang666" onClick={() => openExternal(info.links.x)} />
                  <SocialRow icon={<Tv size={12} />} name={t('About_Link_Bilibili')} handle="葬爱非主流小明" onClick={() => openExternal(info.links.bilibili)} />
                  <SocialRow icon={<BrandGlyph brand="tiktok" />} name={t('About_Link_Douyin')} handle="葬爱非主流小明" onClick={() => openExternal(info.links.douyin)} />
                </div>
              </div>
            </InspectorCard>
          </aside>

          {/* Working settings — ONE grouped card, hairline rows */}
          <main className="min-w-0 flex-1 pb-8">
            <InspectorCard>
              <Row label={t('Settings_Language')}>
                <Segmented
                  size="sm"
                  className="max-w-[300px]"
                  value={settings.language}
                  onChange={(language) => void updateSettings({ language })}
                  options={[
                    { value: 'System', label: t('Language_System') },
                    { value: 'zh-Hans', label: t('Language_ZhHans') },
                    { value: 'en', label: t('Language_English') },
                  ]}
                />
              </Row>
              <Row label={t('Settings_Theme')}>
                <Segmented
                  size="sm"
                  className="max-w-[300px]"
                  value={settings.theme}
                  onChange={(theme) => void updateSettings({ theme })}
                  options={[
                    { value: 'System', label: t('Theme_System') },
                    { value: 'Dark', label: t('Theme_Dark') },
                    { value: 'Light', label: t('Theme_Light') },
                  ]}
                />
              </Row>
              {/* 快捷方式箭头 (panel record 2026-07-11): the canonical home of the
                  keep-beautification arrow restore. Status text is the authority;
                  the action shows ONLY while the overlay is active — there is
                  nothing to restore when the arrow is already native. */}
              <Row
                innerRef={arrowRowRef}
                highlight={arrowHighlight}
                label={t('Settings_ArrowRestore')}
                desc={
                  <>
                    {t(arrowRow.statusKey)}
                    {arrowRow.showRestore && (
                      <span className="mt-0.5 block text-t3/70">{t('Settings_ArrowConstraint')}</span>
                    )}
                  </>
                }
              >
                {arrowRow.showRestore && (
                  <ActionButton
                    icon={<RotateCcw size={12} />}
                    disabled={overlayRestoring}
                    onClick={() => setArrowRestoreOpen(true)}
                  >
                    {t('Settings_ArrowRestoreAction')}
                  </ActionButton>
                )}
              </Row>
              {/* Auto-beautify new icons is HIDDEN until it actually works (owner call
                  2026-07-10): the toggle promised "new icons styled when the app opens" but
                  NOTHING consumes `keepNewIconsStyled` — no watcher, no catch-up pass. Showing
                  a switch that does nothing is a broken promise. Flip SHOW_KEEP_UP back on when
                  the host watcher + catch-up bake exist (default also flips false meanwhile). */}
              {SHOW_KEEP_UP && (
                <Row label={t('Settings_KeepUp')} desc={t('Settings_KeepUpDesc')}>
                  <ToggleSwitch
                    checked={settings.keepNewIconsStyled}
                    onChange={(keepNewIconsStyled) =>
                      // The §2 precondition (② saved-style must exist) rejects enabling before the
                      // first Apply — surface WHY instead of a silently stuck switch.
                      void updateSettings({ keepNewIconsStyled }).catch(() =>
                        toast(t('Toast_AutoFormatNeedsApply'), 'warn'),
                      )
                    }
                    label={t('Settings_KeepUp')}
                  />
                </Row>
              )}
              <Row label={t('Settings_LocalData')} desc={t('Settings_LocalDataDesc')}>
                <div className="flex shrink-0 flex-wrap justify-end gap-1.5">
                  <ActionButton icon={<ImageDown size={12} />} onClick={() => void useIcons.getState().exportCompare()}>
                    {t('Settings_ExportCompare')}
                  </ActionButton>
                  <ActionButton icon={<FolderOpen size={12} />} onClick={openDataFolder}>
                    {t('Settings_OpenDataFolder')}
                  </ActionButton>
                </div>
              </Row>
              {/* Diagnostics: three exits, one payload — every button copies the
                  FULL report first (issue URLs and mailto bodies both have hard
                  length limits, so the log itself always rides the clipboard). */}
              <Row label={t('Settings_Diag')} desc={t('Settings_DiagDesc')}>
                <div className="flex shrink-0 flex-wrap justify-end gap-1.5">
                  <ActionButton
                    icon={<ClipboardCopy size={12} />}
                    onClick={() => void copyDiagnostics().then((ok) => toast(ok ? t('Diag_Copied') : t('Diag_CopyFailed'), ok ? 'success' : 'warn'))}
                  >
                    {t('Diag_CopyLog')}
                  </ActionButton>
                  <ActionButton
                    icon={<Bug size={12} />}
                    onClick={() =>
                      void buildReport().then(async (report) => {
                        await copyText(report)
                        toast(t('Diag_Copied'), 'success')
                        openExternal(issueUrl(report))
                      })
                    }
                  >
                    {t('Diag_Report')}
                  </ActionButton>
                  <ActionButton
                    icon={<Mail size={12} />}
                    onClick={() =>
                      void copyDiagnostics().then(() => {
                        toast(t('Diag_Copied'), 'success')
                        openExternal(mailtoUrl())
                      })
                    }
                  >
                    {t('Diag_Email')}
                  </ActionButton>
                </div>
              </Row>
              {/* 恢复系统原始外观 (spec 07 §13 level 4): the destructive vertical exit lives HERE
                  (Settings › Advanced), never in the tray — the tray item only routes to this row.
                  Deliberately the LAST row: a level-4 action should not sit above everyday knobs. */}
              <Row
                innerRef={resetRowRef}
                highlight={resetHighlight}
                label={t('Settings_ResetAll')}
                desc={t('Settings_ResetAllDesc')}
              >
                <ActionButton icon={<Undo2 size={12} />} disabled={iconsWorking} onClick={() => setResetOpen(true)}>
                  {t('Settings_ResetAll')}
                </ActionButton>
              </Row>
            </InspectorCard>
          </main>
        </div>

      <ChangelogDialog open={logOpen} onOpenChange={setLogOpen} />

      {/* Restore ceremony (spec 06 §3.7 real-desktop crossing; not destructive-red):
          keeps shapes/colours, only brings the native arrow back — the honest body
          names that beautified icons get the system arrow too. */}
      <ConfirmSheet
        open={arrowRestoreOpen}
        title={t('ArrowRestore_Title')}
        body={t('ArrowRestore_Body')}
        confirmLabel={t('ArrowRestore_Confirm')}
        cancelLabel={t('ArrowRestore_Cancel')}
        onConfirm={() => {
          setArrowRestoreOpen(false)
          void useIcons.getState().restoreOverlay()
        }}
        onCancel={() => setArrowRestoreOpen(false)}
      />

      {/* The §13.2 binding reset copy: the headline sentence + the two bullets that pre-empt the
          two likely misreadings (saved appearances survive; automation turns off, not resumes). */}
      <ConfirmSheet
        open={resetOpen}
        destructive
        title={t('ResetAll_Title')}
        body={
          <>
            {format(t('ResetAll_Body1'), styledCount)}
            <span className="mt-1.5 block">{format(t('ResetAll_Body2'), savedLooks)}</span>
            <span className="block">{t('ResetAll_Body3')}</span>
          </>
        }
        confirmLabel={t('ResetAll_Confirm')}
        cancelLabel={t('ResetAll_Cancel')}
        onConfirm={() => {
          setResetOpen(false)
          void useIcons.getState().restore()
        }}
        onCancel={() => setResetOpen(false)}
      />
    </FullPage>
  )
}

/** One settings row: label(+desc) left, the control right — macOS inset-list
 *  grammar. `innerRef` + `highlight` let the Help/FAQ deep-link pulse this row. */
function Row({
  label,
  desc,
  children,
  innerRef,
  highlight,
}: {
  label: string
  desc?: ReactNode
  children: ReactNode
  innerRef?: React.Ref<HTMLDivElement>
  highlight?: boolean
}) {
  return (
    <div
      ref={innerRef}
      className={cn(
        'flex min-h-[54px] items-center justify-between gap-6 px-5 py-3 transition-colors duration-500',
        // ring-INSET: the card wrapper is overflow-hidden, so an outset ring loses its left/right
        // edges at the card boundary (same clipping class as the 2026-07-09 swatch-corner fix,
        // inspector.tsx swatchButtonClass) — owner report 2026-07-16 "高亮边框两边被截断".
        highlight && 'rounded-[10px] bg-coral/5 ring-2 ring-inset ring-coral/50',
      )}
    >
      <div className="min-w-0">
        <p className="text-body text-t1">{label}</p>
        {desc && <p className="mt-1 text-[12px] text-t3">{desc}</p>}
      </div>
      {children}
    </div>
  )
}

/** Inline brand glyphs (lucide dropped brand icons): standard marks, currentColor. */
function BrandGlyph({ brand }: { brand: 'github' | 'x' | 'tiktok' }) {
  const paths: Record<string, string> = {
    github:
      'M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12',
    x: 'M18.901 1.153h3.68l-8.04 9.19L24 22.846h-7.406l-5.8-7.584-6.638 7.584H.474l8.6-9.83L0 1.154h7.594l5.243 6.932ZM17.61 20.644h2.039L6.486 3.24H4.298Z',
    tiktok:
      'M12.525.02c1.31-.02 2.61-.01 3.91-.02.08 1.53.63 3.09 1.75 4.17 1.12 1.11 2.7 1.62 4.24 1.79v4.03c-1.44-.05-2.89-.35-4.2-.97-.57-.26-1.1-.59-1.62-.93-.01 2.92.01 5.84-.02 8.75-.08 1.4-.54 2.79-1.35 3.94-1.31 1.92-3.58 3.17-5.91 3.21-1.43.08-2.86-.31-4.08-1.03-2.02-1.19-3.44-3.37-3.65-5.71-.02-.5-.03-1-.01-1.49.18-1.9 1.12-3.72 2.58-4.96 1.66-1.44 3.98-2.13 6.15-1.72.02 1.48-.04 2.96-.04 4.44-.99-.32-2.15-.23-3.02.37-.63.41-1.11 1.04-1.36 1.75-.21.51-.15 1.07-.14 1.61.24 1.64 1.82 3.02 3.5 2.87 1.12-.01 2.19-.66 2.77-1.61.19-.33.4-.67.41-1.06.1-1.79.06-3.57.07-5.36.01-4.03-.01-8.05.02-12.07z',
  }
  return (
    <svg width={12} height={12} viewBox="0 0 24 24" aria-hidden="true">
      <path d={paths[brand]} fill="currentColor" />
    </svg>
  )
}

/** Community row — the About area doubles as the owner's traffic funnel. */
function SocialRow({
  icon,
  name,
  handle,
  onClick,
}: {
  icon: ReactNode
  name: string
  handle: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full items-center gap-2.5 rounded-[8px] px-2 py-2 text-left transition-colors hover:bg-raised-hov active:scale-[0.99]"
    >
      <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-chip text-t1">{icon}</span>
      <span className="shrink-0 text-[12px] font-medium leading-none text-t1">{name}</span>
      <span className="ml-auto min-w-0 truncate text-[11px] leading-none text-t3">{handle}</span>
    </button>
  )
}

function TextLink({ icon, onClick, children }: { icon?: ReactNode; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex max-w-full items-center gap-1.5 text-[12px] text-coral-ink underline-offset-2 hover:underline"
    >
      {icon}
      <span className="truncate">{children}</span>
    </button>
  )
}

function ActionButton({
  icon,
  onClick,
  children,
  disabled,
}: {
  icon: ReactNode
  onClick: () => void
  children: ReactNode
  disabled?: boolean
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'inline-flex items-center gap-1.5 whitespace-nowrap rounded-[8px] bg-chip px-2.5 py-1 text-[11px] text-t2 transition-colors duration-150 hover:bg-raised-hov hover:text-t1',
        disabled && 'cursor-not-allowed opacity-50 hover:bg-chip hover:text-t2',
      )}
    >
      {icon}
      {children}
    </button>
  )
}
