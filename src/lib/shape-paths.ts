import type { IconShape } from '@/bridge/types'
import { applePathD, curvedShapePathD, smoothShapePathD } from '@/icon-compositor/shapes'

// CSS `clip-path` values for the live shape swatch on shape chips (spec 02
// §Geometry, v3 keyline: 20px solid-block extent). Every silhouette comes from
// the engine's canonical authoring (icon-compositor/shapes.ts — the Figma
// corner-smoothing engine + authored cubics), so the swatch a user picks IS
// the mask that bakes — chip and tile cannot drift.
//
// INK INSET: silhouettes are drawn 1px inside the swatch box so curves that
// kiss the box edge keep their anti-aliasing. Masks always use inset 0; this
// is chip presentation only.
//
// Note on units: CSS `clip-path: path()` coordinates are absolute CSS pixels — a
// path() does NOT scale to the element the way an SVG objectBoundingBox clip does.
// So every path-based silhouette is emitted at the swatch's pixel size (SWATCH),
// and SWATCH MUST equal ShapeSwatch's rendered box or the clip anchors top-left
// and the glyph sits off-centre.

const SWATCH = 20 // = ShapeSwatch box (chip-preview keyline: solids draw 20px)
const INK_INSET = 1 // px of breathing room inside the swatch box

/** A `clip-path` value clipping the 20px shape swatch to `shape`'s silhouette. */
export function clipPathFor(shape: IconShape): string {
  switch (shape) {
    case 'Apple':
      return `path('${applePathD(SWATCH, INK_INSET)}')`
    case 'Circle':
      return `circle(${SWATCH / 2 - INK_INSET}px)`
    case 'None':
      return 'inset(0)'
    case 'Tile':
    case 'Teardrop':
    case 'Bookmark':
    case 'Lemon':
    case 'Diamond':
    case 'Folder':
    case 'File':
      return `path('${smoothShapePathD(shape, SWATCH, INK_INSET)}')`
    case 'Samsung':
    case 'Flower':
    case 'Pebble':
      return `path('${curvedShapePathD(shape, SWATCH, INK_INSET)}')`
    default: {
      const _exhaustive: never = shape
      return _exhaustive
    }
  }
}
