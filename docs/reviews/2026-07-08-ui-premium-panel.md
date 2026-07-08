# Expert-panel review — premium UI redesign (2026-07-08)

Four isolated design seats (fresh context each, single-vendor Claude — cross-vendor
was not used; verdicts are code-based, not screenshot-based, because the icons/
wallpaper canvases only render under the C# host and the running instance was the
owner's live session). Artifact = the shipped `src/DeskMakeover.Web` source + specs
01–05. Findings deduped across seats; ⭐ = independently named by ≥2 seats.

Dispositions reflect the owner's four interview answers (2026-07-08) plus the
commander's auto-accept of consensus fixes. See ADR-0012 for the governing decision.

## Owner decisions (interview)

| # | Question | Answer |
|---|----------|--------|
| 1 | Aesthetic north star | **macOS System Settings** (calm/grouped/spacious/material) |
| 2 | Customization depth | **Visible by default** + presets as top fast path |
| 3 | Wallpaper scope | **Perfect the existing capability; add no features** |
| 4 | Zone-editor tech | **Keep hand-rolled DOM overlay**; invest in feel, no library |

## Disposition table (most severe first)

| # | Seat(s) | P | Finding | Evidence (intent ∧ artifact) | Fix direction | Disposition |
|---|---------|---|---------|------------------------------|---------------|-------------|
| 1 | ⭐ PM·UX | P1 | Style controls silently retarget "this zone" vs "every zone" with no indicator; deselect on empty-click/delete → tweak repaints all | spec 04 §2.5 dual-target demands a visible mode ∧ `wallpaper-panel.tsx:274-307` `selected!==null ? mutateZone : zones.map` | Persistent `正在编辑:<zone>` header; controls target selection; explicit 应用到全部; per-zone style dot; honest "多个分区" readout | **accept** (ADR-0012 D3) |
| 2 | ⭐ PM·UX·Interaction | P1 | "Drag to create" unsignified; coach mark + spec empty-state are dead code | spec 04 §1/§3 mandate coach mark + centered dashed frame ∧ `wallpaper-mirror.tsx:324-331` tiny pill; `wallpaperCoachShown`/`Paper_Coach` never rendered | Render dashed drop-frame + hint + inline 用推荐布局; `cursor-crosshair`; wire one-shot coach mark | **accept** (D3) |
| 3 | ⭐ Interaction | P1 | Left-drag = pan in icons but = create in paper (gesture conflict); paper canvas has zero zoom/pan | direct-manipulation consistency + spec 04 §3 "manual zoom still works" ∧ `wallpaper-mirror.tsx` no onWheel/pan; `icons-mirror.tsx:107` pans | Port Ctrl+wheel-zoom/pan; crosshair create; pan = space/middle | **accept** (D3) |
| 4 | ⭐ UX | P1 | No undo anywhere in wallpaper; Delete + 用推荐布局 (replace-all, one chip from 添加分区) irreversible | product reversibility promise + icons has history ∧ `stores/wallpaper.ts` no history; `wallpaper-panel.tsx:82-96` replace-all | Ctrl+Z over look snapshots; confirm 用推荐布局 when zones exist; drop Backspace-delete | **accept** (D3) |
| 5 | ⭐ UI | P1 | Type "scale" is ~10 near-identical micro-steps (9.5–13 in a 3.5px band) + three "large" title sizes | type discipline ∧ literals across codebase; `settings-page.tsx:31,32,34,51` 17/19/22 | Collapse to 11/13/15/19/26, one role each; delete 0.5px steps | **accept** (D2) |
| 6 | ⭐ UI | P1 | "Light not lines" inverted: bg→raised ~6%, zero card shadows, everything hairline-separated → generic dark dashboard | spec 02 §3 "separation via elevation/shadow" ∧ `index.css:66,79`; flat `bg-raised` cards everywhere | Add `--elev-1/2` shadow tokens on every raised card; widen bg→raised | **accept** (D2, highest-leverage visual) |
| 7 | ⭐ UI | P1 | Chips are text-only; speced live-preview swatches (shape/color/mark) missing → 7 selectors look identical | spec 02 §Geometry mandates in-chip previews ∧ `chip.tsx:34` renders only children; `icons-panel.tsx:185,195,233` | Implement 14px clip / 10px dot / 22px mark previews | **accept** (D2) |
| 8 | ⭐ UI·UX·PM | P2 | Settings page: two card radii side by side + orphan coral icon badge (only Appearance) + unbalanced 2-col grid (lone toggle) | Gestalt similarity + spec 03 §3 ∧ `settings-page.tsx:29,42 vs 160`; badge only at `:56`; grid `:82` | One radius; consistent icon badge; equal-weight full-width cards | **accept** (D4) |
| 9 | ⭐ UX·PM | P2 | 导出快照 doesn't export (both it and 打开文件夹 call `openDataFolder`); reuses retired `Drawer_Save` | spec 03 §3 three distinct actions ∧ `settings-page.tsx:94,100` identical; `:97` retired string | Make 导出快照 export or drop; three distinct actions; drop `Drawer_Save` | **accept** (D4) |
| 10 | PM | P1 | Panel IA tuned for restraint-persona: all axes collapsed on load, 自定义 below presets/history → power user's knobs 6+ clicks deep | review target = power customizers ∧ `icons-panel.tsx:60`, `wallpaper-panel.tsx:43` all-closed | Default/persist open; current-values summary strip; presets stay top fast path | **accept** (D2 owner Q2) |
| 11 | Interaction | P2 | Rubber-band create doesn't live-snap (jumps on release) while move/resize snap live; spec's 80ms snap pulse absent | spec 04 §3 "live-snaps" + pulse ∧ `wallpaper-mirror.tsx:315-318` raw; `motion.ts` no pulse | Draw `createFromDrag` live; add 80ms pulse on snap-cell change | **accept** (D3) |
| 12 | Interaction | P2 | Hold-to-compare in paper leaves editor chrome (outline/handles/ghosts) painted over the original | spec 04 §3 "shows the un-styled desktop" ∧ `wallpaper-mirror.tsx:236` zones render regardless of `comparing` | Hide zone chrome + ghosts + rubber while comparing | **accept** (D3) |
| 13 | PM·UX·Interaction | P2 | Rename split across surfaces: type in cramped panel list while title bakes on canvas (execution gulf) | spec 04 §3 "double-click title → inline TextBox" ∧ rename only at `wallpaper-panel.tsx:142-148` | Double-click on-canvas title band to rename; keep list input secondary | **accept** (D3) |
| 14 | UI·UX | P2 | Zone list reads as a spreadsheet: bare transparent title input (looks static), no style swatch, ~6% row contrast | affordance + hierarchy ∧ `wallpaper-panel.tsx:139,146` | Leading 20px style swatch per row; title input with hover/focus underline | **accept** (D3/D2) |
| 15 | UI·PM | P2 | Canvas pills use off-token cool grays; app chrome tonally identical to the mirrored OS taskbar | color cohesion + spec 02 tokens ∧ `icons-mirror.tsx:13,410`; `wallpaper-mirror.tsx:346`; `bg-[#0E0E10]` literals | Warm-token glass for app chrome; reserve white frost for taskbar; tokenize stage | **accept** (D2) |
| 16 | PM | P2 | 清晰度 dumps dim+gradient+angle+scrim inline (spec put them behind a 高级 fold); the module's "not premium" clutter | spec 04 §2.4 "高级 fold (collapsed)" ∧ `wallpaper-panel.tsx:194-265` all flat | Keep 关/柔和/强 + 推荐 visible; fold dim/gradient/angle/scrim under 高级 | **accept** (D3) |
| 17 | UX | P2 | Per-icon override (keep/follow/tint) reachable only by unsignified right-click | discoverability ∧ `icons-mirror.tsx:315` contextmenu-only; canvas suppresses native menu | Hover "⋯" chip on tiles opens the same menu | **accept** (D2) |
| 18 | Interaction | P2 | Module switch janky: `mode="wait"` empty frame + `key={module}` remount re-runs scan/load every switch | latency/continuity ∧ `App.tsx:87` + `:112,:140` | Drop `mode="wait"` (overlap); guard scan/load so loaded module doesn't refetch | **accept** (D2 global) |
| 19 | UX·Interaction | P2 | 添加分区 drops fixed 0.5,0.5 oversized (not spec's 6×4 at first free area) → stacks on itself; two create paths disagree | spec 04 §2.5/§3 ∧ `wallpaper-panel.tsx:73-80` | First-free-area 6×4 placement, shared by button + click-fallback | **accept** (D3) |
| 20 | UI | P3 | No baseline grid: ad-hoc 0.5/1.5/2.5 gaps; link chips at 2px merge | spacing rhythm ∧ `icons-panel.tsx:82-144`; `settings` mixed | Snap to {4,8,12,16,24}; bump link-chip gap | **accept** (D2) |
| 21 | Interaction·UX | P3 | Load-bearing gestures (Ctrl+1/2/3, hold-Space, drag-create, Del) have no on-screen legend | discoverability ∧ `App.tsx:40-73` no legend | Small "?" keymap popover in the title bar | **accept** (D2 global) |
| 22 | Interaction·UI | P3 | Wallpaper load shows a dead box (no shimmer) then hard-cuts to canvas; resize handles are tiny hit targets | feedback/polish ∧ `wallpaper-mirror.tsx:107,202,300` | Skeleton shimmer + first-frame fade; enlarge handle hit boxes | **accept** (D3) |

**No findings rejected.** Wallpaper scope-expansion ideas (more styles, per-zone
templates, align/distribute tools) were raised by the Interaction/PM seats as
*possible* directions and are **out of scope** per owner Q3 (perfect existing, add
nothing).

## Highest-leverage moves (seat self-nominations)

- **PM & UX:** rebuild the wallpaper edit-scope model (findings 1–4) — the owner's
  loudest complaint and where the power user lives; mostly logic + small components.
- **UI:** install real elevation (finding 6) — the app's own law, one systemic
  change across every surface; type-scale collapse (5) a close second.
- **Interaction:** fix the paper canvas gesture identity (findings 2+3+11) — one
  stroke converts the worst module from "I keep mis-firing" to "it announces itself."

## Fix sequence (PM, quality-per-effort) — adopted

1. Wallpaper interaction logic (1·2·3·4·11·12·13·19) → *usable*
2. Wallpaper visual + zone list (14·16) → *premium*
3. Settings page (8·9) → cheap, kills obvious "ugly" + a real bug
4. Customization-depth IA (10) → serves the target user across icons + wallpaper
5. Icons polish + global + tokens (5·6·7·15·17·18·20·21·22) — the systemic visual
   layer underpins all of the above and lands as one design-system pass first
   (foundation), with module-specific polish last.
