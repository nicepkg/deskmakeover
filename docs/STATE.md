---
updated: 2026-07-10 (doc-sync correction sweep — Codex audit reconciled against HEAD)
version: Unreleased (Directory.Build.props + Web package.json both 0.0.0; the owner names the first release number; the About-line + in-app changelog narrative is RESTORED per ADR-0013 amendment)
branch: main — synced with origin/master (repo exists on GitHub but is PRIVATE; making it public is the owner's call)
---

# State

Completed work is swept to `docs/journal/2026-07.md` (append-only). This file is a
pointer: what is TRUE now, what is in flight, what comes next.

> ⚠️ **Doc drift is live (2026-07-10).** Several specs/ADRs still describe the OLD
> architecture and were flagged in the Codex audit. Until the correction sweep + owner
> decisions land, trust THIS file and the CODE over the specs. The stale map is in
> **§Known doc drift** below. New sessions: read that section before treating any spec as truth.

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
1. Zone rebuild polish tail: equal-gap ticks (deferred, NOT accepted), rename-input polish,
   SwiftShader/`MAX_TEXTURE_SIZE` startup probe with reduced-res fallback, TS bake fixtures.
2. Dark theme + zh locale full regression screenshots → `docs/plans/evidence/2026-07-v3/`.
3. F7: cross-vendor adversarial review (codex via /multi-ai) over the full diff.
4. **This doc-sync sweep** — reconcile the specs/ADRs flagged in §Known doc drift.

## Known doc drift (Codex audit 2026-07-10 — pending correction / owner decision)

Facts verified against HEAD. Specs are NOT yet rewritten; do not trust them over code.

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

## Open decisions (owner — block the spec rewrites)

1. **`win-native-arrow.png` is git-tracked** and imported by production renderer, but
   ADR-0015 forbids shipping extracted Windows assets. Release blocker: redraw
   programmatically vs. relax the ADR with a licensed asset?
2. **Apple corner geometry** — unify chip-preview (Lamé n=5) + TS renderer (iOS 0.225
   cubic) + C# oracle (cubic) onto ONE shared path, THEN fix Spec 02 / old ADR.
3. **`ConfigDto.size` / `TrySetIconSize`** survive the size-control removal — make `size`
   a read-only OBSERVED field, or a migration field? (must not let history replay resize the
   real desktop.)
4. **Wallpaper gesture** — HEAD is blank-left-drag-creates-zone (pan on middle/compare);
   ADR-0013 wants drag-pans / explicit create tool. Amend the ADR, or change the code?
5. **New-icon auto-beautify** — C# + mock default `true` and settings promise auto-handling,
   but there is NO watcher/consumer anywhere. Default false + hide until built, or override Spec 06?
6. **Ordinary-file participation** — `kindPolicy.File` + `CanStyle` include ordinary files
   by default; Spec 01 promised opt-in. Which is the final trust model?
7. **Space key** — App.tsx makes hold-Space = compare everywhere (owner call), but Spec 02
   a11y wants Space to activate a focused button. Except buttons, or amend the spec?
8. **First release number/name** — standardize all changelogs to `Unreleased` until set?

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
- Extreme DRY; files ≤500 lines; WYSIWYG (preview == bake pixels); bake/apply owner-supervised.
- Specs are the intended source of truth — but see §Known doc drift; the old prototype HTML is historical only.

## Blockers

- None for web development. F8 needs a Windows machine. Release gates: signing cert (owner),
  public repo visibility (owner), first version number (owner).
