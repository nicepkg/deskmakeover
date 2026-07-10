import * as React from 'react'
import type { ReactNode } from 'react'
import { Bug, ClipboardCopy, Download, FolderOpen, ImageDown, Mail, MessageSquare, ScrollText, Tv } from 'lucide-react'
import { call } from '@/bridge/client'
const appIcon = '/app-icon.svg'
import { InspectorCard } from '@/components/common/inspector'
import { Segmented } from '@/components/common/segmented'
import { ToggleSwitch } from '@/components/common/toggle-switch'
import { useApp } from '@/stores/app'
import { useIcons } from '@/stores/icons'
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

// Hidden until the host actually implements new-icon auto-beautify (owner 2026-07-10).
// See the Row it gates below.
const SHOW_KEEP_UP = false

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

  if (!settings || !info) return null

  const openExternal = (url: string) => void call('shell.openExternal', { url })
  const toast = useToasts.getState().show
  const copyDiagnostics = async () => copyText(await buildReport())

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[1080px] px-10 py-8">
        <header className="mb-6">
          <h1 className="text-display font-medium text-t1">{t('Panel_SettingsTitle')}</h1>
        </header>

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
              {/* Auto-beautify new icons is HIDDEN until it actually works (owner call
                  2026-07-10): the toggle promised "new icons styled when the app opens" but
                  NOTHING consumes `keepNewIconsStyled` — no watcher, no catch-up pass. Showing
                  a switch that does nothing is a broken promise. Flip SHOW_KEEP_UP back on when
                  the host watcher + catch-up bake exist (default also flips false meanwhile). */}
              {SHOW_KEEP_UP && (
                <Row label={t('Settings_KeepUp')} desc={t('Settings_KeepUpDesc')}>
                  <ToggleSwitch
                    checked={settings.keepNewIconsStyled}
                    onChange={(keepNewIconsStyled) => void updateSettings({ keepNewIconsStyled })}
                    label={t('Settings_KeepUp')}
                  />
                </Row>
              )}
              <Row label={t('Settings_LocalData')} desc={t('Settings_LocalDataDesc')}>
                <div className="flex shrink-0 flex-wrap justify-end gap-1.5">
                  <ActionButton icon={<ImageDown size={12} />} onClick={() => void useIcons.getState().exportCompare()}>
                    {t('Settings_ExportCompare')}
                  </ActionButton>
                  <ActionButton icon={<FolderOpen size={12} />} onClick={() => void call('shell.openDataFolder')}>
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
            </InspectorCard>
          </main>
        </div>
      </div>

      <ChangelogDialog open={logOpen} onOpenChange={setLogOpen} />
    </div>
  )
}

/** One settings row: label(+desc) left, the control right — macOS inset-list grammar. */
function Row({ label, desc, children }: { label: string; desc?: string; children: ReactNode }) {
  return (
    <div className="flex min-h-[54px] items-center justify-between gap-6 px-5 py-3">
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

function ActionButton({ icon, onClick, children }: { icon: ReactNode; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex items-center gap-1.5 whitespace-nowrap rounded-[8px] bg-chip px-2.5 py-1 text-[11px] text-t2 transition-colors duration-150 hover:bg-raised-hov hover:text-t1"
    >
      {icon}
      {children}
    </button>
  )
}
