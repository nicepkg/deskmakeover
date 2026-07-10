import type { WallpaperGridInfoDto, ZoneDto } from '@/bridge/types'
import type { StringKey } from '@/lib/i18n'
import { t } from '@/lib/i18n'
import { makeZone } from '@/stores/wallpaper'
import { ACCENT_PALETTE } from '@/compositor/material'

// Curated zone-layout presets (spec 04 §2.3, ADR-0014 D6): human-designed
// layouts with semantic names + emoji + accents — choice, not prediction.
// Geometry is authored in GRID FRACTIONS (0..1 of columns/rows) so one preset
// fits every desktop; projection snaps to half cells and enforces 2×2 minimums.

interface PresetZoneSpec {
  titleKey: StringKey
  emoji: string
  accent: string
  /** Fractions of the usable grid (columns × rows). */
  x: number
  y: number
  w: number
  h: number
}

export interface ZonePreset {
  id: string
  nameKey: StringKey
  zones: PresetZoneSpec[]
}

// Vertically-stacked zones keep a 0.06 seam MATCHING the horizontal gap
// (owner 2026-07-09: 0.04 read as glued, 0.10 was too big — 0.06 is balanced).
// Pairs run 0.05→0.47 and 0.53→0.95.
export const ZONE_PRESETS: ZonePreset[] = [
  {
    id: 'workbench',
    nameKey: 'Preset_Workbench',
    zones: [
      { titleKey: 'Zone_TitleApps', emoji: '🚀', accent: ACCENT_PALETTE[0], x: 0.03, y: 0.05, w: 0.29, h: 0.9 },
      { titleKey: 'Zone_TitleWork', emoji: '📁', accent: ACCENT_PALETTE[1], x: 0.38, y: 0.05, w: 0.29, h: 0.42 },
      { titleKey: 'Zone_TitleDoing', emoji: '🔥', accent: ACCENT_PALETTE[3], x: 0.38, y: 0.53, w: 0.29, h: 0.42 },
    ],
  },
  {
    id: 'minimal-duo',
    nameKey: 'Preset_MinimalDuo',
    zones: [
      { titleKey: 'Zone_TitleApps', emoji: '🚀', accent: ACCENT_PALETTE[0], x: 0.03, y: 0.08, w: 0.26, h: 0.8 },
      { titleKey: 'Zone_TitleInbox', emoji: '📥', accent: ACCENT_PALETTE[2], x: 0.71, y: 0.08, w: 0.26, h: 0.8 },
    ],
  },
  {
    id: 'quadrants',
    nameKey: 'Preset_Quadrants',
    zones: [
      { titleKey: 'Zone_TitleWork', emoji: '💼', accent: ACCENT_PALETTE[1], x: 0.03, y: 0.05, w: 0.28, h: 0.42 },
      { titleKey: 'Zone_TitleDoing', emoji: '🔥', accent: ACCENT_PALETTE[3], x: 0.37, y: 0.05, w: 0.28, h: 0.42 },
      { titleKey: 'Zone_TitleApps', emoji: '🚀', accent: ACCENT_PALETTE[0], x: 0.03, y: 0.53, w: 0.28, h: 0.42 },
      { titleKey: 'Zone_TitleArchive', emoji: '🗃️', accent: ACCENT_PALETTE[5], x: 0.37, y: 0.53, w: 0.28, h: 0.42 },
    ],
  },
  {
    id: 'side-rail',
    nameKey: 'Preset_SideRail',
    zones: [
      { titleKey: 'Zone_TitleDoing', emoji: '🔥', accent: ACCENT_PALETTE[3], x: 0.03, y: 0.05, w: 0.22, h: 0.42 },
      { titleKey: 'Zone_TitleArchive', emoji: '🗃️', accent: ACCENT_PALETTE[5], x: 0.03, y: 0.53, w: 0.22, h: 0.42 },
    ],
  },
]

/** Project one preset onto a concrete grid. Alignment law (owner findings
 *  2026-07-09): origins land on x.5 half-lines and spans are WHOLE cells, so
 *  icon columns sit 0.5 cell from BOTH panel edges (Side rail had 0.5 left /
 *  0 right) and the title chip always has its 0.5-cell headroom above row 1
 *  (Minimal duo left 0.12 cell — the chip crowded the first icon row). */
export function projectPreset(preset: ZonePreset, grid: WallpaperGridInfoDto): ZoneDto[] {
  const cols = grid.columns
  const rows = grid.rows
  const rects = preset.zones.map((spec) => {
    const w = Math.min(Math.max(2, Math.round(spec.w * cols)), cols - 1)
    const h = Math.min(Math.max(2, Math.round(spec.h * rows)), rows - 1)
    const x = Math.min(Math.max(0.5, Math.floor(spec.x * cols) + 0.5), Math.max(0.5, cols - w - 0.5))
    const y = Math.min(Math.max(0.5, Math.floor(spec.y * rows) + 0.5), Math.max(0.5, rows - h - 0.5))
    return { x, y, w, h, spec }
  })

  // Enforce a >= 1-cell seam between adjacent zones. The fraction design leaves
  // a seam, but independent rounding (h rounds up, y floors) can collapse it to
  // ZERO at some grid sizes — the zones then touch (owner 2026-07-09: 还是粘在
  // 一起). We trim the UPPER/LEFT zone by whole cells so a designed neighbour is
  // always at least one cell away; zones designed far apart never trigger it.
  for (const a of rects) {
    for (const b of rects) {
      if (a === b) continue
      const xOverlap = a.x < b.x + b.w && b.x < a.x + a.w
      const yOverlap = a.y < b.y + b.h && b.y < a.y + a.h
      if (xOverlap && a.y < b.y && b.y - (a.y + a.h) < 1) {
        a.h = Math.max(2, Math.floor(b.y - a.y - 1))
      }
      if (yOverlap && a.x < b.x && b.x - (a.x + a.w) < 1) {
        a.w = Math.max(2, Math.floor(b.x - a.x - 1))
      }
    }
  }

  return rects
    .map((r) =>
      makeZone({
        cellX: r.x,
        cellY: r.y,
        cellsWide: Math.min(r.w, cols),
        cellsTall: Math.min(r.h, rows),
        title: t(r.spec.titleKey),
        emoji: r.spec.emoji,
        accent: r.spec.accent,
      }),
    )
    .filter((z) => z.cellsWide >= 2 && z.cellsTall >= 2)
}
