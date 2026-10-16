import { describe, expect, test } from 'bun:test'
import type { ConfigDto } from '../src/bridge/types'
import { IconCompositor } from '../src/icon-compositor/icon-renderer'

// Content-keyed sources (ADR-0004 D7 amendment 2026-10-16): the decoded pixels
// live ONCE PER CONTENT — the source URL — never once per item. The all-drives
// explorer expansion styles thousands of folders sharing ONE stock-folder
// source; keying the decode by item registered N copies of the same RGBA and
// the owner box's WebView renderer died at 2.8GB mid-apply. These tests pin
// the three halves of the fix: ONE dispatch per content (joiners share it),
// the item→content alias every lookup resolves through, and a failed decode
// clearing its claim so a retry can dispatch again.

const U = 'dmicon://localhost/sAliasFixture'
const U2 = 'dmicon://localhost/sDeadFixture'

type Priv = {
  dispatchSource: (url: string) => void
  ackSource: (url: string, ok: boolean, seed?: string | null) => void
  itemSource: Map<string, string>
  ready: Map<string, string>
  seeds: Map<string, string | null>
  loadSource: (id: string, url: string) => Promise<void>
}

describe('content-keyed source aliasing', () => {
  test('N items sharing one URL join a SINGLE decode dispatch', async () => {
    const r = new IconCompositor() as unknown as Priv
    let dispatches = 0
    r.dispatchSource = (url) => {
      dispatches++
      setTimeout(() => r.ackSource(url, true, '#aabbcc'), 5)
    }
    await Promise.all([
      r.loadSource('folder-1', U),
      r.loadSource('folder-2', U),
      r.loadSource('folder-3', U),
    ])
    expect(dispatches).toBe(1) // not one per item — this IS the memory fix
    expect(r.itemSource.get('folder-2')).toBe(U)
    expect(r.ready.get(U)).toBe(U)
    expect(r.seeds.get(U)).toBe('#aabbcc')
    // Decoded already: a later item aliases instantly, no dispatch.
    await r.loadSource('folder-4', U)
    expect(dispatches).toBe(1)
  })

  test('a failed decode rejects every joiner and clears its claim (retry dispatches)', async () => {
    const r = new IconCompositor() as unknown as Priv
    let dispatches = 0
    r.dispatchSource = (url) => {
      dispatches++
      setTimeout(() => r.ackSource(url, false), 0)
    }
    let err = ''
    try {
      await Promise.all([r.loadSource('a', U2), r.loadSource('b', U2)])
    } catch (e) {
      err = String(e)
    }
    expect(err).toContain('source failed')
    expect(r.ready.has(U2)).toBe(false)
    expect(r.seeds.has(U2)).toBe(false)
    // Claim cleared → a fresh load dispatches again (no permanent wedge).
    try {
      await r.loadSource('a', U2)
    } catch {
      // expected — the fixture decode always fails
    }
    expect(dispatches).toBe(2)
  })

  test('tile and bake lookups resolve through the item→content alias', async () => {
    const r = new IconCompositor() as unknown as {
      itemSource: Map<string, string>
      ready: Map<string, string>
      seeds: Map<string, string | null>
      hasSource: (id: string, url: string) => boolean
      seedOf: (id: string) => string | null | undefined
      getTile: (id: string, cfg: ConfigDto, sc: boolean, orig: boolean, size: number) => unknown
      bakeMasterPng: (id: string, cfg: ConfigDto, sc: boolean) => Promise<unknown>
      poolSize: number
      post: (shard: string, msg: unknown) => void
      localRenderer: unknown
      onWorkerMessage: (msg: unknown) => void
    }
    r.itemSource.set('explorer-7', U)
    r.ready.set(U, U)
    r.seeds.set(U, '#feed10')
    expect(r.hasSource('explorer-7', U)).toBe(true)
    // Capture BOTH lanes so the assertions hold whether bun runs the in-thread
    // fallback (poolSize 0) or a spawned pool: the in-thread renderer asks for
    // the content key, the worker message SHARDS by it and carries it as skey
    // while keeping the item id for its coalescing lane + reply.
    const seen: string[] = []
    const posted: Array<{ shard: string; msg: Record<string, unknown> }> = []
    r.post = (shard, msg) => {
      posted.push({ shard, msg })
      if (msg.t === 'bake') {
        setTimeout(() => r.onWorkerMessage({ t: 'baked', req: msg.req, id: msg.id, png: null }), 0)
      }
    }
    r.localRenderer = {
      hasSource: (k: string) => {
        seen.push(k)
        return true // real renderer answers for the content key; render() returns null below
      },
      render: () => null,
      seedOf: (k: string) => (k === U ? '#feed10' : null),
      registerSource: () => null,
    }
    expect(r.seedOf('explorer-7')).toBe('#feed10') // both lanes answer via CONTENT
    seen.length = 0
    const cfg = { shape: 'Circle' } as unknown as ConfigDto
    expect(r.getTile('explorer-7', cfg, false, false, 96)).toBeNull()
    if (r.poolSize === 0) {
      expect(seen).toEqual([U]) // in-thread lane looked up CONTENT
      expect(posted).toHaveLength(0)
    } else {
      expect(posted).toHaveLength(1)
      expect(posted[0].shard).toBe(U) // pixels live under the content key
      expect(posted[0].msg.skey).toBe(U)
      expect(posted[0].msg.id).toBe('explorer-7') // per-item lane + reply identity
    }
    const seenBefore = seen.length
    const postedBefore = posted.length
    expect(await r.bakeMasterPng('explorer-7', cfg, false)).toBeNull()
    if (r.poolSize === 0) {
      expect(seen.length).toBe(seenBefore + 1)
      expect(seen[seen.length - 1]).toBe(U)
    } else {
      expect(posted.length).toBe(postedBefore + 1)
      expect(posted[posted.length - 1].msg.skey).toBe(U)
      expect(posted[posted.length - 1].msg.t).toBe('bake')
    }
  })
})
