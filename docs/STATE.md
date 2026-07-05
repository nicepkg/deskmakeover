---
updated: 2026-07-06
version: unreleased
branch: main
---

# State

## Active Work

- v0.9 desktop-icon module: feature-complete; iterating hard on aesthetics from
  live owner feedback. Accent is warm coral `#FF6F5E` (never blue/violet — reads
  as AI slop). Platform vision: [ADR-0004](decisions/0004-platform-form.md).
- A composable **shape × colour × distinction** tool per
  [ADR-0005](decisions/0005-distinction-shape-color-system.md):
  - **外形 (shape)**: 苹果 squircle / 纯圆 (already-round icons untouched) / 三星 —
    one `ContinuousCornerMask(r,n)` superellipse family (`ShapeGeometry`).
  - **配色 (colour)**: 原彩 / 极简 black&white / 单色 (wallpaper-extracted + accent +
    curated swatches + hex).
  - **快捷方式区分**: VOC-mandated 3-state 美化(default)/保留/去除. 美化 = a soft,
    contrast-adaptive coral **enamel arc** grown into the icon's own bottom-left
    Apple-curve edge (alpha-derived, silky radial+angular falloff) — never a
    top-right notification dot. Candidates: 珐琅/缎带/票根.
- Owner rules (durable): never blue/violet gradient; extreme DRY; aesthetic calls
  go through VOC + expert panel + the 3-second-misread gate, not engineer fiat;
  every substantive decision documented (this file + ADRs) so context survives a
  new session.

## Future-facing IA (ADR-0005-adjacent, panel-designed 2026-07-06)

The single-module screen is the **degenerate state of a multi-module platform**, not a
throwaway. A constant **Frame + region taxonomy** was designed (panel):
F1 title bar / F2 module rail (4+ modules) · O1 Hero verb / O2 module list (2+) /
O3 flagship canvas / O4 discovery · D1 settings / D2 per-icon override / D3 preset
gallery / D4 consent / D5 module detail. Today lights up F1+O1+O3; every future
module/list/rail/preset already has a name → adding them is "lighting up", not
refactoring. Two-speed customisation: main = high-freq (风格 combos + 外形 + 配色),
settings = low-freq (区分三态 + 角标形状 + 角标配色), right-click = per-icon.

**Landed this round:** ONE reusable ColorPicker (icon tint + mark colour, DRY);
user-customisable mark colour (adaptive from any hue); named one-tap **风格 combos**
(苹果极简/糖果彩/纯净黑白/壁纸同色 set shape×colour×distinction in one render);
clear coral filter-chip selection; DRY cleanup.

## Owner-requested, still open

1. **App logo** (titlebar + about) — owner finds the coral sparkle weak; rework via
   /gpt-image-2.
2. Remaining IA polish from the 25-item panel punch-list: Hero once-on compression
   to a status band, 8pt spacing pass, tile focus ring, skipped-count clickable,
   settings drawer fixed-width + scrim + unified row grammar + badge dependency
   disable states, label truncation for same-name items.
3. Codex two-stage review is blocked by the codex Windows-sandbox bug; revisit.
4. Supervised live switch-on→UAC→switch-off run on the owner's machine.

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
