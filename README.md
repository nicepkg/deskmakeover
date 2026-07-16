<div align="center">

<img src=".github/assets/logo.png" width="112" alt="DeskMakeover logo" />

# DeskMakeover

**Give your Windows desktop a one-click makeover — and restore everything, anytime.**

[![License: MIT](https://img.shields.io/badge/License-MIT-FF6F5E.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Windows-10%20%7C%2011-0067C0.svg)](#install)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB.svg)](https://v2.tauri.app/)
[![CI](https://github.com/nicepkg/deskmakeover/actions/workflows/ci.yml/badge.svg)](https://github.com/nicepkg/deskmakeover/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/nicepkg/deskmakeover?color=FF6F5E)](https://github.com/nicepkg/deskmakeover/releases)

**English** · [中文](README.zh-CN.md)

<img src=".github/assets/screenshot.jpg" width="820" alt="DeskMakeover main window" />

</div>

---

DeskMakeover (**桌面美颜**) restyles a messy Windows desktop into a clean, good-looking
one — **reversibly**. It restyles your desktop icons and paints translucent wallpaper
zones behind them, all previewed live before anything is written, and all restorable to
the exact original with one click. No PowerShell, no registry editing, no manual icon
swapping. It is built for people who want a nicer desktop, not a tutorial.

> **Status — beta.** DeskMakeover is in active development toward its first tagged release.
> The desktop shell runs on real Windows 10/11, installers are Authenticode-signed, and the
> read-only surface (scan, geometry, extraction) is verified on-device. Apply/restore paths
> are owner-supervised while the write surface completes its Windows verification pass. Expect
> rough edges and pin a version you trust.

## Why

Windows gives you a wallpaper and a grid of mismatched icons — and no safe way to make it
look designed. The "clean desktop" tricks people share are one-way: they hide icons, edit
the registry, or replace `.ico` files by hand, and undoing them is folklore. DeskMakeover
treats **reversibility as the product**: it snapshots before it changes, previews before it
applies, and keeps a visible, reliable path back to exactly how your desktop looked before.

## Features

- **One-tap beautify** — restyle every desktop icon over a live mirror of your *actual*
  desktop (real wallpaper, real icon positions). Hold to compare with the original, peek the
  before/after, override any single icon by right-click, with full undo/redo and version
  history.
- **A real icon design system** — an 11-shape catalog built on the authentic iOS
  continuous-corner *squircle* geometry (Apple · Circle · Samsung · Tile · Teardrop · Bookmark ·
  Lemon · Diamond · Flower · Pebble · none), color treatments (original / mono / duotone) with a
  shared palette, refined shortcut marks, and finish filters. Subject pixels are never
  recolored — looks differentiate through plates, silhouettes, and backgrounds.
- **Wallpaper zones** — paint translucent panels directly into the wallpaper to group icons:
  five materials, four title styles, optional baked shadow, adjustable corners, and grid-snap.
  Your original wallpaper is backed up for a one-click return.
- **清爽 (Calm Windows)** — a guided, fail-closed helper for quieting noisy system defaults.
  It never writes a tweak until that recipe is certified; until then it *teaches* you where the
  real Windows setting lives and takes you straight there.
- **WYSIWYG, by construction** — the preview pixels are the applied pixels. The same rendering
  code draws the on-screen preview and bakes the final icon at native resolution, so what you
  see is exactly what you get.
- **Restore always available** — a snapshot is taken before any change; restore is one click
  and brings back your original icons, arrows, and wallpaper.

## Install

1. Download the latest signed installer (`DeskMakeover_x.y.z_x64-setup.exe`) from the
   [**Releases**](https://github.com/nicepkg/deskmakeover/releases) page.
2. Run it. It installs per-user (no administrator prompt) and pulls the WebView2 runtime
   automatically if your system doesn't have it.
3. Launch **DeskMakeover** and beautify away — everything is previewed first and reversible.

> Requires Windows 10 (1809+) or Windows 11, x64. The installer and app `.exe` are
> Authenticode-signed; the main UI runs without administrator rights, and the few privileged
> operations go through a small, whitelisted elevated helper.

## Build from source

You need [**Bun**](https://bun.sh) ≥ 1.1 and the Rust toolchain pinned in
[`rust-toolchain.toml`](rust-toolchain.toml) (1.97.0 + the `wasm32-unknown-unknown` target,
installed automatically by `rustup`). Bun is the only JS toolchain — never `npm`/`node`.

```bash
# 1. clone + install JS deps
git clone https://github.com/nicepkg/deskmakeover.git
cd deskmakeover
bun install

# 2. run the web UI against a mock backend — works on any OS, browser + hot reload
bun run dev
bun test            # 600+ web tests

# 3. run the full desktop app (Tauri 2 + Rust host) — Windows
bun run tauri:dev   # compiles the Rust workspace, starts Vite, opens the window

# 4. produce an installer (unsigned NSIS under target/release/bundle/nsis/)
bun run tauri:build
```

Signing is a CI-only overlay (see [`docs/signing-setup.md`](docs/signing-setup.md)); a local
`tauri:build` is always unsigned and works anywhere. The full dev runbook (dev modes, the
Tauri loop, packaging) lives in [`docs/development.md`](docs/development.md).

## How it works

DeskMakeover is a **Tauri 2 + Rust** desktop app with a **React** UI rendered in the system
WebView (WebView2 on Windows). The pixels are owned by one Rust icon core:

```
React UI  ──(generated bridge, tauri-specta)──▶  Rust host
  │                                                 │
  │  live preview + design controls                ├─ dm-icon-core   one pixel truth (WASM preview + native bake)
  │  WYSIWYG canvas (Pixi wallpaper)                ├─ dm-windows     shell / registry / desktop geometry
  └─ mock backend for browser dev                   ├─ dm-operations  snapshot · apply · restore
                                                     ├─ dm-resident    background tray + reconciler
                                                     └─ dm-elevated    tiny whitelisted privileged helper
```

The bridge contract is **generated** from the `dm-contracts` crate, so the TypeScript and Rust
sides can never drift. The web half runs standalone against a mock backend, which is why most
of the UI is built and tested in a browser loop. See
[`docs/development.md`](docs/development.md) and the design specs under
[`docs/specs/`](docs/specs) for the full picture.

## Privacy & safety

- **Local-only.** No account, no upload, no telemetry, no cloud dependency — everything runs
  and stays on your machine.
- **Snapshot before change, preview before apply, restore always visible.** These are hard
  rules, not settings.
- **No admin for the main app.** Privileged operations are isolated in a small helper with a
  fixed whitelist of actions.

## Contributing

Issues and pull requests are welcome. Start with [`CONTRIBUTING.md`](CONTRIBUTING.md) for the
setup, the house rules (extreme DRY, files ≤ 500 lines, warm-coral-only accent, no dashes in
user-facing copy, a regression test with every bug fix), and the commit/PR conventions.
Security reports go through [`SECURITY.md`](SECURITY.md).

## License

[MIT](LICENSE) © 2026 [Jinming Yang](https://github.com/2214962083). Free and open source.
