# 0023. 清爽 (Calm Windows) Module: IA, Honest-State Grammar, Admission Rule, Capability-Gated Release

**Status:** accepted (owner dispositions 2026-07-13)
**Panel record:** `docs/reviews/2026-07-13-calm-windows-panel.md` (4 seats, 2 adversarial rounds)
**Capability truth:** `docs/references/windows-settings-rust/README.md` (2026-07-11 research handoff)

## Context

The owner ordered the calm-Windows capability family (disable Windows ads/recommendations:
search highlights, Start promoted recommendations, widgets feed, lock-screen tips,
notification/settings suggestions, advertising ID, taskbar chrome, etc.) integrated into the
product, and asked a four-seat panel (chief PM / chief product designer / chief UI / chief UX
engineer) to debate "new page vs. extend the settings page". The research handoff tiers the
20 controls: **automatic** (registry write, reversible, per-environment certification) /
**advanced** (empty initial allowlist) / **guided** (no stable setter — deep-link to
`ms-settings:` only; includes the widgets feed and the UCPD-protected taskbar Widgets button) /
**invariant** (never touch). The certification manifest is initially EMPTY for every direct write.

## Decisions

1. **IA: a new rail MODULE** — rail label **清爽**, full name **清爽系统** (4th tile, before
   the pinned 设置). Never a settings-page section (settings stays app meta-config); never
   merged into the icons hero flow (its delayed/uncertain feedback must not pollute the
   instant-WYSIWYG icon trust loop). Unanimous panel verdict.
2. **v1 form = Direction B, north star = Direction A.** B: an honest outcome-grouped list —
   at most 3 groups: 「一键就能帮你关的」(automatic-certified) / 「带你去系统里关的」(guided)
   / 「这个 Windows 版本暂时不碰的」(fail-closed) — reusing the existing hero CTA,
   module-row, ceremony, and disabled-slot grammars. A: a schematic "noise map" canvas
   (labelled Start/search/lock-screen/taskbar miniatures whose status chips flip and whose
   visible-delta surfaces animate; Space-hold compares; NEVER faked screenshots). Both share
   one three-state data model; B→A is incremental, no rework.
3. **Honest-state grammar (module law).**
   - Result states: `已生效` (delayed read-back + functional effect verification passed) /
     `已设置·重启或下次打开生效` / `需你手动 · 冲突 · 此机型暂不支持`. Write success alone
     NEVER lights success; a pending (shimmer) state bridges write→verify.
   - **Guided items are never toggles** — visually distinct 「带我去关」 action rows with
     deep-link + return-probe (readable state → auto-confirm 「已确认关闭」; unreadable →
     user-attested 「你手动关的」, never counted as ours). The widgets-feed handoff is a
     first-class opening act, not a footnote.
   - Hero one-click covers **automatic-certified switches only**; 「已帮你做的事」 counts
     only verified writes.
   - Fail-closed / policy-managed / EEA states are framed as OUR restraint ("为了不动坏东西，
     这几项还没在你的 Windows 版本上验证过"), quiet visuals (existing disabled-slot grammar),
     always with a reason + alternative exit; never a red warning, never a security palette.
4. **HealthCheck = re-detect + re-PROPOSE** (amends ADR-0004 §3's "re-apply is a platform
   capability" for THIS module): inside the certified boundary, value drift may re-close with
   an honest notice; a feature-update boundary crossing invalidates certification and NEVER
   auto-replays (research contract) — the module re-proposes instead. A startup drift
   re-check ships WITH the write slice (a module without it is a silently rotting promise).
5. **Default-package admission rule (composed, replaces any visible-vs-privacy axis):** a
   control enters the default one-click package only if **(a) user-perceivable, (b)
   effect-verifiable, (c) zero-residue reversible, (d) legitimate-content collateral
   disclosed**. Consequences: **advertising ID out** (fails a + c — re-enable mints a new ID)
   and **Device Usage out** (advanced tier, empty allowlist, composite fragility);
   **sync-provider notifications in** as a disclosed row (visible noise; collateral caption).
   A per-item-consent back room may exist later; its door is labelled **不可评估**, not 隐私.
6. **Release timing: capability gate, not calendar (dual track).** Build the Windows-VM
   certification lab during the existing Windows integration phase; certify the starter write
   slice — `SearchboxTaskbarMode`, `ShowTaskViewButton`, `Start_IrisRecommendations` (2-4
   items × the release build families). Lab green in time → the write slice rides the first
   public release. Not green → v1 ships the guided-only 「教你关」 face (zero registry
   writes, zero certification dependency, covers the most-hated widgets feed) and writes
   follow certification in the first update. Launch copy only ever promises the certified slice.
7. **Naming / copy.** Module speaks its own honest verb (清爽 / 收起打扰 register) under the
   美颜 umbrella. Banned in this module's copy: 净化 / 清理 / 优化 / 加速 / 扫描 / problem
   counts. Per-item precision table is binding (see spec 08 §Copy): hide search box ≠ disable
   search; ad ID = 「减少个性化追踪」 never 「减少广告数量」; Start recommendations ≠ all
   promotions; restore = 「恢复系统推送」 with per-item asymmetry disclosures. Enforced by a
   test gate alongside the banned-colour gate.
8. **Discovery** stays constitutional (after first success, never a first-screen menu) and is
   surfaced at the FIRST module's success moment. *(Adopted per panel recommendation; owner
   did not explicitly rule — may veto.)*

## Governance repairs (executed with this ADR)

- **ADR-0004 §6 timing is superseded** by decision 6 above (the v1.0-ships claim predated the
  research that emptied its premise); §6's item list is re-tiered by the research README —
  the capability boundary of record is the README + this ADR.
- **ADR-0004 §5 admission test gains a written guided exception**: "adds no step to the
  primary flow" applies to the hero one-click (automatic-certified only); guided surfaces are
  optional walkthroughs the user chooses, never auto-included, never counted as done.
- **Roadmap** re-sliced: the 「系统净化 module」 Next-entry becomes the 清爽 module per this
  ADR (conditional v1 rider + post-v1 extensions).

## Consequences

- New capability spec: `docs/specs/08-calm-windows.md` (module identity, grammar, states,
  copy table, degraded journeys, verification). Build plan:
  `docs/plans/2026-07-13-calm-windows-module.md`.
- Engineering split follows the research README's production layout (`dm-domain/system-tweaks`,
  `dm-operations/system-tweaks/`, `dm-windows/system-tweaks/`, contracts DTOs, thin host) and
  rides the v1 resident/ledger spine — the reference crates are copied by boundary, never
  depended on at runtime. The Modules.Contracts prerequisite from the old roadmap note is
  RESOLVED by this spine (PM round-2 concession).
- The Windows lab matrix (builds × editions × geography × arch × managed × packaged) becomes a
  release-gate item for the write slice only; the guided face carries no such gate.
- `Start_TrackDocs` remains globally forbidden (research invariant). UCPD is never written,
  disabled, or bypassed.
