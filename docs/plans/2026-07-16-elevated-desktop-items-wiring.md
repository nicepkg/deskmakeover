---
plan: elevated-desktop-items-wiring
created: 2026-07-16
owner-approved: direction ("直接上全量提权", 2026-07-15)
status: in-progress
touches: dm-domain, dm-operations, dm-windows, src-tauri, dm-elevated (host-side already built 3c4b933)
---

# Wire the main app for elevated desktop-item apply/restore

## Problem

`更新桌面` (icons apply) fails for shared/all-users items (`C:\Users\Public\Desktop\*.lnk`,
e.g. Chrome). The in-process `IconApplier` writes them unelevated → `os error 5` (Access Denied)
→ the batch rolls back → "美化未完成，桌面没改动". The elevated helper (`dm-elevated`,
`apply|restore-desktop-items`) that CAN write them was built host-side (3c4b933) but the main app
never routes anything to it. This wires it, preserving the app's core promise: **full
reversibility**, crash-safe.

## Design

### Partition, not a second applier

`commit_apply` builds `ApplyRequest`s, then runs `TxnDriver`. We PARTITION by
`scope.classify(&target.path)`:

- `None` (the user's own desktop) → the existing in-process `TxnDriver` path (unchanged).
- `Some(_)` AND `kind ∈ {Shortcut, AppxShortcut}` → a NEW elevated batch (one UAC for the whole
  batch). AppxShortcut is an ordinary `.lnk`, so it maps to the helper's COM `SetIconLocation`.
- `Some(_)` AND any other kind → `conflict` (skipped, honest), as today. `.url`/folder/file on a
  privileged root are rare and use a different write mechanism; deferred + documented (§follow-ups).

Why not an `IconApplier` that routes to the helper per item? One `runas` per item = N UAC prompts.
The elevated path is inherently batch-granular, so it is its own transaction envelope beside
`TxnDriver`, reusing the SAME journal + ledger + recovery primitives.

### The elevated batch (`apply_privileged_batch`), journal order

Mirrors `TxnDriver`'s durability, batched around one helper call:

```
1. Phase-1 prepare per item (unprivileged, in-process): reuse the CAS + heal + anchor-capture
   logic (read_fingerprint == expected; capture_anchor → FileBytes). Conflict/HealedConflict skip.
2. journal TxnBegin{items}
3. per item: assets.put(ico) ; journal AssetWritten{asset}
4. per item: journal ItemApplied{ new_fingerprint = elevated.plan(item) }   ← DERIVED, pre-write
5. elevated.apply(items)   ← ShellExecuteEx runas, ONE UAC; the helper CAS-re-checks + writes +
   atomically LIFO-rolls-back on any internal failure.
   • Applied  → journal TxnCommitted ; ledger.upsert(all) ; committed += all
   • Declined → journal TxnRolledBack ; error = "elevation declined" (no ledger rows)
   • Failed   → journal TxnRolledBack ; error = "elevated apply failed" (no ledger rows)
```

`plan()` derives the styled fingerprint WITHOUT writing (Shortcut ⇒ `IconRef{path,index:0}`),
identical to what the in-process applier's `expected_after_apply` returns — so ItemApplied carries
the real post-apply fingerprint BEFORE the helper runs. This is the crux of crash-safety (below).

### Crash-safety (every window is reversible)

Recovery becomes **scope-aware**: `recover(records, reader, applier, ledger, scope)`. In
`abort_incomplete`, the `is_ours && live == new_fingerprint` arm (item still wears our style) —
which the NON-elevated applier could never revert on a privileged target — instead **rolls the
item FORWARD**: rebuild its committed ledger row from the journal (adopt the landed style), so the
desktop==ledger and the item stays reversible via the (now-wired) elevated reset. Non-privileged
items keep the existing unelevated restore.

| crash point | desktop | recovery |
|---|---|---|
| after ItemApplied, before helper runs | original | `live==original` → drop row (9bcb1a8 fix). ✓ |
| power-loss mid-helper (partial) | some styled, some original | styled+privileged → adopt forward; original → drop row. ✓ per-item |
| helper Applied, before TxnCommitted | all styled | styled+privileged → adopt forward (all). ✓ matches desktop |
| after TxnCommitted, before/mid upsert | all styled | committed txn → `reconcile_committed`. ✓ (scope-free) |
| Declined/Failed (helper self-rolled-back) → TxnRolledBack | original | clean rolled-back txn. ✓ |

`abort_incomplete` adopt-forward is safe (never claims a user edit): it fires ONLY when
`live == new_fingerprint` (proven ours). A user's own privileged edit (`live == other`) still
→ `preserve`.

### Reset (`reset_to_original`)

The §14 red-line arm (currently `skip`) collects privileged rows that are still ours into a batch;
after the ledger walk, ONE `elevated.restore(batch)` (FileBytes replay of the original `.lnk`
bytes, CAS-guarded so a user re-edit is left alone). Applied → `ledger.remove` + `restored`;
Declined/Failed → keep the row + `skipped` + a note. `Unresolved`/unwired scope or a `None`
elevated port → keep today's skip (fail-closed).

### Security — A1 + C3 signature gate

Before EVERY `runas` (overlay AND desktop-items), verify `dm-elevated.exe`'s Authenticode signer
(WinVerifyTrust) — the per-user install dir is user-writable, so a swapped helper would run
elevated. Shared `guards::verify_trusted_helper(path)`; refuse the runas on an untrusted/unsigned
signer. `[WINDOWS-VERIFY]` on the signed build.

## Ports / DTOs (dm-domain)

```rust
pub trait ElevatedIconApplier {
    fn plan(&self, items: &[ElevatedApplyItem]) -> PortResult<Vec<Fingerprint>>; // derive, no write
    fn apply(&self, items: &[ElevatedApplyItem]) -> PortResult<ElevatedOutcome>; // stage+runas
    fn restore(&self, items: &[ElevatedRestoreItem]) -> PortResult<ElevatedOutcome>;
}
struct ElevatedApplyItem   { target: ItemTarget, asset_path: String }
struct ElevatedRestoreItem { target: ItemTarget, original_bytes: Vec<u8>, applied_icon: String }
enum   ElevatedOutcome     { Applied, Declined, Failed(String) }
```

## Task list

1. dm-domain: the port + DTOs + `Fingerprint` reuse; export.
2. dm-operations `icons/mod.rs`: thread `scope` + `elevated: Option<&dyn ElevatedIconApplier>`
   into `commit_apply` + `reset_to_original`; partition; `apply_privileged_batch`; reset batch.
3. dm-operations `txn/recovery.rs`: `scope` param + adopt-forward; update ALL `recover_from_journal`
   callers (version_switch, resident reconciler, startup).
4. dm-operations tests: `FakeElevated` + partition / ledgering / Declined / Failed / recovery
   adopt-forward (run via WSL — operations tests are `#[cfg(not(windows))]`).
5. dm-windows: `WindowsElevatedIconApplier` (plan via `fingerprint_surface`, apply/restore via
   manifest + stage + runas), `guards::verify_trusted_helper` (A1/C3), wired into overlay too.
6. src-tauri: build the port (win real / dev fake), thread into `IconHost` + call sites + startup
   recovery scope; `DevElevatedIconApplier` so the dev host exercises the path.
7. Verify: cargo (win build + WSL not(windows) tests), tsc/bun unaffected, bindings unchanged.
8. codex adversarial review (spec-compliance + code-quality) → fix → re-review.

## Follow-ups (documented, out of v1)

- Privileged `.url` / folder / loose-file kinds (different write mechanism) → still conflict.
- Manifest-file TOCTOU during the UAC window is low-severity (the helper independently confines
  every target to Public Desktop/ProgramData + caps every icon); a signed-manifest channel is a
  follow-up.
- Per-item skip reporting from the helper (today all-or-nothing; only the exit code returns).
