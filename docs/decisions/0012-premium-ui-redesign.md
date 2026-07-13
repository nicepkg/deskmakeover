# ADR-0012: Premium UI redesign — retire the prototype as law, adopt a macOS-Settings visual language

**Status:** superseded by [ADR-0013](0013-v3-flat-light-redesign.md) — v3 "Premium Flat" replaces
this ADR's dark-default + Segoe visual language with light-first + bundled type. The load-bearing
decision here ("the v2 prototype is no longer the binding UI law") STANDS; only the visual language
was replaced.
**Date:** 2026-07-08
**Amends:** ADR-0008 (the `桌面美颜 v2.dc.html` prototype is no longer the binding
UI contract), spec 02 (visual language — rewritten), spec 03 (settings + IA),
spec 04 (wallpaper interaction model). The engine, the WYSIWYG law (ADR-0011 §4),
the coral-only accent rule, and the owner-supervised bake/apply gates are
**unchanged**.

## Context

- The WebView2 + React replatform (ADR-0011) is complete and the UI is faithful
  to the prototype-derived spec 02 — yet the owner's verdict on the result is
  still "勉强可用，谈不上好用" (barely usable, not premium). The target audience is
  **aesthetically demanding power users with strong customization needs**; the
  current UI clears neither bar.
- Four isolated design seats (PM · UX/Norman-Nielsen · UI/visual · Interaction)
  reviewed the shipped code. Their disposed findings are recorded in
  `docs/reviews/2026-07-08-ui-premium-panel.md`. The verdict was unanimous on the
  root causes: **the problem is *how it works and looks*, not missing features.**
- The visual language in spec 02 was **transcribed from a prototype** (ADR-0008)
  and made binding ("the prototype wins every conflict"). Faithfully porting it
  reproduced its ceiling. A premium result requires *superseding* the prototype,
  not matching it.
- Nothing is released; there is no compat surface and no version history to honour
  (the "v1.0/v1.1" narrative in code and changelog is premature and is removed).

## Owner decisions (2026-07-08, via the design-panel interview)

1. **Aesthetic north star — macOS System Settings.** From-calm, grouped inset
   cards, generous whitespace, large radii, a disciplined type ladder, material
   depth. Not the dense "precision tool" (Raycast/Linear) direction, not the
   expressive/creative direction. Expressed within the hard invariants below
   (dark stays the default theme; coral stays the only accent).
2. **Customization is visible by default.** Presets remain a one-tap fast path at
   the top; the customization axes are exposed (open/persisted) with an
   at-a-glance current-values summary — depth is *seen*, not hunted. (Serves the
   power-customizer persona; corrects spec 01's restraint-first assumption.)
3. **Wallpaper scope = make the existing capability excellent; add no new
   features.** 分区 + 清晰度 + 标题 stay; every interaction and visual defect is
   fixed to a premium bar. (Owner's earlier "Basic 1.0, complexity capped" is
   lifted only on *quality*, not scope.)
4. **The zone editor stays a hand-rolled DOM overlay** (no third-party
   transform/interaction library). Cell-grid snapping is domain logic a generic
   library would fight; the hand overlay aligns exactly with the WYSIWYG compose
   buffer. Invest the effort in feel (hit targets, live snapping, snap pulse,
   cursor semantics), not in a dependency.

## Decision

### D1 — The prototype is retired as binding law; spec docs are the source of truth.

`docs/references/prototype/桌面美颜 v2.dc.html` becomes a *historical reference*,
not a contract. Where spec 02/03/04 and the prototype disagree, **the specs win.**
The durable owner rule "the prototype wins every conflict" is struck from STATE.md.

### D2 — New visual language: "Quiet Material" (spec 02 rewritten).

Personality: **从容 · 分组 · 材质 · 克制发光 · 精确**. macOS System Settings is the
reference for composition; the product's warmth and coral accent are the identity.

- **Type ladder — a real scale, one role per step:** `11 caption · 13 body/label ·
  15 card-title · 19 section · 26 page-display`. Every 0.5px increment and the
  9.5/10.5/11.5/12.5 clutter are deleted. Body vs label differentiate by
  weight/colour (macOS idiom), not by a 0.5px size delta.
- **Separation by light, not lines** (the app's own stated law, finally honoured):
  an elevation system (`--elev-1` cards, `--elev-2` popovers) + a widened
  `--bg → --raised` luminance step. Hairlines survive only *inside* grouped cards
  (macOS inset-list row dividers), never as the primary separator between surfaces.
- **Grouped inset cards** are the primary layout unit (settings, panels): a raised
  card containing hairline-separated rows. Container radius rises to the macOS
  register (cards ≈16px, controls ≈10–12px, chips ≈9px).
- **8px soft spacing grid** ({4,8,12,16,24}); 2px survives only as a deliberate
  optical nudge.
- **Chips carry live previews** (already speced, now enforced): shape chips a 14px
  live clip swatch, colour chips a 10px filled dot, mark chips a 22px live render.
  This restructures presentation only — the capability set is untouched.
- **App chrome vs simulated OS are tonally distinct.** Floating canvas controls use
  warm app tokens (`--glass` ≈ raised@72% + `--glass-ink` = t1); the white frosted
  glass is reserved exclusively for the decorative Win11 taskbar so "app" and
  "mirrored desktop" read as different layers. Stage colours become named tokens
  (`--canvas-stage`) — no more `bg-[#0E0E10]` magic literals.
- **Coral stays the only accent; blue/violet stay banned; dark stays default;**
  light is brought to the same premium bar (macOS Settings is beautiful in both).

### D3 — Wallpaper interaction model (spec 04 §3 rewritten to the premium bar).

- **Explicit edit scope.** Style/fill/opacity/corner controls act on the
  **selected** zone; the 分区样式 accordion shows a persistent `正在编辑：<名称>`
  header. Editing all zones requires an explicit **应用到全部** action — never the
  silent "nothing selected → repaint every zone" behaviour. The zone list shows a
  per-zone style dot; the no-selection state reports "多个分区" honestly instead of
  defaulting the readout to 磨砂白.
- **Discoverable creation.** The empty state renders the speced centered dashed
  drop-frame + 「拖一个框，把桌面分成区」 + an inline **用推荐布局** button; the
  interaction host wears `cursor-crosshair`. The one-shot coach mark
  (`wallpaperCoachShown`, already authored) is wired to first module entry.
- **One canvas navigation model.** The paper canvas gains Ctrl+wheel-zoom-at-pointer
  and pan, identical to the icons canvas (spec 04 §3 already promised "manual zoom
  still works"). Left-drag on empty canvas creates a zone (crosshair-signified);
  pan is space/middle-drag, matching muscle memory across modules.
- **Reversibility.** A session undo (Ctrl+Z) over `look` snapshots covers zone
  create/move/resize/delete/restyle; **用推荐布局** confirms before replacing
  existing zones. `Backspace` is dropped as a delete key (Delete only).
- **Direct manipulation feel.** The create rubber-band live-snaps to the grid as it
  is dragged (draws the snapped rect, not the raw one); on a snap-cell change the
  zone pulses scale 1.02→1.0 in 80ms (reduced-motion: none). Resize handles get
  enlarged transparent hit boxes. Hold-to-compare hides all zone chrome (outline,
  handles, ghosts) so the true before-state is visible.
- **On-canvas rename.** Double-click a zone's on-canvas title band → inline edit
  (the panel list input stays as a secondary path); the title is edited where it is
  seen (closes the execution gulf).

### D4 — Settings page = the macOS-Settings showcase (spec 03 §3 rewritten).

Grouped inset cards, one radius, a consistent leading icon badge per card,
equal-weight full-width cards (no lone-toggle-in-a-2-col-grid). `本地数据` exposes
three *distinct* actions (导出快照 actually exports, 前后对比图 saves, 打开文件夹) —
the duplicate `openDataFolder` binding and the retired `Drawer_Save` string are
removed. Trust indicators render as real chips. The version chip and the changelog
section are removed from the UI until there is a release to describe.

### D5 — No version narrative pre-release.

`Directory.Build.props` version returns to a pre-release marker; `changelog.json`
is no longer surfaced in the UI; the title-bar version chip is removed. A version
story returns only at the first real release.

## Consequences

- Spec 02 is rewritten; spec 03 §3 and spec 04 §2–§3 are rewritten; ADR-0008's
  "binding prototype" status is downgraded to reference. STATE.md's durable
  "prototype wins" rule is removed and replaced with "specs are the source of
  truth; coral-only + dark-default + WYSIWYG remain inviolable."
- The change is UI-layer only: `src/DeskMakeover.Web`. The C# engine, bridge,
  bake/apply, snapshot/restore, and the owner-supervised gates are untouched.
  Zone-title rasterisation stays host-side (WYSIWYG law).
- New shared web primitives (grouped `Card`, live-preview `Chip` variants, warm
  `Glass`), a rebuilt `index.css` token layer, and a rebuilt wallpaper zone editor.
  Files stay ≤500 lines; `bun test` state/token tests and the banned-colour grep
  gate expand to cover the new system.
- Execution: plan `docs/plans/2026-07-08-premium-ui-redesign.md`, phased in the
  panel's quality-per-effort order (wallpaper interaction → wallpaper visual →
  settings → customization IA → icons/global), each phase gated by an adversarial
  cross-vendor review + green build/tests before the next.
- The real desktop bake and the wallpaper apply remain owner-supervised, click-only
  (ADR-0011 §7 unchanged); this redesign never auto-triggers them.
