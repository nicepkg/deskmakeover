# Spec 04 — 美化桌面壁纸 2.0 (wallpaper module)

Living spec. Rewritten 2026-07-09 per **ADR-0014** (client-side compositor +
adaptive materials + curated presets), superseding the 1.0 spec (ADR-0009/0012
rendering pipeline and the 4-named-styles axis). Owner dispositions:
`docs/reviews/2026-07-09-zone-editor-expert-panel.md`. **Round-2 amendment
(same day): five material finishes, four title styles, wallpaper import/export,
gallery-led empty state — `docs/reviews/2026-07-09-style-sets-import-export.md`
is the binding detail record for §2.1/§2.3/§4.1/§4.2.**

Goal: make a messy desktop read as DESIGNED — zones (分区) painted into the
wallpaper as visual containers for grouping icons, plus 图标清晰度 (visibility
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
- "Scrim overlay" is banned vocabulary — the control is 图标清晰度 / Icon clarity.

## 2. Panel (280px inspector, view `paper`)

1. Status line + hero title (unchanged 5-state CTA machine from 1.0).
2. **图标清晰度**: segmented 关/柔和/强 + 高级 fold (强度 slider, 渐变方向 dial,
   标签暗晕) — semantics unchanged from 1.0, now composed client-side.
3. **分区 list**: rows = leading material swatch (renders that zone's own adaptive
   look + accent) · editable title · ✕ on hover. Header verbs: [预设] [＋添加].
4. **正在编辑：<名称>** (selection-gated, §3.5 law unchanged): 不透明度 slider ·
   色调 Auto/浅/深 · 只描边 toggle · accent 色 swatches (auto-assigned,
   overridable) · 圆角 slider 8–28 (default 20) · 标题行: emoji picker + 字号
   S/M/L + 字体 (默认 HarmonyOS Sans SC; 手写体为可选项). ALL per-zone; one
   explicit 应用到全部分区 button. No global-pretending-to-be-local controls.
5. Footer honesty line (unchanged).

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
  removed); zone-edge magnetism ≤0.35 cell; cross-zone span lines + equal-gap
  ticks when spacing matches.
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
- 图标清晰度 changes and hold-to-compare behave as in 1.0.

## 4. Compositor (ADR-0014 D1 — the ONE renderer)

`src/DeskMakeover.Web/src/compositor/` — TypeScript, pixi.js v8 / WebGL2.

- **Inputs**: source RGBA (host-decoded, cover-cropped to primary-monitor pixels;
  mock supplies its scene bitmap), grid metrics, `look`.
- **Live**: visible canvas at viewport resolution; source texture uploaded once
  per wallpaper change; dual-Kawase blur; every-frame composite during gestures.
- **Bake**: identical code at native resolution in an OffscreenCanvas worker →
  `convertToBlob('image/png')` → bytes to host (`wallpaper.applyBaked`), host
  writes `%LocalAppData%\DeskMakeover\wallpaper\current-bake.png` +
  `IDesktopWallpaper.SetWallpaper` + DWPOS_FILL. `wallpaper.recompose` frame
  streaming is deleted.
- **Parameters scale with resolution** (σ, chip metrics, hairlines are authored in
  desktop-pixel space) so live and bake are the same picture at two sizes.
- Degradation: probe MAX_TEXTURE_SIZE + SwiftShader at startup; software-GL drops
  preview resolution, never blocks bake.

### 4.1 Adaptive Frost material (D3)

Per-panel OKLCH sample of the covered wallpaper (L̄,C̄,H̄); tone auto light/dark at
L̄ 0.55 (hysteresis .05; override Auto/浅/深). Light fill OKLCH(0.92,
min(C̄×0.5,.03), H̄) α.60 (slider .35–.85); dark OKLCH(0.20,…) α.52 (.30–.80).
Frost σ = cellHeight/6; blur-less tier = α+0.12 + bottom inner shadow (video/weak
GPU). Depth = 1px top inner highlight (light α.35 / dark α.14) + 1px outer
contour (black α.10 / white α.12, untinted); NO baked drop shadow. Radius
per-zone 8–28 default 20 (clamp ≤ cellHeight×0.45). Outline-only variant: fill
α≤.05 + 2px OKLCH(0.45,·,H̄) contour, label chip forced. Per-zone accent from a
curated harmonious palette (auto-assigned round-robin, overridable) tints the
chip + subtly the fill hue.

### 4.2 Title system (D4)

Top-left label chip: x = panelLeft + radius×0.5 + 14; preferred lane OVERHANGS
the panel top ~0.4 cell into the gutter (reclaims icon row 1); fallback to an
in-panel strip when flush to screen top or a stacked neighbour. Chip = material
one step denser (light OKLCH(0.96) α(intensity+.22); dark OKLCH(0.16)
α(intensity+.18)), pill or r10, padding 10×5; auto-omitted when the panel itself
carries enough contrast. Ink auto-inverts vs chip: OKLCH(0.25)/OKLCH(0.97) α.96.
Font HarmonyOS Sans SC + Inter 600; size clamp(cell×0.20, 15, 22)px; S/M/L =
cell×0.17/0.20/0.24; Latin tracking +1.5%; sentence case; optional emoji prefix
rendered at ink size. Handwritten = optional font choice, never default. Shadow
only in no-chip/outline mode (1px OKLCH(0.10) α.45, offset 0,1).

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

- Bridge: `wallpaper.getState` (grid/fingerprint/backup flags), `wallpaper.
  getSource` → decoded RGBA (or lossless bytes) of the cover-cropped source,
  `wallpaper.applyBaked` (PNG bytes) → apply result, `wallpaper.restore`.
  `fonts.list` unchanged. Mock: scene bitmap as source; applyBaked stores the
  PNG for inspection.
- `zones.json`: zone semantics gain `id`, `accent`, `emoji`, `tone`, `outline`,
  per-zone `cornerRadius` + title settings; environment fingerprint + mismatch
  banner + wallpaper snapshot/restore + slideshow honesty carried over from 1.0
  verbatim.
- Bake/apply remain owner-supervised on live runs
  (`docs/verification/owner-supervised-live-runs.md`).

## 6. Files (≤500 lines each)

- Web: `src/compositor/` (core renderer, material, title-chip, sampling, worker
  bake entry), `canvas/zone-layer.tsx` (chrome only), `canvas/wallpaper-mirror.tsx`
  (gestures), `panels/wallpaper-panel.tsx` + `wallpaper-zone-list.tsx` +
  `panels/paper-presets.tsx` (gallery), `stores/wallpaper.ts`.
- Host (F8): source decode/crop handoff, `wallpaper.applyBaked`, delete
  `WallpaperBakeRenderer.cs`/`WallpaperComposer.cs` + their tests after parity.

## 7. Acceptance

- **Parity (migration gate)**: 5 fixture looks, TS bake vs legacy C# bake,
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
- Copy: 图标清晰度 naming everywhere; no 破折号 in user-facing strings; zh + en.
- Suite green, 0 warnings; evidence to `docs/plans/evidence/2026-07-v3/`.
