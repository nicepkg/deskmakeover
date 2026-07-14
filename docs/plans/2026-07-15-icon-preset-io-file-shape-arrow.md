# Plan — Icon reset lens, panel P-B/H-A, preset packages, File shape, Comet mark (2026-07-15)

Owner-disposed design: `docs/reviews/2026-07-15-icon-preset-io-file-shape-arrow.md`.
Normative specs: 06 §2/§3.10/§3.13/§3.14 · 02 §Shape System/§Shortcut Marks · 09 (new).

Global constraints
- No behavior change to the ArrowGateSheet / native-arrow option (owner decision #7).
- Hover try-on (90ms rest) and live renderer minis survive every layout move verbatim.
- One serializer/validator path (`lib/icon-look.ts`); no second JSON.stringify of recipes.
- All fs/zip/validation in Rust; frontend touches only path strings + typed DTOs.
- Gates per phase: `bun run tsc -b` · `bun test ./tests/` · `bun run check:bindings`
  (when contracts change) · `cargo test -p dm-icon-core` (when engine changes) ·
  `bun run wasm` rebuild + playwright board (visual phases).

## P1 — Lens model (bug fix + projection) [frontend]
- `src/stores/icons.ts`: `resetTypeOverrides` and `setKindPolicy` stop clearing
  `bareLook`; `setTypeOverride` keeps the lens on its clear-to-follow branch
  (entry null / empty patch) and lifts it only when writing a custom patch.
- `src/components/panels/icons-participation.tsx`: `KindTypeSection` +
  `KeptBar` subscribe to `bareLook`; while true — display patches project to {}
  (badges 跟随全局, AutoGlyph anchors selected), `anyCustom` → false (hides
  全部重置), per-bucket ↺ hidden, KeptBar returns null; participation
  checkboxes keep rendering real `kindPolicy`.
- Tests (`tests/`): reset-keeps-lens, kind-policy-keeps-lens, custom-patch-lifts,
  clear-branch-keeps, draft survives lens round-trip.

## P2 — Panel layout P-B + history H-A [frontend]
- `src/components/panels/icons-panel.tsx`: inline preset area → exactly
  [系统默认][当前风格] two cards + full-width 「风格库 +N」 trigger strip; delete
  the 4-card grid + 「更多风格」 fold; 当前风格 card = live minis of the DRAFT
  config, name = active preset name ?? 自定义, selected iff `!bareLook`.
- New `src/components/panels/icons-style-library.tsx`: the 风格库 popover —
  2-col grid of preset cards (live minis + hover try-on + click select),
  scrollable (max-height + inner scroll), opens from the trigger strip.
- History: replace the scroller-tail HistoryStrip block with a Popover anchored
  on the footer 「历史 N」 button, `side="top"`; HistoryStrip renders inside
  (thumbs + 回到此版 + 回到最初 unchanged; close before ceremony).
- i18n: `Preset_Library` 风格库 keys en+zh; remove dead fold keys if unused.

## P3 — Single-truth serializer + versioning + migrations [frontend]
- New `src/lib/icon-look.ts`: `IconLookPayload`, `serializeIconLook` (writes
  `v: 1`), `parseIconLook` (accepts legacy v-less), `normalizeIconLook`
  (enum whitelists, numeric clamps, string caps).
- New `src/lib/preset-migrations.ts`: ordered pure chains; icon v0→v1;
  wallpaper chain absorbs `MATERIAL_MIGRATION` (wallpaper-assemble delegates).
- `src/stores/icons.ts` apply path + `src/lib/icons-assemble.ts` `parseRecipe`
  route through icon-look; history parsing migrates forward.
- Tests: round-trip, clamp table, legacy migration, unknown-enum rejection.

## P4 — Rust preset store + bridge commands [host]
- Cargo: `zip` crate; `tauri-plugin-dialog` (grants: open+save only in
  `src-tauri/capabilities/main.json`).
- New `src-tauri/src/preset_store.rs`: bounded unzip (≤20MB pack / ≤64 entries /
  ≤100MB total / ratio ≤200:1 / no nested archives / zip-slip refusal),
  manifest serde + validation, library CRUD under `data_dir/presets/<id>/`,
  atomic export writer (create_new), thumbnail bytes.
- `dm-contracts`: `PresetPackageDto`, `PresetMetaDto`, `ImportResultDto`;
  commands `presets_list/import/export/save/delete/rename`; `dmpreset://`
  protocol registration; `BRIDGE_SCHEMA_VERSION` → 9; regen bindings.
- `src/bridge/{types,tauri,mock}.ts`: BridgeMethods + Tauri impl + in-memory
  mock library (web dev works end-to-end).
- Rust tests: zip bombs/slip fixtures, import→list→delete round-trip, atomic
  export non-overwrite.

## P5 — Library UI + import/export flow [frontend]
- 风格库 popover gains: 「内置 / 我的」 groups; 「我的」 card hover ⋯ menu
  (重命名/删除/导出); header toolbar [导入][导出当前][保存为我的风格].
- Flows: 导入 = dialog-open (plugin) → `presets.import` → preview/confirm sheet
  (per-entry name/author/thumb/status; partial failures listed) → library
  refresh. 导出当前 = name+「包含参与策略」checkbox → save dialog →
  `presets.export`. 保存为我的风格 = name prompt → `presets.save`.
- Drag-drop: Tauri file-drop event (`.dmpreset`) → same import flow; mock via
  HTML5 drop.
- i18n en+zh for all new strings.

## P6 — File shape [engine + frontend]
- `crates/dm-icon-core/src/shapes/mod.rs`: `IconShape::File` + vertex table
  (spec 02: 5 vertices, c=30, r12 outer / r6 cut-edge, s=0.6, fit:false) wired
  through polygon/smooth/contains/facts paths; check `shape_facts.rs` margins.
- ABI: shape index mapping (`src/icon-wasm/config-abi.ts` ↔ core `config.rs`);
  `dm-contracts` IconShape enum; regen bindings; `bun run wasm`.
- TS: `src/icon-compositor/shapes.ts` SMOOTH_SHAPES entry (chip path only) +
  `src/lib/shape-paths.ts` case + `icon-axis-options.ts` MORE_SHAPES + i18n
  `Shape_File` 文件/File.
- Tests: cargo shape tests + bun geometry/chip tests; parity fixtures updated.

## P7 — Comet mark [engine + frontend]
- `crates/dm-icon-core/src/marks/styles.rs`: Comet per spec 02 row (squircle
  seat via applePolygon ≈0.36·S, ring, shadow, refined ↗ glyph, 0.58 adaptive,
  markColor tints seat, `over`, no carve); `MarkStyle::Comet` through ABI +
  contracts; regen; `bun run wasm`.
- TS: `MarkStyle` union; `MARKS` list — Comet FIRST (curated row); `MarkGlyph`
  comet swatch in `chip-preview.tsx`; i18n `Mark_Comet` 箭头徽章/Arrow badge.
- Visual verify: playwright board — Comet on Apple/Circle/Diamond/Teardrop,
  light+dark, 32px crop.

## P8 — Gates, adversarial review, records
- Full gates: tsc · bun test · cargo test (workspace icon crates + desktop) ·
  check:bindings · wasm build · playwright boards (panel two-card layout,
  风格库 popover, history popover, import/export round-trip, File shape, Comet).
- Codex cross-review (`/multi-ai` solo codex) on: preset_store.rs security,
  icon-look/migrations, store lens changes, panel rework. Fix or disposition
  every finding; record in the review doc.
- STATE.md active-work entry + this plan's completion notes.
