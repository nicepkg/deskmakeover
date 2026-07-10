// Bridge DTOs — mirrors Host/Bridge/Contracts.cs. Bump together with the host
// (BridgeSchema.Version) so drift fails loudly at startup.
export const BRIDGE_SCHEMA_VERSION = 4

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
  dotnetVersion: string
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

export interface FrameMeta {
  key: string
  width: number
  height: number
  revision: number
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
/** SUBJECT axis (ADR-0018): how the artwork renders. Field is GONE as a
 *  mode — 满彩 is now the preset coordinate (Original × 随图标 plate). */
export type Subject = 'Original' | 'BlackWhite' | 'Mono'
export type Distinction = 'Mark' | 'Keep' | 'None'
export type MarkStyle = 'Glass' | 'Shadow' | 'Halo' | 'Satin' | 'Arc' | 'Fold' | 'Ring'
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
  /** 原彩 excluded: types may only step DOWN (no colour islands). */
  subject?: 'Mono' | 'BlackWhite'
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
  /** v2: entries carry their full config so 回到此版 re-bakes locally. */
  config: ConfigDto
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
}

export interface ToastDto {
  key: string
  arg: string | null
}

export interface ScanResultDto {
  revision: number
  items: IconItemDto[]
  state: IconsStateDto
}

export interface IconsOpResultDto {
  state: IconsStateDto
  toast: ToastDto | null
  ok: boolean
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
/** Material finishes (spec 04 §4.1, designer set 2026-07-09). All keep the
 *  adaptive tone sampling; they differ in how the sample becomes paint. */
export type ZoneMaterial = 'Frost' | 'Luminous' | 'Solid' | 'Halo' | 'Outline'
/** Title styles (spec 04 §4.2): pill chip / bare label / folder tab / header bar. */
export type ZoneTitleStyle = 'Chip' | 'Bare' | 'Tab' | 'Bar'

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
  /** Baked outer drop shadow (投影 finish; ignored by Halo/Outline). */
  shadow: boolean
  /** Fill alpha override; null = the material's tone default. */
  fillOpacity: number | null
  /** Per-zone corner radius, 8–28 px (desktop pixels). */
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

export interface WallpaperStateDto {
  look: LookDto
  hasBackup: boolean
  working: boolean
  dirty: boolean
  pale: boolean
  fingerprintMismatch: boolean
  wallTint: string
  grid: WallpaperGridInfoDto
  originalUrl: string | null
}

export interface FontChoiceDto {
  display: string
  family: string | null
}

export interface WallpaperOpDto {
  state: WallpaperStateDto
  toast: ToastDto | null
  ok: boolean
}

/** Request/response method map — grows with each controller. */
export interface BridgeMethods {
  'wallpaper.getState': { params: void; result: WallpaperStateDto }
  'wallpaper.getSource': { params: void; result: WallpaperSourceDto }
  'wallpaper.setLook': { params: { look: LookDto }; result: null }
  'wallpaper.applyBaked': { params: { pngBase64: string; look: LookDto }; result: WallpaperOpDto }
  'wallpaper.restore': { params: void; result: WallpaperOpDto }
  'fonts.list': { params: void; result: FontChoiceDto[] }
  // Icons contract v2 (spec 06 §2): sources in ONCE per scan, chunked 256px
  // masters out ONLY on apply; preview traffic never crosses the bridge.
  'icons.getState': { params: void; result: IconsStateDto }
  'icons.scan': { params: void; result: ScanResultDto }
  'icons.setLook': { params: { config: ConfigDto; overrides: OverrideEntryDto[]; kindPolicy: KindPolicy; typeOverrides: TypeOverrides }; result: null }
  'icons.applyBakedBegin': { params: { revision: number; count: number }; result: null }
  'icons.applyBakedChunk': {
    /** sourceIndex maps multi-source items (Recycle Bin: 0=empty, 1=full). */
    params: { items: { id: string; sourceIndex: number; masterPng: string }[] }
    result: null
  }
  'icons.applyBakedCommit': {
    params: { config: ConfigDto; typeOverrides: TypeOverrides; overrides: OverrideEntryDto[]; label: string }
    result: IconsOpResultDto
  }
  'icons.restore': { params: void; result: IconsOpResultDto }
  'icons.exportCompare': { params: void; result: IconsOpResultDto }
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
