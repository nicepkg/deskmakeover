# Spec 03 — Shell Navigation: Module Rail + Settings Page

Living spec (ADR-0009, amended by ADR-0010, **amended by ADR-0012** — settings is
rebuilt to the macOS-System-Settings visual language; the version chip + changelog
UI are removed pre-release). The prototype is now a historical reference only
(ADR-0012); spec 02 v2 governs the look.

## 1. Window anatomy

```
┌────────────────────────────────────────────────────────────────┐
│ 标题栏: ✦logo 桌面美颜 ················· [?] ······· ─ ▢ ✕      │  46px
├──────┬──────────────┬──────────────────────────────────────────┤
│ rail │ 控制面板/页面 │ 桌面镜像画布或设置内容                      │
│ 66px │ (per module) │ (module decides the main surface)         │
└──────┴──────────────┴──────────────────────────────────────────┘
```

- The title bar has no ⚙/⋯ and **no version chip** (pre-release — ADR-0012). It
  keeps logo · 产品名 · an optional `?` keymap affordance · caption buttons. The `?`
  opens a small legend popover (Space 对比 · Ctrl+1/2/3 模块 · 画布拖拽建区 · Del 删除).
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

## 3. Settings page (macOS System Settings language — ADR-0012)

The settings page is the **showcase of the v2 visual language** (spec 02): a calm
full page of **grouped inset cards**. On wide windows a narrow identity column sits
beside the settings column; below compact width it stacks into one scroll surface.

Composition law (fixes the "very ugly" verdict):

- **One card system.** Every group is a `--raised` card with `--elev-1`, one radius
  (16), and a consistent **leading icon badge** (coral 16% seat) on its header — no
  card wears a badge while its siblings don't; no two radii sit side by side.
- **Equal-weight, full-width cards.** A lone control (e.g. 新图标自动美化) never
  shares a cramped 2-column grid with a dense card. Each group is its own full-width
  card; a card may hold multiple hairline-separated rows (macOS inset-list idiom).
- Type per spec 02 ladder: page title `display/26`, card title `card/15`, row label
  `body/13` t2, value `body/13` t1, helper `caption/11` t3.

Final groups:

1. **外观**: language segmented (跟随系统 / 简体中文 / English) and theme segmented
   (跟随系统 / 深色 / 浅色), both defaulting to 跟随系统.
2. **自动化**: 新图标自动美化 toggle (its own full-width card).
3. **本地数据**: **three distinct actions** — 导出还原快照 (actually exports the
   restore snapshot), 保存前后对比图 (the comparison exporter), 打开数据文件夹. No two
   buttons may bind to the same effect (the old 导出/打开-both-open-folder duplication
   and the retired `Drawer_Save` string are removed).
4. **关于**: product identity, **trust chips rendered as real pill chips** (not bare
   text), author/repo cards, 检查更新, 联系反馈, homepage. **No 更新日志 / changelog
   section and no version string** are shown pre-release (ADR-0012) — a version story
   returns only at the first real release.

Settings actions that open external links open the default browser. About content is
inline; no modal/scrim is used for normal settings navigation.

## 3.1 Customization visibility (ADR-0012 — applies to the 图标 & 壁纸 panels)

The audience is customization-heavy, so depth is **visible, not hunted**:

- Presets stay the one-tap fast path at the top of each panel.
- The 自定义 axes are **open by default / persist their last open-state** across
  sessions (localStorage), instead of all-collapsed on every launch.
- A compact **current-values summary strip** shows every axis's active value at a
  glance, so the user sees the full configuration without expanding each row.
- Rarely-used sub-controls (e.g. the clarity 高级 fold, custom scrim/angle) may still
  nest one level down — depth is exposed, not dumped flat.

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
