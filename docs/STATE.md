---
updated: 2026-07-11 (replatform executing Mac-first: M2 + M3/M4 blind-write + M5/M5.11/M5.12 DONE — icon core certified byte-exact over the all-real corpus (1487 cells, committed real-icon SSoT); community-standard layout, .NET quarantined under legacy/)
version: Unreleased (Directory.Build.props + Web package.json both 0.0.0; the owner names the first release number; the About-line + in-app changelog narrative is RESTORED per ADR-0013 amendment)
branch: main — synced with origin/master (repo exists on GitHub but is PRIVATE; making it public is the owner's call)
---

# State

Completed work is swept to `docs/journal/2026-07.md` (append-only). This file is a
pointer: what is TRUE now, what is in flight, what comes next.

> 🚨 **REPLATFORM DECIDED (2026-07-10, owner + 4-seat adversarial panel incl.
> Codex).** The product moves to **Tauri 2 + Rust**; .NET exits. One Rust icon
> core (WASM preview + native apply/background) is the single pixel truth in
> v1.0; the TS compositor is FROZEN as the parity oracle until certification;
> background resident auto-format ships in v1 (spec 07); the global transparent
> arrow overlay is the default and the 60s penance gate retired. Read:
> **ADR-0019 / ADR-0020 / ADR-0021**, plan
> `docs/plans/2026-07-10-tauri-migration.md` (M0–M8), panel record
> `docs/reviews/2026-07-10-tauri-rust-migration-panel.md`. §F8 below is VOID.

> ✅ **Doc-sync sweep COMPLETE (2026-07-10).** The Codex-audit drift was reconciled:
> specs 00/01/05 rewritten, 02/03/04/06 bodies synced, ADR amendments recorded,
> changelogs → Unreleased, onboarding docs corrected. Specs are trustworthy again;
> §Known doc drift below is kept as the RECORD of what was fixed (+ the few
> deliberately deferred low-stakes items in §Decisions).

## Governing docs (current truth)

- **ADR-0019/0020/0021 + `docs/plans/2026-07-10-tauri-migration.md`** — the
  Tauri 2 + Rust replatform, background-resident v1 (spec 07), arrow default.
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

- Web bridge = **schema 4** (`src/bridge/types.ts`,
  `BRIDGE_SCHEMA_VERSION = 4`, two-axis subject×plate); C# host = **schema 1**
  (`Contracts.cs`) and will NEVER be wired — the host is replaced by the Tauri/Rust
  stack (ADR-0019). Only the browser/mock loop runs today.
- Under Tauri the contract is GENERATED from `dm-contracts` (tauri-specta); the
  schema-1/4 split is the standing proof that hand-mirrored schemas fail.
- Spec 05 rewrite (Tauri bridge) happens alongside M2 of the migration plan.

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

**359 bun tests + `tsc -b` green**; browser visual-acceptance evidence
`docs/plans/evidence/2026-07-icons-v2/` (01-75). The contract truth is
`src/bridge/types.ts` (bridge **schema 4**). Mock desktop = a full
fake desktop (`bridge/mock-desktop.ts` + the REAL pack `public/real-icons/`, the
COMMITTED asset SSoT — ADR-0015 D9 amendment, owner override 2026-07-11; no
synthetic fallback; vite closeBundle strips it from every build). The synthetic
source pack is DELETED — the corpus is all-real (M5.12).

**M0b DONE (recaptured all-real at M5.12)** — parity oracle corpus committed under
`testdata/icons/` (1,611 PNGs): Tier A full desktop under the spectrum default,
Tier B 29-source style matrix (47 cells each), Tier C per-look hue-spread sessions,
per-source stage dumps. Harness `scripts/capture-oracle.ts` (`--capture` /
`--verify [--sample N]`, deterministic); test-suite smoke
`tests/oracle-corpus.test.ts` (no CI pipeline exists yet — all gates are local).
This is the TS side of the M5 tri-target differential.

**M1 Spike 4 DONE (tri-target pixel slice — the one M1 spike that runs on Mac)** —
Circle + white plate + subject blit + dock shadow over 120 sources × {256,512} = 240
cells: **native↔wasm 240/240 byte-identical, TS↔Rust 0 diff bytes of 157.3M**
(byte-equal; gate was SSIM≥0.995; re-run at HEAD over the real pack:
248/248 native↔wasm, TS↔Rust 0/162,529,280). `crates/dm-icon-core` slice modules + plain-ABI
`dm-icon-wasm` + `xtask spike4-*`; one command `bun tests/icon-parity/spike4/run.ts`.
Determinism intel (M5 checklist seed): JSC pow vs libm pow = 1 ulp apart on 34/256
decode-LUT entries → TS↔Rust byte parity is EMPIRICAL (full-corpus differential remains
the certification gate); wasm↔native parity is structural. Details in the migration
plan §M1. Remaining M1 spikes (1/2/3/5) are Windows-bound.

**Replatform progress (Mac-first, 2026-07-10→11 — details live in the migration
plan's DONE blocks, this is the pointer):**
- **Layout restructured (ADR-0019 Amendment 1, owner order):** community-standard
  Tauri — web app at the repo ROOT (`src/`, `public/`, root `package.json`),
  `src-tauri/` + `crates/` at root, the ENTIRE frozen .NET tree quarantined under
  `legacy/`, `apps/` deleted, `.gitignore` .NET globs scoped (Rust `src/bin` no
  longer swallowed). Six-commit chunk `1aadc6e..fd2ff77`.
- **M2 DONE:** Tauri 2 window hosts the app on Mac; tauri-specta generated
  bindings (`src/bridge/generated.ts`); rusqlite settings store; 359 bun tests.
- **M3/M4 blind-write DONE:** dm-domain/dm-operations/dm-windows/dm-elevated —
  durable WAL transaction + incremental CAS ledger fully unit-tested on Mac
  (non-icon crates 59→125 tests after the coverage audit, incl. kill-point
  battery); COM adapters msvc-check clean in isolation; runtime verification =
  [WINDOWS-VERIFY] checklist in `docs/plans/2026-07-10-m34-windows-blind.md`,
  batched with M1 spikes.
- **M5 + M5.11 + M5.12 DONE, CERTIFIED over the all-real corpus:** full TS pixel
  pipeline in `dm-icon-core` + `dm-icon-codec` ICO writer. Real-icon SSoT
  `public/real-icons/` committed (ADR-0015 D9 amendment, owner override — never
  ships: vite closeBundle strips it; taskbar/wallpaper fixtures repointed).
  One-command cert `bun tests/icon-parity/m5/run.ts`: **1487/1487 corpus cells
  byte-identical (0/389,808,128 diff bytes)** over 124 real sources, profiles
  124/124, masks 48/48, hue-spread 7/7; setHash `8a6c19ee69235d95`; pack 100%
  PNG, parity gate decodes in-process on any platform. bun-only sweep done
  (`node:zlib` kept deliberately — Bun's native zlib; `Bun.*` variants are
  raw-deflate and cannot read/write PNG streams).
- **In flight:** icon-algorithm deep coverage on dm-icon-core/codec/wasm
  (degenerate-input families, threshold pairs, property tests, malformed-ICO
  rejection, wasm ABI smoke). **Next:** M6 dual-target cutover (render_tile wasm
  export + Config ABI + wasm↔native CI → flip preview to WASM → delete TS pixel
  modules).
  **Blocked on the owner's win11:** M1 spikes 1/2/5 (+3 needs him present),
  workspace-wide msvc check (rusqlite bundled C), all [WINDOWS-VERIFY] items.
- **Codex full-review + wave-2 hardening (2026-07-11, EXECUTING):** an independent
  Codex full-review of snapshot `7dc82c1` produced 26 findings; triaged vs HEAD
  (independent reviewer + lead re-verify) = 6 FIXED · **7 OPEN-CONFIRMED** · 3
  OUT-OF-SCOPE · 10 OVER-RATED. The 7 open defects all live behind the
  currently-unwired apply/txn/CAS surface (no live user harm today) and **gate the
  M6 cutover** — fixing now is the pre-M6 prep. Owner approved the fix docs
  2026-07-11; two disjoint-crate agents dispatched: **m34** (dm-operations/
  dm-windows/dm-domain: P1-4 CAS-verify, P1-10 RegularFile-commit, P1-14 empty-ICO,
  P1-9 fsync, P2-5 error-honesty, P2-2 settings-txn + latent P1-12/7/5, P2-4) ·
  **m5** (dm-icon-core: P2-7 thread-local arrow → OnceLock, byte-parity re-cert).
  Brief: `docs/plans/2026-07-11-windows-hardening-wave2.md`. Each fix ships a
  red→green regression test; Phase-6 Codex review + Phase-7 verify gate the merge.
- **M6 performance architecture (design input, 2026-07-11):** two converged reviews
  (senior-eng subagent + Codex 60min) → recommended parallelism = N workers × 1
  WASM instance, register-once ABI, latest-generation coalescing; ranked byte-safe
  optimizations (#0 = fix the thread-local arrow, above). Open questions need the
  owner's win11 + real benchmarks. Doc: `docs/plans/2026-07-11-m6-performance-architecture.md`.

**In flight / next (web):**
-1b. **PRESET COLLECTION v2 SHIPPED + ACCEPTED** (`b7dd226`+`f8eb20d`, all
   designer PASS): seven coordinate-bookmark presets (spectrum default ·
   stationery · glass · pebble · ink · white · ascast), six mark styles
   (Fold retired), #65470D dark-brown folder boards banned, featured-four
   fold + 「更多风格 +N」 counting chip, glass preset wears Shadow (Glass
   bead mark redesign = COMPONENT DEBT). Preset hover carries typeOverrides
   (`5d3b589` bug fix); type-row picker overflow fixed (`f03abe0`).
   Normative: docs/product/preset-collection-v2.md. OWNER-PENDING: which
   sets enter factory lineup + default confirmation (all seven live now,
   spectrum default). Owed unchanged: colour-migrate tests · banned-arc
   guard · fixed-plate shadow regression test · folder-drift probe (likely
   obsolete — folders now derive/manila by design) · spec02/06 amendments ·
   alpha-plate exploration (owner asked; verdict: plumbing not algorithm).
-1a. **TWO-AXIS RESHAPE DESIGNER-ACCEPTED (final PASS at `46a26bc`)** — subject×plate + 本色 fifth stop + panel two rows + accordion parity all certified (§6 full pass, four-way glyph distinguishability, violet-arc rulings executed). Paper file band PASS at `c080912` (owner A). NEW OWED: ① regression
   assertion - fixed-plate tiles MUST carry the silhouette shadow (T2 bug
   relapsed once, designer strong-rec); ② INVESTIGATE folder band colour
   drift - designer saw multicolour folders in paper-band-v1-zoom despite
   the #65470D factory pin (suspect: the widened user-plate lane's
   backdrop-swap path); ③ owner Q: folders 统一金 vs 各自色 (designer says
   multicolour aids within-folder findability; owner earlier picked 统一金).
   Owed T4 tail: colour-migrate mapping tests · banned-arc swatch guard in banned-colors.test · spec02/06 colour-axes amendments · OWNER-PENDING: 灰暗文件板 A(暖纸色板,推荐)/B(深板变暖)/C(不改) disposition. Was: T1/T2 `26aa3db` · T3 `e4bac52` (Subject+Plate rows live, dual-tab popover dead, accordion pins added; designer §6 acceptance IN FLIGHT). Owed T4: colour-migrate mapping tests · shape=None disabled-state screenshot · spec02/06 colour-axes amendments. Old note: **T1/T2** — model+engine+bridges live on
   subject×plate (smoke: 124 icons byte-identical look, presets match, 0
   errors). Spec FINAL (本色 = plate stop (null,'white'), faithful/minimal
   collapse fixed). NEXT: T3 panel two-row rebuild (主体行+底板行 + 本色
   chip + QuadPlateGlyph + kill dual-tab popover + accordion plate 本色 +
   i18n Subject_/Plate_ keys) + T4 (migration-table tests, screenshots,
   designer §6 four-way distinguishability acceptance) per
   docs/plans/2026-07-10-two-axis-colour.md.
-1. **TWO-AXIS COLOUR RESHAPE — OWNER-APPROVED (2026-07-10), spec being
   written to `docs/product/two-axis-colour-spec.md` by the chief-designer
   seat.** Panel 3/3 convergence: dissolve `colorMode` into 主体 (subject:
   原彩/黑白/单色) × 底板 (plate: 随图标-first/白/bounded swatches/wheel);
   满彩 demotes from mode to the default preset coordinate; presets become
   coordinate bookmarks; the word "mode" leaves the UI; plateColor's
   per-mode semantics collapse to one plate value; Original's
   plate-recolour gap dies structurally; net-new combos (BW×Auto etc.).
   Deterministic old→new mapping = zero feature loss; schema 3→4 BEFORE F8
   (translation layer VETOED by the UI seat). Naming law: 主体/底板, never
   前景色/背景色. Guardrail: plate axis leads with 随图标 (anti "all apps
   one colour"). Type rows keep the just-shipped chip grammar; per-type
   rule becomes "types may only step DOWN".
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

## F8 — VOID (superseded 2026-07-10 by the ADR-0019 replatform)

The C#-host reconciliation list that lived here is dead work: the host is never
wired; the Tauri/Rust migration plan (`docs/plans/2026-07-10-tauri-migration.md`)
replaces it. Items that CARRY FORWARD into the plan (not lost, re-homed):
wallpaper get-source/apply (M4) · parity fixtures → the TS-oracle corpus +
tri-target harness (M0/M5) · host error capture → Rust tracing/minidumps (M2) ·
fonts attribution line in About (M8) · packaging made real → NSIS + helper (M8) ·
**findability gate (ADR-0016 D4)** at release exit (M8) · i18n: the resx sweep is
CANCELLED — TS dictionaries become the source of truth (ADR-0019 defaults).

⚠️ **Owner-only gates unchanged**: supervised LIVE icon-bake + wallpaper-apply +
resident-mode audit (never auto-triggered) —
`docs/verification/owner-supervised-live-runs.md` (rewrite for the Tauri stack at M8).

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
- **Arrow semantics (ADR-0021, 2026-07-10)**: the global transparent overlay is the
  DEFAULT; every shortcut is redrawn; 「保留原样」 = subject + baked classic arrow;
  the 60s penance gate RETIRED (its object no longer exists). The rest of the
  welcome-gate ritual is untouched owner brand ceremony — do not soften it.
- ⛔ **Icon subject pixels are never recoloured** (ADR-0016 D8, owner 2026-07-10):
  every icon keeps its own colours; looks differentiate via plates, silhouette
  shadows/halos, outlines, backgrounds — never by re-inking some subjects.
- **Visual work acceptance loop** (owner order 2026-07-10): a look/effect is done only
  when the designer-seat subagent passes a pixel-level acceptance on REAL renders;
  FAIL → iterate and resubmit.
- Extreme DRY; files ≤500 lines; WYSIWYG (preview == bake pixels); bake/apply owner-supervised.
- Specs are the intended source of truth — but see §Known doc drift; the old prototype HTML is historical only.

## Blockers

- None for web/core development (M0/M2/M5 run on Mac). M1 spikes + M3/M4/M7/M8
  need the Windows box (SSH/Tailscale, logged-in interactive session). Release
  gates: signing cert (owner), public repo visibility (owner), first version
  number (owner).
