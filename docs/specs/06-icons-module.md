# Spec 06 — Icons Module v2.0 (web renderer)

Status: ACTIVE (2026-07-09). Supersedes the icon-editing details of spec 01 §Canvas
Behaviour / §Control Panel where they disagree; spec 01 remains the product-identity
and safety source. Decisions: ADR-0015 (renderer ownership). Panel record:
`docs/reviews/2026-07-09-icon-frontend-panel.md`. Owner approved Q1-Q12 on 2026-07-09.

## Scope / Non-scope / Assumptions / Dependencies

**Scope**: icon styling rendered by ONE **CPU TypeScript renderer** (Worker-run —
NOT pixi; see Dependencies, which this line once contradicted) at two resolutions
(display-size interactive preview + 256px bake master); bridge contract **schema 3**
(sources in, masters out); editing UX contract (live scrubbing, hover try-on, undo,
exception visibility, owned-verb context menu); desktop-mirror fidelity (taskbar,
labels, selection states); dev mock icon pack; C# renderer freeze discipline;
background auto-format CONTRACT (direction only — build later).

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

## 2. Bridge contract (icons.* — schema 3)

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
   shape, color mode, filter, mark) repaint ALL tiles in the same frame
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
6. **Icon size: REMOVED from the product** (owner, commit `d708f87` 2026-07-09 —
   the user sets icon size on the real desktop, not here). No panel control, no
   canvas-menu entry. `ConfigDto.size` survives as a READ-ONLY observed field;
   nothing (including history replay) may write the real desktop icon size
   (C# guard = F8). The old "size honesty" preview contract is retired with it.
7. **Every real-desktop crossing is ceremonied**: apply, applyVersion and restore all
   get the same confirm + DoneCard treatment. No silent desktop writes, ever.
8. **Context menu = owned verbs only** (app-styled, our chrome): on a tile —
   保留原样 / 跟随全局 / 单独配色 + the whole-bucket kindPolicy shortcut
   (「所有<此类>不参与美化」); on empty canvas — 刷新 (真 rescan) ONLY (图标大小
   was removed with the size control). Windows' Sort/AutoArrange verbs are
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
    (web bridge is schema 3 today; the C# `Contracts.cs` + `BridgeSchema.Version`
    sync is F8). Marks are silhouette-aware on free-form icons; Card→Shadow
    (neutral drop shadow), Echo→Halo (silhouette outline).
12. **Default look = 满彩 colour field (ADR-0016 + amendment, owner
    2026-07-10).** The colour axis gains a fourth foreground mode **满彩
    (Field)** — the factory default; recipe v7 (designer-seat acceptance PASS)
    lives in spec 02 §Default Composition. **Iron law: subject pixels are never
    recoloured** (the knockout lane was built, owner-rejected, deleted);
    separation = coloured plates (one light line) + silhouette shadows/halos +
    the cross-icon hue-spread pass (worker-reported seeds → deterministic
    main-thread relaxation → `RenderOpts.fieldSeed` on renders AND bakes).
    Preset lineup: 默认(满彩) · 极简白 · 安静(柔彩, per-icon hue — replaces the
    single-hue wallpaper-tone) · 原彩保真 (the only home of the white
    fallback); Candy left the preset row; 玻璃 rim rework SHIPPED (T7). The
    kindShapes boolean toggle was built (T5) and then SUPERSEDED by the
    per-type distinction system (§6.5, ADR-0017); kind colour families and
    the letter badge were built and owner-rejected (deleted). Still owed:
    the ΔE-separability corpus test wired to the committed mock manifest (D4).
    Engine facts: memoized whole-canvas `dominantColor`, pale contrast-target
    lane, glyph box 36/256 (~72%), full-bleed 0.82.

## 4. Desktop mirror fidelity (taskbar P0 + tiles)

OS-mirror layer exemption: OS-authentic hues are allowed INSIDE the simulated OS
chrome (the reviewed allowlist in `tests/banned-colors.test.ts` — Windows blues +
the taskbar-icons palette); coral never appears there. The taskbar is scenery:
zero interactivity, no flyouts, no fake notifications; it looks like real Windows
but never impersonates the user's own taskbar. The SAME taskbar renders on both
the icons and wallpaper canvases (one component).

1. **Pinned row (centered)**: Start + Search + a small set of COLOURFUL
   own-vector app glyphs depicting the OS layer (flag / folder / browser orb /
   store / music — own art, OS-authentic hues per the exemption; owner iteration
   `88ed05f`), 40×40 hover cells (`hover:bg-white/[.08]` dark /
   `hover:bg-black/[.05]` light), 24px glyphs. *Running-indicator pills were
   REMOVED (owner reversal — the earlier 2-open/1-active prescription is void).*
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
- Target ~120 at 256px, stored as **PNG** (as built — the WebP plan was dropped;
  120 files at HEAD) under `src/DeskMakeover.Web/public/mock-icons/` with a
  generated `manifest.json` (id, kind, label).
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
- The mock bridge maps pack entries onto item kinds (lnk/exe/folder/file/url/bin/
  uwp). UWP stand-ins are STYLEABLE like every AppxShortcut (§6 correction);
  only genuinely `Unsupported` entries carry `styleable:false` + statusReason.
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

**Participation policy (schema v3, chief-UI/UX + owner 2026-07-09).** A
persistent per-BUCKET participation switch, `IconsStateDto.kindPolicy`
(`{App, Folder, File, System}: boolean`, default all true — buckets over
IconKind per `lib/kind-policy.ts`; Unsupported is ungoverned). ONE switch per
bucket governs BOTH manual apply (that kind renders as-original, ships no
master — RESTORE-FIRST) AND the future background auto-format (§7). It is NOT
part of ConfigDto (that would pollute every preset/history entry) — it rides
the module state, persisted via `icons.setLook`. **Cascade** (folded by
`effectiveTileConfig`): `styleable:false` > per-icon override > kindPolicy —
an icon the user styled individually stays styled even if its whole bucket is
opted out. Two entry points, one state: the panel's **always-visible 「参与美化的
类型」 section — a 2×2 grid of labeled chips** (glyph + name + check/hollow ring;
the earlier collapsed-fold and toggle-wall designs were rejected by the owner
2026-07-10; shows even count-0 buckets) and the tile right-click
「所有<此类>不参与美化」. The RegularFile 「文件美化」 setting is subsumed as
`kindPolicy.File` — **default TRUE** (owner decision 2026-07-10: ordinary files
participate by default; reversibility + the one-click bucket opt-out replace the
old opt-in consent flag).

## 6.5 Per-type distinction system (ADR-0017, owner-disposed 2026-07-10)

The Beautified-types area grows into a TYPE DISTINCTION SYSTEM — per-type
styling bounded by a findability envelope. Normative decisions in ADR-0017;
contract facts here:

- **Type space**: the 4 buckets × shortcut as an orthogonal modifier.
  MECHANISM semantics (owner override): every shortcut kind (`Shortcut`,
  `UrlShortcut`, `AppxShortcut`) buckets to App regardless of target; bare
  `.exe` files join the App bucket (host classifies by extension at F8; the
  dev mock flags fixtures now). App's user-facing label becomes 程序.
  `isShortcut` includes `AppxShortcut` (bug fix — UWP shortcuts must wear
  the mark).
- **Config model**: `ConfigDto.kindShapes` is DELETED. `IconsStateDto` (not
  ConfigDto) carries `typeOverrides: Partial<Record<Bucket, {source:
  'global' | 'custom'; patch?: TypePatch}>>` where `TypePatch ⊂ {shape,
  colorMode ∈ {Field, Mono, BlackWhite}, tint, fieldBand, plateColor}`.
  `resolveTypeConfig(bucket)` = pure merge; it feeds preview, styleKey
  (hash of the RESOLVED config) and bake identically. Filter and Original
  stay global-only; the beautify switch (kindPolicy) is unchanged and lives
  in the same UI rows.
- **Bounded plate colour**: per-type `plateColor` picks from ≤6 curated
  low-saturation swatches; a fixed-plate type EXITS the hue-spread pool.
  The pool filters to icons whose resolved colorMode is Field and whose
  type has no fixed plate.
- **Factory default (the saliency ladder)**: App=Apple squircle+Field ·
  Folder=Bookmark+Field · File=Tile+Field · System=Circle+**Mono** ·
  shortcut mark ON. Shape carries the type split; System demotes to quiet
  grayscale; Field keeps per-icon identity.
- **Panel**: the Shape section's One-shape/By-type segmented is REMOVED;
  the type area is an accordion (one row per bucket: summary chip 名称·形
  状·显著性 + custom badge; expand-to-edit reusing the global controls;
  跟随全局|自定义 + reset; one open at a time). HARD REQUIREMENT: an
  expanded type row scope-highlights that type's icons on the canvas and
  dims the rest. Shortcut controls stay a dedicated area: mark style/colour
  + 「快捷方式统一形状」 toggle (default OFF; overrides type shapes for
  shortcuts only, badge unchanged; type Shape sections show a ghost note
  while active).
- **Composition invariant**: `style = resolve(bucket) + badge(isShortcut)`
  — the shortcut layer never supplies base shape/colour except via the
  explicit uniform-shape toggle.
- **v2 parking lot** (recorded, not built): per-type hue BIAS over Field
  derivation · copy-once from another type · live follow-type chains.

## 7. Background auto-format contract (direction approved, build later)

> **Status 2026-07-10 (owner decision):** nothing consumes `keepNewIconsStyled`
> yet — no watcher, no catch-up. Accordingly the settings toggle is **HIDDEN**
> (`SHOW_KEEP_UP=false` in settings-page.tsx) and the default flipped to
> **false** in both mock and `AppSettings.cs`, honouring this section's
> default-OFF trust contract. Un-hide + re-default only when the build below ships.

- Renders in C# in-process (frozen TileRenderer path); never via hidden WebView2.
- Tray-resident watcher: FileSystemWatcher on user+public Desktop with poll
  fallback; user-set debounce 2-10s (default 4s) coalescing installer bursts.
- Incremental: the `active-makeover.json` ledger gains styled-item ids + source
  hashes; only NEW/changed items are styled; user per-tile 「保留原样」 exceptions,
  styleable:false items, AND any bucket with `kindPolicy` false (§6) are NEVER
  touched — a newly-added folder is auto-styled ONLY when auto is ON, the Folder
  bucket participates, and the icon has no per-icon keep. The type section is the
  退订面 (opt-out surface) for auto-format.
- Trust contract: default OFF; the offer appears in the post-apply DoneCard; the
  FIRST run is a proposal (「有 N 个新图标，要美化吗？」), silent mode only after
  that consent; every run lands in version history as an undoable entry; the icons
  module shows 「新增图标」markers since last visit + an always-visible toggle
  chip; turning it off never retro-reverts (and says so). App closed → next-open
  audit summary is the primary surface; Windows toast optional (settings).

## 8. Verification

- bun tests: OKLab/ramp math vs known values; shape geometry properties (IoU vs
  authored paths); analysis classifiers on synthetic canvases (plate/bare/
  transparent-edge fixtures); compose-mode decision table; undo granularity.
- **Findability net (ADR-0016 D4)**: (a) automated — over the committed mock
  corpus under the default look, neighbouring-plate ΔE separability must clear
  the threshold (bun test; also guards the hue de-dup pass); (b) owner-supervised
  at F8 exit — default look, 20 random targets, locate time/error rate not worse
  than the stock-desktop threshold. A default losing to stock on findability may
  not ship.
  *Gap note 2026-07-10: the contract-chunking (applyBaked ≤20/chunk, count math)
  and mock-manifest-integrity tests promised here are NOT yet written — they are
  owed with the F8 host wiring, when the chunk path first runs against a real host.*
- Parity fixtures (Windows batch): §1 tolerances, goldens committed under
  `tests/fixtures/icon-parity/`.
- Visual acceptance (every UI slice, browser): scrub latency, hover try-on,
  exception badges, size honesty caption, taskbar realism vs spec values, label
  shadow legibility on light + dark wallpapers, mock pack variety on screen.
- `tsc` clean; 500-line law; PENDING-RESX discipline for all new strings.
