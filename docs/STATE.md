---
updated: 2026-07-08 (v3 owner-iteration marathon checkpoint)
version: pre-release (version narrative RESTORED per ADR-0013 amendment; Directory.Build.props stays 0.0.0 until the owner names the first release)
branch: main (~50 local commits, NOT pushed — push is the owner's call)
---

# State

Completed work is swept to `docs/journal/2026-07.md` (append-only). This file is a
pointer: what is TRUE now, what is in flight, what comes next.

## Governing docs (current truth)

- **ADR-0013** + amendments — v3 "Premium Flat": light-first OKLCH, follows system;
  bundled Inter + HarmonyOS Sans SC; version narrative RESTORED (About version line +
  in-app changelog, auto-opens once per UPDATE, never on first install).
- **spec 02 v3** (+ §Addenda 2026-07-08) — visual language; ONE 16px glyph keyline;
  slash-circle 无 dialect; SwatchPicker/ChoiceList selection grammars; dash-free copy.
- **plan `docs/plans/2026-07-08-v3-premium-flat.md`** — F1-F8 build plan.
- **`docs/references/webview2-pitfalls.md`** — hardening checklist (web-side items
  DONE, host-side items = F8).
- Runbook: `docs/development.md` (dual-mode dev: browser/mock on any OS, native host
  on Windows).

## Active Work — v3 Premium Flat build (F-phases)

**Done through today (web side, Mac mock loop, ~50 commits, 168 bun tests green):**
F1 fonts/tokens · layout rebuild (canvas-first + 280px inspector, InspectorCard
grammar) · canvas nav (non-passive wheel zoom, fill-axis floor, micro toolbar,
top light-line progress) · gradient direction dial-synced dropdown (engine-truth
0°=top mapping fixed) · ONE none-dialect (slash-circle first in every axis) +
16px keyline + SwatchPicker extraction · flat mark row + native-arrow penance
gate (60s stare) · keymap legend by page (keyboard icon) · colour axis speaks the
swatch dialect directly (无=原彩 · duotone disc=黑白 · tint dots=单色, pure
black/white dots hidden as redundant, wheel=custom; mode segmented deleted —
supersedes the old mono-seeds-wallpaper-primary rule) · presets carry NO mark (mock aligned to engine NamedStyles truth) +
badge-free preset minis + per-style mark previews · welcome gate (language
roster → editorial brand beat + mosaic → TWO innocent survey questions →
roast/bluff → typed confession, paste refused) · DEV-only debug menu (flask,
localStorage resets) · changelog-on-update semantics verified · dash purge
across ALL user-facing strings · web-side WebView2 hardening
(`lib/webview-hardening.ts`) · global error capture + diagnostics (ring-buffer
error log in localStorage, window.onerror/rejection/prod-console.error/host
event; settings 问题诊断 row with copy/GitHub/email exits, every exit copies
the FULL report first because issue URLs cap near 8KB; CrashGate boundary
with a bilingual zero-dependency crash card + DEV crash probe) · settings page at page-scale type (controls stay
sm — unified rule below) · ONE `Reveal` fold grammar for every conditional
inspector section (ghost sub-margin bug fixed; native arrow sits `apart` at
the mark flow's far edge) · segmented/toggle thumbs animate translateX in
track space (layout-projection drift on panel-height changes killed).

**Zone editor rebuild (spec 04 v2.0, ADR-0014) — BUILT on the Mac loop
(commits 7cb8104/9b7ad79+, 211 bun tests green, browser-verified incl. baked
PNG pixel check):** client WebGL compositor (pixi v8) live+bake, Adaptive
Frost material + accent/emoji/label-chip titles, curated preset gallery,
stable ids, per-zone controls + apply-to-all, magnetism + snapped-only guides
+ overlap warn-wash, Alt-drag duplicate, visible undo/redo + delete-undo
toast, auto-rename on create, 分区落版 apply wave, DoneCard last-step. Panel
record + dispositions: `docs/reviews/2026-07-09-zone-editor-expert-panel.md`;
plan: `docs/plans/2026-07-09-zone-editor-rebuild.md`.

**Round 2 (same day, commit 39d4051): five material finishes + four title
styles (combo matrix + designer pairing), wallpaper import (picker/drag-drop/
empty-state link, session bar chip) + 导出图片 (bake→PNG download), empty state
= glass preset gallery on the user's wallpaper gated on compositor ready
(refresh dashed-frame flash killed), ghost slots redesigned as drawn landing
slots with panel-tone ink, ▴▾ glyphs → ChevronDown icons app-wide. Record:
`docs/reviews/2026-07-09-style-sets-import-export.md`. 220 bun tests green.**

**Round 3 (owner-driven polish + reviews, commits 00805e2..b1540cb):** codex
adversarial review disposed (9 fixed / 1 scheduled / 1 spec-amended / splits
restore the 500-line law); motion audit disposed (3 added incl. two-layer zone
delete exit via a compositor alpha exit lane, 10 no-add verdicts); modules stay
mounted and hide via visibility+inert (display:none zeroed hidden viewports —
the module-switch preview flash, triple-confirmed by rAF frame recorder + two
independent investigations); toasts anchor to the canvas stage; sliding active
washes (zone list + module rail, layoutId); presets alignment law (x.5 origins,
whole-cell spans); emoji picker two pages + OS-panel free input; 壁纸压暗
rename; per-input history coalescing. 222 bun tests green.

**Icons v2 migration (spec 06, ADR-0015) — IN PROGRESS (overnight run
2026-07-09, owner-approved Q1-Q12):** web icon compositor (pixi) becomes the
interactive renderer + 256-master bake; C# TileRenderer frozen (oracle +
reserved background renderer); bridge v2 (sourceUrls in, chunked masters out);
UX contract (live scrub, hover try-on, per-pick undo, exception badges, size
honesty, owned-verb menus, arrow gate softened to one-time 8s); taskbar P0 +
mock icon pack. Panel record + dispositions:
`docs/reviews/2026-07-09-icon-frontend-panel.md`; plan:
`docs/plans/2026-07-09-icon-frontend-migration.md`.

**In flight / next (web):**
1. Zone rebuild polish tail: equal-gap ticks (deferred), rename-input visual
   polish over the chip, SwiftShader/`MAX_TEXTURE_SIZE` startup probe with
   reduced-res fallback (renderer TODO), TS bake fixtures pinning compositor
   output (Z6 remainder), Halo's under-frost (v1 skips it).
2. D10 gesture unification remainder (icons module side).
2. Dark theme + zh locale full regression screenshots; evidence to
   `docs/plans/evidence/2026-07-v3/`.
3. F7: cross-vendor adversarial review (codex via /multi-ai) over the full diff.
4. Debug/component-gallery cleanup (accordion-axis, link-chip, card.tsx,
   persisted-set now unused outside the gallery).

**F8 (Windows machine required) — the reconciliation list:**
- **Zone compositor host handoff (ADR-0014)**: implement `wallpaper.getSource`
  (WIC decode + cover-crop → RGBA/PNG to web) + `wallpaper.applyBaked` (PNG in →
  write file + SetWallpaper) + `wallpaper.exportPng` (native Save dialog) +
  `wallpaper.setImportedSource` (persist imported source across launches;
  session-only acceptable degrade) + `wallpaper.setLook` persistence; run the parity fixtures (5 looks, legacy C# vs TS
  bake, ΔE<2 / SSIM>0.99); THEN delete WallpaperBakeRenderer.cs /
  WallpaperComposer.cs + their dotnet tests; verify WebView2 GPU path +
  SwiftShader fallback on a weak VM.
- Host-side error capture (web side is LIVE, contract ready): hook
  AppDomain.UnhandledException + TaskScheduler.UnobservedTaskException +
  Application.ThreadException + WebView2 ProcessFailed into a file log
  (%LOCALAPPDATA%), implement `diagnostics.getInfo` (OS/.NET/WebView2/arch +
  hostLogTail), stream errors as the `host-error` bridge event, add
  `links.email` to AppInfo, and show a NATIVE crash dialog (copy log / GitHub
  issue / email) for host-fatal crashes where the web layer is already dead —
  mirror crash-gate.tsx's three exits.
- resx is the i18n SOURCE: sweep every `PENDING-RESX` marker in
  `src/DeskMakeover.Web/src/lib/i18n/*.ts` into `Strings*.resx` via
  `scripts/dev/upsert-strings.py`, then regenerate the TS; delete dead strings
  (e.g. Dist_Mark / Color_Mono / Paper_Footer if unreferenced).
- Host changelog data source + real version (Directory.Build.props off 0.0.0
  when the owner names it).
- Fonts attribution line in About (HarmonyOS Sans license obligation — NOT yet
  rendered anywhere).
- WebView2 host hardening audit: the 🔴 must-list in
  `docs/references/webview2-pitfalls.md` §补丁清单 (Runtime probe, ProcessFailed,
  UDF path, DPI/RasterizationScale, kiosk settings, NewWindowRequested
  whitelist, SharedBuffer reuse, occlusion flag…). Web-side items are checked
  off in that doc; verify host items against `Host/WebShellWindow.cs`.
- Engine preset minis: confirm badge-free on real host (engine NamedStyles are
  Distinction.None, so they should be — verify, don't assume).
- **Icons v2 host handoff (ADR-0015)**: `icons.scan` v2 (256px `sourceUrls`
  incl. Recycle Bin ×2 + arrowUrl) · chunked `icons.applyBaked` →
  GeneratedIconStore · golden fixture generation (frozen oracle) + parity run
  (flat ΔE<2/SSIM≥0.995; filters SSIM≥0.98) · discovery fix (shell-namespace
  scan surfaces Recycle Bin; This PC/Network CLSID writers) · delete
  `IconsSession.Render.cs` PNG-per-tile path · re-verify the grep-based v0.9
  dead-code deletion with `dotnet build && dotnet test`.
- `dotnet test` (was 277 green pre-v3) + real-host verify (fonts/IME/DPI/125%
  hairlines) + `scripts/dev/publish.ps1`.

⚠️ **Owner-only gates unchanged**: supervised LIVE icon-bake + wallpaper-apply
(never auto-triggered) — `docs/verification/owner-supervised-live-runs.md`.

## Owner rules (durable)

- Accent = warm coral `#FF6F5E` only; blue/violet permanently banned (grep+test
  gated). OS-authentic depictions (Windows arrow blue `#0067C0`, taskbar chips)
  are the ONE reviewed exemption list in `tests/banned-colors.test.ts`.
- **Light-first, theme follows system** (ADR-0013 D3; supersedes the old
  dark-default rule).
- **Version narrative RESTORED** (ADR-0013 amendment; supersedes ADR-0012):
  version visible in About, changelog auto-opens exactly once per update,
  never on first install.
- **No dashes in user-facing copy** (owner decree 2026-07-08: reads as AI
  text). Grep `—` over i18n values before shipping strings.
- Every axis's 「无」 sits FIRST wearing the slash-circle glyph; dashed = auto,
  slash = none — never conflate. ONE keyline for all axis glyphs: authored on
  the 20/16 grid, rendered at 25px canvas = 20px ink (owner legibility call
  2026-07-09; `GLYPH` in chip-preview.tsx must stay = shape-paths `SWATCH` ÷ 0.8).
- **Control scale is unified app-wide** (owner 2026-07-09): segmented stays
  `sm` (22px/11px) and chip buttons 11px on EVERY page, same as the
  icons/wallpaper inspectors. Page-scale adjustments touch the TEXT layer
  only (titles, labels, descriptions, row rhythm) — never inflate controls.
- Presets never carry a shortcut mark (engine NamedStyles truth, owner call
  2026-07-07); nothing arrow-shaped may appear near preset thumbnails.
- The native arrow is legal but gated (60s penance sheet); the welcome survey
  must never reveal it is a gate.
- Extreme DRY; files ≤500 lines; WYSIWYG law (preview == bake pixels);
  bake/apply are owner-supervised, never auto-triggered.
- Specs are the source of truth; the old prototype HTML is historical only.

## Blockers

- None for web development. F8 needs a Windows machine. Release gates: signing
  cert (owner), public repo push (owner).

## Open Questions

- First release version number + naming (owner).
- Gloss filter (the coming-soon second tile): engine implementation timing
  (owner said "到时候我再实现").
