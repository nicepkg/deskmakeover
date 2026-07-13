# ADR-0014 — Zone editor rebuild: client-side compositor + Adaptive Frost

- Status: accepted — host I/O ownership **amended by [ADR-0019](0019-tauri-rust-replatform.md)**
  (source decode / PNG write / `SetWallpaper` / backup-restore are now Rust, not the C# host; the
  client-side Pixi compositor + Adaptive Frost decisions stand). (owner, 2026-07-09)
- Supersedes: the rendering pipeline of ADR-0009/ADR-0012 for the wallpaper module
  (spec 04 §4 "one C# renderer"); the 4-named-styles axis; the handwritten-centered
  title recipe.
- Input: 5-seat expert panel + owner dispositions —
  `docs/reviews/2026-07-09-zone-editor-expert-panel.md` (Q1–Q10 all disposed).

## D1 — One TypeScript WebGL compositor (A3)

Zone/clarity composition moves from C# to the web layer: a single TS compositor
(pixi.js v8, WebGL2) renders

- **live**: the visible editor canvas at viewport resolution, every frame during
  gestures (source texture uploaded once per wallpaper change; per-frame cost is
  blur+composite of a ≤viewport-res target, 2–5ms class);
- **bake**: the same code at native resolution in an OffscreenCanvas worker on
  apply → `convertToBlob('image/png')` → PNG bytes to the host, which writes the
  file and calls `IDesktopWallpaper.SetWallpaper`.

`WallpaperBakeRenderer.cs` / `WallpaperComposer.cs` are to be DELETED at F8 (⚠️ status
correction 2026-07-10: both files are STILL PRESENT in the tree — the native-host handoff
that retires them is not done; see STATE.md §F8). Source DECODE
stays in C# (WPF/WIC handles JPEG XR/HEIC/cover-crop; browsers do not): the host
hands the web one cover-cropped RGBA bitmap per source change. The per-edit
host→web 33MB (4K) / 133MB (8K) frame traffic disappears; the bridge carries the
source once and a small PNG back on apply.

**WYSIWYG law restated**: was "preview and bake read one buffer"; is now "preview
and bake run ONE renderer at two resolutions". Migration gate: parity fixtures
(5 looks) C#-vs-TS, ΔE<2 / SSIM>0.99; after migration the TS compositor is the
single truth and the fixtures pin TS output instead.

Degradation: probe `MAX_TEXTURE_SIZE` and SwiftShader (`WEBGL_debug_renderer_info`)
at startup; software-GL hosts drop to reduced preview resolution (bake unaffected —
it is one frame, not 60fps).

## D2 — Video wallpaper: rejected FFmpeg, reserved interfaces (B4 → B3)

Baking zones into video files via FFmpeg is REJECTED: third-party (Steam Workshop)
content is copyrighted and re-encoding derivatives violates ToS; frost frozen from
one frame over moving video reads as broken; every zone move would re-encode.
v1 ships static-only. The compositor reserves: source-texture provider (static
texture today, VideoTexture later), per-style blur-cost tiers (incl. a no-blur
tier), output-target abstraction (StaticImagePNG today, LiveOverlaySurface later),
headless operation (worker). Video itself is a v1.x+ bet in the own-player
direction (own WorkerW surface), never parasitism on another engine's window
hierarchy.

## D3 — Adaptive Frost replaces the 4 named styles

ONE material. Per-panel OKLCH sampling of the covered wallpaper region (L̄, C̄, H̄):
tone auto light/dark at L̄ 0.55 (hysteresis 0.05; user override Auto/Light/Dark).
Light fill OKLCH(0.92, min(C̄×0.5, 0.03), H̄) α 0.60 default (0.35–0.85 slider);
dark fill OKLCH(0.20, …) α 0.52 (0.30–0.80). Frost σ = cellHeight/6. Blur-less
tier: α+0.12 + bottom inner shadow. Depth: 1px top inner highlight (light α.35 /
dark α.14) + 1px outer contour (black α.10 / white α.12, untinted). No baked drop
shadows. Corner radius default 20, range 8–28, per-zone. Outline-only variant
(fill α≤.05 + 2px deep-tone contour) forces the label chip. Per-zone accent colour
auto-assigned from a harmonious palette, overridable — the zone-to-zone
categorization signal the old single-tint system lacked.

## D4 — Title = top-left label chip, not a centered poster

Label chip anchored top-left, title text + optional emoji prefix; chip fill = the
material one step denser; ink auto-inverts against the chip (OKLCH 0.25 / 0.97,
α.96). Default font HarmonyOS Sans SC + Inter, weight 600, size
clamp(cell×0.20, 15, 22)px; S/M/L = cell×0.17/0.20/0.24. Handwritten becomes an
optional font choice. The title OVERHANGS the panel top (~0.4 cell into the
gutter), reclaiming the icon row the old title band consumed; falls back to a
narrow in-panel strip when flush against the screen top or a neighbouring zone.
Title settings are per-zone with an explicit 应用到全部.

## D5 — Interaction contract

Outline and material are same-source same-frame (`look.zones[i]`), both track the
pointer at display refresh; the create rubber-band draws the forming MATERIAL
(not a marquee) + W×H badge; release auto-selects and enters rename. Half-cell
snapping stays; guides light only the edges being snapped (full-grid overlay
removed); zone-edge magnetism ≤0.35 cell + cross-zone span lines + equal-gap
ticks. Overlap allowed with warn-wash. Snap-pulse plays on release commit only.
Stable zone ids (no index keys) + AnimatePresence delete exits. Visible undo/redo
+ delete-undo toast. Alt-drag duplicates. Apply plays the 「分区落版」 wave
(coral sweep 300ms + staggered zone bloom 480ms `[0.34,1.4,0.4,1]`, 60ms/zone;
reduced-motion: one 120ms brightness pulse) and the DoneCard gains a
「最后一步」 line + [去桌面整理] (minimize).

## D6 — Presets are curation, not prediction (owner call)

Auto-suggest from icon clusters was REJECTED (no on-device model; cluster naming
is guesswork; most desktops have no spatial structure; empty desktops have no
input). Instead: 4–6 human-curated presets (semantic names + emoji + accent
palettes) whose gallery thumbnails render live ON the user's actual wallpaper via
the compositor. Guided post-apply 整理模式 is deferred to v1.x.

## Consequences

- Mock/Mac dev loop renders true zone visuals for the first time (the compositor
  is client code) — visual iteration stops being Windows-gated.
- Host shrinks to: decode+crop source, grid metrics, fingerprint, backup/restore,
  write PNG + SetWallpaper. New bridge surface: `wallpaper.getSource` (RGBA or
  losslessly-encoded source), `wallpaper.applyBaked` (PNG bytes in), goodbye
  `wallpaper.recompose` streaming.
- `dotnet` tests for the deleted renderer are replaced by TS compositor tests +
  parity fixtures (F8 verifies on real host).
- Spec 04 is rewritten in place (living spec); §7 acceptance now tests the TS
  compositor, the material/title recipes above, and the interaction contract.

## Amendments

- **2026-07-10 (owner, round 2 — D3/D4 material system EXPANDED).** D3's "ONE
  material (Adaptive Frost)" and D4's single label-chip title were reversed after
  the round-2 review: shipped behaviour is **five materials** and **four title
  styles**, an **optional baked drop shadow** (D3's "no baked drop shadows" no
  longer holds), and wallpaper **import / export**. The default Frost opacity moved
  from `.60/.52` to about **`.74/.76`** (`src/compositor/material.ts`). The
  ownership decision (compositor in the web, host shrinks) stands; only the
  material/title catalogue widened. Formal record here supersedes the "one material"
  wording in D3 above; Spec 04's body still needs the same widening (pending sync).
- **2026-07-10 (status correction).** Bake runs on the MAIN thread via Pixi
  `canvas.toBlob`, not an OffscreenCanvas worker as some passages imply. The
  equal-gap ticks in D5 remain DEFERRED (not accepted). `WallpaperBakeRenderer.cs` /
  `WallpaperComposer.cs` are still in the tree (F8 deletion — see the D1 note).
