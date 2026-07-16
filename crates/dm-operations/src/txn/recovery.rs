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

use dm_domain::{IconApplier, ItemStateReader, ItemTarget, PortError, RestoreAnchor};

use crate::error::{OperationError, Result};
use crate::icons::scope::ScopeRoots;
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
    /// Items an incomplete-transaction abort LEFT UNTOUCHED because their live desktop state could not
    /// be positively identified as ours — a user's manual edit made between crash and restart, or a
    /// torn/foreign write (never-clobber, codex `recovery:265`). This is a FINAL decision, not a
    /// fault: the desktop was NOT mutated and the journal IS checkpointed (never retried), unlike
    /// `degraded`. The caller surfaces these for review ("N items left as found") and re-syncs.
    pub preserved: Vec<dm_domain::ItemId>,
    /// Per-item runtime faults that prevented a full reconcile (a restore/ledger I/O error while
    /// replaying a prior crash's journal). Non-empty means recovery is INCOMPLETE: the caller must
    /// NOT stack a new transaction on top and must NOT checkpoint the journal (the unreconciled
    /// records stay for a retry) — it returns a repair-required status instead of a bare Err over a
    /// desktop the recovery already partially mutated (codex R4-Block 5).
    pub degraded: Vec<String>,
}

impl RecoveryOutcome {
    /// Whether recovery TOUCHED or left uncertain state the caller must re-sync from before stacking
    /// a new mutation: a restored (`aborted`) item moved the desktop, and a `preserved` item's state
    /// is unverified. Either fences the cached scan. (`degraded` is handled separately — it withholds
    /// the checkpoint for a retry.)
    pub fn moved_or_uncertain(&self) -> bool {
        !self.aborted.is_empty() || !self.preserved.is_empty()
    }
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
    scope: &ScopeRoots,
) -> Result<RecoveryOutcome> {
    let records = journal.read_all()?;
    let mut outcome = recover(&records, reader, applier, ledger, scope)?;
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
    scope: &ScopeRoots,
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
            abort_incomplete(&group, txn, &committed_owner, reader, applier, ledger, scope, &mut outcome)?;
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

/// Incomplete transaction: restore every prepared item WE can positively identify to its original
/// and drop its ledger entry; LEAVE anything else untouched (never-clobber).
fn abort_incomplete(
    group: &TxnRecovery,
    txn: u64,
    committed_owner: &HashMap<String, u64>,
    reader: &dyn ItemStateReader,
    applier: &dyn IconApplier,
    ledger: &mut dyn LedgerStore,
    scope: &ScopeRoots,
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
        //
        // Residual (codex F2b-review 🟠, [WINDOWS-VERIFY]): `ItemRolledBack` proves the restore's Rust
        // call returned Ok, not that a MULTI-write side effect (the Recycle Bin's 3 registry values)
        // was flushed to disk before a power loss. In that narrow compound case the skip does not
        // self-heal a durably-partial restore. This is the SAME live-state re-verify gap as the
        // deferred `recovery:265` fork: the complete fix reads the live state and re-restores only a
        // recognised-partial one — leaving a genuine user edit intact. Chosen tradeoff: preserving a
        // user edit (the more-severe, common bug) over self-healing a rare power-loss-partial registry
        // write (cosmetic, one system icon, recoverable by re-running reset).
        if group.rolled_back_items.contains(id.as_str()) {
            continue;
        }
        let rec = &group.items[id.as_str()];
        // NEVER-CLOBBER (codex `recovery:265`, owner-approved 2026-07-14 for極致 UX): only undo a
        // state we can identify as ours. Read the live surface and restore ONLY when it is the item's
        // true original (we never wrote, or a prior pass already restored — undo is a no-op) OR exactly
        // the style this txn applied (`new_fingerprint`, present once `ItemApplied` was journaled). ANY
        // other live state — the user's own ICON edit made between crash and restart, or a torn/foreign
        // write — is PRESERVED, never blind-restored. A `NotFound` read (the target is gone — the user
        // deleted it) is a FINAL never-clobber outcome, handled in the reconcile branch below; any OTHER
        // read fault cannot confirm the state, so it is a runtime fault (degraded → retried), NOT a
        // licence to clobber.
        //
        // SCOPE, honestly (codex nc-review): the identity here is the CAS `Fingerprint`, which for the
        // registry-icon kinds (Recycle Bin / System) IS the full restore surface, but for a `.lnk`/
        // `.url` covers only the icon path/index while restore replays the WHOLE file (folder/wrapper
        // attrs are similar). So this fully protects the user's ICON customization — the app's purpose,
        // and a strict improvement over the old blind-restore — but a user edit to a NON-icon field
        // (a shortcut's target/args) whose icon is unchanged is NOT yet protected. Two [WINDOWS-VERIFY]
        // gaps remain for a total guarantee: (1) a full-restore-surface identity (a wider capture than
        // the icon fingerprint), and (2) an ATOMIC compare-and-swap restore (this read-then-restore is
        // not atomic vs a concurrent external edit — the F1 platform-CAS item). Both are real-platform
        // work (the in-memory fake's fingerprint is already full-surface, so it cannot exercise gap 1).
        let live = match reader.read_fingerprint(&rec.target) {
            Ok(fp) => Some(fp),
            // The item's target is GONE — the user deleted it (or it never materialised). A missing
            // file is a FINAL never-clobber outcome, not a retryable fault (codex R2 A-1): reporting a
            // deletion as `degraded` would withhold the journal checkpoint forever, so every future
            // apply/reset would defer on a phantom crash that can NEVER resolve (the file stays gone).
            // Fall through with `None` → the reconcile-and-relinquish branch below drops any stale row.
            Err(PortError::NotFound(_)) => None,
            Err(e) => {
                outcome.degraded.push(format!("recover abort read {}: {e}", id.as_str()));
                continue;
            }
        };
        let is_ours = matches!(
            live,
            Some(fp) if fp == rec.original_fingerprint || rec.new_fingerprint == Some(fp)
        );
        if !is_ours {
            // The live state is NOT this txn's original or applied style. Before preserving, RECONCILE
            // the ledger so recovery never leaves a committed row that lies about the live desktop
            // (codex R2 A-2):
            //  * a committed row whose `last_applied` STILL matches the live fingerprint means the item
            //    sits at a style a PRIOR transaction legitimately committed and this incomplete txn
            //    never changed it (it crashed before its own apply) — the item is correctly tracked, so
            //    KEEP the row and leave everything clean (nothing moved, no fence);
            //  * every other case — a row that CONTRADICTS the live desktop (the user edited our styled
            //    icon), the target now DELETED (`live == None`), or no row at all — means we can no
            //    longer honestly claim ownership. Drop any row (an idempotent no-op when absent) and
            //    surface the item as `preserved`. A remove fault is a real ledger I/O problem →
            //    `degraded` + retain the journal for an idempotent retry, never a checkpoint over a
            //    still-stale row.
            let existing = match ledger.get(id) {
                Ok(row) => row,
                Err(e) => {
                    outcome.degraded.push(format!("recover abort preserve read {}: {e}", id.as_str()));
                    continue;
                }
            };
            let row_matches_live = matches!(
                (&existing, live),
                (Some(e), Some(fp)) if e.state.is_committed() && e.last_applied_fingerprint == fp
            );
            if row_matches_live {
                continue; // ledger already reflects the live desktop — a correctly-tracked prior style
            }
            if let Err(e) = ledger.remove(id) {
                outcome
                    .degraded
                    .push(format!("recover abort preserve ledger remove {}: {e}", id.as_str()));
                continue;
            }
            outcome.preserved.push(id.clone());
            continue;
        }
        // ALREADY at its true original (owner 2026-07-16): `is_ours` holds because the live surface is
        // EITHER this txn's applied style OR its original. When it is already the original, the styling
        // write never landed (crash / Access-Denied before the write), so the desktop is already
        // correct — writing the original back is a POINTLESS restore. And that pointless write can
        // itself FAIL forever on a permission-protected target (a `C:\Users\Public\Desktop\*.lnk` an
        // unelevated apply can never write): the failed restore degrades, the txn never checkpoints,
        // and every future apply/reset wedges on the un-clearable crash. So never write to restore a
        // value that already matches: drop any stale row (a no-op for an incomplete txn that committed
        // none) and move on — nothing moved, so this item is NOT counted as `aborted`.
        if live == Some(rec.original_fingerprint) {
            if let Err(e) = ledger.remove(id) {
                outcome.degraded.push(format!("recover abort ledger remove {}: {e}", id.as_str()));
            }
            continue;
        }
        // §14 elevated crash-recovery (M8): the live surface is this txn's APPLIED style (`is_ours`
        // held and `live != original` was ruled out just above, so `live == new_fingerprint`). If the
        // target is privileged-scope (Public Desktop / ProgramData), it can ONLY have been styled by
        // the ELEVATED helper, and the unelevated `applier` here can NEVER revert it (Access Denied) —
        // a doomed `restore` would degrade recovery forever (the exact wedge the on-box report hit).
        // Since the desktop provably wears OUR style, ADOPT it FORWARD instead of rolling back: rebuild
        // the committed ledger row from the journal so desktop == ledger and the item stays reversible
        // via the (now-wired) elevated reset path. This fires ONLY for `live == new_fingerprint`, so a
        // user's own elevated edit (`live == other`) never reaches here — it was `preserve`d above.
        if scope.classify(&rec.target.path).is_some() {
            reconcile_committed_row(rec, id, ledger, outcome);
            continue;
        }
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
        reconcile_committed_row(&group.items[id.as_str()], id, ledger, outcome);
    }
    Ok(())
}

/// Rebuild ONE item's committed ledger row from its journal records (asset + confirmed
/// `new_fingerprint` + anchor) and upsert it, unless the ledger already matches. Shared by
/// [`reconcile_committed`] (the crash-in-commit→upsert-gap close) AND `abort_incomplete`'s §14
/// elevated adopt-forward (a privileged item the elevated helper styled that the unelevated applier
/// cannot revert). Best-effort: any ledger fault is recorded in `outcome.degraded` (the journal then
/// stays for an idempotent retry) rather than bailing the whole recovery — this never touches the
/// desktop, only the ledger. Pushes the id to `outcome.reconciled` on a real upsert.
fn reconcile_committed_row(
    rec: &ItemRecovery,
    id: &dm_domain::ItemId,
    ledger: &mut dyn LedgerStore,
    outcome: &mut RecoveryOutcome,
) {
    let (Some(asset), Some(new_fp)) = (rec.asset.clone(), rec.new_fingerprint) else {
        // Reached this row without a full apply record (asset + new_fingerprint) — impossible under
        // the driver's / elevated batch's ordering (both journal AssetWritten + ItemApplied before a
        // commit or an elevated helper call), but skip defensively rather than fabricate an entry.
        return;
    };
    // Skip an already-reconciled row ONLY when it matches the journal in full — including the
    // paired empty ref. A legacy row committed before empty_asset existed loads as `None`; if we
    // skipped it on the fingerprint alone, the exact empty ref the journal carries would never be
    // persisted and would then be checkpointed away, orphaning the empty ICO (new-P1, wave-2R).
    // Best-effort (codex R4-Block 5): a ledger read/write fault reconciling ONE item must not bail
    // the whole recovery — this path never touches the desktop (the item already sits at its styled
    // state), so a fault just leaves the row unreconciled for an idempotent retry. Record + continue.
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
            return;
        }
    };
    if already {
        return;
    }
    let version = match ledger.next_version() {
        Ok(v) => v,
        Err(e) => {
            outcome.degraded.push(format!("recover reconcile version {}: {e}", id.as_str()));
            return;
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
        return;
    }
    outcome.reconciled.push(id.clone());
}
