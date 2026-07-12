import { call } from '@/bridge/client'
import type { SystemInfoDto } from '@/bridge/types'
import { useApp } from '@/stores/app'
import { formatEntry, getErrors } from '@/lib/error-log'

// Diagnostics reporting (owner order 2026-07-09): three exits, one payload.
//   复制日志  — the full report to the clipboard.
//   GitHub 报告 — copies the full report FIRST, then opens a prefilled new-issue
//                URL. The URL carries only environment + the newest errors:
//                GitHub rejects query strings past ~8KB, so the full log travels
//                via the clipboard and the body says "paste it here".
//   邮件反馈  — same copy-first trick, then a mailto: (bodies there are even
//                more length-hostile). For users who cannot reach GitHub.

const FALLBACK_EMAIL = '2214962083@qq.com'
/** Stay well under GitHub's ~8KB URL rejection line. */
const MAX_ISSUE_BODY = 5500
const bootTime = Date.now()

async function systemInfo(): Promise<SystemInfoDto> {
  try {
    return await call('diagnostics.getInfo')
  } catch {
    return { osVersion: 'unknown', webview2Version: 'unknown', arch: 'unknown', hostLogTail: [] }
  }
}

function webEnvLines(): string[] {
  const app = safeApp()
  return [
    `App: DeskMakeover ${app.version}`,
    `Locale: ${app.language} · Theme: ${app.theme}`,
    `Viewport: ${window.innerWidth}×${window.innerHeight} @${window.devicePixelRatio}x`,
    `UA: ${navigator.userAgent}`,
    `Uptime: ${Math.round((Date.now() - bootTime) / 1000)}s`,
  ]
}

/** Store reads are best-effort: diagnostics must work even mid-crash. */
function safeApp(): { version: string; language: string; theme: string } {
  try {
    const s = useApp.getState()
    return {
      version: s.info?.version ?? '?',
      language: s.settings?.language ?? '?',
      theme: s.settings?.theme ?? '?',
    }
  } catch {
    return { version: '?', language: '?', theme: '?' }
  }
}

/** The FULL report: environment + host log tail + every captured entry. */
export async function buildReport(): Promise<string> {
  const sys = await systemInfo()
  const errors = [...getErrors()].reverse()
  const lines = [
    '## Environment',
    ...webEnvLines(),
    `OS: ${sys.osVersion}`,
    `WebView2: ${sys.webview2Version} (${sys.arch})`,
    '',
    `## Error log (newest first, ${errors.length} entries)`,
    ...(errors.length ? errors.map(formatEntry) : ['(empty)']),
  ]
  if (sys.hostLogTail.length) lines.push('', '## Host log tail', ...sys.hostLogTail)
  return lines.join('\n')
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    // Clipboard API can be denied outside secure contexts — legacy fallback.
    try {
      const ta = document.createElement('textarea')
      ta.value = text
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      const ok = document.execCommand('copy')
      ta.remove()
      return ok
    } catch {
      return false
    }
  }
}

function issuesBase(): string {
  try {
    return useApp.getState().info?.links.issues ?? 'https://github.com/nicepkg/deskmakeover/issues'
  } catch {
    return 'https://github.com/nicepkg/deskmakeover/issues'
  }
}

export function supportEmail(): string {
  try {
    return useApp.getState().info?.links.email ?? FALLBACK_EMAIL
  } catch {
    return FALLBACK_EMAIL
  }
}

/** Prefilled new-issue URL: environment + newest errors, capped under the URL limit. */
export function issueUrl(report: string): string {
  const newest = getErrors().slice(-1)[0]
  const title = `[bug] ${newest ? newest.message.slice(0, 80) : 'problem report'}`
  const note =
    '<!-- 完整错误日志已复制到你的剪贴板,请粘贴到下方 / The full error log is on your clipboard, paste it below. -->'
  let body = `${note}\n\n${report}`
  if (body.length > MAX_ISSUE_BODY) body = `${body.slice(0, MAX_ISSUE_BODY)}\n…(truncated · paste the copied full log)`
  const q = new URLSearchParams({ title, body, labels: 'bug' })
  return `${issuesBase().replace(/\/+$/, '')}/new?${q.toString()}`
}

/** Mailto with a short body: the full report rides the clipboard, not the URL. */
export function mailtoUrl(): string {
  const app = safeApp()
  const subject = `DeskMakeover 错误报告 · v${app.version}`
  const body = '错误日志已复制到剪贴板,请在此处粘贴:\n(The error log is on your clipboard, please paste it here.)\n\n'
  const q = new URLSearchParams({ subject, body })
  return `mailto:${supportEmail()}?${q.toString().replace(/\+/g, '%20')}`
}
