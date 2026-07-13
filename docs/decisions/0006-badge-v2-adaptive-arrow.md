# ADR-0006 · Shortcut badge v2 — the refined adaptive arrow

Status: Mark design superseded by [ADR-0007](0007-stacked-card-distinction.md) then
[ADR-0008](0008-prototype-v2-ui-contract.md); the adaptive native-arrow **semantics** are further
superseded by [ADR-0021](0021-global-arrow-overlay-default.md) (the global transparent overlay is
now the default; the 60s penance gate retired). The remaining **engineering facts stay binding**
(per-icon `.ico` bake, transparent global overlay, 16–256 size ladder, alpha-edge adaptive ink —
the latter lives on inside the 玻璃箭头 style and the 自动 mark colour). · 2026-07-06
Supersedes the distinction-mark design of [ADR-0005](0005-distinction-shape-color-system.md)
(the enamel-arc / sash / notch marks). Driven by owner rejection ("太丑了太丑了太丑了")
+ a top icon-designer panel.

## Context

The v1 marks (coral enamel arc, satin sash, ticket notch) were rejected: ugly, and
the arc only fit the Apple squircle — it did not hug the pure circle. The owner asked
for the shortcut **arrow** done well: refined, auto black/white by local brightness,
and hugging every shape. The panel also surfaced two blocking engineering facts.

## Decision — the mark is a refined, adaptively-inked arrow

- **Form**: a solid 45° arrow (stem + triangle head) at the **bottom-left** (the
  historical shortcut-arrow slot), on a **frosted, shape-hugging seat**. Variants:
  `ArrowSolid` (default) / `ArrowLine` (chevron) / `ArrowBare` (no seat). The arc /
  sash / notch are gone.
- **Adaptive ink (WCAG)**: sample the icon pixels under the mark (coverage-weighted),
  convert to WCAG relative luminance, and pick **black** ink above L\*=0.1791 else
  **white** — whichever contrasts more. Add a **reverse-colour separation ring** when
  single-colour contrast < 3.0 (mid-grey) or local variance > 0.22 (straddling an
  edge). A **user colour** always gets a reverse ring. Default mark colour = **null =
  auto** (coral is demoted to just one swatch; a fixed colour never survives all icons).
- **Shape-hugging with zero shape branches**: anchor to the icon's **own alpha edge**
  (step out along the down-left diagonal until alpha<40), seat = disc ∩ icon-alpha, and
  everything is multiplied by the icon alpha. The outer edge therefore equals the real
  icon curve — circle, squircle, or Samsung — with no per-shape code. Uses the alpha
  edge (not the mask) because already-round art is kept unmasked (`IsRoundish`).
- **GPT Image 2 is NOT used for the final mark** — 16–48px marks must be hinted,
  re-tinted at runtime (the adaptive ink), and edge-clipped; a baked bitmap blurs and
  freezes the colour. Algorithmic drawing is strictly better; gpt-image-2 is only a
  shape-template source if ever needed.

## Two blocking engineering facts (from the panel)

1. **The real desktop stamps ONE global overlay image on every shortcut** (registry
   `Shell Icons\29`), so it cannot know per-icon brightness or shape. The adaptive
   mark therefore **cannot** live in that overlay. **Fix (pending impl):** bake the
   mark into **each per-icon generated `.ico`** in `MakeoverService.ApplyAsync` (same
   `ApplyMark` path as the preview) and set the registry overlay **transparent**. Also
   emit the icon at 16/20/24/32/48/256 (small sizes hinted) instead of one 256.
2. **`CreateRefinedMarkIco()` was broken** — it fed a fully transparent canvas to the
   alpha-edge-riding renderer → an empty ico → the real "美化" state had *no* mark. Now
   returns a transparent ico by design (the mark is baked per-icon, per fact 1).

## Logo

App icon = **实心星 (solid cream 4-point sparkle on coral)** — block language that
reads at 30px (the wand's thin lines collapsed when small). True superellipse squircle
mask (22.37%/n=5), not a rounded-rect.

## Consequences

- Preview (what the owner sees) renders the adaptive arrow per icon — done.
- Real-desktop parity requires the per-icon bake (fact 1) — tracked in STATE, not yet
  wired into `ApplyAsync` / `GeneratedIconStore`.
- `BadgeGlyph` = ArrowSolid/ArrowLine/ArrowBare; `MarkColor`/`ActiveMarkColor` are `int?`
  (null = auto); combos default to auto.
