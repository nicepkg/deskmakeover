// M6 config ABI (TS encoder) — the mirror of `crates/dm-icon-wasm/src/abi.rs`.
// Packs a resolved `ConfigDto` into the fixed 24-byte record that
// `dm_session_set_config` decodes. Enum tags follow each axis's union
// declaration order in `bridge/types.ts`; the Rust decoder shares the exact
// numbering. A mismatch is not silent — it moves pixels and the 1487-cell
// byte-differential (`tests/icon-parity/m6/run.ts`) catches it.
//
// This module survives the M6 cutover (the frozen `icon-compositor/` pixel
// modules are deleted in P4; the WASM path lives here).

import type { ConfigDto } from '@/bridge/types'

/** Byte length of the packed config record (must equal `abi::CONFIG_BYTES`). */
export const CONFIG_BYTES = 24

// Tag tables — index = the on-wire u8. Order is load-bearing (shared with Rust).
const SHAPE = ['Apple', 'Circle', 'Samsung', 'None', 'Bookmark', 'Lemon', 'Tile', 'Teardrop', 'Diamond', 'Flower', 'Pebble', 'Folder']
const SUBJECT = ['Original', 'BlackWhite', 'Mono']
const MONO = ['Tonal', 'Flat']
const BAND = ['Vivid', 'Quiet']
const DISTINCTION = ['Mark', 'Keep', 'None']
const MARK = ['Glass', 'Shadow', 'Halo', 'Satin', 'Arc', 'Fold', 'Ring']
const FILTER = ['None', 'Gloss', 'Glass', 'Pixel', 'Sticker']
const FALLBACK = ['derived', 'white']

function tag(list: readonly string[], value: string, field: string): number {
  const i = list.indexOf(value)
  if (i < 0) throw new Error(`config-abi: bad ${field} "${value}"`)
  return i
}

/** `hexToInt` — the same 24-bit parse the frozen pixel path applies at every
 *  colour use site (`raster.ts` hexToInt). Kept here so the WASM path needs no
 *  frozen-module import. */
export function hexToInt(hex: string): number {
  return parseInt(hex.replace('#', ''), 16) & 0xffffff
}

/** Encode a resolved per-tile config into the 24-byte record. `size` and any
 *  non-pixel axes are intentionally ignored — the render size is a separate ABI
 *  argument and the per-type ladder is already folded upstream. */
export function encodeConfig(c: ConfigDto): Uint8Array {
  const b = new Uint8Array(CONFIG_BYTES)
  const dv = new DataView(b.buffer)
  b[0] = tag(SHAPE, c.shape, 'shape')
  b[1] = tag(SUBJECT, c.subject, 'subject')
  b[2] = tag(MONO, c.monoStyle, 'monoStyle')
  b[3] = tag(BAND, c.plateBand, 'plateBand')
  b[4] = tag(DISTINCTION, c.distinction, 'distinction')
  b[5] = tag(MARK, c.markStyle, 'markStyle')
  b[6] = tag(FILTER, c.filter, 'filter')
  b[7] = tag(FALLBACK, c.plateFallback, 'plateFallback')
  b[8] = c.shortcutShape == null ? 0xff : tag(SHAPE, c.shortcutShape, 'shortcutShape')
  b[9] = c.markColor == null ? 0 : 1
  b[10] = c.plateColor == null ? 0 : 1
  dv.setUint32(12, hexToInt(c.tint), true)
  if (c.markColor != null) dv.setUint32(16, hexToInt(c.markColor), true)
  if (c.plateColor != null) dv.setUint32(20, hexToInt(c.plateColor), true)
  return b
}
