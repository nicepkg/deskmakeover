//! Wallpaper operations (M6-WIRE A3, owner ruling D1): thin orchestration over the
//! wallpaper ports — the durable pre-first-apply snapshot (snapshot-once), baked-PNG
//! materialization (base64 → app-data file), and apply/restore driving. NO reconcile,
//! NO draft-look persistence, NO `WallpaperStateDto` assembly — those are frontend.
//!
//! Ordering invariant (the data-loss guard): on the FIRST apply the original desktop
//! is captured and durably saved BEFORE any `set` mutates it. Any failure to capture
//! or persist aborts the apply — fail closed, the desktop stays untouched.

mod decode;
mod snapshot_store;

use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use base64::Engine;
use dm_domain::WallpaperApplier;

use crate::error::{OperationError, Result};

pub use decode::RustImageDecoder;
pub use snapshot_store::SnapshotStore;

const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Outcome of a mutating wallpaper op; the command layer maps this onto the thin
/// `WallpaperResultDto` (ok / toast / hasBackup).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallpaperOutcome {
    /// Whether the pre-first-apply snapshot exists AFTER this op.
    pub has_backup: bool,
}

/// The thin apply/restore driver. Owns no state — the snapshot store and baked-file
/// dir are paths under the app-data dir, the applier is the platform port.
pub struct WallpaperOps<'a> {
    applier: &'a dyn WallpaperApplier,
    store: &'a SnapshotStore,
    baked_dir: PathBuf,
}

impl<'a> WallpaperOps<'a> {
    pub fn new(
        applier: &'a dyn WallpaperApplier,
        store: &'a SnapshotStore,
        baked_dir: impl Into<PathBuf>,
    ) -> Self {
        Self { applier, store, baked_dir: baked_dir.into() }
    }

    /// Applies a baked wallpaper PNG to one monitor.
    ///
    /// Order is load-bearing: ① decode + validate the payload (fail fast, desktop
    /// untouched) → ② snapshot-once (capture + durably save the original BEFORE the
    /// first mutation; abort on any failure) → ③ materialize the PNG under the baked
    /// dir → ④ `set`. Earlier baked files for the same monitor are pruned best-effort
    /// afterwards (the new file name is content-hashed, so re-applies always change
    /// the path and `SetWallpaper` never sees a stale-path no-op).
    pub fn apply_baked(&self, monitor_id: &str, png_base64: &str) -> Result<WallpaperOutcome> {
        // ① decode + validate before anything else touches disk or desktop.
        let png = base64::engine::general_purpose::STANDARD
            .decode(png_base64)
            .map_err(|e| OperationError::InvalidPayload(format!("base64: {e}")))?;
        if png.len() < PNG_MAGIC.len() || png[..PNG_MAGIC.len()] != PNG_MAGIC {
            return Err(OperationError::InvalidPayload("not a PNG payload".into()));
        }

        // ② snapshot-once: the ONLY point in the system allowed to create the
        // pre-first-apply snapshot. A corrupt existing file fails closed here
        // (CorruptSnapshot from load), which also aborts the apply.
        if self.store.load()?.is_none() {
            let original = self.applier.capture()?;
            self.store.save(&original)?;
        }

        // ③ materialize the baked PNG (atomic temp + rename).
        let path = self.baked_path(monitor_id, &png);
        fs::create_dir_all(&self.baked_dir)?;
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, &png)?;
        fs::rename(&tmp, &path)?;

        // ④ point the monitor at it.
        self.applier.set(monitor_id, &path.to_string_lossy())?;

        self.prune_stale(monitor_id, &path);
        Ok(WallpaperOutcome { has_backup: true })
    }

    /// Restores from the pre-first-apply snapshot.
    ///
    /// `'all'` reverts the whole desktop (background colour + position + every
    /// monitor) and then clears the snapshot — the desktop is back to its true
    /// original, so the backup has served its purpose and the next apply captures
    /// fresh. A single `monitor_id` restores only that monitor's image ([WINDOWS-
    /// VERIFY]: a `None` image — solid colour at capture — restores via an empty
    /// path clear) and KEEPS the snapshot for the remaining monitors / a later 'all'.
    pub fn restore(&self, monitor_id: &str) -> Result<WallpaperOutcome> {
        let snapshot = self.store.load()?.ok_or(OperationError::NothingToRestore)?;
        if monitor_id == "all" {
            self.applier.restore(&snapshot)?;
            self.store.clear()?;
            return Ok(WallpaperOutcome { has_backup: false });
        }
        let monitor = snapshot
            .monitors
            .iter()
            .find(|m| m.monitor_id == monitor_id)
            .ok_or_else(|| OperationError::UnknownMonitor(monitor_id.into()))?;
        match &monitor.image {
            Some(original) => self.applier.set(monitor_id, original)?,
            None => self.applier.set(monitor_id, "")?,
        }
        Ok(WallpaperOutcome { has_backup: true })
    }

    /// `baked-<sanitized-monitor>-<content-hash>.png` — the hash keys the CONTENT, so
    /// a re-apply with new pixels always lands at a fresh path (cache-bust for both
    /// `SetWallpaper` and the `dmwallpaper://` protocol's `rev`).
    fn baked_path(&self, monitor_id: &str, png: &[u8]) -> PathBuf {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        png.hash(&mut hasher);
        let digest = hasher.finish();
        self.baked_dir.join(format!("baked-{}-{digest:016x}.png", sanitize(monitor_id)))
    }

    /// Best-effort removal of older baked files for the same monitor; failure never
    /// fails the apply (the desktop already points at the new file).
    fn prune_stale(&self, monitor_id: &str, keep: &Path) {
        let prefix = format!("baked-{}-", sanitize(monitor_id));
        let Ok(entries) = fs::read_dir(&self.baked_dir) else { return };
        for entry in entries.flatten() {
            let p = entry.path();
            if p != keep
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".png"))
            {
                let _ = fs::remove_file(p);
            }
        }
    }
}

/// Windows device paths (`\\?\DISPLAY#...`) carry characters illegal in file names;
/// keep alphanumerics, map the rest to `_`.
fn sanitize(monitor_id: &str) -> String {
    monitor_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use base64::Engine;
    use dm_domain::{MonitorWallpaper, PortError, PortResult, WallpaperSnapshot};

    use super::*;

    /// Recording fake: logs every port call in order; capture/set can be armed to fail.
    #[derive(Default)]
    struct FakeApplier {
        calls: RefCell<Vec<String>>,
        fail_capture: bool,
        fail_set: bool,
    }

    impl FakeApplier {
        fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    fn original() -> WallpaperSnapshot {
        WallpaperSnapshot {
            background_color: 0x00AABBCC,
            position: 4,
            slideshow_active: false,
            monitors: vec![
                MonitorWallpaper { monitor_id: "m0".into(), image: Some("C:/orig0.jpg".into()) },
                MonitorWallpaper { monitor_id: "m1".into(), image: None },
            ],
        }
    }

    impl WallpaperApplier for FakeApplier {
        fn capture(&self) -> PortResult<WallpaperSnapshot> {
            self.calls.borrow_mut().push("capture".into());
            if self.fail_capture {
                return Err(PortError::Com("capture boom".into()));
            }
            Ok(original())
        }
        fn set(&self, monitor_id: &str, image_path: &str) -> PortResult<()> {
            self.calls.borrow_mut().push(format!("set {monitor_id} {image_path}"));
            if self.fail_set {
                return Err(PortError::Com("set boom".into()));
            }
            Ok(())
        }
        fn restore(&self, snapshot: &WallpaperSnapshot) -> PortResult<()> {
            self.calls.borrow_mut().push(format!("restore {} monitors", snapshot.monitors.len()));
            Ok(())
        }
    }

    fn png_b64() -> String {
        let mut bytes = PNG_MAGIC.to_vec();
        bytes.extend_from_slice(b"fake body");
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    struct Rig {
        _dir: tempfile::TempDir,
        store: SnapshotStore,
        baked: PathBuf,
    }

    fn rig() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(dir.path().join("snapshot.json"));
        let baked = dir.path().join("baked");
        Rig { _dir: dir, store, baked }
    }

    #[test]
    fn first_apply_captures_and_saves_before_set() {
        let r = rig();
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        let out = ops.apply_baked("m0", &png_b64()).unwrap();
        assert!(out.has_backup);
        let calls = fake.calls();
        assert_eq!(calls[0], "capture", "capture must precede set: {calls:?}");
        assert!(calls[1].starts_with("set m0 "), "{calls:?}");
        assert_eq!(r.store.load().unwrap(), Some(original()));
    }

    #[test]
    fn second_apply_never_recaptures() {
        let r = rig();
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        ops.apply_baked("m0", &png_b64()).unwrap();
        ops.apply_baked("m1", &png_b64()).unwrap();
        let captures = fake.calls().iter().filter(|c| *c == "capture").count();
        assert_eq!(captures, 1, "snapshot-once violated: {:?}", fake.calls());
    }

    #[test]
    fn capture_failure_aborts_before_any_mutation() {
        let r = rig();
        let fake = FakeApplier { fail_capture: true, ..Default::default() };
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        assert!(ops.apply_baked("m0", &png_b64()).is_err());
        assert!(!fake.calls().iter().any(|c| c.starts_with("set")), "desktop was mutated");
        assert!(r.store.load().unwrap().is_none(), "no partial snapshot may persist");
    }

    #[test]
    fn snapshot_save_failure_aborts_before_set() {
        let dir = tempfile::tempdir().unwrap();
        // Parent of the snapshot path is a FILE — save's create_dir_all/rename must fail.
        let blocker = dir.path().join("blocker");
        fs::write(&blocker, b"x").unwrap();
        let store = SnapshotStore::new(blocker.join("snap.json"));
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &store, dir.path().join("baked"));
        assert!(ops.apply_baked("m0", &png_b64()).is_err());
        assert!(!fake.calls().iter().any(|c| c.starts_with("set")), "desktop was mutated");
    }

    #[test]
    fn corrupt_snapshot_fails_the_apply_closed() {
        let r = rig();
        fs::create_dir_all(r.store.path().parent().unwrap()).unwrap();
        fs::write(r.store.path(), b"{ torn").unwrap();
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        let err = ops.apply_baked("m0", &png_b64()).unwrap_err();
        assert!(matches!(err, OperationError::CorruptSnapshot));
        // Fail closed: NOTHING ran — no re-capture over the styled desktop, no set.
        assert!(fake.calls().is_empty(), "{:?}", fake.calls());
    }

    #[test]
    fn invalid_base64_and_non_png_fail_fast() {
        let r = rig();
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        assert!(matches!(
            ops.apply_baked("m0", "!!not-base64!!"),
            Err(OperationError::InvalidPayload(_))
        ));
        let jpeg = base64::engine::general_purpose::STANDARD.encode([0xFF, 0xD8, 0xFF, 0xE0]);
        assert!(matches!(ops.apply_baked("m0", &jpeg), Err(OperationError::InvalidPayload(_))));
        assert!(fake.calls().is_empty(), "payload failures must not touch any port");
    }

    #[test]
    fn baked_file_lands_with_sanitized_content_hashed_name_and_stale_is_pruned() {
        let r = rig();
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        ops.apply_baked("\\\\?\\DISPLAY#MOCK#0", &png_b64()).unwrap();
        let first: Vec<_> = fs::read_dir(&r.baked).unwrap().flatten().collect();
        assert_eq!(first.len(), 1);
        let name = first[0].file_name().to_string_lossy().into_owned();
        assert!(name.starts_with("baked-____DISPLAY_MOCK_0-"), "{name}");
        assert!(name.ends_with(".png"));

        // Different content → different path; the older file is pruned.
        let mut other = PNG_MAGIC.to_vec();
        other.extend_from_slice(b"different body");
        let other64 = base64::engine::general_purpose::STANDARD.encode(other);
        ops.apply_baked("\\\\?\\DISPLAY#MOCK#0", &other64).unwrap();
        let after: Vec<_> = fs::read_dir(&r.baked).unwrap().flatten().collect();
        assert_eq!(after.len(), 1, "stale baked file not pruned");
        assert_ne!(after[0].file_name(), first[0].file_name());
    }

    #[test]
    fn restore_all_reverts_clears_and_reports_no_backup() {
        let r = rig();
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        ops.apply_baked("m0", &png_b64()).unwrap();
        let out = ops.restore("all").unwrap();
        assert!(!out.has_backup);
        assert!(fake.calls().iter().any(|c| c == "restore 2 monitors"));
        assert!(r.store.load().unwrap().is_none(), "snapshot must clear after restore-all");
    }

    #[test]
    fn restore_single_monitor_keeps_snapshot() {
        let r = rig();
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        ops.apply_baked("m0", &png_b64()).unwrap();
        let out = ops.restore("m0").unwrap();
        assert!(out.has_backup);
        assert!(fake.calls().contains(&"set m0 C:/orig0.jpg".to_string()));
        assert!(r.store.load().unwrap().is_some(), "snapshot must survive per-monitor restore");
    }

    #[test]
    fn restore_solid_colour_monitor_clears_via_empty_path() {
        let r = rig();
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        ops.apply_baked("m1", &png_b64()).unwrap();
        ops.restore("m1").unwrap();
        assert!(fake.calls().contains(&"set m1 ".to_string()));
    }

    #[test]
    fn restore_without_snapshot_is_a_typed_error() {
        let r = rig();
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        assert!(matches!(ops.restore("all"), Err(OperationError::NothingToRestore)));
    }

    #[test]
    fn restore_unknown_monitor_is_a_typed_error() {
        let r = rig();
        let fake = FakeApplier::default();
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        ops.apply_baked("m0", &png_b64()).unwrap();
        assert!(matches!(
            ops.restore("ghost"),
            Err(OperationError::UnknownMonitor(m)) if m == "ghost"
        ));
    }

    #[test]
    fn set_failure_after_snapshot_leaves_backup_intact() {
        let r = rig();
        let fake = FakeApplier { fail_set: true, ..Default::default() };
        let ops = WallpaperOps::new(&fake, &r.store, &r.baked);
        assert!(ops.apply_baked("m0", &png_b64()).is_err());
        // The original is already durably saved — the failed set loses nothing.
        assert_eq!(r.store.load().unwrap(), Some(original()));
    }
}
