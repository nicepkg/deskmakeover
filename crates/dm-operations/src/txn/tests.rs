//! Behavioural tests for the durable transaction machinery, including the kill-point recovery
//! battery. Every named test corresponds to an invariant harvested from the frozen C# oracle
//! (cited inline) or a spec/ADR requirement.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use dm_domain::{Fingerprint, ItemId, ItemKind, ItemTarget, OwnedFields};

use super::driver::{ApplyRequest, TxnDriver};
use super::id::TxnIdAllocator;
use super::fakes::{
    paired_empty_path, styled_bytes, FailingAssetStore, FailingJournal, FakePlatform,
    RecordingJournal, World,
};
use super::journal::{JournalRecord, VecJournal};
use super::recovery::{recover, recover_from_journal, repair_pending, RecoveryOutcome};
use crate::error::{OperationError, Result};
use crate::ledger::entry::LedgerEntry;
use crate::ledger::store::{JsonLedgerStore, LedgerStore, MemLedgerStore};
use crate::ledger::TxnState;
use dm_domain::ItemId as DomainItemId;

fn target(name: &str) -> ItemTarget {
    ItemTarget::new(ItemId::from_raw(name), ItemKind::Shortcut, format!("C:/Desktop/{name}.lnk"))
}

fn seed(world: &Rc<RefCell<World>>, t: &ItemTarget, original: &[u8]) {
    world.borrow_mut().put(&t.path, original);
}

fn request(t: &ItemTarget, world: &Rc<RefCell<World>>, asset_hash: &str) -> ApplyRequest {
    let current = Fingerprint::of_bytes(&world.borrow().get(&t.path).unwrap());
    ApplyRequest {
        target: t.clone(),
        expected_fingerprint: current,
        owned: OwnedFields::icon_only(),
        asset_hash: asset_hash.into(),
        asset_bytes: b"ico-bytes".to_vec(),
        empty_asset_bytes: None,
        pinned_seed: None,
    }
}

#[test]
fn apply_happy_path_commits_and_styles() {
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let out = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();

    assert_eq!(out.committed, vec![ItemId::from_raw("A")]);
    assert!(out.conflicts.is_empty() && out.error.is_none());
    assert_eq!(world.borrow().get(&a.path).unwrap(), styled_bytes("hashA"));
    let entry = ledger.get(&a.id).unwrap().unwrap();
    assert_eq!(entry.state, TxnState::Committed);
    assert_eq!(entry.last_applied_fingerprint, Fingerprint::of_bytes(&styled_bytes("hashA")));
    assert_eq!(entry.original_fingerprint, Fingerprint::of_bytes(b"orig-A"));
}

#[test]
fn preflight_conflict_when_content_changed_after_snapshot() {
    // Oracle: DesktopIconApplyOperationsTests.Shortcut_operation_aborts_when_target_changed
    // — but ADR-0020 §2 makes it a per-item skip, not a batch abort.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    // Build the request against orig-A, then an external process changes the file.
    let req = request(&a, &world, "hashA");
    world.borrow_mut().put(&a.path, b"changed-elsewhere");

    let out = driver.apply(1, vec![req], &mut journal, &mut ledger).unwrap();

    assert_eq!(out.conflicts, vec![ItemId::from_raw("A")]);
    assert!(out.committed.is_empty());
    // Never overwritten: the external change stands, nothing was journaled.
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"changed-elsewhere");
    assert!(journal.records().is_empty());
}

#[test]
fn anchor_is_written_before_any_mutation() {
    // Oracle: DesktopBakeService codex B4 — the restore anchor is declared before the desktop
    // is touched. Assert ItemPrepared (which carries the anchor) precedes AssetWritten/Applied.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();

    let recs = journal.records();
    let prepared_at = recs.iter().position(|r| matches!(r, JournalRecord::ItemPrepared { .. })).unwrap();
    let asset_at = recs.iter().position(|r| matches!(r, JournalRecord::AssetWritten { .. })).unwrap();
    let applied_at = recs.iter().position(|r| matches!(r, JournalRecord::ItemApplied { .. })).unwrap();
    assert!(prepared_at < asset_at, "anchor must precede the asset write");
    assert!(asset_at < applied_at, "asset write must precede the mutation (write-new-then-swap)");
    // The anchor carries the true original bytes.
    if let JournalRecord::ItemPrepared { anchor, .. } = &recs[prepared_at] {
        assert!(anchor.has_material());
    }
}

#[test]
fn rollback_is_lifo_and_replays_original_content() {
    // Oracle: DesktopIconApplyOperationsTests.Journaled_runner_rolls_back_completed_operation.
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    world.borrow_mut().fail_apply(&b.path); // second item's mutation fails
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let out = driver
        .apply(1, vec![request(&a, &world, "hashA"), request(&b, &world, "hashB")], &mut journal, &mut ledger)
        .unwrap();

    assert!(out.error.is_some());
    assert!(out.committed.is_empty());
    // Both items are back to their originals; the ledger holds nothing.
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A");
    assert_eq!(world.borrow().get(&b.path).unwrap(), b"orig-B");
    assert!(ledger.all().unwrap().is_empty());
    // Rollback order is LIFO: B rolled back before A.
    let rollback_ids: Vec<_> = journal
        .records()
        .iter()
        .filter_map(|r| match r {
            JournalRecord::ItemRolledBack { item, .. } => Some(item.as_str().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(rollback_ids, vec!["B".to_string(), "A".to_string()]);
}

#[test]
fn reapply_pins_true_original_and_uses_last_applied_for_cas() {
    // ADR-0019/0020: re-apply is incremental; the true original stays pinned; CAS compares the
    // live state against the ledger's last-applied fingerprint, not the scan observation.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();

    // Re-apply with a new asset. The request's expected fp is stale (it names styled-A), but the
    // driver overrides it with the ledger's last-applied, so CAS still passes.
    let mut req2 = request(&a, &world, "hashB"); // request() reads current == styled-A
    req2.expected_fingerprint = Fingerprint::of_bytes(b"totally-wrong");
    let out = driver.apply(2, vec![req2], &mut journal, &mut ledger).unwrap();

    assert_eq!(out.committed, vec![ItemId::from_raw("A")]);
    let entry = ledger.get(&a.id).unwrap().unwrap();
    // Original preserved; last-applied advanced to style B.
    assert_eq!(entry.original_fingerprint, Fingerprint::of_bytes(b"orig-A"));
    assert_eq!(entry.last_applied_fingerprint, Fingerprint::of_bytes(&styled_bytes("hashB")));
    assert_eq!(world.borrow().get(&a.path).unwrap(), styled_bytes("hashB"));
}

#[test]
fn reapply_conflicts_when_externally_modified() {
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();
    driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();

    // A user/installer changes the styled shortcut out from under us.
    world.borrow_mut().put(&a.path, b"user-edited");
    let out = driver.apply(2, vec![request(&a, &world, "hashC")], &mut journal, &mut ledger).unwrap();

    assert_eq!(out.conflicts, vec![ItemId::from_raw("A")]);
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"user-edited"); // untouched
}

#[test]
fn capture_failed_item_is_skipped() {
    // Oracle: SnapshotRestoreVerifier.HasRestoreMaterial — an item without restore material is
    // never styled (no way back).
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    plat.fail_capture(&a.path);
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let out = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();
    assert_eq!(out.conflicts, vec![ItemId::from_raw("A")]);
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A"); // untouched
}

#[test]
fn pinned_hue_seed_is_recorded_and_existing_entries_never_reflow() {
    // ADR-0020 §2: background additions allocate against pinned seeds; existing icons keep theirs.
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let mut req_a = request(&a, &world, "hashA");
    req_a.pinned_seed = Some(11);
    driver.apply(1, vec![req_a], &mut journal, &mut ledger).unwrap();

    let mut req_b = request(&b, &world, "hashB");
    req_b.pinned_seed = Some(22);
    driver.apply(2, vec![req_b], &mut journal, &mut ledger).unwrap();

    assert_eq!(ledger.get(&a.id).unwrap().unwrap().pinned_seed, Some(11));
    assert_eq!(ledger.get(&b.id).unwrap().unwrap().pinned_seed, Some(22));
}

#[test]
fn reconcile_committed_rebuilds_lost_ledger_entry() {
    // Crash in the commit→upsert gap: journal says committed, ledger is empty. Recovery must
    // rebuild the entry (a corrupt/absent ledger never reads as "nothing applied").
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut applied_ledger = MemLedgerStore::new();
    driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut applied_ledger).unwrap();

    // Simulate the lost ledger write: recover into a fresh empty ledger.
    let mut fresh_ledger = MemLedgerStore::new();
    let out = recover(journal.records(), &plat, &plat, &mut fresh_ledger).unwrap();
    assert_eq!(out.reconciled, vec![ItemId::from_raw("A")]);
    let entry = fresh_ledger.get(&a.id).unwrap().unwrap();
    assert_eq!(entry.state, TxnState::Committed);
    assert_eq!(entry.last_applied_fingerprint, Fingerprint::of_bytes(&styled_bytes("hashA")));
    // Desktop stays styled (committed work is not undone).
    assert_eq!(world.borrow().get(&a.path).unwrap(), styled_bytes("hashA"));
}

#[test]
fn rollback_restore_failure_withholds_terminal_so_recovery_can_retry() {
    // Oracle: DesktopBakeService rollback-incomplete handling — if a rollback restore fails,
    // the anchor/terminal is withheld so the next startup can retry (restore is idempotent).
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    world.borrow_mut().fail_apply(&b.path); // B's mutation fails → triggers rollback
    world.borrow_mut().fail_restore(&a.path); // ...and A cannot be rolled back this attempt
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let out = driver
        .apply(1, vec![request(&a, &world, "hashA"), request(&b, &world, "hashB")], &mut journal, &mut ledger)
        .unwrap();

    assert!(out.error.unwrap().contains("rollback-incomplete"));
    // No terminal record: the transaction stays incomplete for recovery to retry.
    assert!(!journal.records().iter().any(|r| matches!(r, JournalRecord::TxnRolledBack { .. })));
    assert!(ledger.get(&a.id).unwrap().is_none()); // never committed

    // The transient fault clears; startup recovery finishes the job exactly.
    world.borrow_mut().clear_faults();
    recover(journal.records(), &plat, &plat, &mut ledger).unwrap();
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A");
    assert_eq!(world.borrow().get(&b.path).unwrap(), b"orig-B");
    assert!(ledger.all().unwrap().is_empty());
}

#[test]
fn durably_rolled_back_item_is_not_restored_again_over_a_user_edit() {
    // codex E recovery:250: an item durably rolled back during the txn's own rollback
    // (`ItemRolledBack` journaled) is already terminal — restored to its original. If the txn-level
    // terminal write is then lost (crash right after the per-item rollback) the whole txn still
    // reads as "incomplete". Recovery must NOT restore that item a SECOND time: replaying the
    // anchor over a user edit made between rollback and restart destroys the edit.
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    world.borrow_mut().fail_apply(&b.path); // B's mutation fails → the driver rolls A (and B) back
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();
    driver
        .apply(1, vec![request(&a, &world, "hashA"), request(&b, &world, "hashB")], &mut journal, &mut ledger)
        .unwrap();

    // A is back on its original after the in-call rollback, and ItemRolledBack(A) is durable.
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A");
    let mut records: Vec<JournalRecord> = journal.records().to_vec();
    assert!(records
        .iter()
        .any(|r| matches!(r, JournalRecord::ItemRolledBack { item, .. } if item.as_str() == "A")));
    // Simulate the lost txn-level terminal: drop TxnRolledBack, keeping the per-item rollbacks.
    records.retain(|r| !matches!(r, JournalRecord::TxnRolledBack { .. }));

    // Between the rollback and restart the user re-styles A themselves.
    world.borrow_mut().put(&a.path, b"user-edit-after-rollback");

    let mut fresh = MemLedgerStore::new();
    let out = recover(&records, &plat, &plat, &mut fresh).unwrap();

    // The durably-rolled-back item is left exactly as the user left it — never re-restored.
    assert_eq!(
        world.borrow().get(&a.path).unwrap(),
        b"user-edit-after-rollback",
        "durably rolled-back A must not be restored again over the user edit"
    );
    assert!(!out.aborted.contains(&ItemId::from_raw("A")));
    assert!(out.degraded.is_empty(), "a clean skip is not a degraded outcome");
}

#[test]
fn a_user_edit_between_crash_and_restart_is_preserved_never_clobbered() {
    // recovery:265 (owner-approved 2026-07-14, 極致 UX): the ONE outcome we never accept is silently
    // destroying the user's own customization. An incomplete txn crashed after PREPARING A but before
    // applying it; the user then replaces A's icon themselves before restart. Recovery must LEAVE the
    // user's edit exactly as found and surface it — never blind-restore the journaled anchor over it.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();
    driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();

    // Keep only up to ItemPrepared: an incomplete txn that crashed after preparing A, before any
    // desktop write — so `new_fingerprint` is absent and the anchor's original is the only thing we
    // journaled about A.
    let records: Vec<JournalRecord> = journal
        .records()
        .iter()
        .take_while(|r| !matches!(r, JournalRecord::AssetWritten { .. }))
        .cloned()
        .collect();
    assert!(records.iter().any(|r| matches!(r, JournalRecord::ItemPrepared { .. })));
    assert!(!records.iter().any(|r| matches!(r, JournalRecord::ItemApplied { .. })));

    // The user chooses their own icon for A before restart.
    world.borrow_mut().put(&a.path, b"user-chosen-icon");

    let mut fresh = MemLedgerStore::new();
    let out = recover(&records, &plat, &plat, &mut fresh).unwrap();

    assert_eq!(
        world.borrow().get(&a.path).unwrap(),
        b"user-chosen-icon",
        "a user edit must be preserved, never clobbered by recovery"
    );
    assert_eq!(out.preserved, vec![ItemId::from_raw("A")], "the item is surfaced as preserved");
    assert!(out.aborted.is_empty(), "nothing was restored over the user's edit");
    assert!(out.degraded.is_empty(), "a deliberate preserve is not a runtime fault");
}

/// The kill-point battery: run a two-item apply, then for every truncation of the journal
/// replay recovery against the world as it was at that fsync — plus a "torn/foreign write" variant —
/// and assert: a CLEAN crash restores each incomplete item EXACTLY to its original (or leaves a
/// committed item on its target); a torn/foreign write we cannot identify as ours is LEFT exactly as
/// found (never-clobber, recovery:265) and surfaced. Full idempotency throughout.
#[test]
fn killpoint_recovery_leaves_each_item_original_or_target() {
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    let originals: HashMap<String, Vec<u8>> = world.borrow().snapshot();

    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = RecordingJournal::new(world.clone());
    let mut ledger = MemLedgerStore::new();
    driver
        .apply(1, vec![request(&a, &world, "hashA"), request(&b, &world, "hashB")], &mut journal, &mut ledger)
        .unwrap();

    let full: Vec<JournalRecord> = journal.records().to_vec();
    let snaps: Vec<HashMap<String, Vec<u8>>> = journal.snapshots().to_vec();

    let expected_target: HashMap<&str, Vec<u8>> =
        [("A", styled_bytes("hashA")), ("B", styled_bytes("hashB"))].into_iter().collect();

    for k in 0..=full.len() {
        let records = &full[..k];
        // World exactly as it was when record[k-1] became durable (k==0 → pre-transaction).
        let world_at_crash = if k == 0 { originals.clone() } else { snaps[k - 1].clone() };
        let committed = records.iter().any(|r| matches!(r, JournalRecord::TxnCommitted { .. }));

        // Variant 1: clean crash (desktop exactly at the snapshot). At an aligned snapshot the live
        // state is always either the true original or exactly the style we applied, so never-clobber
        // restores every incomplete item to its original.
        assert_recovers_consistently(records, &world_at_crash, &originals, &expected_target, committed, k);

        // Variant 2: torn/foreign write — every prepared item's file is garbage we cannot identify as
        // ours. NEVER-CLOBBER (recovery:265): recovery must LEAVE it exactly as found (it could be the
        // user's own edit made between crash and restart) and surface it, never blind-restore over it.
        // A committed item's file is authoritative and is not corrupted in this variant.
        if !committed {
            let mut torn = world_at_crash.clone();
            for id in prepared_ids(records) {
                torn.insert(format!("C:/Desktop/{id}.lnk"), b"TORN-GARBAGE".to_vec());
            }
            assert_preserves_unrecognized(records, &torn, k);
        }
    }
}

/// Never-clobber (recovery:265): every incomplete item whose live state we cannot confirm is ours is
/// LEFT exactly as found, reported in `preserved`, and never given a ledger row. Idempotent.
fn assert_preserves_unrecognized(records: &[JournalRecord], world_at_crash: &HashMap<String, Vec<u8>>, k: usize) {
    let world = Rc::new(RefCell::new(World::default()));
    world.borrow_mut().restore_snapshot(world_at_crash.clone());
    let plat = FakePlatform::new(world.clone());
    let mut ledger = MemLedgerStore::new();

    let out1 = recover(records, &plat, &plat, &mut ledger).unwrap();
    let out2 = recover(records, &plat, &plat, &mut ledger).unwrap(); // idempotency

    let prepared = prepared_ids(records);
    for name in ["A", "B"] {
        if !prepared.contains(&name.to_string()) {
            continue; // not yet prepared at this truncation → not corrupted, not asserted
        }
        let path = format!("C:/Desktop/{name}.lnk");
        assert_eq!(
            world.borrow().get(&path).unwrap(),
            b"TORN-GARBAGE",
            "k={k}: unrecognized incomplete item {name} must be left EXACTLY as found (never-clobber)"
        );
        assert!(
            out1.preserved.iter().any(|i| i.as_str() == name),
            "k={k}: {name} must be surfaced as preserved, not silently overwritten"
        );
        assert!(
            out1.aborted.iter().all(|i| i.as_str() != name),
            "k={k}: {name} must NOT be restored"
        );
        assert!(
            ledger.get(&ItemId::from_raw(name)).unwrap().is_none(),
            "k={k}: a preserved item has no ledger row"
        );
    }
    assert_eq!(out1.preserved.len(), out2.preserved.len(), "k={k}: preserve is idempotent");
}

fn prepared_ids(records: &[JournalRecord]) -> Vec<String> {
    records
        .iter()
        .filter_map(|r| match r {
            JournalRecord::ItemPrepared { item, .. } => Some(item.as_str().to_string()),
            _ => None,
        })
        .collect()
}

fn assert_recovers_consistently(
    records: &[JournalRecord],
    world_at_crash: &HashMap<String, Vec<u8>>,
    originals: &HashMap<String, Vec<u8>>,
    expected_target: &HashMap<&str, Vec<u8>>,
    committed: bool,
    k: usize,
) {
    // Fresh world seeded to the crash state; fresh empty ledger (worst case: ledger writes lost).
    let world = Rc::new(RefCell::new(World::default()));
    world.borrow_mut().restore_snapshot(world_at_crash.clone());
    let plat = FakePlatform::new(world.clone());
    let mut ledger = MemLedgerStore::new();

    recover(records, &plat, &plat, &mut ledger).unwrap();
    // Idempotency: a second pass changes nothing.
    recover(records, &plat, &plat, &mut ledger).unwrap();

    for name in ["A", "B"] {
        let path = format!("C:/Desktop/{name}.lnk");
        let live = world.borrow().get(&path).unwrap();
        if committed {
            assert_eq!(
                live, expected_target[name],
                "k={k}: committed item {name} must stay on its target"
            );
            assert!(
                ledger.get(&ItemId::from_raw(name)).unwrap().is_some(),
                "k={k}: committed item {name} must have a ledger entry"
            );
        } else {
            assert_eq!(
                live, originals[&path],
                "k={k}: incomplete item {name} must be restored EXACTLY to its original"
            );
            assert!(
                ledger.get(&ItemId::from_raw(name)).unwrap().is_none(),
                "k={k}: incomplete item {name} must have no ledger entry"
            );
        }
    }
}

// ---- Failure-branch coverage: one test per driver/recovery transition that can fail ----

/// A ledger whose `upsert` always fails (simulates the commit→ledger write dying).
struct FailingLedger;
impl LedgerStore for FailingLedger {
    fn upsert(&mut self, _entry: LedgerEntry) -> Result<()> {
        Err(OperationError::Io("injected ledger upsert failure".into()))
    }
    fn get(&self, _item: &DomainItemId) -> Result<Option<LedgerEntry>> {
        Ok(None)
    }
    fn all(&self) -> Result<Vec<LedgerEntry>> {
        Ok(Vec::new())
    }
    fn remove(&mut self, _item: &DomainItemId) -> Result<()> {
        Ok(())
    }
}

#[test]
fn read_error_during_preflight_fails_the_batch() {
    // P2-5: a non-NotFound reader error (COM/IO) is a real infrastructure failure, NOT a benign
    // per-item conflict. It must fail the batch with `outcome.error` set, so the operator learns
    // the restore path may be compromised — never a silent skip.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let req = request(&a, &world, "hashA");
    world.borrow_mut().fail_read(&a.path); // reader errors on the CAS read
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let out = driver.apply(1, vec![req], &mut journal, &mut ledger).unwrap();
    assert!(out.error.is_some(), "a real reader error must surface as a batch failure");
    assert!(out.conflicts.is_empty(), "a real error is not a benign conflict");
    assert!(out.committed.is_empty());
    assert!(journal.records().is_empty()); // nothing entered the transaction
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A"); // untouched
}

#[test]
fn anchor_capture_error_during_preflight_fails_the_batch() {
    // P2-3: a hard capture failure (locked file / COM / registry error) is a real infrastructure
    // problem, not a benign skip. It must fail the batch with error set, so the compromised
    // restore path is surfaced rather than silently not styling. (A capture with NO material still
    // skips — see capture_failed_item_is_skipped.)
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    plat.error_capture(&a.path);
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let out = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();
    assert!(out.error.is_some(), "a hard capture error must surface as a batch failure");
    assert!(out.conflicts.is_empty(), "a hard capture error is not a benign conflict");
    assert!(out.committed.is_empty());
    assert!(journal.records().is_empty());
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A"); // untouched, never styled
}

#[test]
fn corrupt_ledger_during_preflight_fails_the_batch_not_overwrites() {
    // A corrupt active ledger must fail CLOSED at prepare time: the item is never styled (which
    // could strand the only path back), AND — per P2-5 — the failure surfaces as a batch error
    // rather than a benign conflict that hides the compromised restore path.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.json");
    std::fs::write(&path, b"{ not json").unwrap();
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = JsonLedgerStore::new(&path);

    let out = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();
    assert!(out.error.is_some(), "a corrupt ledger must surface as a batch failure");
    assert!(out.conflicts.is_empty(), "a corrupt ledger is not a benign conflict");
    assert!(out.committed.is_empty());
    assert!(journal.records().is_empty()); // nothing entered the transaction
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A"); // untouched, never styled
}

#[test]
fn asset_write_failure_rolls_back_before_any_mutation() {
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let assets = FailingAssetStore;
    let driver = TxnDriver::new(&plat, &plat, &assets); // asset store fails
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let out = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();
    assert!(out.error.is_some());
    assert!(out.committed.is_empty());
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A"); // no mutation happened
    assert!(ledger.all().unwrap().is_empty());
}

#[test]
fn noop_apply_fails_verification_and_rolls_back() {
    // An apply that "succeeds" but changes nothing must be caught by verify (new == original)
    // and rolled back, never committed.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    world.borrow_mut().noop_apply(&a.path);
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let out = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();
    assert!(out.error.is_some());
    assert!(out.committed.is_empty());
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A");
    assert!(ledger.all().unwrap().is_empty());
}

#[test]
fn apply_that_lands_a_different_asset_than_requested_does_not_commit() {
    // P1-4: verify must check "matches the requested asset", not merely "changed from original".
    // A writer that lands a stale/other asset leaves a state that differs from the true original
    // (so the old `new_fp == original` guard passed) yet does NOT match the asset the driver was
    // asked to apply — committing it would poison the ledger with an asset the desktop never shows.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    world.borrow_mut().wrong_write(&a.path); // apply lands a different asset than hashA
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let out = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();

    // The mismatch is caught: nothing commits, the batch fails, and the item is walked back.
    assert!(out.committed.is_empty(), "a stale/wrong-asset apply must never commit");
    assert!(out.error.is_some());
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A"); // rolled back to the true original
    assert!(ledger.all().unwrap().is_empty());
}

#[test]
fn driver_rejects_a_reused_txn_id() {
    // P1-7: a txn id must be strictly greater than any already in the journal, so a reused id can
    // never merge two transactions' records under one id (which recovery would misclassify).
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();
    // Reusing id 1 (≤ the max already journalled) must be rejected before any mutation.
    let result = driver.apply(1, vec![request(&b, &world, "hashB")], &mut journal, &mut ledger);
    assert!(matches!(result, Err(OperationError::Journal(_))));
    assert_eq!(world.borrow().get(&b.path).unwrap(), b"orig-B"); // B never touched
}

#[test]
fn allocator_feeds_the_driver_monotonic_ids_that_survive_restart() {
    // P1-7: the composition root drives the id from TxnIdAllocator, which resumes past the journal
    // after a crash so ids never regress.
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let mut alloc = TxnIdAllocator::from_journal(&journal).unwrap();
    let id1 = alloc.next_id();
    driver.apply(id1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();
    let id2 = alloc.next_id();
    driver.apply(id2, vec![request(&b, &world, "hashB")], &mut journal, &mut ledger).unwrap();
    assert_eq!((id1, id2), (1, 2));
    // A fresh allocator (process restart) resumes past the durable journal, never back to 1.
    let resumed = TxnIdAllocator::from_journal(&journal).unwrap();
    assert!(resumed.peek() > id2, "allocator must resume past the journal after a crash");
}

#[test]
fn recovery_fails_closed_when_one_txn_id_has_both_terminals() {
    // P1-7: if a committed txn's id is reused by a later rolled-back txn, their records merge under
    // one id. The old code let rolled-back win and silently DROPPED the committed work. A single id
    // with both terminals is definitively id reuse / corruption — recovery must fail closed, not
    // guess (neither terminal is correct for all the merged items).
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();
    driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();

    // Simulate id reuse: the same id later also acquires a rollback terminal.
    let mut records = journal.records().to_vec();
    records.push(JournalRecord::TxnRolledBack { txn: 1 });

    let mut fresh = MemLedgerStore::new();
    let result = recover(&records, &plat, &plat, &mut fresh);
    assert!(
        matches!(result, Err(OperationError::Journal(_))),
        "a txn id bearing both terminals must fail closed, not misclassify committed work"
    );
}

#[test]
fn both_terminals_fail_closed_before_any_earlier_incomplete_txn_mutates_the_desktop() {
    // codex R5-#4: the both-terminals corruption check must be a structural PREFLIGHT over every txn,
    // run before the drain loop touches anything. An EARLIER incomplete txn in first-seen order would
    // otherwise be aborted first — restoring (mutating) its item on the desktop — and only THEN would
    // the corrupt txn surface the Err, leaving a bare Err over a half-recovered desktop. Here txn 10 is
    // incomplete (its item styled on disk, would revert on abort) and txn 20 carries both terminals;
    // recovery must fail closed with txn 10's item STILL styled (never aborted).
    let a = target("A");
    let b = target("B");
    let orig_a = b"orig-A".to_vec();
    let styled_a = styled_bytes("hash10");
    let styled_a_fp = Fingerprint::of_bytes(&styled_a);
    let anchor_a = dm_domain::RestoreAnchor::FileBytes { bytes: orig_a.clone() };

    let records = vec![
        // txn 10 — incomplete: A prepared + applied (styled on disk), NO terminal → an abort candidate.
        JournalRecord::TxnBegin { txn: 10, items: vec![a.id.clone()] },
        JournalRecord::ItemPrepared {
            txn: 10,
            item: a.id.clone(),
            target: a.clone(),
            anchor: anchor_a,
            original_fingerprint: Fingerprint::of_bytes(&orig_a),
            expected_fingerprint: Fingerprint::of_bytes(&orig_a),
            asset_hash: "hash10".into(),
            owned: OwnedFields::icon_only(),
            pinned_seed: None,
        },
        JournalRecord::AssetWritten {
            txn: 10,
            item: a.id.clone(),
            asset: dm_domain::AssetRef::new("hash10", "assets/hash10.ico"),
            empty: None,
        },
        JournalRecord::ItemApplied { txn: 10, item: a.id.clone(), new_fingerprint: styled_a_fp },
        // txn 20 — corrupt: id reuse merged a commit AND a rollback terminal under one id.
        JournalRecord::TxnBegin { txn: 20, items: vec![b.id.clone()] },
        JournalRecord::TxnCommitted { txn: 20 },
        JournalRecord::TxnRolledBack { txn: 20 },
    ];

    // The desktop reflects txn 10's styling at crash time.
    let world = World::shared();
    seed(&world, &a, &styled_a);
    let plat = FakePlatform::new(world.clone());
    let mut ledger = MemLedgerStore::new();

    let result = recover(&records, &plat, &plat, &mut ledger);
    assert!(
        matches!(result, Err(OperationError::Journal(_))),
        "both-terminals corruption must fail closed"
    );
    assert_eq!(
        world.borrow().get(&a.path).unwrap(),
        styled_a,
        "the earlier incomplete txn must NOT have been aborted — the preflight fails before any mutation"
    );
}

#[test]
fn repair_pending_covers_committed_unreconciled_and_incomplete_but_not_terminal_states() {
    // codex R6-#6/#1: `repair_pending` is the "a styled desktop may exist that the ledger doesn't yet
    // reflect" signal that keeps the restore affordance reachable. It must cover BOTH an incomplete txn
    // (abort candidate) AND a committed txn whose ledger upsert never landed (the driver writes
    // TxnCommitted BEFORE the ledger upsert, which can then fault) — the latter is exactly what the old
    // `active_txns` signal missed. A rolled-back txn, and a committed txn already in the ledger, are safe.
    let a = target("A");
    let prepared = |txn: u64| JournalRecord::ItemPrepared {
        txn,
        item: a.id.clone(),
        target: a.clone(),
        anchor: dm_domain::RestoreAnchor::FileBytes { bytes: b"orig".to_vec() },
        original_fingerprint: Fingerprint::of_bytes(b"orig"),
        expected_fingerprint: Fingerprint::of_bytes(b"orig"),
        asset_hash: "h".into(),
        owned: OwnedFields::icon_only(),
        pinned_seed: None,
    };
    let empty = MemLedgerStore::new();

    // Committed terminal, but the ledger upsert never landed → repair pending (active_txns would miss it).
    let committed = vec![
        JournalRecord::TxnBegin { txn: 1, items: vec![a.id.clone()] },
        prepared(1),
        JournalRecord::ItemApplied { txn: 1, item: a.id.clone(), new_fingerprint: Fingerprint::of_bytes(b"styled") },
        JournalRecord::TxnCommitted { txn: 1 },
    ];
    assert!(repair_pending(&committed, &empty).unwrap(), "committed-but-unreconciled → pending");

    // Incomplete (no terminal) → repair pending.
    let incomplete = vec![JournalRecord::TxnBegin { txn: 1, items: vec![a.id.clone()] }, prepared(1)];
    assert!(repair_pending(&incomplete, &empty).unwrap(), "incomplete → pending");

    // Rolled-back terminal → NOT pending (desktop restored, ledger clean).
    let rolled_back = vec![
        JournalRecord::TxnBegin { txn: 1, items: vec![a.id.clone()] },
        prepared(1),
        JournalRecord::TxnRolledBack { txn: 1 },
    ];
    assert!(!repair_pending(&rolled_back, &empty).unwrap(), "rolled-back → not pending");

    // A committed txn WITH its ledger row present (a normal post-apply journal awaiting checkpoint) is
    // reconciled → NOT pending, so a healthy desktop never spuriously shows repair.
    let world = World::shared();
    seed(&world, &a, b"orig");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();
    driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();
    assert!(
        !repair_pending(journal.records(), &ledger).unwrap(),
        "committed + present ledger row → reconciled → not pending"
    );
}

#[test]
fn an_abandoned_txn_does_not_clobber_a_later_committed_txn_on_the_same_item() {
    // New-P1 (abandon is not a write fence): txn 1 abandons item A (restores the original, writes NO
    // journal terminal), then txn 2 re-styles A and commits. Both sets of records coexist in the log
    // (no checkpoint between them — the abandon-then-retry window). On recovery the terminal-less
    // txn 1 must NOT re-abort A over txn 2's committed result, or the desktop lands on the original
    // (O) while the ledger says committed (C) — an unrecoverable split.
    let a = target("A");
    let orig_fp = Fingerprint::of_bytes(b"orig-A");
    let anchor = dm_domain::RestoreAnchor::FileBytes { bytes: b"orig-A".to_vec() };
    let styled2 = styled_bytes("hash2");
    let styled2_fp = Fingerprint::of_bytes(&styled2);
    let asset2 = dm_domain::AssetRef::new("hash2", "assets/hash2.ico");

    let records = vec![
        // txn 1 — abandoned: prepared A, then abandon left NO terminal record.
        JournalRecord::TxnBegin { txn: 1, items: vec![a.id.clone()] },
        JournalRecord::ItemPrepared {
            txn: 1,
            item: a.id.clone(),
            target: a.clone(),
            anchor: anchor.clone(),
            original_fingerprint: orig_fp,
            expected_fingerprint: orig_fp,
            asset_hash: "hash1".into(),
            owned: OwnedFields::icon_only(),
            pinned_seed: None,
        },
        // txn 2 — re-styled A to hash2 and committed durably.
        JournalRecord::TxnBegin { txn: 2, items: vec![a.id.clone()] },
        JournalRecord::ItemPrepared {
            txn: 2,
            item: a.id.clone(),
            target: a.clone(),
            anchor: anchor.clone(),
            original_fingerprint: orig_fp,
            expected_fingerprint: orig_fp,
            asset_hash: "hash2".into(),
            owned: OwnedFields::icon_only(),
            pinned_seed: None,
        },
        JournalRecord::AssetWritten { txn: 2, item: a.id.clone(), asset: asset2, empty: None },
        JournalRecord::ItemApplied { txn: 2, item: a.id.clone(), new_fingerprint: styled2_fp },
        JournalRecord::ItemVerified { txn: 2, item: a.id.clone() },
        JournalRecord::TxnCommitted { txn: 2 },
    ];

    // The desktop at crash time reflects txn 2's committed styling (txn 1 restored O, txn 2 applied
    // hash2, crash after commit).
    let world = World::shared();
    seed(&world, &a, &styled2);
    let plat = FakePlatform::new(world.clone());
    let mut ledger = MemLedgerStore::new();

    let out = recover(&records, &plat, &plat, &mut ledger).unwrap();

    // txn 1's abort must be suppressed for A; txn 2's commit reconciled.
    assert!(!out.aborted.contains(&a.id), "the committed item must not be aborted by the earlier txn");
    assert_eq!(out.reconciled, vec![a.id.clone()]);
    assert_eq!(
        world.borrow().get(&a.path).unwrap(),
        styled2,
        "desktop must keep txn 2's committed styling, not be reverted by txn 1's abandon"
    );
    let entry = ledger.get(&a.id).unwrap().unwrap();
    assert_eq!(entry.state, TxnState::Committed);
    assert_eq!(entry.last_applied_fingerprint, styled2_fp);

    // Idempotent: a second pass changes nothing.
    recover(&records, &plat, &plat, &mut ledger).unwrap();
    assert_eq!(world.borrow().get(&a.path).unwrap(), styled2);
}

#[test]
fn recyclebin_apply_materializes_and_verifies_the_paired_empty_asset() {
    // P1-14: the Recycle Bin's registry references BOTH a full and an empty ICO, the empty one by
    // convention (`<full>-empty.ico`). The driver used to write only the primary asset, so the
    // empty ICO could be a path the registry pointed at but that was never materialized — a
    // dangling reference that breaks the empty-bin icon. The driver must materialize AND verify
    // the paired empty asset before the mutation.
    let world = World::shared();
    let bin =
        ItemTarget::new(ItemId::from_raw("RB"), ItemKind::RecycleBin, "HKCU/RecycleBin/DefaultIcon");
    world.borrow_mut().put(&bin.path, b"orig-registry-state");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let current = Fingerprint::of_bytes(&world.borrow().get(&bin.path).unwrap());
    let req = ApplyRequest {
        target: bin.clone(),
        expected_fingerprint: current,
        owned: OwnedFields::icon_only(),
        asset_hash: "hashRB".into(),
        asset_bytes: b"full-ico".to_vec(),
        empty_asset_bytes: Some(b"empty-ico".to_vec()),
        pinned_seed: None,
    };
    let out = driver.apply(1, vec![req], &mut journal, &mut ledger).unwrap();

    assert_eq!(out.committed, vec![ItemId::from_raw("RB")]);
    // The paired empty ICO the registry will reference must actually exist in the store.
    let empty_path = paired_empty_path("assets/hashRB.ico");
    assert!(
        plat.asset_exists(&empty_path),
        "driver must materialize the paired empty asset before committing the Recycle Bin"
    );
}

#[test]
fn recyclebin_apply_fails_when_the_paired_empty_asset_does_not_materialize() {
    // P3-2: the driver's existence check must be load-bearing. With a store that reports
    // put_empty_variant success but leaves the asset absent, the driver must refuse to commit —
    // removing the `exists` guard would let it commit a dangling registry reference.
    let world = World::shared();
    let bin =
        ItemTarget::new(ItemId::from_raw("RB"), ItemKind::RecycleBin, "HKCU/RecycleBin/DefaultIcon");
    world.borrow_mut().put(&bin.path, b"orig-registry-state");
    let plat = FakePlatform::new(world.clone());
    plat.make_empty_variant_vanish(); // materialize "succeeds" but the asset never exists
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let current = Fingerprint::of_bytes(&world.borrow().get(&bin.path).unwrap());
    let req = ApplyRequest {
        target: bin.clone(),
        expected_fingerprint: current,
        owned: OwnedFields::icon_only(),
        asset_hash: "hashRB".into(),
        asset_bytes: b"full-ico".to_vec(),
        empty_asset_bytes: Some(b"empty-ico".to_vec()),
        pinned_seed: None,
    };
    let out = driver.apply(1, vec![req], &mut journal, &mut ledger).unwrap();

    assert!(out.committed.is_empty(), "must not commit when the paired empty asset is absent");
    assert!(out.error.is_some());
    assert_eq!(world.borrow().get(&bin.path).unwrap(), b"orig-registry-state"); // rolled back
    assert!(ledger.all().unwrap().is_empty());
}

#[test]
fn recyclebin_apply_fails_when_the_paired_empty_asset_vanishes_during_apply() {
    // P2-1 window narrowing: the paired empty ICO passes the pre-mutation existence check but is
    // deleted DURING the apply (GC / external process). Because the Recycle Bin fingerprint covers
    // only the registry path text, a vanished ICO is invisible to verify — the driver's post-apply
    // existence RE-check is what must refuse the commit, or a dangling registry reference lands.
    let world = World::shared();
    let bin =
        ItemTarget::new(ItemId::from_raw("RB"), ItemKind::RecycleBin, "HKCU/RecycleBin/DefaultIcon");
    world.borrow_mut().put(&bin.path, b"orig-registry-state");
    let plat = FakePlatform::new(world.clone());
    plat.make_empty_vanish_after_apply(); // the empty passes pre-check, then disappears mid-apply
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let current = Fingerprint::of_bytes(&world.borrow().get(&bin.path).unwrap());
    let req = ApplyRequest {
        target: bin.clone(),
        expected_fingerprint: current,
        owned: OwnedFields::icon_only(),
        asset_hash: "hashRB".into(),
        asset_bytes: b"full-ico".to_vec(),
        empty_asset_bytes: Some(b"empty-ico".to_vec()),
        pinned_seed: None,
    };
    let out = driver.apply(1, vec![req], &mut journal, &mut ledger).unwrap();

    assert!(out.committed.is_empty(), "a paired empty that vanished during apply must not commit");
    assert!(out.error.is_some());
    assert_eq!(world.borrow().get(&bin.path).unwrap(), b"orig-registry-state"); // rolled back
    assert!(ledger.all().unwrap().is_empty());
}

#[test]
fn the_paired_empty_asset_is_persisted_in_the_ledger_and_survives_recovery() {
    // New-P1: the empty ICO the Recycle Bin references must be recorded in the ledger AND the
    // journal, so a future asset GC keeps the EXACT empty asset instead of orphaning it right after
    // commit. It lands on commit and is rebuilt by recovery from the AssetWritten record.
    let world = World::shared();
    let bin =
        ItemTarget::new(ItemId::from_raw("RB"), ItemKind::RecycleBin, "HKCU/RecycleBin/DefaultIcon");
    world.borrow_mut().put(&bin.path, b"orig-registry-state");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let current = Fingerprint::of_bytes(&world.borrow().get(&bin.path).unwrap());
    let req = ApplyRequest {
        target: bin.clone(),
        expected_fingerprint: current,
        owned: OwnedFields::icon_only(),
        asset_hash: "hashRB".into(),
        asset_bytes: b"full-ico".to_vec(),
        empty_asset_bytes: Some(b"empty-ico".to_vec()),
        pinned_seed: None,
    };
    driver.apply(1, vec![req], &mut journal, &mut ledger).unwrap();

    let expected_empty = paired_empty_path("assets/hashRB.ico");
    let committed = ledger.get(&bin.id).unwrap().unwrap();
    let empty_ref =
        committed.empty_asset.expect("committed Recycle Bin entry must record its empty asset");
    assert_eq!(empty_ref.path, expected_empty);

    // Recovery from the journal rebuilds the same empty ref (crash in the commit→upsert gap).
    let mut fresh = MemLedgerStore::new();
    recover(journal.records(), &plat, &plat, &mut fresh).unwrap();
    let rebuilt = fresh.get(&bin.id).unwrap().unwrap();
    assert_eq!(
        rebuilt.empty_asset.expect("recovery must rebuild the empty asset").path,
        expected_empty
    );
}

#[test]
fn same_style_reapply_recovery_backfills_a_missing_empty_ref() {
    // New-P1 (wave-2R): a legacy committed row loaded with empty_asset:None must NOT block recovery
    // from persisting the exact empty ref the journal carries. reconcile_committed used to skip any
    // committed row whose fingerprint matched, so the exact reference was lost then checkpointed
    // away — orphaning the empty ICO.
    let bin =
        ItemTarget::new(ItemId::from_raw("RB"), ItemKind::RecycleBin, "HKCU/RecycleBin/DefaultIcon");
    let orig_fp = Fingerprint::of_bytes(b"orig-RB");
    let styled_fp = Fingerprint::of_bytes(b"styled-RB");
    let anchor = dm_domain::RestoreAnchor::FileBytes { bytes: b"orig-RB".to_vec() };
    let asset = dm_domain::AssetRef::new("hashRB", "assets/hashRB.ico");
    let exact_empty = dm_domain::AssetRef::new("hashRB-empty", "assets/hashRB-empty.ico");

    // A committed journal for the SAME styled state, carrying the exact empty ref.
    let records = vec![
        JournalRecord::TxnBegin { txn: 5, items: vec![bin.id.clone()] },
        JournalRecord::ItemPrepared {
            txn: 5,
            item: bin.id.clone(),
            target: bin.clone(),
            anchor: anchor.clone(),
            original_fingerprint: orig_fp,
            expected_fingerprint: orig_fp,
            asset_hash: "hashRB".into(),
            owned: OwnedFields::icon_only(),
            pinned_seed: None,
        },
        JournalRecord::AssetWritten {
            txn: 5,
            item: bin.id.clone(),
            asset: asset.clone(),
            empty: Some(exact_empty.clone()),
        },
        JournalRecord::ItemApplied { txn: 5, item: bin.id.clone(), new_fingerprint: styled_fp },
        JournalRecord::ItemVerified { txn: 5, item: bin.id.clone() },
        JournalRecord::TxnCommitted { txn: 5 },
    ];

    // A legacy ledger already holding the committed row, but WITHOUT the empty ref.
    let world = World::shared();
    let plat = FakePlatform::new(world.clone());
    let mut ledger = MemLedgerStore::new();
    ledger
        .upsert(LedgerEntry {
            item: bin.id.clone(),
            target: bin.clone(),
            original_fingerprint: orig_fp,
            original_anchor: anchor.clone(),
            last_applied_fingerprint: styled_fp,
            owned: OwnedFields::icon_only(),
            asset: asset.clone(),
            empty_asset: None, // legacy row: written before empty_asset existed
            state: TxnState::Committed,
            pinned_seed: None,
            version: 1,
        })
        .unwrap();

    recover(&records, &plat, &plat, &mut ledger).unwrap();

    let entry = ledger.get(&bin.id).unwrap().unwrap();
    assert_eq!(
        entry.empty_asset.expect("recovery must backfill the exact empty ref onto a legacy row").path,
        exact_empty.path
    );
}

#[test]
fn the_fake_applier_does_not_ignore_the_paired_empty_asset() {
    // New-P3: the fake applier used to ignore assets.empty, styling identically whether or not a
    // paired empty was supplied — an unfaithful model of the P2-1 "reference the EXACT empty asset"
    // contract. Now a paired apply's styled surface folds in the empty ref, so the committed
    // fingerprint is NOT the one a primary-only styling would produce.
    let world = World::shared();
    let bin =
        ItemTarget::new(ItemId::from_raw("RB"), ItemKind::RecycleBin, "HKCU/RecycleBin/DefaultIcon");
    world.borrow_mut().put(&bin.path, b"orig-registry-state");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let current = Fingerprint::of_bytes(&world.borrow().get(&bin.path).unwrap());
    let req = ApplyRequest {
        target: bin.clone(),
        expected_fingerprint: current,
        owned: OwnedFields::icon_only(),
        asset_hash: "hashRB".into(),
        asset_bytes: b"full-ico".to_vec(),
        empty_asset_bytes: Some(b"empty-ico".to_vec()),
        pinned_seed: None,
    };
    let out = driver.apply(1, vec![req], &mut journal, &mut ledger).unwrap();
    assert_eq!(out.committed, vec![ItemId::from_raw("RB")]);

    let entry = ledger.get(&bin.id).unwrap().unwrap();
    // If the fake ignored the empty, the styled bytes would be exactly styled_bytes("hashRB").
    assert_ne!(
        entry.last_applied_fingerprint,
        Fingerprint::of_bytes(&styled_bytes("hashRB")),
        "the paired empty must influence the applied surface, not be ignored"
    );
}

#[test]
fn recyclebin_request_without_empty_bytes_is_rejected_not_committed() {
    // P1-2: a RecycleBin request with no empty_asset_bytes used to skip pairing entirely while the
    // Windows applier still pointed the registry at a guessed empty path — committing a dangling
    // empty icon. The driver must reject such a request, not commit it.
    let world = World::shared();
    let bin =
        ItemTarget::new(ItemId::from_raw("RB"), ItemKind::RecycleBin, "HKCU/RecycleBin/DefaultIcon");
    world.borrow_mut().put(&bin.path, b"orig-registry-state");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let current = Fingerprint::of_bytes(&world.borrow().get(&bin.path).unwrap());
    let req = ApplyRequest {
        target: bin.clone(),
        expected_fingerprint: current,
        owned: OwnedFields::icon_only(),
        asset_hash: "hashRB".into(),
        asset_bytes: b"full-ico".to_vec(),
        empty_asset_bytes: None, // ← the defect: no paired empty supplied
        pinned_seed: None,
    };
    let out = driver.apply(1, vec![req], &mut journal, &mut ledger).unwrap();

    assert!(out.committed.is_empty(), "a Recycle Bin apply with no empty icon must not commit");
    assert!(out.error.is_some());
    assert_eq!(world.borrow().get(&bin.path).unwrap(), b"orig-registry-state"); // rolled back
    assert!(ledger.all().unwrap().is_empty());
}

#[test]
fn journal_failure_after_mutation_restores_to_original_without_journaling() {
    // Fail the ItemApplied append (call 4: TxnBegin, ItemPrepared, AssetWritten, ItemApplied).
    // The desktop mutation already happened, but the JOURNAL is the thing that failed and may be
    // torn — so the driver restores the item from its anchor WITHOUT appending rollback records
    // (P1-5). Recovery re-confirms from the durable prefix.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = FailingJournal::fail_on_call(4);
    let mut ledger = MemLedgerStore::new();

    let out = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger).unwrap();
    assert!(out.error.is_some());
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A"); // restored exactly
    assert!(ledger.all().unwrap().is_empty());
    // No rollback records were appended to the possibly-torn journal.
    assert!(!journal.records().iter().any(|r| matches!(r,
        JournalRecord::ItemRolledBack { .. } | JournalRecord::TxnRolledBack { .. })));
}

#[test]
fn applier_failure_with_healthy_journal_still_journals_a_clean_rollback() {
    // The complementary case: the APPLIER fails (not the journal), so the journal is healthy and a
    // clean TxnRolledBack terminal SHOULD be recorded (recovery then skips the txn).
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    world.borrow_mut().fail_apply(&b.path); // B's mutation fails → journaled rollback of A
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();

    let out = driver
        .apply(1, vec![request(&a, &world, "hashA"), request(&b, &world, "hashB")], &mut journal, &mut ledger)
        .unwrap();
    assert!(out.error.is_some());
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A");
    assert!(journal.records().iter().any(|r| matches!(r, JournalRecord::TxnRolledBack { .. })));
}

#[test]
fn prepare_append_failure_mid_batch_restores_mutated_items_without_journaling() {
    // P1-5: item A applies fully, then the ItemPrepared append for item B fails. The journal may
    // be torn, so the driver must NOT append rollback records to it (that could turn a torn tail
    // into fatal mid-file corruption). It restores A from its anchor WITHOUT journaling; recovery
    // re-confirms from the durable prefix.
    // Call order for two items: 1 TxnBegin, 2 ItemPrepared(A), 3 AssetWritten(A), 4 ItemApplied(A),
    // 5 ItemVerified(A), 6 ItemPrepared(B) ← fail here.
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = FailingJournal::fail_on_call(6);
    let mut ledger = MemLedgerStore::new();

    let out = driver
        .apply(1, vec![request(&a, &world, "hashA"), request(&b, &world, "hashB")], &mut journal, &mut ledger)
        .unwrap();

    assert!(out.error.is_some());
    assert!(out.committed.is_empty());
    // A was mutated then restored; B never mutated. The ledger holds nothing.
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A");
    assert_eq!(world.borrow().get(&b.path).unwrap(), b"orig-B");
    assert!(ledger.all().unwrap().is_empty());
    // No rollback records were appended to the (possibly torn) journal.
    assert!(!journal.records().iter().any(|r| matches!(r,
        JournalRecord::ItemRolledBack { .. } | JournalRecord::TxnRolledBack { .. })));
}

#[test]
fn a_journal_tear_during_rollback_still_restores_every_mutated_item() {
    // P1-5: the applier fails (the healthy-journal rollback path), but the journal then tears DURING
    // the rollback. A naive rollback that `?`-aborts on the first failed `ItemRolledBack` append
    // would leave every earlier-applied item stranded in its mutated state. The rollback must keep
    // restoring from anchors, stop journaling, and let recovery finish from the durable prefix.
    // Call order (3 items, C's apply fails): 1 TxnBegin, 2-5 A, 6-9 B, 10 ItemPrepared(C),
    // 11 AssetWritten(C), [C apply fails]. Rollback then appends 12 ItemRolledBack(C) — fail_from(12)
    // tears there and onward, so B's and A's rollback appends would also fail.
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    let c = target("C");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    seed(&world, &c, b"orig-C");
    world.borrow_mut().fail_apply(&c.path); // C's mutation fails → healthy-journal rollback path
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = FailingJournal::fail_from(12); // journal tears at the first rollback append
    let mut ledger = MemLedgerStore::new();

    // Do NOT unwrap: the pre-fix code `?`-propagates an Err here; the fixed code returns Ok. Either
    // way, the load-bearing assertion is that no mutated item is left stranded.
    let _ = driver.apply(
        1,
        vec![request(&a, &world, "hashA"), request(&b, &world, "hashB"), request(&c, &world, "hashC")],
        &mut journal,
        &mut ledger,
    );

    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A", "A stranded mutated after a torn rollback");
    assert_eq!(world.borrow().get(&b.path).unwrap(), b"orig-B", "B stranded mutated after a torn rollback");
    assert_eq!(world.borrow().get(&c.path).unwrap(), b"orig-C");
    assert!(ledger.all().unwrap().is_empty());
}

#[test]
fn persistent_journal_failure_restores_every_mutated_item() {
    // P1-5b: a PERSISTENT journal outage (every append from the failure point on fails) during a
    // two-item batch must still restore every already-mutated item — a naive rollback that `?`-
    // aborts on the first failed append would strand later items.
    // Two-item order: 1 TxnBegin, 2 ItemPrepared(A) ... 6 ItemPrepared(B) ← fails, and so would
    // any subsequent append. A is mutated; the restore path must not touch the journal.
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = FailingJournal::fail_from(6);
    let mut ledger = MemLedgerStore::new();

    let out = driver
        .apply(1, vec![request(&a, &world, "hashA"), request(&b, &world, "hashB")], &mut journal, &mut ledger)
        .unwrap();

    assert!(out.error.is_some());
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A"); // restored despite the outage
    assert_eq!(world.borrow().get(&b.path).unwrap(), b"orig-B");
    assert!(ledger.all().unwrap().is_empty());
}

#[test]
fn commit_append_failure_when_not_durable_recovers_by_rolling_back() {
    // P1-4: the TxnCommitted append fails and the record never reached disk. apply propagates the
    // error WITHOUT rolling back in-process (the record might have been durable — the driver can't
    // know). Because here it was NOT durable, recovery sees an incomplete txn and restores.
    // Single-item order: ... 6 TxnCommitted ← fails, writing nothing.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = FailingJournal::fail_on_call(6);
    let mut ledger = MemLedgerStore::new();

    let result = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger);
    assert!(matches!(result, Err(OperationError::Journal(_))));
    // Not rolled back in-process — the mutation stands, awaiting recovery.
    assert_eq!(world.borrow().get(&a.path).unwrap(), styled_bytes("hashA"));
    assert!(!journal.records().iter().any(|r| matches!(r, JournalRecord::TxnCommitted { .. })));

    // Recovery: no commit record → incomplete txn → restore to the true original.
    let mut fresh = MemLedgerStore::new();
    recover(journal.records(), &plat, &plat, &mut fresh).unwrap();
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A");
    assert!(fresh.all().unwrap().is_empty());
}

#[test]
fn commit_append_failure_when_durable_recovers_by_rolling_forward() {
    // P1-4 (the dangerous case): the TxnCommitted record reached disk but the fsync reported an
    // error. The driver must NOT roll back — the commit is durable, so recovery rolls FORWARD.
    // Rolling back here (the old behaviour) would restore the desktop while the journal says
    // committed, producing an unrecoverable split state.
    // Single-item order: ... 6 TxnCommitted ← written to the log, then reports failure.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = FailingJournal::fail_after_write(6);
    let mut ledger = MemLedgerStore::new();

    let result = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger);
    assert!(result.is_err());
    // Desktop stays styled and the commit record IS on the log.
    assert_eq!(world.borrow().get(&a.path).unwrap(), styled_bytes("hashA"));
    assert!(journal.records().iter().any(|r| matches!(r, JournalRecord::TxnCommitted { .. })));

    // Recovery reconciles the committed txn: the ledger is rebuilt and the desktop stays styled.
    let mut fresh = MemLedgerStore::new();
    let rec = recover(journal.records(), &plat, &plat, &mut fresh).unwrap();
    assert_eq!(rec.reconciled, vec![ItemId::from_raw("A")]);
    assert_eq!(world.borrow().get(&a.path).unwrap(), styled_bytes("hashA"));
    assert!(fresh.get(&a.id).unwrap().is_some());
}

#[test]
fn journal_failure_before_mutation_aborts_without_touching_the_desktop() {
    // Fail the ItemPrepared append (call 2). Nothing has mutated; apply propagates the error and
    // the desktop + ledger stay pristine.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = FailingJournal::fail_on_call(2);
    let mut ledger = MemLedgerStore::new();

    let result = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut ledger);
    assert!(matches!(result, Err(OperationError::Journal(_))));
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A");
    assert!(ledger.all().unwrap().is_empty());
}

#[test]
fn ledger_upsert_failure_at_commit_surfaces_but_recovery_reconciles() {
    // The mutation + TxnCommitted are durable; only the ledger write dies. apply returns Err, and
    // recovery rebuilds the entry from the journal (the commit→upsert gap is closed).
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut failing = FailingLedger;

    let result = driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut failing);
    assert!(result.is_err());
    // Desktop is styled (committed) and the journal recorded the commit.
    assert_eq!(world.borrow().get(&a.path).unwrap(), styled_bytes("hashA"));
    assert!(journal.records().iter().any(|r| matches!(r, JournalRecord::TxnCommitted { .. })));

    let mut fresh = MemLedgerStore::new();
    let rec = recover(journal.records(), &plat, &plat, &mut fresh).unwrap();
    assert_eq!(rec.reconciled, vec![ItemId::from_raw("A")]);
}

#[test]
fn empty_batch_and_all_conflicts_write_no_journal() {
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut ledger = MemLedgerStore::new();

    // Empty request list → empty outcome, no transaction started.
    let mut j1 = VecJournal::new();
    let out = driver.apply(1, vec![], &mut j1, &mut ledger).unwrap();
    assert_eq!(out, super::driver::ApplyOutcome::default());
    assert!(j1.records().is_empty());

    // A batch where the only item conflicts → still no TxnBegin.
    let req = request(&a, &world, "hashA");
    world.borrow_mut().put(&a.path, b"changed");
    let mut j2 = VecJournal::new();
    let out2 = driver.apply(2, vec![req], &mut j2, &mut ledger).unwrap();
    assert_eq!(out2.conflicts, vec![ItemId::from_raw("A")]);
    assert!(j2.records().is_empty());
}

#[test]
fn recovery_of_a_rolled_back_txn_is_a_clean_noop() {
    // Produce a fully-rolled-back transaction, then recover from its journal: nothing to do.
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    world.borrow_mut().fail_apply(&b.path);
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();
    driver
        .apply(1, vec![request(&a, &world, "hashA"), request(&b, &world, "hashB")], &mut journal, &mut ledger)
        .unwrap();
    assert!(journal.records().iter().any(|r| matches!(r, JournalRecord::TxnRolledBack { .. })));

    let mut fresh = MemLedgerStore::new();
    let rec = recover(journal.records(), &plat, &plat, &mut fresh).unwrap();
    assert_eq!(rec.clean_txns, 1);
    assert!(rec.aborted.is_empty() && rec.reconciled.is_empty());
}

#[test]
fn recover_from_journal_reads_the_log_then_recovers() {
    // The startup entry point: instead of handing recover() a record slice, it reads the journal
    // itself and recovers. Same rolled-back scenario as above, driven through that seam.
    let world = World::shared();
    let a = target("A");
    let b = target("B");
    seed(&world, &a, b"orig-A");
    seed(&world, &b, b"orig-B");
    world.borrow_mut().fail_apply(&b.path);
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();
    driver
        .apply(1, vec![request(&a, &world, "hashA"), request(&b, &world, "hashB")], &mut journal, &mut ledger)
        .unwrap();

    let mut fresh = MemLedgerStore::new();
    let rec = recover_from_journal(&mut journal, &plat, &plat, &mut fresh).unwrap();
    assert_eq!(rec.clean_txns, 1);
}

#[test]
fn recover_from_an_empty_journal_is_a_clean_noop() {
    // A fresh install (no journal records) recovers to nothing — the startup path must not error.
    let world = World::shared();
    let plat = FakePlatform::new(world);
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();
    let rec = recover_from_journal(&mut journal, &plat, &plat, &mut ledger).unwrap();
    assert_eq!(rec.clean_txns, 0);
    assert!(rec.aborted.is_empty() && rec.reconciled.is_empty());
}

#[test]
fn recover_from_journal_truncates_the_journal_after_reconciling() {
    // P2-5: after a pass, every txn is reconciled into the ledger, so the journal is truncated —
    // a second recovery replays nothing and the history no longer grows unbounded.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    let mut journal = VecJournal::new();
    let mut applied = MemLedgerStore::new();
    driver.apply(1, vec![request(&a, &world, "hashA")], &mut journal, &mut applied).unwrap();
    assert!(!journal.records().is_empty(), "driver leaves the journal intact for recovery");

    // Recover into a fresh ledger (simulating a lost ledger write), then the journal is truncated.
    let mut fresh = MemLedgerStore::new();
    recover_from_journal(&mut journal, &plat, &plat, &mut fresh).unwrap();
    assert!(journal.records().is_empty(), "checkpoint empties the journal after reconciling");

    // A second pass over the emptied journal is a clean no-op.
    let out2 = recover_from_journal(&mut journal, &plat, &plat, &mut fresh).unwrap();
    assert_eq!(out2, RecoveryOutcome::default());
}

#[test]
fn recovery_surfaces_a_restore_failure_as_degraded_then_retries_clean() {
    // codex R4-Block 5: an incomplete transaction whose restore faults must NOT bail with a bare Err
    // (the caller would relay it as a bridge error over a partially-recovered desktop). It surfaces the
    // fault via `degraded`, leaves the row + journal intact, and a later retry (fault cleared) finishes.
    let world = World::shared();
    let a = target("A");
    seed(&world, &a, b"orig-A");
    let plat = FakePlatform::new(world.clone());
    let driver = TxnDriver::new(&plat, &plat, &plat);
    // Truncate a successful apply's journal to just before commit → an incomplete transaction.
    let mut recording = RecordingJournal::new(world.clone());
    let mut ledger = MemLedgerStore::new();
    driver.apply(1, vec![request(&a, &world, "hashA")], &mut recording, &mut ledger).unwrap();
    let records: Vec<JournalRecord> =
        recording.records().iter().filter(|r| !matches!(r, JournalRecord::TxnCommitted { .. } | JournalRecord::ItemVerified { .. })).cloned().collect();

    // The restore faults on this pass → recovery is degraded, not a bare Err; the item is NOT reported
    // aborted (its revert never landed).
    world.borrow_mut().fail_restore(&a.path);
    let mut fresh = MemLedgerStore::new();
    let out = recover(&records, &plat, &plat, &mut fresh).unwrap();
    assert!(!out.degraded.is_empty(), "the restore fault is surfaced as degraded");
    assert!(out.aborted.is_empty(), "the item was NOT reported reverted");

    // The transient fault clears; a retry finishes the abort exactly (restore is idempotent).
    world.borrow_mut().clear_faults();
    let out2 = recover(&records, &plat, &plat, &mut fresh).unwrap();
    assert!(out2.degraded.is_empty(), "the retry reconciles cleanly");
    assert_eq!(world.borrow().get(&a.path).unwrap(), b"orig-A");
}
