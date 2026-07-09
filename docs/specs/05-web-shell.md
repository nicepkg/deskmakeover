# Spec 05 — Web Shell: WebView2 Host + React UI + Bridge Contract (schema 3)

Living spec (ADR-0011 shell architecture; renderer ownership per ADR-0014/0015).
Rewritten 2026-07-10 to the post-inversion reality: **the web renders the pixels;
C# is the system hand.** Design law remains specs 02 (visual language), 03
(navigation), 04 (wallpaper), 06 (icons). The old prototype and its evidence
screenshots are historical only — the specs + `docs/STATE.md` are the truth.

> ⚠️ **Integration status.** The web speaks bridge **schema 3**
> (`src/DeskMakeover.Web/src/bridge/types.ts`); the C# host still implements
> **schema 1** (`Host/Bridge/Contracts.cs`) with the pre-inversion RPC surface.
> Until the F8 host pass lands, only the browser + mock loop runs; the host
> sections below describe the CONTRACT the host must meet, not what it does today.

## 1. Process architecture

```
DeskMakeover.App.exe (WPF host, frameless window)
├─ Host/            WebView2 container · JSON-RPC dispatcher
├─ Orchestration/   DesktopBakeService · wallpaper snapshot/apply · ScreenMetrics
├─ Resources/       ChangelogProvider · ExternalLinks · AppSettings
└─ engine refs      Core / Operations / Shell / IconRendering / ElevatedHelper
        ▲  JSON-RPC (WebMessageReceived, request/response + events)
        ▼  virtual host (static assets + source images)
DeskMakeover.Web (React 19 SPA, built by Vite/Bun, served from disk)
```

**Renderer ownership (ADR-0014/0015 — the inversion):**

- **Icons**: a CPU TypeScript compositor (`src/icon-compositor/`, run in a Worker)
  renders every preview tile AND the 256px bake master. The host supplies 256px
  source PNGs once per scan (`sourceUrls`); the web returns baked PNGs on apply.
- **Wallpaper**: a Pixi v8 compositor (`src/compositor/`) renders the live zone
  preview and bakes the final PNG at native resolution (main-thread `canvas.toBlob`).
  The host supplies ONE cover-cropped source bitmap per source change.
- **C# keeps**: window/chrome, source decode (WIC: JPEG XR/HEIC/cover-crop), ICO
  ladder + packaging, shell writes (icon bake, `SetWallpaper`), backup/restore,
  settings + changelog + diagnostics, the elevated helper boundary.
- There is **no continuous host→web pixel stream** (the old SharedBuffer
  preview-frame channel is retired). Big data crosses the bridge once per source
  change (host→web source URL) and once per apply (web→host PNG bytes).
- **WYSIWYG holds by construction**: the preview and the bake are the same web
  code at different resolutions — not two renderers to keep in parity.

**Hosting:**

- Production: `SetVirtualHostNameToFolderMapping("app.deskmakeover", <exe>/web)` →
  navigate `https://app.deskmakeover/index.html`; a second mapping serves
  engine-provided content (source images, fonts).
- Dev: `DESKMAKEOVER_DEV_SERVER=http://localhost:5173` navigates to Vite for hot
  reload (Debug-only affordance; Release ignores it).
- WebView2 settings: context menus off, browser accelerators off, zoom controls
  off (the app owns Ctrl+wheel), status bar off, DevTools Debug-only. Navigation
  outside the virtual hosts (+ dev server in dev) is cancelled; external links go
  through `shell.openExternal`. Hardening checklist:
  `docs/references/webview2-pitfalls.md` (web items live in
  `src/lib/webview-hardening.ts`; host items audited at F8).

## 2. Window chrome

- The WPF window is frameless (`WindowChrome`, `CaptionHeight=0`, 6px resize
  border, `WM_GETMINMAXINFO` honours the work area). The web titlebar declares
  `app-region: drag`; `IsNonClientRegionSupportEnabled = true` gives native drag /
  double-click maximize / snap.
- Caption buttons are DOM buttons → `shell.minimize|maximize|restore|close`; the
  host raises `window-state` so the maximize glyph tracks real state.
- Min size 1024×700. Layout is **canvas left + RIGHT inspector** (280px, 248px
  compact — `components/shell/module-layout.tsx`); the compact breakpoint is CSS-only.

## 3. Bridge protocol

Envelope over `postMessage` JSON (host: `WebMessageReceived`; web: typed
`bridge.call(method, params): Promise<T>` in `src/bridge/client.ts`):

```jsonc
{ "kind": "req",   "id": 17, "method": "wallpaper.applyBaked", "params": {...} }
{ "kind": "res",   "id": 17, "ok": true,  "result": {...} }
{ "kind": "res",   "id": 17, "ok": false, "error": { "code": "...", "message": "..." } }
{ "kind": "event", "topic": "settings-changed", "data": {...} }
```

Rules:

- Every request is answered exactly once; host handlers run off the UI thread
  except COM/STA work (marshalled through the STA thread).
- DTOs are defined once per side — `Host/Bridge/Contracts.cs` mirrored by
  `src/bridge/types.ts` — and both carry `BRIDGE_SCHEMA_VERSION` (**3**); host and
  web assert equality at startup (fail loudly, no silent drift).
- In the browser the same `call()` routes to the mock host (`src/bridge/mock.ts`
  + `mock-desktop.ts`): full ~120-icon fake desktop, data-only (the web renders
  the pixels either way).

### 3.1 Method inventory (schema 3 — what the web calls today)

| Method | Notes |
|---|---|
| `shell.minimize/maximize/restore/close` | window controls |
| `shell.openExternal { url }` | default browser; http(s)/mailto whitelist |
| `shell.openDataFolder {}` | Explorer at the app data dir |
| `app.getInfo {}` | version, product names, per-locale changelog entries, links |
| `diagnostics.getInfo {}` | OS/.NET/WebView2/arch + host log tail (crash gate) |
| `settings.get {}` / `settings.set { patch }` | persisted via `AppSettings`; `settings-changed` echoes |
| `fonts.list {}` | installed families (zh display names) + bundled faces |
| `icons.scan {}` | desktop scan → grid + items with 256px `sourceUrls` (incl. Recycle Bin ×2 + arrowUrl) |
| `icons.getState {}` | full `IconsStateDto` (config, kindPolicy, overrides, history) |
| `icons.setLook { ... }` | persist config/overrides/kindPolicy |
| `icons.applyBakedBegin / Chunk / Commit` | **gated: user click only.** web-baked ICO payloads stream host-ward in chunks → GeneratedIconStore → journaled bake |
| `icons.restore {}` | snapshot restore |
| `icons.exportCompare {}` | comparison image export |
| `wallpaper.getState {}` | current `LookDto`, grid, fingerprint match |
| `wallpaper.getSource {}` | ONE cover-cropped source bitmap (URL) per source change |
| `wallpaper.setLook { look }` | persist zones/clarity |
| `wallpaper.applyBaked { png }` | **gated: user click only.** backup anchor → write file → `SetWallpaper` |
| `wallpaper.restore {}` | snapshot-aware restore |

Planned host-side additions at F8 (already in STATE §F8, not yet called by the
web): `wallpaper.exportPng` (native save dialog), `wallpaper.setImportedSource`
(persist imported sources across launches).

### 3.2 Events (host → web)

| Topic | Payload |
|---|---|
| `window-state` | `{ maximized }` |
| `settings-changed` | full settings DTO (any source) |
| `os-theme-changed` | effective dark/light when following system |
| `host-error` | host-side exception surfaced to the web crash gate |

(The old `preview-frame` / `scan-progress` / `apply-progress` topics died with
the pixel stream; progress is computed where the work happens — in the web.)

## 4. Web app structure (`src/DeskMakeover.Web`)

```
src/
├─ bridge/          typed client, schema 3 DTOs, mock host + mock desktop (data-only)
├─ stores/          zustand: app, icons, wallpaper, toasts
├─ icon-compositor/ CPU icon renderer: shapes (Figma corner-smoothing + authored
│                   cubics), colour math, segment (极致单色), filters, marks; Worker-run
├─ compositor/      Pixi v8 wallpaper compositor + material recipes
├─ lib/             i18n (resx-generated), canvas-view (pan/zoom/fit), geometry,
│                   shape-paths (chip clip = engine authoring), zone-math, motion
├─ components/
│  ├─ common/       primitives (Segmented, SwatchPicker, chip-preview, confetti…)
│  ├─ shell/        TitleBar, ModuleRail, ModuleLayout, welcome gate, crash gate, dev menu
│  ├─ canvas/       icons-mirror + tiles, wallpaper-mirror + zone-layer, taskbar strip
│  └─ panels/       icons-panel, icons-participation, wallpaper-panel (+popovers,
│                   dim-card, zone-list), settings-page
└─ App.tsx          module routing (icons/paper/settings), global keyboard map
```

- **State**: zustand; modules stay MOUNTED across switches (visibility-hidden,
  never display:none — ADR-0013 flash fix); undo/redo history coalesces
  continuous inputs.
- **Keyboard**: Ctrl+1/2/3 modules; Ctrl+Z/Y wallpaper history; Esc deselects the
  zone first; **hold-Space = global compare** (text inputs excluded; buttons
  activate via Enter — ADR-0013 amendment 2026-07-10).
- **i18n**: TS dictionaries GENERATED from the resx source; strictly keyed;
  Mac-authored strings carry `// PENDING-RESX` until the F8 sweep.
- **Motion**: motion/react presets in `lib/motion.ts`; reduced-motion degrades to
  crossfades. (Some component-local timings exist; new motion should prefer the
  shared tokens.)
- **Styling**: Tailwind 4 `@theme` tokens per spec 02 v3 — **light-first, follows
  system** (ADR-0013). Banned-colour + cool-gray + dash-free copy rules are
  test-gated (`tests/banned-colors.test.ts` — reviewed exemptions listed there).

## 5. Testing & verification

| Layer | Harness |
|---|---|
| Web logic (compositors, stores, zone math, i18n parity, colour/copy gates) | `bun test` (297 at HEAD) + `tsc -b` |
| Engine + orchestration (C#) | `dotnet test` (277 pre-v3; re-verify at F8) |
| Bridge controllers | .NET tests with a fake transport (F8 refresh to schema 3) |
| E2E | raw CDP client driven by Bun — **no Playwright, no Node** (`docs/development.md` §3.4); opt-in `DESKMAKEOVER_E2E=1`, applies stubbed via `DESKMAKEOVER_FAKE_APPLY=1` |
| Visual | browser screenshots against the evidence dirs (`docs/plans/evidence/`) |

Iron law: no completion claim without fresh build + test output. The two live
gates (icon bake, wallpaper apply) stay owner-supervised, never auto-triggered.

## 6. Packaging

- `bun run build` (tsc -b + Vite) → `src/DeskMakeover.Web/dist`; the App carries
  it as `<out>/web`.
- ⚠️ **Packaging is UNVERIFIED** (see `docs/development.md` §5): `publish.ps1`
  currently publishes the App only (no ElevatedHelper, no web build step). A
  proven shippable artifact is F8 work; nothing has shipped (version 0.0.0).
- WebView2 Evergreen is assumed present (never bundled); user-data folder
  `%LocalAppData%/DeskMakeover/webview2`.
