# ADR-0016 — Icon Default Look: Colour-Field Restoration

Status: ACCEPTED (2026-07-10). Amends the icon-look defaults of ADR-0013/0015-era
builds; engine law additions land in spec 02, module contract in spec 06.
Panel record: `docs/reviews/2026-07-10-icon-findability-panel.md`.

## Context

The shipped default preset (Apple minimal: single squircle, white/auto plate,
glyph capped at ~67% linear) was publicly screenshot-shared; users reported the
result looks worse than expected and that visually locating icons is SLOWER than
the stock messy desktop. A four-seat adversarial panel confirmed the mechanism:
the default zeroes both strong preattentive search channels (colour field via the
strict-plate-gate → white fallback path; silhouette via the unconditional single
shape), degrading visual search from parallel pop-out to serial label reading.
Single-flat-colour icons that passed the plate gate (Spotify) stayed findable —
proof the recipe, not beautification, is at fault. The product promise is
高效 · 整齐 · 统一 · 高审美 — all four.

## Decisions

**D1 — Default recipe = full-colour brand plate (满彩).** The default look fills
the squircle with the icon's dominant colour (normalised into an OKLab harmony
band: shared lightness/chroma envelope so 124 plates read as one set), renders the
segmented subject as a light/dark knockout sized ~80% linear, and keeps the
container (shape/grid/de-arrowing) as the uniformity carrier. Multi-hue sources
(gradient orbs, photos, rich logos) take a **fidelity lane**: original artwork
preserved on its derived plate colour — the two lanes share the same plate-colour
system and envelope so a mixed desktop still reads as one set (the lane split is a
rendering detail, never a visible style break). A deterministic global hue
de-duplication pass (id-cached) pulls same-hue clusters apart in OKLab so
neighbouring plates stay separable.

**D2 — Kind legibility = colour families + in-envelope affordances.** Folders,
files, and system items get kind-derived plate colour families plus a light
geometric affordance INSIDE the shared container (folder tab, document dog-ear).
A full four-shape kind split (folder-shaped folders etc.) ships as an opt-in
toggle, not the default. (Panel tie 2:2, owner disposition.)

**D3 — Preset lineup rework.** Default = D1 recipe · 极简白 (the previous white
board, now an explicit minority-taste preset) · 安静 (pastel envelope: fixed
lightness, low chroma, per-icon hue — replaces the single-hue wallpaper-tone
mono, which was the Material You trap) · 原彩保真 (native-plate faithful).
Candy/glass is demoted from any recommended slot and reworked (glass as rim
highlight, never a full desaturating wash). Icons with no extractable hue
(photos, near-white, generic files) may take a letter/source-colour badge
fallback — tail-only, never global.

**D4 — Findability is an acceptance gate.** F8 exit adds: (a) owner-supervised
light test — default look, 20 random targets, locate time/error rate not worse
than the stock-desktop threshold; (b) automated neighbouring-plate ΔE
separability check over the mock corpus in bun tests. A default that loses to
the stock desktop on findability may not ship.

**D5 — Engine law: uniformity ≠ flattening.** Uniformity is carried by the
container layer (shape, grid rhythm, lightness/chroma envelope, de-arrowing) and
must never be bought by deleting per-icon hue variance. Recorded in spec 02 as a
governing engine rule so future presets/defaults cannot regress to an all-white
field.

**D6 — Engine additions.** `dominantHue` primitive (memoized, chroma-weighted hue
histogram over the subject mask, ~30-40 lines); white plate fallback replaced by
dominant-colour fallback ("a slightly-off hue beats zero hue"); glyph padding
42/256 → 26/256 (~0.10) and full-bleed threshold 0.88 → 0.82; optional
ultra-light shortcut corner-dot mark (restores the shortcut-vs-real cue without
the native arrow).

**D7 — Auto-zones stay post-release.** Automatic zone layout writes real icon
positions (trust + restore surface); with D1 restoring per-icon pop-out it is no
longer load-bearing for findability. (Panel 4:0 after UX flipped.)

## Consequences

- The default demos AND lives well: colour pop-out returns to native level while
  the container keeps the tidy premium read; the white-board look survives as an
  explicit choice for the serenity minority.
- The renderer needs one new analysis primitive and a de-dup pass, both cached
  per source; no new dependencies; hot-path budget unchanged.
- The strict plate gates (`detectFlatPlate` three-gate) remain for the 原彩保真
  lane where 1:1 plate reproduction is the point; the default no longer routes
  its failures to white.
- Presets/history entries created before the rework are pre-release artifacts;
  no migration owed (no released users).
