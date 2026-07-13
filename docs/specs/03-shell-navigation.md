# Spec 03 — Shell Navigation: Module Rail + Settings Page

Living spec (ADR-0009, amended by ADR-0010/0012, **now governed by ADR-0013 v3
"Premium Flat"** — light-first; the version narrative is RESTORED by the ADR-0013
amendment: version line + in-app changelog live in About, auto-opens once per
update). The prototype is a historical reference only; spec 02 **v3** governs the look.

## 1. Window anatomy

```
┌────────────────────────────────────────────────────────────────┐
│ 标题栏: ✦logo 桌面美颜 ················· [?] ······· ─ ▢ ✕      │  46px
├──────┬──────────────┬──────────────────────────────────────────┤
│ rail │ 控制面板/页面 │ 桌面镜像画布或设置内容                      │
│ 66px │ (per module) │ (module decides the main surface)         │
└──────┴──────────────┴──────────────────────────────────────────┘
```

- The title bar has no ⚙/⋯ and no version chip — the version story lives in the
  settings About card instead (ADR-0013 amendment: visible version line + in-app
  changelog, auto-opens once per update, never on first install). The bar keeps
  logo · 产品名 · dev-menu flask (DEV builds) · `?` keymap affordance · caption
  buttons. The `?` legend: Space 对比 · Ctrl+1/2/3/4 模块 · 画布拖拽建区 · Del 删除.
- `OverflowMenuView` and the right-side settings drawer are retired. Normal
  navigation always goes through the rail.

## 2. Rail

- Width **66px**, flex column, `align-items:center`, gap 10, padding `4 0 14`.
- Item = 40×40 icon-only tile (radius 13, 15px glyph/icon) + 9.5px label, 3px
  gap. The tile never contains text like 图/纸/设; the label below carries the
  localized word. Selected: tile bg `accent 16% wash` + accent glyph/label; idle:
  tertiary text, hover raises.
- Items:

| id | glyph | label | full name | view |
|----|-------|-------|-----------|------|
| icons | custom app/icon glyph | 图标 | 美化图标 | existing ControlPanelView + mirror |
| paper | wallpaper/panel glyph | 壁纸 | 美化桌面壁纸 | spec 04 panel + mirror w/ zone overlay |
| calm | breeze craft glyph (never a shield) | 清爽 | 清爽系统 | spec 08 calm page (full page) |
| settings | gear/sliders glyph | 设置 | 设置 | settings page |

- The dashed future "+" slot is removed.
- 设置 is pinned near the bottom with a spacer above it, but it is selected like a
  normal module. Clicking it shows the settings page in the main body; it never
  opens a drawer or modal.
- Module switch keeps the app's single window. 图标/壁纸 use the **canvas-left +
  RIGHT inspector (280px; 248px compact)** model (ADR-0013 amendment — the old
  left-300px panel + slide-over are superseded); 设置 replaces the work area with
  the settings page. **Switching is INSTANT and modules stay MOUNTED**
  (visibility-hidden — spec 05 §4); unsaved zone edits always survive.
- Compact mode (<1100px): rail stays; the inspector narrows to 248px (pure CSS).
- Keyboard/UIA: rail buttons are real Buttons with AutomationProperties.Name =
  full module name; Ctrl+1/2/3/4 switch modules POSITIONALLY (1 图标 · 2 壁纸 ·
  3 清爽 · 4 设置 — the numbers match the visual order, so adding 清爽 moved
  设置 from Ctrl+3 to Ctrl+4; spec 08 / ADR-0023).

## 3. Settings page (v3 "Premium Flat" — ADR-0013; body synced to HEAD 2026-07-10)

A calm full PAGE (not a 280px inspector — it uses page-scale type: 13px labels, md
controls, 54px rows, macOS System Settings density). Two columns on wide windows,
stacking below compact width:

- **Right = the working settings**: **ONE grouped inset card per column** holding
  hairline-separated rows (label left, control right) — never a scatter of
  look-alike cardlets. (The earlier "each group its own full-width card" law was
  superseded by this grouped-card idiom in the v3 build.)
- **Left = identity**: logo, product names, slogan; **trust facts render as one
  quiet dotted TEXT line** (statements, not pills — the pill treatment was
  dropped); links are text links; only real push-buttons wear a chip fill, so the
  text/button hierarchy reads at a glance.

Groups (HEAD):

1. **外观**: language segmented (跟随系统 / 简体中文 / English) and theme segmented
   (跟随系统 / 深色 / 浅色), both defaulting to 跟随系统.
2. **自动化**: 新图标自动美化 — **HIDDEN until the capability actually exists**
   (owner decision 2026-07-10; nothing consumes the setting yet, default false).
   The row returns, in the grouped card, when the host watcher/catch-up ships.
3. **本地数据**: distinct actions, none duplicated — 保存前后对比图
   (`icons.exportCompare`) and 打开数据文件夹 (`shell.openDataFolder`). A separate
   导出还原快照 stays deferred until a real snapshot-export RPC exists.
4. **关于**: product identity + trust line + author/repo links, 检查更新, 联系反馈
   — and the **version line + 更新日志** (RESTORED per the ADR-0013 amendment,
   reversing ADR-0012's removal): the version opens the in-app changelog dialog;
   the changelog auto-opens exactly once per installed update, never on first
   install. Version values stay `Unreleased` until the owner names the first release.

Settings actions that open external links go through `shell.openExternal` to the
default browser. About content is inline; no modal/scrim for normal navigation.

## 3.1 Customization visibility (as-built v3)

The audience is customization-heavy, so depth is **visible, not hunted** — realized
in v3 by the inspector grammar itself (spec 02 §Module IA): every axis is a visible
`PropertyRow` with its live value/swatch on the row face; curated first rows with
「更多」 folds expose the full catalog; rarely-used dials nest exactly one Reveal
fold down (clarity 高级, zone 高级). *The older mechanisms this section once
prescribed — localStorage-persisted accordion open-state and a separate
current-values summary strip — were NOT built and are superseded by the always-
visible row grammar (rows are never collapsed, so there is nothing to persist or
summarize).*

## 4. Strings

- New/updated: `Rail_Icons=图标/Icons`, `Rail_Paper=壁纸/Wallpaper`,
  `Rail_Calm=清爽/Calm`, `Rail_Settings=设置/Settings`,
  `Panel_IconsTitle=美化图标/Beautify icons`,
  `Panel_PaperTitle=美化桌面壁纸/Beautify wallpaper`,
  `Panel_CalmTitle=清爽系统/Calm Windows`, `Panel_SettingsTitle=设置/Settings`.
- `Rail_FutureSlot` is removed from product UI.
- About/update/feedback strings live in both zh-Hans and neutral English resx.

## 5. Acceptance

- Title bar shows no ⚙/⋯; no settings drawer exists; 设置 selects the settings
  rail item and shows the settings page.
- The left rail has no inert "+", no text inside the glyph tile, and labels below
  each item.
- Ctrl+1/2/3/4 switch modules positionally; Esc order has no settings drawer branch.
- Theme and language default to system on a clean settings file.
- Zone edits survive module round-trip 壁纸 → 图标 → 壁纸.
- The web test suite stays green (297 at HEAD 2026-07-10; count moves with the tree).
