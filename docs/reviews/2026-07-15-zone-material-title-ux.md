# Review — Zone material lineup, title system & editor UX (2026-07-15)

Two isolated expert seats (Chief Product Designer — visual/UI; Chief UX Engineer —
interaction, same-vendor subagents, fresh context each) reviewed the zone material
axis, title system, and the zone-editing flow against live screenshots + code.
Owner disposed every item the same day. **This document is the binding detail
record for the spec 04 §2/§3/§4.1/§4.2 amendments dated 2026-07-15** (round 3),
in the lineage of `2026-07-09-style-sets-import-export.md` (round 2).

## Evidence base

- Owner feedback (verbatim complaints): materials other than Frost / LiquidGlass /
  Outline "差异不大"; material swatches "看不清样式，点了才知道"; emoji editing
  far from the title text "操作太割裂"; no zone context menu ("删除要跑右侧面板");
  opacity 3–95% and corner 8–28px ranges too narrow; wants hide-title.
- Designer diagnosis (systemic, not taste): Frost/Luminous/Solid/Halo fills all
  sit at L 0.92–0.96 on ONE axis (`material.ts` fillL) — four materials are one
  idea at different brightnesses. Whole palette chroma-capped ≤0.03 (timid).
- UX diagnosis: title is ONE concept split across three surfaces (list-row text,
  canvas double-click rename, emoji in the style row's sub slot); delete exists
  three ways but none is discoverable (hover-only ✕, unhinted Delete key);
  right-click is verified no-op on zones (safe to claim); material-switch reset
  model is self-contradictory (resets titleStyle+cornerRadius, keeps fillOpacity).

## Dispositions (owner, 2026-07-15 — all ACCEPTED as recommended)

| # | Decision | Disposition |
|---|----------|-------------|
| 1 | **Material lineup — six finishes, one axis each**: keep Outline·Frost·LiquidGlass; retire Luminous/Solid/Halo; add **Glaze 釉色** (accent-saturated colored glass, chroma unlocked to ~0.10–0.12, the ONE color-committed material), **Paper 素笺** (warm matte paper: opaque warm off-white/off-charcoal, fine noise dither, letterpress 1px top-light/bottom-dark, the anti-glass option), **Float 浮屿** (minimal tray: low fill ~0.18, signature soft baked drop shadow AS the identity, depth axis). All three reuse existing recipe hooks (chroma/innerGlow/shadow/fill) — no new render pipeline. | accept |
| 2 | **Title system — converge + glass title**: keep Chip·Bare·Bar; **retire Tab** (2010s folder skeuomorph); add **Etched 冰签** — glass-native title: translucent frosted lozenge (white α≈0.16) + 1px top-light / 1px bottom-dark bevel ("etched into glass"), adaptive ink, NO accent block; becomes LiquidGlass's default. **「无」(hide title) is the FIRST swatch of the title-style row** (hiding is a legitimate answer to "how is this zone labeled"), selecting it collapses/disables the size controls. System = 4 styles + 无. | accept |
| 3 | **Picker WYSIWYG — wallpaper-crop tiles**: every material swatch renders the REAL effect on a crop of the user's current wallpaper, ≥40px, with a persistent selected-material name caption; reuses the preset-popover on-wallpaper pattern. Hover-live-preview on the real zone = phase-2 enhancement. Unified selected-state token + ≥40px hit areas across material/title swatches. | accept |
| 4 | **Material-switch semantics — "untouched → new default / touched → keep"**: an axis (titleStyle/cornerRadius/fillOpacity) whose current value equals the OUTGOING material's default is untouched → adopt the new material's tuned default; otherwise keep the user's value. titleStyle additionally falls back when illegal for the new material. | accept |
| 5 | **Corner radius 0–60px, LiquidGlass default 44**: slider 0–60 step 2 (true square corners allowed); render-side invariant `min(radius, shortestSide/2)`; remove the hard `clamp(…,8,28)` in `material.ts`; other material defaults stay 20–28. | accept |
| 6 | **Zone context menu — full set**: 重命名 · 改 emoji · 隐藏标题 · 复制分区 · 应用样式到全部 ┊ 删除分区 (red, NO confirm — removeZone already ships toast+undo). Menu reuses the icons-layer TileMenu dialect; right-click selects the zone first; host gets `onContextMenu preventDefault` (fixes the OS-menu inconsistency). 置顶/置底 REJECTED — overlap is an allowed-but-discouraged anti-pattern; no legitimizing controls. | accept |

## Pre-decided by owner (no panel question needed)

- **Opacity slider 0–100%** (was 3–95). For LiquidGlass, fillOpacity maps to the
  shader Tint: 0 = pure refraction (no wash), 1 = solid white. **LiquidGlass
  defaults: fillOpacity 0 + cornerRadius 44.** Solid@0 panel-invisible is an
  explicit user act — allowed.
- **Emoji lives beside the title text**: the zone-list row's emoji becomes the
  EmojiPicker trigger (remove the picker from the title-style row's sub slot);
  target state is a shared `TitleField` ([emoji][text]) reused by the canvas
  rename band. (UX seat: A1 now, A3 as the follow-up.)
- Hidden-capability fixes ride along: Delete-key affordance via the context menu.

## Migration (owner-approved lineup change, TS-side only — enums are not in the Rust contract)

Persisted zones map on load: `Luminous → Frost` · `Solid → Paper` ·
`Halo → Float` · titleStyle `Tab → Chip`. One-way, silent, applied in the
wallpaper store's load path.

## Codex cross-review (post-build, run-20260714165308) — verdict FIX-4, dispositions

| P | Finding | Disposition |
|---|---------|-------------|
| P2 | Material tiles are CSS approximations over the wallpaper, not compositor-rendered thumbnails (no refraction/grain in the mini) | **defer** — the disposed design chose the preset-popover on-wallpaper pattern (designer's recommended cheap path; glass mini flagged "较贵"); compositor-true thumbnails ride phase 2 WITH hover-live-preview, which is the committed-truth path |
| P3 | Swatch hit areas 36px < the record's ≥40px | **accept, fixed** — size-10 (40px) + gap-0.5 so six tiles still fit the 280px inspector |
| P3 | Auto-tone zones display the Light opacity default even when resolved Dark (≤2% off for every finish) | **defer** — display-only (render uses the resolved tone); the fix needs ZoneMeta piped into the store; revisit if any finish's Light/Dark defaults ever diverge >2% |
| P3 | Context menu offered only a 6-emoji strip — no full picker / explicit 无 | **accept, fixed** — strip trimmed to 5 + the full shared EmojiPicker (custom input + 无) appended in the menu |

## Owner look-review round 2 (same day) — Glaze & Float cut, replacements shipped

Owner verdict on the live render: Glaze 釉色 "很丑" (dark tone = muddy
accent×wallpaper mix — no luminosity), Float 浮屿 "很丑" (α0.18 invisible on a
busy wallpaper — the Halo disease again); Paper 素笺 praised ("内置的纹理让人看
起来很舒服" despite zero translucency) → **the owner's bar is tactile texture,
not translucency games**. Designer's definitive replacements (accepted and
built):

- **Fluted 棱纹玻璃** (slot 3): vertical fluted-glass light bands over frost —
  geometric texture instead of color mixing; chroma capped ≤0.018 kills the
  muddy-mix death at the root. Title default Etched; no Bar (no solid top edge).
- **Brushed 拉丝金属** (slot 5): near-opaque (α0.88/0.90) warm-graphite brushed
  metal — presence via opacity + ONE diagonal sheen band (the anti-invisibility
  weapon; dark-tone alpha raised 0.14→0.2 after live check), fine 2px streak
  tile, metal bevel + plate bottom edge. Title default Chip; radius default 18.
- Engineering note: procedural tiles moved to `panel-textures.ts`
  (noise/flute/brush + baked sheen gradient); Graphics texture fills MUST pass
  `textureSpace: 'global'` (local default stretches the tile over the panel and
  erases the pattern) — the sheen alone uses a baked canvas gradient stretched
  `local` (FillGradient alpha stops proved unreliable).
- Migration additions: Glaze→Fluted, Float→Brushed, Halo re-aimed →Frost.

**Owner look-review round 3**: Brushed approved ("很不错"); Fluted's innovation
approved but the first cut dazzled under icons ("用户会将软件放置于其上…眼花缭
乱") — zones are ICON CONTAINERS, the material is a quiet stage. Calmed: rib
period 12→28px, ridge α0.12→0.065, valley α0.06→0.032 (panel-textures.ts) —
the light-band identity survives at a whisper; icon legibility wins.

## Rejected / deferred (with reasons)

- 置顶/置底 in the context menu — rejected (legitimizes overlap anti-pattern).
- 样式刷 (copy/paste style) — deferred; overlaps 应用到全部, needs clipboard state.
- Empty-canvas context menu (添加分区/预设) — deferred, low frequency.
- Hover-live-preview material picking — deferred to phase 2 (WYSIWYG tiles first).
- 辉映 Aura (accent-halo revival of Halo) — deferred, optional 7th slot later.
