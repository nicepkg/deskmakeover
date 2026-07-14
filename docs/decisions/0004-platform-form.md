# 0004. Platform Form: One Verb, Many Battlefields

**Status:** accepted — amended by [ADR-0023](0023-calm-windows-module.md) (§3 HealthCheck scope
narrowed · §5 gained a guided exception · §6 system-purification timing superseded). See the
amendment at the end of this file.
**Date:** 2026-07-05

## Context

The owner expanded the product vision beyond desktop icon styling: disable Windows
system ads, beautify File Explorer icons, and further experience refinements. A
third expert-panel round (product / UX / visual / Windows engineering) examined how
to grow into a platform without becoming the bundleware ("全家桶") this product
exists in opposition to. The owner resolved the three open conflicts.

## Decision

1. **Identity.** The name 桌面美颜 stays through v1.x. The unifying narrative is
   "让 Windows 回到它本该的样子" — a self-pleasing space ("悦己空间"), never a
   butler/管家. Ad removal is framed as removing visual noise (part of the
   makeover), not as a cleaner capability.
2. **IA: the synthesized one-switch model.** The main screen keeps exactly one
   hero switch. Its default package = cold-tier modules plus disclosed warm-tier
   ones. Once on, modules appear as a "已帮你做的事" checklist rendered in
   module-row grammar (icon + name + status line + per-row exclusion toggle) —
   exclusion, not selection. Hot-tier items live only in an 进阶 area with
   per-item informed consent. New modules are discovered after the first success,
   never as a first-screen menu. A thin icon sidebar may appear only at 4+
   modules. First-screen decisions ≤ 1; outcome-named groups ≤ 3.
3. **Module contract.** Every capability implements `IBeautifyModule`:
   Probe → Preview → CaptureSnapshot (namespaced into a shared snapshot) →
   journaled Apply → idempotent zero-residue Restore → **HealthCheck** (drift
   detection; Windows updates roll changes back, re-apply is a platform
   capability). Metadata: RequiresElevation, Scope (HKCU/HKLM), Reversibility,
   RiskTier, SupportedEditions. Operations.Engine becomes the module host;
   all privileged steps across modules batch into one helper call / one UAC.
   The helper stays a fixed whitelist of named, validated verbs — never a
   generic registry RPC.
4. **Risk tiers.** Cold (pure rendering, user-space) / Warm (toggles Microsoft
   itself exposes; HKCU, no elevation) / Hot (deep registry, resident self-heal).
   **The global one-click never touches the hot tier.**
5. **Module constitution (admission test, all mandatory):** one-click reversible
   with zero residue; local-only, no account, no telemetry; adds no step to the
   primary flow; additive/backstop tone (never "扫描到 X 个问题" anxiety
   language); no destructive or unverifiable pseudo-features (cleaner /
   accelerator / memory optimizer are constitutionally banned). Taste is defined
   by what we refuse.
6. **System-ads module ships in v1.0 as the HKCU one-shot version** (Start menu
   recommendations, lock-screen Spotlight tips, Explorer sync promotions,
   settings/notification suggestions, advertising ID, search highlights — all
   per-user, no elevation, home-edition compatible). Machine-level CloudContent
   policies (HKLM, ignored on Home) are a later advanced option. Resident
   self-heal watching stays opt-in and out of the minimal release. Restore
   distinguishes original value absent/0/1 and never overwrites third-party
   drift (mark, don't clobber).
7. **Explorer icons boundary.** Folder styles (`desktop.ini`) and drive icons
   (registry DriveIcons; never autorun.inf) are in scope for v1.x. **Global
   file-type association icons (ProgId DefaultIcon) are constitutionally banned**
   — the one feature that can break a machine (association hijack signature,
   un-restorable merge states).

## Consequences

- Roadmap re-sliced (specs/00-roadmap.md): v1.0 gains the ads module (HKCU
  one-shot); v1.2 gains folder/drive icon modules; file-type icons removed
  permanently; resident watcher remains v1.1+ opt-in.
- The engine refactor to `Modules.Contracts` + module host is scheduled before
  module #2 lands (v1.1 spine work), not during v0.9.
- The UI keeps the v0.9 single-module layout; the module-row grammar and
  checklist appear when module #2 ships.
- EV signing upgrade becomes more important as modules multiply; whitelist
  submissions (MSRC/360/火绒) become ongoing ops.

## Amendment (2026-07-13 — ADR-0023, calm-Windows panel)

- **§6's "ships in v1.0" timing is SUPERSEDED.** The 2026-07-11 research
  (`docs/references/windows-settings-rust/`) re-tiered §6's item list (lock-screen tips are
  ADVANCED with an empty allowlist; widgets surfaces are GUIDED-only; every direct write
  needs per-environment certification, manifest initially empty), falsifying the "clean HKCU
  one-shot" premise. Release timing is now **capability-gated** (ADR-0023 decision 6): the
  certified 2-4-item write slice rides the first release iff the Windows-VM lab turns green
  during the Windows integration phase; otherwise v1 ships the guided-only face and writes
  follow certification. The capability boundary of record is the research README + ADR-0023.
- **§5 admission test — guided exception (written):** "adds no step to the primary flow"
  binds the hero one-click, which covers automatic-certified switches ONLY; guided surfaces
  are optional user-chosen walkthroughs, never auto-included in the one-click, never counted
  in 「已帮你做的事」.
- **§3 HealthCheck semantics for the calm-settings module = re-detect + re-PROPOSE** (never
  silent auto-replay across a feature-update boundary; in-boundary drift may re-close with an
  honest notice). §3's generic "re-apply is a platform capability" wording stands for pixel
  modules (icons/wallpaper) only.
- The module is named **清爽 / 清爽系统** (ADR-0023 decision 7); 净化/清理/优化 are banned
  copy for it. §2's "已帮你做的事" checklist grammar is refined by ADR-0023 decision 3
  (three-state honest results; verified writes only).
