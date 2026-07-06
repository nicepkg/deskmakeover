# ADR-0008 · The v2 prototype is the binding UI/UX contract for v1.0

**Status:** accepted
**Date:** 2026-07-06
Supersedes: the single-mark decision of [ADR-0007](0007-stacked-card-distinction.md)
(stacked cards survive as one of seven styles), the round-6 "three-band" layout,
and the v0.9/v1.0 slicing of the previous roadmap. Amends [ADR-0005](0005-distinction-shape-color-system.md)
(三星 shape geometry) and the old spec-02 accent token (which still carried a banned
indigo `#5E5CE6` — a documentation error, never a decision).

## Context

After six same-day iteration rounds (three-band layout, distinction gallery,
generated arrow badges), the owner remained deeply unsatisfied with the shipped UI
and built a complete interactive prototype in Claude Design:

> `docs/references/prototype/桌面美颜 v2.dc.html` (+ `support.js` runtime)

The prototype is not a mood board — it is a working, stateful product simulation
(1,723 lines) containing exact layout geometry, all copy strings, the full control
inventory, seven working shortcut-mark algorithms, colour math, motion curves, and
every interaction (apply / dirty-update / restore / history / per-icon overrides /
hold-to-compare / picker / drawer / about). The owner's directive: **v1.0 must
replicate the prototype's effect completely**; existing docs contained substantial
errors and stale content and are rewritten against it.

## Decision

1. **The prototype file is the single source of truth for v1.0 UI/UX.** Where any
   prose document (specs, ADRs, STATE) conflicts with the prototype, the prototype
   wins. Specs 01/02 are rewritten to *describe* it; the implementation plan
   (`plans/2026-07-06-v1-prototype-parity.md`) decomposes it into tasks. The
   "今日形态" (today form) of the prototype defines v1.0; the "未来形态" (module
   rail + 系统净化 / 资源管理器 / 壁纸 views) is design-locked IA for v1.1+ and out
   of v1.0 scope. The demo-control strip (演示控制 row) is prototype scaffolding,
   not product.
2. **Layout: left control panel + desktop-mirror canvas.** A 300px control column
   (hero → CTA → link chips → 风格 presets → 自定义 accordion) beside a
   desktop-mirror canvas (real wallpaper, icon grid, hold-to-compare pill,
   decorative taskbar). Compact windows overlay the panel (slide-in + scrim) and
   surface a summary-chip toolbar above the canvas. This supersedes the round-6
   horizontal three-band toolbar.
3. **Shortcut distinction: 3-state stays; 美化 becomes a seven-style gallery.**
   美化(default) / 经典箭头 / 无标识 per ADR-0005 governance. The 美化 mark is
   user-selectable from seven styles — 玻璃箭头 (default) / 双层卡片 / 幽灵叠影 /
   缎光角 / 珐琅光弧 / 卷角 / 细描边 — each algorithmic, contrast-adaptive, and
   defined precisely by the prototype. **Mark colour returns**: default 自动
   (adaptive), plus swatches and the shared picker. This reverses ADR-0007's
   "single automatic mark, no glyph/colour options"; ADR-0006's engineering facts
   remain binding (bake the mark into each per-icon `.ico`; the global registry
   overlay stays transparent; emit the full 16–256 size ladder).
4. **Shape geometry per the prototype**: 苹果 = quintic superellipse (n=5,
   ≈22.37% apparent corner) · 纯圆 = exact circle (already-round icons untouched,
   `IsRoundish` rule stands) · 三星 = **the official One UI adaptive-icon mask
   path** (cubic Bézier `M50,0 C10,0 0,10 0,50 …`, scaled), replacing ADR-0005's
   superellipse(r=0.40, n=4) approximation.
5. **New v1.0 capabilities (all in the prototype's today form, all icon-scoped):**
   - **图标大小** third axis: 小 / 中 / 大 (preview 52/64/76 px; real desktop via
     the shell's icon-size API, captured in the snapshot and restored).
   - **版本历史**: last 10 applied looks (time + human label + config); 回到此版
     re-applies a config; 回到最初 = full restore. Plus 上一版 quick chip.
   - **Per-icon overrides** (right-click a tile): 保留原样 / 跟随全局样式 / 单独配色.
   - **Hold-to-compare** pill (按住对比原样) + per-tile press-to-peek.
   - **Dirty-state CTA**: applied + changed axes → 「更新桌面」; in-place tile image
     swap with a 「正在更新预览…」 cue (no clear-and-rebuild flash).
   - **Settings drawer** (320px): 外观主题 3-way · 新图标自动美化 toggle ·
     还原快照 export · 前后对比图 save · 关于 / 更新日志 rows.
   - **Overflow menu** (⋯): 检查更新 / 帮助与反馈 / 更新日志 / 关于桌面美颜.
   - **About panel**: identity + 本地运行/无账号/无遥测/全程可还原/**免费开源**
     chips + GitHub repo card + author card with real links + changelog view.
6. **Release renumbering.** The planned "v0.9 抢发" release becomes **v1.0.0**:
   the first public release is the prototype-parity icon beautifier. In-app version
   strings read v1.0 (the prototype's "v0.9 预览" chip text is updated accordingly).
7. **No new functional territory in v1.0.** Everything above is icon-beautification
   UX from the prototype; system-ads, Explorer, wallpaper modules stay v1.1+.

## Consequences

- Specs 00/01/02, STATE, and both READMEs are rewritten to match (this session).
  ADR-0005/0006/0007 receive status amendments; their governance and engineering
  constraints (VOC 3-state, 3-second misread gate, per-icon bake, alpha-edge
  adaptivity) survive.
- `StylePreset` grows axes: distinction state (3) × mark style (7) × mark colour
  (auto/user) × icon size (3); named presets (苹果极简/糖果彩/纯净黑白/壁纸同色)
  set shape × colour (× tint), require dist=美化, and stay highlighted across
  mark-style/size changes (prototype `activePreset()` semantics).
- New persistence: version-history ledger (≤10 configs) and per-icon override map,
  both local, both restored/cleared coherently by 还原.
- The rebuild is decomposed in `plans/2026-07-06-v1-prototype-parity.md` and will
  be executed by a separate AI against the prototype + rewritten specs.
- Open owner actions: OV/individual signing cert (unchanged gate) and creating the
  public `github.com/nicepkg/deskmakeover` repo the About panel links to.
