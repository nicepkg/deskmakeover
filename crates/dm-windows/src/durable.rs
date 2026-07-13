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
    // Open a UNIQUE temp with create_new (O_EXCL): even though the name embeds the pid + a counter,
    // File::create would still FOLLOW a symlink a hostile process pre-placed at that (predictable)
    // name and truncate the link's target. O_EXCL refuses to follow/overwrite; on a collision (a
    // stale temp, or a hostile placement) we retry a fresh name. A non-collision error — notably a
    // missing parent directory — propagates, preserving P2-4 (fail loudly, never resurrect the tree).
    for _ in 0..1000 {
        let tmp = temp_sibling(target);
        match fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => {
                let result = finish_write(file, &tmp, target, bytes);
                if result.is_err() {
                    let _ = fs::remove_file(&tmp); // never leave a partial temp behind
                }
                return result;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io(e)),
        }
    }
    Err(PortError::Io("could not allocate a unique temp file for the atomic write".into()))
}

/// Finalizes a file a COM writer (`IPersistFile::Save`) has written to a temp path: flush its
/// bytes to disk, then atomically publish it over `target`. This gives the `.lnk` writers the same
/// durability + atomic publication as the plain-fs writers — a crash mid-Save can no longer tear
/// the live shortcut, and a clean commit is durable (P1-3). On any failure the temp is removed.
/// [WINDOWS-VERIFY] the flush + ReplaceFileW on NTFS.
pub fn finalize_saved(tmp: &str, target: &str) -> PortResult<()> {
    let (tmp, target) = (Path::new(tmp), Path::new(target));
    let result = flush_then_publish(tmp, target);
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

fn flush_then_publish(tmp: &Path, target: &Path) -> PortResult<()> {
    // A fresh write handle to the COM-written temp: FlushFileBuffers flushes the file regardless of
    // which handle wrote it. `write(true)` opens without truncating.
    fs::OpenOptions::new().write(true).open(tmp).map_err(io)?.sync_all().map_err(io)?;
    publish(tmp, target)?;
    sync_parent_dir(target)?;
    Ok(())
}

/// Writes `bytes` into the already-created (O_EXCL) temp `file`, flushes it, then atomically
/// publishes it over `target` and makes the namespace change durable. The temp is created by the
/// caller via create_new so a symlink can never be followed here; the parent dir is NOT created
/// (P2-4: a write into a user-deleted directory fails loudly, never resurrects the tree).
fn finish_write(mut file: fs::File, tmp: &Path, target: &Path, bytes: &[u8]) -> PortResult<()> {
    file.write_all(bytes).map_err(io)?;
    // Durability before the publish. FlushFileBuffers on Windows.
    file.sync_all().map_err(io)?;
    drop(file);
    publish(tmp, target)?;
    sync_parent_dir(target)?;
    Ok(())
}

/// fsync the directory that contains `target` so the create/rename is itself durable.
///
/// On POSIX a lost directory fsync means a crash can drop the namespace change even though the
/// file bytes were flushed — so we PROPAGATE its error, never swallow it (P1-#3). On Windows there
/// is no POSIX-style directory fsync; the namespace change's durability is carried by
/// `REPLACEFILE_WRITE_THROUGH` in [`publish`] (and, for a fresh-target rename, by the temp's
/// `FlushFileBuffers`), so this is a documented no-op there. [WINDOWS-VERIFY] the write-through path.
#[cfg(not(windows))]
fn sync_parent_dir(target: &Path) -> PortResult<()> {
    fs::File::open(target_dir(target)).map_err(io)?.sync_all().map_err(io)
}

#[cfg(windows)]
fn sync_parent_dir(_target: &Path) -> PortResult<()> {
    Ok(())
}

/// Atomically publishes `tmp` over `target`, preserving state a plain rename would destroy.
///
/// On Windows, when `target` already exists, `ReplaceFileW` carries the target's security
/// descriptor (DACL), alternate data streams (e.g. `Zone.Identifier`), and compression/encryption
/// attributes onto the replacement — `MoveFileEx(REPLACE_EXISTING)` would silently drop them, and
/// the restore anchors capture only main-stream bytes, so that loss is unrecoverable (P2-4). A
/// fresh target (nothing to preserve) is a plain rename.
///
/// Flags (P1-#3/P2-#4): `REPLACEFILE_WRITE_THROUGH` makes the replacement durable before the call
/// returns (the write-through barrier the journal fsync assumes). We deliberately do NOT pass
/// `REPLACEFILE_IGNORE_MERGE_ERRORS` — a failure to merge the target's ACL/metadata onto the
/// replacement must surface loudly, not silently succeed with lost security state.
///
/// Known limitation ([WINDOWS-VERIFY]): `ReplaceFileW` does not preserve **hard-link identity** —
/// other links to the original file are not redirected to the replacement. DeskMakeover styles
/// desktop items (`.lnk`/`.url`/`desktop.ini`/wrapper) that are not expected to be hard-linked, so
/// this is an accepted limitation rather than a preserved property.
#[cfg(windows)]
fn publish(tmp: &Path, target: &Path) -> PortResult<()> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        REPLACE_FILE_FLAGS,
    };

    // try_exists(), not exists() (audit F3): a metadata error must NOT read as "fresh target" and
    // select the MoveFileEx path that DROPS the existing file's DACL/ADS/compression — fail closed.
    let target_exists = target
        .try_exists()
        .map_err(|e| PortError::Io(format!("cannot stat {}: {e}", target.display())))?;
    if target_exists {
        // ReplaceFileW carries the target's DACL, alternate data streams, and compression/encryption
        // attributes onto the replacement (MoveFileEx would drop them, and the restore anchors
        // capture only main-stream bytes). We do NOT pass REPLACEFILE_WRITE_THROUGH — MS documents it
        // as "not supported" (a no-op), so relying on it was misleading; durability of the
        // same-volume replace rests on the temp's prior FlushFileBuffers + NTFS replace semantics.
        // [WINDOWS-VERIFY] whether that survives power loss; if not, committed-txn recovery must
        // re-verify + repair the live state (handoff §8a #2).
        // SAFETY: both paths are valid UTF-16; no buffers are retained past the call.
        unsafe {
            ReplaceFileW(
                &HSTRING::from(target.as_os_str()),
                &HSTRING::from(tmp.as_os_str()),
                windows::core::PCWSTR::null(),
                REPLACE_FILE_FLAGS(0),
                None,
                None,
            )
        }
        .map_err(|e| PortError::Io(e.to_string()))
    } else {
        // Fresh target: nothing to preserve, so a write-through move makes the namespace change
        // durable before we return — the barrier the journal fsync assumes (a plain rename left the
        // fresh write non-durable, so a cross-volume item's write could be lost after its txn
        // committed on another volume, poisoning CAS). [WINDOWS-VERIFY] the write-through on NTFS.
        // SAFETY: both paths are valid UTF-16; no buffers are retained past the call.
        unsafe {
            MoveFileExW(
                &HSTRING::from(tmp.as_os_str()),
                &HSTRING::from(target.as_os_str()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
        .map_err(|e| PortError::Io(e.to_string()))
    }
}

/// POSIX publish: `rename(2)` is the atomic primitive (no ADS/DACL to preserve).
#[cfg(not(windows))]
fn publish(tmp: &Path, target: &Path) -> PortResult<()> {
    fs::rename(tmp, target).map_err(io)
}

/// Creates a UNIQUE, empty temp sibling of `target` with `create_new` (O_EXCL) and returns its path,
/// for a COM writer (`IPersistFile::Save`) to save into before [`finalize_saved`] flushes + publishes
/// it. Pre-claiming the temp as a proven-regular file means Save can no longer create/truncate
/// THROUGH a symlink a hostile process pre-placed at the (predictable) temp name — it opens the
/// regular file we already own.
///
/// [WINDOWS-VERIFY] residual: a delete+symlink swap in the narrow window between this close and
/// Save's reopen-by-path is a same-user TOCTOU; fully closing it needs an `IPersistStream`-to-memory
/// save published through [`write_atomic`], deferred to the Windows box (handoff §8a #1).
pub fn claim_temp_for(target: &str) -> PortResult<String> {
    let target = Path::new(target);
    for _ in 0..1000 {
        let tmp = temp_sibling(target);
        match fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(_file) => return Ok(tmp.to_string_lossy().into_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io(e)),
        }
    }
    Err(PortError::Io("could not allocate a unique temp for the COM save".into()))
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

#[cfg(not(windows))]
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

    #[cfg(unix)]
    #[test]
    fn never_writes_through_a_symlinked_target() {
        use std::os::unix::fs::symlink;
        // A hostile symlink at the target must not let the write reach the user's file: the publish
        // rename replaces the link itself, and the O_EXCL temp is never a symlink either.
        let dir = tempfile::tempdir().unwrap();
        let victim = dir.path().join("victim.dat");
        fs::write(&victim, b"USER DATA").unwrap();
        let target = dir.path().join("shortcut.lnk");
        symlink(&victim, &target).unwrap();

        write_atomic(&target.to_string_lossy(), b"styled").unwrap();

        assert_eq!(fs::read(&victim).unwrap(), b"USER DATA", "the symlink target is untouched");
        assert_eq!(fs::read(&target).unwrap(), b"styled");
        assert!(!fs::symlink_metadata(&target).unwrap().file_type().is_symlink());
    }

    #[test]
    fn a_missing_parent_directory_fails_and_is_not_resurrected() {
        // P2-4: writing into a directory the user deleted must fail loudly, NOT recreate the tree.
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("deleted-by-user");
        let path = missing.join("file.lnk");
        let result = write_atomic(&path.to_string_lossy(), b"x");
        assert!(result.is_err(), "must not create a directory the caller says exists");
        assert!(!missing.exists(), "the deleted directory tree must NOT be resurrected");
        // No temp straggler in the (existing) grandparent either.
        let stragglers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(stragglers.is_empty(), "a failed write left temp files: {stragglers:?}");
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
