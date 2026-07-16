// 清爽 module — the v1 control catalog (spec 08 §8, ADR-0023 D5).
// The catalog is COMPILE-TIME product truth: which controls exist in the UI, their
// tier, surface, copy keys, and admission flags. Which of them are writable on THIS
// machine is RUNTIME truth and comes from the backend probe — never from here.
// Registry recipes live in Rust (Wave 1+); the frontend never sees a registry path.

import type { StringKey } from '@/lib/i18n'
import type { CalmRowState } from './states'

export type CalmTier = 'automatic' | 'advanced' | 'guided'
// The WHERE axis (review 2026-07-13): every row carries its Windows surface so the
// UI can anchor "关的是哪里" with a glyph + place label. 'system' = full-screen OS
// moments (post-update welcome / finish-setup) that are not the notification center.
export type CalmSurface =
  | 'start' | 'search' | 'taskbar' | 'notifications' | 'settings' | 'system' | 'explorer' | 'widgets' | 'lockscreen'

export type CalmControlId =
  | 'start.recommendations'
  | 'taskbar.search'
  | 'taskbar.taskview'
  | 'search.highlights'
  | 'notifications.suggestions'
  | 'notifications.welcome'
  | 'notifications.finishSetup'
  | 'settings.suggestions'
  | 'explorer.syncNotifications'
  | 'widgets.feed'
  | 'taskbar.widgetsButton'
  | 'lockscreen.status'
  | 'tray.entries'

export interface CalmControl {
  id: CalmControlId
  surface: CalmSurface
  tier: CalmTier
  labelKey: StringKey
  /** One-line honest description shown on the row face. */
  descKey: StringKey
  /** Disclosed legitimate-content collateral (spec 08 admission rule d) — on the row face. */
  collateralKey?: StringKey
  /** True → in the default one-click package (admission rule a-d all hold). */
  inDefaultPackage: boolean
  /** Starter write slice (ADR-0023 D6) — the capability-gated v1 candidates. */
  starterSlice: boolean
  /** guided rows: can the app re-probe a readable off/on state after the walk? */
  readableState?: boolean
  /** The documented route's on-row instruction copy. Present on guided rows AND on fail-closed
   *  automatic rows whose official settings page is known — its presence is what makes an
   *  uncertified automatic row a WALK (guided group) instead of a dead held row (see `groupOf`).
   *  The actual `ms-settings:` URI lives in the Rust catalog only; `open_route` resolves it by id. */
  routeKey?: StringKey
}

// ORDER matters: within a group the UI renders catalog order; widgets.feed leads the
// guided group by design (the opening act — ADR-0023 D3).
export const CALM_CATALOG: readonly CalmControl[] = [
  // ---- automatic, starter write slice (v1 capability-gated candidates) ----
  {
    id: 'start.recommendations',
    surface: 'start',
    tier: 'automatic',
    labelKey: 'Calm_StartRecs',
    descKey: 'Calm_StartRecs_Desc',
    inDefaultPackage: true,
    starterSlice: true,
    routeKey: 'Calm_StartRecs_Route',
  },
  {
    id: 'taskbar.search',
    surface: 'taskbar',
    tier: 'automatic',
    labelKey: 'Calm_TaskbarSearch',
    descKey: 'Calm_TaskbarSearch_Desc',
    inDefaultPackage: true,
    starterSlice: true,
    routeKey: 'Calm_TaskbarSearch_Route',
  },
  {
    id: 'taskbar.taskview',
    surface: 'taskbar',
    tier: 'automatic',
    labelKey: 'Calm_TaskView',
    descKey: 'Calm_TaskView_Desc',
    inDefaultPackage: true,
    starterSlice: true,
    routeKey: 'Calm_TaskView_Route',
  },
  // ---- automatic, next-in-line (enter the package as lab rows land; until then the
  //      probe reports them unsupported and they sit honestly in group 3) ----
  {
    id: 'search.highlights',
    surface: 'search',
    tier: 'automatic',
    labelKey: 'Calm_SearchHighlights',
    descKey: 'Calm_SearchHighlights_Desc',
    inDefaultPackage: true,
    starterSlice: false,
    routeKey: 'Calm_SearchHighlights_Route',
  },
  {
    id: 'notifications.suggestions',
    surface: 'notifications',
    tier: 'automatic',
    labelKey: 'Calm_NotifSuggestions',
    descKey: 'Calm_NotifSuggestions_Desc',
    inDefaultPackage: true,
    starterSlice: false,
    routeKey: 'Calm_NotifSuggestions_Route',
  },
  {
    id: 'notifications.welcome',
    surface: 'system',
    tier: 'automatic',
    labelKey: 'Calm_Welcome',
    descKey: 'Calm_Welcome_Desc',
    inDefaultPackage: true,
    starterSlice: false,
    routeKey: 'Calm_Welcome_Route',
  },
  {
    id: 'notifications.finishSetup',
    surface: 'system',
    tier: 'automatic',
    labelKey: 'Calm_FinishSetup',
    descKey: 'Calm_FinishSetup_Desc',
    inDefaultPackage: true,
    starterSlice: false,
    routeKey: 'Calm_FinishSetup_Route',
  },
  {
    id: 'settings.suggestions',
    surface: 'settings',
    tier: 'automatic',
    labelKey: 'Calm_SettingsSuggestions',
    descKey: 'Calm_SettingsSuggestions_Desc',
    inDefaultPackage: true,
    starterSlice: false,
    routeKey: 'Calm_SettingsSuggestions_Route',
  },
  {
    id: 'explorer.syncNotifications',
    surface: 'explorer',
    tier: 'automatic',
    labelKey: 'Calm_SyncNotif',
    descKey: 'Calm_SyncNotif_Desc',
    collateralKey: 'Calm_SyncNotif_Collateral',
    inDefaultPackage: true,
    starterSlice: false,
  },
  // ---- guided (no stable setter — we walk the user there; NEVER toggles) ----
  {
    id: 'widgets.feed',
    surface: 'widgets',
    tier: 'guided',
    labelKey: 'Calm_WidgetsFeed',
    descKey: 'Calm_WidgetsFeed_Desc',
    inDefaultPackage: false,
    starterSlice: false,
    readableState: false,
    routeKey: 'Calm_WidgetsFeed_Route',
  },
  {
    id: 'taskbar.widgetsButton',
    surface: 'taskbar',
    tier: 'guided',
    labelKey: 'Calm_WidgetsButton',
    descKey: 'Calm_WidgetsButton_Desc',
    inDefaultPackage: false,
    starterSlice: false,
    readableState: true, // TaskbarDa is READ-observational only — never written (UCPD)
    routeKey: 'Calm_WidgetsButton_Route',
  },
  {
    id: 'lockscreen.status',
    surface: 'lockscreen',
    tier: 'guided',
    labelKey: 'Calm_LockStatus',
    descKey: 'Calm_LockStatus_Desc',
    inDefaultPackage: false,
    starterSlice: false,
    readableState: false,
    routeKey: 'Calm_LockStatus_Route',
  },
  {
    id: 'tray.entries',
    surface: 'taskbar',
    tier: 'guided',
    labelKey: 'Calm_Tray',
    descKey: 'Calm_Tray_Desc',
    inDefaultPackage: false,
    starterSlice: false,
    readableState: false,
    routeKey: 'Calm_Tray_Route',
  },
]

export type CalmGroup = 'oneClick' | 'guided' | 'held'

/** Which of the three page groups a row renders in, given its probed state.
 *  A fail-closed automatic row (`unsupported`) whose official page we know is a WALK (guided
 *  group 「带你去系统里关的」, ADR-0023 D2), never a dead held row. Policy-`managed` rows stay
 *  held (the Settings toggle is greyed, so walking there is futile), and an uncertified row with
 *  no known page (`explorer.syncNotifications`) has nowhere to walk, so it stays honestly held. */
export function groupOf(control: CalmControl, state: CalmRowState): CalmGroup {
  if (control.tier === 'guided') return 'guided'
  if (state === 'managed') return 'held'
  if (state === 'unsupported') return control.routeKey ? 'guided' : 'held'
  return 'oneClick'
}

export function controlById(id: CalmControlId): CalmControl {
  const hit = CALM_CATALOG.find((c) => c.id === id)
  if (!hit) throw new Error(`calm: unknown control ${id}`)
  return hit
}
