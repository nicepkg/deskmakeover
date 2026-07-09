# Spec 06 — Icons Module v2.0 (web renderer)

Status: ACTIVE (2026-07-09). Supersedes the icon-editing details of spec 01 §Canvas
Behaviour / §Control Panel where they disagree; spec 01 remains the product-identity
and safety source. Decisions: ADR-0015 (renderer ownership). Panel record:
`docs/reviews/2026-07-09-icon-frontend-panel.md`. Owner approved Q1-Q12 on 2026-07-09.

## Scope / Non-scope / Assumptions / Dependencies

**Scope**: icon styling rendered by ONE TypeScript renderer (pixi.js v8) at two
resolutions (display-size interactive preview + 256px bake master); bridge contract v2
(sources in, masters out); editing UX contract (live scrubbing, hover try-on, undo,
exception visibility, size honesty, owned-verb context menu); desktop-mirror fidelity
(taskbar P0, labels, selection states); dev mock icon pack; C# renderer freeze
discipline; background auto-format CONTRACT (direction only — build is v1.2).

**Non-scope**: ICO assembly (stays C# `IcoWriter`), sub-256 resampling (stays C#
`IconResampler`), all shell writes (stay C#), UWP PACKAGE-asset editing (the
package logo itself is immutable; the desktop .lnk IS styleable — see §6),
taskbar interactivity, desktop Sort verbs, hidden-WebView2 background rendering
(rejected, ADR-0015 D4), five-material icon styles (approved direction,
sequenced AFTER this migration).

**Assumptions**: pre-release, no legacy compat required; dev loop = Mac browser +
Vite + mock bridge; all Windows-gated verification batches into one session with
wallpaper F8 (ADR-0015 D6).

**Dependencies**: none added. The icon compositor is a CPU TypeScript port of the
frozen C# pipeline (same functions at two resolutions: display-size interactive +
256 master), with staged caching (compose / color / filter layers) and a Web Worker
pool for bake and full recomputes. Rationale over a pixi/GPU pipeline: mechanical
translation maximizes oracle parity, the full pipeline runs headless in bun tests,
output is deterministic (no GPU float variance), and zero dependencies are added.
GPU is a reserved optimization if visual acceptance shows scrub lag at large icon
counts. pixi v8 stays wallpaper-only. C# `TileRenderer` frozen as oracle.

## 1. Renderer ownership (ADR-0015 digest)

- The web renderer is authoritative for everything the user SEES and for the 256px
  RGBA master of every applied icon.
- The web NEVER emits sub-256 sizes. C# `GeneratedIconStore` downscales the master
  through the linear-light `IconResampler` ladder [256,48,32,24,20,16] and `IcoWriter`
  assembles — both byte-for-byte unchanged.
- C# `TileRenderer` + its pipeline are FROZEN: parity oracle + reserved background
  renderer. Freeze = banner comment in the C# files + this spec + no new styles in C#.
  New styles ship TS-only until background auto-format is built (v1.2), which renders
  in C# in-process for unattended writes.
- Parity: golden PNGs from C# at 256 only; ~15-20 source canvases × sampled 60-100
  style cells. Tolerance: flat shape/color ΔE<2 AND SSIM≥0.995; glass/pixel/sticker
  SSIM≥0.98 (visual parity, owner-approved — bit-exactness is off the table across
  GPU/CPU). The corpus is the TS renderer's permanent regression net.

## 2. Bridge contract v2 (icons.*)

- `icons.getState → { config, overrides, applied, dirty, history, settings }` —
  unchanged shape minus render fields.
- `icons.scan { } → { revision, grid, items: IconItemDto[] }` where `IconItemDto =
  { id, label, kind, styleable, statusReason?, x, y, sourceUrls: string[],
  overrideMode, overrideTint }`. `sourceUrls` are 256px PNGs served once per scan via
  the existing WebAssets host (Recycle Bin carries TWO sources: empty+full; the
  contract is a list, never assume 1). 300 icons ≈ 3-6MB once, HTTP-cached.
- Preview: NOTHING crosses the bridge per edit. `icons.setLook { config, overrides }`
  persists only (400ms debounce, mirrors wallpaper).
- Apply: `icons.applyBaked` streams PNG-encoded 256 masters in chunks of ≤20 items
  (raw RGBA of 300 icons = 78.6MB and cannot ride one JSON postMessage; PNG total
  ≈5-7MB). Sequence: `applyBakedBegin{revision,count}` → N× `applyBakedChunk{items:
  [{id, sourceIndex, masterPng}]}` → `applyBakedCommit{config,overrides} → {ok,
  applied}`. C# decodes → `GeneratedIconStore.Save` → shell writers, all unchanged.
  Apply is RESTORE-FIRST (today's `DesktopBakeService.ApplyAsync` semantics):
  「保留原样」overrides ship NO master — the restore step returns them to their
  originals. Multi-source items bake one master per source: `sourceIndex` maps
  Recycle Bin 0=empty / 1=full. Every advertised master MUST arrive; the web
  aborts before commit if any source failed to decode.
- `icons.applyVersion` becomes a WEB flow: load the history entry's config into the
  renderer → bake → applyBaked. It gets the SAME ceremony as apply (see §3.7).
- `icons.restore`, `icons.exportCompare` unchanged.
- Mock: `bridge/mock.ts` implements the same v2 surface; sources come from the dev
  icon pack (§5); baked masters are held in memory and viewable via a debug hook.

## 3. Editing UX contract

1. **Two feedback classes, no other latency budget exists**: discrete picks (preset,
   shape, color mode, filter, mark, size) repaint ALL tiles in the same frame
   (≤16ms at 50 tiles, ≤50ms at 300); continuous inputs (前景/背景 tint pickers, hue
   strip, mark-color wheel) live-scrub at display resolution while dragging, throttled
   leading+trailing (~140ms) so 100+ tile recomputes never pile up. The 420ms config
   debounce is deleted; only setLook persistence stays debounced.
2. **Hover try-on**: hovering a shape/filter/mark/preset swatch paints that candidate
   across the whole desktop; pointer-out reverts; click commits. Hover NEVER writes
   history or config.
3. **Undo/redo**: every discrete pick = exactly one history step (no time-window
   merging of two different picks). Wheel drags coalesce per gesture
   (pointerdown→up = one step). Per-tile override changes and preset picks = one step
   each. Undo/redo buttons live in the canvas toolbar (same component as wallpaper).
4. **Exceptions are visible**: any tile with `overrideMode !== follow` renders a
   small corner badge (pin dot); the panel shows「例外 N」with a
   「清除所有例外」action (one undo step). Override tint picker reuses the full
   调色盘, not a 6-swatch subset.
5. **Un-editable items are honest**: `styleable:false` tiles render at FULL fidelity
   (never dimmed) but expose NO ⋯ button and NO context menu; hover shows an info
   tooltip with the human reason (`statusReason`, e.g. 「此图标由 Windows 管理，
   无法修改」). Counts in the status line use styleable counts only.
6. **Size honesty**: the size control previews as IN-PLACE scale at observed
   positions with one caption line 「应用后位置由 Windows 重新排列」. The mirror
   never re-packs into a predicted grid. After a real apply, re-scan and settle tiles
   to actual positions.
7. **Every real-desktop crossing is ceremonied**: apply, applyVersion and restore all
   get the same confirm + DoneCard treatment. No silent desktop writes, ever.
8. **Context menu = owned verbs only** (app-styled, our chrome): on a tile —
   保留原样 / 跟随全局 / 单独配色 (existing) + 对比原图 hint; on empty canvas —
   图标大小 (3 options) + 刷新 (真 rescan). Windows' Sort/AutoArrange verbs are
   permanently out (we do not own positions). Never render a Win32-lookalike menu.
9. **Compare**: global hold-to-compare pill stays primary. The per-tile
   press-to-peek stays but gains a cursor affordance + tooltip on first hover
   (discoverability), and never conflicts with drag/menu gestures.
10. **Native-arrow gate**: the SIXTY-second penance stare, EVERY time. The gate
    is deliberately obnoxious — that is its function (owner decree, re-affirmed
    2026-07-09; a batched-disposition softening to one-time 8s was reverted the
    same day).
11. **Colour is two axes; 极致单色 is a subject/background duotone** (owner +
    chief-UI/UX 2026-07-09). The Colour row is the FOREGROUND axis; a 背景色
    (plate) rides the row's colour entry (前景/背景 dual-tab; Original + Mono, BW
    inert until v2). 单色 gains a 渐变/纯色 depth (`monoStyle`): 纯色 = 极致单色, the
    SEGMENTED subject in one flat colour on one flat plate. Subject/background
    segmentation (`icon-compositor/segment.ts`), layered Mono composition, the
    concentric-pair swatch grammar and the full catalog (shapes/colours/marks/
    filters) live in **spec 02 §Shape System / §Colour Treatments / §Shortcut
    Marks**. `ConfigDto` grew `monoStyle: Tonal|Flat` + `plateColor: string|null`
    (bridge schema v2 — C# `IconsContracts.cs` + `BridgeSchema.Version` sync in the
    Windows batch). Marks are silhouette-aware on free-form icons; Card→Shadow
    (neutral drop shadow), Echo→Halo (silhouette outline).

## 4. Desktop mirror fidelity (taskbar P0 + tiles)

OS-mirror layer exemption: OS blue `#0067C0` only; coral never appears in the
simulated OS chrome. The taskbar is scenery: zero interactivity, no flyouts, no
fake notifications; it looks like real Windows but never impersonates the user's
own taskbar (neutral glyphs, not their real pinned apps).

1. **Pinned row (centered)**: Start + Search + 4 neutral app glyphs (explorer-like,
   browser-like, store-like, media-like — drawn Fluent-style, MIT/own art only),
   40×40 hover cells (`hover:bg-white/[.08]` dark / `hover:bg-black/[.05]` light),
   24px glyphs. Running indicators via `::after` pills: 6px×3px gray for「open」,
   16px×3px OS-blue for「active」; statically assign 2 open + 1 active.
2. **Tray cluster (right)**: chevron-up glyph · grouped pill (wifi + volume +
   battery, 16px glyphs, full states, static) · two-line clock (HH:mm over
   yyyy/M/d, `font-variant-numeric: tabular-nums`, right-aligned, live) ·
   show-desktop sliver (6px, left hairline on hover).
3. **Acrylic follows APP theme, not the wallpaper**: light
   `rgba(243,243,243,.82)` + `backdrop-blur(20px) saturate(1.8)`; dark
   `rgba(32,32,34,.72)` + `saturate(1.6)`; 1px top hairline `rgba(255,255,255,.06)`.
   Height stays 48px (grid.taskbarHeight).
4. **Start flag**: 4 panes with 1.5px gaps and −2° skew (or two-tone blue),
   never a solid 2×2 grid.
5. **Icon labels**: double shadow `0 1px 2px rgba(0,0,0,.55), 0 0 6px
   rgba(0,0,0,.35)`, 12px/15px, 2-line clamp + ellipsis, center.
6. **Tile states**: hover = rounded translucent wash (stronger than today's
   white/[.08]); selected preview state = white 12% wash + 1px dotted
   `rgba(255,255,255,.25)` border.
7. Mock grid Big icon = 96px (was 64; C# truth is 32/48/96).

## 5. Dev mock icon pack

- Committed, ship-safe only: procedurally GENERATED messy icons (script in
  `scripts/dev/`) + hand-drawn Fluent-style neutral glyphs. Encumbered material
  (extracted Windows icons, brand marks) never enters git or any shipped artifact.
- Target ~120 at 256px, stored as WebP (quality 82+) under
  `src/DeskMakeover.Web/public/mock-icons/` with a generated `manifest.json`
  (id, kind, label).
- Distribution: 30% clean flat plates · 15% skeuomorphic/legacy (incl. PRE-BAKED
  rounded corners — the double-rounding trap) · 12% photo-fill · 10% badged (badges
  in ALL four corners) · 12% transparent-edge irregular logos · 8% letter tiles
  (Latin + CJK) · 8% folder variants · 5% document types.
- Generator randomizes: plate presence/shape, corner treatment (pre-rounded /
  square / transparent), background (solid/linear/radial/photo-noise/pattern),
  palette (full hue + near-black + near-white + PURE black/white degenerates),
  glyph kind, glyph-plate contrast (crosses the 0.66 ink and 0.58 mark thresholds),
  alpha edge (hard/AA/glow/semi), badge presence/corner, content safe-area
  (60-100%), source resolution (mix 32px upscales), lightness polarity.
- The mock bridge maps pack entries onto item kinds (lnk/exe/folder/file/url/bin +
  a few styleable:false UWP stand-ins with statusReason).
- **Dev-only real fixtures** (owner call — the synthetic pack is too clean to expose
  真实适配 bugs): `scripts/dev/fetch-real-icons.mjs` harvests genuine MS/brand icons
  from the two win11 simulator clones into a **gitignored** `public/mock-icons-real/`
  (+ scenario wallpapers, art-matched semantic labels). The mock bridge prefers the
  real pack when present and falls back to the committed synthetic pack on a fresh
  clone. Extracted assets NEVER enter git or any shipped artifact (D9). Developer
  Options adds Messy/Office/Gamer scenario switches for demos.

## 6. Item taxonomy the web receives

`kind ∈ {Shortcut, UrlShortcut, AppxShortcut, RecycleBin, SystemIcon, Folder,
RegularFile, Unsupported}` with `styleable` computed by C#.
**SystemIcon** = the per-user CLSID `DefaultIcon` family (This PC / Network /
User Files / Control Panel) — the SAME HKCU registry mechanism the Recycle Bin
writer uses, proven by the owner's original prototype (win-shell
`Set-RecycleBinIcons`); these are STYLEABLE, and an early dev-mock
classification of them as Unsupported was a mistake (corrected 2026-07-09).
The web treats taxonomy as data: it renders and edits per `styleable`, and
NEVER hardcodes kind behavior beyond: RecycleBin consumes 2 sources
(empty/full) and bakes 2 masters; RegularFile carries the wrapper consent flag
it has today. Discovery fix (C#, Windows batch): shell-namespace scan surfaces
Recycle Bin + the SystemIcon set; `DesktopItemKind.SystemIcon` + generalized
CLSID writers (This PC {20D04FE0-3AEA-1069-A2D8-08002B30309D}, Network
{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}, User Files
{59031A47-3F72-44A7-89C5-5595FE6B30EE}) wired behind the same consent grammar.
Recycle Bin is styleable (owner-approved). **AppxShortcut (UWP) is styleable
too** (correction 2026-07-09, owner-prototype-proven): the desktop entry is an
ordinary `.lnk` — `IconLocation` write + full-bytes restore, identical to any
shortcut; what is immutable is the PACKAGE asset, and the early recon wrongly
promoted that fact into "the shortcut can't be masked". Windows batch:
AppxShortcut joins CanStyle, source extraction reads the AppxManifest logo
(port of the prototype's `Get-AppxIconSource`: AUMID → PackageFamilyName →
manifest Application logo → scale-variant PNG), and the purple "APPX"
fallback tile dies. Nothing on the desktop is un-styleable except genuinely
broken/Unsupported items.

## 7. Background auto-format contract (v1.2 — direction approved, build later)

- Renders in C# in-process (frozen TileRenderer path); never via hidden WebView2.
- Tray-resident watcher: FileSystemWatcher on user+public Desktop with poll
  fallback; user-set debounce 2-10s (default 4s) coalescing installer bursts.
- Incremental: the `active-makeover.json` ledger gains styled-item ids + source
  hashes; only NEW/changed items are styled; user per-tile 「保留原样」 exceptions
  and styleable:false items are NEVER touched.
- Trust contract: default OFF; the offer appears in the post-apply DoneCard; the
  FIRST run is a proposal (「有 N 个新图标，要美化吗？」), silent mode only after
  that consent; every run lands in version history as an undoable entry; the icons
  module shows 「新增图标」markers since last visit + an always-visible toggle
  chip; turning it off never retro-reverts (and says so). App closed → next-open
  audit summary is the primary surface; Windows toast optional (settings).

## 8. Verification

- bun tests: OKLab/ramp math vs known values; shape geometry properties (IoU vs
  authored paths); analysis classifiers on synthetic canvases (plate/bare/
  transparent-edge fixtures); compose-mode decision table; undo granularity;
  contract chunking (applyBaked ≤20/chunk, count math); mock manifest integrity.
- Parity fixtures (Windows batch): §1 tolerances, goldens committed under
  `tests/fixtures/icon-parity/`.
- Visual acceptance (every UI slice, browser): scrub latency, hover try-on,
  exception badges, size honesty caption, taskbar realism vs spec values, label
  shadow legibility on light + dark wallpapers, mock pack variety on screen.
- `tsc` clean; 500-line law; PENDING-RESX discipline for all new strings.
