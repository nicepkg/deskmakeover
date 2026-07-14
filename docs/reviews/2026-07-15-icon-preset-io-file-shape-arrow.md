# Review — Icon reset semantics, preset panel & I/O, File shape, arrow mark (2026-07-15)

Three isolated expert seats (Chief Architect — preset package format; Chief Product
Designer — File shape + arrow mark geometry; Chief UX Engineer — panel layout +
reset-semantics model; same-vendor subagents, fresh context each) reviewed the
owner's icon-page bug report and feature batch. Owner disposed every item the same
day via one 7-question batch (visual candidates rendered to a comparison board
before asking — owner rule: show icons, not text walls). **This document is the
binding detail record for the spec 06 / spec 02 amendments and the new spec 09
dated 2026-07-15.**

## Evidence base

- Owner bug report (verbatim): on the Icon page, clicked 系统默认 then 「全部重置」
  under 美化类型 — the desktop/highlight jumped to the second style card.
  「全部重置应只把下面的项目重置为跟随全局」;「点系统默认不该修改美化类型；美化类型
  应默认显示跟随全局」.
- Code diagnosis: `src/stores/icons.ts` `resetTypeOverrides` cleared `bareLook`
  as a side effect (`bareLook: false`), dropping the System-Default selection and
  letting the highlight/canvas fall back to whatever preset the draft config
  matched. `KindTypeSection` (icons-participation.tsx) reads `typeOverrides`
  directly with no bareLook awareness, so factory preset type-ladders showed
  「自定」 badges while System Default was active — the owner's perceived
  "System Default modified Beautified types".
- Owner feature asks: preset import/export as a zip package (community ecosystem
  groundwork, offline sharing, single source of truth between internal and
  exported structures); history panel not discoverable (renders at the bottom of
  the scroller); preset area occupies too much panel height; a File shape
  (dog-eared, rounded); a beautiful arrow mark so users stop wanting the native
  Win11 arrow.

## Dispositions (owner, 2026-07-15)

| # | Decision | Disposition |
|---|----------|-------------|
| 1 | **Preset area → P-B**: inline keeps ONLY [系统默认][当前风格] two cards + a full-width 「风格库 +N」 trigger strip (~96px constant); ALL presets (built-in 7 + imported N) live in a 风格库 popover — 2-col live-mini grid, hover try-on preserved, scrollable, grows without panel height growth. Trade-off accepted: browsing non-current presets costs one extra click. | accept |
| 2 | **History → H-A**: the 「历史 N」 footer button anchors a popover opening ABOVE the button (side top) — immediately in viewport, zero layout-height cost, canvas stays visible for对照, consistent with the panel's popover grammar. The scroller-tail HistoryStrip placement is retired. | accept |
| 3 | **Reset semantics → the lens model (full)**: bareLook is a read-only preview LENS over the draft, never a draft mutation. Toward-default actions (全部重置 / ↺跟随全局) mutate the draft but never lift the lens; only value-asserting edits lift it. While the lens is down, the Beautified-types area PROJECTS follow-global (badges read 跟随全局, sub-axis anchors selected, 全部重置 and ↺ hidden) and KeptBar is suppressed; the underlying draft is preserved and resurfaces losslessly on exit. The participation checkboxes (kindPolicy) stay REAL — orthogonal to the style lens, deliberately not projected. Bug fix: `resetTypeOverrides` stops clearing bareLook. | accept |
| 4 | **Export scope — kindPolicy stripped by default, opt-in checkbox on export**: payload defaults to config + typeOverrides (pure aesthetics); 「包含参与策略」 is an explicit export-dialog opt-in. Evidence: types.ts records kindPolicy as a per-machine personal choice, deliberately excluded from presets. | accept |
| 5 | **File shape → V2 (c=30)**: five-vertex smooth polygon, top-right 45° cut of 30 units, outer corners r12 (Folder-family weight), cut-edge endpoints r6, s=0.6, fit:false. The only variant that still reads "document" at 32px while only grazing the 67% content box (1.4px margin). Solid cut-away at the SHAPE layer; the folded-page semantics stay with the existing Fold mark (bottom-right) — zero collision by position, layer and semantics. | accept |
| 6 | **Arrow mark → Comet**: neutral adaptive squircle-seat arrow badge — seat `applePolygon` (cornerFactor ≈0.30, ~0.34–0.38·tile), 1px ring + soft drop shadow forming its own visual ground (fixed bottom-left anchor works on pointed/round shapes without hovering), refined ↗ glyph (round caps, thin shaft, bold head), charcoal-seat/white-arrow on light tiles and inverted on dark (0.58 threshold), markColor tints the seat. Beacon (coral disk) ≈ Comet with markColor coral — not a separate style; Quill (seatless) rejected (weakest at 32px). | accept |
| 7 | **Native Win11 arrow — status quo KEPT (owner re-affirmation 2026-07-15)**: the 60s ArrowGateSheet stays exactly as shipped; the native arrow keeps its current position. The designer's "reverse dark pattern" objection and the demote-without-popup alternative were heard and REJECTED by the owner. Note: spec 06 §3.10 claimed the gate was retired (ADR-0021) while the code kept it live — the spec is corrected to match reality + this re-affirmation. | reject (designer proposal); keep owner decree |

## Architect verdicts adopted without owner questions (engineering scope)

- **`.dmpreset`** extension (zip container, rename-to-.zip debuggable); manifest.json
  is the ONLY structural JSON (payload inlined; only binary assets under `assets/`).
- **`entries[]` array from day one** (theme packs later without a breaking format bump).
- **Two-level versioning**: container `format: "dmpreset/1"` + per-entry
  `schemaVersion`, driving an ordered/pure/idempotent migration chain
  (`lib/preset-migrations.ts`); the existing MATERIAL_MIGRATION hack graduates into
  that chain. Fixes the discovered hole: persisted styleJson carried NO version.
- **Single source of truth = one type + one validator + one serializer**
  (`lib/icon-look.ts`: `IconLookPayload` + `serializeIconLook`/`parseIconLook`/
  `normalizeIconLook`), replacing the inline `JSON.stringify` in the store and
  `parseRecipe`. **Built-in presets STAY code constants** (compile-time enum safety;
  a data file would rot silently — the MATERIAL_MIGRATION lesson). The owner's
  single-truth goal is satisfied at the schema layer, not the storage layer.
- **Security pipeline in Rust**, cloning the exportCompare posture: zip-slip
  refusal (never extract by entry name; canonicalize + root assertion), bounded
  decompression (pack ≤20MB, entries ≤64, total ≤100MB, ratio >200:1 rejected,
  no nested archives), strict schema + enum whitelists + numeric clamps + string
  caps, magic-byte image sniffing + re-encode before any OS handoff, fonts
  excluded from v1 packs. Preview stage never writes disk.
- **Import semantics**: into the user preset library (`data_dir/presets/<id>/`,
  disk format == package format), never auto-apply; id collision → import-as-copy
  (never silent overwrite); partial success is first-class with per-entry reasons.
- **Tauri**: 4 typed commands (`presets.list/import/export/delete`, bridge schema
  → 9), thumbnails via a `dmpreset://` protocol (clone of dmicon://), zip via the
  Rust `zip` crate, narrow `tauri-plugin-dialog` grants (open+save only), plus
  file drag-drop onto the window as the second import path.
- v1 packs are unsigned; `signature`/`publicKeyId` fields reserved, honesty note
  in docs (no fake author verification).

## UX verdicts adopted without owner questions

- Import/export/save entry = the 风格库 popover header toolbar
  ([导入][导出当前][保存为我的风格]); rename/delete on 「我的」 cards via hover ⋯ menu.
- **Export source = the CURRENT recipe** (draft), not the selected preset — the
  main use case is sharing a custom look (activePresetId may be null); built-ins
  ship with the app anyway. 「保存为我的风格」 and 「导出当前」 are the same recipe
  through two exits (library vs file).

## Rejected / deferred (with reasons)

- Native-arrow demotion + gate removal — rejected by owner (decision 7).
- P-A horizontal preset rail — rejected (new scroll paradigm, poor at 20+ presets).
- P-C keep-4-cards — not chosen (fails the height complaint).
- Quill seatless arrow as a catalog entry — rejected at Q6 (32px illegibility);
  candidate record kept here for future 极简 option discussion.
- Anchor-nudge of the badge toward centroid on pointed shapes — optional
  enhancement deferred (fixed anchor + shadow suffices; avoids shape-specific
  parity surface).
- Signed packages / preset marketplace — out of v1 scope; format fields reserved.

## Build record (same day, P1–P7 per the plan)

- P1 lens model: `resetTypeOverrides`/`setKindPolicy` no longer clear `bareLook`;
  `setTypeOverride` keeps the lens on its clear branch; KindTypeSection projects
  follow-global while the lens is down (one-line projection:
  `typeOverrides = bareLook ? undefined : raw`), `patchType` merges onto the
  REAL draft; KeptBar suppressed; 5 new store tests.
- P2 panel: inline area = [系统默认][当前风格] + 「风格库 +N」 strip; new
  `icons-style-library.tsx` (PresetCard/PresetMinis moved here — one-way
  import); `resumeDraft` store action (click 当前风格 lifts the lens); history
  = popover `side="top"` on the footer button; `IconAction` learned rest-prop
  spreading for Radix asChild.
- P3 single truth: `lib/icon-look.ts` (serialize/parse/normalize, versioned
  `v:1`, MustCover compile probes force whitelist updates on union growth) +
  `lib/preset-migrations.ts` (icon chain v0→1 backfills pre-ADR-0018 fields;
  wallpaper MATERIAL/TITLE maps graduated); store + parseRecipe + wallpaper
  loader delegate; 13 new tests.
- P4 host: `preset_store.rs` (bounded unzip ≤20MB/64 entries/100MB budget/
  ratio≤200:1/no nested archives; read-by-name only — nothing extracts to
  paths; id charset + root assertion; string caps + control-strip; PNG-only
  sniff+decode probe; staging-rename atomic save; create_new export with
  half-file cleanup) + 6 typed commands + `dmpreset://` protocol + narrow
  dialog grants; bridge schema 9; 11 Rust tests. Design refinement recorded in
  spec 09 §6: `import` split into PURE `readPackage` + `save` so the ONE
  validator stays in TS and preview-before-write falls out structurally.
- P5 UI: popover toolbar [导入][导出当前][保存为我的风格]; 「内置/我的」 groups;
  ⋯ menu (inline rename / export / delete); import preview sheet (per-entry
  status, partial success); kindPolicy export opt-in checkbox; window
  drag-drop import; `preset-library` store validates every entry through
  `parseIconLook` and RE-SERIALIZES the validated payload (junk fields never
  enter the library); import-as-copy on id collision.
- P6 File shape: Rust vertex table + TS chip authoring (same numbers), ABI tag
  12 both sides, catalogs + i18n (文件/Document); playwright-verified at grid
  size.
- P7 Comet: `CometMark` (Apple-squircle seat 0.36·S bottom-left + blurred
  drop shadow + 1px ring + refined capsule ↗, adaptive 0.58, markColor tints
  seat with luminance-picked ink), ABI tag 7 both sides, MARKS first slot,
  chip glyph, i18n (箭头徽章/Arrow badge); playwright-verified on Apple +
  Circle (self-grounding confirmed). The frozen TS oracle carries a
  throw-on-use Comet stub (post-dates the freeze; WASM-only style).
- Gates: tsc 0 · bun 632 pass/1 skip · cargo deskmakeover-desktop 49 pass
  (incl. bindings drift) · dm-icon-core/wasm/operations suites pass · WASM
  rebuilt. ArrowGateSheet untouched (decision #7 verified by diff scope).

## Codex cross-review (post-build, run-20260714195508) — verdict FIX-6, all fixed

Reviewer verified lens behavior, File/Comet ABI ordering, history placement, and
the native-arrow gate showed NO violations. Six Major findings, all accepted +
fixed the same day with adversarial regression tests:

| # | Finding | Disposition |
|---|---------|-------------|
| F1 | `kindPolicy` opt-in was write-only — export could bundle it but import discarded it, making owner decision #4's checkbox vacuous | **accept, fixed** — `parseIconLookPayload` preserves the kindPolicy PRESENCE signal; `PresetRecipe`/`LibraryPreset`/`ImportCandidate` carry optional `kindPolicy`; `selectRecipe(config, typeOverrides, kindPolicy?)` adopts a bundled participation on apply, leaves it untouched for style-only/community presets. Round-trip test added; the loss-codifying test rewritten |
| F2 | Import preview trusted the package's embedded thumbnail instead of rendering the validated recipe (spec 09 §2 — thumbnail is an untrusted hint) | **accept, fixed** — the preview sheet renders `PresetMinis` from `c.recipe.config` (the live compositor on the user's own icons, the authoritative preview); the supplied thumbnail is no longer shown as proof |
| F3 | Two package entries sharing an id (both absent from the library) — the second failed `exists` instead of import-as-copy (`existing` computed once, never updated) | **accept, fixed** — `existing.add(entry.id)` after each save; per-batch dedup test added |
| F4 | Thumbnail validation decoded before the pixel check and stored hostile bytes verbatim (no bounded decode / re-encode) | **accept, fixed** — `reencode_thumb_png`: magic → IHDR dimension check (≤4M px) BEFORE decode → decode → fresh canonical PNG from our encoder; package reads re-encode too. Tainted-trailer + IHDR-bomb tests added |
| F5 | Archive-wide zip-slip / ratio / nested-archive checks were missing — only manifest + referenced thumbs were screened; a hostile UNREFERENCED entry passed | **accept, fixed** — `screen_archive` scans EVERY central-directory entry (name traversal/absolute/drive/NUL/backslash, declared size, ratio ≤200:1, aggregate budget, compression method, nested-archive extension) and fail-closes the container. Three hostile-entry tests added |
| F6 | Overwrite removed the old entry BEFORE staging the replacement — a mid-write failure lost it permanently; `rename` truncated `entry.json` in place | **accept, fixed** — stage-first swap: fully write the sibling → park the old at `.bak-` → swap in → drop the backup, with rollback on failure; `rename` uses temp-file + rename; the lister skips `.tmp-`/`.bak-` dirs. Overwrite-failure-keeps-old test added |

## Codex focused security re-review (run-20260714200xxx) — verdict FIX-4

A second, security-focused pass on the F4/F5/F6 fixes CONFIRMED the core hardening
(bounded thumbnail decode+re-encode, archive-wide screening via `by_index_raw`,
div-safe ratio, id charset) and found 4 residual durability/platform findings:

| # | Finding | Disposition |
|---|---------|-------------|
| F4-1 | High — overwrite not crash-atomic: a crash between `dir→.bak-` and `.tmp-→dir` leaves no canonical entry; `list()` hid the backup and a later save would drop it | **accept, fixed** — `recover()` runs in `PresetStore::new()` (before any command): an orphaned `.bak-<id>` with a missing canonical is restored, a superseded one dropped, stale `.tmp-<id>` swept. Two recovery tests added (interrupted-swap restore + superseded-backup drop) |
| F4-2 | High — `rename(tmp, entry.json)` may not replace an existing dest on Windows | **no change needed** — `std::fs::rename` replaces an existing FILE destination on every platform (Windows via `MOVEFILE_REPLACE_EXISTING`); the dir-rename Windows trap does not apply here (the staging→dir swap only ever renames onto a NON-existent path, since the old dir was moved to `.bak` first). Rationale documented at the call site; still `[WINDOWS-VERIFY]` like the whole host |
| F4-3 | Medium — deterministic `.tmp-`/`.bak-` names + no synchronization → concurrent same-id invocations race | **accept, fixed** — a `write_lock: Mutex<()>` serializes `save`/`delete`/`rename` |
| F4-4 | Low — library-preset selected-state ignored bundled `kindPolicy` (a policy-differing preset read as "current") | **accept, fixed** — `kindPolicyMatches` folds the bundled policy into the selected check; style-only presets skip it |

Post-fix gates: tsc 0 · bun 633 pass/1 skip · cargo preset_store **16 pass** ·
cargo deskmakeover-desktop suite green · bindings ok · WASM rebuilt.

## Artifacts

- Visual comparison board: scratchpad `design-board.png` (File V1/V2/V3 at
  96px+32px with content-box guides; Comet/Beacon/Quill × 4 shapes × light/dark
  + 32px acid test). Geometry paths in the designer report are engine-true
  (ported `smoothShapePathD`), regression-checked against the Folder definition.
