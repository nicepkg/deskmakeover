# Spec 04 — 美化桌面壁纸 1.0 (wallpaper module)

Living spec (ADR-0009, **amended by ADR-0012** — premium interaction rebuild).
Goal: pale-wallpaper visibility enhancement + 分区 (partition zones) baked into
the wallpaper, edited visually on the existing desktop-mirror canvas, fully
reversible. Capability scope is unchanged (zones + 清晰度 + titles; no new
features — owner call 2026-07-08); the *quality* cap is lifted — every interaction
and visual defect is fixed to a premium bar. §3.5 is the binding correction layer:
several behaviours specified here (coach mark, dashed empty-state, live-snap, snap
pulse, on-canvas rename) shipped as drift and are now acceptance-gated.

## 0. Non-goals (v1.1)

- No icon auto-placement (`SelectAndPositionItems` is a v1.2 candidate behind an
  explicit, journaled, previewable action).
- No online wallpaper library (v2.0 direction per prototype).
- No multi-monitor editing (primary monitor only; others untouched).
- No self-promo watermark (owner call).
- Never touch Explorer's real label rendering — visibility effects are baked
  into the wallpaper bitmap only (WYSIWYG law).

## 1. Mental model & copy (Norman guardrails)

- Zones are **painted静态底板**, not containers. Copy never says 文件夹/容器;
  says 分区/底板. Panel footer: 「分区是画在壁纸上的底板 · 图标要你自己拖进去 ·
  原壁纸已自动备份」.
- One-time coach mark on first module entry (dismiss = never again):
  「分区会被画进壁纸,图标不会自动跑进去——把图标拖到框里,Windows 网格会帮它们
  站整齐。原壁纸已自动备份,随时一键换回。」 [知道了]
- CTA grammar mirrors the icons module: 预览即所得; nothing touches the desktop
  until 应用到壁纸.

## 2. Panel (300px, view `paper`)

Top→bottom:

1. Status line (11.5px tertiary): scanning / 「预览即所得 · 未经同意不动你的壁纸」
   / 「已应用 · 原壁纸已备份」 / dirty 「有新改动待应用」.
2. Hero title 19px: 「给壁纸分个区」 (applied+clean: 「桌面已经很整齐了」).
3. CTA 44px (5-state machine reused from icons): 应用到壁纸 / 正在合成… /
   更新壁纸 / ✓ 已与桌面同步. Below, when a backup exists: link 「换回我的壁纸」.
4. **清晰度** section (visibility): segmented 关 / 柔和 / 强.
   - Auto-detect: sample wallpaper luminance under the actual icon+label rows
     (positions from `DesktopLayoutReader`); if P50 luminance > 0.72 → status hint
     「壁纸偏亮,建议开启清晰度」+ preselect 柔和 as the *recommended* chip (badge
     「推荐」), never silently baked — user still confirms via CTA.
   - 高级 fold (collapsed): 压暗强度 slider (0-40%), 渐变方向 (顶部/底部/四角
     vignette), 标签暗晕 toggle (bakes soft dark halos under label rows).
   - Mapping: 柔和 = 12% top-down gradient dim + label halo 18%; 强 = 22% + 30%.
     (Constants tuned during build with live screenshot verification.)
5. **分区** section:
   - Zone list: each row = a **leading 20px style swatch** (renders that zone's own
     frosted/tint look) · editable title (visible-editable: hairline underline that
     brightens on hover/focus, never bare static-looking text) · cells WxH · ✕.
     Click selects on canvas; the selected row is washed.
   - [+ 添加分区] drops a **6×4-cell zone at the first free grid area** (never a
     fixed origin that stacks on the previous one) and [用推荐布局] (three zones:
     常用软件 / 工作文件 / 正在进行 — left tall + right two stacked). 用推荐布局
     **confirms before replacing** existing zones.
   - **分区样式 (four styles: 磨砂白 default / 壁纸色 / 半透明黑 / 同色边框)** obey
     the explicit edit-scope model of §3.5 — they act on the **selected** zone under
     a visible `正在编辑：<名称>` header; there is no silent "nothing selected →
     repaint every zone". Applying to every zone is an explicit **应用到全部** action.
6. Footer (11px, hairline-top): honesty line from §1.

## 3. Zone editor (on the existing mirror canvas)

- Active only in 壁纸 view; the mirror keeps real wallpaper + real icons visible
  (icons render above zones, exactly like the final desktop).
- Default canvas view in this module = the WHOLE desktop (fit-all) — the module
  judges overall layout, not icon detail (owner call 2026-07-07); 图标 keeps
  fit-height. Manual zoom still works after the switch.
- **Create**: drag on empty canvas → rubber-band rect that live-snaps to the real
  icon grid (origin + `IFolderView::GetSpacing` cell); release creates the zone
  (min 2×2 cells). Empty state (no zones yet): centered dashed frame + 「拖一个框,
  把桌面分成区」 + [用推荐布局].
- **Select/move/resize**: 8 handles (corners+edges), all positions/sizes snap to
  integer cell multiples; while dragging, coral `#FF6F5E` 1px dashed alignment
  lines mark the snapped cell edges; on snap the zone pulses scale 1.02→1.0 in
  80ms (reduced-motion: no pulse). Del deletes; arrows nudge by one cell.
- **Rename**: double-click the title → inline TextBox (no dialog), Enter/Esc.
- **Zone coordinates move in HALF-CELL steps** (owner call: finer snapping); icons
  still land on the global integer grid — a half-offset zone wears asymmetric
  padding. Drag guides show full-cell lines strong, half-cell lines faint.
- **Zone visual (preview == bake, one renderer; owner-tuned 2026-07-07)**:
  - **Title band**: the zone's FIRST CELL ROW is a header band — a denser wash,
    NO divider (owner: the line read ugly); icons start on row 2, so a full icon
    row can never cover the title. Title centered in the band, bundled handwritten
    font, size = cell height × 0.30 (20–38px), soft ink (α .92), colour per style.
  - Four styles (all borrow the wallpaper's own palette — 深度融合):
    - 磨砂白 (default): baked gaussian blur (σ ≈ cell/6) + white overlay α .55 +
      hairline inset; title = deep wallpaper ink (OKLab L .35).
    - 半透明黑: baked blur + dark overlay rgba(16,18,23, .46); light title.
    - 壁纸色: dominant colour lifted to L≈85%, α .45, no stroke; ink title.
    - 同色边框: near-transparent body (white α .13) + border in the wallpaper's
      deep tone (L .45, width ≈ cell×0.032 ≥3px) — the wallpaper stays the hero.
  - Corner radius: a USER slider 0–24px, default 12 (owner: 方形 is a valid
    taste); the selected-zone outline mirrors it.
  - **Simulated icons (editor only, never baked)**: a PARTIAL spread (~1.5 rows,
    3–12) of the owner's blueprint mark (apple-squircle, 82% icon size for air on
    all sides), aligned to the GLOBAL grid so they sit exactly where real icons
    would land. Real desktop icons are hidden in this module (owner call).
  - Edit chrome shows ONLY on the selected zone (owner: idle outlines polluted
    the preview); recommended/default layouts keep half-cell margins from edges.
- Hold-to-compare (Space) shows the un-styled desktop, consistent with icons —
  and **hides all zone edit chrome** (selection outline, handles, ghost icons,
  rubber-band) so the true before-state is visible (was drift: chrome stayed painted).

## 3.5 Edit scope, reversibility & navigation (ADR-0012 — binding)

The premium corrections that turn "barely usable" into "good". All are acceptance
items (§7).

- **Explicit edit scope (fixes the loudest complaint).** Style / fill / opacity /
  corner controls act on the **currently selected** zone. The 分区样式 accordion
  carries a persistent header `正在编辑：<zone name>`; with no selection it reads
  `未选择分区` and the style controls are inert (or offer an explicit **应用到全部**
  affordance) — the panel MUST NEVER silently repaint every zone because nothing is
  selected. The zone-list readout with no selection reports `多个分区` honestly, not
  a defaulted 磨砂白. Selection persists across an accidental empty-canvas click;
  deleting a zone clears selection deliberately, not as a side effect that then
  mass-edits.
- **Reversibility (parity with icons' history).** A session undo/redo stack over
  `look` snapshots (Ctrl+Z / Ctrl+Shift+Z) covers zone create / move / resize /
  delete / restyle and 用推荐布局. Delete is `Delete` only — **`Backspace` is not a
  delete key** (it is a common "go back" reflex over a stale selection). 用推荐布局
  and any replace-all confirm first when zones exist.
- **One canvas navigation model.** The paper canvas gains **Ctrl+wheel zoom at the
  pointer + pan**, identical to the icons canvas (this spec already promised "manual
  zoom still works after the switch"). Left-drag on empty canvas **creates** a zone
  and the host wears `cursor-crosshair` so it reads as *draw*, not *pan*; pan is
  space-drag or middle-drag. The two modules must feel like one app.
- **Direct-manipulation feel.** The create rubber-band **live-snaps** as it is
  dragged (it draws the snapped rect, not the raw pointer rect — was drift); on a
  snap-cell change the zone plays `snap-pulse` (scale 1.02→1.0, 80ms; reduced-motion
  none — was drift). Resize handles carry enlarged transparent hit boxes.
- **On-canvas rename.** Double-click a zone's on-canvas **title band** → inline edit
  (Enter/Esc) — the title is edited where it is seen (was drift: rename lived only in
  the panel list, an execution gulf). The panel list input stays as a secondary path.
- **First-run + empty state (binding, was drift).** The one-shot coach mark
  (`wallpaperCoachShown`) shows on first module entry with the §1 copy. The empty
  canvas (no zones) renders a **centered dashed drop-frame** + 「拖一个框，把桌面分成区」
  + an inline **[用推荐布局]** button — not a small floating pill.
- **Load feedback.** The paper canvas shows the shared scanning `shimmer` skeleton
  while loading and fades the first compose frame in (no dead box → hard cut).

## 4. Bake pipeline (one renderer, WYSIWYG)

`WallpaperBakeRenderer.Render(source, config) → Rgba32[]` — pure, shared verbatim
by the canvas preview (downscaled through the existing linear-light resampler)
and the applied wallpaper (native resolution):

1. Load the **original** wallpaper source (`IDesktopWallpaper.GetWallpaper` path;
   fallback TranscodedWallpaper), area-resample to the primary monitor's physical
   pixels (reuse `IconResampler` linear-light math).
2. Visibility layers (清晰度 + 高级 params): gradient dim, vignette, label halos.
3. Zones: blur+fill+stroke+title per §3, positioned by cell-grid geometry.
4. Encode PNG → `%LocalAppData%\DeskMakeover\wallpaper\current-bake.png`; apply
   via `IDesktopWallpaper.SetWallpaper(primaryMonitorId)` + `DWPOS_FILL`
   (same-size Fill == 1:1, guaranteeing painted cells align with Explorer's grid).

## 5. Persistence, fingerprint, restore

- `zones.json` (LocalAppData): zone semantics (cell rect, name, style), 清晰度
  config, and the **environment fingerprint**: monitor device id, native WxH, DPI
  scale, taskbar edge+thickness, icon size px, cell spacing, source-wallpaper
  SHA-256, bake file hash.
- On module open / desktop refresh: recompute fingerprint; mismatch (resolution,
  DPI, grid, or wallpaper changed underneath us) → non-blocking banner in the
  panel: 「桌面环境变了,分区可能错位 — 一键按当前环境重新合成」[重新合成].
  Never silently re-bake.
- **Wallpaper snapshot** (before first apply): full `IDesktopWallpaper` state —
  per-monitor wallpaper paths, position mode, background colour, slideshow
  folder/options/state — plus a byte copy of the primary source image, stored
  under `wallpaper\backup\`. 「换回我的壁纸」 restores everything; a failed
  restore keeps the backup anchor (never delete-then-verify).
- If the user's wallpaper is a slideshow: applying pauses it honestly — status
  line says 「幻灯片已暂停,换回壁纸时恢复」; restore re-enables it.

## 6. Files (≤500 lines each, planned homes)

- `DeskMakeover.Shell/DesktopWallpaperInterop.cs` — IDesktopWallpaper COM wrapper.
- `DeskMakeover.Operations/WallpaperSnapshotService.cs` — backup/restore.
- `DeskMakeover.IconRendering/WallpaperBakeRenderer.cs` (+ `ZoneStyle.cs`) — pure
  compose (visibility + zones); reuses SrgbLinear/IconResampler/WallpaperColor.
- `DeskMakeover.Core/WallpaperConfig.cs` — zones, clarity, fingerprint records.
- `DeskMakeover.App/ViewModels/WallpaperViewModel*.cs`, `Views/WallpaperPanelView
  .xaml`, canvas zone-overlay additions.

## 7. Acceptance

- Preview == applied wallpaper pixel-for-pixel at native res (fixture test on the
  shared renderer + live screenshot diff).
- Apply → 换回我的壁纸 round-trip leaves zero residue (position mode, slideshow
  state, background colour all restored; fixture-level tests).
- Zones snap to the real grid: drop an icon inside a zone on the live desktop →
  it lands visually inside the panel with padding intact (manual gate, evidence
  screenshot).
- Fingerprint mismatch banner appears after a simulated resolution change
  (unit-tested via injected fingerprint).
- Coach mark shows exactly once; all copy per §1; dark+light verified.
- **§3.5 (ADR-0012):** with no selection, a style-control change repaints **nothing**
  (never all zones); the 分区样式 header shows the selected zone name; the empty
  canvas shows the dashed drop-frame (not a pill); the create rubber-band draws a
  snapped rect that pulses on cell change; Ctrl+wheel zooms + drag pans the paper
  canvas; hold-Space hides all zone chrome; double-click a zone title renames inline;
  Ctrl+Z undoes a delete; `Backspace` does not delete; 用推荐布局 confirms when zones
  exist. Each is a state/interaction test or a live-verified manual gate.
- All new code unit-tested; suite stays green, 0 warnings.
