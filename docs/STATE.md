---
updated: 2026-07-10 (ADR-0017 type distinction system v1 built — factory ladder in designer acceptance)
version: Unreleased (Directory.Build.props + Web package.json both 0.0.0; the owner names the first release number; the About-line + in-app changelog narrative is RESTORED per ADR-0013 amendment)
branch: main — synced with origin/master (repo exists on GitHub but is PRIVATE; making it public is the owner's call)
---

# State

Completed work is swept to `docs/journal/2026-07.md` (append-only). This file is a
pointer: what is TRUE now, what is in flight, what comes next.

> ✅ **Doc-sync sweep COMPLETE (2026-07-10).** The Codex-audit drift was reconciled:
> specs 00/01/05 rewritten, 02/03/04/06 bodies synced, ADR amendments recorded,
> changelogs → Unreleased, onboarding docs corrected. Specs are trustworthy again;
> §Known doc drift below is kept as the RECORD of what was fixed (+ the few
> deliberately deferred low-stakes items in §Decisions).

## Governing docs (current truth)

- **ADR-0013** + amendments — v3 "Premium Flat": light-first OKLCH, follows system;
  bundled Inter + HarmonyOS Sans SC; version narrative RESTORED (About version line +
  in-app changelog, auto-opens once per UPDATE, never on first install).
- **spec 02 v3** (+ §Addenda) — visual language; ONE 16px glyph keyline; slash-circle
  无 dialect; selection grammars; dash-free copy. (Has drift — see §Known doc drift.)
- **plan `docs/plans/2026-07-08-v3-premium-flat.md`** — F1-F8 build plan.
- **`docs/references/webview2-pitfalls.md`** — hardening checklist (web-side items
  DONE, host-side items = F8).
- Runbook: `docs/development.md` (browser/mock loop on any OS; native host = F8, NOT
  yet wired — see §Bridge state).

## Bridge state (the P0 reality)

- Web bridge = **schema 3** (`src/DeskMakeover.Web/src/bridge/types.ts`,
  `BRIDGE_SCHEMA_VERSION = 3`); C# host = **schema 1** (`Contracts.cs`, `Version = 1`).
- Web already calls `wallpaper.applyBaked` + chunked `icons.applyBaked*`; the host still
  exposes the OLD `icons.setConfig` / `icons.apply` / `wallpaper.recompose`.
- **Native host CANNOT drive Web v3 today.** Only the browser/mock loop runs. Wiring the
  host to schema 3 is F8 (Windows machine required). Spec 05 must be rewritten to schema 3.

## Recently shipped (web side, Mac mock loop → swept to `docs/journal/2026-07.md`)

- **v3 Premium Flat build** (F1 fonts/tokens · canvas-first layout + RIGHT 280px
  inspector, 248px compact · WebView2 hardening + diagnostics/CrashGate · welcome gate ·
  dash purge). Sweep 2026-07-08.
- **Zone editor rebuild** (spec 04, ADR-0014) rounds 1-3 — pixi v8 compositor live+bake,
  five materials + four title styles, import/export, codex+motion review disposed. Sweep 2026-07-08.
- **Icons v2 migration** (spec 06, ADR-0015) — WEB SIDE COMPLETE. CPU TS compositor
  renders every preview + the 256 bake master; bridge schema 3; desktop mirror + taskbar;
  60s arrow gate; Figma corner-smoothing shape engine + curated 11-shape catalog; Gloss
  filter; 极致单色 duotone; dual-tab colour + plateColor; silhouette-aware marks. Sweep 2026-07-09.
- **2026-07-09→10 corrections (post-marathon):** icon-SIZE control REMOVED (panel + canvas
  menu, commit `d708f87`); per-bucket `kindPolicy` (apps/folders/files/system) surfaced as a
  persistent 2×2 labeled-chip section; preview fit toggle 满宽 ⇄ 满高·靠左; canvas-confetti
  celebration (飘丝带 from both screen corners, first-apply-per-launch, shared DRY across
  icons + wallpaper); the first-screen wand+bloom veil/reveal was TRIED then ROLLED BACK
  (broke icons); wallpaper seam/blur polish; zone-list active wash slide FIXED (`b881568`).

## Live now — web, through commit `b881568`

**297 bun tests + `tsc -b` green**; browser visual-acceptance evidence
`docs/plans/evidence/2026-07-icons-v2/` (01-75). The contract truth is
`src/DeskMakeover.Web/src/bridge/types.ts` (bridge **schema 3**). Mock desktop = a full
~120-icon fake desktop (`bridge/mock-desktop.ts` + `public/mock-icons/`, PNG pack from
`scripts/dev/generate-mock-icons.mjs`).

**In flight / next (web):**
0. **ADR-0017 per-type distinction system — v1 BUILT (commit `7ba12cf`),
   designer acceptance of the factory ladder in flight.** Three-round
   adversarial panel (PM/UX/UI) + owner dispositions; docs: ADR-0017,
   spec 06 §6.5, plan 2026-07-10-type-distinction-system.md. Shipped: sparse
   `typeOverrides` + `resolveTypeConfig` chain (preview/styleKey/bake all
   consume resolved configs), kindShapes DELETED, `shortcutShape` (uniform
   shortcut shape, default off), ExecutableFile→App bucket (bare exe =
   program), AppxShortcut isShortcut bug fix, hue-spread pool filter
   (fixed-plate types exit), type ACCORDION panel (expand-to-edit
   shape/saliency/bounded-plate + canvas scope-highlight dimming), factory
   saliency ladder (App Apple+Field / Folder Bookmark / File Tile / System
   Circle+BlackWhite). 355 tests + tsc green. OWNER-PENDING: shortcut mark
   default (panel consensus = badge ON; owner decree 2026-07-07 = presets
   ship None — unresolved conflict, decide before F8). F8 additions: host
   `.exe` classification + Appx mark fix + typeOverrides in Contracts.cs.
1. **ADR-0016 icon colour-field default** — COMPLETE to the owner's FIVE-STEP
   LAW + the RIM BAND law (spec 02 §Default Composition, ADR-0016 Amendments
   2-3; `IconProfile` metadata layer in profile.ts). Derived plates take the
   artwork's outermost solid BAND (α≥245, ~minDim/16 deep, majority hue via
   dominantColor): 亮圈深底 / 暗圈浅底 / 黄圈黄底; law-② boards additionally
   pass a corner-symmetry gate (dog-eared pages rejected); deep boards keep
   fitted chroma ≥0.09 where the gamut allows (yellow-green zone pulls to
   amber h≈78 — 深金, never olive). Designer verdicts: v7 PASS · v8.1 PASS ·
   v17 PASS · **v19 FAIL → v20 PASS (4/4 items cleared, 11 own-boards
   pixel-identical, zero collateral)**. ⛔ Iron laws: subject pixels never
   recoloured; own backgrounds never altered. T7 glass rim SHIPPED. T9 codex
   review DONE (11/11). 346 bun tests + tsc green (rim regression trio:
   light-outline / accent-vs-majority / soft-shadow). OWNER-PENDING: info-class
   solid circle badges legally anchor their own colour (law ②) — designer
   suggests optional ring-seam lightness polish, owner call. Owed: D4 corpus
   ΔE probe (browser harness) · resx sweep of new PENDING-RESX. Plan:
   `docs/plans/2026-07-10-icon-colour-field.md`.
2. Zone rebuild polish tail: equal-gap ticks (deferred, NOT accepted), rename-input polish,
   SwiftShader/`MAX_TEXTURE_SIZE` startup probe with reduced-res fallback, TS bake fixtures.
3. Dark theme + zh locale full regression screenshots → `docs/plans/evidence/2026-07-v3/`.
4. F7: cross-vendor adversarial review (codex via /multi-ai) over the full diff.

## Known doc drift (Codex audit 2026-07-10 — RESOLVED; kept as the record)

Facts verified against HEAD. Every item below has been corrected in the named docs
(commits `6ec1ffc` / `f0542ff` / `9453656` / `80dbaf4`); this list stays as the map of
what changed and why.

- **Specs 01 & 05 — most misleading.** They describe C# producing pixels (SharedBuffer
  frame stream, WebView2-as-viewer, left 300px panel). CURRENT: icons rendered by CPU
  TypeScript + Worker; wallpaper by Pixi; C# keeps only window / source decode / ICO
  packaging / shell write / backup-restore. Pending full rewrite to schema 3 + current arch.
- **Spec 00** — the v1.0-icons-only / v1.1-rail release train is void; prototype-parity is
  no longer a release gate; icon-size was reverted; move to `Unreleased → first public → next`.
- **Spec 02** — 300px panel → RIGHT 280/248px; shape catalog is 11 (not 13); Apple corner
  math has 3 conflicting sources (see decision below); font budget line stale; first-scan
  original→beautified wow was rolled back; module switch is instant/keep-mounted.
- **Spec 03** — settings page drifted: grouped card (not per-group full-width), version +
  changelog RESTORED (not removed), trust facts are dotted text (not pills).
- **Spec 04** — opens citing round-2 but the body still says single-Frost/single-chip/no
  baked shadow; references `paper-presets.tsx` which does NOT exist; bake is main-thread
  Pixi `toBlob`, not an OffscreenCanvas worker.
- **Spec 06** — Pixi vs CPU-renderer self-contradiction (it is CPU TS + Worker; Pixi is
  wallpaper only); schema v2 → 3; icon-size + canvas-size menu removed; taskbar running
  pills removed; mock pack is PNG (not WebP); auto-beautify default conflicts with settings.
- **ADR-0014 line 22** — says `WallpaperBakeRenderer.cs` / `WallpaperComposer.cs` are
  DELETED; they still exist (F8 deletion). **ADR-0015 line 113** — says the IconStyler chain
  is deleted; `IconStyler.cs` + tests still exist (F8 deletion). Corrected to pending.
- **ADR status table** — many are partially superseded (0003/0005 default-mark governance
  reversed → default is None; 0008/0012 dark-default + no-version reversed; 0011 renderer
  ownership superseded by 0014/0015; 0014 material decision reversed to five materials).
  Full table lives in the Codex audit; amendments to be added, history NOT rewritten.

## Decisions (owner-resolved 2026-07-10 — drive the spec rewrites)

1. **`win-native-arrow.png`** — RESOLVED: leave it. The owner accepts the git-tracked
   extracted arrow as-is (do NOT redraw, do NOT re-flag as a release blocker). ADR-0015's
   no-ship-extracted-assets clause carries an owner exception for this one asset.
2. **Apple corner geometry** — the TS renderer's iOS-0.225 cubic (`shapes.ts`) is CANONICAL
   (it is the WYSIWYG bake truth). chip-preview must drop its Lamé n=5 and share the same
   cubic path; Spec 02 + old ADR updated to cubic. C# oracle already cubic. [code + doc]
3. **`ConfigDto.size` / `TrySetIconSize`** — `size` becomes a READ-ONLY observed field;
   guard the writer so history/version replay can never resize the real desktop. [C# guard = F8]
4. **Wallpaper gesture** — KEEP HEAD (blank-left-drag creates a zone; pan on middle/compare);
   add a reversal amendment to ADR-0013. [doc]
5. **New-icon auto-beautify** — default FALSE and HIDE the setting until a real watcher/
   consumer exists (no promising an absent capability); Spec 06 updated. [code + doc]
6. **Ordinary-file participation** — KEEP default-on (product is reversible + supervised;
   kindPolicy gives one-click per-bucket opt-out); update Spec 01 to match. [doc]
7. **Space key** — REVISED 2026-07-10 (owner challenge): the "focused button gets Space"
   a11y clause does NOT apply to this product. Space stays a GLOBAL compare gesture (only
   text inputs excluded). Reason: the inspector is button-dense and a just-clicked swatch/chip
   keeps focus — letting Space activate it would break the compare gesture exactly when it is
   used. Buttons remain keyboard-activatable via ENTER, so nothing is stranded. No code
   behaviour change; amend Spec 02's generic Space-activates-button clause to record this. [doc]
8. **Release identity** — standardize ALL changelogs to `Unreleased` until the owner names
   the first number (root CHANGELOG, Host changelog.json, mock). [doc]

**doc-sync part 2: COMPLETE 2026-07-10.** Code 2/5/7 `3a6ec48` (3 size-guard = F8) ·
ADR amendments `9453656` · changelogs → Unreleased `b1890fa` (Host json feature-copy
still needs owner curation at release) · Specs 00/01/05 rewritten `6ec1ffc` ·
Specs 02/03/04/06 bodies synced `f0542ff` · code-style.md two-stack rewrite.
Still deferred (low-stakes): per-ADR Superseded status banners (map above suffices),
historical banners on old plans/reviews/evidence, HarmonyOS font subsetting task,
webview2-pitfalls SharedBuffer-era scope note.

## F8 (Windows machine required) — the reconciliation list

- **Host → schema 3**: implement `wallpaper.getSource` (WIC decode + cover-crop → RGBA/PNG),
  `wallpaper.applyBaked` (PNG → write file + SetWallpaper), `wallpaper.exportPng`,
  `wallpaper.setImportedSource`, `wallpaper.setLook`; chunked `icons.applyBaked` →
  GeneratedIconStore; `icons.scan` v2 (256px `sourceUrls` incl. Recycle Bin ×2 + arrowUrl).
- **Parity fixtures**: 5 wallpaper looks (legacy C# vs TS bake, ΔE<2 / SSIM>0.99); icon
  golden oracle (flat ΔE<2/SSIM≥0.995; filters SSIM≥0.98).
- **Delete-at-F8 (still present today)**: `WallpaperBakeRenderer.cs` / `WallpaperComposer.cs`
  + dotnet tests; `IconStyler.cs` + `IconStylerTests` (patch `ComparisonImageExporterTests`
  off it first); `IconsSession.Render.cs` PNG-per-tile; dead `StylePreset` statics.
- **Port web→C# geometry**: replace `IconShapeGeometry.cs` with a port of `shapes.ts` (Figma
  corner-smoothing); IconShape enum follows the 11-shape curated catalog (+Diamond/Flower/Pebble).
- **ConfigDto parity**: `plateColor`, `monoStyle: Tonal|Flat`, `Gloss` filter, Card→Shadow /
  Echo→Halo mark renames, silhouette-aware marks; then port segment.ts / filters.ts / marks.ts.
- **Host-side error capture** (web LIVE, contract ready): AppDomain/TaskScheduler/ThreadException
  + WebView2 ProcessFailed → file log; `diagnostics.getInfo`; `host-error` event; native crash dialog.
- **resx i18n sweep**: `PENDING-RESX` markers in `src/.../i18n/*.ts` → `Strings*.resx` via
  `scripts/dev/upsert-strings.py <path-to-json>`; regenerate TS; delete dead strings.
- **Fonts attribution** line in About (HarmonyOS Sans license — not yet rendered).
- **WebView2 host hardening** audit (webview2-pitfalls.md §补丁清单) against `Host/WebShellWindow.cs`.
- `dotnet build && dotnet test` (was 277 green pre-v3) + real-host verify (fonts/IME/DPI/125%).
- **Release packaging is UNVERIFIED**: `scripts/dev/publish.ps1` publishes only the App (no
  ElevatedHelper), does not build Web first, yet the App depends on an adjacent `web/`. The
  "single shippable exe" narrative does NOT hold — treat as incomplete until proven.

- **Findability gate (ADR-0016 D4)**: at F8 exit, default look, 20 random targets —
  locate time/error rate not worse than the stock-desktop threshold (owner-supervised).

⚠️ **Owner-only gates unchanged**: supervised LIVE icon-bake + wallpaper-apply (never
auto-triggered) — `docs/verification/owner-supervised-live-runs.md` (itself pending F8 rewrite).

## Owner rules (durable)

- Accent = warm coral `#FF6F5E` only; blue/violet permanently banned (grep+test gated).
  Reviewed exemptions in `tests/banned-colors.test.ts`: OS-authentic depictions (Windows
  arrow blue `#0067C0`, taskbar chips) AND the multicolour celebration confetti (one file).
- **Light-first, theme follows system** (ADR-0013 D3; supersedes old dark-default).
- **Version narrative RESTORED** (ADR-0013 amendment; supersedes ADR-0012).
- **No dashes in user-facing copy** (owner decree; reads as AI text).
- Every axis's 「无」 sits FIRST wearing slash-circle; dashed = auto, slash = none. ONE
  keyline for all axis glyphs (25px canvas = 20px ink).
- **Control scale unified app-wide**: segmented `sm` (22px/11px), chip buttons 11px on every
  page. Page-scale adjustments touch the TEXT layer only, never inflate controls.
- Presets never carry a shortcut mark; nothing arrow-shaped near preset thumbnails.
- The native arrow is legal but gated (60s penance sheet); the welcome survey must never
  reveal it is a gate. (The roast/penance tone is a DELIBERATE owner brand choice — do not soften.)
- ⛔ **Icon subject pixels are never recoloured** (ADR-0016 D8, owner 2026-07-10):
  every icon keeps its own colours; looks differentiate via plates, silhouette
  shadows/halos, outlines, backgrounds — never by re-inking some subjects.
- **Visual work acceptance loop** (owner order 2026-07-10): a look/effect is done only
  when the designer-seat subagent passes a pixel-level acceptance on REAL renders;
  FAIL → iterate and resubmit.
- Extreme DRY; files ≤500 lines; WYSIWYG (preview == bake pixels); bake/apply owner-supervised.
- Specs are the intended source of truth — but see §Known doc drift; the old prototype HTML is historical only.

## Blockers

- None for web development. F8 needs a Windows machine. Release gates: signing cert (owner),
  public repo visibility (owner), first version number (owner).
