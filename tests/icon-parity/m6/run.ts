#!/usr/bin/env bun
// M6 byte-parity gate — drives the FULL `dm-icon-wasm` render_tile ABI (the M6
// worker-pool adapter, not the Spike-4 slice) over the real 1487-cell M5 corpus
// and asserts every cell is byte-identical to the frozen TS golden. This is the
// hard flip gate: the preview path may only move to WASM once this is 1487/1487.
//
//   bun tests/icon-parity/m6/run.ts
//
// It reuses the M5 pixel corpus (target/m5/pixels: cells.jsonl + sources/ +
// expected/ + the genuine Win11 arrow), regenerating it via the M5 dumper if
// absent, and (re)builds the release wasm. The config encoder here is the exact
// 24-byte packed record `dm-icon-wasm/src/abi.rs` decodes — it becomes the P2
// worker's encoder.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

const REPO_ROOT = resolve(import.meta.dir, '../../..')
const PIXELS = join(REPO_ROOT, 'target/m5/pixels')
const WASM = join(REPO_ROOT, 'target/wasm32-unknown-unknown/release/dm_icon_wasm.wasm')
const CORPUS_SIZE = 256 // every corpus cell renders at 256² (xtask CORPUS_SIZE)

// ---- config marshalling (mirrors dm-icon-wasm/src/abi.rs layout) ------------

const CONFIG_BYTES = 24
const SHAPE = ['Apple', 'Circle', 'Samsung', 'None', 'Bookmark', 'Lemon', 'Tile', 'Teardrop', 'Diamond', 'Flower', 'Pebble', 'Folder']
const SUBJECT = ['Original', 'BlackWhite', 'Mono']
const MONO = ['Tonal', 'Flat']
const BAND = ['Vivid', 'Quiet']
const DISTINCTION = ['Mark', 'Keep', 'None']
const MARK = ['Glass', 'Shadow', 'Halo', 'Satin', 'Arc', 'Fold', 'Ring']
const FILTER = ['None', 'Gloss', 'Glass', 'Pixel', 'Sticker']
const FALLBACK = ['derived', 'white']

function tag(list: string[], v: string, field: string): number {
  const i = list.indexOf(v)
  if (i < 0) throw new Error(`bad ${field}: ${v}`)
  return i
}
function hexToInt(hex: string): number {
  return parseInt(hex.replace('#', ''), 16) & 0xffffff
}
function encodeConfig(c: Record<string, string | null>): Uint8Array {
  const b = new Uint8Array(CONFIG_BYTES)
  const dv = new DataView(b.buffer)
  b[0] = tag(SHAPE, c.shape as string, 'shape')
  b[1] = tag(SUBJECT, c.subject as string, 'subject')
  b[2] = tag(MONO, c.monoStyle as string, 'monoStyle')
  b[3] = tag(BAND, c.plateBand as string, 'plateBand')
  b[4] = tag(DISTINCTION, c.distinction as string, 'distinction')
  b[5] = tag(MARK, c.markStyle as string, 'markStyle')
  b[6] = tag(FILTER, c.filter as string, 'filter')
  b[7] = tag(FALLBACK, c.plateFallback as string, 'plateFallback')
  b[8] = c.shortcutShape == null ? 0xff : tag(SHAPE, c.shortcutShape, 'shortcutShape')
  b[9] = c.markColor == null ? 0 : 1
  b[10] = c.plateColor == null ? 0 : 1
  dv.setUint32(12, hexToInt(c.tint as string), true)
  if (c.markColor != null) dv.setUint32(16, hexToInt(c.markColor), true)
  if (c.plateColor != null) dv.setUint32(20, hexToInt(c.plateColor), true)
  return b
}

// ---- wasm harness -----------------------------------------------------------

interface M6Exports {
  memory: WebAssembly.Memory
  dm_alloc(len: number): number
  dm_set_native_arrow(ptr: number, w: number, h: number): number
  dm_session_new(): number
  dm_session_register(s: number, idPtr: number, idLen: number, sourceHash: bigint, srcPtr: number, w: number, h: number): number
  dm_session_set_config(s: number, cfgPtr: number, cfgLen: number): number
  dm_session_render(s: number, idPtr: number, idLen: number, isShortcut: number, showOriginal: number, size: number, hasFieldSeed: number, fieldSeed: number, out: number): number
}

function ensureCorpus(): void {
  if (existsSync(join(PIXELS, 'cells.jsonl'))) return
  console.log('== M5 corpus missing → dumping (bun tests/icon-parity/m5/cells.ts)')
  const r = Bun.spawnSync(['bun', 'tests/icon-parity/m5/cells.ts'], { cwd: REPO_ROOT, stdout: 'inherit', stderr: 'inherit' })
  if (r.exitCode !== 0) process.exit(r.exitCode ?? 1)
}

function buildWasm(): void {
  console.log('== build wasm: cargo build --target wasm32-unknown-unknown --release -p dm-icon-wasm')
  const r = Bun.spawnSync(
    ['cargo', 'build', '--target', 'wasm32-unknown-unknown', '--release', '-p', 'dm-icon-wasm'],
    { cwd: REPO_ROOT, stdout: 'inherit', stderr: 'inherit' },
  )
  if (r.exitCode !== 0) process.exit(r.exitCode ?? 1)
}

interface LaneStat {
  cells: number
  equal: number
  diffBytes: number
  totalBytes: number
}

function main(): void {
  ensureCorpus()
  buildWasm()

  const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(WASM)), {})
  const wasm = instance.exports as unknown as M6Exports
  const mem = () => new Uint8Array(wasm.memory.buffer) // re-view: alloc/register may grow + detach

  // Install the genuine Win11 arrow badge, exactly as the corpus dumper does.
  const arrowMeta = JSON.parse(readFileSync(join(PIXELS, 'arrow.json'), 'utf8')) as { width: number; height: number }
  const arrowBytes = readFileSync(join(PIXELS, 'arrow.rgba'))
  const arrowPtr = wasm.dm_alloc(arrowMeta.width * arrowMeta.height * 4)
  mem().set(arrowBytes, arrowPtr)
  if (wasm.dm_set_native_arrow(arrowPtr, arrowMeta.width, arrowMeta.height) !== 0) throw new Error('dm_set_native_arrow failed')

  const session = wasm.dm_session_new()
  const srcPtr = wasm.dm_alloc(CORPUS_SIZE * CORPUS_SIZE * 4)
  const outPtr = wasm.dm_alloc(CORPUS_SIZE * CORPUS_SIZE * 4)
  const cfgPtr = wasm.dm_alloc(CONFIG_BYTES)
  const idPtr = wasm.dm_alloc(256)
  const enc = new TextEncoder()

  // Each distinct sourceId gets a distinct hash so the profile cache never
  // collides (register's trust contract). Register a source once, on first sight.
  const registered = new Map<string, bigint>()
  let nextHash = 1n

  const perLane = new Map<string, LaneStat>()
  const total: LaneStat = { cells: 0, equal: 0, diffBytes: 0, totalBytes: 0 }
  const failures: string[] = []
  const cellBytes = CORPUS_SIZE * CORPUS_SIZE * 4

  const lines = readFileSync(join(PIXELS, 'cells.jsonl'), 'utf8').split('\n').filter((l) => l.length > 0)
  for (const line of lines) {
    const rec = JSON.parse(line)
    const file: string = rec.file
    const sourceId: string = rec.sourceId
    const lane: string = rec.lane

    if (!registered.has(sourceId)) {
      const src = readFileSync(join(PIXELS, `sources/${sourceId}.rgba`))
      mem().set(src, srcPtr)
      const hash = nextHash++
      const idLen = enc.encodeInto(sourceId, mem().subarray(idPtr, idPtr + 256)).written ?? 0
      const code = wasm.dm_session_register(session, idPtr, idLen, hash, srcPtr, CORPUS_SIZE, CORPUS_SIZE)
      if (code !== 0) throw new Error(`register(${sourceId}) → ${code}`)
      registered.set(sourceId, hash)
    }

    mem().set(encodeConfig(rec.config), cfgPtr)
    if (wasm.dm_session_set_config(session, cfgPtr, CONFIG_BYTES) !== 0) throw new Error(`set_config(${file}) failed`)

    const idLen = enc.encodeInto(sourceId, mem().subarray(idPtr, idPtr + 256)).written ?? 0
    const fieldSeed: string | null = rec.opts?.fieldSeed ?? null
    const code = wasm.dm_session_render(
      session,
      idPtr,
      idLen,
      rec.isShortcut ? 1 : 0,
      rec.showOriginal ? 1 : 0,
      CORPUS_SIZE,
      fieldSeed == null ? 0 : 1,
      fieldSeed == null ? 0 : hexToInt(fieldSeed),
      outPtr,
    )
    if (code !== 0) throw new Error(`render(${file}) → ${code}`)

    const out = mem().subarray(outPtr, outPtr + cellBytes)
    const expected = readFileSync(join(PIXELS, `expected/${file}.rgba`))

    const s = perLane.get(lane) ?? { cells: 0, equal: 0, diffBytes: 0, totalBytes: 0 }
    s.cells++
    total.cells++
    s.totalBytes += cellBytes
    total.totalBytes += cellBytes

    let diff = 0
    let firstAt = -1
    for (let i = 0; i < cellBytes; i++) {
      if (out[i] !== expected[i]) {
        diff++
        if (firstAt < 0) firstAt = i
      }
    }
    s.diffBytes += diff
    total.diffBytes += diff
    if (diff === 0) {
      s.equal++
      total.equal++
    } else if (failures.length < 40) {
      const px = (firstAt / 4) | 0
      failures.push(`${file} [${lane}]: ${diff} diff bytes, first at (${px % CORPUS_SIZE},${(px / CORPUS_SIZE) | 0}) ch ${firstAt % 4} wasm=${out[firstAt]} ts=${expected[firstAt]}`)
    }
    perLane.set(lane, s)
  }

  console.log('\n== M6 full pixel differential — dm-icon-wasm render_tile ↔ TS golden ==')
  console.log(`${'lane'.padEnd(18)}${'cells'.padStart(6)}${'equal-cells'.padStart(13)}${'diff-bytes'.padStart(19)}`)
  for (const [lane, s] of [...perLane].sort(([a], [b]) => (a < b ? -1 : 1))) {
    console.log(`${lane.padEnd(18)}${String(s.cells).padStart(6)}${String(s.equal).padStart(13)}${`${s.diffBytes}/${s.totalBytes}`.padStart(19)}`)
  }
  console.log(`${'TOTAL'.padEnd(18)}${String(total.cells).padStart(6)}${String(total.equal).padStart(13)}${`${total.diffBytes}/${total.totalBytes}`.padStart(19)}`)

  if (total.equal === total.cells && total.cells > 0) {
    console.log(`RESULT: PASS — all ${total.cells} cells byte-identical (WASM render_tile ABI == TS golden), 0/${total.totalBytes} diff bytes`)
    return
  }
  console.log('RESULT: FAIL')
  for (const f of failures) console.log(`  ${f}`)
  process.exit(1)
}

main()
