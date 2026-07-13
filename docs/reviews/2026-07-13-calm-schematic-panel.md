# 2026-07-13 — 清爽 schematic-first redesign (owner-directed, 2 seats + layout research)

**Trigger:** owner directive after the polish round — the page still read like an article;
he ordered frontend-drawn schematics marking the operation areas (no need to replicate
Windows), plus animation; words demoted. A follow-up owner correction: several highlight
regions were wrong (e.g. search-flyout news lives in the RIGHT column) and grey placeholder
squares had no recognizability — real screenshots had to be researched, and icon shapes
drawn so users see WHAT gets removed.

## Panel (PM + chief designer, isolated) — converged design

- **Clash ruled:** PM wanted ONE hero schematic + text rows; designer argued the 9 surfaces
  never coexist on a real desktop (one panorama would be a false schematic) and rows must
  LEAD with an image to honor 示意图为主. **Ruling: designer's carrying model** (per-row
  104×64 mini-screen + hero establishing shot) **with PM's discipline** (delete surface
  chips + result-sentence prose; ship-scope cuts; honesty guards).
- Unified frame language: same screen outline every row, one coral highlight per frame,
  ≤8 placeholder shapes, taskbar anchor on popup/window scenes, full-screen scenes look
  full-screen (the shape difference itself encodes interruption magnitude).
- Noise-exit apply animation = the state machine's view (verified-only removal; setAwaiting
  dims in place; guided noise leaves only on confirmedOff/userAttested; restore reverses).
  Hover row ↔ schematic pulse via pure CSS (`.group/calmrow`), reduced-motion degrades.

## Real-layout research (win11-layout-scout)

Corrections applied to every region: weather/widgets entry FAR LEFT on the taskbar; the
centered start→search→taskview cluster; search-flyout news in the RIGHT ~40% column;
notification calendar at the BOTTOM (cards on top); Settings promo inside the card grid
below the device header; Explorer banner under the address bar; widgets board slides from
the LEFT with the MSN feed below pinned cards; lock-screen status cards TOP-LEFT (22H2+);
SCOOBE full-screen takeover. Glyphs drawn as own geometry per research descriptions: flat
2×2 start squares (never the skewed flag), magnifier, two stacked task-view rects, tray ^
caret, amber weather pill.

## Owner rulings this round

- **O4 (2026-07-13): official icon assets permitted** — the owner ruled extracting official
  icons acceptable for this free open-source product. Implementation note: Microsoft's
  fluentui-system-icons are MIT (usable even commercially); brand logos (Windows flag, Edge)
  remain trademarks, so the schematics keep own-geometry evocations — same result, cleaner
  licence. This loosens ADR-0015's no-extracted-assets posture for the calm schematic layer
  only; `public/real-icons/` rules unchanged.
- Direction A ("noise map") is hereby superseded by this schematic-first system — the
  north-star line in ADR-0023 D2 is satisfied earlier and differently than planned.

## State

Shipped in `a98aba1` (+ predecessors `aac7434`/`13777d2`): schematic-map, schematic-parts
(Highlight/NoiseGroup), 9 scenes, SurfaceSchematic + HeroSchematic, noiseExit motion,
ping-soft hover, page integration, compressed copy. Gates green (tsc · bun 573 incl.
banned-colors over the SVG layer). Browser-verified: initial/applied/guided evidence
`docs/plans/evidence/2026-07-calm/04-06*.webp`.

**Open:** owner eyeballing round (he is reviewing renders directly this round); designer-seat
pixel acceptance + codex adversarial review of the schematic diff owed at the W0.6 exit;
motion fine-pass (stagger feel, hero exhale) after owner sign-off.
