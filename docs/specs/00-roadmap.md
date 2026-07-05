# DeskMakeover Roadmap

Living document: edit in place as versions ship; history lives in CHANGELOG + git
tags. Scopes set by ADR-0002/0003/0004/0005 and the 2026-07 expert panels.
Standing rule (ADR-0002): every new capability folds into the default result or
lives in settings — the primary flow (one Makeover Switch) never gains steps.

## v0.9 — 抢发 (catch the traffic) · in progress

**Goal:** the smallest trustworthy release a novice can double-click, so the
comment-section demand lands on us instead of the open-source script.

**Built (verified by tests + screenshots):**
- Warm-coral (`#FF6F5E`), dark-default product shell: title bar (real app logo +
  gear + overflow), hero (glowing Makeover Switch + promise + done-state share),
  grid region with left-aligned header + toolbars, right slide-in settings drawer,
  Apple-"About"-quality about panel, toast, all-custom squircle dialogs.
- **Composable 外形 × 配色 × 区分** (ADR-0005):
  - Shape: 苹果 squircle / 纯圆 (round icons untouched) / 三星, one superellipse engine.
  - Colour: 原彩 / 极简 B&W / 单色 (wallpaper-extracted colour + system accent +
    curated swatches + hex entry).
  - Distinction: 3-state 美化(default)/保留/去除 — 美化 = contrast-adaptive coral
    **enamel arc** grown into the icon's own bottom-left edge (珐琅/缎带/票根).
- Crisp hi-res icon extraction (256px); folder/file previews match Explorer exactly.
- Auto snapshot → journaled apply → **zero-residue one-click restore**; multi-snapshot
  keep-up; single batched UAC via the whitelisted elevated helper.
- Motion (bloom wave / skeleton / hover / peek / panel / toast / cross-fade / settle),
  reduced-motion aware. Light theme + follow-system. "保存对比图" share export.
- Self-contained win-x64 publish + app.ico/manifest.

**Remaining v0.9 gates:**
- Distinction-mark colour picker (reuse the one colour-picker component — DRY).
- App logo rework (owner: current one is weak) via /gpt-image-2.
- OV/individual code-signing cert (owner action); fresh-VM smoke; supervised live
  switch-on → UAC → switch-off run.

**Exit gate:** tests green; fresh-VM smoke (apply → reboot → restore, zero residue);
signed exe passes SmartScreen without a red interstitial.

> Platform guardrails (ADR-0004): one hero switch forever; modules are an
> exclusion checklist, not a toolbox; risk tiers cold/warm/hot with the global
> switch never touching hot; the module constitution permanently bans
> cleaners/accelerators and global file-type-association icons.

## v1.0 — 站稳 (soul) · weeks

- Named one-tap **preset combos** (shape+colour+distinction) so novices skip the
  three axes; per-icon override (right-click: keep as-is / change).
- Baked premium mark evolution; accessibility pass (AutomationProperties, live
  regions, high-contrast).
- Keep-up hardening: persistent baseline + append-only per-item undo ledger.
- **系统广告/推荐关闭 (module 1, HKCU one-shot, warm tier)** — see Future §A.

## v1.1 — 信任 (boundaries) · weeks

- Opt-in real-time watcher (default off, visible exit) + **tray presence** as the
  face of resident mode (状态 / 一键还原 / 暂停 / 退出并移除常驻). No tray, no
  background process while off.
- AV whitelist submissions (Microsoft / 360 / 火绒); identification edge coverage
  (UWP/Store, special icons, no misfires); multi-monitor / mixed-DPI hardening;
  installer + emergency restore.

## v1.2 — 第二战场 (platform proof) · weeks

- `Modules.Contracts` + module host carries its first new battlefields:
  **File-Explorer visual optimisation** — folder styles (desktop.ini) and drive
  icons (registry DriveIcons only, never autorun.inf). See Future §C.
- Module-row exclusion checklist UI (ADR-0004 grammar).
- Global file-type-association icons: **permanently out** (constitution).

## v2.0+ — 代差 (moat) · months

- AI icon generation (`IIconGenerator` extension point reserved).
- **Wallpaper beautification** (Future §B). Whole-desktop unified colour-filter
  style packs. Baked-mark evolution; EV cert when volume justifies.

## Future battlefields (owner-directed, discussed — not yet scheduled to a firm date)

The product is not just a desktop-icon beautifier — it's a **Windows visual-experience
beautifier**. Each battlefield below stays inside the ADR-0004 identity ("clean up
Windows' visual mess") and constitution (no bundleware, no cleaners/accelerators).

### A. 关闭 Windows 广告 / 咨询推送 (v1.0 first slice, then extend)
Turn off the ad/recommendation/news surfaces Windows injects — all **HKCU one-shot,
warm tier, no elevation, no residency**, disclosed in plain language:
- Start-menu recommendations, Settings/Spotlight tips, Explorer promotions,
  "suggested content", advertising ID, search highlights.
- Taskbar/lock-screen **news & interests / 资讯与兴趣 (咨询) widget**, weather feed,
  and Widgets board promotions.
- Grouped as an opt-in module inside (or beside) the hero switch's default package;
  fully reversible.

### B. 壁纸美化 (wallpaper beautification, v2.0 direction)
Beautify/curate the desktop wallpaper as part of the unified look — e.g. tasteful
wallpaper sets, colour-matched-to-icons backgrounds, or subtle treatment — so icons
and wallpaper form one coherent aesthetic. Read-only/reversible; never hijacks the
user's own wallpaper without consent.

### C. 文件管理器视觉优化 (File Explorer, v1.2 direction)
Extend the unified-mask idea beyond the desktop into File Explorer: beautify folder
and drive icons (desktop.ini / DriveIcons), and unify the icon look inside Explorer
windows — the same "system-level unified mask" value proposition, second surface.

## Standing open questions

- Signing entity/name for the OV certificate (owner).
- Distribution channel for v0.9 (direct download + pinned comment reply).
