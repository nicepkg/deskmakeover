# Icon Findability vs Uniformity — Expert Panel Record

Date: 2026-07-10 · Mode: product-studio `optimize` (user-feedback driven) · Seats:
Chief Designer / Chief UX / Chief UI Engineer / Chief PM (isolated, same-vendor
subagents, two adversarial rounds) · Artifact: 6 live screenshots captured from the
dev build (Apple minimal default, original hold-compare, Candy, Pure B&W, Wallpaper
tone, plus same-region detail crops) + engine sources.

Owner-relayed user evidence (screenshots of the default result shared publicly):
(1) 「视觉效果不佳」 (2) 「肉眼查找图标的效率不如以前」. Owner constraint: the
solution must stay 高效 · 整齐 · 统一 · 高审美 — native icons and the shortcut
arrow remain rejected.

## Diagnosis (all four seats converged independently — mechanism, not taste)

Visual search runs on preattentive parallel channels (colour ≫ silhouette ≫
lightness). A target unique in one channel "pops out" in O(1); when every tile
matches on all channels, search degrades to serial label-reading, O(n) at 124 icons.
The Apple-minimal default zeroes BOTH strong channels at once:

1. **Colour field → colour dot.** With `plateColor:null` the engine intends the
   detected native plate colour, but the plate gates are strict
   (`tryDetectBackground` ring tolerance 18/24; `detectFlatPlate` IoU≥0.95 / rim 0.9
   / flat 0.85) — any gradient/two-tone/badged/photo icon fails all gates and lands
   in the WHITE fallback (`compose.ts`), while `CONTENT_PADDING_FRACTION = 42/256`
   caps the glyph at ~67% linear (~45% area). Office's full-bleed green/blue/orange
   becomes three near-identical white tiles.
2. **Silhouette channel deleted.** Every kind is clipped into the same squircle
   unconditionally; folder-shape/document-corner/round-app cues vanish.
3. **Lightness variance flattened.** Equal-lightness white plates read as one bright
   wall against the wallpaper (figure-ground between tiles lost).

Same-image control: single-flat-colour sources (Spotify, Minecraft) PASS the plate
gate, keep their full-bleed colour, and stay findable after beautification — the
damage tracks exactly the whitened+shrunken subset. The problem is the default
recipe, not beautification itself.

**Core principle (4/4 seats):** *uniformity ≠ flattening*. iOS = uniform container
+ maximised per-app colour. The default copied Apple's shape and inverted Apple's
colour strategy; cohesion comes from squircle+grid+de-arrowing, not from the white
plate. Market echo (PM): Android's Material You themed-icons monochrome shipped at
billion scale and findability is its top complaint; commercially winning packs are
"uniform shape + preserved brand colour".

Secondary audience finding (UX): complaint #2 came from viewers with ZERO spatial
memory — and sharing screenshots is a core product moment, so the default cannot
lean on the owner's muscle memory.

## Round-1 findings (deduped, ⭐ = independently named by ≥2 seats)

| # | Seats | P | Finding | Fix direction |
|---|-------|---|---------|---------------|
| 1 | ⭐ all 4 | P1 | Colour channel destroyed by white-fallback + 67% glyph cap | colour-field default (below) |
| 2 | ⭐ 3 | P1 | Silhouette variance deleted by unconditional single shape | kind differentiation (below) |
| 3 | ⭐ 3 | P1 | Candy (circle+glass) kills contrast+saturation+silhouette at once; reads as disabled/loading | demote + rework glass as rim highlight |
| 4 | ⭐ 2 | P1 | All four presets sit on the "reduce hue" side; no preset preserves hue | preset lineup rework |
| 5 | ⭐ 2 | P2 | Wallpaper tone = single-hue mono = Material You trap (Office cluster → one brown blob) | per-icon-hue quiet variant |
| 6 | ⭐ 3 | P2 | Glyph ≈45% area too small; white padding amplifies the wash-out | padding 42/256 → ~0.10 |
| 7 | UI-eng | P2 | Engine has NO per-icon dominant-colour primitive (context-pack claim was wrong) | add memoized `dominantHue` (~30-40 lines) |
| 8 | UI-eng | P3 | No-hue tail (photos/near-white/generic files) breaks any hue-derived recipe | letter/source-colour badge fallback, tail-only |
| 9 | UX | P3 | De-arrowing loses the only shortcut-vs-real cue | optional ultra-light corner dot mark |
| 10 | PM | P2 | The two complaints are two evidence classes (bystander aesthetics vs liveability); conflating them fixes the wrong thing | treat findability as the hard signal |

PM's axis-reduction proposal (cut shapes/filters for v1) was NOT adopted — it
conflicts with the standing owner law "customization is maxed, nothing cut for
restraint" (spec 02); progressive disclosure already handles it.

## Round-2 adversarial cross (votes + strongest attacks)

**Clash 1 — default colour recipe: 满彩 3 : 柔彩 1.**
- (b) full-colour brand plate + knockout (Designer/UX/PM): colour pop-out strength =
  area × saturation; only a full plate restores the channel to native level.
  Attack on (c): at 32-48px desktop scale, fixed L≈0.92 + clamped chroma
  (0.035-0.145; usable ~0.03-0.05 near the sRGB gamut edge at that lightness) puts
  neighbouring-plate ΔE below JND — "an imperceptible colour halo on the same
  failed recipe" (panel-05 is the live preview of this failure).
- (c) pastel envelope (UI-eng): one code path covers ALL icons; his attack on (b)
  stands as a real design constraint: multi-hue sources (Edge/weather/photos) have
  no clean single colour to knock out, so (b) needs a fidelity gate whose second
  lane must be designed as part of ONE system or the desktop splits into
  half-knockout/half-native inconsistency.
- PM flipped from the a/b fence to (b): native-plate (a) is ill-defined for
  full-bleed/transparent sources; a full fill is always well-defined.

**Clash 2 — kind-differentiated shapes in default: 2 : 2 tie.**
Designer+UX (converted): four semantic silhouettes = biggest findability lever,
controlled variance ≠ per-icon noise. PM+UI-eng: don't spend the just-bought
container unity; colour families + in-envelope affordances (folder tab, document
dog-ear) suffice; full shape split as an opt-in toggle.

**Clash 3 — auto-zones in the default beautify: 0 : 4 (UX flipped).**
Once the default restores per-icon pop-out, zones stop being load-bearing; auto
placement writes real icon positions (trust + restore surface), stays post-release.

## Owner dispositions (2026-07-10, one batch, all as recommended)

| Q | Decision | Disposition |
|---|----------|-------------|
| Q1 | Default recipe = **(b) 满彩品牌色底**: full-bleed dominant-colour plate (OKLab harmony band), subject knockout, enlarged glyph; multi-hue sources take the fidelity lane designed as one coherent system | **accept** → ADR-0016 D1 |
| Q2 | Kind differentiation = **colour families + in-envelope light affordances** (folder tab / document dog-ear inside the same squircle); full four-shape split ships as an opt-in toggle, not default | **accept** → D2 |
| Q3 | **Preset lineup rework**: new default + 极简白 (current white board, explicit) + 安静 (pastel envelope (c), replaces single-hue wallpaper-tone) + 原彩保真 (a); Candy demoted from recommended slot and reworked (glass → rim highlight, not full wash); letter/source-colour badge fallback for the no-hue tail (tail-only) | **accept** → D3 |
| Q4 | **F8 findability gate**: default look, 20 random targets, locate time/error rate not worse than stock threshold (owner-supervised light test) + automated neighbouring-plate ΔE separability check in bun tests | **accept** → D4, spec 06 §8 |

Consensus engineering items approved with the batch: 「统一=统一容器+保留辨识」
into spec 02 engine law; glyph padding 42/256 → ~0.10 and full-bleed threshold
0.88 → 0.82; `dominantHue` primitive; white fallback → dominant-colour fallback;
global hue de-duplication pass (deterministic, id-cached); optional ultra-light
shortcut dot mark.

Evidence screenshots are ephemeral dev captures (real-brand mock icons — never
committed, D9); reproduce via the dev pack + preset clicks.
