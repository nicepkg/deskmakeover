#!/usr/bin/env bun
// P5 perf before/after — TS `renderTile` (frozen oracle) vs the WASM
// `render_tile` ABI, same machine + corpus, warm, at preview (96) and bake (256)
// sizes. Both paths are warmed (analysis/profile caches hot) before timing, then
// each renders the 124-icon set 5× and reports the median.
//
//   bun tests/icon-parity/m6/bench.ts
//
// Honest framing (perf doc §1): the WASM flip is SINGLE-TRUTH, not a raw
// per-icon speedup — the scalar-f64 + libm core is SLOWER than the frozen TS per
// icon. The interactive wins are the profile cache (skips cold analysis on
// re-render), latest-generation coalescing (no dead drag renders), and N-worker
// sharding (÷N wall) — none of which a straight-line warm throughput bench shows.
//
// MEASURED (Apple M2, 20 icons, warm, median):
//   • In-browser V8 (Chrome 150 — the real preview engine): AUTHORITATIVE
//       96px:  TS 1.27 ms/icon  WASM 3.85 ms/icon  → 3.0×
//       256px: TS 6.94 ms/icon  WASM 19.2 ms/icon  → 2.8×
//   • bun/JSC (this script): reports ~40× at 96 / ~24× at 256 — a JSC wasm
//     tier-up artifact (short runs stay in the baseline tier), NOT the real cost.
//     Read the perf number in-browser; this bun harness is directional only.
// So a 124-icon warm settings change at 96px ≈ 477 ms single-thread WASM, but
// ÷6 workers ≈ 80 ms wall + coalescing kills drag-storm dead work → acceptable,
// and the flip buys the single pixel-truth source (perf doc §0).

import { readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

import type { ConfigDto } from '../../../src/bridge/types'
import { renderTile } from '../../../src/icon-compositor/compose'
import { setNativeArrowRaster } from '../../../src/icon-compositor/marks'
import { WasmIconRenderer } from '../../../src/icon-wasm/wasm-loader'

const REPO_ROOT = resolve(import.meta.dir, '../../..')
const PIXELS = join(REPO_ROOT, 'target/m5/pixels')
const WASM = join(REPO_ROOT, 'target/wasm32-unknown-unknown/release/dm_icon_wasm.wasm')
const N = 124 // one settings-change worth of icons (perf doc baseline set)

interface Cell {
  sourceId: string
  config: ConfigDto
  isShortcut: boolean
  opts?: { fieldSeed?: string | null }
}

function median(xs: number[]): number {
  const s = [...xs].sort((a, b) => a - b)
  return s[Math.floor(s.length / 2)]
}

async function main(): Promise<void> {
  const lines = readFileSync(join(PIXELS, 'cells.jsonl'), 'utf8').split('\n').filter(Boolean)
  const bySource = new Map<string, Cell>()
  for (const l of lines) {
    const r = JSON.parse(l) as Cell
    if (!bySource.has(r.sourceId)) bySource.set(r.sourceId, r)
    if (bySource.size >= N) break
  }
  const cells = [...bySource.values()]
  const rgba = new Map<string, Uint8ClampedArray>()
  for (const c of cells) rgba.set(c.sourceId, new Uint8ClampedArray(readFileSync(join(PIXELS, `sources/${c.sourceId}.rgba`))))

  const arrowMeta = JSON.parse(readFileSync(join(PIXELS, 'arrow.json'), 'utf8')) as { width: number; height: number }
  const arrowRgba = new Uint8ClampedArray(readFileSync(join(PIXELS, 'arrow.rgba')))

  // WASM path: install arrow + register-once every source (profile cache warms Rust-side).
  const wasm = await WasmIconRenderer.fromBytes(readFileSync(WASM))
  wasm.setArrow(arrowRgba, arrowMeta.width, arrowMeta.height)
  for (const c of cells) wasm.registerSource(c.sourceId, rgba.get(c.sourceId)!)

  // TS path: same genuine arrow; reuse ONE raster object per source so its
  // analysis cache stays warm across calls (fair warm-vs-warm).
  setNativeArrowRaster({ width: arrowMeta.width, height: arrowMeta.height, data: arrowRgba })
  const tsRaster = new Map(cells.map((c) => [c.sourceId, { width: 256, height: 256, data: rgba.get(c.sourceId)! }]))

  const benchTs = (size: number): number => {
    const t = performance.now()
    for (const c of cells) renderTile(tsRaster.get(c.sourceId)!, c.config, c.isShortcut, false, size, { fieldSeed: c.opts?.fieldSeed ?? null })
    return performance.now() - t
  }
  const benchWasm = (size: number): number => {
    const t = performance.now()
    for (const c of cells) wasm.render(c.sourceId, c.config, c.isShortcut, false, size, { fieldSeed: c.opts?.fieldSeed ?? null })
    return performance.now() - t
  }

  console.log(`P5 render throughput — ${N} icons, warm, median of 5 (${process.platform})`)
  console.log(`${'size'.padEnd(6)}${'TS total'.padStart(12)}${'TS/icon'.padStart(11)}${'WASM total'.padStart(13)}${'WASM/icon'.padStart(12)}${'ratio'.padStart(9)}`)
  for (const size of [96, 256]) {
    benchTs(size) // warm caches
    benchWasm(size)
    const ts = median(Array.from({ length: 5 }, () => benchTs(size)))
    const wa = median(Array.from({ length: 5 }, () => benchWasm(size)))
    const row = `${String(size).padEnd(6)}${`${ts.toFixed(1)}ms`.padStart(12)}${`${(ts / N).toFixed(2)}`.padStart(11)}${`${wa.toFixed(1)}ms`.padStart(13)}${`${(wa / N).toFixed(2)}`.padStart(12)}${`${(wa / ts).toFixed(2)}×`.padStart(9)}`
    console.log(row)
  }
}

main()
