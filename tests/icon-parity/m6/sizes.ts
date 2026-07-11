#!/usr/bin/env bun
// Off-golden size sweep — fast-vs-scalar self-consistency at sizes the TS golden
// does not cover. The 256px gate (run.ts) diffs against the frozen golden, but a
// mask-cache keyed on (shape, dims, offset) could collide or miskey and pass at
// 256 while corrupting other sizes, where there is no golden to catch it. Because
// `fast` must be byte-identical to the scalar reference at EVERY size, we diff the
// two kernels directly here.
//
// Today (empty `fast` feature) the two binaries are identical, so this is a no-op
// green; it goes live the moment Phase 1 fills `fast` with the mask cache.
//
//   bun tests/icon-parity/m6/sizes.ts    # builds both kernels, sweeps

import { buildWasm, type CorpusCell, loadArrow, loadCells, WasmDriver } from './harness'

// small · odd (non-power-of-two, exercises fractional offsets) · preview · large.
export const SWEEP_SIZES = [32, 47, 96, 129, 512]

/** Diff fast vs scalar over one representative cell per source at each sweep size.
 *  Pass prebuilt artifact paths to reuse run.ts's builds; omit to build here. */
export function sizeSweep(scalarPath = buildWasm('scalar'), fastPath = buildWasm('fast')): void {
  const arrow = loadArrow()
  const bySource = new Map<string, CorpusCell>()
  for (const c of loadCells()) if (!bySource.has(c.sourceId)) bySource.set(c.sourceId, c)
  const probe = [...bySource.values()]
  const maxSize = Math.max(...SWEEP_SIZES)

  const scalar = WasmDriver.create(scalarPath, arrow, maxSize)
  const fast = WasmDriver.create(fastPath, arrow, maxSize)

  console.log(`\n== fast ↔ scalar size sweep — ${probe.length} sources × [${SWEEP_SIZES.join(', ')}] ==`)
  let totalDiffCells = 0
  const firstFail: string[] = []
  for (const size of SWEEP_SIZES) {
    let diffCells = 0
    for (const c of probe) {
      const a = scalar.render(c, size)
      const b = fast.render(c, size)
      if (a.length !== b.length) throw new Error(`SWEEP FAIL — ${c.sourceId} @${size}: length ${a.length} != ${b.length}`)
      let differs = false
      for (let i = 0; i < a.length; i++) {
        if (a[i] !== b[i]) {
          differs = true
          if (firstFail.length < 20) firstFail.push(`${c.sourceId} @${size}: first diff at byte ${i} scalar=${a[i]} fast=${b[i]}`)
          break
        }
      }
      if (differs) diffCells++
    }
    console.log(`  ${String(size).padStart(4)}px: ${diffCells === 0 ? 'OK — fast == scalar' : `${diffCells} cells differ`}`)
    totalDiffCells += diffCells
  }

  if (totalDiffCells !== 0) {
    for (const f of firstFail) console.log(`  ${f}`)
    throw new Error(`SWEEP FAIL — fast != scalar in ${totalDiffCells} (cell,size) pairs`)
  }
  console.log('RESULT: PASS — fast == scalar at every off-golden size')
}

if (import.meta.main) sizeSweep()
