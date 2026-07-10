//! Durable, atomic file replacement for the user-file writers.
//!
//! The transaction journal is fsync'd before each mutation, but the styling writes themselves were
//! plain `fs::write` — a power loss after a clean commit silently lost the styling (and poisoned
//! CAS: the ledger says styled, the disk is original), and a crash mid-write could tear a
//! `.lnk`/`.url`/`desktop.ini` (P1-9). This helper gives those writes the same durability the
//! journal has: write a sibling temp file, fsync it, then rename it over the target, so a crash
//! leaves either the old file or the fully-written new one — never a torn write.
//!
//! The temp+rename logic is unit-tested on the Mac host; the `FlushFileBuffers`/`MoveFileEx`
//! behaviour on real NTFS is `[WINDOWS-VERIFY]`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dm_domain::{PortError, PortResult};

/// Durably and atomically writes `bytes` to `path`. A sibling temp file is written and fsync'd,
/// then atomically renamed over the target. On any failure the temp is cleaned up so a partial
/// write never lingers. On POSIX `rename(2)` is atomic; `[WINDOWS-VERIFY]` the
/// `MoveFileEx(REPLACE_EXISTING)` + `FlushFileBuffers` path.
pub fn write_atomic(path: &str, bytes: &[u8]) -> PortResult<()> {
    let target = Path::new(path);
    let tmp = temp_sibling(target);
    let result = write_then_rename(&tmp, target, bytes);
    if result.is_err() {
        let _ = fs::remove_file(&tmp); // never leave a partial temp behind
    }
    result
}

fn write_then_rename(tmp: &Path, target: &Path, bytes: &[u8]) -> PortResult<()> {
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(io)?;
        }
    }
    {
        let mut file = fs::File::create(tmp).map_err(io)?;
        file.write_all(bytes).map_err(io)?;
        // Durability before the rename that publishes the bytes. FlushFileBuffers on Windows.
        file.sync_all().map_err(io)?;
    }
    fs::rename(tmp, target).map_err(io)?;
    // fsync the directory so the rename itself survives a crash (POSIX). Best-effort where a
    // directory can't be opened as a file.
    if let Ok(handle) = fs::File::open(target_dir(target)) {
        let _ = handle.sync_all();
    }
    Ok(())
}

/// A process-unique sibling temp path in the target's own directory, so the rename is
/// same-filesystem (hence atomic) and concurrent writers never collide.
fn temp_sibling(target: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp_name = format!(".{name}.dm-{}-{n}.tmp", std::process::id());
    match target.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.join(tmp_name),
        _ => PathBuf::from(tmp_name),
    }
}

fn target_dir(target: &Path) -> &Path {
    match target.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

fn io(e: std::io::Error) -> PortError {
    PortError::Io(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_new_file_with_exact_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shortcut.lnk");
        write_atomic(&path.to_string_lossy(), b"styled bytes").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"styled bytes");
    }

    #[test]
    fn overwrites_an_existing_file_and_leaves_no_temp_residue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("desktop.ini");
        fs::write(&path, b"OLD content").unwrap();
        write_atomic(&path.to_string_lossy(), b"NEW content").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"NEW content");
        // The temp+rename must not leave a sibling `.tmp` behind.
        let stragglers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(stragglers.is_empty(), "atomic replace left temp files: {stragglers:?}");
    }

    #[cfg(unix)]
    #[test]
    fn replacement_is_atomic_via_a_sibling_temp_not_an_in_place_truncate() {
        // The defining anti-torn property: the write goes to a NEW inode and is renamed over the
        // target, so a reader observing the target sees either the whole old file or the whole new
        // one — never a truncated in-place write. We assert the target's inode changes (a plain
        // `fs::write` would keep the same inode and be observably truncated mid-write).
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.url");
        fs::write(&path, b"original").unwrap();
        let ino_before = fs::metadata(&path).unwrap().ino();
        write_atomic(&path.to_string_lossy(), b"replaced").unwrap();
        let ino_after = fs::metadata(&path).unwrap().ino();
        assert_ne!(ino_before, ino_after, "atomic replace must publish a fresh inode, not truncate in place");
        assert_eq!(fs::read(&path).unwrap(), b"replaced");
    }

    #[test]
    fn creates_missing_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/sub/file.lnk");
        write_atomic(&path.to_string_lossy(), b"x").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"x");
    }

    #[test]
    fn a_failed_write_cleans_up_its_temp_and_leaves_the_target_intact() {
        // Force the rename to fail by making the target an existing directory. The original state
        // (the directory) must be untouched and no temp file may linger.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("occupied");
        fs::create_dir(&target).unwrap();
        let err = write_atomic(&target.to_string_lossy(), b"data");
        assert!(err.is_err(), "renaming a file over a directory must fail");
        assert!(target.is_dir(), "the pre-existing target is untouched by a failed write");
        let stragglers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(stragglers.is_empty(), "a failed write left temp files: {stragglers:?}");
    }
}
