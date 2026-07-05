---
updated: 2026-07-05
version: unreleased
branch: main
---

# State

## Active Work

- v0.9 build per [plans/2026-07-05-ui-ux-v2.md](plans/2026-07-05-ui-ux-v2.md)
  (as revised by ADR-0003) and [specs/00-roadmap.md](specs/00-roadmap.md).
- Tasks 1–7 substantially landed: platform foundation, squircle system,
  StylePreset + presentation model, copy rewrite, Makeover Switch main screen
  (MainViewModel state machine, consent card, restore confirm, press-to-peek),
  MakeoverService orchestration (auto snapshot → journaled apply → overlay →
  light refresh; zero-residue restore incl. cross-session via active-makeover.json),
  elevated helper verbs (apply-overlay --style refined|transparent /
  restore-overlay, self-persisted original state in ProgramData), refined-mark
  multi-size ico factory.
- Smoke-verified on the real desktop (read-only path): dark stage, switch,
  badge pills, 20 styleable items rendered as squircle tiles with auto
  backgrounds, humanized Chinese statuses. Screenshot evidence in session.

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

0. **App-shell round (owner demand: 简洁不是简陋, product not demo):**
   settings flyout per spec 01 IA (gear: theme dark/light/system, language,
   keep-up toggle, wrap-files opt-in, backup folder, diagnostics export),
   About section (author/版本/主页/开源声明), title area polish, visual
   hierarchy pass, and the extension seams already specced (filter-bar slot
   under the mirror for v1.0 presets; settings as the future-feature home per
   ADR-0002 rule). Renderer double-tile + jaggies fixed this round
   (full-plate detection + AA mask + bilinear).
1. Fix: first tile row renders clipped at the top of the mirror grid (visual bug
   seen in smoke run).
2. Task 8 motion: bloom wave on apply, skeleton shimmer while scanning, restore
   settle, reduced-motion fallbacks.
3. Tests: MakeoverService apply/restore with fakes + temp-dir `.url` round trip;
   badge outcome mapping. (Current orchestration landed ahead of its tests —
   acknowledged debt against the plan's TDD intent.)
4. Supervised live run of switch-on → UAC → switch-off on the owner's machine
   (restore immediately), then Task 9: full sweep + adversarial review + publish.
5. Badge pills → real three-state thumbnails (spec 01) — v1.0 polish if not in v0.9.
6. Owner: purchase OV/individual code-signing certificate (v0.9 gate).

## Blockers

- No blocker. GitHub CLI not in PATH; no remote repository yet.

## Open Questions

- Signing entity/name for the OV certificate (owner decision, purchase pending).
- v0.9 distribution channel details (direct download + pinned comment reply).
