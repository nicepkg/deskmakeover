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
mode; 4 风格 presets; 自定义 accordion (外形 3 · 配色 3+tint · 快捷方式标识
3-state/7-mark-style/mark-colour · 图标大小 3); version history (10) + 上一版 +
回到最初; per-icon right-click overrides; hold-to-compare + press-to-peek;
dirty-state 更新桌面 with in-place refresh; one shared 调色盘 (SV+hue+hex+
eyedropper+wallpaper palette); settings drawer / overflow / About(开源) /
changelog; keep-up run-and-exit; journaled apply + zero-residue restore; per-icon
mark bake with transparent global overlay; multi-size ico ladder; motion suite +
reduced-motion.

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

## v1.1 — 信任 + 第二模块 · weeks

- **系统净化 module** (HKCU one-shot, warm tier, no elevation): start-menu
  recommendations, lock-screen Spotlight tips, Explorer promotions, settings
  suggestions, advertising ID, search highlights. UI = the prototype's future-form
  净化 view (toggle list + 并入一键美化 + honest footer 「不是清理软件…」).
  Module list / 「已帮你做的事」 checklist grammar appears with module #2
  (ADR-0004; the icon rail waits for 4+ modules).
- `Modules.Contracts` + module host refactor lands **before** module #2 ships.
- Trust hardening: AV whitelist submissions (Microsoft/360/火绒); opt-in real-time
  watcher with visible exit + tray presence; multi-monitor/mixed-DPI edge
  coverage; installer + emergency restore entry.
- English localization if demand shows.

## v1.2 — 第二战场 (Explorer) · weeks

- **资源管理器 module** per the prototype future-form view: folder icons
  (desktop.ini) + drive icons (registry DriveIcons only — autorun.inf never),
  same shape×colour engine, per-surface toggles, full restore.
- Global file-type-association icons: **permanently out** (constitution).

## v2.0+ — 代差 (moat) · months

- **壁纸 module** (prototype future-form view): curated wallpaper sets matched to
  the icon look, auto-backup + one-click revert, never替换 without consent.
- AI icon generation (`IIconGenerator` extension point reserved); whole-desktop
  unified colour-filter style packs; EV cert when volume justifies.

## Standing open questions

- Signing entity/name for the OV certificate (owner; v1.0 gate).
- Create the public GitHub repo `nicepkg/deskmakeover` the About panel links to
  (owner; needed before release since the About card and 免费开源 chip promise it).
- v1.0 distribution channel (direct download + pinned comment reply).
