# DeskMakeover Frontend (`apps/desktop/frontend`)

The visible UI of DeskMakeover — a React 19 + TypeScript + Tailwind 4 + Motion SPA. It runs in a
plain browser (with a mock bridge) on any OS today, and will be hosted by the Tauri shell after
the ADR-0019 replatform (it previously ran inside a WebView2 window; that C# host is now frozen).

> This is a subproject. The authoritative docs live at the repo root: **`docs/STATE.md`**
> (what is true / in flight) and **`docs/development.md`** (the full dev + build runbook). Read
> those first. This file is only the local quick reference.

## Run it (Bun only — never npm/node)

```bash
bun install        # first time
bun run dev        # Vite dev server (default :5173, auto-increments) + mock bridge
bun run build      # tsc -b + production bundle -> dist/
bun test           # unit tests (297 at HEAD) — includes the banned-colour + copy-law gates
bun run lint       # oxlint
```

`bun run dev` uses the **mock bridge** (`src/bridge/mock.ts` + `src/bridge/mock-desktop.ts`):
a full ~120-icon fake desktop, both mirrors, every panel, settings, and the welcome gate — no
C# host required. This is the ONLY working dev loop today (see below).

## Where the pixels come from

The web renders the real preview AND the bake master itself: **icons** via a CPU TypeScript
compositor + Worker (`src/icon-compositor/`), **wallpaper** via Pixi (`src/compositor/`). C#
only decodes source images, packages ICO ladders, writes to the shell, and backs up/restores.
There is no live SharedBuffer pixel stream. WYSIWYG still holds: what the preview paints is what
the bake writes, because the bake is the same web code at native resolution.

## Bridge

`src/bridge/types.ts` is the contract truth — `BRIDGE_SCHEMA_VERSION = 3`. ⚠️ The C# host is
still schema 1, so the **native host (Modes B/C) is NOT wired yet** — wiring it is F8 (Windows).
Until then only the browser + mock loop runs. Details: `docs/STATE.md` §Bridge state.

## Layout of `src/`

| Dir | What |
|-----|------|
| `components/` | `shell/` (module rail + layout), `panels/` (icons / wallpaper / settings inspectors), `canvas/` (desktop mirror + zone editor), `common/` (primitives) |
| `stores/` | Zustand stores (`icons.ts`, `wallpaper.ts`) |
| `icon-compositor/` | CPU icon renderer: shapes, colour math, filters, marks, segment (极致单色) |
| `compositor/` | Pixi wallpaper compositor + material |
| `bridge/` | RPC client, DTO types (schema 3), mock host + mock desktop |
| `lib/` | i18n (resx-generated), canvas-view, geometry, motion, zone math, utils |

## Conventions

Coral `#FF6F5E` is the only accent (blue/violet + stock cool-greys are test-banned); no dashes in
user-facing copy; files ≤ 500 lines; a bug fix ships a regression test. Full rules:
`docs/conventions/code-style.md` and the gates in `tests/banned-colors.test.ts`.
