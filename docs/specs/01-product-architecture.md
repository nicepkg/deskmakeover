# DeskMakeover Product Architecture (v1.0 · prototype-derived)

**UI/UX source of truth:** `docs/references/prototype/桌面美颜 v2.dc.html`
(ADR-0008). This spec describes the product; the prototype defines it.

## Product Identity

DeskMakeover is the English product name. The Chinese product name is **桌面美颜**
(ADR-0002). Chinese slogan: 「一键美颜你的 Windows 桌面，随时完整还原」. English:
"Give your Windows desktop a one-click makeover. Restore everything anytime."
Narrative (ADR-0004): 「让 Windows 回到它本该的样子」 — a beauty camera for the
desktop, never a 管家/cleaner. Positioning chips (About panel): 本地运行 · 无账号 ·
无遥测 · 全程可还原 · 免费开源.

**Tone rule (ADR-0003):** never judge the user's desktop; additive voice only;
no anxiety language, ever.

## Target Users

Non-technical Chinese Windows users first; design-sensitive users satisfied
through restraint, not options. Windows 10 + 11; Win7 excluded. Win10 degrades
gracefully (spec 02). The app ships Simplified Chinese and English from the
first public release; language defaults to the Windows UI culture and falls back
to English for unsupported cultures.

## v1.0 Scope (icons only — ADR-0008)

One reliable loop, all on one screen:

1. Launch → auto-scan (shimmer skeleton, no scan button).
2. Live preview on the desktop mirror; default style = 苹果极简.
3. 一键美化 → invisible snapshot → journaled apply → bloom wave → done state.
4. Tweak any axis → dirty state → 更新桌面 (in-place re-render, no flash).
5. 还原 anytime → complete zero-residue restore; 版本历史 re-applies old looks.

**Included** (all from the prototype's today form):

- Discovery: user Desktop + Public Desktop (+ OneDrive-redirected detection);
  `.lnk`, `.url`, AppX/UWP, Recycle Bin, folders, regular files (wrapping stays
  opt-in and off by default).
- Composable style axes: **风格 presets** (4) · **外形** (platform defaults +
  expanded maskable shapes) · **配色** (3 + tint) · **快捷方式标识** (3 states,
  6 user-facing mark styles, mark colour) · **图标大小** (3).
- Version history (last 10 looks) + 上一版 + 回到最初.
- Per-icon right-click overrides: 保留原样 / 跟随全局样式 / 单独配色.
- Hold-to-compare + per-tile press-to-peek.
- Keep-up without residency (新图标自动美化): logon run-and-exit task + app-launch
  catch-up; ADR-0003 model unchanged.
- Auto snapshot → journaled apply → one-click restore; single batched UAC via the
  whitelisted elevated helper; per-icon mark bake with transparent global overlay
  (ADR-0006 facts).
- Desktop icon layout snapshot/restore, silent best-effort (never gates release).
- 保存对比图 share export; 还原快照导出.
- Settings page, About + changelog.
- Local-only: no account, no telemetry, no cloud — stated in the UI.

**Excluded from v1.0**: module rail + 系统净化 / 资源管理器 / 壁纸 modules (future
form, v1.1+ per roadmap); AI icon generation; real-time watcher (v1.1 opt-in);
installer beyond self-contained zip/exe; English UI as a gate; Win7.

> **v1.1 (ADR-0009)**: the module rail + 壁纸 module shipped early — spec 03
> (shell navigation: rail, title bar sheds ⚙/⋯, overflow merges into the drawer)
> and spec 04 (wallpaper module) supersede this section's window IA where they
> disagree. The ASCII frame below is the v1.0 historical shape.
>
> **Icons v2 (ADR-0015, 2026-07-09)**: icon styling is rendered by the web
> compositor (spec 06) — spec 06 supersedes this file's icon-editing interaction
> details (debounce, per-tile menus, size preview, arrow gate) where they
> disagree. The preview==desktop parity law below is amended by ADR-0015 D5
> (visual tolerance bands at 256, not bit-exactness).

## The Window (IA)

Three regions plus overlays — no navigation, no second page:

```
┌───────────────────────────────────────────────────────────────┐
│ 标题栏: ✦logo 桌面美颜 [v1.0] ······· ⚙ ⋯ ─ ▢ ✕                │
├───────────────┬───────────────────────────────────────────────┤
│ 控制面板 300px │ 桌面镜像画布 (真实壁纸 + 图标网格)              │
│  状态行        │   [紧凑模式: 摘要工具条在此]                    │
│  Hero 标题     │   图标 tiles (列优先流式，同真实桌面)           │
│  CTA 44px     │   「⇄ 按住对比原样」pill                        │
│  链接排:还原/  │   装饰任务栏 (开始/搜索/应用/实时时钟)           │
│   上一版/历史/  │                                               │
│   对比图       │                                               │
│  [版本历史卡]  │                                               │
│  风格 (2×2 卡) │                                               │
│  自定义 (4 行  │                                               │
│   手风琴)      │                                               │
└───────────────┴───────────────────────────────────────────────┘
Overlays: 设置抽屉(320) · ⋯菜单 · 关于/更新日志 · 调色盘(244) · 图标右键菜单 · Toast
```

**Compact mode** (window width below ~1100px): the control panel becomes a
left slide-in overlay with scrim; a summary toolbar appears above the canvas
(4 preset chips + 「自定义 ▸」 + compact CTA). Esc closes any overlay.

## Control Panel Contents

### Hero + CTA state machine (exact copy)

| State | 状态行 | Hero 标题 | CTA |
|---|---|---|---|
| Scanning | 正在扫描桌面… | 你的桌面，即将焕然一新 | 正在扫描… (disabled, raised) |
| Ready | 可以美化 N 个图标 · 全程可还原 | 你的桌面，即将焕然一新 | **一键美化** (solid coral) |
| Working | 可以美化 N 个图标 · 全程可还原 | 你的桌面，即将焕然一新 | 正在应用… (coral 75%, disabled) |
| Applied+dirty | 已美化 N 个图标 · 有新样式待应用 | 有新的样式待应用 | **更新桌面** (solid coral) |
| Applied+clean | 已美化 N 个图标 · 快照已保存 | 已经焕然一新 | ✓ 已与桌面同步 (raised, teal text, disabled) |

Link chips (visible once applied or history exists): 还原 (applied only) ·
上一版 (history permitting) · 历史 N (toggles the history card) · 对比图.

### 版本历史 card

Header 「版本历史 · 保留最近 10 版」. Rows: HH:mm · label (e.g.
「苹果 · 原彩 · 珐琅光弧」) · 「当前」 teal pill on the live top entry ·
「回到此版」 accent action. Footer row: 「最初 · Windows 原生桌面 · 回到最初」
(= full restore; history itself is preserved). Re-applying a version pushes a
new entry (stack semantics, cap 10). Label grammar = 外形 · 配色 · 标识
(标识 label = mark-style name, or 经典箭头/无标识).

### 风格 presets (2×2 cards, two live mini-previews each)

| Preset | Sets |
|---|---|
| 苹果极简 (统一圆角 · 原彩) | shape=苹果, colour=原彩, dist=美化 |
| 糖果彩 (饱满曲线 · 缤纷) | shape=三星, colour=原彩, dist=美化 |
| 纯净黑白 (安静的灰阶秩序) | shape=苹果, colour=黑白, dist=美化 |
| 壁纸同色 (取壁纸主色统一) | shape=苹果, colour=单色, tint=壁纸主色, dist=美化 |

Active preset = exact match on shape/colour/dist (+tint if set) — mark-style,
mark-colour and size changes do **not** deactivate it; any non-matching state
shows 「自定义中」 beside the 风格 label. Default on launch = 苹果极简.

### 自定义 accordion (4 rows, each: label · summary value · chevron; ＋/− expand-all)

- **外形**: 无/苹果/纯圆/三星/方块/水滴 curated + 「更多」 fold (11 shapes; catalog
  and geometry engine in spec 02 §Shape System).
- **配色** (two axes since 2026-07-09; detail in spec 02 §Colour Treatments): 原彩 /
  黑白 / 单色 with concentric fg/bg pair swatches; 单色 gains a 渐变/纯色 depth switch
  (纯色 = 极致单色); the row-end wheel opens a 前景/背景 dual-tab picker.
- **快捷方式标识**: 美化标识 / 经典箭头 / 无标识; when 美化 → 6 mark-style chips
  with 22px live previews (投影 / 光环 / 缎光角 / 珐琅光弧 / 卷角 / 细描边) +
  标识配色 row (自动 default · 5 swatches · 调色盘; hidden for 投影). 玻璃箭头 is
  not selectable after ADR-0010.
- **图标大小**: 小 / 中 / 大 (preview 52/64/76; desktop mapping in plan).

Every change: ~420ms debounce → in-place tile refresh + 「正在更新预览…」 cue.

### 调色盘 (one component, two consumers — ADR-0005 D3)

SV field + hue slider + hex input + screen eyedropper ⌖ + 「从壁纸自动提取」
4 swatches + 「快捷选择」 6 swatches. Consumers: icon 单色 tint · 标识配色.
Title switches 图标单色 / 标识配色.

## Canvas Behaviour

- Background = the user's real wallpaper (colour palette also extracted from it);
  icons arranged column-major like the real desktop, real labels, ellipsis.
- Scanning: shape-clipped shimmer tiles. Applying: bloom wave. Restore: settle.
- **Press-to-peek** per tile (hold shows that icon's original); **按住对比原样**
  pill flips the entire canvas to originals while held.
- **Right-click menu** per tile: header = item name; 保留原样 (✓ toggle);
  跟随全局样式; 单独配色 (6 swatches). Overrides live-update the preview and,
  when already applied, restyle that one icon immediately (journaled).
- Decorative taskbar strip completes the mirror illusion (real clock/date).
- Kept-original shortcuts always render the classic Windows arrow in preview,
  matching the real desktop.

## Settings Page · About

- **Settings page**: opened by the 设置 rail module, not a drawer. Groups:
  外观 (language + theme, both default 跟随系统) · 自动化 (新图标自动美化) · 本地数据
  (还原快照 / 前后对比图 / data folder) · 关于 (product identity, repo, author,
  检查更新, 联系反馈, 更新日志). Normal settings/about navigation does not use
  modals or scrims.
- **About**: hand-authored SVG-derived logo · 桌面美颜 / DeskMakeover · v1.0.0 · slogan · 5 positioning
  chips · GitHub card (`github.com/nicepkg/deskmakeover` — 开源地址 · 欢迎 Star) ·
  author card (小明 · XiaomingLab, 「同一件事做两次，就写个工具」; links: 个人主页
  xiaominglab.com / GitHub 2214962083 / X jinmingyang666 / 哔哩哔哩 space 83540912 /
  抖音) · buttons 检查更新 / 更新日志 · footer 「© 2026 XiaomingLab · Windows 10 / 11」.
  更新日志 is inline on the settings page.
- 检查更新 (v1.0): open the GitHub releases page; no auto-update framework.
  帮助与反馈: open GitHub issues.

## UI Language Rules (unchanged, still binding)

Banned in user-facing strings: 快照*, 应用计划, 扫描*, dry-run, 注册表, 缓存,
HKLM, journal, raw enums. (*Exceptions now product-official per the prototype:
「正在扫描桌面…」 and 「还原快照」 are approved copy; the ban still covers jargon
like 增量扫描/快照ID.) Domain enums never bind to XAML — presentation mappers
only. Skipped items get human reasons behind a low-key entry. Errors = what
happened / what changed / what next; tech detail behind an expander.

## Trust Flow (privileged work — unchanged)

Explain → one batched UAC → on denial apply all non-privileged styling, mark the
privileged step skippable/retryable, never dead-end. Light Explorer refresh
first; disruptive refresh only after telling the user. The helper stays a fixed
whitelist of named verbs.

## System Architecture

Projects (existing, retained):

| Project | Role |
|---|---|
| `DeskMakeover.App` | WPF UI (non-admin): window shell, control panel, canvas, overlays, theming, presentation mappers, MakeoverService orchestration |
| `DeskMakeover.Core` | Pure domain: DesktopItem, IconSource, IconStylePlan, **StylePreset (extended axes)**, OperationPlan, Snapshot, **LookVersion (history)** — no Win32/WPF |
| `DeskMakeover.IconRendering` | Raster pipeline: shape masks, colour treatments, the 7 mark styles, resampler, multi-size IcoWriter, GeneratedIconStore |
| `DeskMakeover.Shell` | Adapters: scanner, shortcut read/write COM, `.url`, Explorer refresh, restore metadata, **desktop icon size** |
| `DeskMakeover.Operations` | Journaled runner, planner, snapshot factory/store, **history ledger** |
| `DeskMakeover.ElevatedHelper` | Whitelisted privileged verbs, one-UAC batching |

**StylePreset axes (v1.0+)**: shape {apple, circle, samsung, none, google, brave,
bookmark, lemon, squircle, tile, teardrop, blob, rectellipse} × colour {orig, bw,
mono(+tint)} × distinction {mark, keep, none} × markStyle {card, echo, satin,
arc, fold, ring} × markColor {auto, RGB} × iconSize {small, mid, big}.
Presets are data; one rendering engine consumes plans (no per-style code paths
in callers). Per-icon overrides {keep | tint} ride alongside as an item-id map.

**Snapshot contents** add: current desktop icon-size/view state and the active
per-icon override map. **Restore** returns the desktop to baseline (icons, ico
cache, overlay registry, layout best-effort, icon size) and clears applied state;
version history persists across restore.

**Preview == desktop parity law**: the preview tile bitmap and the baked `.ico`
must come from the same rendering functions; only DPI/size selection differs.

## Safety Rules (unchanged)

No snapshot, no apply · no silent wrapping · no destructive deletion · no hidden
network (the only network actions are user-clicked GitHub links) · no silent
Explorer kill · OneDrive warnings · uncertain restore → stop and show recovery ·
skipped items visible with reasons.

## Verification Strategy

Unit/integration (existing 100+ tests stay green, extended): shape-mask geometry
(3 shapes), colour treatments incl. 纯黑/纯白 edges, all 7 mark styles (adaptive
tone + no-spill + user-colour), resampler ladder, StylePreset resolution/dirty
semantics, history ledger cap/ordering, override map, journaled apply/rollback,
snapshot roundtrip incl. icon size, presentation mapping, exporter.

Manual matrix on clean VMs (Win10 + Win11, multi-monitor/mixed DPI, OneDrive
on/off, auto-arrange on/off, UAC denial mid-flow, interrupted apply): plus the
**prototype parity audit** — side-by-side screenshots of every region and state
against the prototype in a browser (the release gate; checklist lives in the
plan).

Build/release verification commands: `dotnet build` (0 warnings) · `dotnet test`
· `node scripts/publish-win.mjs` · fresh-VM smoke (apply → reboot → restore,
zero residue).
