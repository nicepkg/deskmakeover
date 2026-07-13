# 2026-07-13 — 「清爽 Windows」能力设计专家团（四席两轮对抗）

**Artifact under review:** how the calm-Windows capability family (disable Windows
ads/recommendations — `docs/references/windows-settings-rust/` handoff, 20 controls in
4 tiers) enters the product: IA home, experience model, scope, naming, timing.
**Trigger:** owner order 2026-07-13 — integrate these capabilities; panel to debate
"new page vs. extend the settings page"; owner explicitly invited hard pushback.
**Seats:** 首席 PM · 首席产品设计师（概念模型/交互）· 首席 UI/视觉 · 首席 UX 工程师 —
four isolated same-vendor subagents (Fable 5), fresh context each, no owner-preference
leakage, artifacts given as file paths (research README + ADR-0004 + specs 00/01/02/03/06 +
ADR-0009 + spec 07/ADR-0022). Round 1 independent; round 2 adversarial cross on the two
genuine clashes (PM ↔ UX).

---

## Verdict in one line

**Unanimous (4/4): this is a new rail MODULE — not a settings-page section, not a merge
into the icons hero flow.** The owner's original binary ("new page vs. settings page")
was rejected by every seat: the settings page is app meta-config; a capability with a
hero action, snapshot/restore, drift-health and celebration is a beautify module by
ADR-0004 §3's own definition. The real design problem the panel surfaced is different:
**the research's 4-tier reality (automatic / advanced / guided / invariant) breaks the
homogeneous "已帮你做的事" checklist grammar that ADR-0004 §2 assumed** — the module
needs a three-state honest results grammar of its own.

---

## Round 1 — seat digests

### 首席 PM
- Direction: rail module, visible-noise automatic slice only (≤6 items); guided = quiet
  deep-link escape hatch, never module rows; invisible privacy items excluded (anti-优化大师).
- P1-1 governance bug: ADR-0004 §6 ("ships in v1.0") vs roadmap re-slice ("post-first-release,
  requires Modules.Contracts") — two live docs contradict; must be adjudicated before anyone builds.
- P1-2 guided tier fails the module admission test (not our reversible act; adds a step).
- P1-3 not in first release (cert lab empty; first release already loaded; blast radius
  during the trust-building window). *(revised in round 2 — see below)*
- P2: "automatic candidate ≠ shippable" without effect-verifier + HealthCheck; 20-item
  framing is scope-creep bait; anxiety-free copy; name in 清爽 register, never 净化/清理.
- Boldest pushback: the brief's verb "整合" presupposes all 20 enter; the real question is
  which SUBSET deserves the 美颜 brand. Differentiation lives in icons/wallpaper; ad-removal
  is a red ocean that erodes trust — don't let feature-table anxiety re-order the moat.

### 首席产品设计师
- Core: the users' mental verb is **收起打扰/让它安静 (boundary-setting), not 美化
  (authoring)** — same brand umbrella, but the module must be allowed its own honest verb.
- Direction A: "noise map" canvas — labelled schematic Windows surfaces with status chips
  (会推送 / 已安静 / 需你自己关 / 此版本暂不支持); the map IS the preview; never fake
  screenshots. Direction B (ship first): 3 outcome-named groups ("一键就能帮你关的 /
  带你去系统里关的 / 这个版本暂时不碰的") — grouping itself is the honesty. B→A shares
  one three-state data model; incremental, no rewrite.
- P1-1 HealthCheck contract conflict: ADR-0004 §3 says re-apply; research says a feature
  update invalidates certification and must never auto-replay → redefine this module's
  HealthCheck as **re-detect + re-PROPOSE**, never silent re-apply (spec 07 §2.5 precedent).
- P1-2 「已帮你做的事」 cannot honestly hold guided (a TODO we can't even verify) or
  fail-closed (things we did NOT do) items → hero checklist counts only verified writes.
- P1-3 the one-click promise breaks on its flagship item (widgets feed = guided) → make the
  guided widgets handoff the FIRST-CLASS opening act, not a footnote.
- Boldest pushback: ADR-0004 §6's "clean HKCU one-shot list" (07-05) is partially fiction
  against the research tiers (07-11) — lock-screen tips are ADVANCED, search-only-local is
  ★★☆☆☆; ship the honestly-smaller automatic set instead of the ADR's ideal list. The
  admission test's "adds no step" clause needs a written guided exception: hero one-click
  fires automatic-certified switches only.

### 首席 UI / 视觉
- Direction A (north star): "Windows surface mirror" canvas — stylised Start/search/
  lockscreen/taskbar miniatures (taskbar-strip.tsx precedent) where the one-click visibly
  slides the search box out, fades the recommendation rows; Space-hold = compare to
  un-cleaned Windows (zero new grammar). Direction B (v1 shippable skin): 3 InspectorCard
  surface groups + module-rows + tiny static before/after thumbnails; same skeleton, zero
  rework to grow into A.
- P1: organising "20 toggles" is unconstitutional per ADR-0004 §2 (exclusion list, not
  selection menu) — the design win is refusing the toggle grid itself. Guided/unavailable
  items must never be dead toggles (text-link action rows; existing 40%-opacity
  disabled-slot grammar + quiet caption). Security-palette (green shield / red alert /
  blue guard) would break both brand and the banned-colour gate — coral-ink checks only.
- P2: rail glyph must be a coral craft glyph (never a shield / OS blue); rail active-wash
  spring needs re-testing at 4 tiles; naming/copy must never drift to 清理/优化/加速/扫描;
  mirror carries only visible-delta surfaces — invisible items go to a separate
  「看不见但更清净」 group in plain words; toggle rows need pending→verified motion states
  (cta-working shimmer → verified check), restart-needed disclosed in caption.
- P3: reuse the existing ceremony components (ConfirmSheet, once-per-launch confetti —
  second module success degrades to toast + 「去看看开始菜单」).

### 首席 UX 工程师
- Core: heterogeneous engineering truth vs homogeneous checklist grammar is the root
  trust risk. Three honest states: 已生效 (delayed + functional verification passed) /
  已设置·重启后生效 / 需你手动·冲突·此机型暂不支持.
- P1: guided items rendered as toggles = false affordance lying about their own identity
  (README: no stable setter; ADR-0009 §8 honesty precedent) → visually distinct action
  rows. Merging into 一键美化 would pollute the icons module's clean instant-WYSIWYG trust
  with delayed/uncertain feedback.
- P2: 「已生效」 must bind to the effect verifier, never to write-success (README step 8:
  registry match ≠ surface reloaded); Windows feature updates silently roll back →
  startup HealthCheck re-check must ship WITH the module; copy-precision table (hide
  search box ≠ disable search; ad ID = "减少个性化追踪" never "减少广告数量") enforced by
  a test gate like banned-colors; disabled states always carry reason + alternative exit.
- P3: guided round-trip gulf of evaluation — re-probe on window refocus; readable-state
  items auto-confirm 「已确认关闭 ✓」; unreadable ones ask, and are recorded as
  「你手动关的」(user-attested), never "we did it".
- Boldest pushback: the "one-click" frame itself half-lies for this module — rename the
  click honestly ("一键关闭我能安全关的") and treat the guided walkthrough as the module's
  REAL added value (Windows hid the switch; we take you to it). "随时完整还原" slightly
  over-claims here (re-enabling ad ID mints a NEW id) — restore copy = 「把这些开关调回
  Windows 默认」 + per-item asymmetry warnings.

---

## Round 2 — adversarial cross (the two genuine clashes)

### Clash A: does the module enter the first public release?
- **PM revised** (from "never in v1"): the Modules.Contracts prerequisite is weaker than
  claimed — v1's resident mode (dm-resident + dm-operations WAL/ledger/CAS) already builds
  the module spine, and the research's production split reuses exactly that pattern; the
  REAL gate is the Windows-VM certification lab, which no other v1 work incidentally
  produces (icon/wallpaper writes are not build-version-sensitive; calm-settings recipes
  are build×edition×geo×arch×managed×packaged-sensitive). Final: **capability-gated, not
  calendar-gated** — the module may ride v1 iff (i) it rides the resident/ledger spine,
  (ii) slice = 2-4 highest-confidence visible items (SearchboxTaskbarMode,
  ShowTaskViewButton, Start_IrisRecommendations), (iii) recertification-style startup
  HealthCheck ships with it (in-boundary drift → re-close with notice; out-of-boundary
  feature update → never replay, honest re-confirm), (iv) uncertified tuples fail closed
  to guided, (v) launch copy promises only the certified slice. If 2-4 items × 3 builds
  can't be certified inside the Windows integration phase, it slips.
- **UX revised** (from "must ship v1 with HealthCheck"): by his own round-1 acceptance
  bar, the write tiers are unshippable today — README self-declares no production effect
  verifiers, no populated cert manifest, and "a successful raw registry read-back alone
  does not make any initial recipe writable"; ADR-0004 §6's v1.0 decision (07-05) predates
  the evidence (07-11) that falsified its "HKCU one-shot = homogeneous ready" premise.
  Final: **write tiers (automatic + advanced) do NOT enter the first release; writes ship
  when cert lab + effect verifier + drift recheck all turn green (capability gate).** But
  the module must not be rejected as one block: the guided-only 「教你关」 surface has
  zero registry writes / zero cert dependency / zero blast radius and covers the single
  most-hated item (widgets feed) — evaluate IT for v1 on its own low-risk profile; its
  only real cost is polish focus, which is the owner's call.
- **Convergence:** both seats land on a capability gate. Residual owner choice = appetite:
  hold v1 for the small certified write slice (PM) vs. ship guided-only first and let
  writes follow certification (UX).

### Clash B: invisible privacy items (ad ID / Device Usage / sync notifications)
- **PM revised** (from "exclude the whole class"): concedes the inconsistency — ad ID is
  A-level, programmatically verifiable (AdvertisingId becomes empty), higher engineering
  confidence than the search-highlights item he kept; an honestly-named disclosed group is
  NOT an 优化大师 wall (that is defined by scan/anxiety framing + unverifiable pseudo-gains).
  New axis: **verifiable effect + clean admission** → ad ID in (separate group, copy gate),
  Device Usage held (advanced, empty allowlist, composite fragility, "does not disable
  telemetry or all ads"), sync-provider notifications held (suppresses legitimate OneDrive
  status notices — real collateral needing disclosure).
- **UX revised** (agrees with exclusion, swaps the axis): the bucket is heterogeneous —
  the correct axis is **user-evaluability**, not the "privacy" label. Ad ID is
  double-killed: effect never user-perceivable (violates his own evaluability bar; also
  outside ADR-0004 §1's "visual noise" charter) AND re-enable mints a new ID → fails the
  admission test's "one-click reversible with zero residue". Sync notifications are
  visible noise the user CAN see → belong in the main flow. Final: an **evaluability
  gate** — controls whose effect the user can never perceive are excluded from the default
  package; a per-item consent back room may exist later, its door labelled
  "不可评估", not "隐私".
- **Convergence (composed admission rule for the DEFAULT one-click package):** a control
  enters the default package only if **(a) user-perceivable, (b) effect-verifiable,
  (c) zero-residue reversible, (d) no undisclosed legitimate-content collateral.**
  → Ad ID: out of default (fails a+c) — back-room candidate at most, copy-gated.
  → Device Usage: held (fails a partially, b today).
  → Sync-provider notifications: visible + automatic-tier, but carries collateral (d) —
  in the module as a disclosed row; whether it sits in the default package or per-item
  is an owner sub-call.

---

## ⭐ Cross-seat consensus (≥2 seats independently, all four in most cases)

1. **IA: new rail module** (4th tile), never the settings page, never merged into the
   icons hero flow. Settings page stays meta-config. (4/4)
2. **v1 form = Direction B** (honest outcome-grouped list, ≤3 groups, reusing hero CTA /
   module-row / ceremony / disabled-slot grammar); **north star = Direction A** (schematic
   noise-map / surface-miniature canvas, visible-delta animation, never fake screenshots);
   one shared three-state data model, B→A incremental. (4/4)
3. **Guided items are never toggles** — visually distinct 「带我去关」 action rows; never
   counted in 「已帮你做的事」; the widgets-feed handoff is a first-class opening act
   (the module's real added value), not a footnote; hero one-click covers
   automatic-certified switches ONLY. (4/4)
4. **已生效 binds to effect verification**, with a pending intermediate state; write
   success alone never lights success. (4/4 via P2-4/F7/P2-5/三态)
5. **Fail-closed / managed / EEA degraded states framed as OUR restraint** ("为了不动坏
   东西，这几项我们还没在你的 Windows 版本上验证过"), quiet visuals, reason + exit,
   never security palette, never anxiety counts. (4/4)
6. **HealthCheck for this module = re-detect + re-propose** (in-cert-boundary value drift
   may re-close with an honest notice; feature-update boundary crossings never auto-replay).
   Shipping the module without a startup drift re-check is a silently rotting promise. (3/4)
7. **Copy-precision table + test gate** (per-README caveats: hide ≠ disable, tracking ≠
   ad count, Start promo ≠ all promos; bans: 净化/清理/优化/加速/扫描/问题计数; no dashes)
   extending the existing banned-colors/copy gate pattern. (4/4)
8. **Restore asymmetries disclosed** (ad ID new-ID; drifted values skipped-with-reason;
   restore copy = 「恢复系统推送 / 调回 Windows 默认」, not "复原"). (3/4)
9. **Scope: design the certified visible-noise slice, never the 20-item wall** — the
   research enumerates capability space, not a shipping list. (3/4)
10. **Naming in the 清爽/收起打扰 register** under the 美颜 umbrella; the module speaks
    its own honest verb; 净化/清理/优化 banned. (4/4 — UI keeps the umbrella framing,
    3 seats push the distinct verb; compatible.)

## Owner disposition table

| # | Decision | Panel recommendation | Owner disposition |
|---|----------|----------------------|-------------------|
| D1 | IA home + v1 form | New rail module; ship Direction B skeleton, hold Direction A as north star | **APPROVED as recommended** (owner, 2026-07-13) |
| D2 | Release timing | Capability gate, not calendar: certify 2-4 visible items during the Windows integration phase → small write slice rides v1 (PM); if the lab can't turn green in time, v1 ships the guided-only 「教你关」 surface (zero-write, covers widgets) and writes follow certification (UX fallback) | **APPROVED as recommended** (owner, 2026-07-13) |
| D3 | Invisible items | Composed admission rule (perceivable + verifiable + zero-residue + disclosed collateral) for the default package: ad ID & Device Usage out of default; sync notifications in as a disclosed row | **APPROVED as recommended** (owner, 2026-07-13) |
| D4 | Governance repairs | (i) annotate ADR-0004 §6 ↔ roadmap timing conflict once D2 is ruled; (ii) write the guided exception into the §5 admission test ("hero one-click = automatic-certified only"); (iii) record this module's HealthCheck = re-propose semantics | **APPROVED (follow-through of D1-D3)** — executed via ADR-0023 + ADR-0004 amendment |
| D5 | Module name | 清爽-register working name (rail label 清爽; full name candidates: 清爽系统 / 收起打扰) — owner names it | **APPROVED: rail label 清爽, full name 清爽系统** (owner, 2026-07-13) |
| D6 | Discovery moment | Keep the constitution (discovered after first success, never a first-screen menu) but surface it at the FIRST module's success moment; owner may also consider a first-run "先做哪件" choice | **ADOPTED per recommendation** (not explicitly ruled — owner may veto; recorded in ADR-0023) |

**Ruling record:** all four asked decisions (D1/D2/D3/D5) approved exactly as recommended.
Decision ADR: `docs/decisions/0023-calm-windows-module.md`.

*Raw seat reports live in the session transcript (2026-07-13); this file is the durable
digest + disposition record. Update the Disposition column when the owner rules.*
