// Bridge DTOs — mirrors the host contracts (Rust dm-contracts on Tauri; the old
// C# tree was removed 2026-07-14). Bump together with the host (BridgeSchema.Version) so drift
// fails loudly at startup.
// Schema 6 (owner ruling D1, 2026-07-12): the wallpaper bridge contract SHRINKS to
// thin platform I/O. Rust does ONLY screen enumeration + get/set wallpaper +
// capture/restore snapshot; reconcile, per-monitor draft-look persistence and
// WallpaperStateDto assembly are ALL frontend. So `wallpaper.getState`→`getScreens`
// (thin ScreenInfoDto[] + globals, NO looks/grids); `setLook` leaves the bridge
// (frontend localStorage `wallpaper.look.v2::<device-path>`, like `dm.icons.bareLook`);
// `applyBaked`/`restore` return a THIN WallpaperResultDto (ok/toast/hasBackup — NO
// state; the store re-fetches getScreens and re-assembles). MonitorLookDto /
// WallpaperStateDto remain, but as FRONTEND-ASSEMBLED store shapes, not bridge DTOs.
// Schema 7 (owner ruling D1, 2026-07-12): the ICON bridge contract SHRINKS like wallpaper's did.
// Rust does scan / package + apply / restore / persist ②③ and returns THIN data (IconScanDto raw
// items · IconPersistedDto the ②③+native bits · IconOpResultDto thin op results); the FRONTEND
// assembles IconsStateDto (presets/palette/swatches/grid/activePresetId) via lib/icons-assemble,
// and `icons.setLook` LEAVES the bridge (the draft is frontend session state, resumed from ② on
// relaunch). IconsStateDto / ScanResultDto / IconsOpResultDto are now FRONTEND store shapes.
// Schema 8 (Wave 1): the 清爽 (calm-Windows) settings decision core lands as real Rust commands
// behind dm-contracts (tweaks* verbs → CalmProbeRowDto / CalmApplyRowDto / CalmRestoreRowDto /
// CalmGuidedProbeDto). The frontend CalmBackend port maps 1:1 onto these; the store never learns
// mock vs real Rust.
// Schema 9 (spec 09, 2026-07-15): preset packages + the user preset library. presets.* verbs →
// PresetPackageReadDto / PresetEntryDto / PresetSaveDto (dm-contracts); library thumbnails ride
// the scoped dmpreset:// protocol. Rust owns structure/security; payload SEMANTICS stay with the
// ONE TS validator (lib/icon-look), so readPackage is PURE and save is the only library writer.
export const BRIDGE_SCHEMA_VERSION = 9

// Preset package DTOs (schema 9) are consumed straight from the generated
// contract — they carry no frontend-only fields, so a hand mirror would be
// pure drift surface. Imported for the BridgeMethods map below AND re-exported.
import type {
  PresetEntryDto,
  PresetMetaDto,
  PresetPackageReadDto,
  PresetReadEntryDto,
  PresetSaveDto,
} from './generated'

export type { PresetEntryDto, PresetMetaDto, PresetPackageReadDto, PresetReadEntryDto, PresetSaveDto }

export interface SettingsDto {
  theme: 'System' | 'Dark' | 'Light'
  language: 'System' | 'zh-Hans' | 'en'
  keepNewIconsStyled: boolean
  wallpaperCoachShown: boolean
}

export type SettingsPatch = Partial<SettingsDto>

export interface ChangelogEntryDto {
  version: string
  items: string[]
}

export interface AppLinksDto {
  repo: string
  releases: string
  issues: string
  homepage: string
  githubProfile: string
  x: string
  bilibili: string
  douyin: string
  /** Support mailbox for users who cannot reach GitHub (optional until F8 hosts send it). */
  email?: string
}

/** Environment snapshot for diagnostics reports (mirrors Host DiagnosticsService, F8). */
export interface SystemInfoDto {
  osVersion: string
  webview2Version: string
  arch: string
  /** Recent host-side error lines, already formatted (empty until F8). */
  hostLogTail: string[]
}

export interface AppInfoDto {
  schemaVersion: number
  version: string
  productNameZh: string
  productNameEn: string
  effectiveDark: boolean
  links: AppLinksDto
  changelogZh: ChangelogEntryDto[]
  changelogEn: ChangelogEntryDto[]
}

/** Host → web event topics. */
export interface BridgeEvents {
  'window-state': { maximized: boolean }
  'settings-changed': SettingsDto
  'os-theme-changed': { dark: boolean }
  toast: { text: string; tone?: 'info' | 'success' | 'warn' }
  /** Host-side captured errors stream into the web error log (F8 wires the host end). */
  'host-error': { message: string; stack?: string }
}

// ---- Icons module (mirrors Host/Bridge/IconsContracts.cs) ----

// Owner-curated catalog (2026-07-09): Google/Brave/Squircle/Blob/Rectellipse/
// Hexagon culled; Diamond (Figma-smoothed polygon) and Flower + Pebble
// (maskable.app OEM masks) added. C# enum follows in the Windows batch.
export type IconShape =
  | 'Apple' | 'Circle' | 'Samsung' | 'None' | 'Bookmark'
  | 'Lemon' | 'Tile' | 'Teardrop' | 'Diamond' | 'Flower' | 'Pebble'
  /** Folder-tab silhouette (ADR-0017): the Folder bucket's factory shape. */
  | 'Folder'
  /** Dog-eared document (spec 02 V2, owner-disposed 2026-07-15): top-right 45°
   *  cut c=30, outer corners r12, cut-edge endpoints r6 — built for the File
   *  bucket, available everywhere. */
  | 'File'
/** SUBJECT axis (ADR-0018): how the artwork renders. Field is GONE as a
 *  mode — 满彩 is now the preset coordinate (Original × 随图标 plate). */
export type Subject = 'Original' | 'BlackWhite' | 'Mono'
export type Distinction = 'Mark' | 'Keep' | 'None'
export type MarkStyle = 'Glass' | 'Shadow' | 'Halo' | 'Satin' | 'Arc' | 'Fold' | 'Ring' | 'Comet'
export type IconSizeMode = 'Small' | 'Mid' | 'Big'
export type FilterStyle = 'None' | 'Gloss' | 'Glass' | 'Pixel' | 'Sticker'

/** Mono depth (owner feature 2026-07-09): Tonal = single-hue ramp (classic);
 *  Flat = 极致单色 — the segmented SUBJECT in one flat colour on a flat plate. */
export type MonoStyle = 'Tonal' | 'Flat'

/** Derived-plate depth band (was FieldBand): Vivid = saturated default;
 *  Quiet = the pastel envelope behind the 安静 preset. */
export type PlateBand = 'Vivid' | 'Quiet'

export interface ConfigDto {
  shape: IconShape
  /** 主体 axis (ADR-0018): Original / BlackWhite / Mono. */
  subject: Subject
  tint: string
  monoStyle: MonoStyle
  /** 底板 depth band — meaningful for subject Original × plateColor null. */
  plateBand: PlateBand
  /** Uniform shortcut shape (ADR-0017 D5, default null = off): when set,
   *  every shortcut renders this shape regardless of its type's shape; the
   *  mark badge is unaffected. Opt-in — the badge alone marks shortcuts. */
  shortcutShape: IconShape | null
  distinction: Distinction
  markStyle: MarkStyle
  markColor: string | null
  size: IconSizeMode
  filter: FilterStyle
  /** 底板 axis stop, derived from the value (ADR-0018): null = 随图标
   *  (derived); '#FFFFFF' = 白; other hex = fixed plate. Active for EVERY
   *  subject (the old per-mode semantics chart is dead). */
  plateColor: string | null
  /** null-plate fallback policy: 'derived' = 满彩 lane (themed plates for
   *  bare icons); 'white' = 本色 lane (own boards anchored 1:1, bare icons
   *  white, classic pipeline — no silhouette shadows). */
  plateFallback: 'derived' | 'white'
}

export interface GridDto {
  screenWidth: number
  screenHeight: number
  taskbarHeight: number
  iconPx: number
  cellWidth: number
  cellHeight: number
  inset: number
  labelFontPx: number
}

/** DesktopItem.Kind mirror — taxonomy is DATA to the web (spec 06 §6).
 *  SystemIcon = the HKCU CLSID DefaultIcon family (This PC / Network / User
 *  Files / Control Panel) — the SAME per-user registry mechanism the Recycle
 *  Bin writer uses (and the owner's original prototype proved); STYLEABLE.
 *  AppxShortcut (UWP) is ALSO styleable: the desktop entry is an ordinary
 *  .lnk — icon-location write + full-bytes restore (prototype-proven; the
 *  immutable thing is the PACKAGE asset, not the shortcut). The C# enum +
 *  writers + CanStyle updates land in the Windows batch. */
export type IconKind =
  | 'Shortcut' | 'UrlShortcut' | 'AppxShortcut' | 'RecycleBin' | 'SystemIcon' | 'Folder' | 'RegularFile'
  /** Bare launcher on the desktop (绿色软件 .exe) — a PROGRAM in the user's
   *  mind, so it buckets with App, never with documents (ADR-0017 D1; host
   *  classifies by extension in the Windows batch, v1 scope `.exe` only). */
  | 'ExecutableFile'
  | 'Unsupported'

/** User-facing buckets over IconKind for the participation policy (App / Folder
 *  / File / System). Unsupported has no bucket — it is never styleable, so never
 *  governed. Mapping + defaults live in `lib/kind-policy.ts`. */
export type IconKindBucket = 'App' | 'Folder' | 'File' | 'System'

/** Per-bucket "participate in beautify?" — the persistent participation layer
 *  (chief-UI/UX + owner 2026-07-09). ONE switch per bucket governs BOTH manual
 *  apply (skip that kind) AND the future background auto-format (spec 06 §7:
 *  a bucket set false is NEVER touched, even for newly-added icons). It is NOT
 *  part of ConfigDto (that would pollute every preset/history entry) — it lives
 *  on the module state. Cascade: styleable:false > per-icon override > kindPolicy. */
export type KindPolicy = Record<IconKindBucket, boolean>

/** The per-type style envelope (ADR-0017 D3): ONLY these axes may differ by
 *  type — shape (the findability axis), saliency (colorMode limited to the
 *  desaturation family — never Original islands), its tint/band companions,
 *  and a bounded plate colour. Filter stays global (material mixing veto). */
export interface TypePatch {
  shape?: IconShape
  /** Per-type subject (owner correction 2026-07-12): the per-type rows carry a
   *  系统默认 ⊘ = Original, mirroring the main Subject row's ⊘. Allowing
   *  'Original' relaxes ADR-0017's original "step DOWN only / no colour islands"
   *  law so a type can KEEP its own colours even when the global steps down. This
   *  is frontend-only: the Rust background resolver already accepts any subject
   *  string (`TypePatchJson.subject: Option<String>`) and TypePatch rides the
   *  opaque `styleJson` (not a bridge DTO), so no binding/kernel change. */
  subject?: Subject
  tint?: string
  plateBand?: PlateBand
  monoStyle?: MonoStyle
  /** Bounded (chroma-ceiling) plate; null = 随图标. */
  plateColor?: string | null
  /** Per-type 本色 lane: only with plateColor null (随图标 vs 本色). */
  plateFallback?: 'derived' | 'white'
}

/** One type's style source: follow the global config, or a sparse custom
 *  patch over it. No follow-another-type (v1 cut — live cross-type chains
 *  need cycle detection and propagate edits silently). */
export interface TypeOverrideEntry {
  source: 'global' | 'custom'
  patch?: TypePatch
}

/** Sparse per-bucket overrides resolved by `lib/type-config.ts` — the look
 *  layer of the type distinction system. Rides presets/history/setLook so
 *  a look switch swaps the whole ladder coherently. */
export type TypeOverrides = Partial<Record<IconKindBucket, TypeOverrideEntry>>

/** One desktop item (icons contract v2, spec 06 §2): the web renders styling
 *  locally from `sourceUrls` (256px, [0] primary; RecycleBin ships TWO —
 *  empty + full). Positions are OBSERVED desktop truth, never predicted. */
export interface IconItemDto {
  id: string
  label: string
  kind: IconKind
  isShortcut: boolean
  styleable: boolean
  /** Host-localized human reason when styleable is false (e.g. UWP). */
  statusReason: string | null
  x: number
  y: number
  sourceUrls: string[]
  overrideMode: 'keep' | 'tint' | null
  overrideTint: string | null
}

/** Presets are DATA in v2 — the web renders their minis with the live renderer. */
export interface PresetDto {
  id: string
  config: ConfigDto
  /** The preset's type ladder (ADR-0017 D4) — a look is config + ladder. */
  typeOverrides: TypeOverrides
}

export interface HistoryEntryDto {
  index: number
  time: string
  label: string
  isCurrent: boolean
  /** v2: entries carry their FULL recipe so 回到此版 re-bakes locally + restores the same
   *  participation policy — config + typeOverrides + kindPolicy, the three ② knobs (spec 07 §8.2). */
  config: ConfigDto
  kindPolicy: KindPolicy
  typeOverrides: TypeOverrides
}

export interface OverrideEntryDto {
  id: string
  mode: 'keep' | 'tint'
  tint: string | null
}

export interface IconsStateDto {
  scanning: boolean
  working: boolean
  applied: boolean
  dirty: boolean
  styleableCount: number
  config: ConfigDto
  activePresetId: string | null
  presets: PresetDto[]
  history: HistoryEntryDto[]
  palette: string[]
  monoSwatches: string[]
  markSwatches: string[]
  grid: GridDto
  wallpaperUrl: string | null
  /** Per-bucket participation policy (schema v3). Persisted via setLook; the
   *  host + future auto-format read the same map. Default: every bucket true. */
  kindPolicy: KindPolicy
  /** Per-type style overrides (ADR-0017) — the look's type ladder. */
  typeOverrides: TypeOverrides
  /** [M6-WIRE] Native shortcut-arrow state (ADR-0021 machine-wide overlay).
   *  'native' = Windows draws its own arrow (pre-first-apply, or after a
   *  restore); 'hidden' = the global transparent overlay is installed and
   *  DeskMakeover draws the mark. The Settings row status text is the authority
   *  (panel record 2026-07-11 §5). Mock-only until the Tauri cutover batch wires
   *  the elevated `dm-elevated RestoreOverlay` verb + host contract (schema bump
   *  lands there). */
  arrowOverlay: 'native' | 'hidden'
  /** [M6-WIRE] Count of active user profiles on this machine. >1 makes the
   *  first-run consent's machine-wide arrow disclosure non-skippable (owner
   *  disposition 3). Mocked in the browser loop; the host reports the real count
   *  in the Tauri cutover batch. */
  activeUserProfiles: number
}

export interface ToastDto {
  key: string
  arg: string | null
}

// ---- Thin icon bridge (schema 7, D1): Rust returns raw items + the persisted/native bits; the
// frontend assembles IconsStateDto (presets/palette/grid) via lib/icons-assemble. IconsStateDto
// above is now a FRONTEND-ASSEMBLED store shape, not a bridge DTO. ----

/** Observed desktop metrics a scan reports (mirrors Rust `GridMetricsDto`) — the frontend assembles
 *  its grid from these PLATFORM values, never fabricated dims. */
export interface GridMetricsDto {
  screenWidth: number
  screenHeight: number
  taskbarHeight: number
}

/** `icons.scan` result — raw observed items + revision + the observed grid metrics. NO embedded
 *  state (the store assembles it). */
export interface IconScanDto {
  revision: number
  items: IconItemDto[]
  grid: GridMetricsDto
}

/** One store-③ look-history entry (mirrors Rust `LookVersionDto`); `styleJson` is the opaque
 *  `{config, kindPolicy, typeOverrides}` recipe the frontend parses to render its mini. */
export interface LookVersionDto {
  id: string
  // Nullable: the Rust `created_at` is `Option<i64>`, so the generated binding is `number | null`.
  // The handwritten mirror drifted to a non-null `number`, which a legacy row (created before the
  // field existed) would violate at runtime (codex R2 B-8). Kept in sync with `generated.ts`.
  createdAt: number | null
  label: string | null
  pinned: boolean
  styleJson: string
}

/** The persisted ②③ + native bits the frontend overlays onto its assembled state (mirrors Rust
 *  `IconPersistedDto`). `savedStyleJson` is the opaque store-② recipe (or null before any Apply). */
export interface IconPersistedDto {
  savedStyleJson: string | null
  history: LookVersionDto[]
  applied: boolean
  arrowOverlay: 'native' | 'hidden'
  activeUserProfiles: number
}

/** The THIN result of a mutating icon op (mirrors Rust `IconOpResultDto`) — the store re-assembles
 *  IconsStateDto from `persisted` + its own live draft + items. */
export interface IconOpResultDto {
  ok: boolean
  toast: ToastDto | null
  persisted: IconPersistedDto
}

/** One baked master in an applyBakedChunk batch (mirrors Rust `IconChunkItemDto`). */
export interface IconChunkItemDto {
  id: string
  sourceIndex: number
  masterPng: string
}

// ---- Wallpaper module (mirrors Host/Bridge/WallpaperContracts.cs) ----
// Rewritten for spec 04 v2.0 (ADR-0014): the web compositor owns ALL zone/clarity
// rendering; the host only decodes the source and applies the baked PNG.

export type ClarityLevel = 'Off' | 'Soft' | 'Strong'
export type ClarityGradient = 'Linear' | 'Vignette'
export type ScrimTone = 'Dark' | 'Light' | 'Tint' | 'Custom'
export type TitleSize = 'S' | 'M' | 'L'
/** Adaptive tone: Auto samples the wallpaper under the zone. */
export type ZoneTone = 'Auto' | 'Light' | 'Dark'
/** Material finishes (spec 04 §4.1 round 3, 2026-07-15) — six finishes, one
 *  named axis each: Outline 描边 (contour) · Frost 磨砂 (blurred glass) ·
 *  LiquidGlass 流体玻璃 (physical refraction, compositor/liquid-glass-filter.ts)
 *  · Fluted 棱纹玻璃 (vertical fluted glass) · Paper 素笺 (warm matte paper)
 *  · Brushed 拉丝金属 (anisotropic brushed metal). All keep the adaptive tone
 *  sampling. Retired: Luminous→Frost, Solid→Paper, Halo→Frost, and the
 *  owner-cut Glaze→Fluted / Float→Brushed (migrated on load —
 *  lib/wallpaper-assemble.ts). Front-end-only — the zone/look model never
 *  crosses the Rust bridge, so this is not in generated.ts. */
export type ZoneMaterial = 'Frost' | 'LiquidGlass' | 'Fluted' | 'Paper' | 'Brushed' | 'Outline'
/** Title styles (spec 04 §4.2 round 3): hidden / etched glass lozenge / pill
 *  chip / bare label / header bar. Retired: Tab→Chip (migrated on load). */
export type ZoneTitleStyle = 'None' | 'Etched' | 'Chip' | 'Bare' | 'Bar'

export interface ZoneDto {
  /** Stable identity — selection, reconciliation and exit animations key on this. */
  id: string
  cellX: number
  cellY: number
  cellsWide: number
  cellsTall: number
  title: string
  /** Optional emoji prefix rendered with the title. */
  emoji: string | null
  /** Accent hex; null = auto-assigned from the curated palette by zone order. */
  accent: string | null
  tone: ZoneTone
  material: ZoneMaterial
  titleStyle: ZoneTitleStyle
  /** Baked outer drop shadow (投影 finish; ignored by Outline, always-on for
   *  Float, drives the shader shadow ring for LiquidGlass). */
  shadow: boolean
  /** Fill alpha override; null = the material's tone default. LiquidGlass maps
   *  this to the refraction shader's Tint (0 = pure refraction). */
  fillOpacity: number | null
  /** Per-zone corner radius, 0–60 px (desktop pixels; render caps ≤ shortestSide/2). */
  cornerRadius: number
  titleSize: TitleSize
  /** null = bundled default sans (HarmonyOS Sans SC / Inter). */
  fontFamily: string | null
}

export interface ClarityDto {
  level: ClarityLevel
  gradient: ClarityGradient
  angleDeg: number
  dimOverride: number | null
  tone: ScrimTone
  customScrim: string | null
}

export interface LookDto {
  zones: ZoneDto[]
  clarity: ClarityDto
}

/** Decoded, cover-cropped source the compositor renders from (host: WIC-decoded
 *  PNG served over the virtual host; mock: the scene bitmap URL). */
export interface WallpaperSourceDto {
  url: string
  width: number
  height: number
}

export interface WallpaperGridInfoDto {
  screenWidth: number
  screenHeight: number
  taskbarHeight: number
  iconPx: number
  cellWidth: number
  cellHeight: number
  inset: number
  columns: number
  rows: number
}

/** GLOBAL wallpaper positioning (Windows DesktopWallpaperPosition). Only image
 *  PATHS are per-monitor; position/slideshow/bg-color are whole-desktop. `Span`
 *  stretches ONE image across every monitor, so per-screen isolation is undefined
 *  and the UI degrades to a unified canvas (spec 04 §B6). */
export type WallpaperPosition = 'Center' | 'Tile' | 'Stretch' | 'Fit' | 'Fill' | 'Span'

export type ScreenOrientation = 'portrait' | 'landscape'

/** Virtual-desktop bounds of one monitor, in physical pixels (IDesktopWallpaper
 *  GetMonitorRECT). The switcher tiles reproduce these relative positions. */
export interface MonitorBounds {
  x: number
  y: number
  w: number
  h: number
}

/** One physical monitor's RAW screen info (schema 6 thin bridge DTO — mirrors the
 *  Rust `ScreenInfoDto`; `wallpaper.getScreens` returns `ScreenInfoDto[]`). NO look,
 *  NO grid: per D1 the frontend reconciles persisted looks (monitor-reconcile) and
 *  derives the grid from bounds. `monitorId` is the Windows device path
 *  (GetMonitorDevicePathAt) — durable-ish, not permanent across port/driver/dock/
 *  EDID changes. [WINDOWS-VERIFY] real EDID/DisplayConfig fingerprinting for the
 *  bounds-fallback match runs on the owner's Win11 box; the mock matches by path
 *  then by exact bounds only. */
export interface ScreenInfoDto {
  monitorId: string
  name: string
  bounds: MonitorBounds
  orientation: ScreenOrientation
  /** Decoded per-screen wallpaper source; null when unreadable — a third-party
   *  dynamic/video wallpaper is invisible to IDesktopWallpaper (§A4 import CTA). */
  source: WallpaperSourceDto | null
  /** Windows slideshow active on this monitor (rotation won't re-arm after apply). */
  slideshowActive: boolean
  /** GetWallpaper returned a readable image path (false ⇒ dynamic/video wallpaper). */
  hasReadableSource: boolean
}

/** One monitor's ASSEMBLED look (spec 04 §B1) — a FRONTEND store shape, NOT a bridge
 *  DTO (schema 6, D1). Extends the thin `ScreenInfoDto` with the reconciled per-monitor
 *  `look` (from localStorage `wallpaper.look.v2::<device-path>`) and the `grid` derived
 *  from bounds; position/slideshow-mode/bg-color stay global on `WallpaperStateDto`. */
export interface MonitorLookDto extends ScreenInfoDto {
  look: LookDto
  grid: WallpaperGridInfoDto
}

// Multi-monitor state (spec 04 §B1) — a FRONTEND-ASSEMBLED store shape (schema 6, D1),
// NOT a bridge DTO. The store builds it from `wallpaper.getScreens` + the reconciled
// per-monitor looks (see lib/wallpaper-assemble). The top-level `look`/`grid`/
// `originalUrl`/`wallTint`/`hasBackup`/… fields MIRROR the active screen (+ global
// flags); a single-monitor host yields `screens.length === 1` with the top-level
// fields equal to `screens[0]`, so every consumer behaves as before (parity).
export interface WallpaperStateDto {
  // ---- active-screen mirror: top-level == screens[activeScreenId] ----
  look: LookDto
  grid: WallpaperGridInfoDto
  originalUrl: string | null
  // ---- global desktop state (whole-desktop, not per-monitor) ----
  hasBackup: boolean
  working: boolean
  dirty: boolean
  pale: boolean
  fingerprintMismatch: boolean
  wallTint: string
  // ---- multi-screen (§B1) ----
  /** Every present monitor, reconciled by device path (§B3). */
  screens: MonitorLookDto[]
  /** The monitor currently being edited; the top-level look/grid/originalUrl
   *  mirror THIS screen's entry in `screens`. */
  activeScreenId: string
  position: WallpaperPosition
  /** position === 'Span' — the UI degrades to a unified canvas (§B6). Reported
   *  explicitly (the host detects it) rather than derived, so the web never
   *  guesses the span state. */
  spanActive: boolean
}

export interface FontChoiceDto {
  display: string
  family: string | null
}

/** The THIN screen enumeration `wallpaper.getScreens` returns (schema 6 bridge DTO,
 *  mirrors Rust `WallpaperScreensDto`): raw screens + global desktop flags only. NO
 *  looks, NO grids, NO reconcile, NO `hasBackup` — the frontend owns all of that. */
export interface WallpaperScreensDto {
  screens: ScreenInfoDto[]
  position: WallpaperPosition
  /** position === 'Span' — the UI degrades to a unified canvas (§B6). Reported
   *  explicitly by the host (never derived) so the web never guesses the span state. */
  spanActive: boolean
  /** Whether a durable pre-first-apply snapshot exists. Carried here (not only on
   *  mutating-op results) so a COLD START surfaces the whole-desktop restore
   *  affordance when a snapshot persists — a restart after an apply otherwise hides
   *  the only path back. */
  hasBackup: boolean
}

/** The THIN result of a mutating wallpaper op (`applyBaked` / `restore`) — schema 6
 *  bridge DTO, mirrors Rust `WallpaperResultDto`. Per D1 the host does NOT assemble
 *  state; it reports only success, an optional toast, and whether a pre-first-apply
 *  snapshot now exists (so the frontend can enable the whole-desktop restore
 *  affordance). After it, the store re-fetches `getScreens` and re-assembles. */
export interface WallpaperResultDto {
  ok: boolean
  toast: ToastDto | null
  /** true once the pre-first-apply snapshot has been captured + persisted — the
   *  single durable guard against the first apply destroying the original desktop. */
  hasBackup: boolean
}

/** Request/response method map — grows with each controller. */
export interface BridgeMethods {
  // Schema 6 thin wallpaper contract (D1). getScreens REPLACES getState: raw screen
  // info + globals only — the store reconciles looks + assembles WallpaperStateDto.
  'wallpaper.getScreens': { params: void; result: WallpaperScreensDto }
  // setLook LEFT the bridge — per-monitor draft looks persist in the frontend's
  // localStorage (`wallpaper.look.v2::<device-path>`), like `dm.icons.bareLook`.
  // Per-monitor mutating verbs (§B1). `monitorId` is the device path; the baked PNG
  // is the WHOLE look (host stays look-agnostic); restore accepts the 'all' sentinel
  // for the whole-desktop pre-first-apply snapshot revert (§B5).
  'wallpaper.applyBaked': { params: { monitorId: string; pngBase64: string }; result: WallpaperResultDto }
  'wallpaper.restore': { params: { monitorId: string }; result: WallpaperResultDto }
  'fonts.list': { params: void; result: FontChoiceDto[] }
  // Thin icon contract (schema 7, D1): raw sources in ONCE per scan (served over dmicon://),
  // chunked 256px masters out ONLY on apply; the store assembles IconsStateDto from these + the
  // persisted bits + its own presets/palette/grid. sourceIndex: 0 = primary, 1 = paired empty.
  'icons.getPersisted': { params: void; result: IconPersistedDto }
  'icons.scan': { params: void; result: IconScanDto }
  // setLook LEFT the bridge (D1): the config/override/kindPolicy/typeOverrides DRAFT is frontend
  // session state, resumed from ② (savedStyle) on relaunch — spec 07 §8.2 writes ② only on Apply.
  // Begin returns a SESSION TOKEN; every Chunk + the Commit must present it, so a stale/foreign
  // apply's masters can never land in the wrong buffer (a newer Begin mints a new token).
  'icons.applyBakedBegin': { params: { revision: number; count: number }; result: string }
  'icons.applyBakedChunk': { params: { sessionId: string; items: IconChunkItemDto[] }; result: null }
  // The full recipe rides as an opaque JSON string (the envelope Rust persists as ②③). A tint
  // override is baked into its master; a 「保留原样」/kindPolicy-excluded item that is CURRENTLY
  // styled rides `restoreIds` so Rust REVERTS it (spec 06 §2 — not sending a master ≠ restoring).
  'icons.applyBakedCommit': { params: { sessionId: string; styleJson: string; restoreIds: string[]; label: string | null }; result: IconOpResultDto }
  'icons.restore': { params: void; result: IconOpResultDto }
  /** [M6-WIRE] Keep-beautification restore: brings the native shortcut arrow
   *  back (arrowOverlay → 'native') WITHOUT undoing the icon look — shapes and
   *  colours stay. Distinct from `icons.restore` (which undoes everything). The
   *  real elevated verb is `dm-elevated RestoreOverlay` (exact byte restore). */
  'icons.restoreOverlay': { params: void; result: IconOpResultDto }
  /** Native/tray version switch (spec 07 §9): promote a saved appearance to ② and project it
   *  onto the live desktop. The foreground reaches the same end state via stageVersion + apply. */
  'icons.switchVersion': { params: { versionId: string }; result: IconOpResultDto }
  /** The webview composes the branded before/after sheet (it owns the fonts and both image
   *  states — oracle ComparisonImageExporter); Rust only validates + saves the finished PNG
   *  (raw base64) and toasts the saved path. */
  'icons.exportCompare': { params: { png: string }; result: IconOpResultDto }
  // Preset packages + the user preset library (schema 9, spec 09 §6). readPackage is a PURE
  // bounded read (nothing written); the import flow is read → validate (lib/icon-look) →
  // preview → save-per-entry. save is the ONLY library writer; import-as-copy = mint a new id.
  'presets.readPackage': { params: { path: string }; result: PresetPackageReadDto }
  'presets.list': { params: void; result: PresetEntryDto[] }
  'presets.save': { params: { entry: PresetSaveDto; overwrite: boolean }; result: PresetEntryDto }
  'presets.delete': { params: { entryId: string }; result: null }
  'presets.rename': { params: { entryId: string; name: string }; result: PresetEntryDto }
  'presets.export': { params: { destPath: string; entries: PresetSaveDto[] }; result: string }
  'shell.minimize': { params: void; result: null }
  'shell.maximize': { params: void; result: null }
  'shell.restore': { params: void; result: null }
  'shell.close': { params: void; result: null }
  'shell.openExternal': { params: { url: string }; result: null }
  'shell.openDataFolder': { params: void; result: null }
  'app.getInfo': { params: void; result: AppInfoDto }
  'diagnostics.getInfo': { params: void; result: SystemInfoDto }
  'settings.get': { params: void; result: SettingsDto }
  'settings.set': { params: SettingsPatch; result: SettingsDto }
}
