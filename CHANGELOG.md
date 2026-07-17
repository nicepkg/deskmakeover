# Changelog

All notable changes to DeskMakeover. The format follows
[Keep a Changelog](https://keepachangelog.com/); dates are ISO-8601.

> Cut releases with `bun run release <version> --commit --tag --push` (see
> `docs/signing-setup.md`). The authoritative moving picture lives in
> `docs/STATE.md` + `docs/journal/`.

## Unreleased

Nothing yet.

## [0.1.0] - 2026-07-17

First public release. Signed NSIS installer (Authenticode, CN=Yang Jinming via
Certum + RFC-3161 timestamp; the app exe, the `dm-elevated` privileged helper,
and the uninstaller all ship signed) built and published by the self-hosted
signing runner (`.github/workflows/release.yml`).

v3 "Premium Flat" (ADR-0013), web-first. The visible UI is a React app in the system WebView
(WebView2); a Rust workspace owns everything native — `dm-icon-core` is the one pixel truth
(the same code renders the WASM live preview and bakes final icons natively), `dm-windows`
talks to the shell, `dm-operations` owns snapshot / apply / restore, `dm-resident` keeps the
look reconciled from the tray, and `dm-elevated` is the whitelisted privileged helper. The
desktop shell runs on real Windows 10/11 and `tauri build` produces a working per-user NSIS
installer (CI + release workflows live in `.github/workflows/`; Authenticode signing is an
owner-gated release blocker); the write surface is completing its on-device verification pass
(see `docs/ship-readiness.md`). The C#/WPF host that carried earlier prototypes is fully
retired (see Superseded history below).

### Icons module
- One-tap beautify over a live desktop-mirror canvas (real wallpaper + observed icon positions),
  hold-to-compare, press-to-peek, per-icon right-click overrides, undo/redo + version history.
- Curated **11-shape** catalog on the authentic iOS continuous-corner squircle geometry (Apple /
  Circle / Samsung / Tile / Teardrop / Bookmark / Lemon / Diamond / Flower / Pebble / 无), shared
  by the on-screen swatch and the bake. Colour treatments (原彩 / 黑白 / 极致单色) + shared 调色盘.
  Shortcut marks (six refined + classic arrow + none), silhouette-aware. Filters incl. Gloss.
- Per-bucket participation (`kindPolicy`: apps / folders / files — the system bucket merged
  into apps, 2026-07-16). Per-type accordion chips hover-preview live like the global axes;
  the File kind glyph folds top-right matching the File shape; the Comet arrow badge renders
  at the native classic-arrow footprint (0.28 × tile).

### Wallpaper module
- Zone editor: translucent panels painted into the wallpaper — five materials, four title styles,
  optional baked shadow, adjustable corners, grid-snap, import/export; 壁纸压暗 clarity control.
  Original wallpaper backed up for one-click return.

### Shell
- Left module rail (图标 / 壁纸 / 设置), right inspector, compact layout for narrow windows.
  Light-first theme following the system; in-app version + changelog narrative (ADR-0013).
- Resident tray fully wired (2026-07-16): every menu item responds — deep-links into
  history/settings/reset, honest toggle-precondition feedback, 撤销最近一次整理 (CAS-gated
  batch undo), privileged-scope roots + watcher on real known folders; Settings gains the
  恢复系统原始外观 row (spec 07 §13.2 confirmation) and the auto-format switch.

### Engineering
- Warm coral `#FF6F5E` is the only UI accent (blue/violet banned, test-gated); reviewed
  exemptions: OS-authentic depictions + the multicolour celebration confetti.
- WYSIWYG: preview and bake share the same `dm-icon-core` Rust renderer (WASM in the preview, native in the bake).
- No dashes in user-facing copy; files ≤ 500 lines; bug fixes ship regression tests.

---

### Superseded history (kept for the record — predates v3)

- **v1.0-prototype-parity (2026-07, ADR-0008)** — the icons-only prototype-parity rebuild
  (three shapes on a Lamé superellipse, icon-size control, a settings drawer, WPF-era pixel
  ownership). Its shapes/size/drawer/renderer decisions have since been reversed; see the ADR
  status map in `docs/STATE.md`.
- **v0.9 (2026-06)** — first internal preview: desktop scan, snapshot, one-click restore skeleton.
