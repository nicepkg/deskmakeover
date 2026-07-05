# DeskMakeover Roadmap

Living document: edit in place as versions ship; history lives in CHANGELOG + git
tags. Version scopes were set by ADR-0002/0003 and the 2026-07-05 expert panel.
The rule for all future capabilities (ADR-0002): fold into the default result or
live in settings — the primary flow never gains steps.

## v0.9 — 抢发 (catch the traffic) · days

**Goal:** the smallest trustworthy release a novice can double-click, so the
comment-section demand lands on us instead of the open-source script.

- Dark single-screen UI: before/after mirror + Makeover Switch + status line.
- One excellent default style (Apple continuous-corner squircle, auto background).
- Refined Mark overlay badge (default) + three-state badge choice (refined /
  clean / original) as visual thumbnails.
- Auto snapshot → journaled apply → **zero-residue one-click restore** (= switch off).
- Trust flow for the one UAC elevation; denial degrades gracefully.
- OneDrive/redirected desktop resolution; skip cloud placeholders.
- `StylePreset` parameter pack wired through the renderer (single preset shipped).
- Signed binaries (OV/individual cert), app.manifest (PerMonitorV2, long paths, UTF-8).
- Self-contained publish + downloadable package.

**Explicitly absent:** resident watcher, filter bar, baked badge, layout matrix,
light theme polish, installer wizard.

**Exit gate:** tests green; fresh-VM smoke (apply → reboot → restore, zero
residue); signed exe passes SmartScreen without red interstitial.

> Platform decisions (ADR-0004): one hero switch forever; modules are an
> exclusion checklist, not a toolbox; risk tiers cold/warm/hot with the global
> switch never touching hot; module constitution bans cleaners/accelerators and
> global file-type association icons permanently.

## v1.0 — 站稳 (soul) · weeks

- Filter bar: three live-preview preset pills — 苹果圆角 (default) / 极简描边 /
  单色滤镜. Zero sliders. Presets are `StylePreset` data.
- The Moment: bloom-wave apply animation, press-to-peek original, restore settle,
  skeleton shimmer, reduced-motion fallbacks.
- Baked top-right premium badge (size-adaptive: simplified <32px) as the refined
  mark's upgrade; overlay suppressed automatically when baked badge is active.
- Light theme parity + follow-system option surfaced in settings.
- "保存对比图" share hook on the done state (growth engine).
- Accessibility pass: AutomationProperties, live regions, high-contrast fallback.
- Keep-up v1: logon-triggered run-and-exit task + app-launch catch-up
  (switch semantics without residency).
- **System-ads module (ADR-0004): HKCU one-shot warm-tier toggles** (Start
  recommendations, Spotlight tips, Explorer promotions, suggestions,
  advertising ID, search highlights), disclosed in plain language, inside the
  hero switch's default package; no elevation, no residency.

## v1.1 — 信任 (boundaries) · weeks

- Real-time auto-styling watcher: explicit opt-in, default off, visible
  exit/uninstall, per-path debounce + self-trigger suppression, desktop-dirs-only.
- Persistent baseline snapshot + append-only per-item undo ledger for continuous
  keep-up (no micro-snapshot explosion).
- AV whitelist submissions (Microsoft / 360 / 火绒) + false-positive playbook.
- Identification edge coverage: UWP/Store, special system icons, no misfires.
- Multi-monitor / mixed-DPI / layout-restore hardening; installer packaging;
  emergency restore entry point.

## v1.2 — 第二战场 (platform proof) · weeks

- `Modules.Contracts` + module host refactor (spine landed in v1.1) carries its
  first new battlefields: **folder styles (desktop.ini) and drive icons
  (registry DriveIcons only, never autorun.inf)**.
- Module-row checklist UI appears (exclusion grammar per ADR-0004).
- Global file-type association icons: **permanently out** (constitution).

## v2.0 — 代差 (moat) · months

- AI icon generation (`IIconGenerator` extension point already reserved).
- Whole-desktop unified color-filter styles; style packs.
- Baked badge design evolution; EV certificate upgrade when volume justifies.

## Standing open questions

- Signing entity/name for the OV certificate (owner).
- Distribution channel for v0.9 (direct download + pinned comment reply).
