# 0004. Platform Form: One Verb, Many Battlefields

**Status:** accepted
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
