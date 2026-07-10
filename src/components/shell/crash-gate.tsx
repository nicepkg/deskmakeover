import * as React from 'react'
const appIcon = '/app-icon.svg'
import { buildReport, copyText, issueUrl, mailtoUrl, supportEmail } from '@/lib/diagnostics'
import { logError } from '@/lib/error-log'
import { call } from '@/bridge/client'

// The last line of defence: a React error boundary around the whole app. When
// the tree crashes it renders a self-contained apology card with the three
// diagnostic exits (copy / GitHub / email). DELIBERATELY zero dependencies on
// stores or i18n — any of those may be the thing that just crashed — so the
// copy is hardcoded bilingual (zh first, per the product's audience).

interface CrashState {
  error: Error | null
  copied: boolean
}

export class CrashGate extends React.Component<{ children: React.ReactNode }, CrashState> {
  state: CrashState = { error: null, copied: false }

  static getDerivedStateFromError(error: Error): Partial<CrashState> {
    return { error }
  }

  componentDidCatch(error: Error, info: React.ErrorInfo): void {
    logError('web', `React crash: ${error.message}`, `${error.stack ?? ''}\ncomponent stack:${info.componentStack ?? ''}`)
  }

  private copyLog = async (): Promise<boolean> => {
    const ok = await copyText(await buildReport())
    this.setState({ copied: ok })
    return ok
  }

  private openExternal(url: string): void {
    // In the WebView2 host the bridge owns navigation; in a browser a plain
    // window.open works. Try the bridge first, fall back hard.
    void call('shell.openExternal', { url }).catch(() => window.open(url, '_blank'))
  }

  private report = async (): Promise<void> => {
    const report = await buildReport()
    await this.copyLog()
    this.openExternal(issueUrl(report))
  }

  private email = async (): Promise<void> => {
    await this.copyLog()
    this.openExternal(mailtoUrl())
  }

  render(): React.ReactNode {
    const { error, copied } = this.state
    if (!error) return this.props.children
    return (
      <div className="flex h-screen items-center justify-center bg-background p-6 text-foreground">
        <div className="w-full max-w-[460px] rounded-2xl border border-hair bg-raised p-6 shadow-elev-2">
          <div className="flex items-center gap-3">
            <img src={appIcon} alt="" className="size-10" />
            <div>
              <h1 className="text-cardtitle font-medium text-t1">应用崩溃了,抱歉。</h1>
              <p className="mt-0.5 text-[12px] text-t3">The app crashed. Sorry about that.</p>
            </div>
          </div>

          <p className="mt-4 max-h-24 overflow-y-auto rounded-[10px] bg-chip px-3 py-2 font-mono text-[11px] leading-relaxed text-t2">
            {error.message}
          </p>

          <p className="mt-3 text-[12px] leading-relaxed text-t2">
            把错误日志发给作者就能修。用不了 GitHub 的话,复制后发邮箱 {supportEmail()} 也一样。
            <span className="mt-0.5 block text-[11px] text-t3">
              Copy the log and send it over GitHub or email; either route reaches the author.
            </span>
          </p>

          <div className="mt-4 flex flex-wrap items-center gap-2">
            <CrashButton onClick={() => void this.report()} primary>
              去 GitHub 报告 · Report
            </CrashButton>
            <CrashButton onClick={() => void this.copyLog()}>{copied ? '已复制 ✓' : '复制错误日志 · Copy log'}</CrashButton>
            <CrashButton onClick={() => void this.email()}>发邮件 · Email</CrashButton>
            <CrashButton onClick={() => location.reload()}>重新载入 · Reload</CrashButton>
          </div>
        </div>
      </div>
    )
  }
}

/** DEV-only: lets the dev menu detonate a real render crash so the CrashGate
 *  surface stays testable without hand-breaking code. */
export function CrashProbe() {
  const [boom, setBoom] = React.useState(false)
  React.useEffect(() => {
    if (!import.meta.env.DEV) return
    const fire = () => setBoom(true)
    window.addEventListener('dm-test-crash', fire)
    return () => window.removeEventListener('dm-test-crash', fire)
  }, [])
  if (boom) throw new Error('Test crash from the developer menu')
  return null
}

function CrashButton({
  children,
  onClick,
  primary,
}: {
  children: React.ReactNode
  onClick: () => void
  primary?: boolean
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={
        primary
          ? 'rounded-[9px] bg-coral px-3 py-1.5 text-[12px] font-medium text-cta-ink transition-transform active:scale-[0.98]'
          : 'rounded-[9px] bg-chip px-3 py-1.5 text-[12px] text-t2 transition-colors hover:bg-raised-hov hover:text-t1'
      }
    >
      {children}
    </button>
  )
}
