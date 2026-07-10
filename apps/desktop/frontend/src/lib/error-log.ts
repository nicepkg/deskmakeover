// Global error capture (owner order 2026-07-09: "整体的 catch 一定要做好").
// One ring buffer holds everything the web layer sees — uncaught exceptions,
// unhandled promise rejections, production console.error calls, and errors the
// HOST forwards over the bridge — persisted to localStorage so a crash or
// reload never loses the evidence. The diagnostics module reads this buffer to
// build the copy/report payloads.

export interface ErrorEntry {
  /** ISO timestamp of first occurrence. */
  ts: string
  source: 'web' | 'host'
  message: string
  stack?: string
  /** Consecutive duplicates collapse into one entry with a count. */
  count: number
}

const STORE_KEY = 'dm.errorlog'
const MAX_ENTRIES = 120
/** Persisted-payload guard: localStorage is not for megabytes of stack. */
const MAX_STACK_CHARS = 4000

let entries: ErrorEntry[] = load()

function load(): ErrorEntry[] {
  try {
    const raw = localStorage.getItem(STORE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as ErrorEntry[]
    return Array.isArray(parsed) ? parsed.slice(-MAX_ENTRIES) : []
  } catch {
    return []
  }
}

function persist(): void {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(entries))
  } catch {
    // Quota/private-mode failures must never take the app down from the logger.
  }
}

export function logError(source: ErrorEntry['source'], message: string, stack?: string): void {
  const msg = String(message ?? 'Unknown error').slice(0, 1000)
  const last = entries[entries.length - 1]
  if (last && last.source === source && last.message === msg) {
    last.count += 1
    persist()
    return
  }
  entries.push({
    ts: new Date().toISOString(),
    source,
    message: msg,
    stack: stack ? String(stack).slice(0, MAX_STACK_CHARS) : undefined,
    count: 1,
  })
  if (entries.length > MAX_ENTRIES) entries = entries.slice(-MAX_ENTRIES)
  persist()
}

export function getErrors(): readonly ErrorEntry[] {
  return entries
}

export function clearErrors(): void {
  entries = []
  persist()
}

/** One entry → the human/report line format shared by copy, issue and email. */
export function formatEntry(e: ErrorEntry): string {
  const dup = e.count > 1 ? ` (×${e.count})` : ''
  const head = `[${e.ts}] ${e.source} · ${e.message}${dup}`
  return e.stack ? `${head}\n${e.stack.replace(/^/gm, '    ')}` : head
}

/**
 * Install the global hooks. Called ONCE from main.tsx BEFORE the first render
 * so even boot errors are captured. console.error is tapped only in production
 * builds — dev consoles are full of framework advisories that would drown the
 * signal users actually need to report.
 */
export function installGlobalErrorCapture(): void {
  window.addEventListener('error', (e) => {
    // Resource-load errors (img/script) surface as events without an Error.
    if (e.error instanceof Error) logError('web', e.error.message, e.error.stack)
    else logError('web', e.message || `Resource failed: ${(e.target as HTMLElement)?.tagName ?? '?'}`)
  })

  window.addEventListener('unhandledrejection', (e) => {
    const r: unknown = e.reason
    if (r instanceof Error) logError('web', `Unhandled rejection: ${r.message}`, r.stack)
    else logError('web', `Unhandled rejection: ${String(r).slice(0, 500)}`)
  })

  if (!import.meta.env.DEV) {
    const original = console.error.bind(console)
    console.error = (...args: unknown[]) => {
      original(...args)
      const err = args.find((a): a is Error => a instanceof Error)
      const text = args
        .map((a) => (a instanceof Error ? a.message : typeof a === 'string' ? a : safeJson(a)))
        .join(' ')
      logError('web', text.slice(0, 600), err?.stack)
    }
  }
}

function safeJson(v: unknown): string {
  try {
    return JSON.stringify(v)?.slice(0, 200) ?? String(v)
  } catch {
    return String(v)
  }
}
