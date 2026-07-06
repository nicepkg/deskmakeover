---
updated: 2026-07-06
version: unreleased (target v1.0.0)
branch: main
---

# State

## Active Work

- **v1.0 prototype-parity rebuild** (ADR-0008): the owner rejected the iterated
  UI and produced a complete Claude Design prototype —
  `docs/references/prototype/桌面美颜 v2.dc.html` — which is now the **binding
  UI/UX contract**. All docs were rewritten against it (2026-07-06):
  specs 00/01/02, ADR-0008 (+ status amendments to 0005/0006/0007), READMEs.
- Execution plan for a separate AI:
  **`docs/plans/2026-07-06-v1-prototype-parity.md`** — phased, task-level,
  with prototype line references and per-phase acceptance gates. Start there.

### Rebuild progress (execution log)

- **P0 done (2026-07-06)**: baseline `dotnet build` = 0 warn / 0 err; full suite
  green (118 tests: Core 7 · Shell 26 · Operations 15 · IconRendering 28 · App 42).
  Whole prototype (1724 lines HTML + support.js runtime) read end-to-end — every
  metric/colour/algorithm/copy string extracted. Prototype runtime pulls
  React/Babel from unpkg CDN, so parity is **code-driven** (deterministic from
  source) + verified against WPF screenshots via the new
  `scripts/dev/capture-window.ps1` win-only capture helper. Before-shot of the
  rejected UI captured. Next: P1 design tokens.

## Owner rules (durable)

- Accent is warm coral `#FF6F5E`; **never blue/violet** (reads as AI slop).
- Extreme DRY; files ≤500 lines; aesthetic calls go through VOC + panel + the
  3-second-misread gate, not engineer fiat.
- The prototype wins every conflict with prose docs; when unsure, open it in a
  browser and compare (`open "docs/references/prototype/桌面美颜 v2.dc.html"`).
- v1.0 adds **no functional territory beyond icon beautification** — the
  future-form modules (净化/文管/壁纸) are design-locked IA for v1.1+.

## What exists and works (keep, don't rebuild)

- Foundation: domain models, scanner, snapshots, journaled ops, elevated helper
  with single-UAC batching, keep-up (logon task + catch-up), layout best-effort
  save/restore. ~105 tests green; `dotnet build` 0 warnings.
- Rendering: superellipse mask engine, background classifier, `IsRoundish`,
  hi-res 256px extraction, multi-size ico ladder (16–256, premultiplied
  box-average resampler), per-icon mark bake + transparent global overlay
  (ADR-0006 facts), GeneratedIconStore.
- Wallpaper colour extraction, HSV ColorPicker component (SV+hue+hex+eyedropper),
  comparison-image exporter, theme manager (dark/light/system), toast/dialog
  chrome, reduced-motion awareness.
- The current UI shell/layout/controls are **superseded** by the prototype and
  will be rebuilt per the plan (keep the services/pipeline underneath).

## Next

1. Execute the plan phases in order (docs/plans/2026-07-06-v1-prototype-parity.md),
   each phase gated by its acceptance checks + screenshot evidence.
2. Prototype parity audit (plan P14) before any release claim.
3. Owner actions: purchase OV/individual signing cert; create public
   `github.com/nicepkg/deskmakeover` repo (About panel links to it).
4. Supervised live switch-on → UAC → switch-off run on the owner's machine —
   still the one path never exercised end-to-end.

## Blockers

- None for development. Release gates: signing cert (owner), public repo (owner).
- GitHub CLI not in PATH; no git remote configured yet.

## Open Questions

- Signing entity/name for the OV certificate (owner decision, purchase pending).
- v1.0 distribution channel details (direct download + pinned comment reply).

## History (compressed)

- 2026-07-05: MVP foundation (60→105 tests), product shell v1, ADR-0001..0004.
- 2026-07-06 rounds 1–6: composable shape×colour×distinction engine, ColorPicker,
  responsive grid, multi-size ico, real-desktop mark bake, codex adversarial
  review (10 findings fixed), three-band layout, distinction iterations
  (arc→arrow→stacked cards→gallery) — owner rejected each UI round.
- 2026-07-06 (this session): owner shipped the v2 prototype; ADR-0008; full doc
  realignment; v0.9→v1.0 renumbering; big rebuild plan written.
