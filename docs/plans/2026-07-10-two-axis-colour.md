# Plan — Two-Axis Colour Reshape (ADR-0018)

Normative: docs/product/two-axis-colour-spec.md + ADR-0018 engine mapping.
Every task: tsc -b + bun test green; end with designer pixel acceptance.

## T1 — Model + migration
- types.ts: `ColorMode`→`Subject` ('Original'|'BlackWhite'|'Mono');
  `colorMode`→`subject`; `fieldBand`→`plateBand` (type `PlateBand`);
  TypePatch: subject limited to 'BlackWhite'|'Mono', plateColor bounded set
  comment; BRIDGE_SCHEMA_VERSION → 4.
- mock-desktop: BASE_CONFIGS/PRESET_TYPE_OVERRIDES rewritten to new fields
  (spec §4.4 preset table; field ladder keeps Folder gold/System BW rows —
  System patch becomes {shape:'Circle', subject:'BlackWhite'}).
- No runtime migration code needed beyond presets (unreleased; mock is the
  only store) — but write `lib/colour-migrate.ts` mapping fn + tests as the
  F8 C# reference implementation of spec §3.2.

## T2 — Engine + pipeline
- compose.ts: branch per ADR-0018 mapping (subject×plateColor); Field
  pipeline runs under subject==='Original' (plate null → derived, hex →
  user-plate branch); BW/Mono keep their pipelines with plate override.
- stores/icons.ts: pool predicate = resolved.subject==='Original' &&
  resolved.plateColor===null && !typeHasFixedPlate; accent fallback
  condition likewise; effectiveTileConfig untouched otherwise.
- icon-renderer styleKey: subject/plateBand fields swap in.
- resolveTypeConfig: rename patch keys (subject/plateBand).

## T3 — Panel
- icons-panel: Colour block → 主体行 + 底板行 per spec §4.1 (subject row:
  原彩 FieldGlyph-三点 / 黑白 / mono dots / wheel→tint picker; plate row:
  随图标 QuadPlateGlyph / 白 / 6 swatches / wheel→plate picker); Tonal|Flat
  disclosure under Mono; 鲜明|柔和 under (随图标 × Original); dual-tab
  ColorEntryPopover DELETED → two single-purpose pickers; plate row
  disabled (40%, hint) when shape==='None'.
- chip-preview: new `QuadPlateGlyph` (4-quadrant FIELD_SLOTS colours,
  band-aware, hairline seams).
- icons-participation accordion: colour row → 主体(跟随/黑白/dots/wheel) +
  底板(跟随/随图标/白/6 swatches) per spec §4.3.
- i18n: Subject_*/Plate_* keys (原彩/黑白/单色/自定义/随图标/白/底色/
  渐层/纯平/鲜明/柔和 + disabled hint), PENDING-RESX.

## T4 — Tests + acceptance
- Update: style-key, colour-field (FIELD_CONFIG → subject/plateBand),
  type-config, hue-spread pool, mock preset matcher tests; new
  colour-migrate.test.ts (8-row table) + matrix smoke (BW×derived,
  Mono×derived, BW×swatch render non-regression).
- Screenshots: global two rows, accordion two rows, BW×随图标 desktop,
  shape=None disabled plate row. Designer pixel acceptance (FAIL→fix→PASS).
- STATE checkpoint + spec 02/06 amendments (colour axes section).
