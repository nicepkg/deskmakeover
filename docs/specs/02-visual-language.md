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
   ink text + hairline border.
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

**Families:** `Inter` (Latin, variable, subset) + `HarmonyOS Sans SC` (CJK, static
Regular 400 + Medium 500 subsets). Total budget ≈ 3-5 MB. About page carries the
attribution line "Fonts: HarmonyOS Sans (Huawei) · Inter".

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

No other chrome sizes exist (the 9.5/12.5px strays are deleted). Mirror tile labels
and baked zone titles remain out-of-chrome exceptions (engine/OS rules).

## Spacing & Geometry

8px soft grid `{4, 8, 12, 16, 24}` unchanged. Key metrics carried from v2 unless
listed: window regular ≈1340×840, compact <1100, min 1024×700 (D12). Radius: card 16 ·
control 10-12 · chip 9. Panel 300px. CTA 44px solid `--coral-ink` on light.
**Segmented control is macOS-true**: inset gray track + a sliding WHITE thumb
(spring, soft shadow) carrying the selected segment; max-width 360px — never a
full-card slab. Settings pages use inset list rows (label left, control right,
hairline dividers) at macOS density — no empty slabs, no duplicated identity blocks.

## Module IA touchpoints (visual layer of ADR-0013)

- **Shape axis**: first row = 苹果 · 纯圆 · 三星 · 方块 · 水滴 · 无; 「更多形状」fold
  reveals the other 7. All chip names Chinese-first.
- **Filter axis**: all four (无/玻璃/像素/贴纸) stay visible (owner D6).
- **Wallpaper hero**: clarity-first narrative; zone editing enters via explicit tool
  state (crosshair mode / Alt+drag). Zone list never leaks grid units (no 7×12.5).
- **Axis summary strips** pair label:value (`外形 苹果 · 配色 原彩 · …`).
- **Ceremony**: first-apply consent sheet, per-apply completion + 「去看看桌面」,
  restore confirm — shared components, both modules.
- **First-run wow**: after first scan the mirror auto-plays one skippable
  原样→美化 wave (preview only).

## Motion (owner directive: plentiful, premium, one family)

Principles: every state change explains itself; motion rewards touch; nothing moves
without purpose; **all of it degrades to crossfades under reduced-motion** (including
bloom/settle waves and module slides — no holes).

| Name | Spec | Use |
|---|---|---|
| `bloom` | scale .88→1.05→1 + brightness flash, .6s `cubic-bezier(.34,1.4,.4,1)`, 42ms stagger | apply wave + first-run wow |
| `settle` | scale 1.06→1 fade, .8s family ease (no outlier easings) | restore exhale |
| `pop` | scale .95→1 + fade .12-.18s, transform-origin at anchor | EVERY menu/popover/picker/coach/toast — no dead-cut mounts |
| `thumb-slide` | segmented white thumb spring (damping ≈ 44, no toy bounce) | segmented, toggles |
| `press` | scale .98 on active | all buttons/chips |
| `chip-select` | wash+ink fade .15s; NO weight-jump reflow (reserve bold width) | chips |
| `cta-working` | indeterminate coral shimmer sweep 1.3s ∞ + label crossfade 120ms + ✓ pop on synced | CTA |
| `restyle cue` | latency-gated: only if round-trip >200ms → 88% dim or pill, in place swaps | axis changes (kills the instant 45% dim) |
| `zone drag` | DOM approximate fill (frost/tint + title wash) tracks pointer 1:1; true frame reconciles ≤150ms after release; snap-pulse 80ms on cell change | wallpaper editor |
| `slide/rise` | compact overlay, cards entering — v2 specs carried | |
| module switch | crossfade+slide overlap, no blank frame; reduced → opacity only | Ctrl+1/2/3 |

Single source: every duration/easing/stagger lives in `lib/motion.ts` (helpers for
per-index delays + a reduced-motion wrapper); consumers never inline curves.

Gesture model (D10): drag = pan on both canvases; Ctrl+wheel zoom at pointer; Space =
hold-to-compare ONLY (passes through when focus sits on a button); zone creation via
tool state. Hold-interactions keep non-hold equivalents; `?` keymap legend stays.

---

The sections below are **engine rendering law (WYSIWYG, unchanged from v1/v2)** — the
preview and the bake share this exact math. The v3 chrome renewal does not touch them.

## Shape System (icon geometry)

One `clipFor(shape, size)` service, cached; identical math in preview and renderer:

- **苹果**: quintic Lamé superellipse `|x|⁵+|y|⁵=1` — continuous curvature, apparent
  corner ≈22.37% of width. 96-point polygon.
- **纯圆**: exact circle. Already-round source icons are left untouched (`IsRoundish`).
- **三星**: the official One UI adaptive-icon mask path, scaled:
  `M50,0 C10,0 0,10 0,50 C0,90 10,100 50,100 C90,100 100,90 100,50 C100,10 90,0 50,0`.
- **扩展形状** (ADR-0010): Google, Brave, Bookmark, Lemon, Squircle, Tile, Teardrop,
  Blob, Rectellipse — maskable-icon preview shapes; same `clipFor` service
  (preview==bake). Deterministic local geometry: Google = 20%-radius square · Brave =
  shield/octagon · Bookmark = rounded top + bottom notch · Lemon = two opposing lobes ·
  Squircle = Lamé `n≈4.5` · Tile = small-radius square · Teardrop = one lobe →
  bottom-right point · Blob = soft asymmetric polygon · Rectellipse = rect/ellipse
  hybrid.

The app logo always wears the 苹果 clip (title bar 24, coach 26, about 56).

## Colour Treatments (配色 · `styledFor` math)

Luminance `l = (0.299R+0.587G+0.114B)/255` of the icon's dominant colour.

- **原彩**: keep own colour. Ink = dark `rgba(22,22,24,.85)` when `l > 0.66`, else
  light `rgba(255,255,255,.94)`.
- **黑白**: grey `v = 255·clamp(0.5+(l−0.5)·1.4, 0.08, 0.94)`; ink dark `#2A2A2E`
  when `v > 168`, else `rgba(255,255,255,.92)`.
- **单色 (tint)**: take tint's H,S; per-icon `L = 26+46·l` (%); fill
  `hsl(H, S·0.85, L)`; ink `#26262A` when `L > 56` else light.
- **Document-kind items**: light plate + coloured glyph — 原彩 `#F7F7F4`/own · 黑白
  `#EFEFED`/`#3B3B3F` · 单色 plate `hsl(H, S·0.5, 90%)` / glyph `hsl(H, S·0.9, 30%)`.
- Edge cases 纯黑/纯白 stay legible in all three treatments (keep both as test tiles).

单色 swatch row: 纯白 `#FFFFFF` · 纯黑 `#141414` · 壁纸主色 · 壁纸辅色 · 品牌珊瑚 ·
湖水 `#3FB6A8` · 琥珀 `#D9A94E` · 调色盘 button.

## Shortcut Marks (快捷方式标识 · six styles + classic arrow)

States: **美化** / **经典箭头** (light plate `#F4F4F1`, dark ↗ `#2E3238`, bottom-left,
size `max(14, 0.28S)`, radius 4) / **无标识** (launch default). Mark colour: **自动**
(adaptive B/W per ADR-0006) or user colour (白/黑/珊瑚/壁纸主色/湖水/picker), mixed
per style, never raw. Marks anchor bottom-left (双层卡片/卷角 corner-specific), ride
the icon's alpha, bake into each per-icon `.ico`.

| Style | Algorithm |
|---|---|
| **双层卡片** | same-shape sibling 0.88S behind, offset (+0.17S,+0.18S); adaptive neutral tone (user colour → `hsl(H,S·0.7,30%)`/`hsl(H,S·0.75,86%)`); seam + grounding shadows. |
| **幽灵叠影** | translucent same-shape echo 0.92S behind, offset (+0.14S,+0.155S); `rgba(24,22,20,.45)`/`rgba(255,255,255,.42)` (user colour 60% α); bg blur. |
| **缎光角** | in-shape 45° satin gradient from bottom-left, 62%→30%→transparent by 46%. |
| **珐琅光弧** | in-shape radial glow at (15%,88%), `mc` mixed 78%→`#141414` (light) / 82%→white (dark), fade by 46%. |
| **卷角** | dog-ear bottom-right, corner cut `c=S·{apple .26,samsung .28,circle .30}`; mirrored fold, warm paper gradient, dual shadow. |
| **细描边** | 2.5px same-shape ring behind (S+5), colour `mc`. |

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
  menu/overlay/coach/picker; Space activates the focused control (compare never
  hijacks button focus — D10).
- Hold-interactions (对比/peek) keep non-hold equivalents; `?` keymap legend.
- Reduced motion: complete coverage — waves, slides, thumb springs all degrade to
  crossfade; no exceptions.

## Addenda (v3.1, owner iterations 2026-07-08)

Owner-decided refinements layered onto v3 during the build; on conflict with the
sections above, these win.

- **Axis glyph keyline**: every shape/filter/mark glyph draws EXACTLY 16px on a
  20px canvas; no optical exceptions. Path-based `clip-path` silhouettes are
  authored in absolute pixels and `shape-paths.SWATCH` MUST equal the swatch box.
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
