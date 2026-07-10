//! Behavioural tests for the durable transaction machinery, including the kill-point recovery
//! battery. Every named test corresponds to an invariant harvested from the frozen C# oracle
//! (cited inline) or a spec/ADR requirement.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use dm_domain::{Fingerprint, ItemId, ItemKind, ItemTarget, OwnedFields};

use super::driver::{ApplyRequest, TxnDriver};
use super::fakes::{styled_bytes, FakePlatform, RecordingJournal, World};
use super::journal::{JournalRecord, VecJournal};
use super::recovery::recover;
use crate::ledger::store::{LedgerStore, MemLedgerStore};
use crate::ledger::TxnState;

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

/// The kill-point battery: run a two-item apply, then for every truncation of the journal
/// replay recovery against the world as it was at that fsync — plus a "torn write" variant —
/// and assert each item lands EXACTLY on its original (incomplete txn) or its target
/// (committed txn), with a consistent ledger and full idempotency.
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

        // Variant 1: clean crash (desktop exactly at the snapshot).
        assert_recovers_consistently(records, &world_at_crash, &originals, &expected_target, committed, k);

        // Variant 2: torn write — every prepared item's file is garbage. The anchor must still
        // pull it back to the exact original (or leave a committed item styled, since a committed
        // txn is never undone and its file is authoritative).
        let mut torn = world_at_crash.clone();
        for id in prepared_ids(records) {
            let path = format!("C:/Desktop/{id}.lnk");
            if committed {
                // Post-commit files are authoritative; don't corrupt them in this variant.
                continue;
            }
            torn.insert(path, b"TORN-GARBAGE".to_vec());
        }
        if !committed {
            assert_recovers_consistently(records, &torn, &originals, &expected_target, committed, k);
        }
    }
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
