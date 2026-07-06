# DeskMakeover

DeskMakeover is the English product name for **桌面美颜** (renamed by ADR-0002).

> 一键美颜你的 Windows 桌面，随时完整还原。
> Give your Windows desktop a one-click makeover. Restore everything anytime.

A local Windows 10/11 desktop makeover app focused on reversible desktop-icon styling, designed for non-technical users who want a cleaner desktop without PowerShell, registry editing, or manual icon replacement.

## Current Status

Foundation built and tested; the UI is being rebuilt to match the owner's interactive prototype (`docs/references/prototype/`), the binding v1.0 contract (ADR-0008). See [docs/STATE.md](docs/STATE.md) for the current checkpoint and [docs/plans/2026-07-06-v1-prototype-parity.md](docs/plans/2026-07-06-v1-prototype-parity.md) for the rebuild plan.

## Principles

- Preview before applying.
- Snapshot before changing.
- Restore must stay visible and reliable.
- Main UI runs without administrator permission.
- Privileged operations go through a small whitelisted helper.
- MVP is local-only: no account, upload, telemetry, or cloud dependency.

## Development

Use the local SDK under `.dotnet/` when present:

```powershell
$env:DOTNET_ROOT=(Resolve-Path '.\.dotnet').Path
$env:PATH="$env:DOTNET_ROOT;$env:PATH"
dotnet test DeskMakeover.slnx
dotnet build DeskMakeover.slnx
```

Create a self-contained Windows x64 publish folder:

```powershell
node scripts/publish-win.mjs
```

The output is written to `artifacts/win-x64/DeskMakeover/`.
