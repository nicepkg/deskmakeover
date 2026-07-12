//! Durable store for THE pre-first-apply wallpaper snapshot — the single artifact
//! that lets `restore('all')` return the desktop to its true pre-DeskMakeover state.
//!
//! One JSON file under the app-data dir, written atomically (temp + rename, same
//! discipline as the txn journal). Fail-closed on corruption: an unreadable file
//! surfaces [`OperationError::CorruptSnapshot`], never `None` — reading corrupt as
//! "no backup yet" would let the snapshot-once guard re-capture the already-styled
//! desktop over the user's true original.

use std::fs;
use std::path::{Path, PathBuf};

use dm_domain::WallpaperSnapshot;

use crate::error::{OperationError, Result};

pub struct SnapshotStore {
    path: PathBuf,
}

impl SnapshotStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `Ok(None)` = no snapshot has ever been captured (first apply still pending).
    /// A present-but-unparseable file is [`OperationError::CorruptSnapshot`] — fail
    /// closed, see the module note.
    pub fn load(&self) -> Result<Option<WallpaperSnapshot>> {
        let bytes = match fs::read(&self.path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| OperationError::CorruptSnapshot)
    }

    /// Atomic write (temp + fsync + rename, with the Windows sharing-violation retry) through the
    /// crate's shared [`crate::fs_atomic::write_atomic`]. A crash mid-save leaves either the old
    /// snapshot or the new one, never a torn file.
    pub fn save(&self, snapshot: &WallpaperSnapshot) -> Result<()> {
        crate::fs_atomic::write_atomic(&self.path, &serde_json::to_vec_pretty(snapshot)?)
    }

    /// Removes the snapshot (after a successful whole-desktop restore). Missing is ok.
    pub fn clear(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use dm_domain::MonitorWallpaper;

    use super::*;

    fn snap() -> WallpaperSnapshot {
        WallpaperSnapshot {
            background_color: 0x00332211,
            position: 4,
            slideshow_active: false,
            monitors: vec![
                MonitorWallpaper { monitor_id: "m0".into(), image: Some("C:/orig.jpg".into()) },
                MonitorWallpaper { monitor_id: "m1".into(), image: None },
            ],
        }
    }

    #[test]
    fn missing_file_loads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(dir.path().join("snap.json"));
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(dir.path().join("snap.json"));
        store.save(&snap()).unwrap();
        assert_eq!(store.load().unwrap(), Some(snap()));
        // No stray temp file of ANY name survives the rename (write_atomic uses a unique
        // `.<name>.<pid>.<n>.tmp` sibling, so a fixed-name check would be a false pass).
        let strays: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("tmp"))
            .collect();
        assert!(strays.is_empty(), "stray temp: {strays:?}");
    }

    #[test]
    fn corrupt_file_fails_closed_never_reads_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snap.json");
        fs::write(&path, b"{ not json").unwrap();
        let store = SnapshotStore::new(&path);
        assert!(matches!(store.load(), Err(OperationError::CorruptSnapshot)));
    }

    #[test]
    fn save_creates_missing_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(dir.path().join("a/b/snap.json"));
        store.save(&snap()).unwrap();
        assert!(store.load().unwrap().is_some());
    }

    #[test]
    fn clear_removes_and_tolerates_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(dir.path().join("snap.json"));
        store.clear().unwrap(); // nothing there yet — still ok
        store.save(&snap()).unwrap();
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }
}
