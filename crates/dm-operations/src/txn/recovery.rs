//! Crash recovery: replay the journal and drive every transaction to a consistent terminal
//! state.
//!
//! The policy is database-style abort-on-recovery, made safe by the anchor-before-mutation
//! ordering (`DesktopBakeService` codex B4):
//! * a transaction with no terminal record is **incomplete** → every item is restored from its
//!   journaled anchor (the pinned true original) and its ledger entry removed; because the
//!   anchor was flushed BEFORE any mutation, this succeeds whether or not the real desktop
//!   write had happened at the crash point, so the item lands EXACTLY on its original;
//! * a `TxnCommitted` transaction whose ledger write was lost (crash in the commit→upsert gap)
//!   is **reconciled** — the entry is rebuilt from the journal so a corrupt/absent ledger never
//!   reads as "nothing applied";
//! * a `TxnRolledBack` transaction is already clean.

use std::collections::{HashMap, HashSet};

use dm_domain::{IconApplier, ItemStateReader, ItemTarget, RestoreAnchor};

use crate::error::{OperationError, Result};
use crate::ledger::entry::{LedgerEntry, TxnState};
use crate::ledger::store::LedgerStore;
use crate::txn::journal::{JournalRecord, JournalSink};

/// What recovery did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RecoveryOutcome {
    /// Items from incomplete transactions restored to their original.
    pub aborted: Vec<dm_domain::ItemId>,
    /// Committed items whose ledger entry was (re)written to close the commit→upsert gap.
    pub reconciled: Vec<dm_domain::ItemId>,
    /// Count of transactions that were already terminal (nothing to do).
    pub clean_txns: usize,
    /// Per-item runtime faults that prevented a full reconcile (a restore/ledger I/O error while
    /// replaying a prior crash's journal). Non-empty means recovery is INCOMPLETE: the caller must
    /// NOT stack a new transaction on top and must NOT checkpoint the journal (the unreconciled
    /// records stay for a retry) — it returns a repair-required status instead of a bare Err over a
    /// desktop the recovery already partially mutated (codex R4-Block 5).
    pub degraded: Vec<String>,
}

/// Per-item state accumulated from a transaction's journal records.
#[derive(Clone)]
struct ItemRecovery {
    target: ItemTarget,
    anchor: RestoreAnchor,
    original_fingerprint: dm_domain::Fingerprint,
    owned: dm_domain::OwnedFields,
    pinned_seed: Option<u32>,
    asset: Option<dm_domain::AssetRef>,
    empty_asset: Option<dm_domain::AssetRef>,
    new_fingerprint: Option<dm_domain::Fingerprint>,
}

#[derive(Default)]
struct TxnRecovery {
    committed: bool,
    rolled_back: bool,
    /// Items in first-seen order.
    order: Vec<dm_domain::ItemId>,
    items: HashMap<String, ItemRecovery>,
    /// Items with a durable per-item `ItemRolledBack` record: their in-transaction rollback
    /// already restored them to their original, so a crash-recovery replay must NOT restore them a
    /// SECOND time (codex E `recovery:250`). Replaying the restore would clobber any edit the user
    /// made between the per-item rollback and restart — the exact terminal-write-lost scenario where
    /// the whole txn still reads as "incomplete" but individual items are already terminal.
    rolled_back_items: HashSet<String>,
}

/// Startup entry point: read the journal, then [`recover`] over it. This is the call the
/// composition root makes before exposing any mutation command — a crash mid-transaction is
/// driven to a consistent terminal state first. A missing/empty journal is a clean no-op; a torn
/// journal tail is tolerated by the reader, while mid-file corruption surfaces as an error so
/// startup fails closed rather than on a partially-parsed log.
pub fn recover_from_journal(
    journal: &mut dyn JournalSink,
    reader: &dyn ItemStateReader,
    applier: &dyn IconApplier,
    ledger: &mut dyn LedgerStore,
) -> Result<RecoveryOutcome> {
    let records = journal.read_all()?;
    let mut outcome = recover(&records, reader, applier, ledger)?;
    // Truncate the reconciled history ONLY on a clean pass with something to truncate. An EMPTY journal
    // has nothing to recover, so it must NOT checkpoint (codex R7-#4): an empty checkpoint tries to
    // DELETE the log file, and a zero-byte log that cannot be deleted (an ACL fault) would otherwise
    // mark every apply/reset degraded forever despite there being no crash to recover. With records
    // present: if recovery degraded (an item's restore/ledger op faulted, codex R4-Block 5), the
    // unreconciled records MUST stay so the next (idempotent) recovery can finish them — checkpointing
    // them away would strand a crashed transaction. A checkpoint that FAILS is surfaced as degraded
    // (codex R6-#4): a persistently-failing truncation would otherwise silently replay + defer every op
    // forever with no visible cause. The replay stays safe (idempotent), but the caller learns it is stuck.
    if !records.is_empty() && outcome.degraded.is_empty() {
        if let Err(e) = journal.checkpoint(&[]) {
            outcome.degraded.push(format!("recovery checkpoint: {e}"));
        }
    }
    Ok(outcome)
}

/// A RESTORE-AFFORDANCE signal (NOT a full reconcile-equivalence predicate): true when the journal
/// holds a transaction whose styled desktop the ledger may not yet reflect, so the restore affordance
/// must stay reachable (codex R6-#6). Read-only, and stricter than the checkpoint-only `active_txns`
/// (which excludes committed txns): it covers BOTH an incomplete txn (an abort candidate — desktop
/// styled, no ledger row) AND a committed txn with NO committed ledger row for an item (a reconcile
/// candidate — the driver's `TxnCommitted` is durable but a later `ledger.upsert` faulted, R6-#1). It
/// deliberately does NOT re-implement recovery's full match (which also compares `last_applied` +
/// `empty_asset`): a committed txn whose item HAS a committed row that recovery would still rewrite
/// (a stale-but-present row) does not trip this, but that case is already `applied:true` from the
/// present row, so the affordance is never wrongly hidden. A rolled-back txn is safe. A normal
/// post-apply journal (committed + present ledger rows, not yet checkpointed) never spuriously trips.
pub fn repair_pending(records: &[JournalRecord], ledger: &dyn LedgerStore) -> Result<bool> {
    // txn -> (committed, rolled_back, items-in-first-seen-order)
    let mut txns: HashMap<u64, (bool, bool, Vec<dm_domain::ItemId>)> = HashMap::new();
    for record in records {
        let entry = txns.entry(record.txn()).or_default();
        match record {
            JournalRecord::TxnCommitted { .. } => entry.0 = true,
            JournalRecord::TxnRolledBack { .. } => entry.1 = true,
            JournalRecord::ItemPrepared { item, .. } => entry.2.push(item.clone()),
            _ => {}
        }
    }
    for (_txn, (committed, rolled_back, items)) in txns {
        if rolled_back {
            continue; // terminal, desktop restored, ledger clean → safe
        }
        if !committed {
            return Ok(true); // incomplete → abort candidate → a styled desktop may lack a ledger row
        }
        // Committed: pending unless every item is already a committed ledger row (reconciled).
        for item in items {
            match ledger.get(&item)? {
                Some(e) if e.state.is_committed() => {}
                _ => return Ok(true), // missing / non-committed row → reconcile candidate
            }
        }
    }
    Ok(false)
}

/// Replays `records` and reconciles the desktop + ledger. Idempotent: running it twice is a
/// no-op the second time (restores replay originals over originals; reconciles are skipped when
/// the ledger already matches).
pub fn recover(
    records: &[JournalRecord],
    reader: &dyn ItemStateReader,
    applier: &dyn IconApplier,
    ledger: &mut dyn LedgerStore,
) -> Result<RecoveryOutcome> {
    let mut txns: HashMap<u64, TxnRecovery> = HashMap::new();
    let mut txn_order: Vec<u64> = Vec::new();

    for record in records {
        let txn = record.txn();
        if !txns.contains_key(&txn) {
            txn_order.push(txn);
            txns.insert(txn, TxnRecovery::default());
        }
        let group = txns.get_mut(&txn).expect("txn just inserted");
        apply_record(group, record);
    }

    // An item that a LATER transaction re-styled and committed is owned by that transaction. An
    // earlier INCOMPLETE (abandoned) transaction must not restore its original over that committed
    // result: `abandon` does not journal a terminal, so its terminal-less records would otherwise be
    // replayed here and re-abort an item a later txn legitimately committed — leaving desktop=O while
    // the ledger says C (the abandon-then-retry crash-consistency hole). Monotonic ids (P1-7) make
    // "later" == "higher id", so we skip an incomplete txn's restore for any item a strictly-higher
    // committed txn owns. Built before the drain loop so every committed group is visible up front.
    let mut committed_owner: HashMap<String, u64> = HashMap::new();
    for (&id, group) in &txns {
        if group.committed && !group.rolled_back {
            for item in &group.order {
                let slot = committed_owner.entry(item.as_str().to_string()).or_insert(id);
                if id > *slot {
                    *slot = id;
                }
            }
        }
    }

    // Structural preflight BEFORE any mutation (codex R5-#4): a `both terminals` corruption in ANY
    // transaction must fail closed with the desktop UNTOUCHED. A single txn id can only ever have ONE
    // terminal — a real transaction either commits or rolls back, never both. Both terminals under one
    // id means id reuse or journal corruption: the records of two different transactions merged into
    // one group. Neither "committed-wins" nor "rolled-back-wins" is correct (each mishandles the other
    // transaction's items). If this validation ran INSIDE the drain loop, an EARLIER incomplete txn in
    // `txn_order` would already have been aborted — restoring + mutating the desktop — before the
    // corrupt txn was reached, leaving a bare Err over a half-mutated desktop. Validate every group up
    // front so the whole recovery is refused atomically. The monotonic id allocator + the driver's
    // monotonic guard prevent this upstream; this is the last line.
    for (&txn, group) in &txns {
        if group.committed && group.rolled_back {
            return Err(OperationError::Journal(format!(
                "transaction id {txn} carries both a commit and a rollback terminal — id reuse or journal corruption; refusing to recover"
            )));
        }
    }

    let mut outcome = RecoveryOutcome::default();
    for txn in txn_order {
        let group = txns.remove(&txn).expect("txn in order list");
        // Both-terminals corruption was ruled out atomically by the preflight above, so every group
        // here has at most one terminal — no mutation has happened yet at the point that check runs.
        debug_assert!(!(group.committed && group.rolled_back), "both-terminals must be preflighted");
        if group.rolled_back {
            outcome.clean_txns += 1;
        } else if group.committed {
            reconcile_committed(&group, reader, ledger, &mut outcome)?;
        } else {
            abort_incomplete(&group, txn, &committed_owner, applier, ledger, &mut outcome)?;
        }
    }
    Ok(outcome)
}

fn apply_record(group: &mut TxnRecovery, record: &JournalRecord) {
    match record {
        JournalRecord::TxnBegin { .. } => {}
        JournalRecord::TxnCommitted { .. } => group.committed = true,
        JournalRecord::TxnRolledBack { .. } => group.rolled_back = true,
        JournalRecord::ItemPrepared {
            item,
            target,
            anchor,
            original_fingerprint,
            owned,
            pinned_seed,
            ..
        } => {
            group.order.push(item.clone());
            group.items.insert(
                item.as_str().to_string(),
                ItemRecovery {
                    target: target.clone(),
                    anchor: anchor.clone(),
                    original_fingerprint: *original_fingerprint,
                    owned: *owned,
                    pinned_seed: *pinned_seed,
                    asset: None,
                    empty_asset: None,
                    new_fingerprint: None,
                },
            );
        }
        JournalRecord::AssetWritten { item, asset, empty, .. } => {
            if let Some(rec) = group.items.get_mut(item.as_str()) {
                rec.asset = Some(asset.clone());
                rec.empty_asset = empty.clone();
            }
        }
        JournalRecord::ItemApplied { item, new_fingerprint, .. } => {
            if let Some(rec) = group.items.get_mut(item.as_str()) {
                rec.new_fingerprint = Some(*new_fingerprint);
            }
        }
        JournalRecord::ItemRolledBack { item, .. } => {
            group.rolled_back_items.insert(item.as_str().to_string());
        }
        JournalRecord::ItemVerified { .. } => {}
    }
}

/// Incomplete transaction: restore every prepared item to its original and drop its ledger
/// entry (the item is now un-styled, so the ledger must not claim otherwise).
fn abort_incomplete(
    group: &TxnRecovery,
    txn: u64,
    committed_owner: &HashMap<String, u64>,
    applier: &dyn IconApplier,
    ledger: &mut dyn LedgerStore,
    outcome: &mut RecoveryOutcome,
) -> Result<()> {
    // LIFO: mirror the driver's rollback order.
    for id in group.order.iter().rev() {
        // Do NOT restore an item a strictly-later committed transaction owns: that txn re-styled and
        // committed it after this one was abandoned, so its live styled state (and ledger entry) is
        // authoritative. Restoring the original here would clobber it (desktop O, ledger C).
        if committed_owner.get(id.as_str()).is_some_and(|&owner| owner > txn) {
            continue;
        }
        // Durably rolled back already (codex E `recovery:250`): a per-item `ItemRolledBack` record is
        // in the journal, so the txn's own rollback restored this item to its original before the
        // crash — only the txn-level terminal was lost. Restoring it AGAIN would clobber a user edit
        // made between that rollback and restart. It is already terminal + un-styled (an incomplete
        // txn never committed a ledger row, so there is nothing to remove); skip it entirely.
        if group.rolled_back_items.contains(id.as_str()) {
            continue;
        }
        let rec = &group.items[id.as_str()];
        // Best-effort (codex R4-Block 5): a restore/remove fault on one item must not bail with a bare
        // Err over the items this recovery already restored. Record it as degraded and press on;
        // restore is idempotent, so a later retry finishes the unreconciled items. The row is only
        // removed if its restore actually landed, so desktop and ledger stay consistent per item.
        if let Err(e) = applier.restore(&rec.target, &rec.anchor) {
            outcome.degraded.push(format!("recover abort restore {}: {e}", id.as_str()));
            continue;
        }
        if let Err(e) = ledger.remove(id) {
            outcome.degraded.push(format!("recover abort ledger remove {}: {e}", id.as_str()));
        }
        outcome.aborted.push(id.clone());
    }
    Ok(())
}

/// Committed transaction: make sure the ledger reflects every applied item, rebuilding the
/// entry from the journal when the commit→upsert write was lost.
fn reconcile_committed(
    group: &TxnRecovery,
    _reader: &dyn ItemStateReader,
    ledger: &mut dyn LedgerStore,
    outcome: &mut RecoveryOutcome,
) -> Result<()> {
    for id in &group.order {
        let rec = &group.items[id.as_str()];
        let (Some(asset), Some(new_fp)) = (rec.asset.clone(), rec.new_fingerprint) else {
            // Reached commit without a full apply record for this item — impossible under the
            // driver's ordering, but skip defensively rather than fabricate an entry.
            continue;
        };
        // Skip an already-reconciled row ONLY when it matches the journal in full — including the
        // paired empty ref. A legacy row committed before empty_asset existed loads as `None`; if we
        // skipped it on the fingerprint alone, the exact empty ref the journal carries would never be
        // persisted and would then be checkpointed away, orphaning the empty ICO (new-P1, wave-2R).
        // Best-effort (codex R4-Block 5): a ledger read/write fault reconciling ONE committed item
        // must not bail the whole recovery — this path never touches the desktop (the committed txn
        // already applied before the crash), so a fault just leaves the row unreconciled for an
        // idempotent retry. Record + continue.
        let already = match ledger.get(id) {
            Ok(row) => row
                .map(|e| {
                    e.state.is_committed()
                        && e.last_applied_fingerprint == new_fp
                        && e.empty_asset == rec.empty_asset
                })
                .unwrap_or(false),
            Err(e) => {
                outcome.degraded.push(format!("recover reconcile read {}: {e}", id.as_str()));
                continue;
            }
        };
        if already {
            continue;
        }
        let version = match ledger.next_version() {
            Ok(v) => v,
            Err(e) => {
                outcome.degraded.push(format!("recover reconcile version {}: {e}", id.as_str()));
                continue;
            }
        };
        if let Err(e) = ledger.upsert(LedgerEntry {
            item: id.clone(),
            target: rec.target.clone(),
            original_fingerprint: rec.original_fingerprint,
            original_anchor: rec.anchor.clone(),
            last_applied_fingerprint: new_fp,
            owned: rec.owned,
            asset,
            empty_asset: rec.empty_asset.clone(),
            state: TxnState::Committed,
            pinned_seed: rec.pinned_seed,
            version,
        }) {
            outcome.degraded.push(format!("recover reconcile upsert {}: {e}", id.as_str()));
            continue;
        }
        outcome.reconciled.push(id.clone());
    }
    Ok(())
}
