# Round 2 — material/title style sets + import/export + empty state (2026-07-09)

Owner asks: more material finishes beyond frost; more title styles beyond the
pill; import a custom wallpaper; export the composed PNG for sharing; plan the
buttons in limited space; kill the giant dashed empty-state frame flashing on
refresh. Two isolated seats consulted (visual designer; UI/UX engineer). All
adopted and BUILT the same day (browser-verified on the Mac loop).

## Material set (designer; material.ts is the recipe truth)

ONE adaptive base (per-zone OKLCH sampling + tone auto w/ hysteresis) feeding
five finishes: 磨砂玻璃 Frost (default) · 晨光玻璃 Luminous (vertical gradient
+ accent inner glow, radius 24) · 实色卡片 Solid (α.94/.92, no frost — the
video-wallpaper/weak-GPU baseline) · 柔光晕影 Halo (feathered edge σ=cell×0.45,
no contour/highlight, accent-leaning hue) · 描边卡片 Outline (α.05 body +
2px accent ring). Shared 投影 shadow finish (offsetY cell×0.06, blur cell×0.14,
α .16/.28) for real bodies only. Blur-less tiers defined per finish.

## Title set (designer; title-chip.ts)

胶囊标签 Chip (default) · 净色标题 Bare (accent dot + baked soft halo, no
backing) · 折角页签 Tab (folder tab riding the panel top edge; reserves row 1
when flush) · 顶栏标题 Bar (full-width in-panel header + accent divider —
horizontal, never a left bar — ALWAYS reserves row 1). Combo matrix enforced:
Halo/Outline allow Chip/Bare only (`allowedTitleStyles`). Material switch
applies the designer pairing (Frost/Luminous→Chip, Solid→Tab, Halo/Outline→
Bare) + per-material radius default; user overrides stay available.

## IA (engineer)

- Source-in lives in the inspector's top SESSION BAR: quiet ImageUp entry by
  default; imported state = thumbnail chip 「正在设计导入的图片」 + ✕ cancel.
  Only the non-default state gets labelled.
- Result-out lives in the CTA dock's secondary link slot: [换回我的壁纸]
  [导出图片] as quiet IconAction peers; export gated on dirty; CTA untouched.
- Import paths: session-bar entry, OS drag-drop onto the canvas (coral ring +
  「松手，用这张图设计」 hint over the wallpaper rect), empty-state link.
  Import is client-side only (source never crosses the bridge; apply bakes it).
  No success toast — the wallpaper visibly changes. Failure toasts only.
- Export = compositor.bake() → 桌面壁纸_YYYY-MM-DD.png (browser download now;
  host native Save dialog = F8 `wallpaper.exportPng`). Never touches desktop.
- Empty state redesigned: glass card bound to the WALLPAPER rect (never the
  letterbox) showing the preset gallery inline on the user's own wallpaper +
  draw hint + import link; gated on compositor `ready` (kills the refresh
  flash of the old giant dashed frame); exits 0.18s on first zone.
- Editing card re-org absorbs +2 style rows and gets SHORTER: 正在编辑(材质)
  → 强调色 → 标题样式(+emoji/size) → 高级 fold (tone/opacity/corner/shadow/
  font) → 应用到全部.

## Verification (browser, Mac loop)

Four materials rendered simultaneously on one desktop (evidence 11); Bar title
band + divider + row-1 ghost reservation (12); dark import flips every zone's
adaptive tone + ghost ink (13, 14); empty-state gallery on the imported source
(14); combo matrix trims title options for Outline/Halo live; export button
appears when dirty. 220 bun tests green.

## F8 additions

`wallpaper.exportPng` (native Save dialog), `wallpaper.setImportedSource`
(persist the imported source across launches; session-only is the accepted
degrade), host decode fallback for formats the browser can't read (WIC).
