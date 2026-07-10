//! In-memory fakes that stand in for the Windows platform ports, so the transaction driver and
//! recovery are exercised on the Mac host. The `World` is a virtual desktop (path → bytes); the
//! ports read/mutate it through interior mutability. `RecordingJournal` snapshots the world
//! after every durable append, which is what the kill-point battery replays.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dm_domain::{
    AssetRef, AssetStore, Fingerprint, IconApplier, ItemStateReader, ItemTarget, PortError,
    PortResult, RestoreAnchor,
};

use crate::error::Result;
use crate::txn::journal::{JournalRecord, JournalSink};

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

/// A reader/applier/asset-store bundle over one shared `World`.
#[derive(Clone)]
pub struct FakePlatform {
    world: Rc<RefCell<World>>,
    /// Paths whose anchor capture should fail (simulates `restore.captureError`).
    capture_fails: Rc<RefCell<HashSet<String>>>,
}

impl FakePlatform {
    pub fn new(world: Rc<RefCell<World>>) -> Self {
        Self { world, capture_fails: Rc::new(RefCell::new(HashSet::new())) }
    }

    pub fn fail_capture(&self, path: &str) {
        self.capture_fails.borrow_mut().insert(path.to_string());
    }
}

impl ItemStateReader for FakePlatform {
    fn read_fingerprint(&self, target: &ItemTarget) -> PortResult<Fingerprint> {
        match self.world.borrow().get(&target.path) {
            Some(bytes) => Ok(Fingerprint::of_bytes(&bytes)),
            None => Err(PortError::NotFound(target.path.clone())),
        }
    }

    fn capture_anchor(&self, target: &ItemTarget) -> PortResult<RestoreAnchor> {
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
    fn apply(&self, target: &ItemTarget, asset: &AssetRef) -> PortResult<()> {
        let mut world = self.world.borrow_mut();
        if world.apply_fails.contains(&target.path) {
            return Err(PortError::Com(format!("apply failed for {}", target.path)));
        }
        world.put(&target.path, &styled_bytes(&asset.hash));
        Ok(())
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
        Ok(AssetRef::new(hash, format!("assets/{hash}.ico")))
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
