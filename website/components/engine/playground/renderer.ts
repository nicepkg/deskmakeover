/**
 * Self-contained port of the app's WASM session wrapper for the /engine/
 * playground (provenance: src/icon-wasm/{wasm-loader,config-abi}.ts in the
 * main repo — kept dependency-free here so the website builds without the app
 * workspace). Drives the raw extern "C" ABI of crates/dm-icon-wasm: register
 * 256² sources, set the packed 24-byte config, render tiles.
 */

/** Served from public/engine/; scripts/sync-wasm.mjs keeps it current. */
export const WASM_URL = "/engine/dm_icon_wasm.wasm";

/** Sources and the reused output buffer are 256² (the ABI's MAX_RENDER_SIZE). */
export const MASTER_SIZE = 256;

export const CONFIG_BYTES = 24;

// Tag tables — index = the on-wire u8. Order is load-bearing and shared with
// the Rust decoder (crates/dm-icon-wasm/src/abi.rs); mirror of config-abi.ts.
const SHAPE = ["Apple", "Circle", "Samsung", "None", "Bookmark", "Lemon", "Tile", "Teardrop", "Diamond", "Flower", "Pebble", "Folder", "File"];
const SUBJECT = ["Original", "BlackWhite", "Mono"];
const MONO = ["Tonal", "Flat"];
const BAND = ["Vivid", "Quiet"];
const DISTINCTION = ["Mark", "Keep", "None"];
const MARK = ["Glass", "Shadow", "Halo", "Satin", "Arc", "Fold", "Ring", "Comet"];
const FILTER = ["None", "Gloss", "Glass", "Pixel", "Sticker"];
const FALLBACK = ["derived", "white"];

export interface PlaygroundConfig {
  shape: string;
  subject: string;
  monoStyle: string;
  plateBand: string;
  distinction: string;
  markStyle: string;
  filter: string;
  plateFallback: string;
  shortcutShape: string | null;
  markColor: string | null;
  plateColor: string | null;
  autoSeparation: boolean;
  tint: string;
}

function tag(list: readonly string[], value: string, field: string): number {
  const i = list.indexOf(value);
  if (i < 0) throw new Error(`engine-abi: bad ${field} "${value}"`);
  return i;
}

export function hexToInt(hex: string): number {
  return parseInt(hex.replace("#", ""), 16) & 0xffffff;
}

/** Pack a config into the fixed 24-byte record `dm_session_set_config` decodes. */
export function encodeConfig(c: PlaygroundConfig): Uint8Array {
  const b = new Uint8Array(CONFIG_BYTES);
  const dv = new DataView(b.buffer);
  b[0] = tag(SHAPE, c.shape, "shape");
  b[1] = tag(SUBJECT, c.subject, "subject");
  b[2] = tag(MONO, c.monoStyle, "monoStyle");
  b[3] = tag(BAND, c.plateBand, "plateBand");
  b[4] = tag(DISTINCTION, c.distinction, "distinction");
  b[5] = tag(MARK, c.markStyle, "markStyle");
  b[6] = tag(FILTER, c.filter, "filter");
  b[7] = tag(FALLBACK, c.plateFallback, "plateFallback");
  b[8] = c.shortcutShape == null ? 0xff : tag(SHAPE, c.shortcutShape, "shortcutShape");
  b[9] = c.markColor == null ? 0 : 1;
  b[10] = c.plateColor == null ? 0 : 1;
  b[11] = c.autoSeparation ? 1 : 0;
  dv.setUint32(12, hexToInt(c.tint), true);
  if (c.markColor != null) dv.setUint32(16, hexToInt(c.markColor), true);
  if (c.plateColor != null) dv.setUint32(20, hexToInt(c.plateColor), true);
  return b;
}

interface WasmExports {
  memory: WebAssembly.Memory;
  dm_alloc(len: number): number;
  dm_session_new(): number;
  dm_session_free(s: number): void;
  dm_session_register(s: number, idPtr: number, idLen: number, hash: bigint, srcPtr: number, w: number, h: number): number;
  dm_session_set_config(s: number, cfgPtr: number, cfgLen: number): number;
  dm_session_render(s: number, idPtr: number, idLen: number, isShortcut: number, showOriginal: number, size: number, hasFieldSeed: number, fieldSeed: number, out: number): number;
}

async function loadWasm(url: string): Promise<WasmExports> {
  // Streaming compile when the host serves application/wasm; arrayBuffer
  // fallback otherwise. Either path compiles once per page.
  try {
    const { instance } = await WebAssembly.instantiateStreaming(fetch(url), {});
    return instance.exports as unknown as WasmExports;
  } catch {
    const bytes = await (await fetch(url)).arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return instance.exports as unknown as WasmExports;
  }
}

/** One long-lived render session: register-once sources + config-on-change. */
export class EngineRenderer {
  private readonly wasm: WasmExports;
  private readonly session: number;
  private readonly enc = new TextEncoder();
  // BigInt() call, not a literal — tsconfig targets ES2017 for typecheck
  private nextHash = BigInt(1);
  private lastConfigKey = "";
  // scratch buffers, one per role, allocated once and reused
  private readonly srcPtr: number;
  private readonly outPtr: number;
  private readonly cfgPtr: number;
  private readonly idPtr: number;
  private readonly idCap = 64;

  private constructor(wasm: WasmExports, session: number) {
    this.wasm = wasm;
    this.session = session;
    this.srcPtr = wasm.dm_alloc(MASTER_SIZE * MASTER_SIZE * 4);
    this.outPtr = wasm.dm_alloc(MASTER_SIZE * MASTER_SIZE * 4);
    this.cfgPtr = wasm.dm_alloc(CONFIG_BYTES);
    this.idPtr = wasm.dm_alloc(this.idCap);
  }

  static async create(url: string = WASM_URL): Promise<EngineRenderer> {
    const wasm = await loadWasm(url);
    return new EngineRenderer(wasm, wasm.dm_session_new());
  }

  /** Re-view linear memory — alloc/register may have grown and detached it. */
  private mem(): Uint8Array {
    return new Uint8Array(this.wasm.memory.buffer);
  }

  private writeId(id: string): number {
    return this.enc.encodeInto(id, this.mem().subarray(this.idPtr, this.idPtr + this.idCap)).written ?? 0;
  }

  /** Register (or replace) a 256² straight-alpha RGBA source under an id. */
  registerSource(id: string, rgba: Uint8ClampedArray | Uint8Array): void {
    this.mem().set(rgba, this.srcPtr);
    const hash = this.nextHash++;
    const idLen = this.writeId(id);
    const code = this.wasm.dm_session_register(this.session, this.idPtr, idLen, hash, this.srcPtr, MASTER_SIZE, MASTER_SIZE);
    if (code !== 0) throw new Error(`dm_session_register(${id}) -> ${code}`);
  }

  private ensureConfig(config: PlaygroundConfig): void {
    const key = JSON.stringify(config);
    if (key === this.lastConfigKey) return;
    this.mem().set(encodeConfig(config), this.cfgPtr);
    if (this.wasm.dm_session_set_config(this.session, this.cfgPtr, CONFIG_BYTES) !== 0) {
      throw new Error("dm_session_set_config failed");
    }
    this.lastConfigKey = key;
  }

  /** Render a registered source; returns a detached RGBA copy (size²·4). */
  render(id: string, config: PlaygroundConfig, showOriginal: boolean, size: number): Uint8ClampedArray<ArrayBuffer> | null {
    if (!Number.isInteger(size) || size < 1 || size > MASTER_SIZE) {
      throw new Error(`render size ${size} out of range (1..=${MASTER_SIZE})`);
    }
    this.ensureConfig(config);
    const idLen = this.writeId(id);
    const code = this.wasm.dm_session_render(this.session, this.idPtr, idLen, 0, showOriginal ? 1 : 0, size, 0, 0, this.outPtr);
    if (code === 4) return null; // unknown id — caller registers first
    if (code !== 0) throw new Error(`dm_session_render(${id}) -> ${code}`);
    const len = size * size * 4;
    // slice() copies out of linear memory into a fresh ArrayBuffer for ImageData
    return new Uint8ClampedArray(this.wasm.memory.buffer, this.outPtr, len).slice();
  }

  dispose(): void {
    this.wasm.dm_session_free(this.session);
  }
}
