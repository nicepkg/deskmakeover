# 2026-07-13 — 清爽 page polish panel (3 seats) + codex R2 + designer acceptance

**Trigger:** owner complaint on the W0 skeleton — 「看起来有些怪异。我自己看文字都一头雾水，
不清楚到底关的是哪里，缺乏可视化指引」. **Seats:** PM · UI/visual · UX — isolated subagents
(Fable 5), real zh renders + source as artifacts. (Materials incident: the first screenshot
batch was a stale duplicate — all three seats caught it and reviewed from source; re-captured
via explicit filePath for the acceptance pass.)

## Root causes (all three seats converged)

1. **The WHERE axis was dropped.** spec 08 §2's row grammar (16px keyline glyph) was never
   implemented; `catalog.surface` data existed but the view discarded it — rows were an
   abstract-noun text wall ("推广推荐/热点资讯/使用建议" 同质灰字).
2. **The honest IA whispered.** Group headers (承重的三层诚实分组) rendered at 12px t3 —
   smaller than row descriptions.
3. **The hero was an orphan.** A lone 280px button against ~600px of empty slab (spec 02
   "no empty slabs"), implying "one click = everything" while covering 3 automatic items.
4. Consent sheet: mechanical three-string concatenation with a dangling 「下面这些」 reference.
5. Toggle polarity ambiguity: an iOS switch read as on/off-the-feature, but meant
   include/exclude-from-batch (UX; clashed with UI's keep-toggle — ruled for checkbox,
   precedent: the icons kindPolicy checkboxes).

## Shipped fixes (commits `aac7434` + `47f99ac` predecessor + acceptance `HEAD`)

Per-row surface glyph pin + place tag (per-control glyph map, surface fallback) · hero band
(constellation pins = one per control, count matches 「N 处」copy; promise + CTA 210px right,
restore beneath) · group headers at cardtitle + honest one-line subtitles · descriptions
rewritten as located result sentences (「开始菜单不再推荐你没装的应用（你自己的常用文件保留）」)
· three-line consent (list / guarantee / guided exception) via ConfirmSheet body: ReactNode ·
inclusion checkboxes (kindPolicy grammar) · collateral disclosure upgraded to an ⓘ line ·
right-anchored control column min-w-[132px] · held group folded header at cardtitle + count
pill · surface re-tags (settings/system) · widgets family row names feed+hover+badges+
announcements, route text surfaces after the walk.

## codex R2 closes (same batch)

Lost apply reply → `unknown` + auto re-probe, NEVER an unproven 「已还原」 claim · skip
reasons ride the port and surface as row captions · exclusion LOAD failure now toasts ·
certification boundary outranks a stale ledger ownership claim in probeTransition · honest
hero phases (all-quiet ≠ scanning-forever; awaiting gets its own non-interactive truth;
restore shows 正在恢复) · copy gate strengthened (仍可用 required, hyphen regression gate,
复原 banned, widgets-family completeness assert, account-notice overclaim ban).

## Designer acceptance: **PASS** (2026-07-13, review-ui seat, real renders)

Verified: hierarchy/spacing/color discipline per spec 02; coral stays event-grade; no banned
palette. P2 fixed same-day (constellation per-control + lit ring + glyph dedupe). The
suspected lit-pin inconsistency was diagnosed as glyph ink-density optics, solved by the ring.

## Owner calls (open)

| # | Item | Note |
|---|---|---|
| O1 | IncludeCheckbox 珊瑚预算 | Selected = solid coral fill matches the shipped kindPolicy checkbox grammar; spec 02 rule 3 would prescribe wash+ink+hairline. Panel defers to the owner-shipped pattern — change only if the owner wants the lighter form. |
| O2 | Component-render/a11y test infra | Repo has no render-test substrate; adding happy-dom/@testing-library is a dependency decision. Until then guided≠toggle etc. are pinned at state/store level + browser E2E + this acceptance loop. |
| O3 | Group-1 thinness at launch (PM P2-3) | Mock shows 3 certified rows vs 6 held; real ratio depends on the Wave-3 cert lab — revisit copy/lineup when lab rows land. |

**Deferred to the next visual round:** hero OS-mirror mini taskbar (before→after), settle/pop
motion pass (pending rows currently use animate-pulse as the shimmer stand-in) — re-acceptance
booked with the designer seat after the motion pass.

Evidence: `docs/plans/evidence/2026-07-calm/02-polish-applied.webp` · `03-polish-initial.webp`.
