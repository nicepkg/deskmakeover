# Spec 04 — 美化桌面壁纸 2.0 (wallpaper module)

Living spec. Rewritten 2026-07-09 per **ADR-0014** (client-side compositor +
adaptive materials + curated presets), superseding the 1.0 spec (ADR-0009/0012
rendering pipeline and the 4-named-styles axis). Owner dispositions:
`docs/reviews/2026-07-09-zone-editor-expert-panel.md`. **Round-2 amendment
(same day): five material finishes, four title styles, wallpaper import/export,
gallery-led empty state — `docs/reviews/2026-07-09-style-sets-import-export.md`
is the binding detail record for §2.1/§2.3/§4.1/§4.2.**

Goal: make a messy desktop read as DESIGNED — zones (分区) painted into the
wallpaper as visual containers for grouping icons, plus 壁纸压暗 (visibility
enhancement), edited live at display refresh, fully reversible.

Positioning (PM direction A): a beautiful backdrop, not a file manager. Never
compete with Fences on containment — win on beauty, zero runtime cost, "it IS
your wallpaper", one-click reversal.

## 0. Non-goals (v2.0)

- No icon auto-placement, no icon-cluster auto-suggest (owner call 2026-07-09:
  no on-device model; prediction reads as jarring). Curation over prediction.
- No video/live wallpaper rendering in v1 — but the compositor RESERVES the
  interfaces (§4.4) so video becomes "swap the texture source", not a rewrite.
  FFmpeg-baking zones into video files is permanently rejected (copyright/ToS,
  frozen frost, re-encode per edit).
- No online wallpaper library; no multi-monitor editing (primary only); no
  watermark; never touch Explorer's real label rendering.
- Guided post-apply 整理模式 (icon-in-zone detection) deferred to v1.x.

## 1. Mental model & copy

- Zones are **painted 静态底板**, not containers; copy never says 文件夹/容器.
- First entry: the empty canvas leads with the **preset gallery** (§2.3) — the
  teaching device is seeing real layouts on YOUR wallpaper, not reading a modal.
  One anchored coach line (once, dismissable): 「分区是画在壁纸上的底板,图标要
  你自己拖进去,原壁纸已自动备份。」
- WYSIWYG grammar: nothing touches the desktop until 应用到壁纸.
- "Scrim overlay" is banned vocabulary — the control is 壁纸压暗 / Dim wallpaper
  (owner 2026-07-09: name the ACTION, not the goal; "图标清晰度" implied it edits
  icons when it dims the wallpaper).

## 2. Panel (280px inspector, view `paper`)

1. Status line + hero title (unchanged 5-state CTA machine from 1.0).
2. **壁纸压暗**: segmented 关/柔和/强 + 高级 fold (强度 slider, 渐变方向 dial) —
   composed client-side. (The old 标签暗晕 sub-control was REMOVED in the build.)
3. **分区 list**: rows = leading accent swatch (the zone's categorization signal)
   · editable title · ✕ on hover; ONE container-level active wash slides between
   rows (never a per-row layoutId — regression-tested). Header verbs: [预设] [＋添加].
4. **正在编辑：<名称>** (selection-gated, §3.5 law unchanged; the block stays
   MOUNTED across zone switches — controls morph, nothing remounts). **Style axes
   surface first: 材质 (five finishes, §4.1) · 强调色 swatches (auto-assigned,
   overridable) · 标题样式 (four styles, §4.2)**; the granular dials fold into ONE
   高级 reveal — 不透明度 slider · 色调 Auto/浅/深 · 圆角 slider 8–28 (default 20)
   · 投影 toggle · 标题行: emoji picker + 字号 S/M/L + 字体 (默认 HarmonyOS Sans
   SC; 手写体为可选项). ALL per-zone; one explicit 应用到全部分区 button. No
   global-pretending-to-be-local controls. (只描边 is no longer a toggle — Outline
   is a MATERIAL, round 2.)
5. **壁纸导入 / 导出** (round 2): import your own image as the working source
   (persisted frontend-side across launches, like the per-monitor look under the schema-6
   thin bridge) + export the composed PNG.
6. Footer honesty line (unchanged).

### 2.3 Preset gallery (curation, not prediction — ADR-0014 D6)

4–6 curated layouts (e.g. 工作台 / 极简双区 / 四象限 / 左栏收纳), each shipping
semantic zone names + emoji + accent palette. Gallery thumbnails are LIVE
composites: the user's actual wallpaper rendered through the compositor at
thumbnail resolution. Applying a preset replaces zones (confirm when zones
exist). Works identically on an empty desktop — the user chooses; nothing is
inferred.

## 3. Zone editor (desktop-mirror canvas)

Navigation, create/move/resize/rename, keyboard (Delete deletes, Backspace never,
arrows nudge 0.5 cell), half-cell snapping (owner-retained), min 2×2, undo/redo
history — all carried over from 1.0 §3/§3.5 with these binding changes:

- **Same-frame material.** The compositor renders the zone MATERIAL (frost, fill,
  chip, title) from the same `look` the outline reads, every frame — during
  create/move/resize the material tracks the pointer at display refresh. Nothing
  teleports. Pointer→material latency ≤ 1 display frame; any post-gesture
  refinement lands as a ≤120ms cross-fade, never a jump.
- **Create** draws the forming material (live-snapped), not a marquee; a W×H cell
  badge rides the drag; release = material settles in place, zone auto-selected,
  title enters inline rename (text pre-selected).
- **Guides**: only the edges currently snapped light up (full-grid overlay
  removed); zone-edge magnetism ≤0.35 cell; cross-zone span lines. (Equal-gap
  ticks remain DEFERRED — designed, not built.)
- **Overlap**: allowed; overlapping region wears a warn-wash (coral α.12) during
  the gesture; magnetism makes tiling the path of least resistance.
- **Snap-pulse** (scale 1.02→1.0, 80ms) plays on release commit ONLY.
- **Identity**: zones carry stable `id`s (never index keys); delete plays an exit
  (scale→0.94 + fade, 140ms) via AnimatePresence; visible undo/redo in the canvas
  toolbar + 「已删除·撤销」 toast.
- **Alt-drag** duplicates the zone (copy suffix on the title).
- Ghost icons: full first row + half second row, 3–4 neutral material tones with a
  micro top highlight — reads as "your icons will line up here".
- Edit chrome: selection = inner 1.5px coral + outer 0.5px white halo, radius
  follows the panel; handles = 10×10 white-core rounded squares (r3) with coral
  ring + soft shadow, 20×20 hit boxes, corner-only under 5-cell zones; alignment
  lines coral dashed + 1px white companion (visible on any wallpaper).
- 壁纸压暗 changes and hold-to-compare behave as in 1.0.

## 4. Compositor (ADR-0014 D1 — the ONE renderer)

`src/compositor/` (repo root) — TypeScript, pixi.js v8 / WebGL2.

- **Inputs**: source RGBA (host-decoded, cover-cropped to primary-monitor pixels;
  mock supplies its scene bitmap), grid metrics, `look`.
- **Live**: visible canvas at viewport resolution; source texture uploaded once
  per wallpaper change; dual-Kawase blur; every-frame composite during gestures.
- **Bake**: identical code at native resolution on the MAIN thread — Pixi
  `renderer.extract.canvas(rt)` → `canvas.toBlob('image/png')` → bytes to host
  (`wallpaper.applyBaked`), host writes
  `%LocalAppData%\DeskMakeover\wallpaper\current-bake.png` +
  `IDesktopWallpaper.SetWallpaper` + DWPOS_FILL. (*Corrected 2026-07-10: the
  OffscreenCanvas-worker bake described earlier was not built — main-thread
  toBlob shipped instead; live preview long edge is capped at 4096.*)
  `wallpaper.recompose` frame streaming is deleted from the contract.
- **Parameters scale with resolution** (σ, chip metrics, hairlines are authored in
  desktop-pixel space) so live and bake are the same picture at two sizes.
- Degradation: probe MAX_TEXTURE_SIZE + SwiftShader at startup; software-GL drops
  preview resolution, never blocks bake.

### 4.1 Material system (D3 as amended — FIVE finishes, round 2)

*Round-2 amendment (ADR-0014 Amendments): the single Adaptive Frost became a
five-finish material axis. `src/compositor/material.ts` +
`docs/reviews/2026-07-09-style-sets-import-export.md` are the binding recipes;
the structural laws below hold for every finish.*

- **Finishes** (`ZoneMaterial`): **Frost** (blurred adaptive glass, the default)
  · **Luminous** (gradient glow) · **Solid** (near-opaque panel) · **Halo**
  (soft-edged wash) · **Outline** (fill α≈.05 + deep-tone contour; forces the
  title). Default fill alphas per finish × tone live in `OPACITY_DEFAULTS`
  (Frost .74/.76 — raised from the original .60/.52 after high-contrast
  wallpapers bled through; Solid .94/.92; Halo .55/.55), all overridable by the
  per-zone 不透明度 slider.
- **Adaptive tone** (all finishes): per-panel OKLCH sample of the covered
  wallpaper (L̄,C̄,H̄); auto light/dark at L̄ 0.55 (hysteresis .05; override
  Auto/浅/深); fills derive hue from H̄ with chroma clamped low.
- Frost σ = cellHeight/6; blur-less tier = denser fill + bottom inner shadow
  (video/weak GPU). Depth = 1px top inner highlight + 1px outer contour
  (untinted). **An optional baked drop SHADOW is a per-zone toggle** (round 2 —
  the original "NO baked drop shadow" rule was reversed).
- Radius per-zone 8–28 default 20 (clamp ≤ cellHeight×0.45). Per-zone accent from
  a curated harmonious palette (auto-assigned round-robin, overridable) — the
  zone-to-zone categorization signal.

### 4.2 Title system (D4 as amended — FOUR styles, round 2)

*Round-2 amendment: the single label chip became a four-style title axis
(`ZoneTitleStyle`): **Chip** (the D4 label chip, default) · **Bare** (inkless
text directly on the material) · **Tab** (a tab notched into the panel edge) ·
**Bar** (a full-width header band). Recipes: compositor `title` module + the
round-2 review doc.*

Shared laws (all styles): title anchors top-left; the preferred lane OVERHANGS
the panel top ~0.4 cell into the gutter (reclaims icon row 1); fallback to an
in-panel lane when flush to screen top or a stacked neighbour (ghost slots then
reserve the zone's first row). Ink auto-inverts against the resolved material
tone. Font HarmonyOS Sans SC + Inter 600; size clamp(cell×0.20, 15, 22)px; S/M/L
= cell×0.17/0.20/0.24; optional emoji prefix at ink size. Handwritten = optional
font choice, never default. Chip recipe (Chip style): material one step denser,
pill or r10, padding 10×5; ink OKLCH(0.25)/OKLCH(0.97) α.96.

### 4.3 Apply — 「分区落版」 wave (D5)

On apply success: coral sweep line crosses the canvas 300ms; zones bloom
(scale .97→1 + brightness 1.25→1) 480ms `[0.34,1.4,0.4,1]` staggered 60ms in
reading order — masking bake latency. Reduced-motion: single 120ms brightness
pulse. DoneCard adds 「最后一步:把图标拖进分区,Windows 网格会自动对齐」 +
[去桌面整理] (minimizes the window).

### 4.4 Reserved interfaces (D2 — video later without rewrite)

Source-texture provider (static ⇄ video texture) · blur-cost tiers per material
(incl. no-blur) · output target (StaticImagePNG ⇄ LiveOverlaySurface) · headless
worker operation. No video code in v1.

## 5. Bridge & persistence

- Bridge (thin, schema 6): `wallpaper.getScreens` → `ScreenInfoDto[]` + globals
  (grid/fingerprint/backup flags; NO looks — the frontend assembles the state),
  the cover-cropped source served over the `dmwallpaper://` protocol,
  `wallpaper.applyBaked` (PNG bytes) → thin result, `wallpaper.restore`. `setLook`
  LEFT the bridge (frontend `localStorage`). Mock: scene bitmap as source; applyBaked
  stores the PNG for inspection.
- `zones.json`: zone semantics gain `id`, `accent`, `emoji`, `tone`, `outline`,
  per-zone `cornerRadius` + title settings; environment fingerprint + mismatch
  banner + wallpaper snapshot/restore + slideshow honesty carried over from 1.0
  verbatim.
- Bake/apply remain owner-supervised on live runs
  (`docs/verification/owner-supervised-live-runs.md`).

## 6. Files (≤500 lines each)

- Web: `src/compositor/` (renderer incl. main-thread bake, material, title,
  sampling), `canvas/zone-layer.tsx` (editor chrome only), `canvas/
  wallpaper-mirror.tsx` (gestures) + `canvas/use-wallpaper-compositor.ts`
  (lifecycle) + `canvas/paper-empty.tsx` (gallery-led empty state),
  `panels/wallpaper-panel.tsx` + `wallpaper-zone-list.tsx` +
  `wallpaper-dim-card.tsx` + `wallpaper-panel-popovers.tsx`,
  `lib/zone-presets.ts` (preset data + projection), `stores/wallpaper.ts`.
  (*The `paper-presets.tsx` file named earlier was never created — the gallery
  lives in paper-empty + the presets popover.*)
- Rust host: source decode/crop (WIC), `wallpaper.applyBaked`, `SetWallpaper`, backup/restore
  (Mac-wired schema 6; Windows COM/WIC `[WINDOWS-VERIFY]`). The C#
  `WallpaperBakeRenderer.cs`/`WallpaperComposer.cs` served as the bake oracle and were removed from
  the repo on 2026-07-14 (ADR-0019, ahead of M8).

## 7. Acceptance

- **Parity (migration gate, historical)**: 5 fixture looks, TS bake vs the C# bake (now removed),
  ΔE<2 / SSIM>0.99; thereafter TS fixtures pin the compositor.
- **Latency**: during move/resize/create the material's rect equals the outline's
  rect on the SAME frame (integration test on the render state; manual 120Hz
  verify); no visual element jumps >1 frame after release.
- Material: pale wallpaper → dark tone auto-selected (fixture); every zone's
  accent differs by default (test); outline variant always renders a chip.
- Title: chip contrast ≥4.5:1 against its fill in both tones (computed test);
  overhang lane collapses to in-panel strip at screen-top (fixture); first icon
  row inside the zone is usable (ghost row 1 renders there).
- Presets: gallery thumbnails composite the CURRENT wallpaper; empty-desktop
  first-run shows the gallery; applying with zones present confirms first.
- Interaction: §3 items each unit/integration-tested where state-level (stable-id
  reconciliation, overlap warn state, guide-line selection, pulse-on-release,
  auto-rename-on-create, Alt-drag duplicate, undo toast) or manual-gated where
  pixel-level (chrome visibility over warm wallpaper).
- Copy: **壁纸压暗 / Dim wallpaper** naming everywhere (§1 — the old 图标清晰度
  name is banned: it implied the control edits icons); no 破折号 in user-facing
  strings; zh + en.
- Suite green, 0 warnings.
