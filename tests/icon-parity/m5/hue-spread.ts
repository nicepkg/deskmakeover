#!/usr/bin/env bun
// M5 module-6 gate — dump the FROZEN computeHueSpread input entries + output for
// every preset (the derived-participant filter mirrors the store's
// computeFieldSeeds, minus the app-accent fallback which is app-layer, not
// icon-compositor). `xtask m5-hue` reruns compute_hue_spread in Rust and checks
// the id→hex map is identical.
//
//   bun tests/icon-parity/m5/hue-spread.ts [outDir]

import { mkdirSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { computeHueSpread } from '@/icon-compositor/hue-spread'
import type { SpreadEntry } from '@/icon-compositor/hue-spread'
import { resolveTypeConfig, typeHasFixedPlate } from '@/lib/type-config'
import type { IconKindBucket } from '@/bridge/types'
import { loadMockSources, lookOf, PRESET_IDS } from '../../../scripts/oracle/desktop-session'

const REPO_ROOT = resolve(import.meta.dir, '../../..')
const OUT = join(process.argv[2] ?? join(REPO_ROOT, 'target/m5'), 'hue')
mkdirSync(OUT, { recursive: true })

const sources = loadMockSources(REPO_ROOT)
for (const id of PRESET_IDS) {
  const { config, typeOverrides } = lookOf(id)
  const isDerivedParticipant = (bucket: IconKindBucket | null): boolean => {
    if (typeHasFixedPlate(typeOverrides, bucket)) return false
    const r = resolveTypeConfig(config, typeOverrides, bucket)
    return r.subject === 'Original' && r.plateColor === null && r.plateFallback !== 'white'
  }
  const entries: SpreadEntry[] = sources
    .filter((s) => isDerivedParticipant(s.bucket))
    .map((s) => ({ id: s.id, artKey: s.sourceUrl, seed: s.seed }))
  const out = computeHueSpread(entries)
  writeFileSync(join(OUT, `${id}.json`), JSON.stringify({ entries, output: Object.fromEntries(out) }))
}
console.log(`hue-spread: ${PRESET_IDS.length} presets dumped → ${OUT}`)
