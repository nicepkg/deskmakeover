# DeskMakeover Visual Language

The product sells continuous-corner beauty, so the app itself must wear it. Every
corner, surface, and motion in the UI follows this spec. With no tuning UI in the
product (ADR-0002), default visual quality carries the entire experience.

## Personality

**克制 · 温润 · 精密 · 有光 · 可信** — restrained, warm, precise, luminous,
trustworthy. A polished pebble, not a neon sign. The interface should tell the user
"someone cared", never "this is a utility".

Three governing rules:

1. **The app wears its own curvature.** Every rounded corner in the UI is a
   superellipse (continuous corner), the same family it applies to icons. No plain
   `CornerRadius` arcs on visible surfaces.
2. **Saturation is an event, not decoration.** ≥95% of the interface is neutral.
   The accent color appears only on the primary action and the "styled" state —
   the moment of transformation owns the only color.
3. **Light, not lines.** Separation comes from soft shadow and surface elevation,
   not 1px hairline borders. Hairlines, where unavoidable, are 1px at ~6% opacity.

## Theme Model

- **Dark is the default stage** (styled icons glow on dark). Light and follow-system
  are selectable in settings; the preference persists.
- Base: WPF Fluent theme (`ThemeMode` on .NET 10) supplies control primitives,
  accent plumbing, and system integration; our token dictionaries skin it.
- **Windows 10 degradation**: no Mica → solid backdrop, Segoe UI instead of
  Segoe UI Variable, standard window corners. Never emulate Win11 chrome by hand.
- **High contrast**: when `SystemParameters.HighContrast` is on, custom palettes and
  shadows are dropped entirely in favor of system colors.

## Color Tokens

Neutrals are warm (never the blue-gray admin-dashboard family).

| Token | Dark (default) | Light |
|---|---|---|
| `Surface.Base` | `#1A1A1C` warm charcoal | `#FBFBFA` ceramic |
| `Surface.Raised` | `#242427` | `#FFFFFF` |
| `Surface.RaisedHover` | `#2B2B2F` | `#F5F5F3` |
| `Text.Primary` | `#F4F4F2` | `#1A1A19` warm ink |
| `Text.Secondary` | `#A8A7A1` | `#57534E` warm gray |
| `Text.Tertiary` | `#6E6D68` | `#8A877F` |
| `Hairline` | `#FFFFFF` @ 6% | `#000000` @ 6% |
| `Accent` | `#5E5CE6` deep indigo | `#4F4DD6` |
| `Accent.Glow` (sweep gradient) | `#7C6AF2 → #4CC2FF` | same |
| `Status.Styled` | `#3FB6A8` teal | `#2E9C8F` |
| `Status.Attention` | `#E5A84B` amber | `#C98A2E` |
| `Status.Skipped` | `Text.Tertiary` | `Text.Tertiary` |

Status is never color-only: always icon/text + color (styled ✓ / skipped ⊘ /
needs-attention !).

## Geometry: the Squircle System

- One reusable `SquircleBorder` control (superellipse, n≈5, Apple family) generates
  clip and background geometry. All visible rounded surfaces use it.
- **Concentric rule**: outer radius = inner radius + gap between them.
- Radii scale: window-level blocks **20** · icon tiles **18** · buttons **12** ·
  chips/pills **8**.
- **Icon tiles are naked**: the preview grid is squircle tiles + labels directly on
  `Surface.Base` — no per-item card boxes, no borders around tiles. The tile *is*
  the item. Tile background reflects the rendering pipeline's real decision (white
  tile / preserved background / clipped), never a hardcoded white.
- Focus visuals and selection rings are squircle outlines in `Accent`.

## Typography

Font chain: `Segoe UI Variable Display` (titles) / `Segoe UI Variable Text` (body),
falling back through `Microsoft YaHei UI` for CJK, then `Segoe UI` on Win10.
`UseLayoutRounding` on; ClearType default. No letter-spacing manipulation (WPF has
no native tracking; don't fake it).

| Level | Size / Weight | Color |
|---|---|---|
| Hero title | 22 Display SemiBold | `Text.Primary` |
| Section | 15 Medium | `Text.Primary` |
| Body | 13 Regular | `Text.Secondary` |
| Caption / status | 12 Regular | `Text.Tertiary` |

Counts use tabular numerals (`Typography.NumeralAlignment="Tabular"`). Buttons:
13 Medium. Layout spacing uses a 4px base grid; grid gutter is 16.

## Elevation

- Raised surfaces: `DropShadowEffect` BlurRadius 24, ShadowDepth 2, Direction 270,
  Opacity 0.10, shadow color tinted toward the surface hue (not pure black).
- Dark mode elevates by lightening the surface (+ optional faint top inner
  highlight), not by heavier shadow.
- Default WPF button/control chrome is forbidden on the main screen; primary,
  secondary, and text button styles are custom templates over squircle geometry.
  Pressed = scale 0.98 with 120ms ease-out return; hover = surface lift;
  disabled = reduced opacity, never gray fill swap.

## Motion

Motion encodes meaning: **beautify inhales, restore exhales.**

| Moment | Behavior |
|---|---|
| **Bloom wave** (apply) | Tiles transform in a staggered ripple, top-left → bottom-right, 40–60ms stagger per tile. Each tile: crossfade old→styled + scale 0.92→1.0 with slight `BackEase` overshoot; a narrow highlight band (animated `LinearGradientBrush` offset −0.3→1.3) sweeps the freshly styled tile. Progress **is** the grid transforming; one quiet caption line elsewhere. |
| **Press-to-peek** | Press on a tile: 120ms crossfade to the original icon; release: spring back to styled with slight overshoot. Requires both images in the view model. |
| **Restore settle** | Pure ease-out, slower than bloom, zero overshoot — icons calmly settle back. Deliberately the emotional opposite of apply. |
| **Hover** | Tile lifts 2px, shadow grows, 150ms cubic ease-out. |
| **Skeleton** | While scanning: shimmering squircle placeholder tiles with staggered shimmer — never a spinner, never a frozen empty grid. |
| **Reduced motion** | If the system requests reduced motion, all of the above degrade to plain crossfades; no scale, no sweep, no stagger. |

## Screen States

- **Empty/loading state** (launch, pre-scan-complete): ghost squircle grid + one
  line 「你的桌面，即将焕然一新」 + primary action disabled until preview is ready.
- **The mirror** (default): hero before/after region above the tile grid; the hero
  states the count in human words 「可以美化 N 个图标」.
- **Applying**: bloom wave on the real grid.
- **Done**: quiet celebration, restore link, 「保存对比图」 hook. No confetti.
- Every state keeps the restore entry visible when a snapshot exists.

## Window Chrome & Platform Foundation

- `app.manifest` (App): `dpiAwareness` PerMonitorV2 (fallback PerMonitor),
  `supportedOS` Win10/Win11 GUIDs, `longPathAware`, `activeCodePage` UTF-8,
  `requestedExecutionLevel` asInvoker. Helper manifest: `requireAdministrator`.
- Re-render icon previews on `Window.DpiChanged` (multi-monitor mixed DPI).
- Win11: Mica backdrop via `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)` +
  dark titlebar via `DWMWA_USE_IMMERSIVE_DARK_MODE`, matching the active theme. A
  desktop-beautifier being tinted by the user's own wallpaper closes the loop.
- Win10: solid `Surface.Base` backdrop, system titlebar, standard corners.
- Custom titlebar is out of MVP scope; system chrome + dark titlebar attribute is
  the baseline. Revisit post-MVP.

## Accessibility

- Every interactive element has `AutomationProperties.Name` (localized).
- Status changes announced via UIA live regions ("美化完成，处理了 12 个图标").
- Full keyboard reachability; visible squircle focus ring; Enter/Space activate.
- Status semantics carried by text + glyph, not color alone.
- High contrast drops the custom skin (see Theme Model).
