#!/usr/bin/env bun
// Off-golden sweep — fast-vs-scalar self-consistency at sizes AND shape×mark
// combinations the TS golden does not cover. Two probe sets:
//
//   1. Corpus geometry keys — one cell per distinct (shape, mark, distinction,
//      shortcut, show-original, shortcut-shape) key present in the corpus (28).
//   2. Synthetic shape×mark cross-product — EVERY shape × EVERY mark (84), built off
//      a valid base config, INDEPENDENT of whether the corpus contains it. The corpus
//      pairs only 21 of 84 combos: it has Apple+Shadow but NOT Diamond+Shadow, so a
//      stamp/mask cache key that drops or hardcodes shape corrupts Diamond+Shadow
//      while passing every 256px golden AND the corpus sweep (Apple+Shadow and all
//      unshifted shapes stay green). The synthetic set is the only guard that renders
//      the missing 63 combos (Codex Phase-0 audit #3). Risk sites: styles.rs fractional
//      stamp + compose::render_tile_cached mark-activation predicate.
//
// Because fast must be byte-identical to the scalar reference at EVERY size for EVERY
// combination, both sets diff the two kernels directly. Today (empty fast) they are
// identical; the guard goes live the moment Phase 1 fills the mask cache.
//
//   bun tests/icon-parity/m6/sizes.ts

import type { ConfigDto } from '../../../src/bridge/types'
import { buildWasm, type CorpusCell, loadArrow, loadCells, WasmDriver } from './harness'

// small · odd (non-power-of-two, exercises fractional offsets) · preview · mid ·
// near-master. The top probe is the largest IN-CONTRACT size: sources register at 256²
// and the session ABI refuses `size > MAX_RENDER_SIZE` (256, code 6 — the caller's `out`
// buffer is a fixed 256²·4 scratch), and the JS loader throws the same. 255 is the max
// off-golden downscale (256 itself is the certified golden) and its odd near-cap value is
// the likeliest to expose an off-by-one in any size-keyed cache. An upscale >256 is
// unreachable in production, so the sweep no longer probes it (was 512, refused post-C-1).
export const SWEEP_SIZES = [32, 47, 96, 129, 255]

// Distinct (shape, markStyle, distinction, isShortcut, showOriginal, shortcutShape)
// keys in the certified corpus — pinned so a thinned corpus fails standalone (P2).
export const DISTINCT_GEOMETRY_KEYS = 28

// The full geometry space the fast kernel's mask/stamp cache must key correctly.
const ALL_SHAPES: ConfigDto['shape'][] = ['Apple', 'Circle', 'Samsung', 'None', 'Bookmark', 'Lemon', 'Tile', 'Teardrop', 'Diamond', 'Flower', 'Pebble', 'Folder']
const ALL_MARKS: ConfigDto['markStyle'][] = ['Glass', 'Shadow', 'Halo', 'Satin', 'Arc', 'Fold', 'Ring']
export const SYNTHETIC_PROBE_COUNT = ALL_SHAPES.length * ALL_MARKS.length // 84

function geometryKey(c: CorpusCell): string {
  const g = c.config
  return `${g.shape}|${g.markStyle}|${g.distinction}|${c.isShortcut}|${c.showOriginal}|${g.shortcutShape}`
}

/** One representative corpus cell per distinct geometry key. */
function corpusProbes(): CorpusCell[] {
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

/** EVERY shape × EVERY mark as an activating shortcut Mark cell, built off a real
 *  corpus config (so all ConfigDto fields stay valid) — corpus-independent, so the
 *  63 combos the corpus never pairs are still rendered fast-vs-scalar. */
function syntheticProbes(): CorpusCell[] {
  const cells = loadCells()
  const seed = cells.find((c) => c.isShortcut && c.config.distinction === 'Mark') ?? cells[0]
  const probes: CorpusCell[] = []
  for (const shape of ALL_SHAPES) {
    for (const markStyle of ALL_MARKS) {
      probes.push({
        file: `synthetic__${shape}__${markStyle}`,
        sourceId: seed.sourceId,
        lane: 'synthetic',
        config: { ...seed.config, shape, markStyle, distinction: 'Mark', shortcutShape: null },
        isShortcut: true,
        showOriginal: false,
        fieldSeed: null,
      })
    }
  }
  return probes
}

/** Diff fast vs scalar over `probes` at each sweep size; throws on any divergence. */
function diffProbes(label: string, probes: CorpusCell[], scalar: WasmDriver, fast: WasmDriver): void {
  console.log(`\n== fast ↔ scalar ${label} — ${probes.length} probes × [${SWEEP_SIZES.join(', ')}] ==`)
  let totalDiff = 0
  const firstFail: string[] = []
  for (const size of SWEEP_SIZES) {
    let diffCells = 0
    for (const c of probes) {
      const a = scalar.render(c, size)
      const b = fast.render(c, size)
      if (a.length !== b.length) throw new Error(`SWEEP FAIL — ${c.file} @${size}: length ${a.length} != ${b.length}`)
      for (let i = 0; i < a.length; i++) {
        if (a[i] !== b[i]) {
          diffCells++
          if (firstFail.length < 20) firstFail.push(`${c.file} @${size}: first diff at byte ${i} scalar=${a[i]} fast=${b[i]}`)
          break
        }
      }
    }
    console.log(`  ${String(size).padStart(4)}px: ${diffCells === 0 ? 'OK — fast == scalar' : `${diffCells} cells differ`}`)
    totalDiff += diffCells
  }
  if (totalDiff !== 0) {
    for (const f of firstFail) console.log(`  ${f}`)
    throw new Error(`SWEEP FAIL — ${label}: fast != scalar in ${totalDiff} (probe,size) pairs`)
  }
  console.log(`RESULT: PASS — ${label}: fast == scalar at every size`)
}

/** Diff fast vs scalar over the corpus geometry keys AND the full synthetic shape×mark
 *  cross-product. Pass prebuilt artifact paths to reuse run.ts's builds. */
export function sizeSweep(scalarPath = buildWasm('scalar'), fastPath = buildWasm('fast')): void {
  const arrow = loadArrow()
  const corpus = corpusProbes()
  if (corpus.length !== DISTINCT_GEOMETRY_KEYS) {
    throw new Error(`SWEEP FAIL — corpus probe set has ${corpus.length} distinct geometry keys, expected ${DISTINCT_GEOMETRY_KEYS} (thinned/truncated corpus?)`)
  }
  const synthetic = syntheticProbes()
  if (synthetic.length !== SYNTHETIC_PROBE_COUNT) {
    throw new Error(`SWEEP FAIL — synthetic set has ${synthetic.length} probes, expected ${SYNTHETIC_PROBE_COUNT}`)
  }

  const maxSize = Math.max(...SWEEP_SIZES)
  const scalar = WasmDriver.create(scalarPath, arrow, maxSize)
  const fast = WasmDriver.create(fastPath, arrow, maxSize)

  diffProbes('corpus geometry keys', corpus, scalar, fast)
  diffProbes(`synthetic shape×mark cross-product (${ALL_SHAPES.length}×${ALL_MARKS.length})`, synthetic, scalar, fast)
}

if (import.meta.main) sizeSweep()
