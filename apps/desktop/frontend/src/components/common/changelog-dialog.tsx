import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { useApp } from '@/stores/app'
import { format, useI18n, useT } from '@/lib/i18n'

// The in-app changelog (owner model 2026-07-08): auto-opens ONCE after an update
// (App shell compares the last-seen version), and stays reachable any time from
// the About area's 更新日志 link.

const SEEN_KEY = 'dm.changelog.seen'

/** True exactly once per installed update: a previous version was seen and it differs. */
export function shouldAutoShowChangelog(version: string): boolean {
  const seen = localStorage.getItem(SEEN_KEY)
  localStorage.setItem(SEEN_KEY, version)
  return seen !== null && seen !== version
}

export function ChangelogDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const t = useT()
  const lang = useI18n((s) => s.lang)
  const info = useApp((s) => s.info)
  if (!info) return null
  const entries = lang === 'zh-Hans' ? info.changelogZh : info.changelogEn
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[70vh] w-[380px] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="text-cardtitle font-medium text-t1">{t('Changelog_Title')}</DialogTitle>
        </DialogHeader>
        {entries.length === 0 ? (
          <p className="text-caption text-t3">{format(t('About_VersionFormat'), info.version)}</p>
        ) : (
          <div className="space-y-4">
            {entries.map((entry) => (
              <div key={entry.version}>
                <p className="text-body font-medium tabular-nums text-t1">{entry.version}</p>
                <ul className="mt-1.5 space-y-1">
                  {entry.items.map((item) => (
                    <li key={item} className="flex gap-2 text-caption leading-relaxed text-t2">
                      <span aria-hidden className="mt-[3px] size-1 shrink-0 rounded-full bg-t3/60" />
                      {item}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
