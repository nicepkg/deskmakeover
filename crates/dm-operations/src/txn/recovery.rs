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

use std::collections::HashMap;

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
    let outcome = recover(&records, reader, applier, ledger)?;
    // Every transaction in the journal is now reconciled into the ledger / desktop, so the history
    // is no longer needed — truncate it so the next recovery replays nothing (P2-5 checkpoint).
    // Best-effort: a failed truncation just leaves the reconciled records for the next
    // (idempotent) recovery, so it must not fail an otherwise-successful recovery pass.
    let _ = journal.checkpoint(&[]);
    Ok(outcome)
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

    let mut outcome = RecoveryOutcome::default();
    for txn in txn_order {
        let group = txns.remove(&txn).expect("txn in order list");
        // A single txn id can only ever have ONE terminal — a real transaction either commits or
        // rolls back, never both. Both terminals under one id means id reuse or journal corruption:
        // the records of two different transactions merged into one group. Neither "committed-wins"
        // nor "rolled-back-wins" is correct (each mishandles the other transaction's items —
        // rolled-back-wins silently drops the committed work, committed-wins wrongly reconciles the
        // rolled-back items as styled). Fail closed and surface it (P1-7), consistent with the
        // fail-closed stance for a corrupt ledger / mid-file journal damage. The monotonic id
        // allocator + the driver's monotonic guard prevent this upstream; this is the last line.
        if group.committed && group.rolled_back {
            return Err(OperationError::Journal(format!(
                "transaction id {txn} carries both a commit and a rollback terminal — id reuse or journal corruption; refusing to recover"
            )));
        }
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
        JournalRecord::ItemVerified { .. } | JournalRecord::ItemRolledBack { .. } => {}
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
        let rec = &group.items[id.as_str()];
        applier.restore(&rec.target, &rec.anchor)?;
        ledger.remove(id)?;
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
        let already = ledger
            .get(id)?
            .map(|e| e.state.is_committed() && e.last_applied_fingerprint == new_fp)
            .unwrap_or(false);
        if already {
            continue;
        }
        let version = ledger.next_version()?;
        ledger.upsert(LedgerEntry {
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
        })?;
        outcome.reconciled.push(id.clone());
    }
    Ok(())
}
