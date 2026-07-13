# Plan — 清爽 Module (Calm Windows) Build

Executes ADR-0023 + spec 08. Panel: `docs/reviews/2026-07-13-calm-windows-panel.md`.
Capability truth: `docs/references/windows-settings-rust/README.md`. Owner working model:
Mac-closable first; Windows = integration + [WV] pass only.

## Global constraints (bind every task)

- Coral-only accents; no security palette; files ≤500 lines; no dashes in user copy;
  module copy bans 净化/清理/优化/加速/扫描/问题计数 (spec 08 §9 table is binding).
- Guided rows: no toggle affordance anywhere (incl. a11y tree). `已生效` only via
  pending→verified. Fail-closed default for unknown environments.
- Reference crates under `docs/references/windows-settings-rust/` are copied by boundary,
  NEVER added as dependencies. `dm_domain::RegistryValue` is NOT reused (research README).
- Every task: `bun test` + `tsc -b` (web) / `cargo test --workspace` + msvc cross-check
  (Rust) green before commit; bug fixes ship regression tests.

## Wave 0 — web module skeleton (Mac mock loop; STARTED 2026-07-13)

Vertical slice: the module exists end-to-end in the browser loop against a fake backend,
with the honest-state grammar fully unit-tested. No bridge schema change yet (Wave 1 does
that deliberately) — the store talks to a `CalmBackend` port with a mock implementation.

| # | Task | Files | Verify |
|---|---|---|---|
| 0.1 | Catalog + state machine (pure TS): control ids (starter slice + guided + held rows), surface groups, tiers, per-control states (`quiet/pushing/pending/verified/setAwaiting/needsManual/unsupported/managed/needsReconfirm/userAttested/confirmedOff`), legal transitions, admission-rule flags | `src/lib/calm/catalog.ts`, `src/lib/calm/states.ts` | `tests/calm-catalog.test.ts` (bun) |
| 0.2 | `CalmBackend` port + mock backend (probe/apply/restore/reProbe with fake latency + a configurable fake environment incl. managed/uncertified tuples) | `src/bridge/mock-calm.ts` | store tests drive it |
| 0.3 | Zustand store: probe → three groups; hero apply (batch → per-row pending → verified/setAwaiting), exclusions (remembered), restore w/ skip-with-reason, refocus re-probe for guided rows | `src/stores/calm.ts` | `tests/calm-store.test.ts` |
| 0.4 | Module page (Direction B): hero strip + 「一键就能帮你关的」 rows (exclusion toggles) + 「带你去系统里关的」 action rows (widgets first) + collapsed 「这个 Windows 版本暂时不碰的」 + restore text-button; ConfirmSheet explain-before-apply; toast on partial | `src/components/panels/calm-page.tsx` (+ small row components inside, split if >500 lines) | manual browser pass + tsc |
| 0.5 | Shell wiring: `AppModule` union + rail 4th tile (coral craft glyph, NOT shield) + Ctrl+4 + mounted-module slot | `src/stores/app.ts`, `src/components/shell/module-rail.tsx`, `src/App.tsx` | tsc + existing tests |
| 0.6 | i18n keys (zh-hans + en) per spec 08 §9 copy table | `src/lib/i18n/zh-hans.ts`, `src/lib/i18n/en.ts` | 0.7 gate |
| 0.7 | Copy gate: banned-word + precision assertions over `Calm_*` keys (pattern: `tests/banned-colors.test.ts`) | `tests/calm-copy.test.ts` | bun |

Exit gate W0: `bun test` + `tsc -b` green · rail shows 4 tiles · module fully drivable in
`bun run dev` · codex adversarial review over the diff · designer-seat acceptance queued for
the visual pass (scaffold ≠ accepted look).

**W0 status (2026-07-13):** built (`cede075`+`2476d4b`), browser-verified end to end
(evidence `docs/plans/evidence/2026-07-calm/`), codex R1 Request-Changes (2 Block +
15 Major + 1 Minor) → **ALL FIXED in `47f99ac`** (external state for drifted restores ·
ledger-owned probe channel `probeTransition` · one op lock · stranded-pending recovery ·
`skipped` outcome · consent sheet lists names · held group collapsed with per-row reasons ·
honest hero phases · full-outcome toast · widgets family row · strengthened copy gate;
the Minor resolved by rewording the two hyphenated en strings). Gates re-run green
(tsc · bun 566). **Owed:** component-render/a11y test infra does not exist in this repo —
adding happy-dom/testing-library is an OWNER dependency decision; until then the honest
grammar is pinned at state/store level + browser E2E + designer acceptance.

**W0.5 polish DONE (2026-07-13, owner complaint → 3-seat panel → redesign → codex R2 closes →
designer acceptance PASS):** `docs/reviews/2026-07-13-calm-page-polish-panel.md`. Surface
glyph pins + place tags (the WHERE axis), hero constellation band, cardtitle group headers +
subtitles, located result-sentence descs, three-line consent, inclusion checkboxes,
lost-reply=unknown+reprobe, skip reasons, strengthened copy gate. Gates green (tsc · bun 568).
Open owner calls O1-O3 in the review record; motion pass + hero OS-mirror deferred with
re-acceptance booked.

**W0.6 viz DONE (2026-07-13, owner escalation ×3 → real-screenshot redraw → top-UX
acceptance FAIL→fix→PASS):** per-row 104×64 schematics drawn from downloaded real Win11
screenshots (schematic panel ruling O4 `docs/reviews/2026-07-13-calm-schematic-panel.md`);
honest-motion vocabulary NoiseGroup/ReflowGroup/ShrinkRect (no hollow sockets — surfaces
compact or shrink like the real desktop); done-state ghost outline removed; task-view glyph
per owner pixel description; per-row 「恢复」; shared `FullPage` shell unifies 清爽/设置
title geometry; group-1 subtitle discloses untick-to-skip. Spec 08 §2/§2.1 amended.
Acceptance verdict PASS (start-menu P1 reflow hole fixed in `9e17d26`, copy↔picture
contradiction resolved by keeping the your-files row). Gates green (tsc · bun 575).

**W0.6-viz-r2 DONE (2026-07-13/14, owner double complaint → typography ladder +
taskbar re-centre + codex R3-R6 adversarial loop):** owner: wall-of-equal-text +
"survivors must re-centre like the real centre-aligned taskbar". Type ladder
18(hero)>16(group)>14(row name, medium)>12(desc, t3)>11(meta); cluster axis moved to
x54.5 (weather↔tray midpoint), survivors shift ±half the freed width — designer
pixel re-acceptance PASS (1.01:1 / 1.00:1 symmetric margins). codex R3 (1🔴+5🟠:
§6 HealthCheck drift notice unimplemented, quiet ghost frame, sweep ignored
reduced-motion, walk-token race, teal synced vs §10 coral, tray copy↔picture) →
R4 (reopened parallel-array crash → 'reopened' becomes a REAL CalmRowState; mock
drift = one-shot flip; same-row probe race; tray region hugged) → R5
(RESTORABLE_LEDGER keeps Restore alive when every write drifts; ONE actionable
reopened set feeds notice/button/hero — synced unreachable while any row sits
reopened) → R6 (celebration: spec §4 launch-first gate via claimCelebration;
applyAll returns THIS call's summary, null on lock/no-op — stale-lastApply
confetti race dead; owner-ordered confetti rides the shared icons/wallpaper
module). Gates: tsc · bun 597. codex R7 verification in flight.

## Wave 1 — Rust decision core + bridge (Mac)

Copy the reference boundaries into production per spec 08 §12: `crates/dm-domain/src/
system_tweaks.rs` (ids/environment/states/anchors/ports — new types, NOT RegistryValue) ·
`crates/dm-operations/src/system_tweaks/` (catalog resolver, WAL/ledger on the existing
rusqlite spine, apply/restore/recovery driver, verification-receipt model, fakes + kill-point
battery) · `crates/dm-contracts/src/tweaks.rs` (thin DTOs; **bridge schema 7→8**) ·
`src-tauri/tweaks_host.rs` + devhost fake backend (mirrors icon_host/devhost_icons pattern) ·
regenerate `src/bridge/generated.ts`; swap the store's mock for bridge verbs under Tauri
(browser keeps the mock). Exit: cargo workspace green + drift-guard + bun/tsc; codex review.

## Wave 2 — Windows platform layer (Mac blind-write, msvc-clean)

`crates/dm-windows/src/system_tweaks/` — winreg raw CRUD backend (exact kind/bytes, 64-bit
view, fail-closed on extension types), `WindowsSystemProfileProbe` (RtlGetVersion + UBR +
identities cross-check), refresh adapters (bounded WM_SETTINGCHANGE, guarded ms-settings
launch), per-setting PolicyStateProbe implementations. All `[WINDOWS-VERIFY]`; extend the
m34-windows-blind checklist. Exit: msvc cross-check clean, Mac fakes green.

## Wave 3 — Windows box: certification lab + [WV] battery (release gate for the write slice)

Build the VM manifest for the starter slice (`SearchboxTaskbarMode`, `ShowTaskViewButton`,
`Start_IrisRecommendations`) × release build families (README §Required Windows lab matrix);
implement the typed effect verifiers + delayed read-back; run the inspect→apply→verify→
reboot→restore ladder; populate the allowlist. **ADR-0023 D2 gate decision happens here:**
lab green → write slice rides v1; else v1 = guided-only face. Never implement build ≥ 26100
inference; unknown tuples stay fail-closed.

## Wave 4 — post-v1 (roadmap §Next)

Direction-A noise-map canvas · additional certified rows · per-item-consent back room
(不可评估 door) · machine-level policies (advanced, HKLM).

## Standing review loop

Every wave lands through: self-review → codex adversarial review (`/multi-ai`, Request-
Changes cycle to convergence) → designer-seat pixel acceptance for anything visual → gates
green → STATE.md checkpoint (sweep to journal per doc-structure).
