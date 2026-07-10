// Dev tool: render every icon-shape mask off-screen and dump PGMs for a
// contact sheet — the fastest way to eyeball silhouette quality when tuning
// geometry in icon-compositor/shapes.ts.
//
// Usage: bun scripts/render-shape-sheet.ts [outDir] [size]

import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { IconShape } from '../src/bridge/types'
import { shapeMask } from '../src/icon-compositor/raster'

const OUT = process.argv[2] ?? '/tmp/shape-sheet'
const SIZE = Number(process.argv[3] ?? 160)

const SHAPES: IconShape[] = [
  'Apple', 'Circle', 'Samsung', 'None', 'Tile', 'Teardrop',
  'Bookmark', 'Lemon', 'Diamond', 'Flower', 'Pebble',
]

mkdirSync(OUT, { recursive: true })
for (const shape of SHAPES) {
  const mask = shapeMask(shape, SIZE, SIZE, 0, 0)
  const px = new Uint8Array(SIZE * SIZE)
  for (let i = 0; i < mask.length; i++) px[i] = Math.round(mask[i] * 255)
  const header = `P5\n${SIZE} ${SIZE}\n255\n`
  const buf = new Uint8Array(header.length + px.length)
  buf.set(new TextEncoder().encode(header), 0)
  buf.set(px, header.length)
  writeFileSync(join(OUT, `${shape}.pgm`), buf)
  console.log(shape)
}
console.log(`masks -> ${OUT}`)
