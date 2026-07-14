# ADR-0009: Unlock the module rail now; wallpaper module ships before 净化

- **Status**: accepted — the wallpaper preview==bake pipeline (the shared `WallpaperBakeRenderer`
  in the body) is **superseded by [ADR-0014](0014-zone-editor-rebuild.md)** (TS/Pixi compositor;
  host I/O later amended to Rust by ADR-0019). The rail-unlock decision stands. (owner, 2026-07-07)
- **Supersedes/amends**: the roadmap note "the icon rail waits for 4+ modules";
  spec 01's "Excluded from v1.0: module rail + 壁纸" (rail + wallpaper now land in
  v1.1); the rail IA contract is now the v3 specs + ADR-0013 (ADR-0008's prototype was
  superseded by ADR-0012 and is historical reference only).

## Context

The owner ordered (2026-07-07): move the title-bar ⚙ settings and the ⋯ overflow
menu (检查更新/联系反馈/更新日志/关于) into a left rail settings entry, unlock the
rail with 美化图标 + 美化桌面壁纸, and build wallpaper beautification 1.0 —
visibility enhancement for pale wallpapers plus 小红书-style partition zones baked
into the wallpaper, with a visual editor. Reference shots:
`C:\Users\yangxiaomingwin\Pictures\壁纸分区` (translucent rounded panels with
handwritten titles like 常用软件/工作文件/正在进行, icons aligned inside).

A four-expert panel (Don Norman persona — usability; top PM persona — scope; top
interaction-designer persona; codex — Windows systems engineering) reviewed the
directive. Full opinions are session artifacts; the durable conclusions are below.

## Decision

1. **Rail now, three entries.** 66px rail per the prototype future form
   (`桌面美颜 v2.dc.html` L58-71): modules **图标** and **壁纸** (2-char labels —
   owner picked over his own 4-char draft; full names 美化图标/美化桌面壁纸 live in
   panel titles + tooltips), plus **设置** pinned at the rail bottom as a utility
   (visually separated from modules). The title bar drops ⚙ and ⋯ entirely; the
   settings drawer absorbs the four overflow items as an "关于" group. The rail's
   justification is repaying the two-entry settings IA debt + honouring the binding
   prototype — not the module count. The dashed "+" future-slot stays.
2. **Wallpaper module jumps the queue** (was v2.0 direction; 净化 shifts one
   release later). v1.1 = rail + wallpaper 1.0.
3. **Zones are semantics, not pixels.** A zone is stored as cell-grid coordinates
   (integer multiples of the REAL desktop icon cell from `IFolderView::GetSpacing`)
   plus an **environment fingerprint** (resolution, DPI, taskbar edge/size, icon
   size, grid spacing, wallpaper source hash). The baked bitmap is a projection; on
   fingerprint mismatch the app offers one-click regeneration instead of silently
   drifting. This is the moat over 小红书 static partition-wallpaper sellers — they
   ship a picture, we ship real alignment + reversibility.
4. **Bake at native resolution, Fill.** The composed wallpaper is rendered at the
   primary monitor's physical pixel size and set with `DWPOS_FILL` (same-size Fill
   degenerates to 1:1), so painted cells match Explorer's grid. v1.0 of the module
   edits the **primary monitor only**.
5. **Full wallpaper snapshot, not just TranscodedWallpaper.** Backup/restore goes
   through `IDesktopWallpaper`: per-monitor wallpaper path, position mode,
   background colour, and slideshow configuration/state. 换回我的壁纸 is one click;
   restore failure never deletes the backup anchor.
6. **No icon auto-placement in v1.1** (owner call — Norman dissented, PM/engineer
   prevailed): the module paints panels + snap reference lines only; users drag
   icons in themselves and Windows' own grid alignment does the rest. "整理到分区"
   (explicit, previewable, journaled `SelectAndPositionItems`) is a v1.2 candidate.
7. **Visibility enhancement is subordinate to zones, and never three raw
   sliders.** Auto-detect pale wallpapers (sample luminance where the icon labels
   actually sit, not the whole-image mean) → recommend, never silently apply. The
   user control is 清晰度 关/柔和/强; fine-grained 渐变/压暗/阴影 controls fold
   into 高级. Label "shadows" are baked darkening halos in the wallpaper under
   label regions — Explorer's real label rendering is never touched (WYSIWYG law).
8. **Honest affordance language** (Norman): zones are called 底板/分区标签, never
   文件夹/容器; a one-time coach mark on first module entry states plainly that
   zones are painted into the wallpaper and icons must be dragged in by the user.
9. **No self-promo watermark** (owner call, PM's growth pitch declined). Output
   stays clean.
10. **Zone titles use a bundled free-for-commercial handwritten-style CJK font**
    (owner call): candidate 站酷快乐体; used only when rasterising titles into the
    bake, never in app chrome. Licence file ships alongside the font asset.

## Consequences

- Roadmap: v1.1 = rail + wallpaper module (this ADR); 净化 → v1.2; Explorer →
  v1.3. Spec 03 (shell navigation) and spec 04 (wallpaper module) carry the
  details; plan `2026-07-07-rail-and-wallpaper.md` executes.
- The desktop-mirror canvas gains an editable overlay layer (zones) — extraction
  must keep preview==bake through one shared `WallpaperBakeRenderer`.
- New risk surface: wallpaper write + restore. Mitigated by the full
  `IDesktopWallpaper` snapshot (risk #1 from the engineer), fingerprint mismatch
  prompts (risk #1 from PM/Norman), and the existing journaled-rollback ethos.
