# DeskMakeover Visual Language (v2 · "Quiet Material")

**Source of truth:** this spec (ADR-0012). The earlier prototype
`docs/references/prototype/桌面美颜 v2.dc.html` is now a *historical reference*, not
a contract — where it and this spec disagree, **this spec wins**. The engine
rendering law (shape geometry, colour treatments, mark algorithms — the WYSIWYG
sections below) is unchanged from v1; only the *chrome* design language is renewed.

## Personality

**从容 · 分组 · 材质 · 克制发光 · 精确** — calm, grouped, material, restrained-glow,
precise. The reference for *composition* is **macOS System Settings**: content lives
in grouped inset cards, breathes with generous whitespace, and separates by soft
material depth rather than lines. The identity on top of that calm is the product's
warmth and a single coral accent. A polished pebble, not a neon sign; a settings
pane, not a dashboard.

Governing rules:

1. **Separation by light, not lines.** Surfaces separate through *elevation* (a soft
   shadow + a real luminance step from `--bg` to `--raised`), not hairlines.
   Hairlines survive only as *row dividers inside a grouped card* (the macOS inset
   list idiom) — never as the primary boundary between two surfaces.
2. **Grouped inset cards are the layout unit.** A section = a raised, shadowed card
   (radius ~16px) holding hairline-separated rows. This is the shape of settings,
   the control panels, and every grouped list.
3. **Saturation is an event.** Warm coral `#FF6F5E` is the only accent; it marks the
   primary action, selection, and the moment of transformation. Selection uses a
   *soft* coral wash (17% mix); solid coral is reserved for the CTA.
   **Blue/violet are permanently banned** (owner rule — reads as AI slop).
4. **A disciplined type ladder.** Five sizes, one role each; no 0.5px increments.
   Difference in emphasis comes from *weight and colour*, not from a half-pixel.
5. **App chrome and the simulated OS are different layers.** Floating canvas controls
   wear warm app tokens; the white frosted glass belongs only to the decorative
   Win11 taskbar. The user must never confuse "the app" with "the mirrored desktop".
6. **Status is never colour-only** — always glyph/text + colour.
7. **The app wears its own curvature.** The logo and icon previews use the true
   superellipse family; no default control chrome anywhere.

## Colour Tokens

Palette (dark is the default theme; `.light` flips the surface set). Coral is the
only accent in both.

| Token | Dark (default) | Light |
|---|---|---|
| `--bg` window base | `#1A1A1C` | `#F5F5F3` |
| `--raised` card | `#2A2A2E` | `#FFFFFF` |
| `--raised-hov` hover | `#323238` | `#F0EFEC` |
| `--chip` control base | `#212124` | `#ECEBE7` |
| `--t1` primary text | `#F4F4F2` | `#1A1A19` |
| `--t2` secondary text | `#A8A7A1` | `#57534E` |
| `--t3` tertiary/status | `#6E6D68` | `#8A877F` |
| `--hair` in-card divider | `rgba(255,255,255,.06)` | `rgba(0,0,0,.08)` |
| `--coral` accent | `#FF6F5E` | `#FF6F5E` |
| `--coral-ink` accent-as-text | `var(--coral)` | `color-mix(coral 70%, #40140C)` |
| `--cta-ink` text on coral | `#FFF7F3` | `#FFF7F3` |
| teal (success) | `#3FB6A8`, wash `rgba(63,182,168,.14)` | same |
| amber (attention) | `#E5A84B`, wash `rgba(229,168,75,.16)` | same |

**The `--bg → --raised` step widened** from v1's ~6% to a clearly readable lift
(dark `#1A1A1C → #2A2A2E`), so a card reads as *raised* before its shadow is even
seen — the core fix for the "flat dark dashboard" verdict.

Derived (all via `color-mix`, no new hexes): selected chip = coral 17% into chip;
selected preset card = coral 15% into chip; rail active = coral 16% into bg; icon
badge seat = coral 16% into raised.

### Elevation (new — the "light, not lines" system)

| Token | Value (dark) | Use |
|---|---|---|
| `--elev-1` | `0 1px 2px rgba(0,0,0,.28), 0 6px 20px -10px rgba(0,0,0,.5)` | every raised card |
| `--elev-2` | `0 2px 6px rgba(0,0,0,.32), 0 16px 40px -12px rgba(0,0,0,.6)` | popovers, menus, picker, coach mark |
| `--elev-cta` | `0 6px 18px rgba(255,111,94,.32)` | the coral CTA glow (identity, kept) |

Light theme uses the same offsets at lower alpha (`.10 / .16` families). Shadows are
soft and low-contrast — macOS material, not a drop-shadow box.

### Canvas stage & glass (new tokens — no more magic literals)

| Token | Dark | Use |
|---|---|---|
| `--canvas-stage` | `#0E0E10` | the mirror card fill behind wallpaper/tiles |
| `--glass` | `color-mix(in srgb, var(--raised) 78%, transparent)` | floating **app** control pills |
| `--glass-ink` | `var(--t1)` | glyphs/text on app glass |
| `--glass-ring` | `rgba(255,255,255,.10)` | app glass hairline |

The decorative Win11 taskbar keeps its own white frost (`bg-white/70` + dark glyphs)
— that is *simulated OS chrome* and is intentionally distinct from app glass.

## Typography

Font chain: `Segoe UI Variable Text` → `Segoe UI` → `Microsoft YaHei UI` →
`PingFang SC` → system-ui. Counts use tabular numerals.

**The ladder — five steps, one role each (no other sizes exist in chrome):**

| Step | Size / weight | Role | Colour |
|---|---|---|---|
| `display` | 26 / 600 | page title (settings, module page headers) | t1 |
| `section` | 19 / 600 | panel hero title | t1 |
| `card` (util `text-cardtitle`) | 15 / 600 | card / group title | t1 |
| `body` | 13 / 400–500 | row value, control label, chip, menu item, list | t1 value · t2 label |
| `caption` | 11 / 400 | status, hint, fine print, tabular counts | t3 |

Emphasis between `body`-label and `body`-value is **weight + colour**, never a size
delta. The v1 clutter (9.5 / 10.5 / 11.5 / 12.5 and every 0.5px value) is deleted.
Two exceptions live outside chrome and keep their own rules: the **tile label** on
the mirror (11 / 400, `text-shadow 0 1px 3px rgba(0,0,0,.85)`, `#F2F2F0`) and baked
**zone titles** (handwritten font, sized by cell — engine, WYSIWYG).

## Spacing

An **8px soft grid**: gaps come from `{4, 8, 12, 16, 24}`. 2px survives only as a
deliberate optical nudge (e.g., a chip's internal icon gap). Section gap in a panel
= 16; card inner padding = 16–20; row rhythm inside a grouped card = 12.

## Geometry & Metrics

| Element | Metric |
|---|---|
| Window | regular ≈1340×840 · **compact breakpoint** ~1100px (below → overlay panel) · min ~1024×700 |
| Radius scale | card/container **16** (`rounded-2xl`) · control/pill/menu **10–12** · chip **9** · swatch/dot round |
| Title bar | height 46; logo 24 (apple-squircle clip, coral); caption buttons 46×46. **No version chip** (pre-release). Optional `?` keymap affordance. |
| Module rail | width 66; item 40×40 tile (radius 13, 16px glyph) + 11px label; selected = coral 16% wash + accent glyph/label; 设置 pinned bottom |
| Control panel | width 300; padding 16; section gap 16 |
| Grouped card | raised + `--elev-1`, radius 16, inner padding 16–20; rows separated by `--hair`, row rhythm 12 |
| CTA button | height 44, radius 12, `--elev-cta` glow on coral (compact toolbar variant: 34 / radius 10) |
| Link chips (还原/上一版/历史/对比图) | padding 6×11, radius 9, body/13 |
| Choice chips | padding 6×10, radius 9; **shape chips carry a 14px live clip swatch; colour chips a 10px dot; mark chips a 22px live mark render** |
| Preset cards | 2-col grid, gap 8, radius 12, padding 8×10; two 18px mini previews + name 13/600 |
| Accordion rows | height 44, chevron rotates 180°, summary value right-aligned t1; `--hair` top-border between rows |
| Swatches | mono 20⌀, mark 18⌀; selection = 2px bg-ring + 3px coral ring |
| 调色盘 popup | width 244, radius 14, `--elev-2`; SV field 122 (radius 10), hue bar 14, hex mono 11.5, eyedropper 28×26 |
| Toggle switch | 32×19, knob 15, radius 10; on = coral, off = `rgba(128,128,128,.35)` |
| Canvas mirror | radius 16, `--canvas-stage` fill, inset ring `--glass-ring`, real wallpaper |
| App glass pills | `--glass` + `--glass-ink` + `--glass-ring`, blur backdrop, radius 999/12 |
| Compare pill | bottom-center; idle = app glass; held = coral 85% + white ink |
| Taskbar (decorative) | height per real taskbar; white frost + blur, generic app chips, live clock — *simulated OS, not app chrome* |
| Icon context menu | width 188, radius 12, `--elev-2`; 6 swatches 18px; also reachable via a hover ⋯ affordance |
| Coach mark / dialog | radius 18, `--elev-2`, scrim `rgba(0,0,0,.40)`; centered |
| Toast | bottom-center, radius 12, `--elev-2` + blur, body/13, auto-dismiss ≈2.6s |

## Motion

| Name | Spec | Use |
|---|---|---|
| `bloom` | scale .88→1.05→1 + brightness/saturate flash, .6s `cubic-bezier(.34,1.4,.4,1)`, 42ms/tile stagger | apply wave |
| `settle` | scale 1.06→1, opacity .35→1, .8s ease, 24ms stagger | restore exhale |
| `shimmer` | gradient sweep, 1.3s linear ∞, clipped to the icon shape | scanning skeleton (both canvases) |
| `snap-pulse` | scale 1.02→1.0, 80ms ease-out | zone snaps to a new cell (create/move/resize) |
| `rise` | translateY 10→0 + fade, .25–.4s | cards entering (history / coach) |
| `slide` | translateX 36→0 + fade, .22s | compact panel / page transitions |
| `pop` | scale .95→1 + fade, .12–.18s | menus, picker, coach mark, toast |
| CTA press | scale .98 | active state |
| module switch | crossfade + slide, enter/exit **overlap** (no empty frame); loaded modules do not refetch on re-entry | Ctrl+1/2/3 |
| Chip/hover/toggle/chevron | .15s (toggles/chevrons .2s) | |
| Updating cue | tiles dim to 45% + 「正在更新预览…」 pill, ~420ms debounce, images swap in place | axis changes |

Reduced motion (system setting): degrade everything to plain crossfades — no scale,
no stagger, no sweep, no snap pulse.

---

The sections below are **engine rendering law (WYSIWYG, unchanged from v1)** — the
preview and the bake share this exact math. Design-language renewal above does not
touch them.

## Shape System (icon geometry)

One `clipFor(shape, size)` service, cached; identical math in preview and renderer:

- **苹果**: quintic Lamé superellipse `|x|⁵+|y|⁵=1` — continuous curvature, apparent
  corner ≈22.37% of width. 96-point polygon.
- **纯圆**: exact circle. Already-round source icons are left untouched (`IsRoundish`).
- **三星**: the official One UI adaptive-icon mask path, scaled:
  `M50,0 C10,0 0,10 0,50 C0,90 10,100 50,100 C90,100 100,90 100,50 C100,10 90,0 50,0`.
- **扩展形状** (ADR-0010): Google, Brave, Bookmark, Lemon, Squircle, Tile, Teardrop,
  Blob, Rectellipse — maskable-icon preview shapes; secondary behind the three
  platform defaults; same `clipFor` service (preview==bake). Deterministic local
  geometry: Google = 20%-radius square · Brave = shield/octagon · Bookmark = rounded
  top + bottom notch · Lemon = two opposing lobes · Squircle = Lamé `n≈4.5` · Tile =
  small-radius square · Teardrop = one lobe → bottom-right point · Blob = soft
  asymmetric polygon · Rectellipse = rect/ellipse hybrid.

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
  buttons; **no version chip**, no ⚙/⋯); dark titlebar; Mica disabled (solid `--bg`).
- `app.manifest`: PerMonitorV2 DPI, Win10/11, longPathAware, UTF-8, asInvoker
  (helper requireAdministrator). Re-render previews on `DpiChanged`.
- Win10 degradation: Segoe UI fallback, standard corners; frosted surfaces fall back
  to translucent fills where blur-behind is unavailable.
- High contrast: drop the custom skin for system colours.

## Accessibility

- Every interactive element: localized accessible name; status via live regions.
- Full keyboard reachability; visible coral focus ring; Esc closes
  menu/overlay/coach/picker.
- Hold-interactions (对比/peek) have non-hold equivalents (the compare pill toggles
  via keyboard press state); a `?` keymap legend surfaces the gesture set.
