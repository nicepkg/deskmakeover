# DeskMakeover Visual Language (v3 · "Premium Flat")

**Source of truth:** this spec (ADR-0013; supersedes v2 "Quiet Material" chrome
language, ADR-0012). The engine rendering law (shape geometry, colour treatments,
mark algorithms — the WYSIWYG sections at the bottom) is unchanged law shared with
the bake. Panel evidence and owner dispositions:
`docs/reviews/2026-07-08-ui-v3-premium-flat-panel.md`.

## Personality

**扁平 · 偏白 · 苹果味 · 丰盈微动效 · 自定义拉满** — flat, white-leaning, Apple-flavored,
alive with restrained-but-plentiful micro-motion, customization maxed. The audience is
aesthetics-driven customizers: the app itself must clear the bar it sells. Reference
composition remains macOS System Settings (grouped, spacious, calm); the reference
*material* changes from "material dark" to **crisp flat white** — separation comes
from cool-neutral surface steps, hairline precision, and soft (not heavy) elevation.

Governing rules:

1. **Light-first, follow the system.** The light theme is the design first-citizen —
   its neutral ramp is authored in OKLCH as a *cool/true-white* system (never warm
   taupe). Dark is derived from the same ramp logic and ships at equal quality.
   Default theme = System; both themes are always test-gated together.
2. **Flat with intent.** Elevation exists but whispers: hairlines + one soft ambient
   shadow tier for floating surfaces (popover/menu/coach). Cards on the base surface
   separate by luminance step + hairline, not drop shadows. No pure-black voids in
   chrome: the canvas stage is a neutral matte, letterboxing included.
3. **Saturation is an event.** Coral `#FF6F5E` stays the only accent; blue/violet
   remain permanently banned. Two coral registers: `--coral` (identity, chips, small
   marks) and `--coral-ink` — a deeper, less-orange register for LARGE solid fills on
   white (the CTA) and accent-as-text on light. Selection is never wash-only: wash +
   ink text + hairline border. *Reviewed exceptions (test-gated allowlists in
   `tests/banned-colors.test.ts`): OS-authentic depictions inside the desktop mirror,
   and the ONE-shot celebration confetti (multicolour by owner decree, single file —
   persistent UI stays coral).*
4. **Type carries the premium.** Bundled dual-script system (below); hierarchy from
   size + colour + the 400/500 weight pair only.
5. **Customization is maxed, layered, never dumped.** Every engine capability is
   reachable in the UI (owner law — nothing is cut for "restraint"); the FIRST screen
   of each axis is curated (e.g. 6 shape chips), the full set lives one fold deeper
   (「更多形状」). Progressive disclosure, zero capability loss.
6. **App chrome and the simulated OS are different layers.** OS-mirror surfaces
   (taskbar strip, desktop tile labels) keep OS-faithful fonts/colours — exempt from
   brand tokens; they may use OS blues/grays. The user must never confuse app chrome
   with the mirrored desktop.
7. **Status is never colour-only** — glyph/text + colour.
8. **Motion is a feature** (owner directive 2026-07-08): plentiful Framer Motion
   micro-interactions everywhere state changes — but one family of curves, purposeful,
   and fully degraded under reduced-motion. See §Motion.

## Colour Tokens

Token roles are law; the exact OKLCH values are finalized in the build's design phase
and locked by three gates: banned-colour scan (no hue 195-290 in chrome), WCAG ≥ 4.5:1
for t1/t2 on their surfaces, and the light-first authoring rule (dark derives from
light, never the reverse).

| Token | Role | Light authoring constraint |
|---|---|---|
| `--bg` | window base | near-white, cool-neutral (OKLCH chroma ≈ 0, L ≈ 0.97+) |
| `--raised` | card | pure `#FFFFFF` |
| `--raised-hov` | hover | one visible step below raised, cool-neutral |
| `--chip` | control base | cool light gray (NO warm taupe) |
| `--t1/--t2/--t3` | text ranks | cool-neutral grays; t2 must not read brown |
| `--hair` | hairline | low-alpha neutral |
| `--coral` | accent identity | `#FF6F5E` (fixed) |
| `--coral-ink` | large-solid / text register | deeper, de-oranged mix of coral (fixed recipe at build) |
| `--cta-ink` | text on coral | warm white `#FFF7F3` |
| teal / amber | success / attention | kept from v2 |
| `--canvas-stage` | mirror stage + letterbox | neutral matte (not `#000`), same family both themes |
| `--glass` family | floating app pills on canvas | derived from raised, cool-neutral |

Derived washes stay `color-mix` recipes (no new hexes). Dark theme: same roles, ramp
re-derived (raised above bg, real luminance step), coral unchanged.

### Elevation

Two tiers only: `--elev-soft` (floating surfaces: popover, menu, coach, toast,
compact overlay) and `--elev-stage` (the single inset/ambient treatment framing the
canvas mirror). Base-surface cards use **no** shadow — hairline + luminance step.

## Typography (bundled — ADR-0013 D2)

**Families:** `Inter` (Latin, variable, subset ~100 KB) + `HarmonyOS Sans SC` (CJK,
static Regular 400 + Medium 500). *Status correction 2026-07-10: the CJK faces ship
as FULL TTFs today (~15.7 MB the pair) — the "3-5 MB subset" target was never
executed. Subsetting (≈ GB2312) is an open F8/pre-release task; the fallback-chain
design below already assumes it.* About page carries the attribution line
"Fonts: HarmonyOS Sans (Huawei) · Inter" (NOT yet rendered — F8).

```css
--font-sans: 'Inter', 'HarmonyOS Sans SC', 'Segoe UI', 'Microsoft YaHei UI',
             'PingFang SC', system-ui, sans-serif;   /* app chrome */
--font-os-mirror: 'Segoe UI', 'Microsoft YaHei UI', system-ui, sans-serif;
                  /* ONLY .mirror-tile-label + taskbar strip (OS-faithful) */
```

- Loading: local woff2, `<link rel=preload>`, `font-display: block`, metric overrides
  frozen; `main.tsx` gates the first render on `document.fonts.ready` — zero FOUT.
- Rare glyphs/emoji in user text fall through the chain to system CJK — fallback
  chain, not full-coverage delusion (subset ≈ GB2312).
- Numerals: global `font-variant-numeric: tabular-nums` (they land in Inter).
- Tracking: `.text-display/.text-section` Latin −0.01em; CJK letter-spacing always 0;
  ≤15px never tracked. CJK titles use `word-break: keep-all` (no orphan-char wraps).

**The ladder — six steps, weights {400, 500} only (700 escape for display if needed):**

| Step | Size / weight | Role | Colour |
|---|---|---|---|
| `display` | 26 / 500 | page title | t1 |
| `section` | 19 / 500 | panel hero title | t1 |
| `cardtitle` | 15 / 500 | card / group title | t1 |
| `body-strong` | 13 / 500 | emphasized value, selected item | t1 |
| `body` | 13 / 400 | row value, label, chip, menu, list | value t1 · label t2 |
| `caption` | 12 / 400 | status, hint, counts | t3 |

Mirror tile labels and baked zone titles remain out-of-chrome exceptions (engine/OS
rules). *Amendment 2026-07-09 (owner "control scale is unified app-wide"): the dense
INSPECTOR dialect is a sanctioned sub-ladder — segmented `sm` = 22px tall / 11px
label, chip buttons 11px, fine annotations down to ~10.5px — used identically on
every page. Page-scale adjustments touch the TEXT layer only (titles, labels, row
rhythm), never inflate controls. The six-step ladder governs text; the control
dialect governs controls.*

## Spacing & Geometry

8px soft grid `{4, 8, 12, 16, 24}` unchanged. Key metrics carried from v2 unless
listed: window regular ≈1340×840, compact <1100, min 1024×700 (D12). Radius: card 16 ·
control 10-12 · chip 9. **Inspector RIGHT, 280px (248px compact)** — the old left
300px panel is superseded (ADR-0013 amendment 2026-07-10). CTA 44px solid
`--coral-ink` on light.
**Segmented control is macOS-true**: inset gray track + a sliding WHITE thumb
(spring, soft shadow) carrying the selected segment; max-width 360px — never a
full-card slab. Settings pages use inset list rows (label left, control right,
hairline dividers) at macOS density — no empty slabs, no duplicated identity blocks.

## Module IA touchpoints (visual layer of ADR-0013)

- **Shape axis**: first row = 无 · 苹果 · 纯圆 · 三星 · 方块 · 水滴 (无 sits FIRST —
  the slash-circle law, see Addenda); 「更多形状」fold reveals the other 5 (书签 ·
  柠檬 · 菱形 · 花瓣 · 卵石). **11 options total.** All chip names Chinese-first.
- **Filter axis**: all five (无/光泽/玻璃/像素/贴纸) stay visible (owner D6; 光泽/Gloss
  went live 2026-07-09 — an aqua specular sweep over the upper third, engine in
  `icon-compositor/filters.ts`).
- **Wallpaper hero**: clarity-first narrative; **blank left-drag on the canvas
  CREATES a zone directly** (ADR-0013 amendment 2026-07-10 — the explicit-tool
  model was reversed; pan is middle-drag / compare-hold). Zone list never leaks
  grid units (no 7×12.5).
- **Axis summary strips** pair label:value (`外形 苹果 · 配色 原彩 · …`).
- **Ceremony**: first-apply consent sheet, per-apply completion + 「去看看桌面」 +
  the ONE-per-launch celebration confetti, restore confirm — shared components,
  both modules.
- **First-run wow**: REMOVED (2026-07-10). The auto-played 原样→美化 reveal was
  built, broke the icons twice, and was rolled back by owner order. Do not
  reintroduce without a fixed-duration always-removes-itself design + owner sign-off.

## Motion (owner directive: plentiful, premium, one family)

Principles: every state change explains itself; motion rewards touch; nothing moves
without purpose; **all of it degrades to crossfades under reduced-motion** (including
bloom/settle waves and module slides — no holes).

| Name | Spec | Use |
|---|---|---|
| `bloom` | scale .88→1.05→1 + brightness flash, .6s `cubic-bezier(.34,1.4,.4,1)`, 42ms stagger | apply wave |
| `settle` | scale 1.06→1 fade, .8s family ease (no outlier easings) | restore exhale |
| `pop` | scale .95→1 + fade .12-.18s, transform-origin at anchor | EVERY menu/popover/picker/coach/toast — no dead-cut mounts |
| `thumb-slide` | segmented white thumb spring (damping ≈ 44, no toy bounce) | segmented, toggles |
| `press` | scale .98 on active | all buttons/chips |
| `chip-select` | wash+ink fade .15s; NO weight-jump reflow (reserve bold width) | chips |
| `cta-working` | indeterminate coral shimmer sweep 1.3s ∞ + label crossfade 120ms + ✓ pop on synced | CTA |
| `restyle cue` | latency-gated: only if round-trip >200ms → 88% dim or pill, in place swaps | axis changes (kills the instant 45% dim) |
| `zone drag` | the Pixi compositor paints the TRUE material same-source same-frame with the pointer (ADR-0014 D5 — the old DOM-approximation+reconcile model is superseded); snap-pulse 80ms on cell change | wallpaper editor |
| `slide/rise` | compact overlay, cards entering — v2 specs carried | |
| module switch | **INSTANT, modules stay mounted** (visibility-hidden) — the crossfade overlapped a remounting canvas and was removed (2026-07-09); reduced → same | Ctrl+1/2/3 |

Motion tokens: shared durations/easings/staggers live in `lib/motion.ts` (per-index
delay helpers + reduced-motion wrapper). *Reality note 2026-07-10: a number of
component-local timings exist in the tree; new motion should prefer the shared
tokens, and strays get folded in opportunistically — but "never inline" is an
aspiration, not the current state.*

Gesture model (D10 as amended 2026-07-10): icons canvas drag = pan; wallpaper canvas
blank left-drag = CREATE zone (pan via middle-drag / while comparing); Ctrl+wheel
zoom at pointer; **Space = global hold-to-compare** (text inputs excluded; a focused
button activates via Enter — the old pass-through-on-button clause is dropped, see
§Accessibility). Hold-interactions keep non-hold equivalents; `?` keymap legend stays.

---

The sections below are **engine rendering law (WYSIWYG, unchanged from v1/v2)** — the
preview and the bake share this exact math. The v3 chrome renewal does not touch them.

## Shape System (icon geometry)

One canonical authoring (`icon-compositor/shapes.ts`), cached; identical math in
preview swatch, canvas tile, and bake:

- **苹果**: the TRUE iOS continuous-corner squircle — **three cubic Béziers per
  corner** (the authentic Apple control points, verified against the public
  reverse-engineering; constants like `1.528665/1.08849296/0.86840694…`), corner
  radius `0.225·S` (≈ the documented 22.37%). *Corrected 2026-07-10: this replaces
  the old "quintic Lamé superellipse" description — Apple's real shape is a cubic
  spline, not a Lamé curve, and since `3a6ec48` the chip swatch shares the same
  cubic path (`applePathD`) instead of a Lamé approximation. The C# oracle
  (`IconShapeGeometry.cs`) carries the identical constants.*
- **纯圆**: exact circle. Already-round source icons are left untouched (`IsRoundish`).
- **三星**: the official One UI adaptive-icon mask path, scaled:
  `M50,0 C10,0 0,10 0,50 C0,90 10,100 50,100 C90,100 100,90 100,50 C100,10 90,0 50,0`.
- **扩展形状** (curated 2026-07-09): 方块 Tile · 水滴 Teardrop · 书签 Bookmark ·
  柠檬 Lemon · 菱形 Diamond · 花瓣 Flower · 卵石 Pebble — a 「更多」 fold below the
  curated 无/苹果/纯圆/三星/方块/水滴 row. **11 shapes total.**
  - **Geometry engine** (owner call 2026-07-09, replaces the coarse hand-plotted
    C# polygons): Figma-style corner rounding + `cornerSmoothing` for arbitrary
    polygons, ported from `msurguy/squircle-path-kit` (MIT; itself derived from
    Figma's *Desperately seeking squircles*, verified Figma-exact to 0.01px). The
    smoothing ramp — tangent cubics flanking a shrunken arc, ξ≈0.6 — is what gives
    the iOS-class hand-feel that plain `border-radius` corners lack. Rounded-family
    proportions come from `progressier.com/maskable-icons-editor`; Flower + Pebble
    are `maskable.app`'s OEM masks (MIT, arcs → cubics, normalized to full extent).
  - **Single source of truth**: `icon-compositor/shapes.ts` is the canonical
    authoring; the chip clip-path (`lib/shape-paths.ts`) and the engine raster mask
    both derive from it (preview==bake, can never drift). The C# `IconShapeGeometry`
    now **re-ports FROM the web** (Windows batch) — the geometry oracle direction
    flipped web-authoritative (ADR-0015 amendment 2026-07-09).
  - **Content inscription**: pinched shapes (Diamond/Flower/Pebble) inscribe the
    artwork inside their largest centred square with per-shape breathing margins, so
    square plate icons never kiss the pinched edges.
  - **Culled** (ugly/redundant, owner 2026-07-09): Google, Brave, Squircle, Blob,
    Rectellipse, Hexagon.

The app logo always wears the 苹果 clip (title bar 24, coach 26, about 56).

## Default Composition — Colour Field (ADR-0016, 2026-07-10)

Governing engine rule: **uniformity ≠ flattening**. Uniformity is carried by the
CONTAINER layer (shape, grid rhythm, a shared lightness/chroma envelope,
de-arrowing) and is never bought by deleting per-icon hue variance — a default
that flattens the desktop into a single-hue or white field destroys parallel
visual search (the 2026-07-10 findability panel's unanimous diagnosis) and fails
the ADR-0016 D4 acceptance gate. iOS is the reference proof: uniform container,
maximised per-app colour.

**Iron law (owner, 2026-07-10): subject pixels are NEVER recoloured.** Every
icon keeps its own colours uniformly; separation comes from the plate and
silhouette shadows only. The original knockout lane was built, rejected by the
owner on sight (「很多 icon 根本认不出」), and deleted.

The DEFAULT look (满彩 colour field, recipe v7 — designer-seat acceptance PASS
2026-07-10 after four owner-steered rounds) composes per icon:

- **Plate** = the icon's dominant colour (memoized chroma-weighted OKLab hue
  histogram over the whole canvas — neutral plates carry no votes) set on ONE
  light line: Vivid **L 0.87, C clamp [0.09, 0.12]** (gamut splits the work:
  warm hues saturate fully, blues cap lower — accepted as natural separation);
  Quiet band L 0.91, C [0.04, 0.07].
- **Plated anchors**: sources with their own detected plate KEEP it (identity
  colour), lightness clamped into **[0.60, 0.80]**; near-neutral plates
  (C < 0.04, Office white boards) are exempt so white stays white.
- **Bare artwork**: original pixels at **~72% linear** (36/256 padding; the
  80% attempt read as chaos — owner call) over a same-hue coloured plate,
  lifted by an airy dock shadow (tone L 0.38, α 0.24, blur 4%, drop 1.5%).
- **Pale class** (solid-pixel mean L > 0.72): a **contrast-target plate**
  (L = subject mean − 0.20, clamp [0.62, 0.78], C [0.07, 0.10]) plus a 360°
  ring halo (α 0.34, blur 3.5%, no offset) — near-white line art separates
  while the field stays light.
- **Hue spread** (cross-icon, deterministic, id-cached, feeds preview AND
  bake): global min-gap relaxation guarantees **12° between distinct plates
  inside a ±18° brand cap**; identical artwork keeps identical plates (three
  .docx files SHOULD match).
- **Kind families (D2, as accepted v8.1)**: generic folders plate as ONE amber
  group (chroma capped 0.10) with a top-left tab affordance (same hue, L −0.14,
  9-20% tall — sub-threshold shallower cuts read as nothing at 48px); no-hue
  files take a blue-violet family (~250°, C 0.05-0.06, via per-family chroma
  windows below the Vivid floor); system items a TRUE neutral (C ≤ 0.015) so
  File-cold vs System-neutral actually separates. The plate-level dog-ear was
  CUT (documents carry their own fold; a second one is noise). Special library
  folders with brand artwork (music/video) keep their own plates — anchor
  fidelity beats group purity (designer ruling: never flatten an existing
  strong signal into the family). The WHITE fallback stays retired from the
  default path (原彩保真 only). The four-shape kind split (统一外形/分类外形,
  Folder→Bookmark / File→Tile / System→Circle) ships as a UI toggle, default
  OFF; the designer recommends folder-By-type by default — owner decision open.

**Honest hard limit (designer-acknowledged):** same-hue brand piles (the blue
apps) cannot separate further at the plate level without breaking a law —
rotating past the brand cap lies, lightness offsets break the one-line field,
recolouring subjects is forbidden. Beyond the 12° spread, identification is
carried by the PRESERVED subject glyphs and spatial memory, exactly as on a
real iOS home screen. Do not chase this further in the plate layer.

Preset lineup (D3): 默认 = colour field · 极简白 (the previous white board, an
explicit minority-taste preset) · 安静 (pastel envelope: fixed lightness, low
chroma, per-icon hue — replaces the single-hue wallpaper-tone mono) · 原彩保真
(native-plate faithful). Candy/glass leaves any recommended slot; 玻璃 is
reworked as a rim highlight, never a full desaturating wash.

## Colour Treatments (配色 — two orthogonal axes)

Since 2026-07-09 the Colour row is the **foreground/subject axis**; a separate
**background/plate colour** rides the same row's colour entry. Two axes, never a
single tint pick (chief-UI/UX + owner). Exact channel math lives in
`icon-compositor/color.ts` (OKLab ramp) — this is the structural contract.
*ADR-0016 adds 满彩 (Field) as a fourth foreground mode and the desktop default;
the modes below are unchanged as user choices.*

- **原彩 (Original)**: keep the icon's own colour. White plates take the Auto or a
  chosen background colour.
- **黑白 (BlackWhite)**: perceptual grayscale (luminance-preserving desaturation).
  Its swatch is the concentric black-in-white pair. Background override inert (v2).
- **单色 (Mono)**: the subject maps to the tint's hue. Two depths (`monoStyle`):
  - **渐变 (Tonal)** — the classic single-hue tonal ramp (light end 0.965/0.22:
    white plates read near-white with a whisper of tint).
  - **纯色 (Flat) = 极致单色** — the SEGMENTED subject in ONE flat colour on ONE
    flat plate, hard two-tone contrast, no gradient. Subject/background split =
    `icon-compositor/segment.ts`: transparent-edge silhouette · border-flood that
    follows gradient backdrops · a plate-split (distance-from-field Otsu + line-art
    polarity / coherence / fragmentation guards) for opaque plates; degenerate cases
    fall back to the whole silhouette. Mono composes LAYERED: plate colour raw,
    subject per depth.
- **背景色 (plateColor)**: applies in Original + Mono (BW inert until v2); `null` =
  Auto (Original: detected bg / white; Mono: the ramp's light end).

Colour entry: the row-end wheel opens a **前景 / 背景 dual-tab** popover — 前景 picks
the subject colour (selecting flips to 单色); 背景 = Auto + swatches + full picker.
Mono reveals a 渐变/纯色 depth segmented below the row. The standalone plate ring was
removed (owner: it read inverted — appeared only when NO colour was chosen).

Swatch grammar: **concentric pair dots** — outer disc = the plate the pick produces
(Auto = ramp light end), inner dot = the subject tint. Row: 无(原彩) · 黑白 pair ·
壁纸主色 · 壁纸辅色 · 品牌珊瑚 · 湖水 `#3FB6A8` · 琥珀 `#D9A94E` · 调色盘 wheel.
Continuous drags (前景/背景 picker + hue strip + mark-colour wheel) are throttled
(leading+trailing, 140ms) so 100+ tile recomputes never pile up.

## Shortcut Marks (快捷方式标识 · six styles + classic arrow)

States: **美化** / **经典箭头** (light plate `#F4F4F1`, dark ↗ `#2E3238`, bottom-left,
size `max(14, 0.28S)`, radius 4) / **无标识** (launch default). Mark colour: **自动**
(adaptive B/W per ADR-0006) or user colour (白/黑/珊瑚/壁纸主色/湖水/picker), mixed
per style, never raw — EXCEPT 投影, which is always neutral (the colour wheel hides
for it). **Silhouette-aware** (owner call 2026-07-09): on 原始外形 (free-form / 异形)
icons a mark uses the icon's REAL alpha silhouette (`stampMask` / chamfer
`outsideDistance`), not a phantom box or Apple substitute — the mark follows the
actual outline whatever the form.

| Style | Algorithm |
|---|---|
| **投影** (was 双层卡片) | a neutral DROP SHADOW: the icon's own silhouette offset down-right, blurred, blackish-translucent (`rgba(8,10,14)`×0.44). Mark colour deliberately inert (shadows are neutral by law). |
| **光环** (was 幽灵叠影) | a floating OUTLINE tracing the silhouette at a small gap (chamfer outside-distance band ~4% out) — clearly distinct from 投影's solid offset, and it hugs free-form contours. |
| **缎光角** | in-shape 45° satin gradient from bottom-left, 62%→30%→transparent by 46%. |
| **珐琅光弧** | in-shape radial glow at (15%,88%), `mc` mixed 78%→`#141414` (light) / 82%→white (dark), fade by 46%. |
| **卷角** | dog-ear bottom-right, corner cut `c=S·{apple .26,samsung .28,circle .30}`; mirrored fold, warm paper gradient, dual shadow. |
| **细描边** | same-shape ring behind; on free-form a snug band around the icon's silhouette; colour `mc`. |

`玻璃箭头` removed from the gallery (ADR-0010); renderer stays only as legacy test
scaffolding. Acceptance: ADR-0005 3-second misread gate; parity — mark chips, canvas
tiles, and baked `.ico` render the same math.

## Window Chrome & Platform

- Frameless WebView2 shell (ADR-0011): custom title bar (logo + name + caption
  buttons; **no version chip**, no ⚙/⋯); titlebar follows theme; Mica disabled
  (solid `--bg`).
- `app.manifest`: PerMonitorV2 DPI, Win10/11, longPathAware, UTF-8, asInvoker
  (helper requireAdministrator). Re-render previews on `DpiChanged`.
- Win10 degradation: bundled fonts remove the Segoe-Variable gap; standard corners;
  frosted surfaces fall back to translucent fills where blur-behind is unavailable.
- High contrast: drop the custom skin for system colours.

## Accessibility

- Every interactive element: localized accessible name; status via live regions.
- Full keyboard reachability; visible coral focus ring; Esc closes
  menu/overlay/coach/picker. **Space is the global compare gesture** (owner decision
  2026-07-10, reversing the old Space-activates-button clause): this UI is
  button-dense and a just-clicked control keeps focus, so letting a focused button
  eat Space would break compare exactly when it is reached for. Buttons activate
  via ENTER; only text inputs own Space (it is a character there).
- Hold-interactions (对比/peek) keep non-hold equivalents; `?` keymap legend.
- Reduced motion: complete coverage — waves, slides, thumb springs all degrade to
  crossfade; no exceptions.

## Addenda (v3.1, owner iterations 2026-07-08)

Owner-decided refinements layered onto v3 during the build; on conflict with the
sections above, these win.

- **Axis glyph keyline**: every shape/filter/mark glyph is authored on the 20/16
  grid (ink = 0.8 × canvas); no optical exceptions. *Amended 2026-07-09 (owner
  legibility call): the authored grid now RENDERS at a 25px canvas = 20px ink via
  the single `GLYPH` constant in chip-preview.tsx, which MUST stay =
  `shape-paths.SWATCH ÷ 0.8`.* Path-based `clip-path` silhouettes are authored in
  absolute pixels and `shape-paths.SWATCH` MUST equal the swatch box.
- **The 无 dialect**: one slash-circle glyph (`NoneGlyph`) for every axis's none
  option, always FIRST in its row. Dashed = auto (AutoDot); slash = none; never
  conflated. The native Windows arrow (`WinArrowGlyph`, OS-blue `#0067C0`) sits
  LAST behind the 60s penance gate; its blue never takes the accent.
- **Selection grammars**: axis rows = `SwatchPicker` (uniform 28px tiles, wrap,
  disabled roadmap slots at 40% opacity); welcome-flow choices = `ChoiceList`
  (inset rows, spring ✓, coral ink) + right-set content-width 继续.
- **Compact dropdown**: `SelectPopover compact` = 22px tall (IconAction scale);
  the option list may grow wider than the trigger so labels never truncate.
- **Colour-entry face**: every palette entry wears `WheelRing` (conic ring,
  centre dot = current pick; white = auto). 标识配色 lives in its row header's
  auxiliary slot at size 18.
- **Recompute feedback**: a 1.5px coral top light-line (`CanvasProgress`,
  400ms min-visible); full-canvas skeletons ONLY on true first load (no frame
  AND no original). Slow updates (>200ms) dim the canvas to 88%, never block it.
- **Copy law**: user-facing strings never contain dashes (owner decree; AI-text
  tell). Sentences split with 。/；/： instead.
- **Ceremonies added**: ArrowGateSheet (native-arrow penance) and the welcome
  gate (language → brand → two-question survey → roast/bluff/typed confession).
  Both reduced-motion complete; cancel is always instant.
