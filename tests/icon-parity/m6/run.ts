#!/usr/bin/env bun
// M6 byte-parity CERTIFICATE — the flip-grade gate that pins the icon kernel's
// output to the certified anchor across every target and kernel variant.
//
//   bun tests/icon-parity/m6/run.ts            # four-way: {scalar,fast} × {native,wasm} vs TS
//   KERNEL=scalar bun tests/icon-parity/m6/run.ts   # scalar lane only
//   KERNEL=fast   bun tests/icon-parity/m6/run.ts   # fast lane only
//
// What Phase 0 hardened (M6 kernel-speed plan; Codex audit of the prior harness):
//   1. Anchor safety net (anchor.ts): recompute the setHash from public/real-icons,
//      pin 1487 cells / 124 sources / tier split, per-lane + per-field histograms,
//      every file length, the goldens/sources content digests, and the exact
//      389,808,128-byte compared total. Any drift → red BEFORE a byte is diffed.
//   2. Four-way differential: scalar-native, fast-native, scalar-wasm, fast-wasm —
//      each vs the frozen TS golden at 256px. `fast` is the byte-safe optimized
//      core (empty until Phase 1); it must equal scalar over the whole corpus.
//   3. Off-golden size sweep (sizes.ts): fast vs scalar at small/odd/preview/large
//      sizes — the only guard against a cache-key bug that passes at 256 but
//      corrupts sizes with no golden.
//   4. --locked builds on a pinned toolchain (rust-toolchain.toml).
//
// The old gate accepted any non-empty corpus and passed on any all-equal subset —
// a 1-cell corpus read as a full green. It no longer can.

import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import { assertCorpusAnchor, CELL_BYTES, CELL_COUNT, LANE_HISTOGRAM, TOTAL_BYTES } from './anchor'
import { buildWasm, type Kernel, loadArrow, loadCells, PIXELS, REPO_ROOT, WasmDriver } from './harness'
import { sizeSweep } from './sizes'

const CORPUS_SIZE = 256

function must(cond: boolean, msg: string): void {
  if (!cond) throw new Error(`CERT FAIL — ${msg}`)
}

function ensureCorpus(): void {
  if (existsSync(join(PIXELS, 'cells.jsonl'))) return
  console.log('== M5 corpus missing → dumping (bun tests/icon-parity/m5/cells.ts)')
  const r = Bun.spawnSync(['bun', 'tests/icon-parity/m5/cells.ts'], { cwd: REPO_ROOT, stdout: 'inherit', stderr: 'inherit' })
  if (r.exitCode !== 0) process.exit(r.exitCode ?? 1)
}

interface LaneStat {
  cells: number
  equal: number
  diffBytes: number
}

// ── wasm lane: 1487 cells @256 through the render_tile ABI vs the TS golden ──
function wasmLaneVsGolden(kernel: Kernel): string {
  const wasmPath = buildWasm(kernel)
  const driver = WasmDriver.create(wasmPath, loadArrow(), CORPUS_SIZE)
  const cells = loadCells()
  const perLane = new Map<string, LaneStat>()
  let cellsSeen = 0
  let equal = 0
  let diffBytes = 0
  let totalBytes = 0
  const failures: string[] = []

  for (const cell of cells) {
    const out = driver.render(cell, CORPUS_SIZE)
    const expected = new Uint8Array(readFileSync(join(PIXELS, `expected/${cell.file}.rgba`)))
    must(out.length === CELL_BYTES && expected.length === CELL_BYTES, `${cell.file}: length ${out.length}/${expected.length} != ${CELL_BYTES}`)

    const s = perLane.get(cell.lane) ?? { cells: 0, equal: 0, diffBytes: 0 }
    s.cells++
    cellsSeen++
    totalBytes += CELL_BYTES

    let diff = 0
    let firstAt = -1
    for (let i = 0; i < CELL_BYTES; i++) {
      if (out[i] !== expected[i]) {
        diff++
        if (firstAt < 0) firstAt = i
      }
    }
    s.diffBytes += diff
    diffBytes += diff
    if (diff === 0) {
      s.equal++
      equal++
    } else if (failures.length < 40) {
      const px = (firstAt / 4) | 0
      failures.push(`${cell.file} [${cell.lane}]: ${diff} diff bytes, first at (${px % CORPUS_SIZE},${(px / CORPUS_SIZE) | 0}) ch ${firstAt % 4} wasm=${out[firstAt]} ts=${expected[firstAt]}`)
    }
    perLane.set(cell.lane, s)
  }

  console.log(`\n== ${kernel}-wasm ↔ TS golden (render_tile ABI, 256px) ==`)
  console.log(`${'lane'.padEnd(18)}${'cells'.padStart(6)}${'equal'.padStart(8)}${'diff-bytes'.padStart(14)}`)
  for (const [lane, s] of [...perLane].sort(([a], [b]) => (a < b ? -1 : 1))) {
    console.log(`${lane.padEnd(18)}${String(s.cells).padStart(6)}${String(s.equal).padStart(8)}${String(s.diffBytes).padStart(14)}`)
  }
  console.log(`${'TOTAL'.padEnd(18)}${String(cellsSeen).padStart(6)}${String(equal).padStart(8)}${String(diffBytes).padStart(14)}`)

  if (failures.length) for (const f of failures) console.log(`  ${f}`)
  must(cellsSeen === CELL_COUNT, `${kernel}-wasm rendered ${cellsSeen} cells, expected ${CELL_COUNT}`)
  must(diffBytes === 0, `${kernel}-wasm has ${diffBytes} diff bytes (expected 0)`)
  must(equal === CELL_COUNT, `${kernel}-wasm matched ${equal}/${CELL_COUNT} cells`)
  must(totalBytes === TOTAL_BYTES, `${kernel}-wasm compared ${totalBytes} bytes, expected ${TOTAL_BYTES}`)
  for (const [lane, expectedCount] of Object.entries(LANE_HISTOGRAM)) {
    must(perLane.get(lane)?.equal === expectedCount, `${kernel}-wasm lane ${lane}: ${perLane.get(lane)?.equal}/${expectedCount} matched`)
  }
  console.log(`RESULT: PASS — ${kernel}-wasm byte-identical to TS golden, 0/${TOTAL_BYTES}`)
  return wasmPath
}

// ── native lane: xtask m5-pixels renders the corpus natively vs the TS golden ──
function nativeLaneVsGolden(kernel: Kernel): void {
  const feat = kernel === 'fast' ? ['--features', 'fast'] : []
  console.log(`\n== ${kernel}-native ↔ TS golden (xtask m5-pixels) ==`)
  const r = Bun.spawnSync(
    ['cargo', 'run', '--locked', '--release', '-q', '-p', 'xtask', ...feat, '--', 'm5-pixels', join(REPO_ROOT, 'target/m5')],
    { cwd: REPO_ROOT, stdout: 'pipe', stderr: 'inherit' },
  )
  const out = new TextDecoder().decode(r.stdout)
  process.stdout.write(out)
  must(r.exitCode === 0, `${kernel}-native xtask m5-pixels exit ${r.exitCode}`)

  // TOTAL line: "TOTAL   1487   1487   0/389808128" — pin the count, equal, and total.
  const line = out.split('\n').find((l) => l.trimStart().startsWith('TOTAL'))
  must(!!line, `${kernel}-native: no TOTAL line in xtask output`)
  const cols = line!.trim().split(/\s+/) // [TOTAL, cells, equal, diff/total]
  const cellsSeen = Number(cols[1])
  const equal = Number(cols[2])
  const [diff, total] = cols[3].split('/').map(Number)
  must(cellsSeen === CELL_COUNT, `${kernel}-native rendered ${cellsSeen} cells, expected ${CELL_COUNT}`)
  must(equal === CELL_COUNT, `${kernel}-native matched ${equal}/${CELL_COUNT} cells`)
  must(diff === 0, `${kernel}-native has ${diff} diff bytes (expected 0)`)
  must(total === TOTAL_BYTES, `${kernel}-native compared ${total} bytes, expected ${TOTAL_BYTES}`)
  must(out.includes('lane/fieldLane mismatches: 0'), `${kernel}-native reported a lane/fieldLane mismatch`)
  console.log(`RESULT: PASS — ${kernel}-native byte-identical to TS golden, 0/${TOTAL_BYTES}`)
}

// ── determinism scaffold: cold/hot/random-order/Fold-COW/1-8-thread byte parity ──
function determinismScaffold(kernel: Kernel): void {
  const feat = kernel === 'fast' ? ['--features', 'fast'] : []
  console.log(`\n== ${kernel} determinism scaffold (cargo test parity_determinism) ==`)
  const r = Bun.spawnSync(
    ['cargo', 'test', '--locked', '-q', '-p', 'dm-icon-core', ...feat, '--test', 'parity_determinism'],
    { cwd: REPO_ROOT, stdout: 'pipe', stderr: 'inherit' },
  )
  const out = new TextDecoder().decode(r.stdout)
  process.stdout.write(out)
  must(r.exitCode === 0, `${kernel} parity_determinism exit ${r.exitCode}`)
  must(/test result: ok\./.test(out), `${kernel} parity_determinism did not report "test result: ok"`)
  console.log(`RESULT: PASS — ${kernel} render pipeline byte-deterministic (cold/hot/order/Fold-COW/threads)`)
}

function main(): void {
  const arg = (process.env.KERNEL ?? 'all').toLowerCase()
  const kernels: Kernel[] = arg === 'all' ? ['scalar', 'fast'] : arg === 'scalar' ? ['scalar'] : arg === 'fast' ? ['fast'] : []
  if (!kernels.length) {
    console.error(`bad KERNEL=${arg} (use scalar | fast | all)`)
    process.exit(2)
  }

  ensureCorpus()
  console.log('== anchor: setHash + counts + histograms + file lengths + content digests + 389,808,128-byte total')
  assertCorpusAnchor(PIXELS)
  console.log('RESULT: PASS — corpus matches the certified anchor')

  const built: Partial<Record<Kernel, string>> = {}
  for (const k of kernels) {
    nativeLaneVsGolden(k)
    built[k] = wasmLaneVsGolden(k)
    determinismScaffold(k)
  }

  // The cross-kernel size sweep needs BOTH kernels; a single-kernel shard cannot run
  // it, so it must NOT claim ALL GATES PASS — that would report full coverage while
  // the off-golden differential never ran (Codex Phase-0 audit P2).
  const full = Boolean(built.scalar && built.fast)
  if (full) {
    sizeSweep(built.scalar!, built.fast!)
    console.log(`\nM6 CERTIFICATE: ALL GATES PASS — anchor ✓, scalar+fast × {native,wasm} == TS golden 0/${TOTAL_BYTES}, size sweep ✓, determinism ✓`)
  } else {
    console.log(`\nM6 CERTIFICATE: PARTIAL (KERNEL=${kernels[0]}) — anchor ✓, ${kernels[0]} × {native,wasm} == TS golden 0/${TOTAL_BYTES}, determinism ✓; NO cross-kernel size sweep. Run without KERNEL for the full gate.`)
  }
}

try {
  main()
} catch (e) {
  console.error(`\n${(e as Error).message}`)
  process.exit(1)
}
