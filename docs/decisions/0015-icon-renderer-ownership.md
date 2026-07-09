# ADR-0015 — Icon renderer ownership: web interactive renderer, C# frozen oracle

Date: 2026-07-09 · Status: accepted (owner) · Record:
`docs/reviews/2026-07-09-icon-frontend-panel.md` (4 recon agents + 4 isolated seats)

## Context

Icons were the last module whose pixels were rendered host-side: every knob change
round-tripped to C#, re-rendered N tiles × 2 PNGs to disk behind a 420ms debounce.
Meanwhile `mock-desktop.ts` maintained an approximate TS duplicate for the Mac dev
loop — two divergent renderers, one of them admittedly lying. The owner wants icon
work to move web-ward (stronger AI tooling), and plans a v1.2 tray-resident
background auto-styler for newly added icons. `DeskMakeover.IconRendering` is pure
Rgba32 math (no Win32), so a TS port is translation, not research.

## Decisions

**D1 — One interactive renderer, in TypeScript.** A pixi.js v8 icon compositor
renders the live preview AND the apply output. Same pipeline, two resolutions:
display-size for editing, 256px RGBA master for apply. `mock-desktop.ts`'s styling
approximation is deleted; the mock keeps only data (grid/items/sources).

**D2 — The 256 boundary.** The web NEVER produces sub-256 frames. C# keeps the
tested linear-light ladder ([256,48,32,24,20,16] via `IconResampler`) and
`IcoWriter`. Consequences: the sRGB-vs-linear resampling risk is void; cross-renderer
parity is pinned at 256 only; every shell writer is untouched.

**D3 — C# `TileRenderer` is FROZEN, not deleted.** It remains as (a) the parity
oracle producing golden fixtures and (b) the reserved renderer for unattended
background work. Freeze discipline: banner comments in the frozen files; new styles
ship TS-only; any C# style change requires an ADR. This is not dual maintenance —
the frozen set only ever renders the styles that existed at freeze time, until D4
forces a decision with hardware evidence.

**D4 — Unattended desktop writes never depend on a hidden WebView2.** The v1.2
background auto-styler renders in C# in-process. Evidence: no anti-throttling
browser args exist (`WebShellWindow.cs`), no headless bake has ever run in this
repo (wallpaper bakes on the visible app), hidden/occluded WebView2 throttles rAF,
backgrounds the GPU and can lose WebGL contexts, and the blast radius of baking a
blank frame into a real desktop icon is unacceptable. Revisit only with real
hardware proof.

**D5 — WYSIWYG becomes visual parity, per style tier.** Flat shape/color cells:
ΔE<2 and SSIM≥0.995 vs the oracle. Filtered cells (glass/pixel/sticker): SSIM≥0.98
with a max-region-error cap. Bit-exactness across a GPU shader and a C# CPU pass is
explicitly abandoned (owner-approved).

**D6 — Sequencing.** Development continues on the Mac mock loop. ALL Windows-gated
work batches into one session: wallpaper F8 handoff + icon golden generation + icon
parity run + `icons.applyBaked` host implementation + discovery fix + `dotnet
build/test`.

**D7 — The simulated taskbar is generic-but-photoreal scenery.** Designer P0 only
(pinned Fluent-style neutral glyphs + running-indicator pills + tray cluster +
theme-following acrylic + Start flag fix). It never impersonates the user's real
taskbar, never gains interactivity, and never shows Microsoft-owned assets. PM's
"cut it entirely" was overruled by the owner with the designer's bounded scope.

**D8 — Context menus expose owned verbs only.** Tile: keep/follow/tint. Canvas:
icon size + refresh. Windows' Sort/auto-arrange verbs are permanently out; no
Win32-lookalike menu chrome.

**D9 — Asset licensing gates.** Shipped/committed art: own procedural generation +
MIT sources only. Extracted Windows icons, Segoe fonts, brand marks: local
dev-reference only, never in git. The win11 simulator repos are structure-reference
(code Apache-2.0/CC0), asset-forbidden.

**D10 — Five-material unification approved as direction.** Icon styles gain the
wallpaper's material vocabulary (磨砂/柔光/实色/悬浮/描边) as the FIRST TS-only
style batch after migration; 柔光 takes the curated slot of the weakest filter
(像素, demoted not deleted); 缎光角/珐琅光弧 get heavier or demoted (3s-read gate).
Sequenced after parity, not in the migration slice.

## Amendment 2026-07-09 — web is the geometry oracle too; catalog + duotone

The owner-iteration marathon (commits 5bee40c..7b8a5bc) extended D1 (web owns
rendering) to **shape geometry authoring**: `icon-compositor/shapes.ts` is now the
canonical geometry source (Figma corner-smoothing engine ported from
`squircle-path-kit`, MIT), and the C# `IconShapeGeometry` **re-ports FROM the web**
in the Windows batch — the shape-geometry oracle direction flipped web-authoritative
(the C# pipeline stays the frozen PARITY oracle for colour/filter/mark rasters). Also
decided:

- **Catalog curated** (owner): culled Google/Brave/Squircle/Blob/Rectellipse/Hexagon;
  added Diamond + Flower + Pebble (`maskable.app` OEM masks, MIT). 11 shapes.
- **极致单色 duotone** — a new TS capability: subject/background segmentation
  (`segment.ts`), layered Mono composition, `monoStyle: Tonal|Flat`.
- **ConfigDto grew** `monoStyle` + `plateColor` → **bridge schema v2**; C#
  `IconsContracts.cs` + `BridgeSchema.Version` + 4 preset defaults sync in the
  Windows batch. Nullable/enum additions are strict-decoder-safe.
- **Gloss filter** live (TS `filters.ts`); marks silhouette-aware; Card→Shadow,
  Echo→Halo (MarkStyle enum renamed — C# enum syncs in the batch).

Detail: spec 02 §Shape System/§Colour Treatments/§Shortcut Marks, spec 06 §3.11.

## Consequences

- Icon visual iteration stops being Windows-gated; the Mac loop shows engine truth.
- One source of style truth in TS; the C# freeze is enforceable by banner + review.
- Apply payload moves from disk-PNG-per-restyle to chunked masters per APPLY only
  (~5-7MB per 300-icon apply); preview traffic drops to zero.
- 300-icon apply stays 20-30s (shell-write-bound, unchanged); progress UI required.
- The parity corpus becomes a permanent regression net for the TS renderer.
- Deleted: the v0.9 legacy styling path (IconStyler chain) and the mock styling
  approximation — grep-verified, `dotnet build` re-verified in the Windows batch.
