# DeskMakeover Web App (`src/`, repo root)

The visible UI of DeskMakeover — a React 19 + TypeScript + Tailwind 4 + Motion SPA. It is hosted by
the **Tauri 2 shell** (ADR-0019; WebView2 on Windows) and also runs in a plain browser with a mock
bridge on any OS for development. (It previously ran inside a standalone WebView2 window driven by a
C# host; that host was retired and removed from the repo on 2026-07-14.)

> The web app lives at the repo root (ADR-0019 Amendment 1: `src/`, `public/`, `index.html`,
> one root `package.json`). The authoritative docs are **`docs/STATE.md`** (what is true / in
> flight) and **`docs/development.md`** (the full dev + build runbook). Read those first; this
> file is only the web-app quick reference.

## Run it (Bun only — never npm/node)

```bash
bun install        # first time
bun run dev        # Vite dev server (default :5173, auto-increments) + mock bridge
bun run build      # tsc -b + production bundle -> dist/
bun test           # web unit tests — includes the banned-colour + copy-law gates
bun run lint       # oxlint
bun run tauri:dev  # launch the REAL Tauri app (mock desktop) on macOS
```

`bun run dev` uses the **mock bridge** (`src/bridge/mock.ts` + `src/bridge/mock-desktop.ts`):
a full ~120-icon fake desktop, both mirrors, every panel, settings, and the welcome gate — no
host required. `bun run tauri:dev` launches the real Tauri app over the Rust host on Mac (with a
devhost/mock desktop). Both are working dev loops today.

## Where the pixels come from

Icon pixels are produced by the Rust **`dm-icon-core`** — compiled to **WASM** for the web
preview/bake and to **native** for the resident/background path; the frozen TS `src/icon-compositor/` is the
byte-parity ORACLE only (tree-shaken out of the bundle). **Wallpaper** compositing is Pixi in the
web (`src/compositor/`). The Rust host also decodes source images, packages ICO ladders, writes to
the shell, and backs up/restores. WYSIWYG holds because the web preview and manual bake both run the
WASM `dm-icon-core`; the native background/resident renderer is the same core's native build
(WASM↔native byte-parity).

## Bridge

`src/bridge/types.ts` is the contract truth — `BRIDGE_SCHEMA_VERSION = 8`, with the typed surface
GENERATED into `src/bridge/generated.ts` from `dm-contracts` via tauri-specta. Wallpaper (schema 6),
icons (schema 7) and calm (schema 8) route through real Rust on Mac-Tauri; the Windows runtime for
every native path is `[WINDOWS-VERIFY]`. Details: `docs/STATE.md` §Bridge state.

## Layout of `src/`

| Dir | What |
|-----|------|
| `components/` | `shell/` (module rail + layout), `panels/` (icons / wallpaper / calm / settings inspectors), `canvas/` (desktop mirror + zone editor), `common/` (primitives) |
| `stores/` | Zustand stores (`icons.ts`, `wallpaper.ts`, calm store) |
| `icon-compositor/` | frozen TS icon renderer — the byte-parity oracle (production pixels are `dm-icon-core` WASM) |
| `compositor/` | Pixi wallpaper compositor + material |
| `bridge/` | typed client, generated + hand DTOs (schema 8), mock host + mock desktop |
| `lib/` | i18n (`i18n/{en,zh-hans}.ts` — the source), icons-assemble, canvas-view, geometry, motion, zone math, utils |

## Conventions

Coral `#FF6F5E` is the only accent (blue/violet + stock cool-greys are test-banned); no dashes in
user-facing copy; files ≤ 500 lines; a bug fix ships a regression test. Full rules:
`docs/conventions/code-style.md` and the gates in `tests/banned-colors.test.ts`.
