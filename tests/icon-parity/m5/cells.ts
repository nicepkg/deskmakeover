#!/usr/bin/env bun
// M5 modules 8+9+10 gate — the FULL pixel differential. Re-render every tier-a
// master + tier-b style-matrix cell from the committed corpus manifests, dump
// the expected RGBA + the executed lane/fieldLane, so `xtask m5-pixels` can
// render the same cell through dm-icon-core (compose+marks+filters) and prove
// byte parity. The real Win11 arrow asset is composited exactly as the app does.
//
//   bun tests/icon-parity/m5/cells.ts [outDir]

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { renderTile } from '@/icon-compositor/compose'
import type { ComposeDiagnostics } from '@/icon-compositor/compose'
import { setNativeArrowRaster } from '@/icon-compositor/marks'
import { decodePng } from '../../../scripts/oracle/png-codec'
import { loadMockSources } from '../../../scripts/oracle/desktop-session'

const REPO_ROOT = resolve(import.meta.dir, '../../..')
const TESTDATA = join(REPO_ROOT, 'testdata/icons')
const OUT = join(process.argv[2] ?? join(REPO_ROOT, 'target/m5'), 'pixels')
const MASTER = 256

interface Cell {
  path: string
  sourceId: string
  config: unknown
  isShortcut: boolean
  showOriginal: boolean
  opts: { fieldSeed: string | null; kindBucket: string | null } | null
}

mkdirSync(join(OUT, 'expected'), { recursive: true })
mkdirSync(join(OUT, 'sources'), { recursive: true })

// The genuine Win11 shortcut-arrow badge, exactly as the worker loads it.
const arrowBytes = readFileSync(join(REPO_ROOT, 'public/win-native-arrow.png'))
const arrow = decodePng(new Uint8Array(arrowBytes))
setNativeArrowRaster(arrow)
writeFileSync(join(OUT, 'arrow.rgba'), Buffer.from(arrow.data.buffer, arrow.data.byteOffset, arrow.data.byteLength))
writeFileSync(join(OUT, 'arrow.json'), JSON.stringify({ width: arrow.width, height: arrow.height }))

const sources = loadMockSources(REPO_ROOT)
const byId = new Map(sources.map((s) => [s.id, s]))
for (const s of sources) {
  const d = s.raster.data
  writeFileSync(join(OUT, `sources/${s.id}.rgba`), Buffer.from(d.buffer, d.byteOffset, d.byteLength))
}

const tierA = JSON.parse(readFileSync(join(TESTDATA, 'tier-a/cells.json'), 'utf8')) as { cells: Cell[] }
const tierB = JSON.parse(readFileSync(join(TESTDATA, 'tier-b/cells.json'), 'utf8')) as { cells: Cell[] }
const cells = [...tierA.cells, ...tierB.cells]

const records: string[] = []
for (const cell of cells) {
  const src = byId.get(cell.sourceId)
  if (!src) throw new Error(`missing source ${cell.sourceId}`)
  const diag = {} as ComposeDiagnostics
  const raster = renderTile(src.raster, cell.config as never, cell.isShortcut, cell.showOriginal, MASTER, cell.opts ?? undefined, diag)
  const file = cell.path.replace(/\//g, '__').replace(/\.png$/, '')
  writeFileSync(join(OUT, `expected/${file}.rgba`), Buffer.from(raster.data.buffer, raster.data.byteOffset, raster.data.byteLength))
  records.push(
    JSON.stringify({
      file,
      sourceId: cell.sourceId,
      config: cell.config,
      isShortcut: cell.isShortcut,
      showOriginal: cell.showOriginal,
      opts: cell.opts,
      lane: diag.lane,
      fieldLane: diag.fieldLane ?? null,
    }),
  )
}
writeFileSync(join(OUT, 'cells.jsonl'), records.join('\n'))
console.log(`pixels: ${cells.length} cells (${tierA.cells.length} tier-A + ${tierB.cells.length} tier-B) dumped → ${OUT}`)
