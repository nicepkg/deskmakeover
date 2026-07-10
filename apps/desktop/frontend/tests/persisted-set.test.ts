import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { readPersistedSet, writePersistedSet } from '../src/lib/persisted-set'

// The open-axes persistence (spec 03 §3.1) is localStorage round-tripping under the
// hood. usePersistedSet is a thin React wrapper over these pure helpers; we exercise
// the storage contract directly (no renderer needed): write→read round-trips, absent
// keys fall through to the default, and unreadable / disabled storage never throws.

function makeStore(): Storage {
  const map = new Map<string, string>()
  return {
    get length() {
      return map.size
    },
    clear: () => map.clear(),
    getItem: (k: string) => (map.has(k) ? map.get(k)! : null),
    key: (i: number) => [...map.keys()][i] ?? null,
    removeItem: (k: string) => {
      map.delete(k)
    },
    setItem: (k: string, v: string) => {
      map.set(k, v)
    },
  } as Storage
}

const originalWindow = (globalThis as { window?: unknown }).window

beforeEach(() => {
  ;(globalThis as { window?: unknown }).window = { localStorage: makeStore() }
})
afterEach(() => {
  ;(globalThis as { window?: unknown }).window = originalWindow
})

describe('persisted-set storage helpers', () => {
  test('write then read round-trips the members (order-independent)', () => {
    writePersistedSet('dm.icons.openAxes', new Set(['shape', 'color', 'size']))
    const read = readPersistedSet('dm.icons.openAxes')
    expect(read).not.toBeNull()
    expect([...read!].sort()).toEqual(['color', 'shape', 'size'])
  })

  test('an empty set persists as empty (a collapsed panel stays collapsed)', () => {
    writePersistedSet('dm.paper.openAxes', new Set())
    const read = readPersistedSet('dm.paper.openAxes')
    expect(read).not.toBeNull()
    expect(read!.size).toBe(0)
  })

  test('an absent key reads as null (caller falls back to default-open)', () => {
    expect(readPersistedSet('dm.never.written')).toBeNull()
  })

  test('malformed JSON reads as null instead of throwing', () => {
    ;(window.localStorage as Storage).setItem('dm.icons.openAxes', '{not json')
    expect(readPersistedSet('dm.icons.openAxes')).toBeNull()
  })

  test('a non-array payload reads as null', () => {
    window.localStorage.setItem('dm.icons.openAxes', JSON.stringify({ shape: true }))
    expect(readPersistedSet('dm.icons.openAxes')).toBeNull()
  })

  test('non-string array members are dropped', () => {
    window.localStorage.setItem('dm.icons.openAxes', JSON.stringify(['shape', 3, null, 'size']))
    expect([...readPersistedSet('dm.icons.openAxes')!].sort()).toEqual(['shape', 'size'])
  })

  test('no window / storage is a safe no-op (SSR + privacy mode)', () => {
    ;(globalThis as { window?: unknown }).window = undefined
    expect(readPersistedSet('dm.icons.openAxes')).toBeNull()
    expect(() => writePersistedSet('dm.icons.openAxes', new Set(['shape']))).not.toThrow()
  })
})
