---
updated: 2026-07-05
version: unreleased
branch: main
---

# State

## Active Work

- v0.9 build per [plans/2026-07-05-ui-ux-v2.md](plans/2026-07-05-ui-ux-v2.md)
  (as revised by ADR-0003) and [specs/00-roadmap.md](specs/00-roadmap.md).
- Done so far: Task 1 (app.manifest ×2, Fluent dark theme, ThemeManager, DWM
  interop, token dictionaries) and squircle geometry + 8 tests. Next: Task 2
  remainder (SquircleBorder control, button styles), then Tasks 3–9.

## Last Done

- VOC v1 (launch-video audience feedback) analyzed; second expert-panel round run.
- Owner resolved four revisions → [ADR-0003](decisions/0003-voc-driven-product-revisions.md):
  Makeover Switch as primary control, Refined Mark badge default (three states),
  preset ladder (v0.9 single / v1.0 filter bar, `StylePreset` param pack now),
  OV code signing as v0.9 entry ticket.
- Roadmap written: [specs/00-roadmap.md](specs/00-roadmap.md) (v0.9 抢发 / v1.0
  站稳 / v1.1 信任 / v2.0 代差).
- Specs 01/02 updated (switch flow, badge system, preset axes, keep-up model,
  tone rule).
- Verification: build 0 warnings/errors; squircle geometry tests 8/8 green.

## Next

1. Task 2 remainder: `SquircleBorder` control + custom button styles (Controls.xaml).
2. Task 3: `StylePreset` value object; presentation model (original+styled pairs,
   humanized enum mapping).
3. Tasks 4–9 per revised plan (copy, main screen with switch, orchestration,
   helper overlay refined/transparent, motion, verification sweep).
4. Owner: purchase OV/individual code-signing certificate (v0.9 gate).

## Blockers

- No blocker. GitHub CLI not in PATH; no remote repository yet.

## Open Questions

- Signing entity/name for the OV certificate (owner decision, purchase pending).
- v0.9 distribution channel details (direct download + pinned comment reply).
