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

The web UI runs on **any OS** with Bun; the C# engine/host needs **Windows**. Full runbook
(three dev modes, the .NET-SDK gotcha, packaging): [docs/development.md](docs/development.md).

```bash
# web UI (React SPA) — any OS, from src/DeskMakeover.Web — Bun only, never npm/node
bun install
bun run dev        # browser + mock loop (the only working loop today; native host = F8)
bun test           # 297 tests
```

```powershell
# C# engine/host — WINDOWS ONLY, use the repo-local SDK under .dotnet/
.\.dotnet\dotnet.exe build
.\.dotnet\dotnet.exe test
```

⚠️ **Release packaging is unverified** — `scripts/dev/publish.ps1` / `bun scripts/publish-win.mjs`
exist but no shippable artifact is proven yet (see [docs/development.md](docs/development.md) §5).
