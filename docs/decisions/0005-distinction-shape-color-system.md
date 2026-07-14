# ADR-0005 · Shortcut distinction, shape dimension, and the colour system

Status: Accepted · 2026-07-06 · Amended by [ADR-0008](0008-prototype-v2-ui-contract.md):
三星 shape = the official One UI mask path (not superellipse r=0.40/n=4); the mark form
= the seven-style gallery (the 珐琅/缎带/票根 candidates are dead). Governance (VOC
3-state, default 美化, 3-second misread gate) and the one-picker colour system stand.
Supersedes the badge parts of [ADR-0003](0003-voc-driven-product-revisions.md).
**Later reversal (owner 2026-07-07):** the "default = 美化 (mark ON), no default-none" governance
clause no longer holds — the default shortcut mark is now **None** (presets ship None; owner decree
2026-07-07, resolved and locked by test in `d1f507d`; see
[ADR-0017](0017-per-type-distinction-system.md) + `docs/ship-readiness.md`).
Inputs: an internal VOC v1 analysis (founder position §1.1, constraint §4.3-1, comment cluster A —
held outside this repo, not shipped with it) + a PM/UI/UX expert panel.

## Context

The "makeover" is now a composable transform: every desktop icon is an image, and
we batch-generate a new image from it. Three user-facing axes compose freely:
**shape × colour × shortcut-distinction**. Three failed distinction designs and one
unilateral engineering call ("default no mark") forced a governance + design reset.

## Decision 1 — Shortcut distinction (the hard one)

**Governance first.** Whether to distinguish, and the acceptance bar, are **not an
engineer's call** — they follow the VOC and an expert panel. Engineers choose the
*form* within the constraints and present ≥3 candidates; they never ship unilaterally
and never "dodge" with default-none.

**VOC is binding:**
- Distinction is **required**, not optional (constraint §4.3-1; comment cluster A was
  the highest-upvoted objection — "去掉怎么区分快捷方式?").
- Three states: **美化 (beautify) / 保留 (keep native arrow) / 去除 (remove)**.
- **Default = 美化.** Default-none == default-remove == crosses the VOC red line.

**Why the three prior attempts failed — one structural error:** each made the mark an
*attached corner object* (ribbon, pearl) that collides with a burned-in symbol.
Notification anxiety = **top-right + solid circle + high-contrast pop + overlaid
object**. Reverse all four.

**The design (panel decision):**
- **Bottom-left**, the historical shortcut-arrow slot — semantically "points
  elsewhere", and free of the notification/promo symbol zones.
- **Grown into the icon's own edge**, not a sticker. Default form = **珐琅描角
  (enamel arc)**: a soft coral glow hugging the bottom-left outline.
- **Rides the icon's own alpha edge** (the icon is already clipped to the selected
  shape, so its edge *is* that Apple/circle/Samsung curve) — never a re-derived mask
  that drifts. Silky: radial + angular smoothstep falloff, no hard edge anywhere.
- **Contrast-adaptive:** bright coral on dark icons, deep coral on light, faint dark
  moat for same-hue — same presence on any icon colour (no fixed colour survives all).
- Candidate marks (user-selectable): **珐琅 (enamel arc, default) / 缎带 (satin sash) /
  票根 (ticket notch)**. Colour is user-customisable (see Decision 3).

**UX model:** distinction is a **low-frequency error-prevention** need (delete / drag
the source file), not an ambient signal — so the mark stays "眯眼消失、凑近可辨".
The 3-state preference lives in **settings**; the main screen carries only high-frequency
aesthetic choices.

**Acceptance gate (replaces "I think it's ugly"):** the **3-second misread test** — show
the marked icon, context-free, for 3s; if anyone reads "unread / notification / promo /
badge", it is rejected, no debate.

## Decision 2 — Shape dimension

Three shapes are three points on one superellipse family `|x/a|ⁿ + |y/a|ⁿ = 1` +
corner-radius ratio `r`, so one parameterised `ContinuousCornerMask(r, n)` serves all
(`ShapeGeometry.ParamsFor`):

| Shape | r | n | Feel |
|-------|-----|-----|------|
| 苹果 squircle | 0.2237 | 5 | flat sides + continuous-curvature corners (iOS grid) |
| 三星 One UI | 0.40 | 4 | fuller, "candy" |
| 纯圆 | 0.50 | 2 | true circle |

**Pure circle keeps already-round icons unchanged** (`IconStyler.IsRoundish`: empty
corners + a full disc → left as-is); square/opaque icons are clipped to a disc.

Shape is a main-screen icon-only segmented control (外形), beside colour (配色).

## Decision 3 — Colour system (one reusable picker, two consumers)

Colour is customisable in **two** independent places, served by **one** reusable
picker component (DRY): (a) the **icon tint** (单色 style) and (b) the **distinction
mark colour**. The picker = wallpaper-extracted colour + system accent + curated
swatches + hex entry, with a live selection ring. Default mark colour = coral `#FF6F5E`
(never the AI-cliché blue/violet gradient).

## Consequences

- Product = composable **shape × colour × distinction**, each premium and free to mix,
  plus named one-tap presets for non-technical users.
- Aesthetic decisions run through VOC + panel + the 3-second gate; engineers deliver
  candidates, not verdicts.
- Renderer stays one engine consuming a data plan (`IconStylePlan`); marks derive from
  the icon's own alpha edge — no shape-specific special-casing.
