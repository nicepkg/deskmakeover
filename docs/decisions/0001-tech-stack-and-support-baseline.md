# 0001. Technology Stack And Support Baseline

**Status:** superseded by [ADR-0019](0019-tauri-rust-replatform.md) — the .NET 10 / WPF stack
below is retired to `legacy/`; the product is now Tauri 2 + Rust. Kept as the record of the
original baseline.
**Date:** 2026-07-05

## Context

DeskMakeover needs to ship as a simple Windows app for non-technical users. The existing prototype is implemented as PowerShell scripts, but the product must not depend on PowerShell execution policy, preinstalled runtimes, or user command-line knowledge.

The app also needs to interact with Windows Shell surfaces: desktop shortcuts, `.url` files, AppX/UWP entries, Recycle Bin icons, folder `desktop.ini`, desktop layout, Explorer refresh, and shortcut overlay registry values.

## Decision

Use a native Windows desktop architecture:

- .NET 10 LTS
- WPF for the main app
- self-contained distribution
- a separate elevated helper for privileged operations
- Windows 10 and Windows 11 as the supported MVP operating systems

Windows 7 is not supported in the mainline product. If required later, it must be evaluated as a separate legacy product with reduced functionality and a different runtime strategy.

The existing scripts under `D:\shells` are behavior references and regression examples. They are not embedded or executed by the production app.

## Consequences

- The main product can use a supported modern .NET runtime and avoid near-term runtime end-of-support churn.
- The app can keep the UI process non-admin and constrain privileged operations to a small helper.
- Windows 7 users are excluded from MVP support.
- Shell behavior must be reimplemented as tested .NET modules rather than wrapped as PowerShell.

