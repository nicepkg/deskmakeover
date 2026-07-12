//! Crash-atomic file replacement, shared by every durable store in this crate.
//!
//! Three stores need the identical discipline — write a temp file in the target's directory,
//! fsync it, then atomically rename it over the target — so a crash mid-write always leaves
//! either the old bytes or the new bytes, never a torn file: the active ledger
//! (`ledger/store.rs`), the pre-first-apply wallpaper snapshot (`wallpaper/snapshot_store.rs`),
//! and the content-addressed asset store (`txn/asset_store.rs`). The Windows sharing-violation
//! retry (an indexer / AV / backup agent momentarily holding the target open) lives here once.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{OperationError, Result};

/// Writes `bytes` to `path` crash-atomically: create the parent dir, write a UNIQUE sibling temp
/// file with `O_EXCL` (so a pre-placed symlink can never hijack the write and two writers never
/// share a temp inode), fsync it, rename it over `path`, then fsync the parent directory so the
/// rename's entry is itself durable. A crash mid-write leaves either the old bytes or the new ones,
/// never a partially written or lost target. A partial temp is cleaned up on any failure.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)?;
    }
    let (tmp, mut file) = create_temp(path)?;
    let written = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    drop(file);
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = replace_atomically(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    // The rename's directory entry is durable only after the PARENT dir is fsync'd (POSIX). Without
    // this a crash right after this returns can lose the rename entirely, even though the file's own
    // bytes were fsync'd. Best-effort: not all platforms/filesystems support directory fsync.
    if let Some(parent) = parent {
        fsync_dir(parent);
    }
    Ok(())
}

/// Creates a fresh, uniquely-named temp file in `path`'s directory and returns it with its handle.
/// The name embeds the pid + a process-global counter and is opened with `create_new` (`O_EXCL`),
/// which (a) fails rather than follows if a symlink already sits at that name — closing the
/// classic "pre-place `<target>.tmp` as a symlink to a user file and let the writer truncate it"
/// hole — and (b) guarantees two concurrent writers never collide on one temp inode.
fn create_temp(path: &Path) -> Result<(PathBuf, fs::File)> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty());
    let base = path.file_name().and_then(|n| n.to_str()).unwrap_or("dm-atomic");
    let pid = std::process::id();
    for _ in 0..1000 {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!(".{base}.{pid}.{n}.tmp");
        let candidate = match dir {
            Some(d) => d.join(&name),
            None => PathBuf::from(&name),
        };
        match fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(file) => return Ok((candidate, file)),
            // A stale temp / a hostile pre-placed entry (incl. a symlink) at this name — try the next.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(OperationError::Io("could not allocate a unique temp file name".into()))
}

/// Fsyncs a directory so a just-committed rename's entry is durable (POSIX). Best-effort: opening +
/// syncing a directory is not portable (notably Windows), and the file data is already fsync'd, so a
/// failure here degrades durability, not correctness.
fn fsync_dir(dir: &Path) {
    #[cfg(unix)]
    {
        if let Ok(handle) = fs::File::open(dir) {
            let _ = handle.sync_all();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
    }
}

/// Atomically replaces `target` with `tmp`, retrying transient Windows sharing violations.
///
/// On Windows a rename over a file an indexer / anti-virus / backup agent momentarily holds open
/// returns a sharing violation (or access-denied), which would fail an otherwise-valid commit;
/// those locks are transient, so we retry with a short backoff. POSIX `rename(2)` already tolerates
/// open readers, so [`is_transient_share_violation`] is always false there and the first attempt
/// succeeds. [WINDOWS-VERIFY] the sharing-violation retry against a real AV/indexer.
pub fn replace_atomically(tmp: &Path, target: &Path) -> Result<()> {
    const MAX_RETRIES: u32 = 10;
    const BACKOFF: std::time::Duration = std::time::Duration::from_millis(20);
    let mut attempt = 0;
    loop {
        match fs::rename(tmp, target) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < MAX_RETRIES && is_transient_share_violation(&e) => {
                attempt += 1;
                std::thread::sleep(BACKOFF);
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Whether an `fs::rename` error is a transient Windows sharing conflict worth retrying:
/// `ERROR_SHARING_VIOLATION` (32) or `ERROR_ACCESS_DENIED` (5). Always false off Windows, so POSIX
/// makes exactly one rename attempt.
pub fn is_transient_share_violation(e: &std::io::Error) -> bool {
    #[cfg(windows)]
    {
        matches!(e.raw_os_error(), Some(32) | Some(5))
    }
    #[cfg(not(windows))]
    {
        let _ = e;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_files(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("tmp"))
            .collect()
    }

    #[test]
    fn write_atomic_persists_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("data.json");
        write_atomic(&target, b"hello").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"hello");
        // The unique temp the rename consumed must be gone — no `.tmp` litter of any name.
        assert!(temp_files(dir.path()).is_empty(), "stray temp: {:?}", temp_files(dir.path()));
    }

    #[test]
    fn write_atomic_overwrites_and_creates_missing_parents() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("a/b/c/data.bin");
        write_atomic(&target, b"first").unwrap();
        write_atomic(&target, b"second").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"second");
    }

    #[test]
    fn consecutive_writes_use_distinct_temp_names() {
        // The temp name is per-call unique (pid + counter), so two writes to the same target never
        // reuse one temp inode — guarding concurrent writers.
        let dir = tempfile::tempdir().unwrap();
        let (t1, f1) = create_temp(&dir.path().join("x.json")).unwrap();
        let (t2, f2) = create_temp(&dir.path().join("x.json")).unwrap();
        drop((f1, f2));
        assert_ne!(t1, t2, "each create_temp must yield a fresh name");
        assert!(t1.exists() && t2.exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_never_writes_through_a_symlinked_target() {
        use std::os::unix::fs::symlink;
        // A hostile symlink at the TARGET path must not let the write reach the user's file: rename
        // replaces the link itself, never its referent.
        let dir = tempfile::tempdir().unwrap();
        let user_file = dir.path().join("precious.doc");
        fs::write(&user_file, b"USER DATA").unwrap();
        let target = dir.path().join("ledger.json");
        symlink(&user_file, &target).unwrap();

        write_atomic(&target, b"ours").unwrap();

        assert_eq!(fs::read(&user_file).unwrap(), b"USER DATA", "the user's file must be untouched");
        assert_eq!(fs::read(&target).unwrap(), b"ours", "target is now our regular file, not a link");
        assert!(!fs::symlink_metadata(&target).unwrap().file_type().is_symlink());
    }

    #[test]
    fn replace_atomically_overwrites_an_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("ledger.json");
        let tmp = dir.path().join("ledger.json.tmp");
        fs::write(&target, b"old").unwrap();
        fs::write(&tmp, b"new").unwrap();
        replace_atomically(&tmp, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert!(!tmp.exists()); // the rename consumed the temp file
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_nothing_is_a_transient_share_violation() {
        // POSIX rename tolerates open readers, so the retry loop must make exactly one attempt.
        let e = std::io::Error::from_raw_os_error(32);
        assert!(!is_transient_share_violation(&e));
    }
}
