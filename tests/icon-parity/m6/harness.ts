#!/usr/bin/env bun
// Shared WASM driver for the M6 byte-parity gates — builds a kernel variant of
// dm-icon-wasm, instantiates the raw render_tile ABI, and renders corpus cells.
// Used by run.ts (256px vs the frozen TS golden) and sizes.ts (fast vs scalar at
// off-golden sizes). Keeping the ABI + build + driver in one place is why the two
// gates cannot drift apart.

import { copyFileSync, mkdirSync, readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

import type { ConfigDto } from '../../../src/bridge/types'
import { CONFIG_BYTES, encodeConfig, hexToInt } from '../../../src/icon-wasm/config-abi'

export const REPO_ROOT = resolve(import.meta.dir, '../../..')
export const PIXELS = join(REPO_ROOT, 'target/m5/pixels')
export const SOURCE_SIZE = 256 // every corpus source is a 256² raster

export type Kernel = 'scalar' | 'fast'

/** The raw linear-memory ABI dm-icon-wasm exports (no wasm-bindgen). */
export interface M6Exports {
  memory: WebAssembly.Memory
  dm_alloc(len: number): number
  dm_set_native_arrow(ptr: number, w: number, h: number): number
  dm_session_new(): number
  dm_session_register(s: number, idPtr: number, idLen: number, sourceHash: bigint, srcPtr: number, w: number, h: number): number
  dm_session_set_config(s: number, cfgPtr: number, cfgLen: number): number
  dm_session_render(s: number, idPtr: number, idLen: number, isShortcut: number, showOriginal: number, size: number, hasFieldSeed: number, fieldSeed: number, out: number): number
}

export interface CorpusCell {
  file: string
  sourceId: string
  lane: string
  config: ConfigDto
  isShortcut: boolean
  showOriginal: boolean
  fieldSeed: string | null
}

/** Parse cells.jsonl into typed cell records (render order = file order). */
export function loadCells(pixelsDir = PIXELS): CorpusCell[] {
  return readFileSync(join(pixelsDir, 'cells.jsonl'), 'utf8')
    .split('\n')
    .filter((l) => l.length > 0)
    .map((l) => {
      const r = JSON.parse(l)
      return {
        file: r.file as string,
        sourceId: r.sourceId as string,
        lane: r.lane as string,
        config: r.config as ConfigDto,
        isShortcut: !!r.isShortcut,
        showOriginal: !!r.showOriginal,
        fieldSeed: (r.opts?.fieldSeed ?? null) as string | null,
      }
    })
}

export interface Arrow {
  bytes: Uint8Array
  width: number
  height: number
}

export function loadArrow(pixelsDir = PIXELS): Arrow {
  const meta = JSON.parse(readFileSync(join(pixelsDir, 'arrow.json'), 'utf8')) as { width: number; height: number }
  return { bytes: new Uint8Array(readFileSync(join(pixelsDir, 'arrow.rgba'))), width: meta.width, height: meta.height }
}

export function readSource(sourceId: string, pixelsDir = PIXELS): Uint8Array {
  return new Uint8Array(readFileSync(join(pixelsDir, `sources/${sourceId}.rgba`)))
}

function run(cmd: string, args: string[]): void {
  const r = Bun.spawnSync([cmd, ...args], { cwd: REPO_ROOT, stdout: 'inherit', stderr: 'inherit' })
  if (r.exitCode !== 0) throw new Error(`${cmd} ${args.join(' ')} → exit ${r.exitCode}`)
}

/** Build a kernel variant of the release wasm with --locked and copy the artifact
 *  to a kernel-specific path (scalar and fast share the cargo target path, so we
 *  isolate them). Returns the isolated artifact path. */
export function buildWasm(kernel: Kernel): string {
  const feat = kernel === 'fast' ? ['--features', 'fast'] : []
  console.log(`== build wasm (${kernel}): cargo build --locked --release -p dm-icon-wasm ${feat.join(' ')}`)
  run('cargo', ['build', '--locked', '--target', 'wasm32-unknown-unknown', '--release', '-p', 'dm-icon-wasm', ...feat])
  const src = join(REPO_ROOT, 'target/wasm32-unknown-unknown/release/dm_icon_wasm.wasm')
  const outDir = join(REPO_ROOT, 'target/m6')
  mkdirSync(outDir, { recursive: true })
  const dst = join(outDir, `dm_icon_wasm.${kernel}.wasm`)
  copyFileSync(src, dst)
  return dst
}

/** A session-backed driver over one wasm instance. Sources register once (on first
 *  render); the arrow installs at construction. Re-views memory after every wasm
 *  call because dm_alloc / render may grow (and detach) the buffer. */
export class WasmDriver {
  private readonly wasm: M6Exports
  private readonly session: number
  private readonly srcPtr: number
  private readonly outPtr: number
  private readonly cfgPtr: number
  private readonly idPtr: number
  private readonly enc = new TextEncoder()
  private readonly registered = new Map<string, bigint>()
  private nextHash = 1n

  private constructor(wasm: M6Exports, maxSize: number) {
    this.wasm = wasm
    this.session = wasm.dm_session_new()
    this.srcPtr = wasm.dm_alloc(SOURCE_SIZE * SOURCE_SIZE * 4)
    this.outPtr = wasm.dm_alloc(maxSize * maxSize * 4)
    this.cfgPtr = wasm.dm_alloc(CONFIG_BYTES)
    this.idPtr = wasm.dm_alloc(256)
  }

  /** Instantiate `wasmPath`, install the arrow, and return a ready driver sized for
   *  renders up to `maxSize`. */
  static create(wasmPath: string, arrow: Arrow, maxSize: number): WasmDriver {
    const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(wasmPath)), {})
    const wasm = instance.exports as unknown as M6Exports
    const arrowPtr = wasm.dm_alloc(arrow.width * arrow.height * 4)
    new Uint8Array(wasm.memory.buffer).set(arrow.bytes, arrowPtr)
    if (wasm.dm_set_native_arrow(arrowPtr, arrow.width, arrow.height) !== 0) throw new Error('dm_set_native_arrow failed')
    return new WasmDriver(wasm, maxSize)
  }

  private mem(): Uint8Array {
    return new Uint8Array(this.wasm.memory.buffer)
  }

  private idLen(sourceId: string): number {
    return this.enc.encodeInto(sourceId, this.mem().subarray(this.idPtr, this.idPtr + 256)).written ?? 0
  }

  private ensureRegistered(sourceId: string, pixelsDir: string): void {
    if (this.registered.has(sourceId)) return
    this.mem().set(readSource(sourceId, pixelsDir), this.srcPtr)
    const hash = this.nextHash++
    const code = this.wasm.dm_session_register(this.session, this.idPtr, this.idLen(sourceId), hash, this.srcPtr, SOURCE_SIZE, SOURCE_SIZE)
    if (code !== 0) throw new Error(`register(${sourceId}) → ${code}`)
    this.registered.set(sourceId, hash)
  }

  /** Render a cell at `size`; returns a COPY of the output RGBA (safe to retain). */
  render(cell: CorpusCell, size: number, pixelsDir = PIXELS): Uint8Array {
    this.ensureRegistered(cell.sourceId, pixelsDir)
    this.mem().set(encodeConfig(cell.config), this.cfgPtr)
    if (this.wasm.dm_session_set_config(this.session, this.cfgPtr, CONFIG_BYTES) !== 0) throw new Error(`set_config(${cell.file}) failed`)
    const code = this.wasm.dm_session_render(
      this.session,
      this.idPtr,
      this.idLen(cell.sourceId),
      cell.isShortcut ? 1 : 0,
      cell.showOriginal ? 1 : 0,
      size,
      cell.fieldSeed == null ? 0 : 1,
      cell.fieldSeed == null ? 0 : hexToInt(cell.fieldSeed),
      this.outPtr,
    )
    if (code !== 0) throw new Error(`render(${cell.file} @${size}) → ${code}`)
    const n = size * size * 4
    return this.mem().slice(this.outPtr, this.outPtr + n)
  }
}
