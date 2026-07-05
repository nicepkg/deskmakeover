---
updated: 2026-07-05
version: unreleased
branch: main
---

# State

## Active Work

- v0.9 build per [plans/2026-07-05-ui-ux-v2.md](plans/2026-07-05-ui-ux-v2.md)
  and [specs/00-roadmap.md](specs/00-roadmap.md). Platform vision in
  [ADR-0004](decisions/0004-platform-form.md) (ads module deferred by owner).
- v0.9 feature-complete for the desktop-icon module. Handed to Codex for
  two-stage adversarial review (Phase 6).

## Last Done

- Full product shell built and screenshot-verified aesthetically: title bar
  (dogfooded squircle app icon + gear + overflow), hero (glowing Makeover Switch
  + badge pills), decluttered squircle tile grid (state caption only for
  exceptions), right slide-in settings drawer (segmented theme picker, iOS
  toggle, backup/about rows), Apple-"About this Mac"-quality about panel, toast,
  all custom squircle dialogs (no native MessageBox).
- Motion: bloom wave on apply, skeleton shimmer while scanning, hover lift,
  press-to-peek, panel slide/scale, toast slide-fade, load cross-fade, restore
  settle — all reduced-motion aware.
- Fixed: Mica washed out opaque content (base brush moved to inner canvas; Mica
  disabled pending proper backdrop handling); first-row tile clipping; renderer
  double-tile + jaggies (full-plate detection + AA mask + bilinear); latent
  crash where the non-privileged op factory threw on the privileged overlay step.
- Tests: +6 MakeoverService apply/restore roundtrip (real temp `.url`).
- Verification: `dotnet build` 0 warnings/0 errors; `dotnet test` **98 passed**;
  `node scripts/publish-win.mjs` OK; **published self-contained exe smoke-rendered
  correctly in a fresh run**.

## Next

1. Codex two-stage adversarial review (spec-compliance then code-quality) — in
   progress; fix findings.
2. Supervised live run of switch-on → UAC → switch-off on the owner's machine
   (restore immediately) — the one path not yet exercised end-to-end (real
   `.lnk` COM write + HKLM overlay + Explorer refresh).
3. Owner: purchase OV/individual code-signing certificate (v0.9 release gate).
4. Deferred to v1.0: badge pills → real three-state thumbnails; filter-bar
   presets; baked badge; share hook; light-theme visual pass.

## Blockers

- No blocker. GitHub CLI not in PATH; no remote repository yet.

## Open Questions

- Signing entity/name for the OV certificate (owner decision, purchase pending).
- v0.9 distribution channel details (direct download + pinned comment reply).
