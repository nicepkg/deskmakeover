# Spec 05 — Shell: Tauri Host + React UI + Bridge Contract (schema 8)

Living spec (shell architecture per ADR-0019; renderer ownership per ADR-0019 as amended from
0014/0015). The product is **Tauri 2 + Rust**: the Rust host is the system hand, one Rust icon
core is the pixel truth, and the React web app is the UI. Design law remains specs 02 (visual
language), 03 (navigation), 04 (wallpaper), 06 (icons), 08 (calm). The old prototype and its
evidence screenshots are historical only — the specs + `docs/STATE.md` are the truth.

> **Integration status.** The bridge contract is `BRIDGE_SCHEMA_VERSION = 8` (`src/bridge/types.ts`
> + the generated `src/bridge/generated.ts`). Wallpaper (schema 6), icons (schema 7) and calm
> (schema 8) route through real Rust on **Mac-Tauri**; the browser + mock loop also runs on any OS.
> Every real Windows shell/COM/registry/WIC call is `[WINDOWS-VERIFY]` — the platform bodies are
> blind-written and unproven on a real box. The retired C# WebView2 host is `legacy/`-only (ADR-0019).

## 1. Process architecture

```
Tauri 2 app (src-tauri/, frameless window)
├─ commands       #[specta] verb handlers → AppState (wallpaper/icon/tweaks hosts + settings)
├─ hosts          WallpaperHost · IconHost · TweaksHost (ports + stores under one mutex each)
├─ protocols      dmwallpaper:// · dmicon:// custom protocols serve source bitmaps to the webview
└─ crates/        dm-domain · dm-operations · dm-windows · dm-elevated · dm-icon-core · dm-contracts
        ▲  Tauri invoke (typed commands; generated bindings, request/response + events)
        ▼  custom protocol (source images) + bundled assets
React 19 SPA (src/, built by Vite/Bun) — hosted in the Tauri webview (WebView2 on Windows)
```

**Renderer ownership (ADR-0019 single-truth):**

- **Icons**: the Rust `dm-icon-core` renders every preview tile AND the bake master — compiled to
  **WASM** for the web preview/bake and to **native** for apply/background. The frozen TS
  `src/icon-compositor/` is the byte-parity ORACLE only (tree-shaken out of the bundle).
- **Wallpaper**: a Pixi v8 compositor (`src/compositor/`) renders the live zone preview and bakes
  the final PNG in the web; the Rust host supplies one cover-cropped source bitmap per source change
  over `dmwallpaper://`.
- **Rust host keeps**: window/chrome, source decode (WIC), ICO ladder + packaging (dm-icon-codec),
  shell writes (icon bake, `SetWallpaper`), backup/restore, the durable WAL transaction + CAS
  ledger, settings/changelog/diagnostics, calm registry writes, the elevated-helper boundary.
- **No continuous host→web pixel stream.** Source bitmaps cross once per source change (host→web via
  custom protocol); baked PNG/ICO bytes cross once per apply (web→host).
- **WYSIWYG holds by construction**: the WASM preview and the native bake are the *same Rust core*,
  not two renderers to keep in parity.

**Hosting:**

- The webview loads the bundled app; `dmwallpaper://` / `dmicon://` custom protocols serve
  host-provided source images (content-addressed, `?rev=N`).
- Dev: `bun run tauri:dev` merges `tauri.dev.conf.json` (relaxed CSP for Vite HMR) over the base
  `tauri.conf.json` (strict production CSP; Tauri hashes the app's own inline scripts at build time).
- Webview hardening: drop-navigation guard, Ctrl+wheel page-zoom guard, host-only context-menu /
  accelerator suppression (`src/lib/webview-hardening.ts`, also protects the browser dev loop);
  Windows-side WebView2 settings audited against `docs/references/webview2-pitfalls.md` `[WINDOWS-VERIFY]`.

## 2. Window chrome

- The Tauri window is frameless (`decorations:false`) to match the Win11-style web titlebar, min
  1024×700. The web titlebar declares `data-tauri-drag-region`; caption buttons are DOM buttons →
  the real window minimize/maximize/close; the host raises `window-state` so the maximize glyph
  tracks real state. Position/size persist (`tauri-plugin-window-state`); a second launch focuses
  the existing window (`tauri-plugin-single-instance`).
- Layout is **canvas left + RIGHT inspector** (280px, 248px compact —
  `components/shell/module-layout.tsx`); the compact breakpoint is CSS-only.

## 3. Bridge protocol

The bridge is Tauri `invoke`: typed commands generated from the Rust `#[specta::specta]` surface via
tauri-specta into `src/bridge/generated.ts`. `src/bridge/types.ts` holds the hand DTOs + the
`BRIDGE_SCHEMA_VERSION` constant (**8**), asserted against the host at startup (fail loudly, no
silent drift). In a plain browser the same client routes to the mock host (`src/bridge/mock.ts` +
`mock-desktop.ts`): a full ~120-icon fake desktop, data-only (the web renders the pixels either way).

**Thin-bridge law (owner ruling D1, schema 6/7/8).** Rust does the platform I/O and returns THIN
data; the frontend assembles the rich store shapes. Wallpaper: Rust does screen enumeration +
get/set + capture/restore, returns thin `ScreenInfoDto[]`/`WallpaperResultDto`; per-monitor draft
looks + `WallpaperStateDto` are frontend-assembled, and `wallpaper.setLook` leaves the bridge
(frontend `localStorage`). Icons: Rust does scan / package + apply / restore / persist ②③ and
returns thin `IconScanDto`/`IconPersistedDto`/`IconOpResultDto`; the frontend assembles
`IconsStateDto` via `lib/icons-assemble`, and `icons.setLook` leaves the bridge (frontend draft,
resumed from ② on relaunch). Calm: the `tweaks*` verbs return thin `CalmProbeRowDto`/`CalmApplyRowDto`/
`CalmRestoreRowDto`/`CalmGuidedProbeDto`; the store never learns mock vs real Rust.

### 3.1 Command families (authoritative surface: `src/bridge/generated.ts`)

| Family | What |
|---|---|
| `shell.*` | window controls · open external (http(s)/mailto whitelist) · open data folder |
| `app.getInfo` / `diagnostics.getInfo` | version/product/changelog/links · OS/webview2/arch + host log tail |
| `settings.get` / `settings.set` | persisted in rusqlite; `settings-changed` echoes |
| `wallpaper.*` | `getScreens` (thin) · `getSource` (via `dmwallpaper://`) · `applyBaked` · `restore` (setLook is frontend) |
| `icons.*` | `scan` · `getPersisted` · `apply`/commit (gated, user-click only) · `restore` · `switchVersion` · `exportCompare` (setLook is frontend) |
| `tweaks.*` (calm) | probe rows · apply rows · restore rows · guided probe — fail-closed until the Windows cert lab (ADR-0023 W3) turns green |

Mutating verbs that touch the real desktop (icon bake, wallpaper apply, calm writes) are
**owner-supervised, user-click only, never auto-triggered**.

### 3.2 Events (host → web)

| Topic | Payload |
|---|---|
| `window-state` | `{ maximized }` |
| `settings-changed` | full settings DTO (any source) |
| `os-theme-changed` | effective dark/light when following system |
| `host-error` | host-side error surfaced to the web crash gate |

## 4. Web app structure (`src/`)

```
src/
├─ bridge/          typed client + generated bindings, schema 8 DTOs, mock host + mock desktop (data-only)
├─ stores/          zustand: app, icons, wallpaper, calm, toasts
├─ icon-compositor/ FROZEN TS icon renderer — byte-parity oracle (production pixels = dm-icon-core WASM)
├─ compositor/      Pixi v8 wallpaper compositor + material recipes
├─ lib/             i18n (i18n/{en,zh-hans}.ts = the source), icons-assemble, canvas-view, geometry,
│                   shape-paths (chip clip = engine authoring), zone-math, motion
├─ components/
│  ├─ common/       primitives (Segmented, SwatchPicker, chip-preview, confetti…)
│  ├─ shell/        TitleBar, ModuleRail, ModuleLayout, welcome gate, crash gate, dev menu
│  ├─ canvas/       icons-mirror + tiles, wallpaper-mirror + zone-layer, taskbar strip, calm schematics
│  └─ panels/       icons-panel, icons-participation, wallpaper-panel, calm page, settings-page
└─ App.tsx          module routing (icons/wallpaper/calm/settings), global keyboard map
```

- **State**: zustand; modules stay MOUNTED across switches (visibility-hidden, never display:none —
  ADR-0013 flash fix); undo/redo history coalesces continuous inputs.
- **Keyboard**: Ctrl+1/2/3/4 modules (图标/壁纸/清爽/设置); Ctrl+Z/Y wallpaper history; Esc deselects
  the zone first; **hold-Space = global compare** (text inputs excluded; buttons activate via Enter).
- **i18n**: `src/lib/i18n/{en,zh-hans}.ts` are the hand-edited source (the resx pipeline retired with
  the .NET tree, 2026-07-11); strictly keyed, zh-Hans + English in lockstep.
- **Motion**: motion/react presets in `lib/motion.ts`; reduced-motion degrades to crossfades.
- **Styling**: Tailwind 4 `@theme` tokens per spec 02 v3 — **light-first, follows system** (ADR-0013).
  Banned-colour + cool-gray + dash-free copy rules are test-gated (`tests/banned-colors.test.ts`).

## 5. Testing & verification

| Layer | Harness |
|---|---|
| Web logic (compositors, stores, zone math, i18n parity, colour/copy gates) | `bun test` + `tsc -b` |
| Rust engine (transaction/CAS/recovery, icon parity, calm decision core) | `cargo test --workspace` + `cargo check --target x86_64-pc-windows-msvc` |
| Bindings drift | `bun run check:bindings` (fails if `generated.ts` is stale) |
| Icon parity | `bun tests/icon-parity/m5/run.ts` (TS↔Rust byte-identical over the real corpus) |
| E2E | raw CDP client driven by Bun — no Playwright, no Node (`docs/development.md`); opt-in `DESKMAKEOVER_E2E=1`, apply stubbed via `DESKMAKEOVER_FAKE_APPLY=1` |
| Windows runtime | the `[WINDOWS-VERIFY]` matrix on a real box (M1 spikes, M3/M4 checklist, calm W3 cert lab) — unproven |

Iron law: no completion claim without fresh build + test output. The live gates (icon bake,
wallpaper apply, calm writes, resident audit) stay owner-supervised, never auto-triggered.

## 6. Packaging

- `bun run build` (tsc -b + Vite) → `dist/`; the Tauri build bundles it.
- The shipping artifact is `bun run tauri build` (NSIS on Windows) bundling the Rust host + web +
  the `dm-elevated` helper — **M8, NOT STARTED**; signing/updater are open; version `0.0.0` until
  the owner names the first release. The `legacy/` .NET publish scripts are oracle-only, never ship.
- WebView2 Evergreen is assumed present on Windows (never bundled).
