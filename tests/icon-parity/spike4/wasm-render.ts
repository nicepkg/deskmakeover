#!/usr/bin/env bun
// Spike 4 — WASM side of the tri-target slice. Loads the plain
// wasm32-unknown-unknown build of dm-icon-wasm (no wasm-bindgen; raw
// linear-memory ABI — recorded choice, see crates/dm-icon-wasm/src/lib.rs) in
// Bun's WebAssembly and renders every dumped source at both slice sizes.
//
//   bun tests/icon-parity/spike4/wasm-render.ts [spike4Dir]

import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

const REPO_ROOT = resolve(import.meta.dir, '../../..')
const DIR = process.argv[2] ?? join(REPO_ROOT, 'target/spike4')
const WASM = join(REPO_ROOT, 'target/wasm32-unknown-unknown/release/dm_icon_wasm.wasm')

const SIZES = [256, 512] as const // must match scripts/spike4-slice.ts
const SRC = 256

interface Spike4Exports {
  memory: WebAssembly.Memory
  spike4_alloc(len: number): number
  spike4_render_slice(src: number, srcW: number, srcH: number, size: number, out: number): number
}

function main(): void {
  const module = new WebAssembly.Module(readFileSync(WASM))
  const instance = new WebAssembly.Instance(module, {})
  const wasm = instance.exports as unknown as Spike4Exports

  const srcPtr = wasm.spike4_alloc(SRC * SRC * 4)
  const outPtrs = new Map(SIZES.map((s) => [s, wasm.spike4_alloc(s * s * 4)]))

  mkdirSync(join(DIR, 'wasm'), { recursive: true })
  const ids = readdirSync(join(DIR, 'sources'))
    .filter((f) => f.endsWith('.rgba'))
    .map((f) => f.slice(0, -'.rgba'.length))
    .sort()

  let cells = 0
  for (const id of ids) {
    const src = readFileSync(join(DIR, `sources/${id}.rgba`))
    for (const size of SIZES) {
      // Re-view memory each call: growth may detach earlier ArrayBuffers.
      new Uint8Array(wasm.memory.buffer, srcPtr, SRC * SRC * 4).set(src)
      const outPtr = outPtrs.get(size)!
      const code = wasm.spike4_render_slice(srcPtr, SRC, SRC, size, outPtr)
      if (code !== 0) throw new Error(`spike4_render_slice(${id}, ${size}) → ${code}`)
      const out = new Uint8Array(wasm.memory.buffer, outPtr, size * size * 4)
      writeFileSync(join(DIR, `wasm/${id}-${size}.rgba`), out)
      cells++
    }
  }
  console.log(`spike4 wasm: ${ids.length} sources × ${SIZES.length} sizes = ${cells} cells → ${join(DIR, 'wasm')}`)
}

main()
