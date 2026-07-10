# DeskMakeover

DeskMakeover is the English product name for **桌面美颜** (renamed by ADR-0002).

> 一键美颜你的 Windows 桌面，随时完整还原。
> Give your Windows desktop a one-click makeover. Restore everything anytime.

A local Windows 10/11 desktop makeover app: reversible **icon styling** and **wallpaper zones**, for non-technical users who want a cleaner desktop without PowerShell, registry editing, or manual icon replacement.

## Current Status

**v3 "Premium Flat"** (ADR-0013). The UI is a WebView2 + React app; the web half is built and green (297 web tests) and iterated entirely in a browser + mock loop. Native host integration and release packaging are **not done yet** (tracked as "F8"). See **[docs/STATE.md](docs/STATE.md)** for the authoritative checkpoint (including known doc drift and open decisions) and [docs/development.md](docs/development.md) for the dev + build runbook.

## Principles

- Preview before applying.
- Snapshot before changing.
- Restore must stay visible and reliable.
- Main UI runs without administrator permission.
- Privileged operations go through a small whitelisted helper.
- MVP is local-only: no account, upload, telemetry, or cloud dependency.

## Development

The web UI runs on **any OS** with Bun; the Tauri 2 desktop shell builds on macOS today
(ADR-0019). Full runbook (dev modes, the Tauri loop, packaging):
[docs/development.md](docs/development.md).

```bash
# web UI (React SPA) — any OS, from the repo root — Bun only, never npm/node
bun install
bun run dev        # browser + mock loop
bun test
```

```bash
# desktop shell (Tauri 2 + Rust) — macOS-buildable — from the repo root
bun install                 # installs @tauri-apps/cli
bun run tauri:dev           # compiles the Rust host, starts Vite, opens the window
```

The former .NET/WPF engine + WebView2 host is a **frozen oracle** under `legacy/`
(ADR-0019: being ported to Rust, deleted at migration phase M8). Release packaging
(Tauri NSIS + elevated helper) is phase M8 and not yet proven —
see [docs/development.md](docs/development.md) §5.
