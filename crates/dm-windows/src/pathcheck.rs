//! Host-tested path-existence checks that PROPAGATE access/metadata errors instead of collapsing
//! them to "absent".
//!
//! `Path::exists()` / `Path::is_dir()` return `false` on ANY error — including a permission-denied
//! or unreadable parent — so an existing-but-unreadable item would read as `NotFound`, and the
//! transaction driver would then report a benign per-item conflict, masking a compromised restore
//! path (P2, wave-2R). These wrappers use `try_exists` / `metadata` so a real metadata error
//! surfaces as `Io` while a genuine absence stays `NotFound`. Pure `std::fs` logic, unit-tested on
//! the Mac host (the underlying Win32 error mapping is `[WINDOWS-VERIFY]`, but the exists-vs-error
//! distinction is host logic).

use std::path::Path;

use dm_domain::{PortError, PortResult};

/// `Ok` when `path` exists, `NotFound` when it is genuinely absent, `Io` when its existence cannot
/// be determined (permission denied, I/O error) — never a silent "absent" for an unreadable path.
pub fn require_exists(path: &str) -> PortResult<()> {
    match Path::new(path).try_exists() {
        Ok(true) => Ok(()),
        Ok(false) => Err(PortError::NotFound(path.to_string())),
        Err(e) => Err(PortError::Io(e.to_string())),
    }
}

/// Whether `path` exists, propagating a metadata error rather than reporting `false` (P2).
pub fn path_exists(path: &str) -> PortResult<bool> {
    Path::new(path).try_exists().map_err(|e| PortError::Io(e.to_string()))
}

/// `Ok` when `path` is a directory, `NotFound` when it is absent or not a directory, `Io` when its
/// metadata cannot be read (permission denied) — `is_dir()` would report all of these as `false`.
pub fn require_dir(path: &str) -> PortResult<()> {
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => Ok(()),
        Ok(_) => Err(PortError::NotFound(path.to_string())), // exists but not a directory → skip
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(PortError::NotFound(path.to_string())),
        Err(e) => Err(PortError::Io(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_exists_reports_present_absent_distinctly() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("here.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(require_exists(&file.to_string_lossy()).is_ok());
        let missing = dir.path().join("gone.txt");
        assert!(matches!(require_exists(&missing.to_string_lossy()), Err(PortError::NotFound(_))));
    }

    #[test]
    fn require_dir_distinguishes_dir_file_and_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(require_dir(&dir.path().to_string_lossy()).is_ok());
        let file = dir.path().join("f.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(matches!(require_dir(&file.to_string_lossy()), Err(PortError::NotFound(_))));
        let missing = dir.path().join("nope");
        assert!(matches!(require_dir(&missing.to_string_lossy()), Err(PortError::NotFound(_))));
    }

    #[test]
    fn path_exists_is_true_false_without_error_on_a_readable_parent() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(path_exists(&file.to_string_lossy()).unwrap(), true);
        assert_eq!(path_exists(&dir.path().join("absent").to_string_lossy()).unwrap(), false);
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_parent_propagates_io_not_a_false_absence() {
        // The whole point of P2: under a permission-denied parent, try_exists()/metadata() error out
        // while exists()/is_dir() would report `false`. We must surface Io, never a silent NotFound.
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        let child = locked.join("child.txt");
        std::fs::write(&child, b"x").unwrap();
        // Remove all permissions on the parent so its children cannot be stat'd.
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

        // Probe whether the lock is actually effective — root bypasses permission checks, so skip
        // the assertions there rather than fail spuriously in a root CI container.
        let lock_effective = std::fs::read(&child).is_err();
        let existed = require_exists(&child.to_string_lossy());
        let dir_check = require_dir(&child.to_string_lossy());
        let exists = path_exists(&child.to_string_lossy());
        // Restore permissions so the tempdir can be cleaned up regardless of the assertions.
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

        if lock_effective {
            assert!(matches!(existed, Err(PortError::Io(_))), "unreadable parent must surface Io, got {existed:?}");
            assert!(matches!(dir_check, Err(PortError::Io(_))), "unreadable parent must surface Io, got {dir_check:?}");
            assert!(matches!(exists, Err(PortError::Io(_))), "unreadable parent must surface Io, got {exists:?}");
        }
    }
}
