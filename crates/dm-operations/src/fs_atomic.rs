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

use crate::error::Result;

/// Writes `bytes` to `path` crash-atomically: create the parent dir, write a sibling temp file,
/// fsync it, then rename it over `path`. A crash mid-write leaves either the old bytes or the new
/// ones, never a partially written target.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = temp_sibling(path);
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    replace_atomically(&tmp, path)
}

/// The temp path a [`write_atomic`] uses: the target's full filename with `.tmp` appended, so it
/// lands in the SAME directory (a cross-directory rename is not atomic) and never collides with a
/// same-stem sibling of a different extension. `x.json` → `x.json.tmp`; `x` → `x.tmp`.
fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
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

    #[test]
    fn write_atomic_persists_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("data.json");
        write_atomic(&target, b"hello").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"hello");
        // The sibling temp file the rename consumed must be gone.
        assert!(!dir.path().join("data.json.tmp").exists());
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
    fn temp_sibling_appends_tmp_and_stays_in_the_same_dir() {
        let p = Path::new("/x/y/ledger.json");
        assert_eq!(temp_sibling(p), Path::new("/x/y/ledger.json.tmp"));
        // Extension-less targets still get a sibling temp in the same dir.
        assert_eq!(temp_sibling(Path::new("/x/y/data")), Path::new("/x/y/data.tmp"));
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
