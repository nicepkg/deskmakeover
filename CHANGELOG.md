# Changelog

All notable changes to DeskMakeover. Dates are ISO-8601.

> **Nothing has been released yet.** `Directory.Build.props` and the Web package are both
> `0.0.0`; the owner names the first version at release time. Until then the entry below is
> `Unreleased`, and the authoritative moving picture lives in `docs/STATE.md` + `docs/journal/`.

## Unreleased

v3 "Premium Flat" (ADR-0013), web-first. The visible UI is a WebView2 + React app that renders
its own preview and bake pixels (CPU TypeScript icons, Pixi wallpaper); C# keeps window / source
decode / ICO packaging / shell write / backup-restore. Native host integration (bridge schema 3)
and release packaging are still pending (tracked as "F8" in STATE.md).

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

### Engineering
- Warm coral `#FF6F5E` is the only UI accent (blue/violet banned, test-gated); reviewed
  exemptions: OS-authentic depictions + the multicolour celebration confetti.
- WYSIWYG: the preview pixels are the bake pixels (same web code at native resolution).
- No dashes in user-facing copy; files ≤ 500 lines; bug fixes ship regression tests.

---

### Superseded history (kept for the record — predates v3)

- **v1.0-prototype-parity (2026-07, ADR-0008)** — the icons-only prototype-parity rebuild
  (three shapes on a Lamé superellipse, icon-size control, a settings drawer, WPF-era pixel
  ownership). Its shapes/size/drawer/renderer decisions have since been reversed; see the ADR
  status map in `docs/STATE.md`.
- **v0.9 (2026-06)** — first internal preview: desktop scan, snapshot, one-click restore skeleton.
