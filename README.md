# DeskMakeover

DeskMakeover is the English product name for **桌面整容大师**.

> Give your Windows desktop a one-click makeover. Restore everything anytime.

The MVP is a local Windows 10/11 desktop makeover app focused on reversible desktop icon styling. It is designed for non-technical users who want a cleaner desktop without PowerShell, registry editing, or manual icon replacement.

## Current Status

MVP foundation is under active implementation. See [docs/STATE.md](docs/STATE.md) for the current engineering checkpoint.

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
