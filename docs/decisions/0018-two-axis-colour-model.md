# ADR-0018 — Two-Axis Colour Model (主体 × 底板)

- **Status**: Accepted (owner-approved 2026-07-10, 「批准，现在就做」)
- **Panel**: chief PM absent by scope; chief UX + chief UI + chief designer —
  independent verdicts, 3/3 convergence. Normative design:
  `docs/product/two-axis-colour-spec.md` (chief-designer, owner-ratified).

## Context

The owner hit the wall himself: with Original colours + a shape picked, the
white plates could not be recoloured anywhere (the type accordion hid its
colour/plate rows under global Original), and he judged the whole colour
logic 乱 — proposing a foreground/background split. The panel's diagnosis:
`ColorMode` is a FALSE single axis. Field is not a foreground treatment at
all — it is "subject untouched × per-icon derived plate"; the mode framing
welded plate behaviour into a foreground enum, spawning plateColor's
per-mode semantics chart and the recolour dead-end.

## Decision

Dissolve `colorMode` into two orthogonal axes:

- **主体 Subject** = `'Original' | 'BlackWhite' | 'Mono'` (+tint/monoStyle) —
  how the ARTWORK renders. Never called 前景色 (Original is "don't touch",
  not a colour pick).
- **底板 Plate** = `plateColor: null(随图标 derived) | '#FFFFFF' | swatch hex
  | custom hex` (+`plateBand` Vivid|Quiet, renamed from fieldBand) — what
  fills the container. First stop is ALWAYS 随图标 (the derivation
  algorithm IS the product's soul; the guardrail against "all apps one
  colour").
- 满彩/Field demotes from mode to the DEFAULT PRESET COORDINATE
  (Original × 随图标·鲜明); presets become coordinate bookmarks; the word
  "mode" leaves the UI; the fg/bg dual-tab palette popover dies (each axis
  wheel opens a single-purpose picker).
- 随图标 = "plate follows the DOMINANT SIGNAL": subject Original → icon's
  own colour (themedContrastTone / neutral law / App accent special case);
  subject BW → neutral lightness board; subject Mono → the tint's ramp
  light end (same-hue, no clash by construction).
- 无板 is NOT a plate stop — `shape=None` already owns that meaning; with
  shape None the plate row DISABLES (never hides — hiding was the original
  dead-end's root).
- Type rows: same two axes stepped down — subject loses Original (no colour
  islands), plate loses the free wheel (bounded six + white + 随图标 +
  follow-global anchor). One sentence: 类型只能退下去，不能跳出来.
- Migration: deterministic 8-row mapping (spec §3.2), appearance-preserving;
  schema 3→4 BEFORE F8 (single C# port); translation layer VETOED (net-new
  combos like BW×随图标 cannot map back).
- Net-new lawful combos: BW×随图标, Mono×随图标 (formalized), BW×swatch —
  the old "BW plate inert until v2" debt pays itself.
- Owner-ruled open questions: preset truth = web mock (NamedStyle.cs is
  frozen schema-1 legacy); 本色 = Original × White; old BW stored
  plateColor ACTIVATES; no extra per-type tint saturation cap in v1.

## Amendment 1 — the faithful/minimal collapse (found in implementation dry-run)

The spec's first preset table dropped 本色 and 极简白 onto the same
coordinate (Original × #FFFFFF). Their real difference: 极简白 = fixed
white OVERRIDES everything (own boards included); 本色 = own boards
anchored 1:1, bare icons fall back WHITE. Ruling: `plateFallback:
'derived' | 'white'` (meaningful only when plateColor is null) — 满彩 =
(Original, null, derived); 本色 = (Original, null, white) and becomes the
plate row's fifth stop (本色/Faithful chip: anchors-else-white). Bounded
plate for types = chroma ceiling, not a fixed six (designer correction:
the factory Folder gold #65470D is lawful authored plate).

## Engine mapping (implementation contract)

`renderTile` branches on (subject, plateColor):
- Original × null → the FIELD pipeline unchanged (profile rim law, hue
  spread, accents, silhouette shadows).
- Original × hex → the Field user-plate branch (fixed plate + shadows) —
  the old `Original+plateColor` look folds into this single cell.
- BlackWhite × null → BW subject on `neutralContrastTone` board.
- BlackWhite × hex → BW subject on the fixed plate.
- Mono × null → the existing Mono ramp-light-end plate.
- Mono × hex → Mono on fixed plate.
- shape None → no plate regardless (plate row disabled in UI).

## Consequences

- ConfigDto/TypePatch reshape + preset/history/typeOverrides migration in
  the mock (unreleased: no user data, presets rewritten in place).
- styleKey/hue-spread predicates re-express: pool = subject Original ×
  plate null (× no fixed-plate type); simpler than the colorMode test.
- The 玻璃/filters, marks, shortcutShape, shape axes are untouched.
- Designer holds the acceptance seat for the rebuilt panel (pixel gate
  against the shipped chip grammar).
