# ADR-0011: UI replatforms to WebView2 + React; the engine stays C#

**Status:** accepted
**Date:** 2026-07-08
Amends ADR-0001 (WPF as the UI stack) and ADR-0008 (prototype parity is now
served by porting the React prototype's design back to React). Specs 02/03/04
remain the design law; spec 05 defines the new shell architecture.

## Context

- The owner's verdict on the WPF UI after v1.1: "勉强能用，谈不上好用" — the
  hand-written XAML components repeatedly missed the visual bar and each polish
  round was expensive (build → launch → capture; no hot reload; WPF primitives
  cost 10-20× CSS for hover/transition/blur-grade polish).
- The **binding UI contract is already a React artifact**
  (`docs/references/prototype/桌面美颜 v2.dc.html`, ADR-0008). Porting the
  design to React restores it in its native medium instead of transliterating
  it to XAML.
- The engine layers (`Core`, `Operations`, `Shell`, `IconRendering`) have zero
  WPF type dependencies (verified). The WYSIWYG law lives in engine code
  (`TileRenderer`, `WallpaperComposer`), not in the UI layer.
- v1.1 (rail + wallpaper) is unreleased, so there is no compat surface; the old
  UI can be deleted outright.

## Decision

1. **The visible UI becomes a web app** — React 19 + TypeScript + Tailwind CSS 4
   + shadcn/ui + Motion, hosted in a WebView2 control inside a frameless WPF
   window. Package versions are taken from the live registry at scaffold time
   (never from model memory) and recorded in STATE.md.
2. **Toolchain is Bun-only** (owner order): `bun` for install/scripts/bundling
   glue, `bun test` for TS unit tests, `bunx` for CLIs. **Node is not used.**
   E2E runs through **Microsoft.Playwright (.NET)** against WebView2's CDP
   endpoint, inside `dotnet test` — no Node dependency anywhere.
3. **The engine stays C# and untouched in meaning**: scanning, tile rendering,
   wallpaper compose, bake, snapshot/restore, elevation, persistence. The host
   process exposes them over a JSON-RPC bridge (`WebMessageReceived`) plus a
   shared-buffer channel for preview pixels. Zone-title rasterisation
   (FormattedText) stays host-side so baked text keeps the same pen.
4. **WYSIWYG law is preserved by transport, not re-rendering**: the web layer
   displays engine-produced pixels 1:1 (shared-buffer frames onto a canvas at
   native resolution; tile PNGs at exact device-pixel size). Web CSS scaling is
   viewport fitting only — the same role WPF's Viewbox played. No visual state
   may be painted web-side that the bake cannot reproduce.
5. **Prefer established component primitives** (owner order): shadcn/ui is the
   base vocabulary; hand-built components are reserved for identity pieces the
   library cannot express (AngleDial, zone editor, 调色盘, squircle mask, mirror
   canvas).
6. **The WPF UI layer is deleted** (Views/ViewModels/Controls/Theming
   XAML/Presentation mappers) once module parity is verified. Its ViewModel
   tests are replaced by `bun test` state tests + Playwright E2E; engine and
   orchestration tests stay in .NET.
7. **Gates unchanged**: the real desktop icon bake and the wallpaper apply stay
   owner-supervised, click-triggered only — the bridge exposes them as explicit
   RPCs that no automation calls.

## Consequences

- New project `src/DeskMakeover.Web` (Vite + React SPA) and a `Host/` layer in
  `DeskMakeover.App` (WebView2 window, bridge, RPC controllers). The App project
  gains its first NuGet dependency (`Microsoft.Web.WebView2`).
- The publish script bundles `DeskMakeover.Web/dist` into the app output and is
  executed with Bun.
- Runtime requirement: WebView2 Evergreen (preinstalled on Win11 / updated
  Win10; a bootstrapper note ships in the README — deferred until release).
- Memory footprint grows by the WebView2 process (~100-150 MB working set);
  accepted for the visual ceiling and iteration speed gained.
- resx string tables move to typed TS dictionaries (zh-Hans/en) in the web app;
  the host keeps only engine-facing strings.
- Snap-layout hover on the maximize button is lost (DOM caption buttons);
  dragging/resize/snap themselves keep working via non-client region support.
  Accepted (standard for hybrid shells).
