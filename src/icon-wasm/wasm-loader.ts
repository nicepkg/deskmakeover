// M6 WASM loader + session wrapper. Instantiates dm-icon-wasm (raw extern "C",
// no wasm-bindgen) and drives the render_tile ABI: install the arrow, register
// sources (with a content hash), set the config, render tiles. One instance per
// Web Worker (a shard); the session renders `&mut self`, so a shard needs no
// lock. All buffers cross as raw linear-memory offsets; the memory view is
// re-acquired on every access because alloc/register can grow and detach it.

import type { ConfigDto } from '@/bridge/types'
import { CONFIG_BYTES, encodeConfig, hexToInt } from './config-abi'
import type { RenderOpts } from './protocol'

interface WasmExports {
  memory: WebAssembly.Memory
  dm_alloc(len: number): number
  dm_set_native_arrow(ptr: number, w: number, h: number): number
  dm_session_new(): number
  dm_session_free(s: number): void
  dm_session_register(s: number, idPtr: number, idLen: number, hash: bigint, srcPtr: number, w: number, h: number): number
  dm_session_set_config(s: number, cfgPtr: number, cfgLen: number): number
  dm_session_render(s: number, idPtr: number, idLen: number, isShortcut: number, showOriginal: number, size: number, hasFieldSeed: number, fieldSeed: number, out: number): number
  dm_session_seed_of(s: number, idPtr: number, idLen: number, out7: number): number
}

/** Served path — the build script copies the release `.wasm` into `public/`
 *  (vite copies `public/` wholesale into `dist/`, so this URL works in dev and
 *  prod). */
export const WASM_URL = '/dm_icon_wasm.wasm'

/** Preview sources and bake masters are 256²; `displaySize` clamps preview at
 *  256, so the reused output buffer never needs to exceed this. */
const MASTER_SIZE = 256

export async function loadWasm(url: string = WASM_URL): Promise<WasmExports> {
  // Prefer streaming compile (V8 code cache ≥128 KiB). It needs
  // `Content-Type: application/wasm`; fall back to arrayBuffer for hosts that
  // mis-serve the MIME (WebView2 custom protocol — perf doc open Q#3). Either
  // path compiles once and instantiates one instance per worker.
  try {
    const { instance } = await WebAssembly.instantiateStreaming(fetch(url), {})
    return instance.exports as unknown as WasmExports
  } catch {
    const bytes = await (await fetch(url)).arrayBuffer()
    const { instance } = await WebAssembly.instantiate(bytes, {})
    return instance.exports as unknown as WasmExports
  }
}

/** A long-lived render session over one WASM instance: register-once sources +
 *  per-source profile cache (both owned Rust-side), config set on change. */
export class WasmIconRenderer {
  private readonly wasm: WasmExports
  private readonly session: number
  private readonly enc = new TextEncoder()
  private readonly dec = new TextDecoder()
  private readonly registered = new Set<string>()
  private nextHash = 1n
  private lastConfigKey = ''
  // Scratch buffers, one per role, allocated once and reused (leaked for the
  // instance's life, per the register-once ABI contract).
  private readonly srcPtr: number
  private readonly outPtr: number
  private readonly cfgPtr: number
  private readonly idPtr: number
  private readonly idCap = 512

  // No parameter properties — `erasableSyntaxOnly` forbids them (TS is stripped,
  // not compiled, at runtime).
  private constructor(wasm: WasmExports, session: number) {
    this.wasm = wasm
    this.session = session
    this.srcPtr = wasm.dm_alloc(MASTER_SIZE * MASTER_SIZE * 4)
    this.outPtr = wasm.dm_alloc(MASTER_SIZE * MASTER_SIZE * 4)
    this.cfgPtr = wasm.dm_alloc(CONFIG_BYTES)
    this.idPtr = wasm.dm_alloc(this.idCap)
  }

  static async create(url?: string): Promise<WasmIconRenderer> {
    const wasm = await loadWasm(url)
    return new WasmIconRenderer(wasm, wasm.dm_session_new())
  }

  /** Instantiate from raw module bytes (no fetch) — used where the `.wasm` is
   *  already in hand (host tests, embedded builds). */
  static async fromBytes(bytes: BufferSource): Promise<WasmIconRenderer> {
    const { instance } = await WebAssembly.instantiate(bytes, {})
    const wasm = instance.exports as unknown as WasmExports
    return new WasmIconRenderer(wasm, wasm.dm_session_new())
  }

  /** Re-view linear memory — alloc/register may have grown and detached it. */
  private mem(): Uint8Array {
    return new Uint8Array(this.wasm.memory.buffer)
  }

  private writeId(id: string): number {
    return this.enc.encodeInto(id, this.mem().subarray(this.idPtr, this.idPtr + this.idCap)).written ?? 0
  }

  /** Install the genuine Win11 shortcut-arrow badge for this instance. Must run
   *  before any shortcut render or the drawn fallback breaks byte-parity. */
  setArrow(rgba: Uint8ClampedArray | Uint8Array, w: number, h: number): void {
    const ptr = this.wasm.dm_alloc(w * h * 4)
    this.mem().set(rgba, ptr)
    if (this.wasm.dm_set_native_arrow(ptr, w, h) !== 0) throw new Error('dm_set_native_arrow failed')
  }

  hasSource(id: string): boolean {
    return this.registered.has(id)
  }

  /** Register a 256² straight-alpha RGBA source; returns its hue seed hex (the
   *  subject rim colour), or null for the no-hue tail. */
  registerSource(id: string, rgba: Uint8ClampedArray | Uint8Array): string | null {
    this.mem().set(rgba, this.srcPtr)
    const hash = this.nextHash++
    const idLen = this.writeId(id)
    const code = this.wasm.dm_session_register(this.session, this.idPtr, idLen, hash, this.srcPtr, MASTER_SIZE, MASTER_SIZE)
    if (code !== 0) throw new Error(`dm_session_register(${id}) → ${code}`)
    this.registered.add(id)
    return this.seedOf(id)
  }

  /** The decode-time hue seed for a registered source (`#RRGGBB`), or null. */
  seedOf(id: string): string | null {
    const idLen = this.writeId(id)
    if (this.wasm.dm_session_seed_of(this.session, this.idPtr, idLen, this.outPtr) !== 1) return null
    return this.dec.decode(this.mem().subarray(this.outPtr, this.outPtr + 7))
  }

  /** register-once per settings change: only re-marshal when the value changes. */
  private ensureConfig(config: ConfigDto): void {
    const key = JSON.stringify(config)
    if (key === this.lastConfigKey) return
    this.mem().set(encodeConfig(config), this.cfgPtr)
    if (this.wasm.dm_session_set_config(this.session, this.cfgPtr, CONFIG_BYTES) !== 0) throw new Error('dm_session_set_config failed')
    this.lastConfigKey = key
  }

  /** Render a registered source. Returns straight-alpha RGBA (`size²·4`) as a
   *  COPY detached from linear memory (safe to keep or transfer), or null when
   *  the source is not registered / no config is set (caller retries). */
  render(id: string, config: ConfigDto, isShortcut: boolean, showOriginal: boolean, size: number, opts?: RenderOpts): Uint8ClampedArray<ArrayBuffer> | null {
    this.ensureConfig(config)
    const idLen = this.writeId(id)
    const fieldSeed = opts?.fieldSeed ?? null
    const code = this.wasm.dm_session_render(
      this.session,
      this.idPtr,
      idLen,
      isShortcut ? 1 : 0,
      showOriginal ? 1 : 0,
      size,
      fieldSeed == null ? 0 : 1,
      fieldSeed == null ? 0 : hexToInt(fieldSeed),
      this.outPtr,
    )
    if (code === 4) return null // unknown id / no look set — caller retries after sourceReady
    if (code !== 0) throw new Error(`dm_session_render(${id}) → ${code}`)
    const len = size * size * 4
    // `.slice()` copies into a fresh ArrayBuffer, detaching from (possibly
    // shared) linear memory and giving the concrete `Uint8ClampedArray<ArrayBuffer>`
    // ImageData/putImageData require.
    return new Uint8ClampedArray(this.wasm.memory.buffer, this.outPtr, len).slice()
  }

  dispose(): void {
    this.wasm.dm_session_free(this.session)
    this.registered.clear()
  }
}
