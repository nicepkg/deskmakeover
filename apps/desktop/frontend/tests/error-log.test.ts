import { beforeAll, beforeEach, describe, expect, test } from 'bun:test'

// error-log module state initializes from localStorage at import time, so the
// stub must exist BEFORE the dynamic import below.
function makeStore(): Storage {
  const map = new Map<string, string>()
  return {
    get length() {
      return map.size
    },
    clear: () => map.clear(),
    getItem: (k: string) => map.get(k) ?? null,
    key: (i: number) => [...map.keys()][i] ?? null,
    removeItem: (k: string) => {
      map.delete(k)
    },
    setItem: (k: string, v: string) => {
      map.set(k, v)
    },
  } as Storage
}

let log: typeof import('../src/lib/error-log')

beforeAll(async () => {
  ;(globalThis as { localStorage?: Storage }).localStorage = makeStore()
  log = await import('../src/lib/error-log')
})

beforeEach(() => {
  log.clearErrors()
})

describe('error log ring buffer', () => {
  test('records entries with source and message', () => {
    log.logError('web', 'boom', 'stack-line')
    const all = log.getErrors()
    expect(all.length).toBe(1)
    expect(all[0].source).toBe('web')
    expect(all[0].message).toBe('boom')
    expect(all[0].stack).toBe('stack-line')
  })

  test('consecutive duplicates collapse into one entry with a count', () => {
    log.logError('web', 'same thing')
    log.logError('web', 'same thing')
    log.logError('web', 'same thing')
    const all = log.getErrors()
    expect(all.length).toBe(1)
    expect(all[0].count).toBe(3)
    expect(log.formatEntry(all[0])).toContain('(×3)')
  })

  test('different sources do not collapse', () => {
    log.logError('web', 'same thing')
    log.logError('host', 'same thing')
    expect(log.getErrors().length).toBe(2)
  })

  test('buffer caps at 120 entries, keeping the newest', () => {
    for (let i = 0; i < 150; i++) log.logError('web', `err ${i}`)
    const all = log.getErrors()
    expect(all.length).toBe(120)
    expect(all[all.length - 1].message).toBe('err 149')
    expect(all[0].message).toBe('err 30')
  })

  test('persists across a simulated reload (same storage, fresh read)', () => {
    log.logError('web', 'survives')
    const raw = localStorage.getItem('dm.errorlog')
    expect(raw).toContain('survives')
  })

  test('oversized stacks are truncated to the persistence guard', () => {
    log.logError('web', 'big', 'x'.repeat(10_000))
    expect(log.getErrors()[0].stack!.length).toBeLessThanOrEqual(4000)
  })

  test('formatEntry indents stack lines under the head line', () => {
    log.logError('host', 'headline', 'line1\nline2')
    const text = log.formatEntry(log.getErrors()[0])
    expect(text).toContain('host · headline')
    expect(text).toContain('    line1')
    expect(text).toContain('    line2')
  })
})
