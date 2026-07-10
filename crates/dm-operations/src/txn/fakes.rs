//! In-memory fakes that stand in for the Windows platform ports, so the transaction driver and
//! recovery are exercised on the Mac host. The `World` is a virtual desktop (path → bytes); the
//! ports read/mutate it through interior mutability. `RecordingJournal` snapshots the world
//! after every durable append, which is what the kill-point battery replays.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dm_domain::{
    ApplyAssets, AssetRef, AssetStore, Fingerprint, IconApplier, ItemStateReader, ItemTarget,
    PortError, PortResult, RestoreAnchor,
};

use crate::error::{OperationError, Result};
use crate::txn::journal::{JournalRecord, JournalSink, VecJournal};

/// The deterministic "styled" bytes an apply writes for a given asset hash — models the icon
/// location swap changing the item's file bytes.
pub fn styled_bytes(asset_hash: &str) -> Vec<u8> {
    format!("STYLED:{asset_hash}").into_bytes()
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
}

impl FakePlatform {
    pub fn new(world: Rc<RefCell<World>>) -> Self {
        Self {
            world,
            capture_fails: Rc::new(RefCell::new(HashSet::new())),
            capture_errors: Rc::new(RefCell::new(HashSet::new())),
            materialized: Rc::new(RefCell::new(HashSet::new())),
            empty_variant_vanishes: Rc::new(RefCell::new(false)),
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
        let mut world = self.world.borrow_mut();
        if world.apply_fails.contains(&target.path) {
            return Err(PortError::Com(format!("apply failed for {}", target.path)));
        }
        // The fingerprint the applier PROMISES for this asset, derived from the asset itself and
        // independent of whether the underlying write actually lands — this is exactly the
        // non-tautological derivation P1-1 requires of the real applier. The driver re-reads the
        // live state and rejects the apply unless the two agree (P1-4).
        let promised = Fingerprint::of_bytes(&styled_bytes(&assets.primary.hash));
        if world.noop_apply.contains(&target.path) {
            return Ok(promised); // "succeeds" but writes nothing → driver's re-read won't match
        }
        if world.wrong_write.contains(&target.path) {
            // Lands a different asset than requested while still promising the requested one.
            world.put(&target.path, &styled_bytes("stale-other-asset"));
            return Ok(promised);
        }
        world.put(&target.path, &styled_bytes(&assets.primary.hash));
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
