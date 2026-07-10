#!/usr/bin/env node
// Dev-only mock icon pack generator (spec 06 §5). Procedurally draws ~120 messy
// desktop icons at 256px so the icons module has ship-safe, encumbrance-free source
// art to render, style and stress-test against. Everything is OWN art: no extracted
// Windows icons, no brand marks. Output + manifest live in the web app's public/
// folder and are committed; this script is re-runnable and deterministic (seeded RNG —
// pass a seed as argv[2] to fork a fresh pack).
//
// This entry stays thin; the generator lives in ./mock-icons/*.mjs (each ≤500 lines):
//   constants · prng · color · raster · glyphs · categories · labels · png.

import { writeFileSync, mkdirSync, rmSync, existsSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { SEED } from './mock-icons/constants.mjs'
import { mulberry32 } from './mock-icons/prng.mjs'
import { renderIcon } from './mock-icons/raster.mjs'
import { build, kindFor } from './mock-icons/categories.mjs'
import { labelFor } from './mock-icons/labels.mjs'

const root = new URL('../..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')
const OUT_DIR = join(root, 'public', 'mock-icons')

// Distribution (spec 06 §5), summing to 120.
const PLAN = [
  ['flat', 36],
  ['skeuo', 18],
  ['photo', 14],
  ['badged', 12],
  ['transparent', 14],
  ['letter', 10],
  ['folder', 10],
  ['document', 6],
]

function main() {
  const rng = mulberry32(SEED)
  if (existsSync(OUT_DIR)) for (const f of readdirSync(OUT_DIR)) if (/\.(png|webp|json)$/.test(f)) rmSync(join(OUT_DIR, f))
  mkdirSync(OUT_DIR, { recursive: true })

  const specs = []
  for (const [cat, count] of PLAN) for (let i = 0; i < count; i++) specs.push({ cat, i })
  // Guarantee every taxonomy kind appears at least once (manifest integrity).
  const forced = ['lnk', 'exe', 'url', 'uwp', 'bin']
  const nonReserved = specs.map((s, idx) => idx).filter((idx) => specs[idx].cat !== 'folder' && specs[idx].cat !== 'document')

  const manifest = []
  let total = 0, min = Infinity, max = 0
  const kindTally = {}
  specs.forEach((s, idx) => {
    const { layers, opts } = build(s.cat, rng, s.i)
    let kind
    const fpos = nonReserved.indexOf(idx)
    if (fpos > -1 && fpos < forced.length) kind = forced[fpos]
    else kind = kindFor(rng, s.cat)
    const png = renderIcon(layers, opts)
    const file = `icon-${String(idx).padStart(3, '0')}.png`
    writeFileSync(join(OUT_DIR, file), png)
    total += png.length
    min = Math.min(min, png.length)
    max = Math.max(max, png.length)
    kindTally[kind] = (kindTally[kind] ?? 0) + 1
    manifest.push({ file, id: `mock-${String(idx).padStart(3, '0')}`, kind, label: labelFor(rng, s.cat, kind) })
  })
  writeFileSync(join(OUT_DIR, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n')

  const kb = (n) => (n / 1024).toFixed(1)
  console.log(`mock-icons: ${manifest.length} icons + manifest.json → ${OUT_DIR}`)
  console.log(`pack size: ${kb(total)} KB total · min ${kb(min)} KB · max ${kb(max)} KB · avg ${kb(total / manifest.length)} KB`)
  console.log(`distribution: ${PLAN.map(([c, n]) => `${c}=${n}`).join(' · ')}`)
  console.log(`kinds: ${Object.entries(kindTally).map(([k, n]) => `${k}=${n}`).join(' · ')}`)
  console.log(`seed: ${SEED}`)
}

main()
