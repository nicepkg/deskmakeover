# Changelog

All notable changes to DeskMakeover. Dates are ISO-8601.

## v1.0.0 — 2026-07 (prototype-parity rebuild, ADR-0008)

The first public release: a one-screen, fully-reversible desktop-icon beautifier,
rebuilt to completely replicate the owner's interactive prototype.

### Added

- **Control panel** — hero + one-tap CTA (5-state machine), four 风格 presets with
  live mini-previews, a four-row 自定义 accordion (外形 / 配色 / 快捷方式标识 / 图标大小)
  with live shape swatches and seven live shortcut-mark chips.
- **Three shapes** — 苹果 (quintic Lamé superellipse) · 纯圆 · 三星 (official One UI
  adaptive-icon path). One geometry source drives both the on-screen preview and the
  baked `.ico`.
- **Three colour treatments** — 原彩 / 黑白 (contrast-stretched) / 单色 (tinted), with a
  shared 调色盘 (SV field + hue + hex + screen eyedropper + wallpaper/quick swatches).
- **Seven shortcut marks** — 玻璃箭头 / 双层卡片 / 幽灵叠影 / 缎光角 / 珐琅光弧 / 卷角 /
  细描边, plus 经典箭头 and 无标识; contrast-adaptive, baked into each per-icon `.ico`.
- **Desktop-mirror canvas** — the real wallpaper + real desktop icons in column order,
  hold-to-compare, press-to-peek, and per-icon right-click overrides (保留原样 / 单独配色).
- **图标大小** — 小 / 中 / 大, applied live to the real desktop via `IFolderView2`.
- **Version history** — the last 10 applied looks, 上一版, and 回到最初.
- **Settings drawer · overflow · About + changelog** — theme (深色/浅色/跟随系统, live),
  auto-beautify new icons, snapshot/comparison export, GitHub + author links.
- **Reversible foundation** — invisible snapshot → journaled apply → one-click zero-residue
  restore; a single batched UAC through a whitelisted elevated helper; local-only, no
  account, no telemetry.

### Engineering

- Warm coral `#FF6F5E` is the only accent; blue/violet is banned everywhere.
- Preview == desktop: one `TileRenderer` renders both.
- 208 automated tests; `dotnet build` is 0-warnings.

## v0.9 — 2026-06

First internal preview: desktop scan, restore snapshot, and one-click restore skeleton.
