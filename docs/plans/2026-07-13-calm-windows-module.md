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
module). R7 caught the celebration ledger still per-module against spec 02
§Ceremony/spec 08 §4 global-once → single launch flag, all three modules ride
claimCelebration() (`b68f62f`). **codex R8 = APPROVE (2026-07-13): R3→R7 all
closed, no new defects.** Designer re-acceptance PASS (1.01:1/1.00:1 centring).
Gates: tsc · bun 597. W0 is COMPLETE end to end — next: Wave 1.

**W1 STATUS (2026-07-14): decision core + bridge schema 8 BUILT + wired end to end on Mac.**
Commits `224e3e0`(dm-domain/system_tweaks kernel) → `798fc5e`(catalog + fail-closed manifest) →
`08675c7`(TweakDriver apply/restore/recover) → `2e0fb66`(dm-contracts schema-8 DTOs) → codex
adversarial loop R1-R5 (`2cafe0d`/`4906ba9`/`e4733b6`/`ec1c0df`, ~15 real transaction/policy bugs
fixed: false-verified fast path, clobbering rollback, bypassable generation guard, missing
VerificationReceipt, missing-key not fail-closed, effect-proof skipped on restore, no pre-write
re-auth, policy-managed written via Undo, restore-race permanent-block, inspect misreporting
policy as owned) → `56d40ae`(Tauri host + 6 specta commands + TauriCalmBackend + BRIDGE_SCHEMA
7→8 + regenerated bindings). Architecture: system_tweaks rides its OWN JournalStore contract (the
icon txn spine is ItemId/fingerprint-keyed — incompatible with registry snapshots), with an
unforgeable WriterLease + generation guards so a durable SQLite/WAL adapter drops in later. W1
scope (honest): value-level over pre-existing keys (no key creation), DWORD-only, in-memory
journal, devhost fakes on every platform. codex loop R1→R7 (R6 `a99b916`: recovery re-proves a restore, never launders a failed effect
proof into a no-proof disown; R7 `30a32d0` doc + **codex R7 = APPROVE**, 16 real transaction/
policy bugs fixed across 7 rounds). Gates: cargo 244 (system_tweaks 46) · dm-domain msvc-clean ·
clippy clean · tsc · bun 598 · check:bindings green · files ≤500. **W1 CLOSED (codex Approve).**
Documented coverage gap: multi-leaf restore recovery is correct by inspection but not directly
tested (all writable recipes are single-leaf today). NEXT: Wave 2 (real winreg backend).

**W2 STATUS (2026-07-14): the two Windows platform ports BUILT (Mac blind-write, msvc-clean).**
Commit `005ff6e` — `crates/dm-windows/src/system_tweaks/`: `WinregBackend` (`RegistryBackend`) +
`WindowsSystemProfileProbe` (`SystemProfileProbe`), the ports the `TweakDriver` depends on.
Decomposition (mirrors `fingerprint_surface`): a pure host-tested core (`translate.rs` +
`profile_facts.rs`, 22 Mac tests — view-flag selection, Win32 status classification, snapshot
assembly incl. `Other(raw)` extension types, CAS compare, post-write verify, environment
canonicalization, registry field decode) + a thin `cfg(windows)` FFI shell (`backend.rs` +
`profile.rs`, `[WINDOWS-VERIFY]`), so every DECISION is verified on Mac and only the raw syscalls
carry the blind risk. Design decisions: the backend uses **raw `windows-rs` registry FFI, not the
typed `winreg` layer** — `winreg::get_raw_value` returns `ERROR_BAD_FILE_TYPE` for any type above
`REG_QWORD`, so it cannot read/preserve the domain's `Other(raw)` extension types byte-for-byte
(which `accepts` must see to fail closed). `is_policy_managed` = a side-effect-free `KEY_SET_VALUE`
open-probe (denied → managed) composed with the engine's catalog `policy_guards` read; a mid-CAS
policy takeover surfaces as `ManagedByPolicy` from a write-time access denial (both intents). The
profile probe reads the fingerprint from the registry (unshimmed `CurrentVersion`) + one
`GetProductInfo` call — **no `Wdk`/`RtlGetVersion` dependency, no new Cargo features**. W2 scope
(honest): only the two decision-core platform ports. The real effect verifier + durable journal are
Wave 3, so `TweaksHost` stays devhost until then (W2 exit is standalone-adapter msvc-clean + Mac
host-tests, per the wave contract — NOT end-to-end on the real box). The refresh / `ms-settings:`
launch adapters listed for Wave 2 are DELIBERATELY deferred to Wave 3: they are host glue that only
matters once the real driver is wired, and shipping them now would be unwired, untestable blind
stubs — better wired + on-box together. Gates: cargo workspace green (dm-windows
**90**, incl. 22 new) · msvc cross-check clean · msvc + host clippy `-D warnings` clean (also fixed
two pre-existing msvc-only lints the new gate surfaced) · files ≤500. Runtime Win32 behavior is
`[WINDOWS-VERIFY]` (Wave 3 cert lab). **Codex adversarial loop: R1 = Request-Changes (2 Block + 3
Major, all verified real + fixed in `681df87`)** — Block-1: `compare_exchange` did its own post-write
readback and returned `Err` on a transient verify-read failure AFTER the write committed, so the
driver recorded a mutated leaf as rolled-back; backend now returns `Ok` the instant the write commits
(the driver's own post-write read + the engine `VerificationBackend` are the effect proof). Block-2:
`is_policy_managed` was an ACL write-protection probe over-claimed as policy detection (a GPP-written
user-writable value reads unmanaged); reframed honestly + the per-recipe guard completeness and GPP
limitation deferred to the Wave 3 cert-lab D2 gate, fail-closed until then. Major-3: `is_workstation`
/ native arch / `packaged` now from authoritative APIs (`RtlGetVersion` wProductType /
`GetNativeSystemInfo` / `GetCurrentPackageFullName`), not derived/hardcoded (+2 windows features).
Major-4/5: registry field extractors now fail closed on present-but-malformed (distinct from absent);
`decode_utf16le` uses `from_utf16` not `_lossy`. codex confirmed clean: unsafe handle ownership,
`PCWSTR` lifetimes, `Other(raw)`, undo-to-`ValueMissing` delete. **R2 = Request-Changes (R2 verified
every R1 fix correct); 1 Block + 1 Major fixed in `0a1f031`** — Block: `has_package_identity` mapped
any non-`NO_PACKAGE` status to `packaged=true` (a real API error fabricated a certification fact) →
now `Result`, only `NO_PACKAGE`→false / `INSUFFICIENT_BUFFER`&len>0→true / else→Err (probe fails
closed), named `Foundation` constants. Major: R1 fixed the impl comments but the `RegistryBackend`
trait doc (`ports.rs`) still claimed `is_policy_managed` detects "a policy guard value present" — the
public contract still lied; rewrote it as the honest conservative "do not write this leaf" signal
(GPP false-negatives documented, catalog `policy_guards` = authoritative, not a provenance oracle),
kept the name (a rename across the W1-converged engine is a deferred follow-up). **R3 = Request-Changes
(R3 verified R1+R2 correct, no rename needed); 1 Block + 1 Major fixed in `a45aea5`** — the residual
sweep found two never-clobber/recovery bugs in the W1 ENGINE (surfaced by the Wave-2 honesty doc,
which now states the ACL probe is not a provenance oracle — yet the reverse paths relied on it as
one). Block: rollback / restore / crash-recovery checked only `is_policy_managed` (the ACL probe) and
bypassed the authoritative catalog `policy_guards`, so a guard appearing after apply (leaf still
user-writable) let a reverse write clobber a now-policy-owned setting → the recipe's guard addresses
are now PERSISTED with the transaction (`JournalEntry`) + anchor (`ManagedSetting`) and every reverse
path checks them via `any_guard_present` before any write (drift-immune, no recipe_version deadlock —
the persist option codex endorsed); 3 regression tests (guard before restore / mid-apply rollback /
recovery). Major: apply read-back used `?` → a committed-write-then-read-error bubbled a plain error
that hid the write from recovery; now an explicit match routes both mismatch and read-error through
`fail_apply`. Gates (clean git worktree — the main tree's dm-icon-core is mid-refactor by a concurrent
session): dm-operations **249** tests pass (incl. 3 new) · system_tweaks clippy-clean. **R4 = no new
implementation Block (R4 verified EVERY R3 fix correct); 2 Major = missing regression coverage, added
in `6c35709`** — the R3 fixes were correct but not LOCKED: (a) drift-immunity — a test now swaps the
catalog to a guard-less `taskbar.search` after apply, makes the old guard appear, and asserts restore
still blocks via the persisted `ManagedSetting.policy_guards` (catches a revert to consulting the
current catalog); (b) migrated-recipe restore still succeeds (locks the no-deadlock escape hatch); (c)
committed-write-then-read-error routes through `fail_apply`→Reverted, via a new write-count-triggered
one-shot read fault (`fail_read_after_writes`), catching a revert of the explicit match to `?`. Gates:
dm-operations **252** tests · system_tweaks clippy-clean. **codex R5 = APPROVE (2026-07-14): no Block,
no Major, R1→R4 all confirmed closed, no new transaction/fail-closed/FFI defect.** Its one observation
(the drift test covered only foreground restore) was closed with a mirror recovery-drift test
(`44c14ab`). **W2 CLOSED (codex Approve).** Final gates: dm-operations **253** tests · dm-windows host
**90** (22 new) · msvc cross-check clean · msvc+host clippy `-D warnings` clean · files ≤500 — all
verified in a clean git worktree (the main tree's dm-icon-core is mid-refactor by a concurrent
session; my commits stayed literal-pathspec isolated from it throughout). 5 codex rounds, 8 real
defects fixed (2+3 ports · 1+1 engine never-clobber) + 7 regression tests locking the contracts.
NEXT: Wave 3 (cert lab = the ADR-0023 D2 gate) — real box.

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
