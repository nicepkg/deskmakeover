// Verifies the TS-side WASM wrapper (`WasmIconRenderer`) end to end — arrow
// install, source register, config caching, render, seed — produces bytes
// identical to the frozen TS goldens. The full 1487-cell ABI proof lives in
// run.ts; this pins the wrapper's own logic (buffer reuse, memory re-view,
// config dedup, seed decode) across every lane + all shortcut cells.
//
//   bun test tests/icon-parity/m6/loader.test.ts
//
// Depends on the M5 corpus (target/m5/pixels) + the release wasm; skips with a
// clear message if either is absent (run `bun tests/icon-parity/m6/run.ts` once).

import { existsSync, readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

import { expect, test } from 'bun:test'

import type { ConfigDto } from '../../../src/bridge/types'
import { WasmIconRenderer } from '../../../src/icon-wasm/wasm-loader'

const REPO_ROOT = resolve(import.meta.dir, '../../..')
const PIXELS = join(REPO_ROOT, 'target/m5/pixels')
const WASM = join(REPO_ROOT, 'target/wasm32-unknown-unknown/release/dm_icon_wasm.wasm')
const CORPUS_SIZE = 256
const PER_LANE_CAP = 15 // sample per lane; all shortcut cells are always included

const ready = existsSync(join(PIXELS, 'cells.jsonl')) && existsSync(WASM)

test.if(ready)('WasmIconRenderer bytes match the TS golden across every lane', async () => {
  const renderer = await WasmIconRenderer.fromBytes(readFileSync(WASM))

  const arrowMeta = JSON.parse(readFileSync(join(PIXELS, 'arrow.json'), 'utf8')) as { width: number; height: number }
  renderer.setArrow(readFileSync(join(PIXELS, 'arrow.rgba')), arrowMeta.width, arrowMeta.height)

  const lines = readFileSync(join(PIXELS, 'cells.jsonl'), 'utf8').split('\n').filter((l) => l.length > 0)
  const perLane = new Map<string, number>()
  let checked = 0
  let mismatches = 0
  const seen = new Set<string>()

  for (const line of lines) {
    const rec = JSON.parse(line) as {
      file: string
      sourceId: string
      config: ConfigDto
      isShortcut: boolean
      showOriginal: boolean
      opts?: { fieldSeed?: string | null }
      lane: string
    }

    // Sample: keep every shortcut cell + up to PER_LANE_CAP per lane.
    const laneCount = perLane.get(rec.lane) ?? 0
    if (!rec.isShortcut && laneCount >= PER_LANE_CAP) continue
    perLane.set(rec.lane, laneCount + 1)

    if (!seen.has(rec.sourceId)) {
      renderer.registerSource(rec.sourceId, readFileSync(join(PIXELS, `sources/${rec.sourceId}.rgba`)))
      seen.add(rec.sourceId)
    }

    const out = renderer.render(rec.sourceId, rec.config, rec.isShortcut, rec.showOriginal, CORPUS_SIZE, {
      fieldSeed: rec.opts?.fieldSeed ?? null,
    })
    expect(out).not.toBeNull()

    const expected = readFileSync(join(PIXELS, `expected/${rec.file}.rgba`))
    let diff = 0
    for (let i = 0; i < expected.length; i++) if (out![i] !== expected[i]) diff++
    if (diff !== 0) mismatches++
    checked++
  }

  expect(mismatches).toBe(0)
  expect(checked).toBeGreaterThan(0)
  // Every lane in the corpus must be represented in the sample.
  expect(perLane.size).toBeGreaterThanOrEqual(8)
})

test.if(!ready)('corpus/wasm absent — loader parity skipped', () => {
  expect(ready).toBe(false)
})
