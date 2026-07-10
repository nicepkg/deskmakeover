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
    AssetRef, Fingerprint, IconApplier, ItemId, ItemStateReader, ItemTarget, OwnedFields,
    PortError,
};

use crate::error::Result;
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
                Ok(None) => outcome.conflicts.push(req.target.id.clone()),
                Err(_) => outcome.conflicts.push(req.target.id.clone()),
            }
        }

        if proceeding.is_empty() {
            return Ok(outcome);
        }

        journal.append(&JournalRecord::TxnBegin {
            txn,
            items: proceeding.iter().map(|(r, ..)| r.target.id.clone()).collect(),
        })?;

        // Phase 2 (durable mutation): prepare → asset → apply → verify, per item.
        let mut applied: Vec<Prepared> = Vec::new();
        for (req, anchor, original_fp, expected) in proceeding {
            // Anchor journaled + flushed BEFORE any mutation (codex B4).
            journal.append(&JournalRecord::ItemPrepared {
                txn,
                item: req.target.id.clone(),
                target: req.target.clone(),
                anchor: anchor.clone(),
                original_fingerprint: original_fp,
                expected_fingerprint: expected,
                asset_hash: req.asset_hash.clone(),
                owned: req.owned,
                pinned_seed: req.pinned_seed,
            })?;
            // From here the item is a rollback candidate (a mutation might start below).
            applied.push(Prepared {
                target: req.target.clone(),
                anchor,
                original_fingerprint: original_fp,
                owned: req.owned,
                pinned_seed: req.pinned_seed,
                asset: AssetRef::new(String::new(), String::new()),
                new_fingerprint: None,
            });
            let idx = applied.len() - 1;

            if let Err(e) = self.mutate_item(txn, &req, &mut applied[idx], journal) {
                // Hard failure → roll back everything prepared so far (LIFO), leave no residue.
                return self.rollback(txn, applied, journal, ledger, format!("apply failed: {e}"));
            }
        }

        // All items applied + verified → commit and record the ledger.
        journal.append(&JournalRecord::TxnCommitted { txn })?;
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
    /// modification, deleted, or no restore material). No mutation happens here.
    fn prepare_item(
        &self,
        req: &ApplyRequest,
        ledger: &dyn LedgerStore,
    ) -> Result<Option<(dm_domain::RestoreAnchor, Fingerprint, Fingerprint)>> {
        let current = match self.reader.read_fingerprint(&req.target) {
            Ok(fp) => fp,
            Err(PortError::NotFound(_)) => return Ok(None), // item gone → skip
            Err(e) => return Err(e.into()),
        };

        let existing = ledger.get(&req.target.id)?;
        // CAS anchor: last-applied on re-apply, the scan-time observation on a fresh apply.
        let expected = existing
            .as_ref()
            .map(|e| e.last_applied_fingerprint)
            .unwrap_or(req.expected_fingerprint);
        if current != expected {
            return Ok(None); // external modification → visible conflict, never overwritten
        }

        // The true original is pinned from the ledger on re-apply, captured fresh otherwise.
        let (anchor, original_fp) = match existing {
            Some(e) => (e.original_anchor, e.original_fingerprint),
            None => match self.reader.capture_anchor(&req.target) {
                Ok(a) => (a, current),
                Err(_) => return Ok(None), // cannot capture a way back → skip
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

        // A two-state item (the Recycle Bin) needs a paired empty ICO the mutation references by
        // convention. Materialize AND verify it exists BEFORE the registry write, so we never
        // point a live registry value at an asset that was only guessed and never written (P1-14).
        if let Some(empty_bytes) = &req.empty_asset_bytes {
            let empty = self.assets.put_empty_variant(&asset, empty_bytes)?;
            if !self.assets.exists(&empty)? {
                return Err(PortError::AssetMissing(empty.path).into());
            }
        }

        journal.append(&JournalRecord::AssetWritten { txn, item: req.target.id.clone(), asset: asset.clone() })?;

        // The external mutation (icon-location swap). The applier reports the fingerprint the
        // styleable surface should now carry for THIS asset.
        let expected_applied = self.applier.apply(&req.target, &asset)?;
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
        let mut outcome = ApplyOutcome { error: Some(reason), ..Default::default() };
        let mut restore_errors: Vec<String> = Vec::new();
        for item in applied.into_iter().rev() {
            match self.applier.restore(&item.target, &item.anchor) {
                Ok(()) => {
                    ledger.remove(&item.target.id)?;
                    journal.append(&JournalRecord::ItemRolledBack { txn, item: item.target.id.clone() })?;
                    outcome.rolled_back.push(item.target.id.clone());
                }
                Err(e) => restore_errors.push(format!("{}: {e}", item.target.id.as_str())),
            }
        }
        if restore_errors.is_empty() {
            journal.append(&JournalRecord::TxnRolledBack { txn })?;
        } else {
            let msg = outcome.error.take().unwrap_or_default();
            outcome.error = Some(format!("{msg} · rollback-incomplete: {}", restore_errors.join("; ")));
        }
        Ok(outcome)
    }
}
