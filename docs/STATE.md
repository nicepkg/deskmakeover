---
updated: 2026-07-05
version: unreleased
branch: main
---

# State

## Active Work

- v0.9 desktop-icon module: feature-complete + heavily polished from owner
  feedback. Platform vision in [ADR-0004](decisions/0004-platform-form.md)
  (ads module deferred by owner).
- Now a genuinely composable "make my icons beautiful" tool: pick a style
  (原彩 full-colour / 极简 black&white / 单色 tinted) × a tint colour
  (wallpaper-extracted + accent + curated palette + hex input) × a shortcut
  badge (精致标记 with arrow/fold/dot glyphs, 干净无标记, or 保持原样).

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

## Owner-feedback polish landed (2026-07-05→06)

- **Crisp icons**: PrivateExtractIcons 256px hi-res + HighQuality scaling (was a
  blurry 32px upscale).
- **Don Norman hero re-layout**: one accent (the switch is the sole glow), grid
  region with left-aligned header + style toolbar, badge moved off the first
  screen. First-screen decisions back to 1.
- **True Apple squircle**: flat sides + 22.37% continuous-curvature corners (not
  the whole-icon superellipse that bulges sides = the "Samsung" look).
- **Presets differ by visible colour treatment** (every icon changes): 原彩 /
  极简 black&white (contrast-boosted) / 单色 tinted.
- **单色 colour picker**: wallpaper dominant colour (saturation-weighted) +
  Windows accent + curated swatches + custom hex input; live re-render.
- **Premium adaptive badge**: frosted squircle chip, arrow/fold/dot glyphs,
  bottom-right, legible on any icon; composable in settings; "none" supported.
- **Real app logo** (gpt-image-2 sparkle squircle) in titlebar / about / .ico.
- Verified: build 0 warnings; `dotnet test` 105 passed; screenshot-verified each.

## Next

1. Supervised live run of switch-on → UAC → switch-off on the owner's machine
   (restore immediately) — the one path not exercised end-to-end (real `.lnk`
   COM write + HKLM overlay + Explorer refresh).
2. Owner: purchase OV/individual code-signing certificate (v0.9 release gate).
3. Codex two-stage review is blocked by the codex Windows sandbox bug
   (`orchestrator_helper_launch_canceled`); revisit when codex sandbox works, or
   use a cross-vendor reviewer that can read the repo.
4. Deferred to v1.0: real overlay ico per chosen glyph (helper currently applies
   the default arrow); baked top-right badge; a full HSV colour wheel.

## Blockers

- No blocker. GitHub CLI not in PATH; no remote repository yet.

## Open Questions

- Signing entity/name for the OV certificate (owner decision, purchase pending).
- v0.9 distribution channel details (direct download + pinned comment reply).
