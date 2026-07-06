# ADR-0007 · Shortcut distinction v3 — stacked cards

Status: Partially superseded by [ADR-0008](0008-prototype-v2-ui-contract.md) · 2026-07-06 —
stacked cards survive as 双层卡片, one of seven mark styles in the v2-prototype gallery;
the "single automatic mark, no glyph/colour options" decision is reversed.
Supersedes the arrow/arc/sash/notch marks of [ADR-0006](0006-badge-v2-adaptive-arrow.md).
Driven by a 4th owner rejection ("太丑了…一点审美都没有") + two independent top design
panels that **converged on the same answer**.

## Context

Every prior distinction (coral enamel arc, sash, notch, adaptive B/W arrow) was rejected
as ugly. The owner then loosened the brief: **it need not be an arrow — any elegant way
to signal "this is a shortcut" is fine.**

Two independent designers (an OS-visual-systems lens and an icon-craft lens) reached the
*same* diagnosis and the *same* recommendation:

- **Diagnosis — the "stick a badge in the corner" paradigm is the problem, not the glyph.**
  A unified icon is one self-contained object; adding anything to its corner crams two
  light sources / two edges / two materials into 40px → foreign-object feel, a halo
  machine (the light seat/ring needed for legibility blurs to a hard edge), a contrast
  arms race, no size sweet-spot. The rejected adaptive arrow was that spiral maxed out.

## Decision — stacked cards (silhouette, not badge)

Render the shortcut as **two stacked cards**: the styled icon (top card) with a
**same-shape sibling card peeking out at the bottom-right** — the visual grammar of
"this points to / is a copy of another thing". It changes the icon's **silhouette**, not
its surface, so it reads on any colour or wallpaper and needs no foreign glyph.

- Sibling card = the icon's own alpha, offset ~(8.5%, 9.5%) bottom-right, at ~0.9 scale.
- **Adaptive neutral tone** (never the icon's colour): light behind a dark icon, dark
  behind a light one — separates on any surface.
- A **seam shadow** (top card onto the sibling) lifts the top card; a soft **grounding
  shadow** anchors the stack. All via a cheap separable box blur.
- Zero shape special-casing — circle / squircle / Samsung just translate the same alpha.
- **Pure algorithmic** (no GPT Image 2): deterministic, per-icon consistent, crisp at
  16px; a generated bitmap would drift, blur, and can't sample the base for adaptivity.

The three states become: **美化** = stacked cards · **保留原样** = the classic Windows
arrow (unchanged) · **去除** = nothing. Glyph and mark-colour options are gone (the mark
is automatic + adaptive), which also removes a whole cognitive-load surface.

## Consequences

- `OverlayBadgeIconFactory.ApplyMark` now composites the stack (top-card downscale +
  sibling alpha + shadows). `BadgeGlyph` / mark-colour are vestigial; the toolbar's
  glyph/colour popover is hidden.
- Footprint grows ~8% bottom-right — absorbed by grid whitespace.
- Baked per-icon (ADR-0006 fact 1 still holds); preview == real desktop.
