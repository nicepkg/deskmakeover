# DeskMakeover Roadmap

Living document: edit in place as versions ship; history lives in CHANGELOG + git
tags. Scopes set by ADR-0002/0003/0004/0005/0008. Standing rule (ADR-0002): every
new capability folds into the default result or lives in settings — the primary
flow (one CTA) never gains steps. **ADR-0008 renumbered the release train: the
first public release is v1.0 (was "v0.9 抢发").**

## v1.0 — 原型复刻 (prototype parity, icons only) · in progress

**Goal:** the first public release replicates
`docs/references/prototype/桌面美颜 v2.dc.html` (today form) completely — a
one-screen, fully reversible desktop-icon beautifier a novice can double-click.
No new functional territory beyond icon beautification.

**Scope (spec 01):** left-panel + desktop-mirror layout with compact overlay
mode; 4 风格 presets; 自定义 accordion (expanded 外形 · 配色 3+tint · 快捷方式标识
3-state/6-mark-style/mark-colour + classic arrow · 图标大小 3); version history (10) + 上一版 +
回到最初; per-icon right-click overrides; hold-to-compare + press-to-peek;
dirty-state 更新桌面 with in-place refresh; one shared 调色盘 (SV+hue+hex+
eyedropper+wallpaper palette); settings page / About(开源) /
changelog; keep-up run-and-exit; journaled apply + zero-residue restore; per-icon
mark bake with transparent global overlay; multi-size ico ladder; motion suite +
reduced-motion; zh-Hans/en localization with theme/language following the OS by
default.

**Execution:** `plans/2026-07-06-v1-prototype-parity.md` (handed to an executing
AI; prototype = binding contract).

**Exit gate:** tests green (0 warnings) · prototype parity audit (side-by-side,
every region/state) · fresh-VM smoke (apply → reboot → restore, zero residue) ·
supervised live switch-on → UAC → switch-off on the owner's machine · signed exe
passes SmartScreen without a red interstitial (OV/individual cert — owner).

> Platform guardrails (ADR-0004) stand: one hero action forever; modules are an
> exclusion checklist, not a toolbox; risk tiers cold/warm/hot with the global
> action never touching hot; cleaners/accelerators and global file-type
> association icons permanently banned.

## v1.1 — 侧栏 + 壁纸模块 + Web UI 重平台 · in progress (ADR-0009/0011)

- **Module rail unlocked now** (owner order 2026-07-07, overrides the "rail waits
  for 4+ modules" note): 66px rail with 图标 / 壁纸 + a bottom 设置 utility; the
  title bar sheds ⚙/⋯ and settings is a normal rail page, not a drawer
  (spec 03, ADR-0010).
- **美化桌面壁纸 1.0** (spec 04): pale-wallpaper 清晰度 enhancement (auto-detect
  + 关/柔和/强) + partition zones baked into the wallpaper — semantic cell-grid
  zones + environment fingerprint (regenerate on mismatch), full
  `IDesktopWallpaper` snapshot/restore, primary monitor only, **no icon
  auto-placement** (v1.2 candidate), no watermark, bundled handwritten title font.
- Execution: `plans/2026-07-07-rail-and-wallpaper.md`.
- **UI replatform (ADR-0011, owner order 2026-07-08)**: the entire visible UI
  moves to WebView2 + React 19 + Tailwind 4 + shadcn/ui + Motion (Bun-only
  toolchain, no Node) before the first public release; the C# engine and the
  WYSIWYG law are untouched. Architecture + bridge contract: spec 05.
  Execution: `plans/2026-07-08-webview2-react-migration.md`. The old WPF UI
  layer is deleted after parity (nothing is released yet — no compat).

## v1.2 — 信任 + 净化模块 · weeks

- **系统净化 module** (HKCU one-shot, warm tier, no elevation): start-menu
  recommendations, lock-screen Spotlight tips, Explorer promotions, settings
  suggestions, advertising ID, search highlights. UI = the prototype's future-form
  净化 view (toggle list + 并入一键美化 + honest footer 「不是清理软件…」).
  Module list / 「已帮你做的事」 checklist grammar arrived with the rail
  (ADR-0004 as amended by ADR-0009).
- 「整理到分区」 icon auto-placement experiment (explicit, previewable,
  journaled `SelectAndPositionItems`) — candidate, per ADR-0009 §6.
- `Modules.Contracts` + module host refactor lands **before** 净化 ships —
  v1.1's rail deliberately used a lightweight two-view switch (ADR-0009).
- Trust hardening: AV whitelist submissions (Microsoft/360/火绒); opt-in real-time
  watcher with visible exit + tray presence; multi-monitor/mixed-DPI edge
  coverage; installer + emergency restore entry.
- English localization if demand shows.

## v1.3 — 第二战场 (Explorer) · weeks

- **资源管理器 module** per the prototype future-form view: folder icons
  (desktop.ini) + drive icons (registry DriveIcons only — autorun.inf never),
  same shape×colour engine, per-surface toggles, full restore.
- Global file-type-association icons: **permanently out** (constitution).

## v2.0+ — 代差 (moat) · months

- **壁纸 curated sets** (prototype future-form view): the local 壁纸 module
  shipped in v1.1 (spec 04); this expands it with curated wallpaper sets matched
  to the icon look — auto-backup + one-click revert, never替换 without consent.
- AI icon generation (`IIconGenerator` extension point reserved); whole-desktop
  unified colour-filter style packs; EV cert when volume justifies.

## Standing open questions

- Signing entity/name for the OV certificate (owner; v1.0 gate).
- Create the public GitHub repo `nicepkg/deskmakeover` the About panel links to
  (owner; needed before release since the About card and 免费开源 chip promise it).
- v1.0 distribution channel (direct download + pinned comment reply).
