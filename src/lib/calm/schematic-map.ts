// 清爽 schematic system — compile-time scene/region map (viz panel 2026-07-13).
// Every control renders a "mini screen" schematic (104×64 frame); this file says
// WHICH scene it uses and WHERE its coral highlight sits. Scenes are deliberately
// abstract wireframes (rounded rects + placeholder ink) — never fake screenshots.

import type { CalmControlId } from './catalog'

export const FRAME_W = 104
export const FRAME_H = 64

export type CalmScene =
  | 'taskbar' // the taskbar as the hero of the frame (4 controls share it)
  | 'start' // start panel open over the taskbar
  | 'searchPanel' // search flyout with the highlights band
  | 'notif' // notification stack, right column
  | 'settings' // the Settings app window
  | 'systemFull' // full-screen OS takeover (welcome / finish-setup)
  | 'explorer' // File Explorer window with a promo banner
  | 'widgets' // widgets board sliding from the left
  | 'lock' // lock screen with the status card

export interface SchematicRegion {
  x: number
  y: number
  w: number
  h: number
  rx?: number
}

export interface SchematicSpec {
  scene: CalmScene
  region: SchematicRegion
}

/** Per-control scene + coral highlight region (coordinates in the 104×64 frame).
 *  Regions verified against real Windows 11 layouts (win11-layout-scout
 *  2026-07-13): news in the search flyout's RIGHT column, weather/widgets entry
 *  at the taskbar's FAR LEFT, lock-screen status cards TOP-LEFT, Settings promo
 *  inside the card grid, Explorer banner under the address bar. */
export const SCHEMATICS: Record<CalmControlId, SchematicSpec> = {
  'start.recommendations': { scene: 'start', region: { x: 25, y: 30.5, w: 54, h: 16.5, rx: 3 } },
  'taskbar.search': { scene: 'taskbar', region: { x: 27.5, y: 45.5, w: 30, h: 13, rx: 5 } },
  'taskbar.taskview': { scene: 'taskbar', region: { x: 56.5, y: 45.5, w: 12, h: 13, rx: 4 } },
  'search.highlights': { scene: 'searchPanel', region: { x: 39, y: 16, w: 53, h: 36, rx: 3 } },
  'notifications.suggestions': { scene: 'notif', region: { x: 58, y: 20, w: 38, h: 13, rx: 3 } },
  'notifications.welcome': { scene: 'systemFull', region: { x: 3, y: 3, w: 98, h: 58, rx: 6 } },
  'notifications.finishSetup': { scene: 'systemFull', region: { x: 3, y: 3, w: 98, h: 58, rx: 6 } },
  'settings.suggestions': { scene: 'settings', region: { x: 63, y: 22, w: 33, h: 31, rx: 3 } },
  'explorer.syncNotifications': { scene: 'explorer', region: { x: 12, y: 11, w: 61, h: 27, rx: 3 } },
  'widgets.feed': { scene: 'widgets', region: { x: 26, y: 7, w: 38, h: 44, rx: 3 } },
  'taskbar.widgetsButton': { scene: 'taskbar', region: { x: 4.5, y: 45.5, w: 17, h: 13, rx: 4 } },
  'lockscreen.status': { scene: 'lock', region: { x: 24, y: 29, w: 56, h: 14, rx: 3 } },
  // tray region hugs the OVERFLOWABLE app icons (x81..89.6) plus the caret
  // entrance edge — the clock/status pills (x≥94) are NOT the operation area.
  'tray.entries': { scene: 'taskbar', region: { x: 80, y: 45.5, w: 13.5, h: 13, rx: 4 } },
}
