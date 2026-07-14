import type { IconShape } from '@/bridge/types'
import type { StringKey } from '@/lib/i18n'
import { monoRamp } from '@/icon-wasm/mono-ramp'
import { hexToInt } from '@/icon-wasm/config-abi'

// Shared axis option catalogs — one source for the global Shape section, the
// per-type accordion and the shortcut uniform-shape picker (ADR-0017; keeping
// them in the panel file made the participation component a circular import).

// Owner law: every axis's 「无」 sits FIRST, wearing the shared slash-circle glyph.
export const CURATED_SHAPES: { value: IconShape; key: StringKey }[] = [
  { value: 'None', key: 'Shape_None' }, { value: 'Apple', key: 'Shape_Apple' },
  { value: 'Circle', key: 'Shape_Circle' }, { value: 'Samsung', key: 'Shape_Samsung' },
  { value: 'Tile', key: 'Shape_Tile' }, { value: 'Teardrop', key: 'Shape_Teardrop' },
]
export const MORE_SHAPES: { value: IconShape; key: StringKey }[] = [
  { value: 'Folder', key: 'Shape_Folder' }, { value: 'File', key: 'Shape_File' },
  { value: 'Bookmark', key: 'Shape_Bookmark' }, { value: 'Lemon', key: 'Shape_Lemon' },
  { value: 'Diamond', key: 'Shape_Diamond' }, { value: 'Flower', key: 'Shape_Flower' },
  { value: 'Pebble', key: 'Shape_Pebble' },
]
export const ALL_SHAPES = [...CURATED_SHAPES, ...MORE_SHAPES]

/** The bounded per-type plate palette (ADR-0017 D3): six LOW-SATURATION
 *  boards — enough to group a type, never loud enough to fight the subjects
 *  (UI-seat guardrail; a fixed-plate type exits the hue-spread pool). */
export const TYPE_PLATE_SWATCHES: string[] = [
  '#65470D', // 深金 deep gold — the Folder band's factory plate
  '#E9E2D4', // warm sand
  '#DDE6F2', // cool mist blue
  '#DFE8DC', // sage
  '#EEE0E2', // rose fog
  '#E4DFEE', // cool slate grey (OKLab C 0.021 < 0.04 neutral line - reads grey, not violet)
  '#E7E7E5', // neutral stone
]

/** The Auto plate a mono tint produces (ramp light end) — the outer ring of
 *  the concentric mono pair-dot, shared by the global Colour row and the
 *  type accordion. */
export function paleOf(tint: string): string {
  const pale = monoRamp(1, hexToInt(tint))
  return `#${((pale.r << 16) | (pale.g << 8) | pale.b).toString(16).padStart(6, '0')}`
}
