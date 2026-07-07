# Spec 04 — 美化桌面壁纸 1.0 (wallpaper module)

Living spec (ADR-0009). Goal: pale-wallpaper visibility enhancement + 分区
(partition zones) baked into the wallpaper, edited visually on the existing
desktop-mirror canvas, fully reversible. Basic 1.0 — the owner explicitly capped
complexity; best-possible UX within that cap.

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
   - Zone list (name · cells WxH · style dot); click selects on canvas; ✕ removes.
   - [+ 添加分区] (drops a 6×4-cell zone at the first free grid area) and
     [用推荐布局] (three zones: 常用软件 / 工作文件 / 正在进行 — left tall +
     right two stacked, mirroring the reference shots).
   - Style: two chips 磨砂白 (default) / 壁纸色; applies per-zone (selected zone)
     or as default for new zones when none selected.
6. Footer (11px, hairline-top): honesty line from §1.

## 3. Zone editor (on the existing mirror canvas)

- Active only in 壁纸 view; the mirror keeps real wallpaper + real icons visible
  (icons render above zones, exactly like the final desktop).
- **Create**: drag on empty canvas → rubber-band rect that live-snaps to the real
  icon grid (origin + `IFolderView::GetSpacing` cell); release creates the zone
  (min 2×2 cells). Empty state (no zones yet): centered dashed frame + 「拖一个框,
  把桌面分成区」 + [用推荐布局].
- **Select/move/resize**: 8 handles (corners+edges), all positions/sizes snap to
  integer cell multiples; while dragging, coral `#FF6F5E` 1px dashed alignment
  lines mark the snapped cell edges; on snap the zone pulses scale 1.02→1.0 in
  80ms (reduced-motion: no pulse). Del deletes; arrows nudge by one cell.
- **Rename**: double-click the title → inline TextBox (no dialog), Enter/Esc.
- **Zone visual (preview == bake, one renderer)**:
  - 磨砂白: baked gaussian blur (σ ≈ cell/6) of the covered wallpaper region +
    white overlay α .55 + 1px inset stroke rgba(255,255,255,.5).
  - 壁纸色: dominant palette colour (existing `WallpaperColorService`) lifted to
    L≈85%, α .45, no stroke.
  - Corner radius `min(cellHeight×0.6, 28px)`; title centered at the zone top,
    bundled handwritten font, size = icon label px × 1.3, letter-spacing 2px,
    colour: deepened wallpaper primary (磨砂白) / white (壁纸色); top padding ≈
    0.8 cell so row 1 icons clear the title; side padding ≈ half cell.
- Hold-to-compare (Space) shows the un-styled desktop, consistent with icons.

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
- All new code unit-tested; suite stays green, 0 warnings.
