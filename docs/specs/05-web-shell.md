# Spec 05 — Web Shell: WebView2 Host + React UI + Bridge Contract

Living spec (ADR-0011). Design law remains specs 02 (visual language), 03
(navigation), 04 (wallpaper); this spec defines **how the UI is built and how it
talks to the engine**. On visual conflicts the prototype + evidence screenshots
win (`docs/plans/evidence/2026-07-settings-i18n-shapes-icon/*.png` are the
current-layout baseline the web UI must restore).

## 1. Process architecture

```
DeskMakeover.App.exe (WPF host, frameless window)
├─ Host/            WebView2 container · JSON-RPC dispatcher · frame pusher
├─ Orchestration/   MakeoverService · DesktopBakeService · WallpaperComposer
│                   WallpaperSnapshotService · WallpaperLookStore · ScreenMetrics
├─ Preview/         shell icon extraction (exact-pixel tile PNGs)
├─ Resources/       ChangelogProvider · ExternalLinks · AppSettings
└─ engine refs      Core / Operations / Shell / IconRendering / ElevatedHelper
        ▲  JSON-RPC (WebMessageReceived, request/response + events)
        ▼  SharedBuffer (BGRA preview frames)   ▼ virtual host (static assets)
DeskMakeover.Web (React 19 SPA, built by Vite, served from disk)
```

- **Production**: `SetVirtualHostNameToFolderMapping("app.deskmakeover", <exe>/web)`
  → navigate `https://app.deskmakeover/index.html`. A second mapping
  `assets.deskmakeover` → `%LocalAppData%/DeskMakeover/webassets` serves
  engine-generated content (tile PNGs, wallpaper source snapshots, fonts).
- **Dev**: `DESKMAKEOVER_DEV_SERVER=http://localhost:5173` env var switches the
  navigation target to the Vite dev server (bun-run) for hot reload. Dev-only;
  release builds ignore it unless a debugger is attached.
- WebView2 settings: context menus off, browser accelerator keys off, zoom
  controls off (the app owns Ctrl+wheel), status bar off, DevTools on only in
  Debug. Any navigation outside the two virtual hosts (and the dev server in
  dev) is cancelled; `window.open`/external links route through `shell.openExternal`.

## 2. Window chrome

- The WPF window is frameless (`WindowChrome` with `CaptionHeight=0`, resize
  border 6px, `WM_GETMINMAXINFO` honours the work area — port from the current
  MainWindow). The web titlebar (46px, spec 02) declares `app-region: drag`;
  `CoreWebView2.IsNonClientRegionSupportEnabled = true` makes dragging /
  double-click maximize / system snap work natively.
- Caption buttons are DOM buttons → `shell.minimize|maximize|close`. The host
  raises `window-state` events so the maximize glyph tracks real state.
- Min size 1024×700; compact breakpoint (<1100px) is handled entirely in CSS.

## 3. Bridge protocol

Envelope over `postMessage` JSON (host: `WebMessageReceived`; web: a typed
`bridge.call(method, params): Promise<T>` client):

```jsonc
{ "kind": "req",   "id": 17, "method": "wallpaper.recompose", "params": {...} }
{ "kind": "res",   "id": 17, "ok": true,  "result": {...} }
{ "kind": "res",   "id": 17, "ok": false, "error": { "code": "...", "message": "..." } }
{ "kind": "event", "topic": "preview-frame", "data": {...} }
```

Rules:

- Every request is answered exactly once; host handlers run off the UI thread
  except COM/STA work, which marshals through the existing `StaThread`.
- All RPC results are serializable DTOs defined once in
  `Host/Bridge/Contracts.cs` and mirrored in `src/DeskMakeover.Web/src/bridge/types.ts`.
  The two files carry a shared `BRIDGE_SCHEMA_VERSION`; host and web assert
  equality at startup (fail loudly, no silent drift).
- **Stale-result safety**: mutating preview calls carry a client `revision`
  int; responses and frames echo it; the web store drops anything older than
  the latest revision it issued (ports the WPF debounce-token pattern).

### 3.1 Method inventory

| Method | Notes |
|---|---|
| `shell.minimize/maximize/restore/close` | window controls |
| `shell.openExternal { url }` | default browser; whitelisted schemes http(s)/mailto |
| `shell.openDataFolder {}` | Explorer at app data dir |
| `app.getInfo {}` | version, product names, changelog entries, links |
| `settings.get {}` / `settings.set { theme?, language?, autoKeepUp? }` | persisted via `AppSettings`; `settings-changed` event echoes |
| `system.getEnvironment {}` | primary monitor px + DPI, grid (cell/origin/icon px), wallpaper source URL (assets host), fingerprint, dark/light effective theme |
| `icons.scan {}` | rescan desktop; returns items `{ id, label, kind, isShortcut, canStyle, cell }` + `tilesRevision` |
| `icons.restyle { config, overrides, revision }` | engine re-renders every tile PNG into the assets host dir (exact device px, versioned filenames); returns tile URL map |
| `icons.getOriginalTiles {}` | untouched-look tile URLs (compare/peek) |
| `icons.apply { }` | **gated: user click only.** snapshot → journaled bake → progress events |
| `icons.restore {}` / `icons.applyVersion { versionId }` / `icons.history {}` | history stack semantics unchanged (cap 10) |
| `icons.setOverride { itemId, mode, tint? }` | per-icon 保留原样/跟随全局/单独配色 |
| `icons.exportComparison {}` / `icons.exportSnapshot {}` | existing exporters |
| `palette.get {}` | wallpaper-extracted swatches (主色/辅色/quick row) |
| `fonts.list {}` | installed families (zh-cn display names) + bundled handwriting face |
| `wallpaper.getState {}` | current `WallpaperLook`, fingerprint match, paleness |
| `wallpaper.recompose { look, revision }` | debounced web-side; composes at native res; pushes a `preview-frame`; returns `{ pale, fingerprintOk }` |
| `wallpaper.apply {}` | **gated: user click only.** backup anchor → set wallpaper |
| `wallpaper.restore {}` | snapshot-aware restore |
| `wallpaper.saveLook { look }` | persist zones.json |

### 3.2 Events (host → web)

| Topic | Payload |
|---|---|
| `preview-frame` | `{ bufferKey, width, height, revision }` — after `PostSharedBufferToScript` |
| `scan-progress` / `apply-progress` | staged counts for shimmer/bloom choreography |
| `window-state` | `{ maximized }` |
| `settings-changed` | full settings DTO (theme/language changes from any source) |
| `os-theme-changed` | effective dark/light when following system |
| `toast` | `{ text, tone }` for host-initiated notices |

### 3.3 Preview pixel transport (WYSIWYG-critical)

- `WallpaperComposer.Compose` output (BGRA, native monitor res) is posted via
  `CoreWebView2.PostSharedBufferToScript` (read-only). The web canvas paints it
  with `ImageData` at **native pixel dimensions**; fitting to the viewport uses
  CSS transforms only. Lossy encoding is **forbidden** on this path.
- Icon tiles: the engine already renders exact-device-pixel bitmaps
  (`TileRenderer` + `IconResampler`); they are written as PNG (lossless) into
  the assets host dir with content-versioned names (`tile-<id>-<rev>.png`) so
  the browser cache never shows stale styles. DOM lays them out at
  `devicePixelRatio`-corrected CSS sizes; at 100% zoom the browser performs no
  resampling (parity with the WPF rule that the compositor never rescales
  tiles).

## 4. Web app structure (`src/DeskMakeover.Web`)

```
src/
├─ bridge/        typed client, schema version, mock bridge (browser-only dev)
├─ stores/        zustand stores: session, icons, wallpaper, settings, toasts
├─ lib/           snap math (half-cell), geometry (clipFor SVG paths), i18n, motion presets
├─ components/
│  ├─ ui/         shadcn primitives (generated) — restyled by tokens only
│  ├─ common/     Chip, ChipGroup, Slider, AccordionAxis, ExpandAllToggle,
│  │              AngleDial, ColorPicker, SquircleMask, CtaButton(HeroPhase), Toast
│  ├─ shell/      TitleBar, ModuleRail, CompactToolbar
│  ├─ canvas/     MirrorViewport (pan/zoom), IconTile grid, TaskbarStrip,
│  │              ComparePill, ZoneLayer (create/move/resize/rename), GridGuide
│  └─ panels/     IconsPanel, WallpaperPanel, SettingsPage
└─ App.tsx        rail routing (icons/paper/settings), keyboard map
```

- **State**: zustand; `look`/`styleConfig` mutations flow through one
  `mutate(fn)` helper that bumps `revision`, persists (debounced 140ms
  wallpaper / 420ms icons — same constants as WPF), and calls the bridge.
- **i18n**: typed dictionaries (`lib/i18n/{zh-Hans,en}.ts`) ported from the two
  resx files; `t()` is strictly keyed (missing key = type error); a `bun test`
  asserts both locales cover the same key set. Language preference comes from
  `settings.get` (System resolves host-side).
- **Motion**: Motion (`motion/react`) implements spec 02's named keyframes
  (bloom/settle/shimmer/rise/slide/pop) as shared presets in `lib/motion.ts`;
  reduced-motion degrades to crossfades via `useReducedMotion`.
- **Styling**: Tailwind 4 `@theme` tokens transcribe spec 02's variables
  verbatim (dark default + `.light` overrides). The banned-colour rule holds:
  a `bun test` greps built CSS/TSX for blue/violet accent hexes.
- Zone editor math (half-cell snap, exclusive-edge resize, min 2×2, arrow
  nudges) ports 1:1 from `DesktopCanvasView.Zones.cs` into `lib/zone-math.ts`
  with `bun test` fixtures mirroring the C# tests.

## 5. Testing & verification

| Layer | Harness |
|---|---|
| Engine + orchestration (C#) | existing `dotnet test` suites, unchanged |
| Bridge controllers | .NET tests with a fake `IWebBridgeTransport` |
| TS logic (snap math, stores, i18n parity, token grep) | `bun test` |
| E2E | Microsoft.Playwright (.NET) attaching to WebView2 (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port`), driving real clicks through CDP; runs inside `dotnet test`, tagged `E2E` |
| Visual | screenshot evidence vs the three baseline PNGs, per module |

The iron law stands: no completion claim without fresh build + test output.
The two live gates (icon bake, wallpaper apply) remain owner-supervised and are
excluded from E2E (E2E stubs them behind a `DESKMAKEOVER_FAKE_APPLY=1` host flag).

## 6. Packaging

- `bun run build` (Vite) → `src/DeskMakeover.Web/dist`.
- `scripts/publish-win.mjs` (executed with **bun**) builds web first, then
  `dotnet publish`, then copies `dist` → `<out>/web`. Self-contained layout
  otherwise unchanged.
- WebView2 Evergreen is assumed present (Win11 ships it); the user-data folder
  is `%LocalAppData%/DeskMakeover/webview2`.
