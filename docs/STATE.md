---
updated: 2026-07-09 (icons v2 web build + shape/colour/marks marathon — doc-sync checkpoint)
version: pre-release (version narrative RESTORED per ADR-0013 amendment; Directory.Build.props stays 0.0.0 until the owner names the first release)
branch: main (~90 local commits, NOT pushed — push is the owner's call)
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

## Recently shipped (web side, Mac mock loop → swept to `docs/journal/2026-07.md`)

- **v3 Premium Flat build** (F1 fonts/tokens · canvas-first layout + 280px
  inspector · WebView2 hardening + diagnostics/CrashGate · welcome gate · dash
  purge). Sweep 2026-07-08.
- **Zone editor rebuild** (spec 04 v2.0, ADR-0014) rounds 1-3 — pixi v8 compositor
  live+bake, five materials + four title styles, import/export, codex+motion review
  disposed. Sweep 2026-07-08.
- **Icons v2 migration** (spec 06, ADR-0015) — WEB SIDE COMPLETE. The CPU TS
  compositor renders every preview + the 256 bake master; bridge v2; store rewrite;
  desktop mirror + taskbar P0; 60s arrow gate. Then the 2026-07-09 marathon: Figma
  corner-smoothing shape engine + curated 11-shape catalog · Gloss filter · 极致单色
  duotone (`segment.ts` + layered Mono + `monoStyle`) · 前景/背景 dual-tab colour +
  `plateColor` (schema v2) · silhouette-aware marks (Shadow/Halo). Sweep 2026-07-09.

## Live now — icons module (web), fully built through commit 7b8a5bc

283 bun tests + `tsc` green; browser visual-acceptance evidence
`docs/plans/evidence/2026-07-icons-v2/` (01-69). Current catalog + math is
authoritative in **spec 02** (§Shape System / §Colour Treatments / §Shortcut Marks),
**spec 06** (module contract + §3.11), **ADR-0015** (+ 2026-07-09 amendment). The
contract truth is `src/DeskMakeover.Web/src/bridge/types.ts` (bridge schema v2).

**In flight / next (web):**
1. Zone rebuild polish tail: equal-gap ticks (deferred), rename-input visual
   polish over the chip, SwiftShader/`MAX_TEXTURE_SIZE` startup probe with
   reduced-res fallback (renderer TODO), TS bake fixtures pinning compositor
   output (Z6 remainder), Halo's under-frost (v1 skips it).
2. D10 gesture unification remainder (icons module side).
3. Dark theme + zh locale full regression screenshots; evidence to
   `docs/plans/evidence/2026-07-v3/`.
4. F7: cross-vendor adversarial review (codex via /multi-ai) over the full diff
   (the shape/colour/marks marathon has NOT had a cross-vendor pass yet).
5. Debug/component-gallery cleanup (accordion-axis, link-chip, card.tsx,
   persisted-set now unused outside the gallery); AuxColorDot now used only by the
   mark-colour wheel.

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
  scan surfaces Recycle Bin + SystemIcon set; `DesktopItemKind.SystemIcon` +
  generalized per-user CLSID writers for This PC/Network/User Files — same
  mechanism as RecycleBinIconWriter, owner-prototype-proven) · AppxShortcut
  joins CanStyle (UWP .lnk = ordinary IconLocation write, prototype-proven;
  extraction ports Get-AppxIconSource manifest-logo resolution; delete the
  purple APPX fallback tile) · **replace `IconShapeGeometry.cs` with a port of
  the web's canonical geometry** (`src/icon-compositor/shapes.ts` — Figma
  corner-smoothing engine from squircle-path-kit (MIT) + authored cubics; the
  C# coarse polygons are retired and the golden-parity baseline for shapes
  moves web→C#) · **IconShape enum follows the owner-curated catalog**
  (2026-07-09: Google/Brave/Squircle/Blob/Rectellipse/Hexagon culled; Diamond/
  Flower/Pebble added — see the web union in `bridge/types.ts`; TileRenderer
  content placement gains the INSCRIBE_SHAPES rule from web compose.ts) ·
  **ConfigDto gains `plateColor` (schema v2) + `monoStyle: Tonal|Flat`** —
  plate override now Original + LAYERED Mono; Flat = 极致单色 (segmented
  subject flat on a flat plate — port web segment.ts: border-flood +
  distance-from-field Otsu + guards); FilterStyle gains `Gloss`
  (port web filters.ts gloss); MarkStyle renames Card→Shadow (neutral
  translucent drop shadow, markColor inert) + Echo→Halo (silhouette outline);
  marks are silhouette-aware on free-form (port web marks.ts stampMask +
  outsideDistance) · delete
  `IconsSession.Render.cs` PNG-per-tile path · re-verify the v0.9 deletion
  (`MakeoverService`/`PreviewItemFactory` + their tests removed 2026-07-09,
  grep-verified only) with `dotnet build && dotnet test` · patch
  ComparisonImageExporterTests off `IconStyler`, then delete IconStyler +
  IconStylerTests (banner in file) · slim `StylePreset.cs` to the live enums
  (MaskShape/ColorTreatment/Badge*) — the StylePreset record + StylePresets/
  StyleCombos statics are dead.
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
- 极致单色 on photo-fill art (e.g. Minecraft): the fragmentation guard falls back
  to the whole silhouette, but full-bleed photos still read noisy at 256px — a
  tighter guard is a future polish item (owner-noted, not blocking).
