#!/usr/bin/env bun
// M5 module-2 gate — dump the FROZEN TS `shapeMask` (the production consumer of
// shapeContains → the polygon builders) as raw Float64Array bytes for every
// catalog shape at representative sizes, so `xtask m5-shape-masks` can prove the
// Rust polygon geometry (libm hypot/tan/atan2/acos vs JSC) is bit-identical.
//
//   bun tests/icon-parity/m5/shape-masks.ts [outDir]

import { mkdirSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import type { IconShape } from '@/bridge/types'
import { shapeMask } from '@/icon-compositor/raster'

const REPO_ROOT = resolve(import.meta.dir, '../../..')
const OUT = join(process.argv[2] ?? join(REPO_ROOT, 'target/m5'), 'shapes')

const SHAPES: IconShape[] = [
  'Apple', 'Circle', 'Samsung', 'None', 'Bookmark', 'Lemon',
  'Tile', 'Teardrop', 'Diamond', 'Flower', 'Pebble', 'Folder',
]
// Whole-tile mask (offset 0) + an inset card mask (pad) — the two shapes the
// compositor actually builds (tileAlpha + cardMask).
const CASES = [
  { size: 48, shapeSize: 48, ox: 0, oy: 0 },
  { size: 256, shapeSize: 256, ox: 0, oy: 0 },
  { size: 256, shapeSize: 232, ox: 12, oy: 12 },
  { size: 512, shapeSize: 512, ox: 0, oy: 0 },
]

mkdirSync(OUT, { recursive: true })
let count = 0
for (const shape of SHAPES) {
  for (const c of CASES) {
    const mask = shapeMask(shape, c.size, c.shapeSize, c.ox, c.oy)
    const name = `${shape}-${c.size}-${c.shapeSize}-${c.ox}-${c.oy}.f64`
    writeFileSync(join(OUT, name), Buffer.from(mask.buffer, mask.byteOffset, mask.byteLength))
    count++
  }
}
console.log(`shape masks: ${SHAPES.length} shapes × ${CASES.length} cases = ${count} → ${OUT}`)
