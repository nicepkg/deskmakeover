# Windows hardening — wave 2 (apply/txn/CAS surface)

**Date:** 2026-07-11
**Milestone gate:** must land **before** the apply/restore commands are wired at M5/M6 cutover.
**Source:** independent Codex full-review of snapshot `7dc82c1` → 26 findings → triaged against
current `main` HEAD (`30081b1`) by an independent reviewer + lead re-verification.

## Why now (structural context)

The entire apply / transaction / mutation surface is **unwired in production today**:
`TxnDriver::apply` and `is_unmodified` are called only from tests, there is no production
`AssetStore` (only fakes), and `tauri.ts` routes every non-settings verb to the web mock. So the
open defects below cause **no live user harm right now** — but they are "ships-broken-when-wired."
M6 is the single-truth cutover that wires this surface. Fixing them now is the correct pre-M6 prep;
shipping the cutover on top of them would silently poison CAS/restore on real desktops.

## Triage result (26 Codex findings vs HEAD)

| Class | Count | Findings |
|-------|-------|----------|
| FIXED (already resolved on HEAD) | 6 | P1-1 LPE, P1-3 ICO forge, P1-6 recovery, P1-8 torn WAL, P1-15 wallpaper restore, P1-16 STA pump |
| **OPEN-CONFIRMED (this wave)** | 7 | P1-4, P1-9, P1-10, P1-14, P2-2, P2-5, P2-7 |
| OUT-OF-SCOPE (deferred by design) | 3 | P1-11 manual undo (post-M6 UI), P1-13 consent ladder (M7), P2-10 mock routing (M6 cutover) |
| OVER-RATED (real but mis-severity) | 10 | P1-2, P1-5, P1-7, P1-12, P2-1, P2-3, P2-4, P2-6, P2-8, P2-9 |

## Wave-2 fix list (ordered by severity)

Each fix **ships a regression test reproducing the failure** (dev-cycle Phase 9 HARD-GATE). Nearly
all are host-testable on macOS (pure txn/fingerprint logic, sqlite, fs) — the durability guarantee of
P1-9 and any COM path get a `[WINDOWS-VERIFY]` marker, but the atomic-replace/verify logic is tested
on host.

### Tier A — data-integrity, gate M6 (do first)

1. **P1-4 — CAS verify checks "changed" not "matches requested asset"**
   `dm-operations/src/txn/driver.rs:226` — `if verify_fp != new_fp || new_fp == item.original_fingerprint`.
   Verifies the fp is stable and differs from original, but **never compares to the requested
   asset's expected fingerprint**. A no-op-on-reapply writer (O→A→B) leaves `fp(A) ≠ fp(O)` → passes →
   commits a ledger claiming asset=B while the desktop shows A. Permanently poisons CAS/restore.
   **Fix:** the applier returns the achieved fingerprint (or the driver takes an `expected_applied`
   fp); verify becomes `new_fp == expected_applied`. This is a **port-contract change** (dm-domain
   `ports.rs` + dm-windows impl + dm-operations driver) — see P1-10, they are two halves of one verify.

2. **P1-10 — RegularFile item can never commit**
   `dm-windows/src/state_reader.rs:27-28` (fp = untouched file bytes) vs
   `dm-windows/src/apply/file_wrapper.rs:27-29` (apply only makes a sibling `.lnk` + hides original
   attrs, never touches bytes) → `new_fp == original_fp` → `driver.rs:226` verify fails → always
   rolls back. A whole item kind is dead.
   **Fix:** RegularFile fingerprint must cover **what apply actually changes** — the wrapper `.lnk`
   presence/bytes + the original file's hidden attribute — not the untouched file bytes.

3. **P1-14 — Recycle Bin empty asset is a guessed, never-written path**
   `dm-windows/src/apply/mod.rs:64-68` `paired_empty` is a string transform with no existence check;
   `dm-windows/src/recyclebin.rs:35-37` points the registry at `<hash>-empty.ico`; the driver writes
   only ONE asset (`dm-operations/src/txn/driver.rs:215`). The registry fp changes so verify+commit
   pass while the empty-bin ICO may never exist → broken empty-state icon.
   **Fix:** the driver must materialize **and verify existence of** the paired empty asset before the
   registry write.

### Tier B — durability / error-honesty

4. **P1-9 — user-file writes have no fsync / atomic-replace**
   `dm-windows/src/apply/{shortcut.rs:28, url_shortcut.rs:18, folder.rs:23, file_wrapper.rs:42}` are
   plain `fs::write` while the journal is fsync'd (`journal.rs:132`). Power-loss after a clean commit
   silently loses styling and poisons CAS (ledger says styled, disk is original → later read as an
   external edit, never re-styled); a crash mid-write can tear a `.lnk`/`.url`.
   **Fix:** fsync each writer's handle (or write-temp + atomic rename) **before** its `ItemApplied`
   journal record. `[WINDOWS-VERIFY]` the FlushFileBuffers path; test temp+rename on host.

5. **P2-5 — ledger corruption / real COM error swallowed as a benign item conflict**
   `dm-operations/src/txn/driver.rs:99` — `Err(_) => outcome.conflicts.push(...)`. A `CorruptLedger`
   or real COM error from `prepare_item` (`driver.rs:179,182` `?`) is misreported as a benign per-item
   "conflict" with `outcome.error` left `None`. The operator never learns the restore path is
   compromised.
   **Fix:** only `PortError::NotFound` → conflict; real/ledger errors → propagate as a batch failure
   with `outcome.error` set.

6. **P2-2 — settings migration is not transactional**
   `dm-operations/src/settings_store.rs:80-104` runs CREATE, INSERT, `user_version` as three separate
   autocommit statements. A crash between CREATE and the pragma leaves `user_version=0` + the table →
   next start re-runs CREATE → "table exists" → DB permanently unopenable.
   **Fix:** wrap migrate in one transaction, or `CREATE TABLE IF NOT EXISTS` + `INSERT OR IGNORE`.

### Tier C — byte-parity prerequisite (also M6 perf item #0)

7. **P2-7 — shortcut-arrow raster hidden in a `thread_local!`**
   `dm-icon-core/src/marks/mod.rs:184` `thread_local! NATIVE_ARROW`; `set_native_arrow_raster` sets it
   per-thread, `draw_classic_arrow` falls back to the drawn arrow when a **different** render thread
   sees `None` → breaks native↔wasm byte-parity. Latent until multi-thread render, but it is the
   **hard prerequisite** for the M6 worker-pool parallelism (see the M6 perf doc, optimization #0).
   **Fix:** move `NATIVE_ARROW` to a shared `OnceLock`/global instead of `thread_local!`. Must re-run
   the byte-parity cert battery (1487 corpus cells, anchor setHash `8a6c19ee69235d95`) and prove
   0/389,808,128 diff bytes preserved.

## Cheap latent fixes to fold in while touching these surfaces

Rated OVER-RATED (severity wrong) but genuinely real; ~free to fix while the file is open:

- **P1-12** — correct `is_styleable`/`is_shortcut` for Appx/System to match spec 06 §6/§6.5
  (`dm-domain/src/item.rs:71-83`). Virtual-item **enumeration** stays M4 breadth (out of scope).
- **P1-7** — add a monotonic txn-id allocator + committed-wins reconcile precedence
  (`driver.rs:81`, `recovery.rs:102-108`) **before** apply is wired.
- **P1-5** — wrap the two post-`TxnBegin` appends (`driver.rs:116,146`) so an append error also calls
  `rollback()`.
- **P2-4** — checkpoint the journal after each successful commit (bounds recovery replay).

## Ownership & dispatch (disjoint crates → parallel-safe)

| Bundle | Crates | Findings | Owner |
|--------|--------|----------|-------|
| Apply/txn/CAS surface | dm-operations, dm-windows, dm-domain | P1-4, P1-10, P1-14, P1-9, P2-5, P2-2 + latent P1-12/P1-7/P1-5/P2-4 | m34-windows-blind (warm; did the wave-1 LPE/journal/recovery/STA fixes) |
| Byte-parity arrow | dm-icon-core | P2-7 | m5-icon-core (owns the cert battery) |

The two bundles touch **disjoint crates** (m34: dm-operations/dm-windows/dm-domain; m5: dm-icon-core),
so no shared-file conflict — they run in parallel. Shared-worktree rule: each agent `git add` only its
own paths and commits with an explicit pathspec `git commit -- <paths>`.

The apply-surface bundle is one **tightly-coupled** change (P1-4 + P1-10 + P1-14 + P2-5 all pivot on
the driver verify + the reader/applier port contract), so it is one sequential owner, not fanned out.

## Review & verify plan (dev-cycle Phase 6-7)

- **Phase 6 adversarial review:** cross-vendor via `/multi-ai` (Codex) on the landed wave-2 diff —
  spec-compliance (did we fix exactly these, no scope creep) then code-quality.
- **Phase 7 verify:** `cargo test` green across the touched crates + the byte-parity cert battery for
  P2-7; `[WINDOWS-VERIFY]` markers for the COM/FlushFileBuffers paths (no msvc runtime on host).
- **Phase 9 gate:** every one of the 7 fixes ships a red→green regression test.

## Landed (2026-07-11, lead-verified)

Apply/txn/CAS bundle complete — `0b71476` P1-4 (verify = `new_fp == expected_applied`, applier
returns achieved fp) · `609b72d` P1-10 (fingerprint the styled surface, host-tested
`fingerprint_surface.rs`) · `59f44d2` P1-14 (`AssetStore::put_empty_variant` + existence verify)
· `d7cbade` P2-5 (only `Ok(None)` → conflict; real errors fail the batch) · `2ed365b` P1-9
(`durable.rs::write_atomic`, temp+fsync+rename) · `834cb2e` P2-2 (single-txn idempotent migrate)
· `d21ee1f` P1-5 folded (append failure rolls back mutated items). Every fix ships a red→green
regression test; independently re-run by the lead: dm-domain 24 · dm-operations 54 · dm-windows 31,
0 failed; msvc cross-check clean. M6 wiring note: the real Windows `AssetStore` must place the empty
ICO at `paired_empty(primary.path)` to match the applier (documented in the trait comments).

**Flagged, NOT folded (deliberate — need their own decision/task):**
- **P1-12** — spec 06 §6 is ambiguous on SystemIcon (documented registry-styleable yet
  `is_styleable` excludes it); route to the classification owner with the spec in hand.
- **P1-7** — txn-id allocator + committed-wins precedence is its own small task (allocation
  semantics + recovery precedence + test surface), required **before apply is wired at M6**.
- **P2-4** — driver-side checkpoint CONFLICTS with the current recovery-owns-truncation invariant
  (`recover_from_journal_truncates_the_journal_after_reconciling` asserts the driver leaves the
  journal intact). Needs a who-owns-truncation design decision first; not a fold-in.

## Not in this wave (explicit)

OUT-OF-SCOPE by design: P1-11 (manual undo/restore txn — post-M6 UI), P1-13 (Public-desktop consent
ladder — M7), P2-10 (mock routing — resolved by the M6 cutover itself). OVER-RATED residuals not
listed above (P1-2 REG-kind fidelity, P2-1 style-arg reject, P2-3 M7 concurrency, P2-6 M6 ABI, P2-8
exotic path fidelity, P2-9 REG-type fidelity) are logged for their owning milestone, not this wave.

## Round 3 (wave-2R, 2026-07-11): Codex re-review fixes

The wave-2 diff was re-reviewed by Codex (FAIL: 5 fixed / 7 still-open / 6 new). The lead triaged the
survivors into three buckets. All host-testable logic fixes ship a red→green regression test;
pure-Windows-runtime proof is frozen into the [WINDOWS-VERIFY] ledger below rather than looped on
Codex forever.

**Bucket A — host-testable logic bugs (fixed + real-variant red→green):**
- `dc3c5e1` P1-#5 + new-P1 — `FileJournal::append` classifies every failure as `OperationError::Journal`
  so a real journal outage reaches the driver's `abandon` path (not a rollback onto a torn log); the
  driver's rollback is resilient (never `?`-aborts the LIFO restore, stops journaling once the log
  tears); recovery suppresses an incomplete/abandoned txn's restore for any item a strictly-later
  committed txn owns (`abandon` is now a write fence — the abandon-then-retry hole is closed).
- `1ded42c` new-P1 — the state reader routes its `.lnk` COM reads through the shared `StaExecutor`
  (was calling `IShellLinkW` off-apartment); composition root shares one executor.

**Bucket B — structure correct-by-construction (runtime proof → [WINDOWS-VERIFY]):**
- `e1d561e` P1-#1 — the styleable surface now covers folder `READONLY`, the wrapper target/working-dir,
  and the Recycle Bin `default` value + icon indices, so a partial writer is caught (host tests pin the
  derivation/dispatch/attr-masking; new `shell_link::read_wrapper_identity`).
- `77c4b44` P1-#3 + P2-#4 — POSIX parent-dir fsync propagates; `ReplaceFileW` uses
  `REPLACEFILE_WRITE_THROUGH` (durability barrier) and drops `REPLACEFILE_IGNORE_MERGE_ERRORS` (ACL/
  metadata merge failures surface); hard-link identity documented as an accepted limitation.
- `4259ece` P2-#3 — `attrs::get` keys off `GetLastError`, the Recycle Bin registry reads return
  `Result` and propagate non-NotFound errors, anchor capture uses `try_exists` — an existing-but-
  unreadable item is never recorded as absent (which would irreversibly delete it on restore).
- `bbffad7` P2-#1 + new-P3 — the driver re-checks the paired empty asset exists AFTER the apply
  (narrowing the dangling-ref window); the fake applier folds in `assets.empty` (no longer ignored).
- `15de596` new-P3 — a failed `IPersistFile::Save` cleans up its temp sibling instead of stranding it.

**Bucket C — recorded / cheap structural now:**
- `6949a7a` new-P1 — the paired empty asset is persisted in `LedgerEntry` + the `AssetWritten` journal
  record (both `#[serde(default)]`), so a future GC keeps the exact empty ref instead of orphaning it.
- `f275347` P1-12 — `AppxShortcut` is wired end-to-end through the Shortcut mechanism (it IS a `.lnk`);
  `System` returns an honest labelled `[WINDOWS-VERIFY]`-pending error instead of the generic
  `Unsupported`; the apply/read matches are exhaustive over `ItemKind`.
- **new-P2 (txn-id reservation race) → deferred to M7.** `from_journal` + read-max/append is not
  atomic, so two concurrent allocators/handles could both issue id 1; commit + rollback would then
  trip the both-terminal fail-closed guard and brick startup. Codex itself confirmed serialized
  execution is safe and post-checkpoint reuse is harmless (old groups are gone, ledger entries carry
  no txn id), so this only bites the **M7 resident** with concurrent apply. M7 must give the allocator
  an atomic reserve (single owner + `&mut`, or a lock) before it drives concurrent transactions.

## [WINDOWS-VERIFY] frozen ledger (wave-2R)

Pure Windows-runtime items that cannot be closed on the Mac host — code-verified via the msvc
cross-check + host derivation tests, runtime behaviour to be confirmed on the owner's Windows box.
Frozen here; NOT re-looped on Codex.

1. **STA routing (A3)** — the reader's `.lnk` reads run on the correct STA apartment at runtime.
2. **Surface reads (B4/P1-#1)** — whether Explorer honours folder `READONLY` / the wrapper
   target+working-dir / the Recycle Bin indices; `read_wrapper_identity`'s `GetPath`/
   `GetWorkingDirectory`/`GetIconLocation` values on a real `.lnk`.
3. **Durability (B5/P1-#3)** — `ReplaceFileW(REPLACEFILE_WRITE_THROUGH)` write-through on NTFS; the
   Windows directory-fsync is a documented no-op (durability delegated to ReplaceFileW).
4. **Merge / hard-link (B6/P2-#4)** — ACL/metadata merge failures surface without
   `IGNORE_MERGE_ERRORS`; hard-link identity loss is an accepted limitation (desktop items are not
   hard-linked).
5. **Adapter error codes (B7/P2-#3)** — `GetLastError` file/path-not-found vs. real errors;
   registry `ErrorKind::NotFound` vs. access errors; `try_exists` metadata-error behaviour.
6. **Dangling-empty residual (B8/P2-#1)** — a paired empty deleted strictly AFTER the post-apply
   re-check is unclosable without the applier re-validating at write time.
7. **COM Save→flush→Replace coverage (P3-#3, new-P3)** — the per-writer Save/finalize/temp-cleanup
   path cannot be exercised on the host without substantial COM injection.
8. **System DefaultIcon (C12/P1-12)** — the HKCU CLSID `DefaultIcon` writer + reader + discovery for
   `ItemKind::System` are a Windows-scoped follow-up; apply/read currently return a labelled pending
   error (never a silent mis-reject).
