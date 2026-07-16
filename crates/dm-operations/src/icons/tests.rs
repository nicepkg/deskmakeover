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
use crate::txn::fakes::{FakeElevatedIconApplier, FakePlatform, World};
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
                ScannedItem { item: it.clone(), fingerprint: fp, cas_icon: None, source_ok: true }
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
            &scope::ScopeRoots::Unprivileged,
            None,
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
        ops.reset_to_original(&scope::ScopeRoots::Unprivileged, None, &mut self.journal, &mut self.ledger, &self.history)
            .unwrap()
    }

    /// Applies with an explicit privileged scope + (optional) elevated port — the wiring the real
    /// Windows host uses. Privileged-scope targets route to the elevated batch; the rest to the driver.
    fn apply_scoped(
        &mut self,
        masters: &[(&str, u32, [u8; 4])],
        style: IconStyle,
        look_id: &str,
        scan: &[DesktopItem],
        scope: &scope::ScopeRoots,
        elevated: Option<&dyn dm_domain::ElevatedIconApplier>,
    ) -> IconApplyOutcome {
        let mut session = IconApplySession::begin(0, masters.len());
        for (id, slot, rgba) in masters {
            session.push(*id, *slot, master_b64(*rgba));
        }
        let scanned = self.scanned(scan);
        let fake = FakePlatform::new(self.world.clone());
        let platform = IconPlatform::new(&fake, &fake, &self.assets);
        let ops = IconOps::new(platform, &self.settings);
        ops.commit_apply(
            session, style, Some("L".into()), look_id, 1, &scanned, &[], scope, elevated,
            &mut self.txn, &mut self.journal, &mut self.ledger, &mut self.history,
        )
        .unwrap()
    }

    fn reset_scoped(
        &mut self,
        scope: &scope::ScopeRoots,
        elevated: Option<&dyn dm_domain::ElevatedIconApplier>,
    ) -> IconResetOutcome {
        let fake = FakePlatform::new(self.world.clone());
        let platform = IconPlatform::new(&fake, &fake, &self.assets);
        let ops = IconOps::new(platform, &self.settings);
        ops.reset_to_original(scope, elevated, &mut self.journal, &mut self.ledger, &self.history)
            .unwrap()
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
        let cold = ops.read_state(&f.history, &f.ledger, &f.journal).unwrap();
        assert!(cold.saved_style.is_none() && cold.history.is_empty() && !cold.applied);
    }
    let app = item("app", ItemKind::Shortcut);
    f.seed(&app, b"orig-app");
    f.apply(&[("app", 0, [1, 2, 3, 255])], style(1), "A", "v1", &[app.clone()]);
    let fake = FakePlatform::new(f.world.clone());
    let ops = IconOps::new(IconPlatform::new(&fake, &fake, &f.assets), &f.settings);
    let warm = ops.read_state(&f.history, &f.ledger, &f.journal).unwrap();
    assert!(warm.applied && warm.saved_style.is_some() && warm.history.len() == 1);
}

#[test]
fn reset_reverts_originals_clears_saved_style_and_gcs_assets() {
    let mut f = Fixture::new();
    let app = item("app", ItemKind::Shortcut);
    f.seed(&app, b"orig-app");
    f.apply(&[("app", 0, [1, 2, 3, 255])], style(1), "A", "v1", &[app.clone()]);
    assert_eq!(f.ico_files().len(), 1);
    // A completed Apply set ②, so the user could enable auto-format — do so, to prove reset
    // turns it back off (the coupling below).
    f.settings
        .set(&dm_contracts::SettingsPatch {
            keep_new_icons_styled: Some(true),
            ..Default::default()
        })
        .unwrap();
    assert!(f.settings.get().unwrap().keep_new_icons_styled);

    let out = f.reset(false);

    assert_eq!(out.restored, vec![ItemId::from_raw("app")]);
    assert!(out.skipped.is_empty());
    // The desktop is back to the true original.
    assert_eq!(f.world.borrow().get(&app.path).unwrap(), b"orig-app");
    // ① emptied, ② cleared, so nothing is applied and the resident stays dormant.
    assert!(f.ledger.all().unwrap().is_empty());
    assert!(f.settings.get_saved_style().unwrap().is_none());
    // Reset coupling (spec 07 §10 ★): the auto-format toggle is turned OFF in the same operation,
    // so a reset desktop never silently re-styles the next new icon.
    assert!(!f.settings.get().unwrap().keep_new_icons_styled, "reset disables auto-format");
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
        let scan = vec![ScannedItem { item: app.clone(), fingerprint: dm_domain::Fingerprint::of_bytes(&world.borrow().get(&app.path).unwrap()), cas_icon: None, source_ok: true }];
        ops.commit_apply(s, style(1), Some("A".into()), "v1", 1, &scan, &[], &scope::ScopeRoots::Unprivileged, None, &mut txn, &mut journal, &mut ledger_a, &mut history)
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
        let scan = vec![ScannedItem { item: app.clone(), fingerprint: dm_domain::Fingerprint::of_bytes(&world.borrow().get(&app.path).unwrap()), cas_icon: None, source_ok: true }];
        let out = ops
            .commit_apply(s, style(2), Some("B".into()), "v2", 2, &scan, &[], &scope::ScopeRoots::Unprivileged, None, &mut txn, &mut journal, &mut ledger_b, &mut history)
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
        let scan = vec![ScannedItem { item: app.clone(), fingerprint: dm_domain::Fingerprint::of_bytes(&world.borrow().get(&app.path).unwrap()), cas_icon: None, source_ok: true }];
        ops.commit_apply(s, style(1), Some("A".into()), "v1", 1, &scan, &[], &scope::ScopeRoots::Unprivileged, None, &mut txn, &mut journal, &mut ledger, &mut history)
            .unwrap();
    }
    assert!(ledger.get(&app.id).unwrap().is_some(), "A is styled");
    {
        let fake = FakePlatform::new(world.clone());
        let ops = IconOps::new(IconPlatform::new(&fake, &fake, &assets), &settings);
        ops.reset_to_original(&scope::ScopeRoots::Unprivileged, None, &mut journal, &mut ledger, &history).unwrap();
    }
    assert!(ledger.all().unwrap().is_empty(), "reset emptied the ledger");
    assert_eq!(world.borrow().get(&app.path).unwrap(), b"orig-app", "desktop reverted to original");

    // Simulate a RESTART: startup recovery over the same on-disk journal into a FRESH ledger. It
    // must find NOTHING to revive — the reset checkpointed the committed records away.
    let mut restart_ledger = MemLedgerStore::new();
    let fake = FakePlatform::new(world.clone());
    let mut restart_journal = crate::txn::FileJournal::new(&journal_path);
    crate::txn::recover_from_journal(&mut restart_journal, &fake, &fake, &mut restart_ledger, &scope::ScopeRoots::Unprivileged).unwrap();
    assert!(
        restart_ledger.all().unwrap().is_empty(),
        "a restart after reset must NOT revive the deleted ledger rows",
    );
}

#[test]
fn reset_leaves_privileged_scope_rows_untouched_and_surfaces_them() {
    // audit F2b (owner#6 = SKIP + surface): a NON-elevated reset must never restore a Public
    // Desktop / ProgramData ledger row through the ordinary applier — it leaves the row AND the
    // desktop untouched and surfaces a "needs elevation" note, so a future elevated reset can
    // finish it, rather than attempting a restore that would only fail on a privileged target.
    let dir = tempfile::tempdir().unwrap();
    let world = World::shared();
    let app = item("app", ItemKind::Shortcut); // path C:/Desktop/app
    world.borrow_mut().put(&app.path, b"orig-app");
    let assets = FsAssetStore::new(dir.path().join("assets"));
    let settings = SettingsStore::open(&dir.path().join("s.sqlite3")).unwrap();
    let mut history = LookHistoryStore::new(dir.path().join("h.json"));
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();
    let mut txn = TxnIdAllocator::starting_at(1);

    // Style it → a real committed ledger row + a styled desktop.
    {
        let fake = FakePlatform::new(world.clone());
        let ops = IconOps::new(IconPlatform::new(&fake, &fake, &assets), &settings);
        let mut s = IconApplySession::begin(0, 1);
        s.push("app", 0, master_b64([9, 9, 9, 255]));
        let scan = vec![ScannedItem {
            item: app.clone(),
            fingerprint: dm_domain::Fingerprint::of_bytes(&world.borrow().get(&app.path).unwrap()),
            cas_icon: None,
            source_ok: true,
        }];
        ops.commit_apply(s, style(1), Some("A".into()), "v1", 1, &scan, &[], &scope::ScopeRoots::Unprivileged, None, &mut txn, &mut journal, &mut ledger, &mut history)
            .unwrap();
    }
    let styled = world.borrow().get(&app.path).unwrap();
    assert!(ledger.get(&app.id).unwrap().is_some(), "row exists before reset");

    // Reset under a Resolved scope whose Public Desktop root COVERS `C:/Desktop` → the row is privileged.
    let privileged = scope::ScopeRoots::resolved(vec!["C:/Desktop".into()], vec!["C:/ProgramData".into()]).unwrap();
    let out = {
        let fake = FakePlatform::new(world.clone());
        let ops = IconOps::new(IconPlatform::new(&fake, &fake, &assets), &settings);
        ops.reset_to_original(&privileged, None, &mut journal, &mut ledger, &history).unwrap()
    };

    // Observable contract: the desktop keeps its styled bytes, the ledger keeps the row, and it is
    // counted toward `skipped` (left-alone, ok:true) — never restored, never a false runtime-fault
    // degrade. (The toast REASON text ("你自己改过") is imprecise for a privileged item; a distinct
    // "needs elevation" surface is M8 work — see the gate comment — and unreachable until then.)
    assert_eq!(world.borrow().get(&app.path).unwrap(), styled, "privileged desktop must NOT be restored");
    assert!(ledger.get(&app.id).unwrap().is_some(), "privileged row must NOT be dropped");
    assert!(out.restored.is_empty(), "nothing restored under a privileged scope");
    assert_eq!(out.skipped, vec![app.id.clone()], "the privileged row is left alone (counted as skipped)");
    assert!(out.degraded.is_none(), "a privileged skip is NOT a runtime-fault degrade");
}

#[test]
fn reset_still_heals_a_deleted_privileged_row_without_a_desktop_write() {
    // codex F2b-review 🟡: the §14 gate must be DEEP (only the actual restore arm), so SAFE ledger
    // healing is not suppressed. A privileged row whose icon the user DELETED needs no privileged
    // desktop write to drop its stale ledger row — the reset must still clear it, or the row + its
    // ICO leak forever and `applied` stays true.
    let dir = tempfile::tempdir().unwrap();
    let world = World::shared();
    let app = item("app", ItemKind::Shortcut); // path C:/Desktop/app
    world.borrow_mut().put(&app.path, b"orig-app");
    let assets = FsAssetStore::new(dir.path().join("assets"));
    let settings = SettingsStore::open(&dir.path().join("s.sqlite3")).unwrap();
    let mut history = LookHistoryStore::new(dir.path().join("h.json"));
    let mut journal = VecJournal::new();
    let mut ledger = MemLedgerStore::new();
    let mut txn = TxnIdAllocator::starting_at(1);
    {
        let fake = FakePlatform::new(world.clone());
        let ops = IconOps::new(IconPlatform::new(&fake, &fake, &assets), &settings);
        let mut s = IconApplySession::begin(0, 1);
        s.push("app", 0, master_b64([7, 7, 7, 255]));
        let scan = vec![ScannedItem {
            item: app.clone(),
            fingerprint: dm_domain::Fingerprint::of_bytes(&world.borrow().get(&app.path).unwrap()),
            cas_icon: None,
            source_ok: true,
        }];
        ops.commit_apply(s, style(1), Some("A".into()), "v1", 1, &scan, &[], &scope::ScopeRoots::Unprivileged, None, &mut txn, &mut journal, &mut ledger, &mut history)
            .unwrap();
    }
    // The user deletes the icon after styling → read_fingerprint returns NotFound.
    world.borrow_mut().remove(&app.path);
    assert!(ledger.get(&app.id).unwrap().is_some(), "row exists before reset");

    let privileged = scope::ScopeRoots::resolved(vec!["C:/Desktop".into()], vec!["C:/ProgramData".into()]).unwrap();
    let out = {
        let fake = FakePlatform::new(world.clone());
        let ops = IconOps::new(IconPlatform::new(&fake, &fake, &assets), &settings);
        ops.reset_to_original(&privileged, None, &mut journal, &mut ledger, &history).unwrap()
    };

    // The stale row was dropped (a local ledger op, no privileged desktop write) — no leak.
    assert!(ledger.get(&app.id).unwrap().is_none(), "a deleted privileged icon's stale row is healed");
    assert!(out.skipped.is_empty(), "a deleted icon is not a needs-elevation skip");
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
fn a_lingering_row_whose_desktop_is_already_original_is_healed_not_skipped() {
    // codex R4-Block 2: if a revert's `ledger.remove` faulted, the desktop is original but the row
    // lingers (last_applied=styled). A later reset must HEAL it (remove the row), NOT read the
    // mismatch as a hand-edit and skip it forever — which would poison re-apply with a stale CAS
    // anchor and reset with a false "你自己改过".
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.apply(&[("a", 0, [1, 1, 1, 255])], style(1), "A", "v1", &[a.clone()]);
    assert!(f.ledger.get(&a.id).unwrap().is_some(), "styled → row present");
    // Simulate "restore landed but the remove faulted": put the desktop back to the true original
    // WITHOUT removing the ledger row (so last_applied still says styled).
    f.world.borrow_mut().put(&a.path, b"orig-a");

    let out = f.reset(false);

    assert!(f.ledger.get(&a.id).unwrap().is_none(), "the lingering row was healed (removed)");
    assert!(out.skipped.is_empty(), "it must NOT be counted as a hand-edit skip");
    assert!(out.restored.is_empty(), "nothing to revert — the desktop was already original");
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

#[test]
fn a_poisoned_row_re_applied_directly_is_dropped_and_conflicts_then_a_second_apply_styles() {
    // codex R5-#2 / R8-#1: a prior keep/reset restored this item on disk (desktop == original) but its
    // paired `ledger.remove` faulted, leaving a POISONED row (last_applied=styled). The observable tuple
    // (current == original != last_applied) is IDENTICAL to a user who manually restored the icon, and a
    // fingerprint-equality "is the scan fresh?" test is ABA-unsafe — so the driver must NOT silently
    // re-style it. The first re-apply DROPS the stale row (un-poisoning it, so it can never cause a
    // PERMANENT conflict) and CONFLICTS; the row is now gone, so a SECOND apply is an ordinary fresh
    // apply that styles the icon cleanly.
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.apply(&[("a", 0, [1, 1, 1, 255])], style(1), "A", "v1", &[a.clone()]);
    // Poison: the desktop is back to the true original, but the ledger row lingers (remove faulted).
    f.world.borrow_mut().put(&a.path, b"orig-a");

    // First re-apply: the heal drops the row + conflicts, never silently overwriting.
    let first = f.apply(&[("a", 0, [7, 7, 7, 255])], style(2), "B", "v2", &[a.clone()]);
    assert!(first.committed.is_empty(), "the ambiguous heal must NOT silently re-style");
    assert_eq!(first.conflicts, vec![ItemId::from_raw("a")], "it conflicts, forcing a fresh attempt");
    assert!(f.ledger.get(&a.id).unwrap().is_none(), "the poison row WAS dropped (un-poisoned)");
    assert!(
        first.requires_rescan,
        "a heal demands the host fence the scan revision — a same-revision retry would pass the \
         row-less fresh CAS and overwrite the possible hand-edit (codex R9-#1)",
    );

    // Second apply: no ledger row now → an ordinary fresh apply styles it.
    let second = f.apply(&[("a", 0, [7, 7, 7, 255])], style(2), "B2", "v3", &[a.clone()]);
    assert_eq!(second.committed, vec![ItemId::from_raw("a")], "with the row gone it styles cleanly");
    assert_eq!(
        f.ledger.get(&a.id).unwrap().unwrap().original_fingerprint,
        dm_domain::Fingerprint::of_bytes(b"orig-a"),
        "the fresh apply captured the TRUE original anchor",
    );
    assert_ne!(f.world.borrow().get(&a.path).unwrap(), b"orig-a", "the desktop is styled");
}

#[test]
fn an_all_conflicts_apply_never_writes_the_saved_style_or_history() {
    // codex R5-#2: an apply where every icon CAS-conflicts (nothing styled) must NOT persist ② with a
    // look the desktop never wears, nor push ③ — writing them would make the host report a clean
    // success over a no-op and resume from a phantom style next launch.
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.apply(&[("a", 0, [1, 1, 1, 255])], style(1), "A", "v1", &[a.clone()]);
    let saved_after_first = f.settings.get_saved_style().unwrap();
    assert!(saved_after_first.is_some(), "the first, real apply persisted ②");
    let history_after_first = f.history.all().len();
    // Hand-edit A so a re-apply CAS-conflicts (current is neither original nor last-applied).
    f.world.borrow_mut().put(&a.path, b"user-hand-edit");

    let out = f.apply(&[("a", 0, [9, 9, 9, 255])], style(2), "B", "v2", &[a.clone()]);

    assert!(out.committed.is_empty(), "nothing was styled");
    assert_eq!(out.conflicts, vec![ItemId::from_raw("a")], "A CAS-conflicted");
    assert_eq!(f.settings.get_saved_style().unwrap(), saved_after_first, "② was NOT overwritten by the no-op");
    assert_eq!(f.history.all().len(), history_after_first, "③ got no phantom entry");
}

#[test]
fn a_rollback_with_no_keep_revert_still_flags_the_desktop_as_mutated() {
    // codex R5-#1: re-styling already-styled icons where the styling batch then rolls back DID move the
    // desktop (each icon was re-applied then restored to its true original), even though no keep-revert
    // ran and nothing committed. `desktop_mutated` must be true so the host shows a partial-change
    // repair toast, never "桌面没有改动".
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    let b = item("b", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.seed(&b, b"orig-b");
    f.apply(
        &[("a", 0, [1, 1, 1, 255]), ("b", 0, [2, 2, 2, 255])],
        style(1),
        "A",
        "v1",
        &[a.clone(), b.clone()],
    );
    // Re-style both, but b's apply faults → the batch rolls back (a re-applied then restored to orig-a).
    f.world.borrow_mut().fail_apply(&b.path);
    let out = f.apply(
        &[("a", 0, [8, 8, 8, 255]), ("b", 0, [9, 9, 9, 255])],
        style(2),
        "B",
        "v2",
        &[a.clone(), b.clone()],
    );

    assert!(out.error.is_some(), "the styling batch rolled back");
    assert!(out.reverted.is_empty(), "no keep-revert ran — nothing was opted out");
    assert!(out.committed.is_empty(), "nothing committed");
    assert!(out.desktop_mutated, "the rollback moved the desktop → not 'nothing changed' (codex R5-#1)");
    assert_eq!(
        f.world.borrow().get(&a.path).unwrap(),
        b"orig-a",
        "the rolled-back item was restored to its TRUE original — a real desktop change from styled-v1",
    );
}

#[test]
fn a_clean_recovery_that_restored_the_desktop_defers_the_current_apply() {
    // codex R5-#3: a prior crash left an incomplete transaction in the journal. This apply's up-front
    // recovery cleanly ABORTS it — restoring (mutating) the crashed item's desktop. The apply must NOT
    // stack a new styling on top (a later bare `?` would then surface over a recovery-moved desktop);
    // it defers with a repair/resync note + the authoritative state, and the retry finds a clean journal.
    use crate::txn::journal::{JournalRecord, JournalSink};
    let mut f = Fixture::new();
    let x = item("x", ItemKind::Shortcut); // an item a PRIOR crashed txn styled
    let y = item("y", ItemKind::Shortcut); // the item THIS apply wants to style
    f.seed(&y, b"orig-y");
    // The prior crash left X styled on disk with an incomplete (no-terminal) txn in the journal.
    let orig_x = b"orig-x".to_vec();
    let styled_x = b"styled-x-bytes".to_vec();
    f.world.borrow_mut().put(&x.path, &styled_x);
    let target = x.target();
    for rec in [
        JournalRecord::TxnBegin { txn: 1, items: vec![x.id.clone()] },
        JournalRecord::ItemPrepared {
            txn: 1,
            item: x.id.clone(),
            target: target.clone(),
            anchor: dm_domain::RestoreAnchor::FileBytes { bytes: orig_x.clone() },
            original_fingerprint: dm_domain::Fingerprint::of_bytes(&orig_x),
            expected_fingerprint: dm_domain::Fingerprint::of_bytes(&orig_x),
            asset_hash: "hashX".into(),
            owned: dm_domain::OwnedFields::icon_only(),
            pinned_seed: None,
        },
        JournalRecord::ItemApplied {
            txn: 1,
            item: x.id.clone(),
            new_fingerprint: dm_domain::Fingerprint::of_bytes(&styled_x),
        },
        // no terminal → incomplete → recovery aborts it, restoring X to orig-x.
    ] {
        f.journal.append(&rec).unwrap();
    }

    let out = f.apply(&[("y", 0, [3, 3, 3, 255])], style(1), "Y", "v1", &[y.clone()]);

    assert_eq!(f.world.borrow().get(&x.path).unwrap(), orig_x, "recovery restored the crashed item's desktop");
    assert!(out.committed.is_empty(), "the current apply was DEFERRED, not stacked on top of recovery");
    assert!(out.degraded.is_some(), "it returns a repair/resync note, never a bare Err over a moved desktop");
    assert!(out.degraded.as_deref().unwrap().contains("recovered"), "the note explains the recovery");
    assert_eq!(f.world.borrow().get(&y.path).unwrap(), b"orig-y", "Y was NOT styled this round");
}

#[test]
fn a_poisoned_row_re_applied_with_a_stale_scan_is_healed_but_not_silently_restyled() {
    // codex R7-#1: a poison row (a restore landed, its `ledger.remove` faulted) and a user's MANUAL
    // restore-to-exact-original since the scan are INDISTINGUISHABLE by fingerprint (both: current ==
    // original != last_applied). When the scan is STALE (its fingerprint still reads the old styled
    // state), a direct re-apply must NOT silently overwrite what could be the user's hand-edit. It
    // drops the stale row (un-poisoning it, so it can never cause a PERMANENT conflict) and CONFLICTS,
    // forcing a rescan — after which a fresh apply styles it unambiguously (the fresh-scan path is the
    // sibling test). The earlier fresh-scan test alone masked this stale/ambiguous case.
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.apply(&[("a", 0, [1, 1, 1, 255])], style(1), "A", "v1", &[a.clone()]);
    // The fingerprint a PRE-restore scan would still hold (the styled desktop).
    let stale_fp = dm_domain::Fingerprint::of_bytes(&f.world.borrow().get(&a.path).unwrap());
    // Poison / or a manual restore: desktop back to original, but the ledger row lingers.
    f.world.borrow_mut().put(&a.path, b"orig-a");

    // Re-apply A directly with a STALE scan (styled fingerprint) — NOT the fixture's fresh re-scan.
    let mut session = IconApplySession::begin(0, 1);
    session.push("a", 0, master_b64([7, 7, 7, 255]));
    let stale_scan = vec![ScannedItem { item: a.clone(), fingerprint: stale_fp, cas_icon: None, source_ok: true }];
    let fake = FakePlatform::new(f.world.clone());
    let ops = IconOps::new(IconPlatform::new(&fake, &fake, &f.assets), &f.settings);
    let out = ops
        .commit_apply(
            session,
            style(2),
            Some("B".into()),
            "v2",
            2,
            &stale_scan,
            &[],
            &scope::ScopeRoots::Unprivileged,
            None,
            &mut f.txn,
            &mut f.journal,
            &mut f.ledger,
            &mut f.history,
        )
        .unwrap();

    assert!(out.committed.is_empty(), "a STALE scan must NOT silently overwrite a possible hand-edit");
    assert_eq!(out.conflicts, vec![ItemId::from_raw("a")], "it conflicts instead, forcing a rescan");
    assert!(f.ledger.get(&a.id).unwrap().is_none(), "but the stale poison row WAS dropped (un-poisoned)");
}

#[test]
fn a_revert_only_apply_still_writes_the_saved_style_and_history() {
    // codex R6-#2: an Apply that only REVERTS (the user opts every styled icon to 「保留原样」 and
    // styles none) is still a completed global Apply and MUST write ②③ (spec 07 §8.2) — the new look
    // "everything original" is the saved style. Only a genuinely zero-effect all-conflicts batch skips
    // ②③. Gating the ②③ write on `committed` alone (R5) wrongly dropped this case.
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.apply(&[("a", 0, [1, 1, 1, 255])], style(1), "A", "v1", &[a.clone()]);
    let history_before = f.history.all().len();

    // Re-apply with NO masters (nothing to style) and a → 「保留原样」 (revert it).
    let out = f.apply_reverting(&[], style(2), "B", "v2", &[a.clone()], &["a"]);

    assert_eq!(out.reverted, vec![ItemId::from_raw("a")], "a was reverted");
    assert!(out.committed.is_empty(), "nothing was styled");
    assert_eq!(f.settings.get_saved_style().unwrap(), Some(style(2)), "② reflects the revert-only Apply");
    assert_eq!(f.history.all().len(), history_before + 1, "③ recorded the completed Apply");
}

#[test]
fn a_heal_survives_a_same_batch_mutation_failure_and_still_demands_the_fence() {
    // codex R10-#A: item A is a poison row (heal → drop + conflict in phase 1); item B proceeds but
    // its mutation FAULTS, so the driver rolls the batch back. rollback/abandon used to rebuild the
    // outcome from Default, LOSING the phase-1 `healed` (and `conflicts`) — the host then skipped the
    // scan-revision fence, and a same-revision retry (A now row-less) passed the fresh CAS and
    // overwrote the possible hand-edit. The phase-1 outcome must survive the batch failure.
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    let b = item("b", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.seed(&b, b"orig-b");
    f.apply(
        &[("a", 0, [1, 1, 1, 255]), ("b", 0, [2, 2, 2, 255])],
        style(1),
        "A",
        "v1",
        &[a.clone(), b.clone()],
    );
    // Poison A (desktop back to original, row lingers); B's next apply will fault mid-mutation.
    f.world.borrow_mut().put(&a.path, b"orig-a");
    f.world.borrow_mut().fail_apply(&b.path);

    let out = f.apply(
        &[("a", 0, [7, 7, 7, 255]), ("b", 0, [9, 9, 9, 255])],
        style(2),
        "B",
        "v2",
        &[a.clone(), b.clone()],
    );

    assert!(out.error.is_some(), "B's fault rolled the styling batch back");
    assert!(out.conflicts.contains(&ItemId::from_raw("a")), "A's heal-conflict survives the rollback");
    assert!(f.ledger.get(&a.id).unwrap().is_none(), "A's poison row was dropped in phase 1");
    assert!(
        out.requires_rescan,
        "the heal must still demand the fence — losing it re-opens the same-revision ABA (codex R10-#A)",
    );
}

#[test]
fn a_policy_only_apply_with_no_current_targets_still_persists_the_intent() {
    // codex R9-#2: the user changes kindPolicy/typeOverrides when NO icon currently needs styling or
    // reverting (zero-target). There is no desktop effect, but the global intent MUST persist to ②③
    // (spec 07 §8.2) — the resident and the next launch resume from it. `packaged.is_empty()` alone is
    // not a no-op: only a zero-effect apply WITH conflicts is an incomplete one.
    let mut f = Fixture::new();
    let out = f.apply(&[], style(7), "策略", "v1", &[]);

    assert!(out.committed.is_empty() && out.reverted.is_empty() && out.conflicts.is_empty());
    assert!(out.intent_persisted, "a conflict-free zero-target Apply IS completed");
    assert_eq!(f.settings.get_saved_style().unwrap(), Some(style(7)), "② carries the policy intent");
    assert_eq!(f.history.all().len(), 1, "③ recorded the completed Apply");
}

#[test]
fn a_partial_revert_failure_does_not_write_the_saved_style() {
    // codex R7-#2: two icons both opt to 「保留原样」; A's restore lands, B's restore FAULTS (B still
    // wears the old style). This is NOT a completed Apply — writing ② ("everything original") while B
    // is still styled would resume from a lie next launch. The `repair.is_empty()` guard blocks ②③.
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    let b = item("b", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.seed(&b, b"orig-b");
    f.apply(
        &[("a", 0, [1, 1, 1, 255]), ("b", 0, [2, 2, 2, 255])],
        style(1),
        "A",
        "v1",
        &[a.clone(), b.clone()],
    );
    let saved_after_first = f.settings.get_saved_style().unwrap();
    // B's revert will fault; opt BOTH to 「保留原样」 (no masters, restore both).
    f.world.borrow_mut().fail_restore(&b.path);
    let out = f.apply_reverting(&[], style(2), "B", "v2", &[a.clone(), b.clone()], &["a", "b"]);

    assert_eq!(out.reverted, vec![ItemId::from_raw("a")], "only A reverted");
    assert!(out.degraded.is_some(), "B's restore fault surfaces as degraded");
    assert_eq!(
        f.settings.get_saved_style().unwrap(),
        saved_after_first,
        "② was NOT overwritten by the partial (incomplete) revert",
    );
}

#[test]
fn an_apply_whose_only_intent_is_a_hand_edited_revert_reports_no_effect() {
    // codex R7-#3: the user opts a styled icon to 「保留原样」 but has hand-edited it since the scan.
    // Trust-first leaves it alone — but the apply then did NOTHING, so it must NOT read as a clean
    // success (which would clear the draft). The hand-edit skip counts as a conflict → the host's
    // no-effect branch fires.
    let mut f = Fixture::new();
    let a = item("a", ItemKind::Shortcut);
    f.seed(&a, b"orig-a");
    f.apply(&[("a", 0, [1, 1, 1, 255])], style(1), "A", "v1", &[a.clone()]);
    let saved_after_first = f.settings.get_saved_style().unwrap();
    let history_after_first = f.history.all().len();
    // Hand-edit a since the scan (neither its original nor its last-applied).
    f.world.borrow_mut().put(&a.path, b"user-hand-edit");
    // Opt a to 「保留原样」 (no masters, restore a) — but a was hand-edited.
    let out = f.apply_reverting(&[], style(2), "B", "v2", &[a.clone()], &["a"]);

    assert!(out.committed.is_empty() && out.reverted.is_empty(), "nothing styled, nothing reverted");
    assert_eq!(out.conflicts, vec![ItemId::from_raw("a")], "the hand-edited revert skip counts as a conflict");
    // A zero-effect restore-only apply must NOT write a phantom ②③ (codex R8-#2): `packaged.is_empty()`
    // is not a valid "completed Apply" proxy — the real effect (committed || reverted) was empty.
    assert_eq!(f.settings.get_saved_style().unwrap(), saved_after_first, "② was NOT overwritten by the no-op");
    assert_eq!(f.history.all().len(), history_after_first, "③ got no phantom entry");
}

#[test]
fn a_reset_defers_when_up_front_recovery_had_to_heal_a_prior_crash() {
    // codex R6-#4: a reset whose up-front recovery ABORTS an interrupted txn already moved the desktop;
    // the ledger reset must NOT run on top (the strict `journal.checkpoint(&[])?` would otherwise bare-
    // Err over it). The outcome carries `deferred: true` so the host SKIPS the reset-only finalizers
    // (auto-format off + arrow lift), which would otherwise leave a partial state.
    use crate::txn::journal::{JournalRecord, JournalSink};
    let mut f = Fixture::new();
    let x = item("x", ItemKind::Shortcut);
    let orig_x = b"orig-x".to_vec();
    let styled_x = b"styled-x".to_vec();
    f.world.borrow_mut().put(&x.path, &styled_x);
    let target = x.target();
    for rec in [
        JournalRecord::TxnBegin { txn: 1, items: vec![x.id.clone()] },
        JournalRecord::ItemPrepared {
            txn: 1,
            item: x.id.clone(),
            target,
            anchor: dm_domain::RestoreAnchor::FileBytes { bytes: orig_x.clone() },
            original_fingerprint: dm_domain::Fingerprint::of_bytes(&orig_x),
            expected_fingerprint: dm_domain::Fingerprint::of_bytes(&orig_x),
            asset_hash: "h".into(),
            owned: dm_domain::OwnedFields::icon_only(),
            pinned_seed: None,
        },
        JournalRecord::ItemApplied {
            txn: 1,
            item: x.id.clone(),
            new_fingerprint: dm_domain::Fingerprint::of_bytes(&styled_x),
        },
    ] {
        f.journal.append(&rec).unwrap();
    }

    let out = f.reset(false);

    assert!(out.deferred, "reset deferred to let recovery heal the prior crash first");
    assert!(out.restored.is_empty(), "the ledger reset did NOT run this round");
    assert!(out.degraded.is_some(), "a repair/resync note is returned, never a bare Err");
    assert_eq!(f.world.borrow().get(&x.path).unwrap(), orig_x, "recovery restored the crashed item's desktop");
}

// ── Elevated batch (privileged shared items: Public Desktop / ProgramData) ─────────────────────

/// A Public-Desktop shortcut (privileged scope) — the ACL-protected item the elevated helper styles.
fn pub_item(id: &str) -> DesktopItem {
    DesktopItem {
        id: ItemId::from_raw(id),
        name: id.into(),
        path: format!("C:/Users/Public/Desktop/{id}"),
        kind: ItemKind::Shortcut,
        icon: None,
        state: ItemState::Ready,
        requires_explicit_consent: false,
        status_message: None,
    }
}

/// Roots where `C:/Users/Public/Desktop/*` is privileged but the user's own `C:/Desktop/*` is not.
fn public_desktop_scope() -> scope::ScopeRoots {
    scope::ScopeRoots::resolved(
        vec!["C:/Users/Public/Desktop".into()],
        vec!["C:/ProgramData".into()],
    )
    .unwrap()
}

#[test]
fn an_elevated_batch_styles_a_public_desktop_item_alongside_a_user_one_and_both_are_reversible() {
    let mut f = Fixture::new();
    let scope = public_desktop_scope();
    let elev = FakeElevatedIconApplier::new(f.world.clone());

    let user = item("app", ItemKind::Shortcut); // C:/Desktop/app — the driver's path
    let shared = pub_item("chrome"); // C:/Users/Public/Desktop/chrome — the elevated path
    f.seed(&user, b"orig-app");
    f.seed(&shared, b"orig-chrome");

    let out = f.apply_scoped(
        &[("app", 0, [1, 2, 3, 255]), ("chrome", 0, [4, 5, 6, 255])],
        style(1),
        "v1",
        &[user.clone(), shared.clone()],
        &scope,
        Some(&elev),
    );

    // BOTH styled + committed — the whole point ("我要所有图标都可修改").
    assert_eq!(out.committed.len(), 2, "both the user AND the shared item committed");
    assert!(out.error.is_none() && out.conflicts.is_empty());
    // The shared item went through the ELEVATED helper (one batch), the user one did not.
    assert_eq!(elev.applied_paths(), vec![shared.path.clone()]);
    // Both are tracked in the ledger → both reversible.
    assert!(f.ledger.get(&user.id).unwrap().is_some());
    assert!(f.ledger.get(&shared.id).unwrap().is_some());
    // The privileged desktop actually wears the elevated helper's styled bytes now (the staged asset
    // path is the real FsAssetStore path, so match on the elevated scheme rather than reconstruct it).
    assert!(
        f.world.borrow().get(&shared.path).unwrap().starts_with(b"STYLED-ELEV:"),
        "the privileged desktop wears the elevated-styled bytes"
    );

    // Reset reverts BOTH — the privileged one through the elevated helper (byte replay).
    let reset = f.reset_scoped(&scope, Some(&elev));
    assert_eq!(reset.restored.len(), 2, "both reverted");
    assert!(reset.skipped.is_empty());
    assert_eq!(elev.restored_paths(), vec![shared.path.clone()]);
    assert_eq!(f.world.borrow().get(&shared.path).unwrap(), b"orig-chrome", "privileged item back to original");
    assert!(f.ledger.get(&shared.id).unwrap().is_none(), "its ledger row is dropped");
}

#[test]
fn an_elevated_uac_cancel_reports_the_privileged_items_as_conflicts_not_a_failure() {
    let mut f = Fixture::new();
    let scope = public_desktop_scope();
    let elev = FakeElevatedIconApplier::new(f.world.clone());
    elev.set_outcome(dm_domain::ElevatedOutcome::Declined); // the user cancels the UAC prompt

    let user = item("app", ItemKind::Shortcut);
    let shared = pub_item("chrome");
    f.seed(&user, b"orig-app");
    f.seed(&shared, b"orig-chrome");

    let out = f.apply_scoped(
        &[("app", 0, [1, 2, 3, 255]), ("chrome", 0, [4, 5, 6, 255])],
        style(1),
        "v1",
        &[user.clone(), shared.clone()],
        &scope,
        Some(&elev),
    );

    // The user's own icon STILL applied — a UAC cancel must not undo it.
    assert_eq!(out.committed, vec![user.id.clone()], "the user desktop styled despite the cancel");
    // The declined shared item is a retryable CONFLICT, never a hard error.
    assert!(out.error.is_none(), "a UAC cancel is a user choice, not a failure");
    assert_eq!(out.conflicts, vec![shared.id.clone()]);
    assert!(f.ledger.get(&shared.id).unwrap().is_none(), "nothing ledgered for the declined item");
    assert_eq!(f.world.borrow().get(&shared.path).unwrap(), b"orig-chrome", "the privileged desktop is untouched");
    // The chosen style STILL persisted (②③) for the icons that did apply.
    assert_eq!(f.settings.get_saved_style().unwrap().as_ref(), Some(&style(1)));
}

#[test]
fn an_elevated_helper_failure_surfaces_as_an_error() {
    let mut f = Fixture::new();
    let scope = public_desktop_scope();
    let elev = FakeElevatedIconApplier::new(f.world.clone());
    elev.set_outcome(dm_domain::ElevatedOutcome::Failed("write denied".into()));

    let shared = pub_item("chrome");
    f.seed(&shared, b"orig-chrome");

    let out = f.apply_scoped(
        &[("chrome", 0, [4, 5, 6, 255])],
        style(1),
        "v1",
        &[shared.clone()],
        &scope,
        Some(&elev),
    );

    assert!(out.committed.is_empty(), "nothing committed on a helper failure");
    assert!(out.error.is_some(), "a real helper failure surfaces as an error (degraded toast)");
    assert!(out.desktop_mutated, "a helper failure leaves the desktop possibly-changed (best-effort rollback)");
    assert!(f.ledger.get(&shared.id).unwrap().is_none());
    // §P1-2: the elevated batch must NOT journal a `TxnRolledBack` terminal on a helper failure — its
    // rollback is best-effort, so the txn is left TERMINAL-LESS for crash recovery to inspect + adopt
    // forward any residue (a `TxnRolledBack` would tell recovery "cleanly reverted" over possible residue).
    let priv_txn = f.journal.records().iter().filter_map(|r| match r {
        crate::txn::JournalRecord::ItemPrepared { txn, .. } => Some(*txn),
        _ => None,
    }).max().expect("the privileged txn journaled an ItemPrepared");
    assert!(
        !f.journal.records().iter().any(|r| matches!(r, crate::txn::JournalRecord::TxnRolledBack { txn } if *txn == priv_txn)),
        "a failed elevated batch leaves the txn terminal-less (no TxnRolledBack) for recovery"
    );
}

#[test]
fn reset_keeps_the_row_of_a_privileged_item_the_helper_could_not_revert() {
    // §P2-1: the helper silently SKIPS an item the user re-edited during the UAC prompt (still exit 0).
    // The reset must CONFIRM each item is back to its original (a fresh fingerprint read) before dropping
    // its ledger row — otherwise a user's edit would be left untracked (no row → the app forgets it).
    let mut f = Fixture::new();
    let scope = public_desktop_scope();
    let elev = FakeElevatedIconApplier::new(f.world.clone());

    let shared = pub_item("chrome");
    f.seed(&shared, b"orig-chrome");
    f.apply_scoped(&[("chrome", 0, [4, 5, 6, 255])], style(1), "v1", &[shared.clone()], &scope, Some(&elev));
    assert!(f.ledger.get(&shared.id).unwrap().is_some(), "styled + ledgered");

    // The helper will skip reverting chrome (models a UAC-window re-edit) → it stays styled, exit 0.
    elev.skip_restore(&shared.path);
    let reset = f.reset_scoped(&scope, Some(&elev));

    assert!(reset.restored.is_empty(), "nothing confirmed reverted");
    assert_eq!(reset.skipped, vec![shared.id.clone()], "the un-reverted item is a skip, not a false restore");
    assert!(
        f.ledger.get(&shared.id).unwrap().is_some(),
        "its ledger row is KEPT (still tracked) — never dropped over an unconfirmed revert",
    );
}

#[test]
fn without_an_elevated_port_a_privileged_item_is_an_honest_conflict_never_a_doomed_write() {
    let mut f = Fixture::new();
    let scope = public_desktop_scope();

    let shared = pub_item("chrome");
    f.seed(&shared, b"orig-chrome");

    // No elevated port wired (an unwired host) → the privileged item is skipped, NOT written unelevated
    // (which would hit Access Denied and, before the partition, roll back the whole batch).
    let out = f.apply_scoped(
        &[("chrome", 0, [4, 5, 6, 255])],
        style(1),
        "v1",
        &[shared.clone()],
        &scope,
        None,
    );

    assert!(out.committed.is_empty());
    assert_eq!(out.conflicts, vec![shared.id.clone()], "left as an honest skip");
    assert!(out.error.is_none(), "a skip is not a failure — the desktop was never touched");
    assert_eq!(f.world.borrow().get(&shared.path).unwrap(), b"orig-chrome");
}
