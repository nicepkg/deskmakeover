# Spec 03 — Shell Navigation: Module Rail + Consolidated Settings

Living spec (ADR-0009). Prototype `docs/references/prototype/桌面美颜 v2.dc.html`
L58-71 (未来形态 rail) is the binding visual contract; this spec maps it onto the
shipped WPF shell and defines the settings consolidation.

## 1. Window anatomy (after this spec)

```
┌────────────────────────────────────────────────────────────────┐
│ 标题栏: ✦logo 桌面美颜 [v1.1] ····················· ─ ▢ ✕      │  46px
├──────┬──────────────┬──────────────────────────────────────────┤
│ rail │ 控制面板 300px │ 桌面镜像画布                              │
│ 66px │ (per module)  │ (shared scene; module decides overlays)  │
└──────┴──────────────┴──────────────────────────────────────────┘
```

- The title bar **loses ⚙ and ⋯** (buttons, handlers, overflow popup). It keeps
  logo · 产品名 · version chip · caption buttons only.
- `OverflowMenuView` is **retired** (delete view + code-behind; git remembers).

## 2. Rail (prototype L61-69, faithful)

- Width **66px**, flex column, `align-items:center`, gap 10, padding `4 0 14`.
- Item = 40×40 glyph tile (radius 13, 15px glyph) + 9.5px label, letter-spacing
  .4px, 3px gap. Selected: tile bg `accent 16% wash` + accent glyph/label;
  idle: tertiary text, hover raises.
- Items (top→bottom):

| id | glyph | label | panel title (full name) | view |
|----|-------|-------|-------------------------|------|
| icons | 图 | 图标 | 美化图标 | existing ControlPanelView + mirror |
| paper | 纸 | 壁纸 | 美化桌面壁纸 | spec 04 panel + mirror w/ zone overlay |

- After the modules: `flex:1` spacer, then the **dashed future slot** (42×42,
  1px dashed 35% gray, radius 14, "+", tooltip 「更多模块会在这里点亮」), then:
- **设置** entry pinned at the bottom, below the future slot, visually a utility:
  same 40×40 tile + 9.5px「设置」label, glyph ⚙ (Segoe Fluent `E713`), separated
  from the future slot by 10px gap. Clicking toggles the settings drawer (right
  slide-in, unchanged mechanics). It never shows a selected/active wash — the
  drawer is transient, not a module view.
- Module switch keeps the app's single window; the 300px panel swaps content by
  module; the canvas is shared (spec 04 defines what the 壁纸 module overlays).
  Switching modules never discards unsaved zone edits (they live in the VM).
- Compact mode (<1100px): rail stays (it is only 66px); the panel keeps its
  existing slide-over behaviour. The compact summary toolbar remains an
  icons-module-only element (hidden in 壁纸 view).
- Keyboard/UIA: rail buttons are real Buttons with AutomationProperties.Name =
  full module name; Ctrl+1/Ctrl+2 switch modules.

## 3. Settings drawer consolidation

Existing drawer (spec 01 §settings) gains the four former overflow items in an
**关于 group** appended after the current 关于/更新日志 rows — final order:

1. 外观主题 (unchanged) · 新图标自动美化 (unchanged)
2. 还原快照 / 前后对比图 (unchanged)
3. 关于 group: 关于桌面美颜 › · 更新日志 › · **检查更新** › · **联系反馈** ›
   (检查更新/联系反馈 reuse the exact handlers the overflow menu had).

No separate full-page settings view — the drawer stays the one settings surface
(engineer + PM recommendation; smallest change that repays the IA debt).

## 4. Strings

- New: `Rail_Icons=图标/Icons`, `Rail_Paper=壁纸/Wallpaper`,
  `Rail_Settings=设置/Settings`, `Rail_FutureSlot=更多模块会在这里点亮/More
  modules will light up here`, `Panel_IconsTitle=美化图标/Beautify icons`,
  `Panel_PaperTitle=美化桌面壁纸/Beautify wallpaper`.
- Moved: `Overflow_CheckUpdate`/`Overflow_Feedback` become drawer rows (keys may
  be reused; the overflow view itself is deleted).

## 5. Acceptance

- Title bar shows no ⚙/⋯; drawer opens from the rail 设置 entry; all four former
  overflow actions work from the drawer (dark + light, live theme switch).
- Rail visual parity with prototype L61-69 at both themes (screenshot evidence).
- Ctrl+1/2 switch modules; Esc order unchanged (popup → drawer → panel overlay).
- Zone edits survive module round-trip 壁纸 → 图标 → 壁纸.
- 208+ existing tests stay green; new VM tests cover module switching state.
