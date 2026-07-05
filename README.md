# DeskMakeover

DeskMakeover is the English product name for **桌面整容大师**.

> Give your Windows desktop a one-click makeover. Restore everything anytime.

The MVP is a local Windows 10/11 desktop makeover app focused on reversible desktop icon styling. It is designed for non-technical users who want a cleaner desktop without PowerShell, registry editing, or manual icon replacement.

## Current Status

Design approved. Implementation is starting from the documented architecture in [docs/specs/01-product-architecture.md](docs/specs/01-product-architecture.md).

## Principles

- Preview before applying.
- Snapshot before changing.
- Restore must stay visible and reliable.
- Main UI runs without administrator permission.
- Privileged operations go through a small whitelisted helper.
- MVP is local-only: no account, upload, telemetry, or cloud dependency.

