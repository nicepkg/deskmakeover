# 2026-07-21 — Community proposal panel (tdrfish issues #3–#7)

Four isolated same-vendor seats (Chief PM · Chief Designer/UX · Product-standards owner ·
Chief Software Engineer), round-1 independent, no owner-preference leakage; the #3 re-edit
adjudication was dispatched ANONYMIZED (neither competing model attributed to anyone).
Owner disposed all four decisions 2026-07-21. Contributor context: tdrfish has 0 PRs and
no fork; each enhancement says "will implement in my own branch" — this record exists to
set expectations BEFORE that code is written.

## Disposition table (owner-disposed)

| # | Item | Seats | Verdict | Owner disposition |
|---|------|-------|---------|-------------------|
| 1 | #5 body: per-type groups + follow/custom styling | 4/4 ⭐ | Already built — spec 06 §6.5 `typeOverrides` IS the asked-for semantics | **Accept**: reply pointing to §6.5 |
| 2 | #5: "分组" as FIRST tab | 3/4 ⭐ | Rejected — violates one-hero-action IA (ADR-0002/0004) | **Reject** proposal item |
| 3 | #5/#6: auto-move icons into wallpaper zones | 4/4 ⭐ | Hard-rejected — spec 04 §0 owner call (No icon auto-placement); no position-write capability exists (`layout.rs:149` read-only, no SetItemPosition anywhere); no CAS anchor/restore envelope for positions; XL+fragile | **Reject** proposal item |
| 4 | #5: conditional predicate groups (name/readonly…) | split | The ONE genuine gap. Standards: architecturally correct v2 axis (scan-time predicates project onto future icons, unlike per-icon overrides — spec 07 §8.2). Eng: minimal form = generalize `typeOverrides` key space, portable predicates only (glob/ext/kind), never readonly. UX: opposes any rule engine for the mass segment; minimal alt = canvas multi-select → batch per-icon override. PM: park | **Defer**: v2 parking lot (ADR-0017), design-panel review required before any build |
| 5 | #6 premise "auto-format exists but broken" | 4/4 ⭐ | Factually wrong — M7 decision core built + Mac-hardened; Windows platform bodies (watcher→reconciler→driver) still unwritten. Roadmap item, not a bug | **Accept**: reframe issue as M7 tracking + clarify |
| 6 | #6 option 2: desktop shell context menu | 4/4 ⭐ | Rejected — tray is the only resident surface (spec 07 §1/§12); shell-ext = Win10/Win11 dual model, in-proc explorer DLL, AV/EDR exposure, uninstall residue | **Reject** proposal item |
| 7 | #6 options 1/3 | 4/4 ⭐ | Already ruled by ADR-0022 consent ladder (batched proposal + 2h timeout + OS toast default; pure silent = second-tier opt-in, never default) | **Accept**: point to spec 07 |
| 8 | #6 spin-off (UX seat): visible 「立即整理新图标」 entry in the icons panel (today tray-only) | UX | Real discoverability gap | **Accept**: adopt as panel affordance when M7 lands |
| 9 | #7 combined theme-pack export | 4/4 ⭐ | Direction correct and ALREADY reserved (spec 09 §2 `entries[]`, `type:"wallpaper"` reserved). Gates: wallpaper payload writer/reader (v1 non-scope) · #5 group model finalized · schemaVersion bump. Portability red line: per-screen zone geometry + machine-bound predicates never export as-is | **Accept** direction + invite contributor toward the wallpaper-payload gap |
| 10 | #3 re-edit target model | PM+UX independently identical ⭐ | Model A (continuation): base image = ORIGINAL wallpaper snapshot forever; zone OBJECTS restored editable; "start over" stays an explicit in-editor verb (clear / apply preset / 换回我的壁纸). Toggle (C) = permanent IA debt solving nothing (A→B is one action; B→A destroys labor) | **Accept Model A**, no toggle |
| 11 | #3 root cause (Eng forensics) | Eng | Editor working source = `GetWallpaper()` (`topology.rs:86` ← `wallpaper.rs:108`) which post-apply returns the baked `current-bake.png` → old zones arrive as PIXELS under persisted zone objects. Original exists only in the restore snapshot (`wallpaper.rs:45-58`), never served as edit base. Same root-cause family as icons styled-residue: our own output mistaken for original input; wallpaper lacks the `contains_path`-style provenance guard the icons side has. Fix: getScreens serves snapshot original + is-our-bake provenance guard; size M; `[WINDOWS-VERIFY]` | **Accept** fix direction (schedule via normal process) |
| 12 | #4 hi-res/hi-scale small editor icons | Eng | "Correct but small", not a fractional-DPR math bug: 3200×1000 @150% → logical height ≈667 < `DESIGN_H=832` pins √fit auto-zoom to 1.0 (`lib.rs:445-446`); icons-mirror hardcodes height-fit (`icons-mirror.tsx:78`, `canvas-view.ts:103`) → 48px icons render ≈34 CSS px. Fix reco: edit-time focus-zoom preview (S–M, web-only) + icon-legibility zoom floor (S); touching the √fit curve itself flagged high-risk, not recommended | **Accept** diagnosis (fix scheduling via normal process) |

## Sequencing (PM+Eng ⭐)

None of #5/#6/#7 should take engineering time now — the write-surface reliability work
(vanish-class et al., STATE.md) remains the dominant ship risk. #6 = finish M7 as planned.
#5 net-new + #7 land after 1.0 / behind their gates. If fork PRs arrive: require spec-scoped
splits (payload widening+migrations · dual-resolve parity · reconciler projection · UI),
position-write changes physically separated, each PR states touched spec sections + gates
run + the 5 owner invariants self-check; adversarial cross-review on ledger/CAS/parity.

## v2 parking lot additions

- **Conditional predicate groups** (from #5): reframe = generalize `typeOverrides` closed
  3-bucket key space → ordered scan-time predicate list, INSIDE the ADR-0017 findability
  envelope (shape+saliency+bounded plate; filter/Original stay global-only); portable
  predicate whitelist only (name glob/ext/kind); evaluation results never enter the ledger.
  Unresolved clash to settle at design review: predicate list (Standards/Eng) vs named
  multi-select sets / batch per-icon override (UX minimal alternative).
- **Zone↔group visual echo** (accent affinity only, never position) — noted, unscheduled.

## Contributor reply drafts (owner-approved strategy: warm on bugs, honest citations on
enhancements, redirect energy to the wallpaper-payload gap; posted replies live on GitHub)

See the issue threads once posted; drafts were prepared in the same session as this record.
