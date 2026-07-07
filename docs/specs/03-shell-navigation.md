# Spec 03 — Shell Navigation: Module Rail + Settings Page

Living spec (ADR-0009, amended by ADR-0010). Prototype
`docs/references/prototype/桌面美颜 v2.dc.html` L58-71 remains the visual ancestor
for the rail, but ADR-0010 makes 设置 a real page and removes the inert future slot.

## 1. Window anatomy

```
┌────────────────────────────────────────────────────────────────┐
│ 标题栏: ✦logo 桌面美颜 [v1.1] ····················· ─ ▢ ✕      │  46px
├──────┬──────────────┬──────────────────────────────────────────┤
│ rail │ 控制面板/页面 │ 桌面镜像画布或设置内容                      │
│ 66px │ (per module) │ (module decides the main surface)         │
└──────┴──────────────┴──────────────────────────────────────────┘
```

- The title bar has no ⚙/⋯. It keeps logo · 产品名 · version chip · caption
  buttons only.
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
| settings | gear/sliders glyph | 设置 | 设置 | settings page |

- The dashed future "+" slot is removed.
- 设置 is pinned near the bottom with a spacer above it, but it is selected like a
  normal module. Clicking it shows the settings page in the main body; it never
  opens a drawer or modal.
- Module switch keeps the app's single window. 图标/壁纸 keep the 300px panel +
  canvas model; 设置 replaces the work area with the settings page. Switching
  modules never discards unsaved zone edits.
- Compact mode (<1100px): rail stays. 图标/壁纸 keep the existing slide-over panel
  behaviour; 设置 hides the compact summary toolbar and shows a scrollable page.
- Keyboard/UIA: rail buttons are real Buttons with AutomationProperties.Name =
  full module name; Ctrl+1/Ctrl+2/Ctrl+3 switch modules.

## 3. Settings page

The settings page replaces the drawer. It is a calm full page with grouped cards.
On wide windows it may use a narrow identity/summary column plus a wider settings
column; below compact width it stacks into one scroll surface.

Final groups:

1. **外观**: language segmented (跟随系统 / 简体中文 / English) and theme segmented
   (跟随系统 / 深色 / 浅色), both defaulting to 跟随系统.
2. **自动化**: 新图标自动美化 toggle.
3. **本地数据**: 还原快照 export, 前后对比图 save, app data folder access.
4. **关于**: product identity, trust chips, author/repo cards, 检查更新, 联系反馈,
   更新日志.

Settings actions that open external links still open the default browser. About
and changelog content is inline in the settings page; no modal/scrim is used for
normal settings navigation.

## 4. Strings

- New/updated: `Rail_Icons=图标/Icons`, `Rail_Paper=壁纸/Wallpaper`,
  `Rail_Settings=设置/Settings`, `Panel_IconsTitle=美化图标/Beautify icons`,
  `Panel_PaperTitle=美化桌面壁纸/Beautify wallpaper`,
  `Panel_SettingsTitle=设置/Settings`.
- `Rail_FutureSlot` is removed from product UI.
- About/update/feedback strings live in both zh-Hans and neutral English resx.

## 5. Acceptance

- Title bar shows no ⚙/⋯; no settings drawer exists; 设置 selects the settings
  rail item and shows the settings page.
- The left rail has no inert "+", no text inside the glyph tile, and labels below
  each item.
- Ctrl+1/2/3 switch modules; Esc order has no settings drawer branch.
- Theme and language default to system on a clean settings file.
- Zone edits survive module round-trip 壁纸 → 图标 → 壁纸.
- 208+ existing tests stay green; new VM tests cover module switching state.
