# Plan — Per-Type Distinction System (ADR-0017)

Owner-approved 2026-07-10 (panel disposed; docs: ADR-0017, spec 06 §6.5).
Web mock first; C# host items tagged F8. Global constraints: subject pixels
never recoloured; filter/Original stay global; all changes `tsc -b` + full
`bun test` green per task; visual changes end with designer acceptance.

## T1 — Types & taxonomy plumbing
- `src/bridge/types.ts`: delete `ConfigDto.kindShapes`; add `TypePatch`
  (`shape? colorMode?('Field'|'Mono'|'BlackWhite') tint? fieldBand?
  plateColor?`), `TypeOverrideEntry {source:'global'|'custom'; patch?}`,
  `TypeOverrides = Partial<Record<IconKindBucket, TypeOverrideEntry>>`; add
  `kind 'ExecutableFile'`; look payloads (`icons.setLook`, state, presets)
  carry `typeOverrides` beside `config`.
- `src/lib/kind-policy.ts`: `ExecutableFile → 'App'`; bucket label key App →
  程序 (i18n only, enum unchanged).
- `src/bridge/mock-desktop.ts`: `isShortcut` includes `AppxShortcut` (bug
  fix); add 2-3 bare-exe fixtures (`kind:'ExecutableFile'`); presets become
  `{config, typeOverrides}` — factory default ladder: base Apple+Field;
  Folder{shape:Bookmark} · File{shape:Tile} · System{shape:Circle,
  colorMode:'Mono'}; 极简白/安静/原彩保真 keep uniform overrides ({}).
- F8 notes (docs only): host ext→ExecutableFile (v1 `.exe`), Appx mark fix.

## T2 — Resolve chain + store
- New `src/lib/type-config.ts`: `resolveTypeConfig(base, overrides, bucket):
  ConfigDto` pure merge; unit-tested.
- `src/stores/icons.ts`: hold `typeOverrides`; per-icon render path feeds
  RESOLVED config (per bucket) into renderer + styleKey + bake; hue-spread
  pool filters to resolved-Field icons whose bucket has no fixed plateColor;
  `setTypeOverride(bucket, entry)` mutation + preset matcher includes
  typeOverrides; scope-highlight state `editingBucket: Bucket | null`.
- `src/icon-compositor/compose.ts`: delete `KIND_SHAPES` + `config.kindShapes`
  branch (shape arrives resolved); keep RenderOpts.kindBucket only if still
  needed by styleKey seeds — otherwise remove.
- Shortcut uniform shape: `shortcutShape: IconShape | null` (default null) in
  look state; resolve order `resolve(bucket) → shortcutShape override when
  isShortcut → mark badge`.

## T3 — Panel
- `icons-panel.tsx`: remove the One-shape/By-type segmented from Shape.
- Beautified-types area → type accordion: one row per bucket — summary
  (label · shape name · 显著性 · Custom badge), beautify check preserved,
  expand-to-edit (reuse SwatchPicker/Segmented controls bound to the patch),
  跟随全局|自定义 + 恢复跟随 reset, one open at a time; expanding sets
  `editingBucket` (canvas dims non-scope icons); collapse clears it.
- Shortcut mark area: add 「快捷方式统一形状」 toggle (default off) + shape
  swatch row when on; type Shape sections show ghost note while active.
- i18n: en + zh-hans for all new strings (PENDING-RESX convention).

## T4 — Tests + verification
- `tests/type-config.test.ts`: resolve merge, global vs custom, plateColor
  pool-exit predicate, shortcut shape layering.
- Update `tests/style-key.test.ts` (kindShapes → resolved-config hash),
  `tests/hue-spread.test.ts` (pool filter), colour-field fixtures.
- Full suite + tsc; mock desktop screenshot of the new factory default.

## T5 — Acceptance
- Designer subagent pixel-level acceptance of the default ladder on the real
  124-icon pack (standing order); owner review of the new default look.
- STATE.md checkpoint; journal sweep of superseded kindShapes entries.
