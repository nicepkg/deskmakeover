#!/usr/bin/env bun
// Off-golden size sweep — fast-vs-scalar self-consistency at sizes the TS golden
// does not cover. The 256px gate (run.ts) diffs against the frozen golden, but a
// mask-cache keyed on (shape, dims, offset) could collide or miskey and pass at
// 256 while corrupting other sizes, where there is no golden to catch it. Because
// `fast` must be byte-identical to the scalar reference at EVERY size, we diff the
// two kernels directly here.
//
// The probe set covers the corpus's DISTINCT geometry keys — every (shape, mark,
// distinction, shortcut, show-original, shortcut-shape) combination — not one cell
// per source. Per-source-first-cell only reached 4 shapes and the Halo mark, so a
// 47px Diamond or a fractional Shadow-offset cache-key bug slipped every 256px
// golden AND every sweep probe (Codex Phase-0 audit #3). This is the primary guard
// for the Phase-1 mask cache.
//
// Today (empty `fast` feature) the two binaries are identical, so this is a no-op
// green; it goes live the moment Phase 1 fills `fast` with the mask cache.
//
//   bun tests/icon-parity/m6/sizes.ts    # builds both kernels, sweeps

import { buildWasm, type CorpusCell, loadArrow, loadCells, WasmDriver } from './harness'

// small · odd (non-power-of-two, exercises fractional offsets) · preview · large.
export const SWEEP_SIZES = [32, 47, 96, 129, 512]

// Distinct (shape, markStyle, distinction, isShortcut, showOriginal, shortcutShape)
// keys in the certified corpus. Pinned so a truncated/thinned cells.jsonl (fewer
// distinct keys) fails the sweep even when run standalone (Codex Phase-0 audit P2).
export const DISTINCT_GEOMETRY_KEYS = 28

function geometryKey(c: CorpusCell): string {
  const g = c.config
  return `${g.shape}|${g.markStyle}|${g.distinction}|${c.isShortcut}|${c.showOriginal}|${g.shortcutShape}`
}

/** One representative cell per distinct geometry key. */
function probeSet(): CorpusCell[] {
  const seen = new Set<string>()
  const probe: CorpusCell[] = []
  for (const c of loadCells()) {
    const k = geometryKey(c)
    if (seen.has(k)) continue
    seen.add(k)
    probe.push(c)
  }
  return probe
}

/** Diff fast vs scalar over the geometry-key probe set at each sweep size. Pass
 *  prebuilt artifact paths to reuse run.ts's builds; omit to build here. */
export function sizeSweep(scalarPath = buildWasm('scalar'), fastPath = buildWasm('fast')): void {
  const arrow = loadArrow()
  const probe = probeSet()
  if (probe.length !== DISTINCT_GEOMETRY_KEYS) {
    throw new Error(`SWEEP FAIL — probe set has ${probe.length} distinct geometry keys, expected ${DISTINCT_GEOMETRY_KEYS} (thinned/truncated corpus?)`)
  }
  const maxSize = Math.max(...SWEEP_SIZES)

  const scalar = WasmDriver.create(scalarPath, arrow, maxSize)
  const fast = WasmDriver.create(fastPath, arrow, maxSize)

  console.log(`\n== fast ↔ scalar size sweep — ${probe.length} distinct geometry keys × [${SWEEP_SIZES.join(', ')}] ==`)
  let totalDiffCells = 0
  const firstFail: string[] = []
  for (const size of SWEEP_SIZES) {
    let diffCells = 0
    for (const c of probe) {
      const a = scalar.render(c, size)
      const b = fast.render(c, size)
      if (a.length !== b.length) throw new Error(`SWEEP FAIL — ${c.file} @${size}: length ${a.length} != ${b.length}`)
      let differs = false
      for (let i = 0; i < a.length; i++) {
        if (a[i] !== b[i]) {
          differs = true
          if (firstFail.length < 20) firstFail.push(`${c.file} [${geometryKey(c)}] @${size}: first diff at byte ${i} scalar=${a[i]} fast=${b[i]}`)
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
  console.log(`RESULT: PASS — fast == scalar at every off-golden size across ${probe.length} geometry keys`)
}

if (import.meta.main) sizeSweep()
