//! The durable transaction driver.
//!
//! Ports `DesktopBakeService.ApplyAsync` + `JournaledOperationRunner.RunAsync` onto a
//! crash-durable journal. The pipeline per item is spec 07 §5's state machine
//! (prepared → asset-written → applied → verified → committed); the restore anchor is
//! journaled and flushed BEFORE any desktop mutation (`DesktopBakeService` codex B4). Two
//! deliberate divergences from the oracle, per ADR-0019/0020:
//! * conflict handling is **per-item CAS skip**, not batch-abort — an externally modified item
//!   surfaces as a conflict and is left untouched, the rest proceed (ADR-0020 §2);
//! * re-apply is **incremental** — the true original stays pinned in the ledger, never
//!   re-captured from an already-styled desktop (no restore-first, ADR-0019).

use dm_domain::{
    ApplyAssets, AssetRef, Fingerprint, IconApplier, ItemId, ItemKind, ItemStateReader, ItemTarget,
    OwnedFields, PortError,
};

use crate::error::{OperationError, Result};
use crate::ledger::entry::{LedgerEntry, TxnState};
use crate::ledger::store::LedgerStore;
use crate::txn::journal::{JournalRecord, JournalSink};
use dm_domain::AssetStore;

/// One item's styling request. `expected_fingerprint` is the CAS anchor for a *fresh* apply
/// (the state observed at scan); on re-apply the driver instead compares against the ledger's
/// last-applied fingerprint.
#[derive(Debug, Clone)]
pub struct ApplyRequest {
    pub target: ItemTarget,
    pub expected_fingerprint: Fingerprint,
    pub owned: OwnedFields,
    pub asset_hash: String,
    pub asset_bytes: Vec<u8>,
    /// The paired empty-state ICO bytes for a two-state item (the Recycle Bin). When present the
    /// driver materializes AND verifies this asset exists before the mutation references it
    /// (P1-14). `None` for single-asset kinds.
    pub empty_asset_bytes: Option<Vec<u8>>,
    pub pinned_seed: Option<u32>,
}

/// The result of an apply batch.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    /// Items styled and committed to the ledger.
    pub committed: Vec<ItemId>,
    /// Items skipped as conflicts (external modification, missing item, or no restore material).
    pub conflicts: Vec<ItemId>,
    /// Items walked back to their original because the batch failed.
    pub rolled_back: Vec<ItemId>,
    /// CONSERVATIVE "the desktop may have been touched" flag: set once the transaction entered its
    /// durable mutation phase (a rollback or abandon ran). The desktop may actually be changed (an item
    /// styled then restored to its true original, or a restore that faulted and left a residual state)
    /// OR still pristine (the very first item's asset-write faulted before any icon-location swap). It
    /// never UNDER-reports — a preflight failure or an empty batch leaves it false — so the host can
    /// safely treat it as "possibly changed": distinguishing "failed, truly nothing changed" from
    /// "possibly moved the desktop" (codex R5-#1), erring toward a repair toast over a false "nothing
    /// changed". A precise did-mutate fact would need the driver to thread the per-item apply phase; the
    /// safe over-report is deliberate.
    pub desktop_mutated: bool,
    /// A human-readable failure reason, if the batch did not fully commit.
    pub error: Option<String>,
}

/// Internal per-item working state carried from prepare through commit.
struct Prepared {
    target: ItemTarget,
    anchor: dm_domain::RestoreAnchor,
    original_fingerprint: Fingerprint,
    owned: OwnedFields,
    pinned_seed: Option<u32>,
    asset: AssetRef,
    /// The paired empty-state asset (Recycle Bin), carried to the ledger on commit (new-P1).
    empty_asset: Option<AssetRef>,
    /// Set once the mutation is applied + verified.
    new_fingerprint: Option<Fingerprint>,
}

/// Drives durable apply transactions over the platform ports.
pub struct TxnDriver<'p> {
    reader: &'p dyn ItemStateReader,
    applier: &'p dyn IconApplier,
    assets: &'p dyn AssetStore,
}

impl<'p> TxnDriver<'p> {
    pub fn new(
        reader: &'p dyn ItemStateReader,
        applier: &'p dyn IconApplier,
        assets: &'p dyn AssetStore,
    ) -> Self {
        Self { reader, applier, assets }
    }

    /// Applies `requests` as one durable transaction. Conflicts are pre-filtered and skipped;
    /// the remaining items run through the journaled state machine. A genuine failure mid-batch
    /// rolls back every item already applied in this transaction (LIFO) and leaves zero residue.
    pub fn apply(
        &self,
        txn: u64,
        requests: Vec<ApplyRequest>,
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
    ) -> Result<ApplyOutcome> {
        let mut outcome = ApplyOutcome::default();

        // Phase 1 (no mutation): CAS + anchor capture. Partition into proceeding vs conflict.
        let mut proceeding: Vec<(ApplyRequest, dm_domain::RestoreAnchor, Fingerprint, Fingerprint)> =
            Vec::new();
        for req in requests {
            match self.prepare_item(&req, ledger) {
                Ok(Some((anchor, original_fp, expected))) => {
                    proceeding.push((req, anchor, original_fp, expected))
                }
                // Benign, safe-to-skip: the item is gone, externally modified, or has no restore
                // material. `prepare_item` returns `Ok(None)` for all of these.
                Ok(None) => outcome.conflicts.push(req.target.id.clone()),
                // A real infrastructure failure (a non-NotFound COM/IO error from the reader, or a
                // corrupt/unreadable ledger) is NOT a benign per-item conflict. Misreporting it as
                // one — with `outcome.error` left None — hides that the restore path may be
                // compromised. Fail the whole batch instead; nothing has been journaled or mutated
                // yet, so we abort cleanly with the error surfaced (P2-5).
                Err(e) => {
                    outcome.error = Some(format!("apply preflight failed: {e}"));
                    return Ok(outcome);
                }
            }
        }

        if proceeding.is_empty() {
            return Ok(outcome);
        }

        // The txn id must be strictly greater than any already in the durable journal, so a reused
        // or regressed id can never merge two transactions' records into one recovery group (which
        // recovery would misclassify) — P1-7. The composition root allocates ids via
        // `TxnIdAllocator`; this guard rejects a caller that bypasses it, before any mutation.
        let max_seen = journal.read_all()?.iter().map(|r| r.txn()).max().unwrap_or(0);
        if txn <= max_seen {
            return Err(OperationError::Journal(format!(
                "txn id {txn} is not monotonic (journal already holds up to {max_seen}); ids must never be reused"
            )));
        }

        journal.append(&JournalRecord::TxnBegin {
            txn,
            items: proceeding.iter().map(|(r, ..)| r.target.id.clone()).collect(),
        })?;

        // Phase 2 (durable mutation): prepare → asset → apply → verify, per item.
        let mut applied: Vec<Prepared> = Vec::new();
        for (req, anchor, original_fp, expected) in proceeding {
            // Anchor journaled + flushed BEFORE any mutation (codex B4).
            if let Err(e) = journal.append(&JournalRecord::ItemPrepared {
                txn,
                item: req.target.id.clone(),
                target: req.target.clone(),
                anchor: anchor.clone(),
                original_fingerprint: original_fp,
                expected_fingerprint: expected,
                asset_hash: req.asset_hash.clone(),
                owned: req.owned,
                pinned_seed: req.pinned_seed,
            }) {
                // The journal just failed and may be torn — appending rollback records after it
                // would risk fatal mid-file corruption (P1-5). Restore already-mutated items from
                // their anchors WITHOUT touching the journal; recovery re-confirms from the durable
                // prefix. If nothing has mutated yet, propagate with a pristine desktop.
                if applied.is_empty() {
                    return Err(e);
                }
                return self.abandon(applied, ledger, format!("prepare journal append failed: {e}"));
            }
            // From here the item is a rollback candidate (a mutation might start below).
            applied.push(Prepared {
                target: req.target.clone(),
                anchor,
                original_fingerprint: original_fp,
                owned: req.owned,
                pinned_seed: req.pinned_seed,
                asset: AssetRef::new(String::new(), String::new()),
                empty_asset: None,
                new_fingerprint: None,
            });
            let idx = applied.len() - 1;

            if let Err(e) = self.mutate_item(txn, &req, &mut applied[idx], journal) {
                // If the JOURNAL failed, it may be torn — restore from anchors without journaling
                // (P1-5). If the applier/asset failed but the journal is healthy, do the normal
                // journaled rollback so recovery sees a clean TxnRolledBack terminal.
                return if is_journal_error(&e) {
                    self.abandon(applied, ledger, format!("apply journal append failed: {e}"))
                } else {
                    self.rollback(txn, applied, journal, ledger, format!("apply failed: {e}"))
                };
            }
        }

        // All items applied + verified → commit. The TxnCommitted record is the linearization
        // point: if its append fails, the record may still be durable, so recovery could roll the
        // transaction FORWARD. Rolling the desktop back here would then contradict a committed
        // ledger and poison it (P1-4). Instead propagate — leaving the mutations in place — and let
        // recovery reconcile (roll forward if the commit is durable, roll back if not). We also
        // must NOT append anything more to a possibly-torn journal (P1-5).
        if let Err(e) = journal.append(&JournalRecord::TxnCommitted { txn }) {
            return Err(e);
        }
        for item in &applied {
            let version = ledger.next_version()?;
            let new_fp = item.new_fingerprint.expect("committed item has a verified fingerprint");
            ledger.upsert(LedgerEntry {
                item: item.target.id.clone(),
                target: item.target.clone(),
                original_fingerprint: item.original_fingerprint,
                original_anchor: item.anchor.clone(),
                last_applied_fingerprint: new_fp,
                owned: item.owned,
                asset: item.asset.clone(),
                empty_asset: item.empty_asset.clone(),
                state: TxnState::Committed,
                pinned_seed: item.pinned_seed,
                version,
            })?;
            outcome.committed.push(item.target.id.clone());
        }

        Ok(outcome)
    }

    /// Phase-1 CAS + anchor capture for one item. Returns `Some((anchor, original_fp,
    /// expected_fp))` when the item may proceed, `None` when it is a conflict (external
    /// modification, deleted, or no restore material). No DESKTOP mutation happens here; the one
    /// ledger write is the idempotent heal of a provably-stale poison row (codex R5-#2), taken before
    /// any desktop change so a fault still leaves the desktop pristine.
    fn prepare_item(
        &self,
        req: &ApplyRequest,
        ledger: &mut dyn LedgerStore,
    ) -> Result<Option<(dm_domain::RestoreAnchor, Fingerprint, Fingerprint)>> {
        let current = match self.reader.read_fingerprint(&req.target) {
            Ok(fp) => fp,
            Err(PortError::NotFound(_)) => return Ok(None), // item gone → skip
            Err(e) => return Err(e.into()),
        };

        let mut existing = ledger.get(&req.target.id)?;
        // Heal a POISONED lingering row before it can force a permanent CAS conflict (codex R5-#2,
        // R6-#3). A prior keep/reset restored this item on disk (desktop == its original) but the paired
        // `ledger.remove` faulted, leaving a stale row whose `last_applied` no longer matches the
        // desktop. If the user now re-styles the item directly (not via keep/reset, so the mod.rs heal
        // arm never sees it), that stale `last_applied` would be used as the CAS anchor forever and
        // reject the apply every time. Detect it — the row's original matches the live desktop, yet its
        // last-applied does not. We have PROVEN the desktop is at its true original, so drop the stale
        // row and anchor this apply on the OBSERVED `current`, NOT on `req.expected_fingerprint`: the
        // host's cached scan may predate the restore (no rescan between the failed remove and this
        // re-apply), so its fingerprint could still read as the old styled state. `remove` here is
        // pre-mutation (nothing journaled yet); a fault propagates as a clean, desktop-untouched Err.
        let mut anchor_on_current = false;
        if let Some(e) = &existing {
            if current == e.original_fingerprint && current != e.last_applied_fingerprint {
                ledger.remove(&req.target.id)?;
                existing = None;
                anchor_on_current = true;
            }
        }
        // CAS anchor: last-applied on re-apply, the OBSERVED current on a healed poison row (proven
        // original), the scan-time observation on an ordinary fresh apply.
        let expected = if anchor_on_current {
            current
        } else {
            existing
                .as_ref()
                .map(|e| e.last_applied_fingerprint)
                .unwrap_or(req.expected_fingerprint)
        };
        if current != expected {
            return Ok(None); // external modification → visible conflict, never overwritten
        }

        // The true original is pinned from the ledger on re-apply, captured fresh otherwise.
        let (anchor, original_fp) = match existing {
            Some(e) => (e.original_anchor, e.original_fingerprint),
            None => match self.reader.capture_anchor(&req.target) {
                Ok(a) => (a, current),
                // The item vanished between the CAS read and the capture → benign skip.
                Err(PortError::NotFound(_)) => return Ok(None),
                // A real capture failure (locked file, COM/registry error) is an infrastructure
                // problem, NOT a benign skip: propagate it so the batch fails and the operator
                // learns the restore path may be compromised, rather than silently not styling
                // (P2-3). A capture with no material returns `Ok(CaptureFailed)`, handled below.
                Err(e) => return Err(e.into()),
            },
        };
        if !anchor.has_material() {
            return Ok(None);
        }
        Ok(Some((anchor, original_fp, expected)))
    }

    /// Phase-2 durable mutation for one already-prepared item: write asset → apply → verify.
    fn mutate_item(
        &self,
        txn: u64,
        req: &ApplyRequest,
        item: &mut Prepared,
        journal: &mut dyn JournalSink,
    ) -> Result<()> {
        // Write the generated asset new-file-first, then journal it.
        let asset = self.assets.put(&req.asset_hash, &req.asset_bytes)?;
        item.asset = asset.clone();

        // Build the exact asset set the applier will point the item at. A two-state item (the
        // Recycle Bin) needs a paired empty ICO: materialize AND verify it exists, then hand its
        // exact ref to the applier so the ref we verified is the ref it references — never a
        // guessed path (P1-14/P2-1). A Recycle Bin request with NO empty bytes is rejected rather
        // than silently committing a dangling empty icon (P1-2).
        let assets = match &req.empty_asset_bytes {
            Some(empty_bytes) => {
                let empty = self.assets.put_empty_variant(&asset, empty_bytes)?;
                if !self.assets.exists(&empty)? {
                    return Err(PortError::AssetMissing(empty.path).into());
                }
                ApplyAssets::paired(asset.clone(), empty)
            }
            None if req.target.kind == ItemKind::RecycleBin => {
                return Err(PortError::AssetMissing(format!(
                    "Recycle Bin item {} requires a paired empty icon; none supplied",
                    req.target.id.as_str()
                ))
                .into());
            }
            None => ApplyAssets::single(asset.clone()),
        };
        // Carry the paired empty ref to the ledger (via the journal) so a future GC keeps the EXACT
        // empty asset, not a guessed paired path (new-P1).
        item.empty_asset = assets.empty.clone();

        journal.append(&JournalRecord::AssetWritten {
            txn,
            item: req.target.id.clone(),
            asset: asset.clone(),
            empty: assets.empty.clone(),
        })?;

        // The external mutation (icon-location swap). The applier reports the fingerprint the
        // styleable surface should now carry — derived from the asset, independent of a re-read.
        let expected_applied = self.applier.apply(&req.target, &assets)?;
        let new_fp = self.reader.read_fingerprint(&req.target)?;
        journal.append(&JournalRecord::ItemApplied { txn, item: req.target.id.clone(), new_fingerprint: new_fp })?;

        // Verify: the live state must have settled (stable re-read) AND must MATCH the asset the
        // apply was asked to establish — not merely differ from the original (P1-4). A writer that
        // no-ops on re-apply (or lands a stale asset) leaves a state that "changed" from the true
        // original yet does not match the request; committing it would poison the ledger with an
        // asset the desktop never actually shows.
        let verify_fp = self.reader.read_fingerprint(&req.target)?;
        if verify_fp != new_fp || new_fp != expected_applied {
            return Err(PortError::Com(
                "applied state did not settle or did not match the requested asset".into(),
            )
            .into());
        }
        // P2-1: the paired empty asset was verified to exist BEFORE the mutation, but it could be
        // deleted (GC, external process) in the window before the registry commits to referencing
        // it — and the Recycle Bin fingerprint covers only the registry path text, so a vanished ICO
        // is otherwise invisible. Re-check it exists AFTER the apply, narrowing the window to a
        // deletion strictly after this point (unclosable without the applier re-validating at write
        // time — recorded in the wave-2 [WINDOWS-VERIFY] ledger).
        if let Some(empty) = &assets.empty {
            if !self.assets.exists(empty)? {
                return Err(PortError::AssetMissing(empty.path.clone()).into());
            }
        }
        journal.append(&JournalRecord::ItemVerified { txn, item: req.target.id.clone() })?;
        item.new_fingerprint = Some(new_fp);
        Ok(())
    }

    /// Rolls back every prepared item LIFO, restoring each from its captured anchor (the pinned
    /// true original) and dropping its ledger entry so the ledger never claims an un-styled item
    /// is styled. On a clean rollback the terminal `TxnRolledBack` is journaled; if any restore
    /// fails the terminal is withheld so startup recovery retries (restoring an already-original
    /// item is a no-op).
    fn rollback(
        &self,
        txn: u64,
        applied: Vec<Prepared>,
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
        reason: String,
    ) -> Result<ApplyOutcome> {
        // `desktop_mutated` = conservative: rollback is reached from inside the mutation loop, so the
        // desktop MAY have moved (a styled-then-restored item, or a residual state from a faulted
        // restore) — or, if the very first item's asset-write faulted before its icon swap, still be
        // pristine. We never under-report, so the host errs toward a repair toast, not "nothing changed"
        // (codex R5-#1 / R6-#5).
        let mut outcome = ApplyOutcome { error: Some(reason), desktop_mutated: true, ..Default::default() };
        let mut restore_errors: Vec<String> = Vec::new();
        // If a journal append fails MID-rollback the log may now be torn, so we must stop appending
        // to it — but never stop restoring: a naive `?` on the per-item append (or ledger remove)
        // would abort the loop and strand every later item still mutated (P1-5). Keep walking the
        // LIFO restore; recovery finishes the terminal from the durable prefix (a terminal-less txn
        // is aborted → restores the remainder, idempotent over what we already restored here).
        let mut journal_torn = false;
        for item in applied.into_iter().rev() {
            match self.applier.restore(&item.target, &item.anchor) {
                Ok(()) => {
                    if let Err(e) = ledger.remove(&item.target.id) {
                        restore_errors.push(format!("{}: ledger {e}", item.target.id.as_str()));
                        continue;
                    }
                    if journal_torn {
                        // Restored, just not journaled — recovery re-confirms from the durable prefix.
                        outcome.rolled_back.push(item.target.id.clone());
                    } else if let Err(e) = journal
                        .append(&JournalRecord::ItemRolledBack { txn, item: item.target.id.clone() })
                    {
                        journal_torn = true;
                        restore_errors.push(format!("journal torn during rollback: {e}"));
                    } else {
                        outcome.rolled_back.push(item.target.id.clone());
                    }
                }
                Err(e) => restore_errors.push(format!("{}: {e}", item.target.id.as_str())),
            }
        }
        // A clean `TxnRolledBack` terminal only when every item restored AND the journal never tore;
        // otherwise withhold it so startup recovery finishes the job. If even the terminal append
        // fails, every item is already restored and the ledger is clean, so recovery's abort is a
        // no-op — record the reason and return `Ok`, never `?`-propagate (which would look like a
        // failure the caller must react to when the desktop is in fact fully clean).
        if restore_errors.is_empty() && !journal_torn {
            if let Err(e) = journal.append(&JournalRecord::TxnRolledBack { txn }) {
                let msg = outcome.error.take().unwrap_or_default();
                outcome.error = Some(format!("{msg} · rollback terminal not durable: {e}"));
            }
        } else {
            let msg = outcome.error.take().unwrap_or_default();
            outcome.error = Some(format!("{msg} · rollback-incomplete: {}", restore_errors.join("; ")));
        }
        Ok(outcome)
    }

    /// Restores every prepared item from its anchor and drops its ledger entry WITHOUT writing to
    /// the journal — used when the journal itself has failed mid-transaction (before the commit
    /// point), so appending more records after a possibly-torn write would corrupt it (P1-5).
    /// Recovery re-confirms the outcome from the durable journal prefix (the incomplete txn has no
    /// terminal, so it is aborted and restored — idempotent over these already-restored items).
    /// Best-effort and resilient: a restore/ledger error is collected, never aborts the remaining
    /// items (P1-5 — a persistent journal failure must not strand mutated items). No item is
    /// committed to the ledger before the commit loop, so `remove` only clears a stale re-apply
    /// entry.
    fn abandon(
        &self,
        applied: Vec<Prepared>,
        ledger: &mut dyn LedgerStore,
        reason: String,
    ) -> Result<ApplyOutcome> {
        // `desktop_mutated` = conservative (see rollback): abandon is reached after the mutation phase
        // began, so the desktop may have moved or (first-item asset-write fault) still be pristine; we
        // never under-report (codex R5-#1 / R6-#5).
        let mut outcome = ApplyOutcome { error: Some(reason), desktop_mutated: true, ..Default::default() };
        let mut errors: Vec<String> = Vec::new();
        for item in applied.into_iter().rev() {
            match self.applier.restore(&item.target, &item.anchor) {
                Ok(()) => {
                    if let Err(e) = ledger.remove(&item.target.id) {
                        errors.push(format!("{}: ledger {e}", item.target.id.as_str()));
                    } else {
                        outcome.rolled_back.push(item.target.id.clone());
                    }
                }
                Err(e) => errors.push(format!("{}: {e}", item.target.id.as_str())),
            }
        }
        if !errors.is_empty() {
            let msg = outcome.error.take().unwrap_or_default();
            outcome.error = Some(format!("{msg} · restore-incomplete: {}", errors.join("; ")));
        }
        Ok(outcome)
    }
}

/// Whether an error is a journal-append failure — the signal that the journal may be torn and must
/// not be appended to again (P1-5). Distinguishes a compromised journal from a healthy-journal
/// applier/asset failure, which can still journal a clean rollback terminal.
fn is_journal_error(e: &OperationError) -> bool {
    matches!(e, OperationError::Journal(_))
}
