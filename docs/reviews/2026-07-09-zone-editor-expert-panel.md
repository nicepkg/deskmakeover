# Expert panel — 壁纸分区 editor rebuild (2026-07-09)

Five isolated seats (same-vendor subagents, fresh context each, no cross-seat leakage,
no owner-preference leakage): PM · UX (Nielsen/Norman) · UI/visual · Interaction ·
Feasibility (rendering/Windows). Artifacts given: spec 04, editor/store/renderer source,
live screenshots (`tmp/zone-recon/*` in the commander workspace). Owner dispositions
pending — this file records findings; the disposition table is appended after the owner
answers the consolidated question list.

## Convergence map (⭐ = independently named by ≥2 seats)

1. ⭐⭐⭐ **Root cause of "不流畅": material lives host-side.** Frost/fill/title are
   composed in C# behind a 140ms debounce (reset every pointermove → frost is FROZEN for
   the whole drag, then teleports). Only vector chrome tracks the pointer. Named by all
   5 seats. Also: the browser mock renders zones not at all → the visual designer (owner)
   has been tuning visuals blind on Mac.
2. ⭐⭐⭐ **Title grammar is wrong, not just ugly.** Centered + handwritten + huge +
   eats a full icon row = "poster" grammar; a category label wants "label" grammar:
   top-left, small, clean sans (HarmonyOS Sans SC/Inter semibold), chip-backed for
   contrast. Handwritten default contradicts Premium Flat and the owner's own
   no-handwriting-fonts poster rule. (UI, UX, PM)
3. ⭐⭐⭐ **Defaults fail on pale wallpapers / zones indistinguishable from each other.**
   Frosted-white α.55 on a pale wallpaper is near-invisible; all zones share one
   wallpaper-derived tone so groups carry zero at-a-glance signal. Converged fix:
   ONE adaptive material (per-zone luminance sampling → light/dark tone auto) replacing
   the 4 named styles, plus per-zone accent + emoji as the categorization signal.
   (UI, UX, PM)
4. ⭐⭐ **Title band cost: one full icon row per zone (~25% of a 6×4 zone).** Fix:
   title overhangs the panel top (~0.4 cell into the gutter), falls back to a narrow
   in-panel strip when flush against screen top / stacked zones. (UI; PM flags the
   oversized typography stack feeding it)
5. ⭐⭐ **FFmpeg-baking zones into video files: REJECTED.** Copyright/ToS on Workshop
   content, re-encode cost, and frozen frost over moving video looks broken. Video =
   overlay/own-player architecture later; static v1 now with interfaces reserved.
   (Tech B1 verdict + PM direction C)
6. ⭐⭐ **The apply moment is the emotional + functional gap.** IXD: no signature
   motion (propose "分区落版" wave: staggered zone bloom + coral sweep, masks bake
   latency). UX: after apply the user is dropped — nothing bridges to actually dragging
   icons in (propose 整理模式 guided mode; PM proposes pre-apply auto-suggest from
   current icon clusters via DesktopLayoutReader).
7. ⭐⭐ **Correctness pair (no owner decision needed):** `key={i}` + no zone id breaks
   selection/rename/exit-animations on delete (add stable id + AnimatePresence);
   `composing` flips on every drag move (suppress recompose while interactionOpen).

## Architecture verdict (feasibility seat)

Recommended: **A3 — one TypeScript WebGL2 compositor (pixi.js v8)**, live at viewport res
(2–5ms/frame vs 33MP source uploaded once) + native-res bake in an OffscreenCanvas worker
→ PNG → host writes + SetWallpaper. C# renderer deleted; **source DECODE stays in C#**
(WIC: JPEG XR/HEIC/cover-crop) handing RGBA to the web once per source change. The
per-edit 33MB/133MB shared-buffer frame traffic disappears. Same shader takes a
VideoTexture later → video wallpaper becomes "swap the texture source", not a rewrite.
CSS backdrop-filter as the material is rejected (cannot be read back for bake →
reintroduces dual-renderer drift). Pivot question: if video is definitively out of the
roadmap, A4 (dirty-rect C# streaming) is cheaper and keeps the proven renderer — the
owner's video answer decides.

WYSIWYG redefinition required: from "literally one buffer" to "one renderer, two
resolutions", gated by a parity fixture (5 looks, C# vs TS output, ΔE<2 / SSIM>0.99)
during migration, then TS is the single truth.

Risk probes (cheap, ordered): parity fixture diff; `MAX_TEXTURE_SIZE` +
SwiftShader detection fallback; Canvas2D fillText vs DirectWrite CJK metrics diff;
`--disable-gpu` degradation path; (if video) 1-day WorkerW + WebView2 mp4 + frost spike.

Perf budget (8K = 7680×4320, 132.7MB RGBA): live drag frame 2–5ms (WebGL) vs
60–120ms full-frame C#; bake 8K ≈ render 10–20ms + PNG encode 0.6–1.2s (worker,
non-blocking, common to all options); peak transient memory ≈ 450–500MB.

## Interaction spec highlights (IXD seat, adopted subject to owner)

- Contract: outline + material always same-source same-frame (`look.zones[i]`), both
  track the pointer; snap stays half-cell hard snap; guides show only the lines being
  snapped to (kill the full-grid overlay); zone-edge magnetism ≤0.35 cell + cross-zone
  span lines + equal-gap ticks; overlap allowed with warn-wash (no hard blocking).
- Create rubber-band draws the MATERIAL forming (not a marquee); W×H badge; release =
  material settles in place (no teleport), auto-select + auto-rename focus.
- snap-pulse only on release commit (was: every half-cell crossing = buzz).
- Delete exit animation; visible undo/redo + "已删除 · 撤销" toast; Alt-drag duplicate.
- Apply = "分区落版" wave: coral sweep 300ms + staggered zone bloom 480ms
  `[0.34,1.4,0.4,1]`, 60ms/zone; reduced-motion: single 120ms brightness pulse.
- Perf contract: pointer→material ≤1 display frame; reconcile fades ≤120ms, never
  teleport.

## Visual system (UI seat, adopted subject to owner)

- **Adaptive Frost**: per-panel OKLCH sample (L̄,C̄,H̄); tone auto light/dark at L̄ 0.55
  (hysteresis 0.05, user-overridable). Light fill OKLCH(0.92, min(C̄×0.5,0.03), H̄)
  α 0.60 (slider 0.35–0.85); dark OKLCH(0.20,…) α 0.52 (0.30–0.80). Frost tier
  σ=cellHeight/6; blur-less tier (video/weak GPU): α+0.12 + bottom inner shadow.
  Depth: 1px top inner highlight (light α.35/dark α.14) + 1px outer contour
  (black α.10/white α.12, untinted). NO baked drop shadow. Radius default 20 (8–28).
  Outline variant (fill α≤.05 + 2px contour) forces the label chip.
- **Title**: top-left label chip (pill/soft-10), padding 10×5; chip = material one step
  denser; ink auto-inverts vs chip (OKLCH 0.25/0.97 α.96); HarmonyOS Sans SC/Inter 600,
  size clamp(cell×0.20, 15, 22)px; S/M/L = cell×0.17/0.20/0.24; handwritten demoted to
  an optional font; shadow only in no-chip mode.
- **Edit chrome**: selection = inner 1.5px coral + outer 0.5px white halo; handles =
  10×10 white-core rounded squares (r3) with coral ring + drop shadow, 4 corner handles
  under 5-cell zones; guides coral dashed + 1px white companion.
- **Ghosts**: full first row + half second row, 3–4 neutral material tones + micro top
  highlight (sell "your icons will line up here", not "skeleton").

## PM positioning

The job is legible grouping, not file management. Direction A adopted: "美的背景板" —
never compete with Fences on containment (no daemon, zero runtime cost, it IS your
wallpaper, one-click reversible). Cut fiddle surface (typography stack, corner slider
granularity), reinvest in categorization signal (per-zone accent, emoji, preset gallery
with semantic names, auto-suggest from icon clusters). Video = separate v2-class bet.

## Question list sent to the owner

See the conversation record / disposition table below (10 consolidated decisions:
architecture pivot, video path, material collapse, accent+emoji, title system, control
scope, app→desktop bridge, half-cell snap, overlap policy, interaction detail pack).

## Dispositions (owner, 2026-07-09)

| # | Decision | Disposition |
|---|----------|-------------|
| Q1 | Rendering architecture | **ACCEPT A3** — one TS WebGL2 compositor (pixi.js v8); live preview at viewport res, native-res bake in OffscreenCanvas worker → PNG → host writes + SetWallpaper. C# renderer deleted; source decode stays C# (WIC). WYSIWYG acceptance redefined: one renderer two resolutions, migration parity fixture ΔE<2 / SSIM>0.99. |
| Q2 | Video wallpaper path | **ACCEPT** — FFmpeg re-encode REJECTED (copyright/ToS, frozen frost, re-encode per edit). v1 static-only; compositor reserves source-texture provider / blur-tier / output-target / headless interfaces. Video itself = v1.x own-player direction, not WorkerW parasitism. |
| Q3 | Material system | **ACCEPT** — 4 named styles collapse into ONE Adaptive Frost (per-zone luminance sampling, auto light/dark with override) + opacity slider + outline-only toggle. |
| Q4 | Categorization signal | **ACCEPT** — per-zone accent (auto-assigned from a harmonious palette, overridable) + emoji title prefix + preset gallery with semantic names/colors/emoji. |
| Q5 | Title system | **ACCEPT** — top-left label chip, auto-inverting ink, HarmonyOS Sans SC/Inter 600 default, title overhangs panel top (~0.4 cell) reclaiming the first icon row; handwritten demoted to optional font. |
| Q6 | Control collapse + scope | **ACCEPT** — typography five-pack → size S/M/L + emoji + auto ink; corner default 20 (8–28); corner + title settings become per-zone with explicit 应用到全部. |
| Q7 | app→desktop bridge | **(a) auto-suggest REJECTED by owner** — no built-in AI model; semantic naming of clusters is guesswork; most desktops have no spatial structure; empty desktops have no input; wrong guesses feel jarring. **Replaced with Q7′ (accepted):** ① curated preset gallery whose thumbnails render live ON the user's actual wallpaper (choice, not prediction; works on empty desktops), ② apply DoneCard gains a 「最后一步」 line + [去桌面整理] minimize button, ③ 「分区落版」 apply wave. Full guided 整理模式 (icon-in-zone detection) deferred to v1.x. |
| Q8 | Half-cell snapping | **ACCEPT (keep half-cell)** — but guides show only the lines currently snapped to; the full-grid overlay is removed. PM's kill-it proposal rejected. |
| Q9 | Overlap policy | **ACCEPT** — allowed with warn-wash + edge magnetism makes tiling the path of least resistance; no hard blocking. |
| Q10 | Interaction detail pack | **ACCEPT ALL** — auto-rename on create, Alt-drag duplicate, visible undo/redo + delete-undo toast, snap-pulse on release only, "Scrim overlay" renamed 图标清晰度 (icon clarity), multi-select deferred. |

Non-decision fixes adopted directly: stable zone ids (kill `key={i}`), suppress
recompose/composing during open interactions, mock renders zones (free once the
compositor is client-side).
