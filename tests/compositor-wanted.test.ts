import { describe, expect, test } from 'bun:test'
import { IconCompositor } from '../src/icon-compositor/icon-renderer'

// The `wanted` dedup gate must be CLEARED once a render lands (owner bug
// 2026-07-16): a satisfied want that lingered turned every post-LRU-eviction
// getTile for that slot into a permanent null — no re-dispatch, the canvas
// kept its old pixels. The original render is the worst case: its styleKey
// never varies (`orig|...`), so after ~20 style-key churns evicted it,
// hold-to-compare showed the CURRENT image forever.

function flightInto(c: IconCompositor, req: number, slot: string, styleKey: string) {
  const anyC = c as any
  anyC.wanted.set(slot, styleKey)
  anyC.inflight.set(req, { slot, styleKey, epoch: anyC.epoch })
}

describe('IconCompositor wanted-gate lifecycle', () => {
  test('a landed render clears its satisfied want (eviction can re-dispatch)', () => {
    const c = new IconCompositor()
    const anyC = c as any
    const slot = 'item1|true|48'
    const styleKey = 'orig|false|48'
    flightInto(c, 1, slot, styleKey)

    const bitmap = { close() {} }
    anyC.onWorkerMessage({ t: 'rendered', req: 1, id: 'item1', bitmap })

    expect(anyC.wanted.has(slot)).toBe(false) // the regression: this lingered
    expect(anyC.styleLru.get(`${anyC.epoch}|${styleKey}`)?.get('item1')).toBe(bitmap)
    expect(anyC.inflight.has(1)).toBe(false)
  })

  test('a superseded render is dropped and KEEPS the newer want', () => {
    const c = new IconCompositor()
    const anyC = c as any
    const slot = 'item1|false|48'
    flightInto(c, 2, slot, 'old-style-key')
    anyC.wanted.set(slot, 'new-style-key') // a newer request overwrote the want

    let closed = false
    anyC.onWorkerMessage({ t: 'rendered', req: 2, id: 'item1', bitmap: { close: () => { closed = true } } })

    expect(closed).toBe(true) // stale bitmap released, not cached
    expect(anyC.wanted.get(slot)).toBe('new-style-key') // in-flight newer want survives
    expect(anyC.styleLru.get(`${anyC.epoch}|old-style-key`)).toBeUndefined()
  })

  test('a pre-invalidation render is dropped under the new epoch', () => {
    const c = new IconCompositor()
    const anyC = c as any
    const slot = 'item1|false|48'
    flightInto(c, 3, slot, 'style-a')
    c.invalidateAll() // epoch bump clears wanted

    let closed = false
    anyC.onWorkerMessage({ t: 'rendered', req: 3, id: 'item1', bitmap: { close: () => { closed = true } } })

    expect(closed).toBe(true)
    expect(anyC.styleLru.size).toBe(0)
  })
})
