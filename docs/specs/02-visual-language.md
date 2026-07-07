# DeskMakeover Visual Language (v1.0 · prototype-derived)

**Source of truth:** `docs/references/prototype/桌面美颜 v2.dc.html` (ADR-0008).
This spec transcribes the prototype into durable design law. On any conflict,
the prototype wins — open it in a browser and compare.

## Personality

**克制 · 温润 · 精密 · 有光 · 可信** — restrained, warm, precise, luminous,
trustworthy. A polished pebble, not a neon sign.

Governing rules:

1. **The app wears its own curvature.** The app logo and icon previews use the
   true superellipse family. UI surfaces use the prototype's soft radii (below);
   no default WPF control chrome anywhere.
2. **Saturation is an event.** Warm coral `#FF6F5E` is the only accent; it marks
   the primary action, selection, and the moment of transformation. Selected
   chips use a *soft* coral wash (17% mix), reserving solid coral for the CTA.
   **Blue/violet gradients are permanently banned** (owner rule — reads as AI slop).
3. **Light, not lines.** Separation comes from surface elevation and soft shadow;
   hairlines are `rgba(255,255,255,.07)` dark / `rgba(0,0,0,.10)` light.
4. **Status is never colour-only** — always glyph/text + colour.

## Colour Tokens (prototype CSS variables, verbatim)

| Token | Dark (default) | Light (`light-vars`) |
|---|---|---|
| `--bg` window base | `#1A1A1C` | `#FBFBFA` |
| `--raised` raised card | `#242427` | `#FFFFFF` |
| `--raisedHov` hover | `#2B2B2F` | `#F1F0ED` |
| `--chip` chip/control base | `#242427` | `#F4F3EF` |
| `--t1` primary text | `#F4F4F2` | `#1A1A19` |
| `--t2` secondary text | `#A8A7A1` | `#57534E` |
| `--t3` tertiary/status | `#6E6D68` | `#8A877F` |
| `--hair` hairline | `rgba(255,255,255,.07)` | `rgba(0,0,0,.10)` |
| `--accent` | `#FF6F5E` | `#FF6F5E` |
| `--accentInk` accent-as-text | `var(--accent)` | `color-mix(accent 70%, #40140C)` — darkened for contrast on light |
| Success/teal | `#3FB6A8`, bg `rgba(63,182,168,.14)` | same |
| Attention/amber | `#E5A84B`, bg `rgba(229,168,75,.16)` | same |
| CTA text on coral | `#FFF7F3` | same |

Derived accent usages (all via colour-mix, no new hexes):
selected chip bg = accent 17% mix; selected preset card = accent 15% mix into
chip; rail active = accent 16% mix; author avatar seat = accent 16% mix.

Outside the window: page/backdrop `#0D0D0F` with a top radial wash — irrelevant
to the shipped app (the OS is our backdrop).

## Typography

Font chain: `Segoe UI Variable Text` → `Segoe UI` → `Microsoft YaHei UI` →
`PingFang SC` → system-ui. `UseLayoutRounding` on; counts use tabular numerals.

| Role | Size / weight | Colour |
|---|---|---|
| Hero title (你的桌面，即将焕然一新) | 19 / 600, line-height 1.35 | t1 |
| Section label (风格 / 自定义) | 12 / 600, letter-spacing .5 | t2 |
| Row label / body | 12.5 / 400 | t2 (label) · t1 (value) |
| Chip text | 12 / 400 (selected 600) | t2 → accentInk when selected |
| Status / caption | 11.5 / 400 | t3 |
| Fine print / hints | 10.5–11 / 400 | t3 |
| CTA | 14 / 600 | #FFF7F3 |
| Title-bar app name | 13 / 600 | t1 |
| Tile label | 11 / 400, `text-shadow 0 1px 3px rgba(0,0,0,.85)` | `#F2F2F0` on wallpaper |

## Geometry & Metrics (prototype-exact)

| Element | Metric |
|---|---|
| Window | regular ≈1340×840 design size · **compact breakpoint** at ~1100px width (below → overlay panel mode); min ~1024×700 |
| Title bar | height 46; logo 24 (apple-squircle clip, coral, ✦ glyph); version chip 10.5px text, radius 7; caption buttons 36×30 |
| Control panel | width 300, padding 6/16/18, section gap 18 |
| CTA button | height 44, radius 12 (compact toolbar variant: height 34, radius 10) |
| Link chips (还原/上一版/历史/对比图) | padding 6×11, radius 9, font 12 |
| Preset cards | 2-column grid, gap 6, radius 11, padding 7×9; two 18px mini icon previews + name 12/600 |
| Accordion rows (外形/配色/快捷方式标识/图标大小) | height 42, chevron ▼ rotates 180°, summary value right-aligned t1; hairline `border-top` between rows |
| Choice chips | padding 6×10, radius 9; shape chips carry a 14px live clip swatch; colour chips a 9px dot |
| Mark-style chips | 22px live mark preview + label, padding 5×10, radius 9 |
| Swatches | mono 20px ⌀, mark 18px ⌀; selection = 2px bg-ring + 3.5px colour ring |
| 调色盘 popup | width 244, radius 14, SV field height 122 radius 10, hue bar height 14, hex input mono 11.5, eyedropper ⌖ 28×26 |
| Toggle switch | 32×19, knob 15, radius 10; on = accent, off = `rgba(128,128,128,.35)` |
| Canvas | radius 14, inset ring `rgba(255,255,255,.06)`, real wallpaper fill |
| Icon tiles | icon S = 小52 / 中64 / 大76; box = 1.08S × 1.10S; cell width box+18; grid = **column-major flow** (Windows order), gap 2×4, hover cell wash `rgba(255,255,255,.08)` radius 10 |
| Compare pill | bottom-center 62 above edge, pill radius 999, blur backdrop; held state = coral 30% mix bg + coral 55% border |
| Taskbar (decorative mirror chrome) | height 49, `rgba(30,30,34,.72)` + blur, start/search glyphs, ~5 generic app chips 24px, live clock right |
| Settings drawer | width 320, right slide-in, scrim `rgba(0,0,0,.35)` |
| Overflow menu | width 172, radius 12, item padding 7×10 |
| Icon context menu | width 188, radius 12; 6 swatches 18px |
| About dialog | width 380, radius 18, centered, scrim `rgba(0,0,0,.45)`; logo 56 |
| Toast | bottom-center, radius 11, `rgba(22,22,26,.88)` + blur, 12.5px, auto-dismiss ≈2.6s |
| History card | raised, radius 14; rows: time 11 t3 tabular · label 12 t1 ellipsis · 当前 teal pill · 回到此版 accent link |

## Shape System (icon geometry)

One `clipFor(shape, size)` service, cached; identical math in preview (WPF
geometry) and renderer (raster mask):

- **苹果**: quintic Lamé superellipse `|x|⁵+|y|⁵=1` — continuous curvature,
  apparent corner ≈22.37% of width, visually flat sides. 96-point polygon is
  sufficient fidelity.
- **纯圆**: exact circle. Already-round source icons are left untouched
  (`IsRoundish`, ADR-0005).
- **三星**: the official One UI adaptive-icon mask path, scaled:
  `M50,0 C10,0 0,10 0,50 C0,90 10,100 50,100 C90,100 100,90 100,50 C100,10 90,0 50,0`.
- **扩展形状** (ADR-0010): Google, Brave, Bookmark, Lemon, Squircle, Tile,
  Teardrop, Blob, Rectellipse. These are maskable-icon preview shapes inspired by
  Progressier's public editor. They are secondary choices behind the three
  platform defaults, but they must use the same `clipFor` service and therefore
  preserve preview==bake. Algorithms are deterministic local geometry:
  - Google = rounded square at 20% radius.
  - Brave = octagonal/shield-like adaptive mask with clipped diagonal shoulders.
  - Bookmark = rounded top with a centered bottom notch.
  - Lemon = two opposing round lobes with pointed diagonal ends.
  - Squircle = Lamé superellipse (`n≈4.5`) distinct from the true Apple curve.
  - Tile = small-radius rounded square.
  - Teardrop = one round lobe tapering to a bottom-right point.
  - Blob = soft asymmetric organic polygon.
  - Rectellipse = rectangle/ellipse hybrid with long straight mid-sides.

The app logo always wears the 苹果 clip (title bar 24px, drawer 26px, about 56px).

## Colour Treatments (配色 · `styledFor` math, prototype-exact)

Luminance `l = (0.299R+0.587G+0.114B)/255` of the icon's dominant colour.

- **原彩**: keep the icon's own colour. Ink = dark `rgba(22,22,24,.85)` when
  `l > 0.66`, else light `rgba(255,255,255,.94)`.
- **黑白**: grey `v = 255·clamp(0.5+(l−0.5)·1.4, 0.08, 0.94)` (contrast-stretched,
  never pure 0/255 walls); ink dark `#2A2A2E` when `v > 168`, else `rgba(255,255,255,.92)`.
- **单色 (tint)**: take tint's H,S; per-icon lightness `L = 26+46·l` (%);
  fill `hsl(H, S·0.85, L)`; ink `#26262A` when `L > 56` else light.
- **Document-kind items** (word/excel/pdf/png…) render as a light plate +
  coloured glyph: 原彩 `#F7F7F4`/own colour · 黑白 `#EFEFED`/`#3B3B3F` ·
  单色 plate `hsl(H, S·0.5, 90%)` / glyph `hsl(H, S·0.9, 30%)`.
- Edge cases 纯黑/纯白 icons must stay legible in all three treatments (the
  prototype ships both as test tiles — keep them in the render test set).

单色 swatch row: 纯白 `#FFFFFF` · 纯黑 `#141414` · 壁纸主色 · 壁纸辅色 (both
auto-extracted) · 品牌珊瑚 accent · 湖水 `#3FB6A8` · 琥珀 `#D9A94E` · 调色盘 button.

## Shortcut Marks (快捷方式标识 · six styles + classic arrow)

States: **美化** / **经典箭头** (classic: light plate `#F4F4F1`, dark ↗
`#2E3238`, bottom-left, size `max(14, 0.28S)`, radius 4) / **无标识**. The
launch default is **无标识**; if the user wants arrow semantics, **经典箭头** is
the recommended choice.

Mark colour: **自动** default (adaptive B/W per ADR-0006 ink law); user colour
via swatches (白/黑/珊瑚/壁纸主色/湖水) or picker — user colour is mixed per
style, never applied raw. All marks anchor **bottom-left** (except 双层卡片 /
卷角 whose geometry is corner-specific per below), ride the icon's own alpha,
and bake into each per-icon `.ico` (ADR-0006 facts).

Style algorithms (S = icon size; `l` = tile luminance; `mc` = mark colour):

| Style | Algorithm |
|---|---|
| **双层卡片** | Same-shape sibling card behind, 0.88S, offset (+0.17S, +0.18S) → peeks bottom-right; tone adaptive neutral: dark `rgba(42,40,38,.92)` behind light tiles / light `rgba(238,234,228,.94)` behind dark (user colour → `hsl(H, S·0.7, 30%)` / `hsl(H, S·0.75, 86%)`); seam + grounding drop-shadows. |
| **幽灵叠影** | Translucent same-shape echo behind, 0.92S, offset (+0.14S, +0.155S); `rgba(24,22,20,.45)` behind light / `rgba(255,255,255,.42)` behind dark (user colour 60% alpha); background blur. |
| **缎光角** | In-shape satin sheen: linear-gradient 45° from bottom-left corner, tone 62%→30%→transparent by 46%; tone = dark `#2A241E` on light tiles / white on dark. |
| **珐琅光弧** | In-shape radial glow at (15%, 88%): `mc` mixed 78% toward `#141414` on light tiles / 82% toward white on dark, fading out by 46%. |
| **卷角** | Dog-ear at bottom-right: corner cut `c = S·{apple .26, samsung .28, circle .30}` via 315° mask; mirrored fold triangle with warm paper gradient (highlight→tone→tip-shade), rounded fold, dual drop-shadow toward the body. |
| **细描边** | 2.5px same-shape ring behind the icon (S+5 total), colour `mc`. |

`玻璃箭头` is removed from the selectable gallery by ADR-0010. Its renderer may
remain only as legacy test scaffolding until the deletion pass; it must not be a
new user's choice.

Acceptance stays ADR-0005's **3-second misread gate**; parity rule: the mark
preview chips, the canvas tiles, and the baked desktop `.ico` must render the
same math.

## Motion (prototype keyframes, verbatim)

| Name | Spec | Use |
|---|---|---|
| `bloom` | scale .88→1.05→1 + brightness/saturate flash, .6s `cubic-bezier(.34,1.4,.4,1)`, 42ms/tile stagger | apply wave |
| `settle` | scale 1.06→1, opacity .35→1, .8s ease, 24ms stagger | restore exhale |
| `shimmer` | gradient sweep, 1.3s linear ∞, clipped to the icon shape | scanning skeleton |
| `rise` | translateY 10→0 + fade, .25–.4s | cards entering (history/checklist) |
| `slide` | translateX 36→0 + fade, .22s | compact panel/page transitions |
| `pop` | scale .95→1 + fade, .12–.18s | menus, picker, about, toast |
| CTA press | scale .98 | active state |
| Chip/hover transitions | all .15s; toggles .2s; chevrons .2s; panel slide .22s | |
| Updating cue | tiles dim to 45% opacity + 「正在更新预览…」 pill top-right, ~420ms debounce, images swap **in place** | axis changes |

Reduced motion (system setting): degrade everything to plain crossfades — no
scale, no stagger, no sweep.

## Window Chrome & Platform

- Custom-drawn title bar per prototype (logo + name + version chip + ⚙ + ⋯ +
  caption buttons); dark titlebar attribute; **Mica stays disabled** (previous
  wash-out bug) — solid `--bg`.
- `app.manifest`: PerMonitorV2 DPI, supportedOS Win10/11, longPathAware, UTF-8,
  asInvoker (helper: requireAdministrator). Re-render previews on `DpiChanged`.
- Win10 degradation: Segoe UI fallback, standard corners, no blur-behind
  (frosted surfaces fall back to translucent fills).
- High contrast: drop the custom skin for system colours entirely.

## Accessibility

- Every interactive element: localized `AutomationProperties.Name`.
- Status changes announced via UIA live regions (e.g. 「美化完成 · 已保存还原快照」).
- Full keyboard reachability; visible focus ring in accent; Esc closes
  menu/drawer/panel/about/picker (prototype behaviour).
- Hold-interactions (对比/peek) have non-hold equivalents where feasible
  (compare pill is also toggleable via keyboard press state).
