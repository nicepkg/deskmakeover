#!/usr/bin/env bun
// M5 modules 4+5 gate — dump each mock source's decoded RGBA and the resolved
// subject mask (raw 0/1 bytes) so `xtask m5-profiles` can (a) recompute the
// StageProfile in Rust and deep-compare it against the PERMANENT committed
// corpus (testdata/icons/sources/profiles/<id>.json) and (b) byte-compare the
// segment mask. Validates analysis + segment + profile jointly.
//
//   bun tests/icon-parity/m5/profiles.ts [outDir]

import { mkdirSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { loadMockSources } from '../../../scripts/oracle/desktop-session'
import { iconProfile } from '@/icon-compositor/profile'
import { segmentSubject } from '@/icon-compositor/segment'

const REPO_ROOT = resolve(import.meta.dir, '../../..')
const OUT = join(process.argv[2] ?? join(REPO_ROOT, 'target/m5'), 'profiles')
mkdirSync(join(OUT, 'sources'), { recursive: true })
mkdirSync(join(OUT, 'masks'), { recursive: true })

const sources = loadMockSources(REPO_ROOT)
for (const s of sources) {
  const rd = s.raster.data
  writeFileSync(join(OUT, `sources/${s.id}.rgba`), Buffer.from(rd.buffer, rd.byteOffset, rd.byteLength))
  // The resolved mask exactly as stage-dump.ts derives it.
  const p = iconProfile(s.raster)
  const mask = p.subjectMask ?? segmentSubject(s.raster).mask
  writeFileSync(join(OUT, `masks/${s.id}.bin`), Buffer.from(mask.buffer, mask.byteOffset, mask.byteLength))
}
console.log(`profiles: ${sources.length} sources + masks dumped → ${OUT}`)
