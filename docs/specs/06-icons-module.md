# Spec 06 — Icons Module v2.0 (Rust icon core)

Status: ACTIVE. Supersedes the icon-editing details of spec 01 §Canvas Behaviour / §Control Panel
where they disagree; spec 01 remains the product-identity and safety source. Decisions: **ADR-0019**
(renderer ownership — one Rust `dm-icon-core`, WASM preview/bake + native resident/background; amends the
ADR-0015 web-render table). The pixel-production digest is §1; the pre-inversion CPU-TS framing that
survives in §Scope/§Dependencies below is HISTORICAL rationale (the TS compositor is now the frozen
parity oracle, not the production renderer). Panel record: `docs/reviews/2026-07-09-icon-frontend-panel.md`.

## Scope / Non-scope / Assumptions / Dependencies

**Scope**: icon styling rendered by ONE **Rust `dm-icon-core`** (WASM for the in-window preview +
manual bake, native for the resident/background path) at two resolutions (display-size
interactive preview + 256px bake master); the thin bridge contract (**schema 8**; scan sources in,
thin data out, frontend assembles state — see §2); editing UX contract (live scrubbing, hover
try-on, undo, exception visibility, owned-verb context menu); desktop-mirror fidelity (taskbar,
labels, selection states); dev mock icon pack; frozen-oracle discipline (the TS compositor is the
parity oracle; the C# `TileRenderer` was removed 2026-07-14); background auto-format (spec 07).

**Non-scope**: ICO assembly + sub-256 resampling (owned by Rust `dm-icon-codec`, a byte-for-byte
port of the frozen C# `IcoWriter`/`IconResampler`), UWP PACKAGE-asset editing (the package logo is
immutable; the desktop .lnk IS styleable — see §6), taskbar interactivity, desktop Sort verbs,
hidden-WebView2 background rendering (rejected, ADR-0015 D4), five-material icon styles (approved
direction, sequenced separately).

**Assumptions**: pre-release, no legacy compat required; dev loop = Mac browser +
Vite + mock bridge; all Windows-gated verification batches into the Windows integration /
`[WINDOWS-VERIFY]` phase (the old "F8" is void per ADR-0019).

**Dependencies**: production pixels come from Rust `dm-icon-core` — WASM in the web for preview +
foreground/manual bake (the host packages + writes those WASM-baked masters, no native re-render);
native only for the resident/background path; output is deterministic (libm-routed transcendentals,
no FMA/SIMD in core v1 → WASM↔native byte-equality). The frozen TS compositor (`src/icon-compositor/`, a mechanical port of the C#
pipeline) is the parity oracle, not a live path. (The C# `TileRenderer`, formerly a secondary oracle,
was removed from the repo on 2026-07-14.) pixi v8 stays wallpaper-only. (Historical rationale for
the mechanical CPU-TS port: it maximized oracle parity and runs headless in bun tests — that port is
now the certifying oracle for the Rust core, ADR-0019.)

## 1. Renderer ownership (ADR-0019 digest; supersedes the ADR-0015 table)

- **One Rust core (`dm-icon-core`) is the single algorithm truth source in v1.0**:
  compiled to WASM for in-window preview + foreground/manual bake, and to native for the
  resident/background path (spec 07). Foreground apply writes WASM-baked masters via the host;
  the native build re-renders for the resident projection. Preview, bake, and background render
  the SAME code — the WYSIWYG law becomes structural.
- The core is authoritative for everything the user SEES and for the 256px RGBA
  master. Sub-256 sizes: `dm-icon-codec` ports the linear-light resample ladder
  [256,48,32,24,20,16] + ICO assembly from C# `IconResampler`/`IcoWriter`
  (semantics byte-for-byte).
- **The TS compositor is FROZEN** (banner comments; no new styles, no fixes except
  oracle corrections): it is the PRIMARY parity oracle for the Rust port. The M6 flip is
  EXECUTED (WASM is the production foreground path); the TS compositor stays a non-production
  parity oracle until its physical deletion at M8. (The frozen C# `TileRenderer`, formerly a
  secondary oracle, was removed from the repo on 2026-07-14.)
- Parity gates (ADR-0019): TS↔Rust — classification/branch/plate-seed decisions
  exactly equal; pixels SSIM≥0.995 / bounded ΔE (documented regional tolerances
  for blur/filters). Rust-WASM↔Rust-native — **byte-equal** (libm-routed
  transcendentals, no FMA, no SIMD in core v1). Golden updates require reviewed
  `--bless`; the corpus lives in `testdata/icons/`.

## 2. Bridge contract (icons.*)

> **Replatform note (ADR-0019)**: the code truth is `bridge/types.ts`
> (`BRIDGE_SCHEMA_VERSION = 8`, two-axis subject×plate per ADR-0018). Under Tauri the icon verbs
> ride Tauri commands + the scoped `dmicon://` asset protocol; the thin DTOs (IconScanDto /
> IconPersistedDto / IconOpResultDto) are generated from `dm-contracts` (tauri-specta) and the
> frontend assembles `IconsStateDto` via `lib/icons-assemble` — hand-mirrored schemas are banned.
> Transport-era details (WebAssets host, JSON postMessage limits) are historical rationale, not
> requirements. (`icons.getState` split into `scan` + `getPersisted`; `icons.setLook` left the
> bridge as frontend draft — see spec 05 §3.)

- `icons.getPersisted → IconPersistedDto { ② saved-style + ③ history + applied + arrow + profiles }` —
  the persisted ②③+native bits; the frontend assembles `IconsStateDto` (config/overrides/grid) from
  it via `lib/icons-assemble`.
- `icons.scan { } → IconScanDto { revision, grid, items: IconItemDto[] }` where the real
  `IconItemDto` (see `src/bridge/generated.ts`) is `{ id, label, kind, styleable, statusReason?,
  x, y, sourceUrls: string[] }` — the bridge item carries NO override fields; per-icon overrides
  are frontend-owned draft state that `lib/icons-assemble` overlays onto the rich `IconsStateDto`.
  `sourceUrls` are 256px PNGs served per scan over the
  `dmicon://<id>/<slot>?rev=N` custom protocol (Recycle Bin carries TWO sources: empty+full; the
  contract is a list, never assume 1). Content-addressed, cached.
- Preview: NOTHING crosses the bridge per edit. **`icons.setLook` LEFT the bridge** — the config/
  overrides draft is frontend session state (resumed from ② on relaunch), like wallpaper's setLook.
- Apply: a three-part chunked bake (exact signatures in `src/bridge/generated.ts`) —
  `applyBakedBegin(revision, count) → sessionId` opens a fenced session; N×
  `applyBakedChunk(sessionId, items: IconChunkItemDto[])` stream the PNG-encoded 256 masters
  (≤20/chunk — raw RGBA of 300 icons = 78.6MB and cannot ride one message; PNG total ≈5-7MB);
  `applyBakedCommit(sessionId, styleJson, restoreIds, label) → IconOpResultDto` finalizes.
  **`restoreIds` is load-bearing**: not sending a master ≠ restoring — 「保留原样」 of an
  already-styled icon reverts only via `restoreIds` (Rust CAS-reverts that tracked subset).
  The host decodes → generated-icon store → shell writers. Apply is INCREMENTAL per item
  (ADR-0019; the old restore-entire-desktop-first semantics are NOT ported): owned fields
  update via compare-and-swap against the ledger fingerprint; externally-modified items surface
  as conflicts. Kept-original overrides ship NO master (+ the baked classic arrow per ADR-0021
  while an overlay-hiding makeover is active). Multi-source items bake one master per source:
  `sourceIndex` maps Recycle Bin 0=empty / 1=full. Every advertised master MUST arrive; the web
  aborts before commit if any source failed to decode.
- `icons.switchVersion(versionId) → IconOpResultDto` is a REAL native command (the resident/tray
  projection path bakes native from a history recipe, spec 07 §9). The FOREGROUND version-apply is
  a web flow instead: load the history entry's config into the renderer → bake → the chunked apply
  above, same ceremony as apply (§3.7).
- `icons.restore()`, `icons.restoreOverlay()`, `icons.exportCompare(pngBase64)` (all → IconOpResultDto)
  round out the surface.
- **Preset packages (2026-07-15, bridge schema → 9)**: `presets.list / import /
  export / delete` + the scoped `dmpreset://` thumbnail protocol serve the user
  preset library and `.dmpreset` import/export. Format, versioning/migrations,
  security pipeline and import semantics are normative in **spec 09**; the icons
  panel consumes them via the 风格库 popover (§3.14).
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
   (guarded in the Rust host). The old "size honesty" preview contract is retired with it.
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
10. **Native-arrow gate: LIVE (owner re-affirmation 2026-07-15).** Picking the
    classic-arrow mark (`Dist_Keep`) opens the 60s ArrowGateSheet before it takes
    effect. History of this clause: the 60s stare was owner decree 2026-07-09;
    ADR-0021 (2026-07-10) declared it retired, but the shipped code kept the gate
    and the owner explicitly chose 「维持现状（60秒弹窗）」 on 2026-07-15 after
    hearing the designer's objection (binding record
    `docs/reviews/2026-07-15-icon-preset-io-file-shape-arrow.md` #7) — the
    re-affirmation supersedes the ADR-0021 retirement note. The native arrow
    keeps its current row position; the new Comet arrow mark (spec 02 §Shortcut
    Marks) is an ordinary un-gated option.
11. **Colour is two axes; 极致单色 is a subject/background duotone** (owner +
    chief-UI/UX 2026-07-09; formalized as the two-axis model in ADR-0018). The colour UI is a
    **two-row panel** — 主体行 (subject: 原彩/黑白/单色) × 底板行 (plate: 随图标/本色/白/bounded
    swatches). *(The earlier 前景/背景 dual-tab popover + "BW inert until v2" model was superseded —
    see spec 02 §two-axis + `docs/product/two-axis-colour-spec.md`.)* 单色 gains a 渐变/纯色 depth
    (`monoStyle`): 纯色 = 极致单色, the
    SEGMENTED subject in one flat colour on one flat plate. Subject/background
    segmentation (authored in `dm-icon-core`; frozen TS `icon-compositor/segment.ts` = oracle),
    layered Mono composition, the
    concentric-pair swatch grammar and the full catalog (shapes/colours/marks/
    filters) live in **spec 02 §Shape System / §Colour Treatments / §Shortcut
    Marks**. `ConfigDto` grew `monoStyle: Tonal|Flat` + `plateColor: string|null`
    (rides the generated `dm-contracts` DTOs at bridge schema 8; F8/C# sync is void
    per ADR-0019). Marks are silhouette-aware on free-form icons; Card→Shadow
    (neutral drop shadow), Echo→Halo (silhouette outline).
12. **Default look = 满彩 colour field (ADR-0016 + amendment, owner
    2026-07-10).** **满彩 (Field)** is the factory-default COORDINATE (原彩 subject × a derived
    plate), NOT a fourth subject mode (ADR-0018 two-axis); recipe v7 (designer-seat acceptance PASS)
    lives in spec 02 §Default Composition. **Iron law: subject pixels are never
    recoloured** (the knockout lane was built, owner-rejected, deleted);
    separation = coloured plates (one light line) + silhouette shadows/halos +
    the cross-icon hue-spread pass (worker-reported seeds → deterministic
    main-thread relaxation → `RenderOpts.fieldSeed` on renders AND bakes).
    Preset lineup: **Preset Collection v2 — seven** (spectrum default · stationery · glass · pebble ·
    ink · white · as-cast), superseding the earlier 默认/极简白/安静/原彩保真 four (see
    `docs/product/preset-collection-v2.md` + spec 02); 玻璃 rim rework SHIPPED (T7). The
    kindShapes boolean toggle was built (T5) and then SUPERSEDED by the
    per-type distinction system (§6.5, ADR-0017); kind colour families and
    the letter badge were built and owner-rejected (deleted). Still owed:
    the ΔE-separability corpus test wired to the committed mock manifest (D4).
    Engine facts: memoized whole-canvas `dominantColor`, pale contrast-target
    lane, glyph box 36/256 (~72%), full-bleed 0.82.
13. **System Default is a LENS, not a draft state (owner-disposed 2026-07-15;
    binding record `docs/reviews/2026-07-15-icon-preset-io-file-shape-arrow.md` #3).**
    `bareLook` is a read-only preview lens laid over the draft
    (config/kindPolicy/typeOverrides); lowering it NEVER mutates the draft, and
    exiting resurfaces the draft losslessly. Action classes:
    - *Value-asserting edits* (pick a shape/subject/plate/filter/mark value, a
      preset, a custom type patch) LIFT the lens (`bareLook → false`) and apply
      onto the preserved draft.
    - *Toward-default actions* (「全部重置」`resetTypeOverrides`, per-bucket
      「↺回到跟随全局」, the participation checkboxes `setKindPolicy`) mutate the
      draft but NEVER lift the lens. (Bug fixed 2026-07-15: 全部重置 used to clear
      `bareLook`, snapping the desktop back to a style card.)
    - While the lens is down, the whole panel PROJECTS system-default: every axis
      row lights its ⊘ (the existing item-6 rule) AND the Beautified-types area
      projects follow-global — badges read 跟随全局, sub-axis 跟随全局 anchors
      show selected, 「全部重置」/「↺」 hidden, KeptBar suppressed. The underlying
      typeOverrides are preserved, not cleared. EXCEPTION: the participation
      checkboxes render their REAL persisted state (kindPolicy is orthogonal to
      the style lens — it says who participates at the NEXT apply).
    Testable rule: an action lifts the lens iff it can only be read as "I want a
    beautified look".
14. **Panel layout — inline area is constant-height; collections live in
    popovers (owner-disposed 2026-07-15, P-B + H-A).** The inline preset area is
    exactly two cards — [系统默认][当前风格] — plus a full-width 「风格库 +N」
    trigger strip (the old 4-card grid + 「更多风格」 fold is retired). ALL
    presets (built-in 7 + the user preset library, spec 09) live in the 风格库
    popover: 2-column live-mini grid (live renderer thumbs + 90ms-rest hover
    try-on preserved verbatim), scrollable, grouped 内置/我的; header toolbar
    carries [导入][导出当前][保存为我的风格]; 「我的」 cards get a hover ⋯ menu
    (重命名/删除/导出). History moves out of the scroller tail: the footer
    「历史 N」 button anchors a popover opening ABOVE it (side top, in-viewport,
    canvas stays visible for对照; live version thumbs + 回到此版 ceremony
    unchanged). Neither collection may ever grow the panel's inline height.

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
  120 files at HEAD) under `testdata/icons/source-pack/` (parity fixtures only
  since 2026-07-11 — the mock desktop no longer uses them) with a generated
  `manifest.json` (id, kind, label).
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
- **Real fixtures are the ONLY mock-desktop source** (owner orders 2026-07-09 +
  2026-07-11 — the synthetic pack is too clean to expose 真实适配 bugs and is now
  parity-fixture-only): `public/real-icons/` is the gitignored asset SSoT
  (subfolders windows/folders/apps/files/wallpapers; own nested git repo;
  harvest/scan via `bun scripts/dev/fetch-real-icons.ts`). No synthetic fallback.
  Extracted assets NEVER enter this repo or any shipped artifact (D9; vite
  closeBundle strips dist/real-icons). Developer Options adds
  Messy/Office/Gamer scenario switches for demos.

## 6. Item taxonomy the web receives

`kind ∈ {Shortcut, UrlShortcut, AppxShortcut, RecycleBin, SystemIcon, Folder,
RegularFile, Unsupported}` with `styleable` computed by the Rust host (`dm-windows`).
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
(`{App, Folder, File}: boolean`, default all true — buckets over IconKind per
`lib/kind-policy.ts`; Unsupported is ungoverned). The former System bucket
merged into App (owner 2026-07-16): RecycleBin/SystemIcon kinds bucket to App;
legacy persisted `System` keys are dropped by the whitelist normalizers on
both sides of the bridge, no migration step. ONE switch per
bucket governs BOTH manual apply (that kind renders as-original, ships no
master — RESTORE-FIRST) AND the future background auto-format (§7). It is NOT
part of ConfigDto (that would pollute every preset/history entry) — it rides
the module state, persisted via `icons.setLook`. **Cascade** (folded by
`effectiveTileConfig`): `styleable:false` > per-icon override > kindPolicy —
an icon the user styled individually stays styled even if its whole bucket is
opted out. Two entry points, one state: the panel's **always-visible 「参与美化的
类型」 section — one labeled accordion row per bucket** (checkbox + glyph +
name + status badge; the earlier collapsed-fold, toggle-wall and 2×2-grid
designs were superseded; shows even count-0 buckets) and the tile right-click
「所有<此类>不参与美化」. The RegularFile 「文件美化」 setting is subsumed as
`kindPolicy.File` — **default TRUE** (owner decision 2026-07-10: ordinary files
participate by default; reversibility + the one-click bucket opt-out replace the
old opt-in consent flag).

## 6.5 Per-type distinction system (ADR-0017, owner-disposed 2026-07-10)

The Beautified-types area grows into a TYPE DISTINCTION SYSTEM — per-type
styling bounded by a findability envelope. Normative decisions in ADR-0017;
contract facts here:

- **Type space**: the 3 buckets × shortcut as an orthogonal modifier.
  MECHANISM semantics (owner override): every shortcut kind (`Shortcut`,
  `UrlShortcut`, `AppxShortcut`) buckets to App regardless of target; bare
  `.exe` files join the App bucket (the Rust host classifies by extension at Windows
  integration; the dev mock flags fixtures now); system virtual items
  (RecycleBin/SystemIcon) joined App too (owner merge 2026-07-16 — the
  separate System bucket and its grayscale demotion ladder are gone).
  App's user-facing label becomes 程序.
  `isShortcut` includes `AppxShortcut` (bug fix — UWP shortcuts must wear
  the mark).
- **Config model**: `ConfigDto.kindShapes` is DELETED. `IconsStateDto` (not
  ConfigDto) carries `typeOverrides: Partial<Record<Bucket, {source:
  'global' | 'custom'; patch?: TypePatch}>>`. Since ADR-0018 (two-axis colour)
  the recipe is 主体 × 底板, so `TypePatch ⊂ {shape, subject, tint, plateBand,
  monoStyle, plateColor, plateFallback}` (the live `TYPE_PATCH_KEYS` in
  `type-config.ts`) — the old single `colorMode ∈ {Field, Mono, BlackWhite}` plus
  `fieldBand` collapsed into `subject` (原彩/黑白/单色) and the plate axis.
  `resolveTypeConfig(bucket)` = pure merge; it feeds preview, styleKey (hash of
  the RESOLVED config) and bake identically. Filter and Original stay
  global-only; the beautify switch (kindPolicy) is unchanged and lives in the
  same UI rows.
- **Bounded plate colour**: per-type `plateColor` picks from ≤6 curated
  low-saturation swatches; a fixed-plate type EXITS the hue-spread pool.
  The pool filters to icons whose resolved lane is the derived colour-field
  (原彩 subject with a derived plate, ADR-0018) and whose type has no fixed plate.
- **Factory default (the saliency ladder)**: App=Apple squircle+derived field ·
  Folder=Folder/Bookmark+derived · File=Tile+warm board · shortcut mark default
  **None** (owner decree 2026-07-07 + the durable rule 「presets never carry a
  shortcut mark」 — the distinction badge marks shortcuts, never a default
  arrow; the 「快捷方式统一形状」 toggle also defaults OFF). Shape carries the
  type split; the derived colour-field keeps per-icon identity. (The System
  grayscale demotion died with the System→App merge, owner 2026-07-16.)
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

## 7. Background auto-format — SUPERSEDED by spec 07 (ADR-0020)

Background resident auto-format is a **v1.0 feature** (owner, 2026-07-10) with
**spec 07** as the normative behaviour spec: native Rust rendering (never C#
TileRenderer, never a hidden WebView), reconcile-led watcher, incremental-ledger
versions with pinned hue seeds, and the consent ladder (default OFF · first-run
proposal · opt-in silent · every run undoable · kindPolicy/per-icon keeps as the
opt-out surface — all carried over from this section's original trust contract).
`keepNewIconsStyled` un-hides when the M7 build ships
(`docs/plans/2026-07-10-tauri-migration.md`).

## 8. Verification

- bun tests: OKLab/ramp math vs known values; shape geometry properties (IoU vs
  authored paths); analysis classifiers on synthetic canvases (plate/bare/
  transparent-edge fixtures); compose-mode decision table; undo granularity.
- **Findability net (ADR-0016 D4)**: (a) automated — over the committed mock
  corpus under the default look, neighbouring-plate ΔE separability must clear
  the threshold (bun test; also guards the hue de-dup pass); (b) owner-supervised
  at the Windows integration exit — default look, 20 random targets, locate time/error rate
  not worse than the stock-desktop threshold. A default losing to stock on findability may
  not ship.
  *Gap note 2026-07-10: the contract-chunking (apply ≤20/chunk, count math) and
  mock-manifest-integrity tests promised here are NOT yet written — they are owed with the
  Windows integration, when the chunk path first runs against a real box.*
- Parity fixtures (Windows batch): §1 tolerances, goldens committed under
  `tests/fixtures/icon-parity/`.
- Visual acceptance (every UI slice, browser): scrub latency, hover try-on,
  exception badges, size honesty caption, taskbar realism vs spec values, label
  shadow legibility on light + dark wallpapers, mock pack variety on screen.
- `tsc` clean; 500-line law; new strings go in the TS i18n dictionaries
  (`src/lib/i18n/{en,zh-hans}.ts`), zh-Hans + English in lockstep.
