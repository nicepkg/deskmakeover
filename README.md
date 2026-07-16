<div align="center">

<img src=".github/assets/logo.png" width="96" alt="DeskMakeover logo" />

# DeskMakeover

<a href="README.zh-CN.md"><img src=".github/assets/name-zh-pill.png" height="22" alt="桌面美颜" /></a>

**Make your Windows desktop beautiful in one click. Put it all back in one click.**

[![Status](https://img.shields.io/badge/beta-v0.1.0-FF6F5E?labelColor=2f363d)](https://github.com/nicepkg/deskmakeover/releases)
[![Windows](https://img.shields.io/badge/Windows-10%20%C2%B7%2011-464f58?labelColor=2f363d)](#install)
[![License](https://img.shields.io/badge/License-MIT-464f58?labelColor=2f363d)](LICENSE)
[![Tauri](https://img.shields.io/badge/Built%20with-Tauri%202-464f58?labelColor=2f363d)](https://v2.tauri.app/)

**English** · [中文](README.zh-CN.md)

<br/>

<a href="https://github.com/nicepkg/deskmakeover/releases">
  <img src=".github/assets/hero-beforeafter.svg" width="880" alt="A cluttered default Windows desktop is beautified into clean squircle icons with one click, then fully restored" />
</a>

</div>

<br/>

DeskMakeover restyles a cluttered Windows desktop into one that looks clean and deliberate, then lets you restore the exact original whenever you want. It restyles your desktop icons and paints translucent zones into the wallpaper behind them, previewing every pixel live before it writes a single file. No PowerShell, no registry edits, no swapping icons by hand.

> **Status: beta.** The desktop shell runs on real Windows 10/11; the write surface is completing its on-device verification pass, and the first public installer is being prepared. Until it lands, [build from source](#for-developers). Details in the [FAQ](#faq).

## Reversible by design

- **It snapshots before it touches anything.** Every apply is preceded by a snapshot of your current icons, arrows, and wallpaper. Restore is one click and brings back the exact original.
- **You approve it before it happens.** The live preview is drawn by the same code that writes the final pixels, so what you see is what gets applied.
- **Local only.** No account, no upload, no telemetry. It reads and writes on your machine and nowhere else.
- **No administrator rights for the app.** The main app runs as your normal user. The few privileged steps go through a small helper that is limited to a fixed list of actions.
- **Signed releases.** Every public installer ships Authenticode-signed, so Windows can confirm it was not tampered with after we built it. Local builds stay unsigned.

<div align="center"><br/><img src=".github/assets/rule-sparkle.svg" width="80" alt="" /><br/><br/></div>

## Nine looks, one click

The same folder, dressed nine ways — every preset was tuned by hand on a real desktop:

<div align="center">
<img src=".github/assets/specimen-nine-styles.webp" width="880" alt="One folder icon rendered in all nine DeskMakeover styles" />
<br/>
<sub>Squircle · Porthole · Pixel Era · Creekstone · Scrapbook · Gleam · Die-Cut · Blueprint · Glaze</sub>
</div>

<br/>

<table>
<tr>
  <td align="center"><img src=".github/assets/preset-squircle.webp" width="250" alt="Squircle preset" /><br/><b>Squircle</b><br/><sub>continuous corners</sub></td>
  <td align="center"><img src=".github/assets/preset-blueprint.webp" width="250" alt="Blueprint preset" /><br/><b>Blueprint</b><br/><sub>monochrome ink</sub></td>
  <td align="center"><img src=".github/assets/preset-pixel-era.webp" width="250" alt="Pixel Era preset" /><br/><b>Pixel Era</b><br/><sub>8-bit afternoon</sub></td>
</tr>
<tr>
  <td align="center"><img src=".github/assets/preset-gleam.webp" width="250" alt="Gleam preset" /><br/><b>Gleam</b><br/><sub>brushed with light</sub></td>
  <td align="center"><img src=".github/assets/preset-glaze.webp" width="250" alt="Glaze preset" /><br/><b>Glaze</b><br/><sub>cool porcelain</sub></td>
  <td align="center"><img src=".github/assets/preset-die-cut.webp" width="250" alt="Die-Cut preset" /><br/><b>Die-Cut</b><br/><sub>sticker outlines</sub></td>
</tr>
<tr>
  <td align="center"><img src=".github/assets/preset-porthole.webp" width="250" alt="Porthole preset" /><br/><b>Porthole</b><br/><sub>clean circles</sub></td>
  <td align="center"><img src=".github/assets/preset-scrapbook.webp" width="250" alt="Scrapbook preset" /><br/><b>Scrapbook</b><br/><sub>pasted by hand</sub></td>
  <td align="center"><img src=".github/assets/preset-creekstone.webp" width="250" alt="Creekstone preset" /><br/><b>Creekstone</b><br/><sub>river-worn stone</sub></td>
</tr>
</table>

Presets are starting points, not cages. Underneath sits a real icon design system: an 11-shape catalog built on authentic iOS continuous-corner squircle geometry, color treatments with a shared palette, refined shortcut marks, finish filters, and per-type overrides so folders and files can keep shapes of their own. Subject pixels are never recolored; looks differentiate through plates, silhouettes, and backgrounds.

## What it does

- **One-tap beautify** — restyle every desktop icon over a live mirror of your *actual* desktop: real wallpaper, real icon positions. Hold to compare with the original, override any single icon by right-click, undo and redo with full version history.
- **Wallpaper zones** — paint translucent panels directly into the wallpaper to group your icons: five materials, four title styles, optional baked shadow, grid-snap. The original wallpaper is backed up for a one-click return.
- **Calm Windows (清爽)** — a guided, fail-closed helper for quieting noisy system defaults. It never writes a tweak until that recipe is certified; until then it teaches you where the real Windows setting lives and takes you straight there.
- **Restore, always** — a snapshot precedes every change; one click brings back your original icons, arrows, and wallpaper, exactly.

<div align="center"><br/><img src=".github/assets/rule-sparkle.svg" width="80" alt="" /><br/><br/></div>

## Inside the studio

<div align="center">
<img src=".github/assets/app-studio.webp" width="880" alt="The DeskMakeover window: live desktop mirror on the left, design controls on the right" />
<br/>
<sub>Pick a look on the right, watch your real desktop restyle live on the left, hit Beautify when it's perfect.</sub>
</div>

## Install

1. Open the [Releases](https://github.com/nicepkg/deskmakeover/releases) page and download the latest `DeskMakeover_x.y.z_x64-setup.exe`. (The first public release is on its way; until it appears there, [build from source](#for-developers).)
2. Double-click it. DeskMakeover installs just for your user, so there is no administrator prompt, and it adds the WebView2 runtime automatically if your PC doesn't already have it.
3. Open DeskMakeover and start. Nothing on your desktop changes until you preview it first, and every change is one click to undo.

Works on Windows 10 (version 1809 or newer) and Windows 11, 64-bit.

> **If Windows shows a blue "Windows protected your PC" screen:** that is SmartScreen being cautious about a publisher it hasn't seen often yet, not a virus alert. The installer is Authenticode-signed; you can confirm this by right-clicking the `.exe`, choosing **Properties → Digital Signatures**, and checking the signer. To continue, click **More info**, then **Run anyway**. As more people install the signed builds, Windows stops showing this prompt.

## FAQ

**Will it slow down my PC?**
The heavy work happens once, when you apply a look. After that a small tray helper watches for Windows resetting your desktop (for example when Explorer restarts) and reapplies your look. It is a reconciler, not a constant background renderer.

**Will my desktop survive a reboot or a Windows update?**
Your styled icons are baked to real image files, so they persist across reboots, and the tray helper reconciles your look back if Windows regenerates its icon cache. A major Windows feature update can reset some system defaults; if that happens, reapply with one click, or restore the original.

**How do I go back to normal?**
One click. DeskMakeover snapshots your original icons, arrows, and wallpaper before any change, and Restore returns them exactly. You can also undo and redo individual steps, and Settings keeps a full "restore original appearance" action.

**Does it edit the registry?**
You never edit the registry by hand, and your original icon files are never modified. Styled icons are new image files kept in the app's own data folder; folders are styled through the standard `desktop.ini` mechanism. The few shell settings the app does change are snapshotted first, so Restore puts them back.

**Does it work on Windows 10?**
Yes, on Windows 10 version 1809 or newer, 64-bit, and on Windows 11.

## How it works

DeskMakeover is a **Tauri 2 + Rust** desktop app with a **React** UI rendered in the system WebView (WebView2 on Windows). The pixels are owned by one Rust icon core:

```
React UI  ──(generated bridge, tauri-specta)──▶  Rust host
  │                                                 │
  │  live preview + design controls                ├─ dm-icon-core   one pixel truth (WASM preview + native bake)
  │  WYSIWYG canvas (Pixi wallpaper)                ├─ dm-windows     shell / registry / desktop geometry
  └─ mock backend for browser dev                   ├─ dm-operations  snapshot · apply · restore
                                                     ├─ dm-resident    background tray + reconciler
                                                     └─ dm-elevated    tiny whitelisted privileged helper
```

The bridge contract is generated from the `dm-contracts` crate and locked by a bindings test in CI, so the TypeScript and Rust sides stay in sync. The web half runs standalone against a mock backend, which is why most of the UI is built and tested in a browser loop. The full picture lives in [`docs/development.md`](docs/development.md) and the design specs under [`docs/specs/`](docs/specs).

## For developers

[![CI](https://github.com/nicepkg/deskmakeover/actions/workflows/ci.yml/badge.svg)](https://github.com/nicepkg/deskmakeover/actions/workflows/ci.yml)

You need [**Bun**](https://bun.sh) ≥ 1.1 and the Rust toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml) (installed automatically by `rustup`). Bun is the only JS toolchain here.

```bash
bun install
bun run dev          # web UI against a mock backend — any OS, browser + hot reload
bun run tauri:dev    # full desktop app (Windows) — compiles the Rust host, opens the window
```

The full dev runbook (dev modes, tests, the Tauri loop, packaging, signing) lives in [`docs/development.md`](docs/development.md); local builds are always unsigned and work anywhere.

## Contributing

Contributions to the React UI, the Rust core, documentation, localization, and Windows compatibility testing are all welcome — most of the UI can be built and tested in a browser without a Windows box. Start with [`CONTRIBUTING.md`](CONTRIBUTING.md) for setup and the house rules, and check the [good first issues](https://github.com/nicepkg/deskmakeover/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22). Security reports go through [`SECURITY.md`](SECURITY.md).

## License

[MIT](LICENSE) © 2026 [Jinming Yang](https://github.com/2214962083). Free and open source.
