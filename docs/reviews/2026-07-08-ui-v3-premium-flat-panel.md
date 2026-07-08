# Expert-panel review — v3 premium flat redesign (2026-07-08, panel run #2)

Five isolated seats (UI · UX · Interaction · PM · Typography/design-system), fresh
context each, single-vendor Claude. **Unlike run #1 (code-vs-spec), this run judged
REAL rendered pixels**: the owner's three light-theme screenshots of the running app
(`evidence/2026-07-08-ui-v3/01..03*.png`) plus the web source. Dispatches carried the
owner's NEW brief (flat · white-leaning · Apple-flavored · dark-follows-system ·
zh/en · responsive · bundled free-commercial fonts · aesthete-customizer audience ·
pre-release, breaking rewrites allowed) and no prior-panel conclusions.

**This is the final full-panel run on this artifact** (max 2 per the studio protocol);
further judgment comes from real-user evidence after release.

## Panel verdict (unanimous)

The layout skeleton (rail + panel + mirror) is right. The premium gap lives in three
inherited layers, not in rearrangement:

1. **Neutral palette**: light theme is a dark-first flip → warm taupe "washed-out
   beige", not crisp premium white. Rebuild light-first (OKLCH cool-neutral ramp),
   derive dark from it.
2. **Typography**: Chinese falls to Microsoft YaHei (the "cheap system tool" face);
   Latin/CJK come from different families (visible seams in every mixed run); Segoe
   UI Variable is Win11-only so Win10 silently degrades. Fix = bundled fonts.
3. **Load-bearing components & feel**: fake-macOS segmented, wallpaper drag where the
   frost never follows the pointer (140ms recompose debounce resets during drag),
   45%-dim restyle dance, dead-cut menus, stranded consent/done ceremony (written,
   zero references).

## Owner decisions (2026-07-08 interview, 12 items)

| # | Question | Decision |
|---|----------|----------|
| D1 | Accent | **Keep coral #FF6F5E + blue/violet ban.** Add `ink-coral` variant for large solids on white; selected states get border reinforcement. |
| D2 | Bundled fonts | **Inter (Latin, VF, OFL) + HarmonyOS Sans SC (Regular 400 / Medium 500)**, ~3-5MB subsets, `document.fonts.ready` first-frame gate, OS-faithful mirror-label token stays Segoe/YaHei. Attribution line in About (HarmonyOS obligation). MiSans rejected (revocable/no-redistribution license). |
| D3 | Theme | **Follow system by default; light theme is the design first-citizen.** Rebuild tokens light-first in OKLCH, derive dark, test both. |
| D4 | Release cut | **All three modules land before the public release** (owner overrode PM's icons-first M1 recommendation — no partial ship). |
| D5 | Shape chips | **6 curated on the first screen (苹果/纯圆/三星/方块/水滴/无) + 「更多形状」 fold for the other 7.** Engine keeps all 13. |
| D6 | Filter axis | **Keep all four (无/玻璃/像素/贴纸).** PM's P1 "scope creep / anti-premium" finding REJECTED — owner wants the playfulness; engine already implements them. Recorded so future panels don't re-raise. |
| D7 | Wallpaper hero | **清晰度 becomes the hero narrative; zones demote to secondary capability.** No feature add/remove — narrative and IA order only. |
| D8 | First-run wow | **Auto-play one 原样→美化 transformation in the MIRROR after first scan** (skippable; preview-only, real apply stays click-gated). |
| D9 | Apply ceremony | **Wire the stranded consent + done strings**: first-apply confirm sheet (N icons / files untouched / one UAC), per-apply completion state + 「去看看桌面」; restore gets its confirm too. |
| D10 | Gesture model | **Unify: drag = pan on BOTH canvases; zone creation becomes an explicit tool state (crosshair mode / Alt+drag); Space = compare only.** |
| D11 | All remaining P1/P2 (and P3 polish group) | **Accepted in bulk** — see table below. |
| D12 | Responsive floor | **Keep current breakpoints** (≥1100 regular, <1100 compact overlay, min 1024×700); polish compact experience (visible ✕, mirror-first). |

## Disposition table

⭐ = independently raised by ≥2 seats. Fix directions are the panel's; sequencing
happens in the build plan, not here.

| # | Seat(s) | P | Finding | Disposition |
|---|---------|---|---------|-------------|
| 1 | UI | P1 | Light palette is warm-taupe dark-flip, not premium white (`--chip #ECEBE7`, `--t2 #57534E`, `--raised-hov #F0EFEC`) | **accept** → D3 light-first OKLCH rebuild |
| 2 | Type | P1 | CJK falls to Microsoft YaHei; 500/600 synthesized → blotchy headings; 11px captions fuzzy | **accept** → D2 bundled fonts |
| 3 | Type | P1 | Latin/CJK different families = visible seams; Segoe UI Variable is Win11-only | **accept** → D2 |
| 4 | ⭐ PM·UX | P1 | Capability dump: 13 shape chips incl. jargon (Google/Brave/Squircle/软团/矩圆), mixed-language names | **accept** → D5 curation + Chinese-first naming |
| 5 | PM | P1 | Filter axis (像素/贴纸) off-scope + fights premium positioning | **REJECT** (owner: keep all four — D6) |
| 6 | PM | P1 | Wow not front-loaded; compact first-run hides the mirror behind a control wall | **accept** → D8 auto-transform + compact mirror-first |
| 7 | UX | P1 | Apply crosses mirror→real desktop with zero consent; `ConsentTitle/ConsentWhatFormat/ConsentNot/ConsentUac/ConsentAgree` stranded (0 refs) | **accept** → D9 |
| 8 | UX | P1 | No "this is a preview" anchor in icons module (paper has one) | **accept** → D9/lang layer |
| 9 | UX | P1 | Post-apply: no completion state; `DoneHeadline`/`GoSeeDesktop` stranded; real change hidden behind window | **accept** → D9 |
| 10 | IXD | P1 | Wallpaper zone drag: frost/title canvas layer never updates during drag (140ms debounce resets); only outline follows | **accept** → DOM approximate fill during gesture, reconcile on true frame |
| 11 | IXD | P1 | Canvas gesture semantics conflict (icons drag=pan vs paper drag=create); paper pan trackpad-unreachable; Space overloads compare+pan | **accept** → D10 |
| 12 | UI | P1 | Settings segmented: full-width slabs, selection nearly invisible on white; not a real macOS segmented | **accept** → sliding white thumb + max-width cap / inset rows |
| 13 | ⭐ PM·UI·Type | P2 | Preset card titles break mid-word (苹果极/简, 壁纸同/色) | **accept** → keep-all + layout fix |
| 14 | UI | P2 | White chrome × near-black canvas stage hard cut + pure-black letterbox | **accept** → elevated stage card, neutral matte letterbox (WYSIWYG untouched) |
| 15 | UI | P2 | Coral untuned for white: large solids read candy; 17% wash selection invisible on white | **accept** → D1 ink-coral + selection reinforcement |
| 16 | ⭐ UI·PM | P2 | Settings density too low (empty slabs, identity column duplicates About card) | **accept** → tighter inset lists, dedupe |
| 17 | IXD | P2 | Restyle "dim dance": instant 45% whole-canvas dim + 420ms idle before request | **accept** → latency-gated cue (~200ms), 88% or pill-only |
| 18 | IXD | P2 | CTA working state static; phase text hard-swaps | **accept** → indeterminate coral shimmer + 120ms text crossfade + ✓ pop |
| 19 | IXD | P2 | Per-icon override menu mounts/unmounts with no `pop` motion (violates own spec) | **accept** |
| 20 | IXD | P2 | reduced-motion holes: bloom/settle waves + module slide ignore it | **accept** |
| 21 | UX | P2 | Axis summary strip values without axis labels (「苹果 · 原彩 · 无 · 无标识 · 中」) | **accept** → label:value pairs |
| 22 | UX | P2 | Zone size leaks internal grid units 「7×12.5」 (half-cell decimals) | **accept** → hide or humanize |
| 23 | UX | P2 | Global Space hijacks button activation (keyboard a11y) | **accept** → pass through when focus is on a button |
| 24 | UX | P2 | Drag-create & per-icon ⋯ undiscoverable after first run | **accept** → persistent faint hint + one-shot coach |
| 25 | PM | P2 | Wallpaper hero sells "organizer" not "beauty" | **accept** → D7 |
| 26 | PM | P2 | "Ship twice via WPF first" | **obsolete** — WPF layer already deleted; roadmap text was stale (fixed in this run) |
| 27 | Type | P2 | Weight 600 doesn't exist in bundled CJK → revised ladder {400,500} (+700 escape) | **accept** → D2 ladder v3 |
| 28 | Type | P2 | No letter-spacing rules (Latin display needs −0.01em; CJK never negative) | **accept** |
| 29 | UI·Type | P3 | Type-scale self-violations (9.5px rail label, 12.5px segmented); caption 11→12px | **accept** (bulk P3) |
| 30 | UX | P3 | Compact overlay lacks visible ✕; restore lacks confirm (string stranded); Squircle/软团 naming | **accept** (bulk P3, restore-confirm folded into D9) |
| 31 | IXD | P3 | Motion family outliers (settle 'easeOut'); tokens re-inlined in consumers; chip weight-jump reflow; toggle spring toy-bounce | **accept** (bulk P3) |
| 32 | PM | P3 | String drift 「新图标自动跟上」 vs spec 「新图标自动美化」 | **accept** → unify + freeze string table pre-release |

## Seat "if you do only one thing" (kept for the record)

- **UI**: rebuild the light neutral ramp cool/true-white — every surface inherits it.
- **Type**: bundle Inter + HarmonyOS Sans SC with a fonts.ready gate.
- **UX**: wire the stranded consent+done ceremony onto 一键美化.
- **IXD**: make the dragged zone's frost follow the pointer 1:1 (DOM approximation, reconcile on real frame).
- **PM**: auto-play the before→after transformation at second 0.

## Standing constraints reaffirmed

WYSIWYG law (canvas = engine pixels; DOM gesture-approximation must reconcile on
pointer-up); real apply/restore remain explicit human clicks; engine/bridge/C# layer
untouched by this redesign; blue/violet stay banned; OS-mirror surfaces (taskbar,
tile labels) keep OS-faithful fonts/colors, exempt from brand tokens.

## Full seat reports

Archived verbatim in `evidence/2026-07-08-ui-v3/seat-reports.md` (UI, UX, Interaction,
PM, Typography). A parallel WebView2 environment-pitfalls research report lands in
`docs/references/webview2-pitfalls.md` when complete and feeds the build plan's
hardening phase.
