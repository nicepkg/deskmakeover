//! In-memory fakes that stand in for the Windows platform ports, so the transaction driver and
//! recovery are exercised on the Mac host. The `World` is a virtual desktop (path → bytes); the
//! ports read/mutate it through interior mutability. `RecordingJournal` snapshots the world
//! after every durable append, which is what the kill-point battery replays.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dm_domain::{
    ApplyAssets, AssetRef, AssetStore, ElevatedApplyItem, ElevatedIconApplier, ElevatedOutcome,
    ElevatedRestoreItem, Fingerprint, IconApplier, ItemStateReader, ItemTarget, PortError,
    PortResult, RestoreAnchor,
};

use crate::error::{OperationError, Result};
use crate::txn::journal::{JournalRecord, JournalSink, VecJournal};

/// The deterministic "styled" bytes an apply writes for a given asset hash — models the icon
/// location swap changing the item's file bytes.
pub fn styled_bytes(asset_hash: &str) -> Vec<u8> {
    format!("STYLED:{asset_hash}").into_bytes()
}

/// The styled bytes a fake apply establishes for a full asset set. A paired item (the Recycle Bin)
/// folds in the EXACT empty ref, so the fake models P2-1 faithfully: the applied surface depends on
/// the empty asset it was handed, never ignores it. A single-asset item is just its primary.
fn fake_styled(assets: &ApplyAssets) -> Vec<u8> {
    match &assets.empty {
        Some(empty) => styled_bytes(&format!("{}+empty:{}", assets.primary.hash, empty.hash)),
        None => styled_bytes(&assets.primary.hash),
    }
}

/// A virtual desktop shared by all fake ports.
#[derive(Default)]
pub struct World {
    files: HashMap<String, Vec<u8>>,
    /// Paths whose `apply` must fail (simulates a mid-batch mutation error).
    apply_fails: HashSet<String>,
    /// Paths whose `restore` must fail (simulates a rollback that cannot complete).
    restore_fails: HashSet<String>,
    /// Paths whose `read_fingerprint` must fail with a non-NotFound error.
    read_fails: HashSet<String>,
    /// Paths whose `apply` succeeds but leaves the bytes unchanged (simulates a mutation that
    /// silently did nothing → the driver's verify step must catch it).
    noop_apply: HashSet<String>,
    /// Paths whose `apply` "succeeds" but lands a DIFFERENT asset than requested — it promises the
    /// requested asset's fingerprint yet writes some other state (models an O→A→B no-op-on-reapply
    /// or a stale writer). The driver's verify must reject it (P1-4).
    wrong_write: HashSet<String>,
    /// Live icon locations by target path, reported by `read_styleable_surface` — models the real
    /// reader exposing where an item's icon currently points (the styled-residue provenance input).
    /// A Vec so a multi-value surface (Recycle Bin's default/empty/full) can be modelled.
    live_icons: HashMap<String, Vec<(String, i32)>>,
}

impl World {
    pub fn shared() -> Rc<RefCell<World>> {
        Rc::new(RefCell::new(World::default()))
    }

    pub fn put(&mut self, path: &str, bytes: &[u8]) {
        self.files.insert(path.to_string(), bytes.to_vec());
    }

    pub fn get(&self, path: &str) -> Option<Vec<u8>> {
        self.files.get(path).cloned()
    }

    /// Removes a file (models the user deleting the icon) so `read_fingerprint` returns `NotFound`.
    pub fn remove(&mut self, path: &str) {
        self.files.remove(path);
    }

    pub fn fail_apply(&mut self, path: &str) {
        self.apply_fails.insert(path.to_string());
    }

    pub fn fail_restore(&mut self, path: &str) {
        self.restore_fails.insert(path.to_string());
    }

    pub fn fail_read(&mut self, path: &str) {
        self.read_fails.insert(path.to_string());
    }

    pub fn noop_apply(&mut self, path: &str) {
        self.noop_apply.insert(path.to_string());
    }

    /// Marks a path whose `apply` writes a different asset than requested while still promising the
    /// requested asset's fingerprint (the driver's verify must catch the mismatch).
    pub fn wrong_write(&mut self, path: &str) {
        self.wrong_write.insert(path.to_string());
    }

    /// Sets the (single) live icon location `read_styleable_surface` reports for `target_path` —
    /// models a desktop item whose icon currently points at `icon_path` (e.g. one of OUR generated
    /// assets, the styled-residue scenario).
    pub fn set_live_icon(&mut self, target_path: &str, icon_path: &str) {
        self.live_icons.insert(target_path.to_string(), vec![(icon_path.to_string(), 0)]);
    }

    /// Sets MULTIPLE live icon locations for `target_path` — models a multi-value surface (the
    /// Recycle Bin's default/empty/full) whose PARTIAL write left our asset in only some of them.
    pub fn set_live_icons(&mut self, target_path: &str, icon_paths: &[&str]) {
        self.live_icons.insert(
            target_path.to_string(),
            icon_paths.iter().map(|p| (p.to_string(), 0)).collect(),
        );
    }

    /// Clears injected apply/restore faults (simulates a transient condition clearing before a
    /// recovery retry).
    pub fn clear_faults(&mut self) {
        self.apply_fails.clear();
        self.restore_fails.clear();
    }

    pub fn snapshot(&self) -> HashMap<String, Vec<u8>> {
        self.files.clone()
    }

    pub fn restore_snapshot(&mut self, snap: HashMap<String, Vec<u8>>) {
        self.files = snap;
    }
}

/// The fake store's paired empty-variant path convention (mirrors the Windows `paired_empty`):
/// `x.ico` → `x-empty.ico`.
pub fn paired_empty_path(primary_path: &str) -> String {
    match primary_path.strip_suffix(".ico") {
        Some(stem) => format!("{stem}-empty.ico"),
        None => format!("{primary_path}-empty"),
    }
}

/// A reader/applier/asset-store bundle over one shared `World`.
#[derive(Clone)]
pub struct FakePlatform {
    world: Rc<RefCell<World>>,
    /// Paths whose anchor capture yields a `CaptureFailed` anchor (simulates `restore.captureError`).
    capture_fails: Rc<RefCell<HashSet<String>>>,
    /// Paths whose anchor capture returns a hard `Err`.
    capture_errors: Rc<RefCell<HashSet<String>>>,
    /// Asset paths the store has materialized (so `exists` can confirm a paired asset was written).
    materialized: Rc<RefCell<HashSet<String>>>,
    /// When set, `put_empty_variant` returns a ref but does NOT materialize it — a store that
    /// "succeeds" yet leaves the asset absent, so the driver's existence check must catch it.
    empty_variant_vanishes: Rc<RefCell<bool>>,
    /// When set, the paired empty asset is deleted DURING `apply` (after the pre-mutation existence
    /// check passed), so only the driver's post-apply re-check stands between it and a dangling ref.
    vanish_empty_after_apply: Rc<RefCell<bool>>,
}

impl FakePlatform {
    pub fn new(world: Rc<RefCell<World>>) -> Self {
        Self {
            world,
            capture_fails: Rc::new(RefCell::new(HashSet::new())),
            capture_errors: Rc::new(RefCell::new(HashSet::new())),
            materialized: Rc::new(RefCell::new(HashSet::new())),
            empty_variant_vanishes: Rc::new(RefCell::new(false)),
            vanish_empty_after_apply: Rc::new(RefCell::new(false)),
        }
    }

    /// Whether the store has materialized an asset at `path` (for test assertions).
    pub fn asset_exists(&self, path: &str) -> bool {
        self.materialized.borrow().contains(path)
    }

    /// Makes `put_empty_variant` report success while leaving the asset unmaterialized, so the
    /// driver's existence check is the only thing standing between it and a dangling registry ref.
    pub fn make_empty_variant_vanish(&self) {
        *self.empty_variant_vanishes.borrow_mut() = true;
    }

    /// Deletes the paired empty asset DURING `apply` (after the pre-mutation existence check), so
    /// the driver's post-apply re-check is what must catch the dangling reference (P2-1).
    pub fn make_empty_vanish_after_apply(&self) {
        *self.vanish_empty_after_apply.borrow_mut() = true;
    }

    /// The anchor capture returns a `CaptureFailed` anchor (no restore material — skipped).
    pub fn fail_capture(&self, path: &str) {
        self.capture_fails.borrow_mut().insert(path.to_string());
    }

    /// The anchor capture returns a hard `Err` (e.g. an I/O failure while reading the original).
    pub fn error_capture(&self, path: &str) {
        self.capture_errors.borrow_mut().insert(path.to_string());
    }
}

impl ItemStateReader for FakePlatform {
    fn read_fingerprint(&self, target: &ItemTarget) -> PortResult<Fingerprint> {
        if self.world.borrow().read_fails.contains(&target.path) {
            return Err(PortError::Io(format!("read failed for {}", target.path)));
        }
        match self.world.borrow().get(&target.path) {
            Some(bytes) => Ok(Fingerprint::of_bytes(&bytes)),
            None => Err(PortError::NotFound(target.path.clone())),
        }
    }

    fn read_styleable_surface(
        &self,
        target: &ItemTarget,
    ) -> PortResult<(Fingerprint, Vec<(String, i32)>)> {
        let fingerprint = self.read_fingerprint(target)?;
        let locations = self.world.borrow().live_icons.get(&target.path).cloned().unwrap_or_default();
        Ok((fingerprint, locations))
    }

    fn capture_anchor(&self, target: &ItemTarget) -> PortResult<RestoreAnchor> {
        if self.capture_errors.borrow().contains(&target.path) {
            return Err(PortError::Io(format!("anchor capture failed for {}", target.path)));
        }
        if self.capture_fails.borrow().contains(&target.path) {
            return Ok(RestoreAnchor::CaptureFailed { reason: "locked".into() });
        }
        match self.world.borrow().get(&target.path) {
            Some(bytes) => Ok(RestoreAnchor::FileBytes { bytes }),
            None => Err(PortError::NotFound(target.path.clone())),
        }
    }
}

impl IconApplier for FakePlatform {
    fn apply(&self, target: &ItemTarget, assets: &ApplyAssets) -> PortResult<Fingerprint> {
        // The fingerprint the applier PROMISES for this asset set, derived from the assets
        // themselves (primary AND the paired empty) and independent of whether the underlying write
        // actually lands — exactly the non-tautological derivation P1-1 requires of the real
        // applier, and honouring the empty ref (P2-1) rather than ignoring it. The driver re-reads
        // the live state and rejects the apply unless the two agree (P1-4).
        let promised = Fingerprint::of_bytes(&fake_styled(assets));
        {
            let mut world = self.world.borrow_mut();
            if world.apply_fails.contains(&target.path) {
                return Err(PortError::Com(format!("apply failed for {}", target.path)));
            }
            if world.noop_apply.contains(&target.path) {
                return Ok(promised); // "succeeds" but writes nothing → driver's re-read won't match
            }
            if world.wrong_write.contains(&target.path) {
                // Lands a different asset than requested while still promising the requested one.
                world.put(&target.path, &styled_bytes("stale-other-asset"));
                return Ok(promised);
            }
            world.put(&target.path, &fake_styled(assets));
        }
        // Model a paired empty asset that is deleted DURING the apply (a GC or an external process),
        // so the driver's post-apply existence re-check is the only thing between it and a committed
        // dangling reference (P2-1 window narrowing).
        if *self.vanish_empty_after_apply.borrow() {
            if let Some(empty) = &assets.empty {
                self.materialized.borrow_mut().remove(&empty.path);
            }
        }
        Ok(promised)
    }

    fn restore(&self, target: &ItemTarget, anchor: &RestoreAnchor) -> PortResult<()> {
        let mut world = self.world.borrow_mut();
        if world.restore_fails.contains(&target.path) {
            return Err(PortError::Io(format!("restore failed for {}", target.path)));
        }
        match anchor {
            RestoreAnchor::FileBytes { bytes } => {
                world.put(&target.path, bytes);
                Ok(())
            }
            RestoreAnchor::CaptureFailed { .. } => {
                Err(PortError::Unsupported("no restore material".into()))
            }
            other => Err(PortError::Unsupported(format!("fake cannot restore {other:?}"))),
        }
    }
}

impl AssetStore for FakePlatform {
    fn put(&self, hash: &str, _bytes: &[u8]) -> PortResult<AssetRef> {
        let path = format!("assets/{hash}.ico");
        self.materialized.borrow_mut().insert(path.clone());
        Ok(AssetRef::new(hash, path))
    }

    fn put_empty_variant(&self, primary: &AssetRef, _bytes: &[u8]) -> PortResult<AssetRef> {
        let path = paired_empty_path(&primary.path);
        if !*self.empty_variant_vanishes.borrow() {
            self.materialized.borrow_mut().insert(path.clone());
        }
        Ok(AssetRef::new(format!("{}-empty", primary.hash), path))
    }

    fn exists(&self, asset: &AssetRef) -> PortResult<bool> {
        Ok(self.materialized.borrow().contains(&asset.path))
    }

    fn gc(&self, _live: &[String]) -> PortResult<()> {
        Ok(())
    }

    /// Provenance opt-in matching `put`'s `assets/{hash}.ico` convention, so recovery's
    /// assets-provenance arm (the 2026-07-19 vanish fix) is exercisable against the fake. The
    /// default reader reports NO icon locations, so tests only hit this through a reader that
    /// overrides `read_styleable_surface` — existing never-clobber tests are unaffected.
    fn contains_path(&self, path: &str) -> bool {
        path.starts_with("assets/")
    }
}

/// A journal that snapshots the world after each durable append. `snapshots[i]` is the world
/// exactly as it was when `records[i]` became durable, so replaying `records[..=i]` against
/// `snapshots[i]` models a crash right after that fsync.
pub struct RecordingJournal {
    records: Vec<JournalRecord>,
    snapshots: Vec<HashMap<String, Vec<u8>>>,
    world: Rc<RefCell<World>>,
}

impl RecordingJournal {
    pub fn new(world: Rc<RefCell<World>>) -> Self {
        Self { records: Vec::new(), snapshots: Vec::new(), world }
    }

    pub fn records(&self) -> &[JournalRecord] {
        &self.records
    }

    pub fn snapshots(&self) -> &[HashMap<String, Vec<u8>>] {
        &self.snapshots
    }
}

impl JournalSink for RecordingJournal {
    fn append(&mut self, record: &JournalRecord) -> Result<()> {
        self.records.push(record.clone());
        self.snapshots.push(self.world.borrow().snapshot());
        Ok(())
    }

    fn read_all(&self) -> Result<Vec<JournalRecord>> {
        Ok(self.records.clone())
    }
}

/// A journal that injects an append failure at a chosen point, with three fault shapes so the
/// crash-window cases (P1-4/P1-5) can be modelled:
/// * `fail_on_call(n)` — fail exactly the `n`-th append, writing nothing (a clean failure);
/// * `fail_from(n)` — fail the `n`-th append AND every one after (a persistent journal failure);
/// * `fail_after_write(n)` — write the `n`-th record to the log THEN report failure (models a
///   record that reached disk while the fsync returned an error — the outcome-ambiguous commit).
pub struct FailingJournal {
    inner: VecJournal,
    fail_at: usize,
    persistent: bool,
    write_before_fail: bool,
    calls: usize,
}

impl FailingJournal {
    fn new(fail_at: usize, persistent: bool, write_before_fail: bool) -> Self {
        Self { inner: VecJournal::new(), fail_at, persistent, write_before_fail, calls: 0 }
    }

    /// Fail exactly the `n`-th append call (1-based), writing nothing; a large `n` never fails.
    pub fn fail_on_call(n: usize) -> Self {
        Self::new(n, false, false)
    }

    /// Fail the `n`-th append and every append after it (a persistent journal outage).
    pub fn fail_from(n: usize) -> Self {
        Self::new(n, true, false)
    }

    /// Write the `n`-th record to the log, then report the append as failed (a durable record whose
    /// fsync returned an error).
    pub fn fail_after_write(n: usize) -> Self {
        Self::new(n, false, true)
    }

    pub fn records(&self) -> &[JournalRecord] {
        self.inner.records()
    }
}

impl JournalSink for FailingJournal {
    fn append(&mut self, record: &JournalRecord) -> Result<()> {
        self.calls += 1;
        let fail = if self.persistent { self.calls >= self.fail_at } else { self.calls == self.fail_at };
        if fail {
            if self.write_before_fail {
                let _ = self.inner.append(record); // the record IS durable; only the fsync "failed"
            }
            return Err(OperationError::Journal("injected journal append failure".into()));
        }
        self.inner.append(record)
    }

    fn read_all(&self) -> Result<Vec<JournalRecord>> {
        self.inner.read_all()
    }
}

/// The deterministic bytes the fake ELEVATED helper writes when it styles a privileged item — keyed
/// on the staged asset path so `plan` (pure) and `apply` (write) agree, exactly as the real port's
/// `expected_after_apply`-then-`SetIconLocation` pair does. A different scheme from `styled_bytes` so
/// a test can tell an elevated-styled item apart from a driver-styled one.
///
/// Its only consumers are the `not(windows)`-gated icon-ops tests, so it reads as dead on Windows
/// (the txn tests, which DO run on Windows, exercise the elevated crash path through the driver +
/// journal directly). `allow(dead_code)` keeps the Windows test build warning-clean.
#[allow(dead_code)]
pub fn elevated_styled(asset_path: &str) -> Vec<u8> {
    format!("STYLED-ELEV:{asset_path}").into_bytes()
}

/// A fake elevated desktop-item applier over a shared `World`, modelling `dm-elevated`'s all-or-nothing
/// batch: on `Applied` it writes the styled/original bytes for every item and records them; on
/// `Declined` (UAC cancel) or `Failed` it writes NOTHING (the real helper never wrote / LIFO-rolled
/// back its own writes), so the desktop is left untouched. `plan` derives each item's post-apply
/// fingerprint WITHOUT writing, so the operations layer can journal it before the batch runs.
///
/// Consumed only by the `not(windows)`-gated icon-ops tests, so `allow(dead_code)` keeps the Windows
/// test build clean (its trait impl still references the elevated types, so no unused imports).
#[allow(dead_code)]
pub struct FakeElevatedIconApplier {
    world: Rc<RefCell<World>>,
    outcome: RefCell<ElevatedOutcome>,
    applied: RefCell<Vec<String>>,
    restored: RefCell<Vec<String>>,
    /// Target paths the helper SKIPS on an `Applied` restore — models the real helper silently
    /// skipping an item whose live icon no longer matches (a user re-edit during the UAC prompt): it
    /// still exits 0, but leaves that target untouched.
    restore_skips: RefCell<Vec<String>>,
    /// The `(target_path, expect_icon)` CAS anchor the ops layer threaded into each applied item —
    /// the real helper compares this against the live location before writing, so a wrong value
    /// (e.g. a stale scan location on a re-apply) makes the real helper refuse (owner box
    /// 2026-07-17). Recorded so a test can assert which anchor the ops layer chose.
    apply_expects: RefCell<Vec<(String, String)>>,
}

#[allow(dead_code)]
impl FakeElevatedIconApplier {
    /// A helper that styles/reverts successfully (Applied).
    pub fn new(world: Rc<RefCell<World>>) -> Self {
        Self {
            world,
            outcome: RefCell::new(ElevatedOutcome::Applied),
            applied: RefCell::new(Vec::new()),
            restored: RefCell::new(Vec::new()),
            restore_skips: RefCell::new(Vec::new()),
            apply_expects: RefCell::new(Vec::new()),
        }
    }

    /// The `expect_icon` CAS anchor the ops layer threaded for `target_path` on the most recent
    /// apply, if any. A re-apply must anchor on the LEDGER's last-applied asset, not the stale scan.
    pub fn expect_for(&self, target_path: &str) -> Option<String> {
        self.apply_expects
            .borrow()
            .iter()
            .rev()
            .find(|(p, _)| p == target_path)
            .map(|(_, e)| e.clone())
    }

    /// Sets the outcome the next `apply`/`restore` returns (Declined models a UAC cancel; Failed a
    /// helper fault). A non-Applied outcome leaves the world untouched.
    pub fn set_outcome(&self, outcome: ElevatedOutcome) {
        *self.outcome.borrow_mut() = outcome;
    }

    /// Makes an `Applied` restore SKIP `path` (leave it untouched) — models the helper's CAS-skip of
    /// an item the user re-edited during the UAC prompt, which still returns exit 0.
    pub fn skip_restore(&self, path: &str) {
        self.restore_skips.borrow_mut().push(path.to_string());
    }

    /// The target paths the helper styled (for assertions).
    pub fn applied_paths(&self) -> Vec<String> {
        self.applied.borrow().clone()
    }

    /// The target paths the helper reverted (for assertions).
    pub fn restored_paths(&self) -> Vec<String> {
        self.restored.borrow().clone()
    }
}

impl ElevatedIconApplier for FakeElevatedIconApplier {
    fn plan(&self, items: &[ElevatedApplyItem]) -> PortResult<Vec<Fingerprint>> {
        Ok(items
            .iter()
            .map(|it| Fingerprint::of_bytes(&elevated_styled(&it.asset_path)))
            .collect())
    }

    fn apply(&self, items: &[ElevatedApplyItem]) -> PortResult<ElevatedOutcome> {
        // Record the CAS anchor the ops layer chose for every item, ALWAYS (even on a non-Applied
        // outcome) — a test asserts a re-apply anchors on the ledger asset, not the stale scan.
        for it in items {
            self.apply_expects
                .borrow_mut()
                .push((it.target.path.clone(), it.expect_icon.clone()));
        }
        let outcome = self.outcome.borrow().clone();
        if outcome == ElevatedOutcome::Applied {
            let mut world = self.world.borrow_mut();
            for it in items {
                world.put(&it.target.path, &elevated_styled(&it.asset_path));
                self.applied.borrow_mut().push(it.target.path.clone());
            }
        }
        Ok(outcome)
    }

    fn restore(&self, items: &[ElevatedRestoreItem]) -> PortResult<ElevatedOutcome> {
        let outcome = self.outcome.borrow().clone();
        if outcome == ElevatedOutcome::Applied {
            let mut world = self.world.borrow_mut();
            for it in items {
                // A skipped target is left exactly as it is (the helper's CAS-skip); the exit is still 0.
                if self.restore_skips.borrow().iter().any(|p| p == &it.target.path) {
                    continue;
                }
                world.put(&it.target.path, &it.original_bytes);
                self.restored.borrow_mut().push(it.target.path.clone());
            }
        }
        Ok(outcome)
    }
}

/// An asset store whose `put` always fails (simulates a full disk / permission error while
/// materializing the generated ICO).
pub struct FailingAssetStore;

impl AssetStore for FailingAssetStore {
    fn put(&self, _hash: &str, _bytes: &[u8]) -> PortResult<AssetRef> {
        Err(PortError::Io("injected asset write failure".into()))
    }

    fn put_empty_variant(&self, _primary: &AssetRef, _bytes: &[u8]) -> PortResult<AssetRef> {
        Err(PortError::Io("injected asset write failure".into()))
    }

    fn exists(&self, _asset: &AssetRef) -> PortResult<bool> {
        Ok(false)
    }

    fn gc(&self, _live: &[String]) -> PortResult<()> {
        Ok(())
    }
}
