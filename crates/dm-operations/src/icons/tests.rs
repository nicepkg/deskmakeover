//! Orchestration tests for the icon apply-session, persistence, reset, and GC. The pixel
//! packaging is covered in `package.rs`; here the platform ports are the shared virtual-desktop
//! fake (`txn::fakes`) and the asset store is the REAL `FsAssetStore`, so GC actually deletes
//! files and the apply drives the real transaction engine.

use dm_contracts::IconStyle;
use dm_domain::{DesktopItem, ItemId, ItemKind, ItemState};
use serde_json::json;

use super::*;
use crate::ledger::store::{JsonLedgerStore, LedgerStore, MemLedgerStore};
use crate::settings_store::SettingsStore;
use crate::txn::fakes::{FakePlatform, World};
use crate::txn::{FsAssetStore, TxnIdAllocator, VecJournal};

/// A base64 straight-alpha RGBA PNG master of a solid colour, at the REQUIRED 256×256 master size
/// (a real, decodable, contract-valid bake master).
fn master_b64(rgba: [u8; 4]) -> String {
    use base64::Engine;
    use image::ImageEncoder;
    let img = image::RgbaImage::from_pixel(256, 256, image::Rgba(rgba));
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&img, 256, 256, image::ExtendedColorType::Rgba8)
        .unwrap();
    base64::engine::general_purpose::STANDARD.encode(png)
}

/// A ready-to-style desktop item at a deterministic path.
fn item(id: &str, kind: ItemKind) -> DesktopItem {
    DesktopItem {
        id: ItemId::from_raw(id),
        name: id.into(),
        path: format!("C:/Desktop/{id}"),
        kind,
        icon: None,
        state: ItemState::Ready,
        requires_explicit_consent: false,
        status_message: None,
    }
}

/// A distinct `{config, kindPolicy, typeOverrides}` recipe per seed.
fn style(seed: i64) -> IconStyle {
    IconStyle::from_value(json!({ "config": { "seed": seed }, "kindPolicy": {}, "typeOverrides": {} }))
        .unwrap()
}

/// The whole fixture: a shared virtual desktop, a real on-disk asset store, and the ②③ stores.
struct Fixture {
    _dir: tempfile::TempDir,
    world: std::rc::Rc<std::cell::RefCell<World>>,
    assets: FsAssetStore,
    settings: SettingsStore,
    history: LookHistoryStore,
    ledger: MemLedgerStore,
    journal: VecJournal,
    txn: TxnIdAllocator,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        Self {
            world: World::shared(),
            assets: FsAssetStore::new(dir.path().join("assets")),
            settings: SettingsStore::open(&dir.path().join("settings.sqlite3")).unwrap(),
            history: LookHistoryStore::new(dir.path().join("look-history.json")),
            ledger: MemLedgerStore::new(),
            journal: VecJournal::new(),
            txn: TxnIdAllocator::starting_at(1),
            _dir: dir,
        }
    }

    /// Seeds an item's original bytes on the virtual desktop so a fresh apply has something to
    /// fingerprint + capture as the restore anchor.
    fn seed(&self, it: &DesktopItem, original: &[u8]) {
        self.world.borrow_mut().put(&it.path, original);
    }

    /// Runs a commit with the given masters, driving the real engine over the fake platform.
    /// Converts scanned items into `ScannedItem`s, capturing each one's CURRENT desktop fingerprint
    /// as its scan-time CAS anchor (what the real host does per scan).
    fn scanned(&self, items: &[DesktopItem]) -> Vec<ScannedItem> {
        items
            .iter()
            .map(|it| {
                let fp = self
                    .world
                    .borrow()
                    .get(&it.path)
                    .map(|b| dm_domain::Fingerprint::of_bytes(&b))
                    .unwrap_or(dm_domain::Fingerprint::of_bytes(b""));
                ScannedItem { item: it.clone(), fingerprint: fp }
            })
            .collect()
    }

    fn apply(
        &mut self,
        masters: &[(&str, u32, [u8; 4])],
        style: IconStyle,
        label: &str,
        look_id: &str,
        scan: &[DesktopItem],
    ) -> IconApplyOutcome {
        self.apply_reverting(masters, style, label, look_id, scan, &[])
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_reverting(
        &mut self,
        masters: &[(&str, u32, [u8; 4])],
        style: IconStyle,
        label: &str,
        look_id: &str,
        scan: &[DesktopItem],
        restore_ids: &[&str],
    ) -> IconApplyOutcome {
        let mut session = IconApplySession::begin(0, masters.len());
        for (id, slot, rgba) in masters {
            session.push(*id, *slot, master_b64(*rgba));
        }
        let scanned = self.scanned(scan);
        let restore: Vec<String> = restore_ids.iter().map(|s| s.to_string()).collect();
        let fake = FakePlatform::new(self.world.clone());
        let platform = IconPlatform::new(&fake, &fake, &self.assets);
        let ops = IconOps::new(platform, &self.settings);
        ops.commit_apply(
            session,
            style,
            Some(label.into()),
            look_id,
            look_id.len() as i64, // a deterministic caller-stamped timestamp
            &scanned,
            &restore,
            &mut self.txn,
            &mut self.journal,
            &mut self.ledger,
            &mut self.history,
        )
        .unwrap()
    }

    fn reset(&mut self, scan_seed_history: bool) -> IconResetOutcome {
        let _ = scan_seed_history;
        let fake = FakePlatform::new(self.world.clone());
        let platform = IconPlatform::new(&fake, &fake, &self.assets);
        let ops = IconOps::new(platform, &self.settings);
        ops.reset_to_original(&mut self.journal, &mut self.ledger, &self.history).unwrap()
    }

    fn ico_files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.assets.root())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| n.ends_with(".ico"))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
}

#[test]
fn commit_styles_items_persists_stores_and_writes_real_icos() {
    let mut f = Fixture::new();
    let app = item("app", ItemKind::Shortcut);
    f.seed(&app, b"orig-app");

    let out = f.apply(&[("app", 0, [10, 20, 30, 255])], style(1), "第一版", "v1", &[app.clone()]);

    assert_eq!(out.committed, vec![ItemId::from_raw("app")]);
    assert!(out.conflicts.is_empty() && out.error.is_none());
    // ① the ledger tracks the styled item.
    assert!(f.ledger.get(&app.id).unwrap().is_some());
    // ② the saved-style is exactly what we applied.
    assert_eq!(f.settings.get_saved_style().unwrap().as_ref(), Some(&style(1)));
    // ③ one look pushed, carrying its name.
    assert_eq!(out.stores.history.len(), 1);
    assert_eq!(out.stores.history[0].label.as_deref(), Some("第一版"));
    assert!(out.stores.applied, "a committed apply reads back as applied");
    // A real laddered ICO landed on disk.
    assert_eq!(f.ico_files().len(), 1, "one content-addressed .ico written");
}

#[test]
fn recycle_bin_pairs_the_empty_variant() {
    let mut f = Fixture::new();
    let bin = item("bin", ItemKind::RecycleBin);
    f.seed(&bin, b"orig-bin");

    let out = f.apply(
        &[("bin", 0, [255, 0, 0, 255]), ("bin", 1, [0, 255, 0, 255])],
        style(2),
        "回收站",
        "v1",
        &[bin.clone()],
    );

    assert_eq!(out.committed, vec![ItemId::from_raw("bin")]);
    let entry = f.ledger.get(&bin.id).unwrap().unwrap();
    assert!(entry.empty_asset.is_some(), "the paired empty asset is recorded in the ledger");
    // Two ICOs on disk: the primary + its paired empty.
    assert_eq!(f.ico_files().len(), 2);
}

#[test]
fn a_master_for_an_item_not_in_the_scan_is_a_conflict_never_applied() {
    let mut f = Fixture::new();
    let app = item("app", ItemKind::Shortcut);
    f.seed(&app, b"orig-app");
    // The scan only knows "app"; a stale "ghost" master must be skipped, not applied.
    let out = f.apply(
        &[("app", 0, [1, 2, 3, 255]), ("ghost", 0, [9, 9, 9, 255])],
        style(1),
        "x",
        "v1",
        &[app.clone()],
    );
    assert_eq!(out.committed, vec![ItemId::from_raw("app")]);
    assert_eq!(out.conflicts, vec![ItemId::from_raw("ghost")]);
}

#[test]
fn an_item_deleted_between_scan_and_commit_is_skipped() {
    let mut f = Fixture::new();
    let app = item("app", ItemKind::Shortcut);
    let gone = item("gone", ItemKind::Shortcut);
    f.seed(&app, b"orig-app");
    // `gone` is in the scan but was deleted from the desktop before we commit (no World entry).
    let out = f.apply(
        &[("app", 0, [1, 2, 3, 255]), ("gone", 0, [4, 5, 6, 255])],
        style(1),
        "x",
        "v1",
        &[app.clone(), gone.clone()],
    );
    assert_eq!(out.committed, vec![ItemId::from_raw("app")]);
    assert_eq!(out.conflicts, vec![ItemId::from_raw("gone")]);
}

#[test]
fn an_unstyleable_item_is_never_requested() {
    let mut f = Fixture::new();
    let broken = item("broken", ItemKind::Unsupported);
    f.seed(&broken, b"orig");
    let out = f.apply(&[("broken", 0, [1, 1, 1, 255])], style(1), "x", "v1", &[broken.clone()]);
    assert!(out.committed.is_empty());
    assert_eq!(out.conflicts, vec![ItemId::from_raw("broken")]);
}

#[test]
fn re_applying_a_different_style_supersedes_and_gcs_the_old_asset() {
    let mut f = Fixture::new();
    let app = item("app", ItemKind::Shortcut);
    f.seed(&app, b"orig-app");

    f.apply(&[("app", 0, [10, 20, 30, 255])], style(1), "A", "vA", &[app.clone()]);
    assert_eq!(f.ico_files().len(), 1);
    let first = f.ico_files()[0].clone();

    // A second global apply of a visually different master re-styles the same item; the superseded
    // ICO is now unreferenced and GC collects it.
    f.apply(&[("app", 0, [200, 100, 50, 255])], style(2), "B", "vB", &[app.clone()]);
    let after = f.ico_files();
    assert_eq!(after.len(), 1, "the superseded asset was collected, the new one kept");
    assert_ne!(after[0], first, "the surviving ICO is the newly applied one");
    // ② now reflects the second style; ③ has both.
    assert_eq!(f.settings.get_saved_style().unwrap().as_ref(), Some(&style(2)));
    assert_eq!(f.history.all().len(), 2);
}

#[test]
fn re_applying_the_same_style_dedups_the_history() {
    let mut f = Fixture::new();
    let app = item("app", ItemKind::Shortcut);
    f.seed(&app, b"orig-app");
    f.apply(&[("app", 0, [10, 20, 30, 255])], style(7), "A", "v1", &[app.clone()]);
    // Same recipe again → the store bumps the head instead of growing (spec 07 §17).
    f.apply(&[("app", 0, [10, 20, 30, 255])], style(7), "A", "v2", &[app.clone()]);
    assert_eq!(f.history.all().len(), 1, "an identical recipe dedups onto the head");
}

#[test]
fn read_state_reflects_the_stores() {
    let mut f = Fixture::new();
    // Cold: nothing applied, no saved style, empty history.
    {
        let fake = FakePlatform::new(f.world.clone());
        let ops = IconOps::new(IconPlatform::new(&fake, &fake, &f.assets), &f.settings);
        let cold = ops.read_state(&f.history, &f.ledger).unwrap();
        assert!(cold.saved_style.is_none() && cold.history.is_empty() && !cold.applied);
    }
    let app = item("app", ItemKind::Shortcut);
    f.seed(&app, b"orig-app");
    f.apply(&[("app", 0, [1, 2, 3, 255])], style(1), "A", "v1", &[app.clone()]);
    let fake = FakePlatform::new(f.world.clone());
    let ops = IconOps::new(IconPlatform::new(&fake, &fake, &f.assets), &f.settings);
    let warm = ops.read_state(&f.history, &f.ledger).unwrap();
    assert!(warm.applied && warm.saved_style.is_some() && warm.history.len() == 1);
}

#[test]
fn reset_reverts_originals_clears_saved_style_and_gcs_assets() {
    let mut f = Fixture::new();
    let app = item("app", ItemKind::Shortcut);
    f.seed(&app, b"orig-app");
    f.apply(&[("app", 0, [1, 2, 3, 255])], style(1), "A", "v1", &[app.clone()]);
    assert_eq!(f.ico_files().len(), 1);

    let out = f.reset(false);

    assert_eq!(out.restored, vec![ItemId::from_raw("app")]);
    assert!(out.skipped.is_empty());
    // The desktop is back to the true original.
    assert_eq!(f.world.borrow().get(&app.path).unwrap(), b"orig-app");
    // ① emptied, ② cleared, so nothing is applied and the resident stays dormant.
    assert!(f.ledger.all().unwrap().is_empty());
    assert!(f.settings.get_saved_style().unwrap().is_none());
    assert!(!out.stores.applied);
    // Every generated ICO is now unreferenced and collected.
    assert!(f.ico_files().is_empty(), "reset GCs the whole asset store");
    // ③ history is advisory and survives a reset (the user can re-pick a look).
    assert_eq!(f.history.all().len(), 1);
}

#[test]
fn reset_is_trust_first_and_leaves_a_hand_edited_icon_alone() {
    let mut f = Fixture::new();
    let mine = item("mine", ItemKind::Shortcut);
    let edited = item("edited", ItemKind::Shortcut);
    f.seed(&mine, b"orig-mine");
    f.seed(&edited, b"orig-edited");
    f.apply(
        &[("mine", 0, [1, 1, 1, 255]), ("edited", 0, [2, 2, 2, 255])],
        style(1),
        "A",
        "v1",
        &[mine.clone(), edited.clone()],
    );
    // The user hand-edits `edited` AFTER our apply (its live state no longer matches ours).
    f.world.borrow_mut().put(&edited.path, b"user-changed-this");

    let out = f.reset(false);

    assert_eq!(out.restored, vec![ItemId::from_raw("mine")]);
    assert_eq!(out.skipped, vec![ItemId::from_raw("edited")], "the hand-edited icon is left alone");
    // The user's change is preserved, never clobbered by a byte-literal revert.
    assert_eq!(f.world.borrow().get(&edited.path).unwrap(), b"user-changed-this");
    // The skipped item's row (and its asset) survive; `mine`'s are gone.
    assert!(f.ledger.get(&edited.id).unwrap().is_some());
    assert!(f.ledger.get(&mine.id).unwrap().is_none());
    assert_eq!(f.ico_files().len(), 1, "only the skipped item's asset is retained");
}

#[test]
fn commit_reconciles_a_committed_but_unledgered_txn_before_preparing() {
    // Simulate the #5 commit→ledger gap: a prior transaction is durable in the journal (committed)
    // but its ledger rows were lost (a crash in the commit→upsert window). A JsonLedgerStore that
    // starts empty against a journal holding a committed txn must be reconciled by the NEXT apply.
    let dir = tempfile::tempdir().unwrap();
    let world = World::shared();
    let app = item("app", ItemKind::Shortcut);
    world.borrow_mut().put(&app.path, b"orig-app");
    let assets = FsAssetStore::new(dir.path().join("assets"));
    let settings = SettingsStore::open(&dir.path().join("s.sqlite3")).unwrap();
    let mut history = LookHistoryStore::new(dir.path().join("h.json"));

    // First apply against a throwaway ledger, driving records into a persistent journal.
    let journal_path = dir.path().join("txn.log");
    let mut journal = crate::txn::FileJournal::new(&journal_path);
    let mut ledger_a = MemLedgerStore::new();
    let mut txn = TxnIdAllocator::starting_at(1);
    {
        let fake = FakePlatform::new(world.clone());
        let ops = IconOps::new(IconPlatform::new(&fake, &fake, &assets), &settings);
        let mut s = IconApplySession::begin(0, 1);
        s.push("app", 0, master_b64([1, 2, 3, 255]));
        let scan = vec![ScannedItem { item: app.clone(), fingerprint: dm_domain::Fingerprint::of_bytes(&world.borrow().get(&app.path).unwrap()) }];
        ops.commit_apply(s, style(1), Some("A".into()), "v1", 1, &scan, &[], &mut txn, &mut journal, &mut ledger_a, &mut history)
            .unwrap();
    }

    // A SECOND apply arrives with a FRESH (empty) ledger but the same durable journal — the gap.
    // commit_apply must reconcile the journal into the ledger before preparing, so the item is a
    // re-apply (its true original stays pinned), not mistaken for a first apply.
    let mut ledger_b = JsonLedgerStore::new(dir.path().join("ledger.json"));
    assert!(ledger_b.all().unwrap().is_empty(), "the fresh ledger starts empty");
    {
        let fake = FakePlatform::new(world.clone());
        let ops = IconOps::new(IconPlatform::new(&fake, &fake, &assets), &settings);
        let mut s = IconApplySession::begin(0, 1);
        s.push("app", 0, master_b64([9, 9, 9, 255]));
        let scan = vec![ScannedItem { item: app.clone(), fingerprint: dm_domain::Fingerprint::of_bytes(&world.borrow().get(&app.path).unwrap()) }];
        let out = ops
            .commit_apply(s, style(2), Some("B".into()), "v2", 2, &scan, &[], &mut txn, &mut journal, &mut ledger_b, &mut history)
            .unwrap();
        assert_eq!(out.committed, vec![ItemId::from_raw("app")]);
    }
    // The reconciled original (orig-app) is preserved as the restore anchor, not the styled state.
    let entry = ledger_b.get(&app.id).unwrap().unwrap();
    assert_eq!(entry.original_fingerprint, dm_domain::Fingerprint::of_bytes(b"orig-app"));
}

#[test]
fn reset_checkpoints_the_journal_so_a_restart_cannot_revive_the_ledger() {
    // The revived-ledger bug (codex 2026-07-12): Apply leaves durable `TxnCommitted` records; if
    // reset deletes the ledger rows WITHOUT emptying the journal, the next launch's startup recovery
    // re-upserts them — resurrecting a styled ledger entry that points at a GC'd ICO over an original
    // desktop. This is NOT a crash-window (no crash needed), so reset must reconcile + checkpoint.
    let dir = tempfile::tempdir().unwrap();
    let world = World::shared();
    let app = item("app", ItemKind::Shortcut);
    world.borrow_mut().put(&app.path, b"orig-app");
    let assets = FsAssetStore::new(dir.path().join("assets"));
    let settings = SettingsStore::open(&dir.path().join("s.sqlite3")).unwrap();
    let mut history = LookHistoryStore::new(dir.path().join("h.json"));
    let journal_path = dir.path().join("txn.log");
    let mut journal = crate::txn::FileJournal::new(&journal_path);
    let mut ledger = MemLedgerStore::new();
    let mut txn = TxnIdAllocator::starting_at(1);

    // Apply A (durable journal now holds a committed txn), then Reset.
    {
        let fake = FakePlatform::new(world.clone());
        let ops = IconOps::new(IconPlatform::new(&fake, &fake, &assets), &settings);
        let mut s = IconApplySession::begin(0, 1);
        s.push("app", 0, master_b64([1, 2, 3, 255]));
        let scan = vec![ScannedItem { item: app.clone(), fingerprint: dm_domain::Fingerprint::of_bytes(&world.borrow().get(&app.path).unwrap()) }];
        ops.commit_apply(s, style(1), Some("A".into()), "v1", 1, &scan, &[], &mut txn, &mut journal, &mut ledger, &mut history)
            .unwrap();
    }
    assert!(ledger.get(&app.id).unwrap().is_some(), "A is styled");
    {
        let fake = FakePlatform::new(world.clone());
        let ops = IconOps::new(IconPlatform::new(&fake, &fake, &assets), &settings);
        ops.reset_to_original(&mut journal, &mut ledger, &history).unwrap();
    }
    assert!(ledger.all().unwrap().is_empty(), "reset emptied the ledger");
    assert_eq!(world.borrow().get(&app.path).unwrap(), b"orig-app", "desktop reverted to original");

    // Simulate a RESTART: startup recovery over the same on-disk journal into a FRESH ledger. It
    // must find NOTHING to revive — the reset checkpointed the committed records away.
    let mut restart_ledger = MemLedgerStore::new();
    let fake = FakePlatform::new(world.clone());
    let mut restart_journal = crate::txn::FileJournal::new(&journal_path);
    crate::txn::recover_from_journal(&mut restart_journal, &fake, &fake, &mut restart_ledger).unwrap();
    assert!(
        restart_ledger.all().unwrap().is_empty(),
        "a restart after reset must NOT revive the deleted ledger rows",
    );
}

#[test]
fn a_kept_icon_that_was_styled_is_reverted_not_left() {
    // spec 06 §2 / codex 2026-07-12: setting a CURRENTLY-styled icon to 「保留原样」 must REVERT it to
    // its original — the frontend excludes it from the bake, so without the restore_ids path the real
    // desktop would keep the old style while the UI shows original.
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    let keep = item("keep", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.seed(&keep, b"orig-keep");
    // Apply A: both get styled.
    f.apply(
        &[("a", 0, [1, 1, 1, 255]), ("keep", 0, [2, 2, 2, 255])],
        style(1),
        "A",
        "v1",
        &[a.clone(), keep.clone()],
    );
    assert!(f.ledger.get(&keep.id).unwrap().is_some());
    assert_ne!(f.world.borrow().get(&keep.path).unwrap(), b"orig-keep", "keep is styled after A");

    // Re-apply B, but the user set `keep` to 「保留原样」 → it rides restore_ids, NOT the bake set.
    let out = f.apply_reverting(&[("a", 0, [9, 9, 9, 255])], style(2), "B", "v2", &[a.clone(), keep.clone()], &["keep"]);

    assert_eq!(out.reverted, vec![ItemId::from_raw("keep")]);
    assert_eq!(f.world.borrow().get(&keep.path).unwrap(), b"orig-keep", "the kept icon is reverted on the real desktop");
    assert!(f.ledger.get(&keep.id).unwrap().is_none(), "its ledger row is gone");
    assert!(f.ledger.get(&a.id).unwrap().is_some(), "a is re-styled");
}

#[test]
fn keep_restore_revert_fault_reports_degraded_never_a_bare_err() {
    // codex R3-Block 4: a 「保留原样」 revert that faults mid-commit must NOT bubble a bare Err over
    // the fresh item it already styled — it records the fault as `degraded` and returns the
    // authoritative state, so the host answers ok:false + persisted (not "nothing changed").
    let mut f = Fixture::new();
    let fresh = item("fresh", ItemKind::Shortcut);
    let keep = item("keep", ItemKind::Shortcut);
    f.seed(&fresh, b"orig-fresh");
    f.seed(&keep, b"orig-keep");
    f.apply(
        &[("fresh", 0, [1, 1, 1, 255]), ("keep", 0, [2, 2, 2, 255])],
        style(1),
        "A",
        "v1",
        &[fresh.clone(), keep.clone()],
    );
    // The kept icon's revert will fault (a locked file / COM error on the real desktop).
    f.world.borrow_mut().fail_restore(&keep.path);

    // Re-apply B: re-style `fresh`, set `keep` to 「保留原样」 (rides restore_ids). keep's revert faults.
    let out = f.apply_reverting(&[("fresh", 0, [9, 9, 9, 255])], style(2), "B", "v2", &[fresh.clone(), keep.clone()], &["keep"]);

    assert_eq!(out.committed, vec![ItemId::from_raw("fresh")], "the fresh item still committed");
    assert!(out.error.is_none(), "the styling batch itself succeeded");
    assert!(out.degraded.is_some(), "the keep-revert fault is surfaced as degraded, never a bare Err");
    assert!(!out.reverted.contains(&ItemId::from_raw("keep")), "keep was NOT reverted (its restore faulted)");
    // Desktop == ledger for keep: it stays styled with its row intact → self-heals on a later reset.
    assert_ne!(f.world.borrow().get(&keep.path).unwrap(), b"orig-keep", "keep stays styled (revert failed)");
    assert!(f.ledger.get(&keep.id).unwrap().is_some(), "keep's ledger row is kept (consistent with the desktop)");
}

#[test]
fn a_failed_styling_batch_that_still_reverted_a_kept_icon_reports_both() {
    // codex R4-Block 1: a keep-revert lands BEFORE the (transactional) styling batch, so if the
    // batch then rolls back, the desktop STILL changed — the outcome must carry error=Some AND a
    // non-empty `reverted`, so the host shows a partial-change repair toast, never "nothing changed".
    let mut f = Fixture::new();
    let keep = item("keep", ItemKind::Shortcut);
    let styled = item("styled", ItemKind::Shortcut);
    f.seed(&keep, b"orig-keep");
    f.seed(&styled, b"orig-styled");
    f.apply(
        &[("keep", 0, [1, 1, 1, 255]), ("styled", 0, [2, 2, 2, 255])],
        style(1),
        "A",
        "v1",
        &[keep.clone(), styled.clone()],
    );
    // Re-apply B: keep → 「保留原样」 (reverts cleanly), styled → re-style but its apply FAULTS → the
    // driver rolls the styling batch back, leaving keep reverted.
    f.world.borrow_mut().fail_apply(&styled.path);
    let out = f.apply_reverting(
        &[("styled", 0, [9, 9, 9, 255])],
        style(2),
        "B",
        "v2",
        &[keep.clone(), styled.clone()],
        &["keep"],
    );
    assert!(out.error.is_some(), "the styling batch failed/rolled back");
    assert_eq!(out.reverted, vec![ItemId::from_raw("keep")], "the kept icon was still reverted");
    assert_eq!(
        f.world.borrow().get(&keep.path).unwrap(),
        b"orig-keep",
        "keep is original on the desktop → this is NOT a no-op failure"
    );
}

#[test]
fn reset_revert_fault_reports_degraded_and_reverts_the_others() {
    // codex R3-Block 4: a reset whose Nth item revert faults must not abandon the items already
    // reverted to a bare Err — it reverts what it can and surfaces the fault via `degraded`.
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    let stuck = item("stuck", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.seed(&stuck, b"orig-stuck");
    f.apply(
        &[("a", 0, [1, 1, 1, 255]), ("stuck", 0, [2, 2, 2, 255])],
        style(1),
        "A",
        "v1",
        &[a.clone(), stuck.clone()],
    );
    // `stuck`'s revert will fault; `a`'s must still land.
    f.world.borrow_mut().fail_restore(&stuck.path);

    let out = f.reset(false);

    assert!(out.degraded.is_some(), "the stuck revert is surfaced as degraded, not a bare Err");
    assert_eq!(out.restored, vec![ItemId::from_raw("a")], "a was reverted despite stuck's fault");
    assert_eq!(f.world.borrow().get(&a.path).unwrap(), b"orig-a", "a is back to its original on the desktop");
    // stuck stays styled with its ledger row (desktop == ledger) → a later reset heals it.
    assert_ne!(f.world.borrow().get(&stuck.path).unwrap(), b"orig-stuck", "stuck stays styled (revert failed)");
    assert!(f.ledger.get(&stuck.id).unwrap().is_some(), "stuck's ledger row is kept");
}
