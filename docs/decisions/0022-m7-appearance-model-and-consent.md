# ADR-0022 — M7 appearance model, trust-first reset, and the resident consent model

- **Status**: Accepted (owner, 2026-07-12)
- **Relates to**: ADR-0019 (Tauri/Rust replatform, native renderer), ADR-0020
  (background resident v1.0, incremental ledger), ADR-0021 (global arrow overlay
  default). Normative behaviour: **spec 07** (this ADR's decisions are integrated
  into spec 07 §2, §8–§14).

## Context

M7 (background resident auto-format, ADR-0020) cannot ship without answering four
questions a three-seat panel (PM/UX/architect, two rounds) converged on and the owner
then ruled on:

1. What does "version" mean once icons are being added and removed automatically —
   a desktop snapshot, or something else?
2. What does "reset to original" do to icons the user has since hand-edited?
3. Is a purely silent background icon rewrite an acceptable v1 default?
4. Can resident automation be enabled before the user has ever run a manual Apply?

Each has a plausible-but-wrong default that either destroys user data, produces a
confusing "why did my icon set change" support burden, or triggers "this app secretly
rewrites my files" trust failures. The owner approved all four panel recommendations
on 2026-07-12; this ADR records them so a future contributor cannot re-litigate a
solved problem without first reading why.

## Decision

1. **`version` = appearance preset, not desktop snapshot — a three-store data
   model.**
   - Store ① is the existing active ledger (`ledger.json`) — the only reversible
     truth, one row per icon, never capped.
   - Store ② (NEW) is a single `icon_style_json` row on `SettingsStore` — "the
     current global style," written ONLY on a completed global Apply.
   - Store ③ (NEW) is `LookHistoryStore` (`look-history.json`, its own
     corruption-tolerant file) — up to 10 named/timestamped style recipes a user can
     switch back to. **It never stores an icon list.**
   - Switching to a saved version PROJECTS that style onto the live desktop scan at
     switch time (spec 07 §9), rather than replaying a recorded icon set. An
     incremental auto-format writes ONLY store ①, so it never creates a
     version-history entry.
   - Full model: spec 07 §8.

2. **Reset is trust-first, not a literal clobber.** `reset_to_original` restores
   only icons still exactly in the state DeskMakeover last left them (CAS match
   against `last_applied_fingerprint`); icons the user has hand-edited since are
   LEFT UNTOUCHED and the skip is reported ("已跳过 N 项(你自己改过)"), never
   silently reverted. A reset additionally, in the same operation, clears store ②,
   turns off the auto-format toggle, and restores the machine-level arrow overlay
   (one batched UAC, ADR-0021) if it was ever enabled — omitting any one of the
   three would leave a state that looks reset but is not. Full model: spec 07 §10.

3. **The v1 default for new-icon auto-format is a batched proposal with a timeout,
   backed by native OS toasts — never pure per-icon silence.** New icons are grouped
   into a proposal with a default 2-hour auto-apply timeout; feedback is tiered
   (always-on environment cues → native-toast-with-undo for the first 3 batches →
   environment-only after 3 un-undone batches, resetting on any undo; anomalies
   always surface the toast tier). A batch is additionally downgraded from silent
   back to a proposal if the saved style is stale (>60 days) or the user has been
   partially reverting icons since the last Apply ("intent freshness," spec 07 §2).
   Pure per-icon silent mode remains available as an explicit second-tier opt-in,
   never the default. Rationale: the "user didn't object, so they wanted it"
   argument requires the user to have been able to *notice* the change; pure
   silence removes the premise the argument needs.

4. **Resident automation cannot be enabled before one successful global Apply.**
   The toggle is disabled at both the UI and the settings-patch layer while store ②
   is empty. This makes "a saved style exists" an invariant for every downstream
   automation code path, eliminating an entire class of null/empty-style edge cases
   rather than special-casing them.

## Consequences

- Spec 07 is updated in the same change as this ADR: §8 (three-store model +
  confusion resolution + action/state-coupling table), §9 (version-switch
  projection algorithm), §10 (reset), §2 (trust model / consent ladder rewrite),
  §14 (privileged queue, restated as an engineering constraint), §11–§13/§15–§17
  (activity detection, tray state machine, reversible touchpoints, performance,
  dependencies, housekeeping) all trace back to this ADR or to ADR-0020/0021.
- `dm-operations` gains the `icon_style_json` `SettingsStore` column and a new
  `LookHistoryStore` — both are Wave B prerequisites (plan
  `2026-07-12-m6-wire-host.md`) before any M7 code lands (plan
  `2026-07-12-m7-resident.md`).
- The UI terminology law (spec 07 §8.1: 外观/外观方案 only, never
  版本/快照/回退/时光机) applies to every future string touching this feature; a
  version's thumbnail must be a rendered style sample, never a historical
  screenshot.
- Per-icon overrides inside a saved appearance version are explicitly out of v1
  scope (spec 07 Non-scope) — an `overrides: Map<ItemId, Patch>` extension to
  `IconStyleDto` is a v2 decision, not implied here.
