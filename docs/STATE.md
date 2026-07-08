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
(`lib/webview-hardening.ts`) · settings page at page-scale type (controls stay
sm — unified rule below) · ONE `Reveal` fold grammar for every conditional
inspector section (ghost sub-margin bug fixed; native arrow sits `apart` at
the mark flow's far edge) · segmented/toggle thumbs animate translateX in
track space (layout-projection drift on panel-height changes killed).

**In flight / next (web):**
1. D10 gesture unification remainder + wallpaper zone-drag DOM approximate fill
   (IXD P1: frost tracks pointer 1:1, reconcile on true frame).
2. Dark theme + zh locale full regression screenshots; evidence to
   `docs/plans/evidence/2026-07-v3/`.
3. F7: cross-vendor adversarial review (codex via /multi-ai) over the full diff.
4. Debug/component-gallery cleanup (accordion-axis, link-chip, card.tsx,
   persisted-set now unused outside the gallery).

**F8 (Windows machine required) — the reconciliation list:**
- resx is the i18n SOURCE: sweep every `PENDING-RESX` marker in
  `src/DeskMakeover.Web/src/lib/i18n/*.ts` into `Strings*.resx` via
  `scripts/dev/upsert-strings.py`, then regenerate the TS; delete dead strings
  (e.g. Dist_Mark / Color_Mono if unreferenced).
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
