# Plan — Premium UI redesign (2026-07-08)

Governing docs: **ADR-0012**, **spec 02 v2** (visual language), **spec 03 §3/§3.1**
(settings + IA), **spec 04 §2/§3.5** (wallpaper interaction). Panel evidence:
`docs/reviews/2026-07-08-ui-premium-panel.md`.

Scope: `src/DeskMakeover.Web` only. Engine, bridge, bake/apply, snapshot/restore,
owner-supervised gates — untouched. Files ≤500 lines. Bun-only. Accent coral-only;
dark default; WYSIWYG law intact. **The real bake and wallpaper apply are never
auto-triggered** (owner-supervised gate stays).

Baseline: 277 dotnet + 63 bun green; the WebView2/React app is live (ADR-0011).

Order = the panel's quality-per-effort sequence, but the **design-system foundation
(P1) lands first** because every later phase consumes it. Each phase ends with a
green build + `bun test` + the banned-colour grep gate, and an adversarial
cross-vendor review before the next phase starts.

---

## P1 — Design-system foundation  (task #48)

The systemic visual layer every module reuses. No behaviour change; pure look.

- `src/index.css`:
  - Rewrite tokens to spec 02 v2: widen `--bg → --raised` (dark `#1A1A1C → #2A2A2E`),
    add `--raised-hov`, `--chip`, the elevation tokens `--elev-1/--elev-2/--elev-cta`,
    the canvas tokens `--canvas-stage`/`--glass`/`--glass-ink`/`--glass-ring`, light
    theme parity. Delete unused washes; keep coral/teal/amber.
  - Map the type ladder to utilities/tokens (11/13/15/19/26) — no 0.5px values remain.
- Primitives:
  - `components/common/card.tsx` — NEW grouped inset `Card` (raised + `--elev-1`,
    radius 16, optional icon badge header, hairline-separated `CardRow`s).
  - `components/common/chip.tsx` — add live-preview slots: `swatch` (14px clip),
    `dot` (10px), `mark` (22px live render); keep the text-only default.
  - `components/common/shape-swatch.tsx` — NEW 14px live `clipFor` preview (reuses
    `lib/geometry` squircle path) for shape chips.
  - Refresh `cta-button.tsx` (elevation token), `accordion-axis.tsx` (44px, tokens),
    `segmented.tsx`, `toggle-switch.tsx`, `color-picker.tsx` to the new tokens/radii.
  - Canvas glass helper: replace the per-file `GLASS`/`bg-[#0E0E10]` literals with the
    `--glass`/`--canvas-stage` tokens (shared const in `lib/`).
- Tests: extend `tests/banned-colors.test.ts` (no new cool-gray literals in app
  chrome; taskbar allowlist stays); add `tests/type-scale.test.ts` (no banned font
  sizes in `.tsx`); `tests/tokens.test.ts` (elevation/glass tokens defined both themes).
- **Gate:** `bun run build` + `bun test` green; grep gate empty; component gallery
  (`?debug=components`) renders every primitive in dark+light.

## P2 — Wallpaper interaction rebuild  (task #49) — the loudest complaint

Spec 04 §3.5. Mostly logic + small components; converts "barely usable" → "usable".

- `stores/wallpaper.ts`: add an undo/redo stack over `look` snapshots (push on
  create/move/resize/delete/restyle/recommended); `undo()`/`redo()`; keep selection
  semantics honest (empty-canvas click does NOT clear a valid selection unless a real
  deselect is intended; delete clears deliberately).
- `components/canvas/wallpaper-mirror.tsx`:
  - Port Ctrl+wheel-zoom-at-pointer + pan from `icons-mirror.tsx` (extract the shared
    zoom/pan hook to `lib/canvas-view.ts` so both mirrors use one implementation).
  - `cursor-crosshair` on the create host; left-drag creates, space/middle-drag pans.
  - Live-snap the rubber-band (draw `createFromDrag` output); `snap-pulse` on cell
    change (add the variant to `lib/motion.ts`).
  - Hide zone chrome + ghosts + rubber-band while `comparing`.
  - Empty state = centered dashed drop-frame + hint + inline 用推荐布局 (replace pill).
  - Double-click the on-canvas title band → inline rename.
  - Skeleton `shimmer` while loading + first-frame fade.
  - Enlarge handle hit boxes (transparent padding).
  - Keyboard: `Delete` only (drop `Backspace`); Ctrl+Z/Ctrl+Shift+Z.
- `components/canvas/paper-coach.tsx` — NEW one-shot coach mark wired to
  `wallpaperCoachShown` (settings flag) on first module entry.
- `App.tsx`: add Ctrl+Z/Ctrl+Shift+Z routing to the active module's undo.
- **Gate:** state tests for undo/redo, edit-scope, first-free-area add; `bun test`
  green; manual live-verify the gestures (real mouse) once the host build runs.

## P3 — Wallpaper visual pass + zone list  (task #50)

Spec 04 §2, spec 02 language.

- `components/panels/wallpaper-panel.tsx`:
  - Explicit edit-scope: persistent `正在编辑：<name>` header on 分区样式; controls
    target the selected zone; explicit 应用到全部; no silent mass-edit; honest
    `多个分区` readout with no selection.
  - Zone list rows → premium cards: leading 20px style swatch, visible-editable title
    (hairline underline), WxH, ✕; selected row washed.
  - Refold 清晰度 advanced (dim / gradient / angle / scrim) behind a 高级 sub-fold;
    keep 关/柔和/强 + 推荐 badge visible.
  - 添加分区 → first-free-area 6×4; 用推荐布局 confirm when zones exist.
  - Curate the font popover (a few handwritten faces up top, full list below).
- **Gate:** `bun test` green; panel renders dark+light in the gallery; edit-scope
  test proves no-selection changes nothing.

## P4 — Settings page macOS redesign + version cleanup  (task #51)

Spec 03 §3.

- `components/panels/settings-page.tsx`: rebuild on the grouped `Card`; consistent
  icon badges; equal-weight full-width cards; three distinct 本地数据 actions (wire
  a real snapshot export via the bridge, or drop 导出 if the engine has no distinct
  export — confirm the RPC before deciding); trust chips as real chips; **remove the
  changelog section**.
- `components/shell/title-bar.tsx`: remove the version chip; add the `?` keymap
  affordance (opens a legend popover — shared component).
- Version cleanup: `Directory.Build.props` → pre-release marker; stop surfacing
  `changelog.json` in the UI (keep the file or delete per owner — it's dev-only now);
  drop `Drawer_Save` string usage.
- **Gate:** `bun test` green; settings renders dark+light; no version/changelog in UI.

## P5 — Customization IA + icons polish + global  (task #52)

Spec 03 §3.1.

- Panels (`icons-panel.tsx`, `wallpaper-panel.tsx`): axes open-by-default / persist
  last-open (localStorage); a current-values summary strip; presets stay top.
- `components/canvas/icons-mirror.tsx`: per-tile hover `⋯` affordance opening the
  existing keep/follow/tint menu; apply the new glass tokens.
- `App.tsx`: module switch overlap (drop `mode="wait"`); guard `scan`/`load` so a
  loaded module does not refetch on re-entry.
- Apply the v2 visual system across the icons panel (chip previews for
  shape/color/mark, card grouping, type ladder).
- **Gate:** `bun test` green; icons module renders dark+light; no re-scan on switch.

## P6 — Review + verify  (task #53)

- Adversarial cross-vendor review (`/multi-ai` codex) — **stage A spec-compliance**
  (built exactly ADR-0012 + specs, no over/under-build) then **stage B code-quality**
  (DRY/cohesion/≤500 lines/naming). Fix findings, re-review.
- Verify (IRON LAW, fresh evidence): `bun run build`; `bun test`; `dotnet test`
  (engine + E2E under `DESKMAKEOVER_E2E=1` with `DESKMAKEOVER_FAKE_APPLY=1`);
  `bun scripts/publish-win.mjs`; fresh-terminal exe smoke; banned-colour grep gate.
- Update `docs/STATE.md`; leave the real bake + wallpaper apply to the owner's
  supervised gate (`docs/verification/owner-supervised-live-runs.md`).

---

## Execution log

- 2026-07-08: docs written (ADR-0012, spec 02 v2, spec 03 §3/§3.1, spec 04 §3.5,
  this plan, panel review). Owner decisions recorded. Build phases pending.
