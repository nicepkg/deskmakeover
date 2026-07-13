//! `.lnk` apply + restore. Ported from `ShellLinkShortcutIconWriter` and the preflight in
//! `ShortcutIconApplyOperation` (`DesktopIconApplyOperations.cs`). The apply runs on the STA
//! thread (COM); restore is a plain byte replay.

use std::path::Path;

use dm_domain::{PortError, PortResult};

use crate::shell::{attrs, shell_link};

/// Points the shortcut at the generated `.ico`. Preflights that both the generated asset and the
/// shortcut still exist (oracle `PreflightTarget`); the durable CAS check lives in the driver.
///
/// [WINDOWS-VERIFY] runtime.
pub fn apply(shortcut_path: &str, icon_path: &str) -> PortResult<()> {
    // try_exists() distinguishes genuine absence from an access failure (audit F3): an unreadable but
    // PRESENT asset/shortcut must surface its real IO error, not masquerade as missing.
    match Path::new(icon_path).try_exists() {
        Ok(true) => {}
        Ok(false) => return Err(PortError::AssetMissing(icon_path.to_string())),
        Err(e) => return Err(PortError::Io(format!("cannot access {icon_path}: {e}"))),
    }
    match Path::new(shortcut_path).try_exists() {
        Ok(true) => {}
        Ok(false) => return Err(PortError::NotFound(shortcut_path.to_string())),
        Err(e) => return Err(PortError::Io(format!("cannot access {shortcut_path}: {e}"))),
    }
    // A .lnk swapped for a symlink since scan would have IPersistFile::Save follow it elsewhere. (APPLY-2)
    if attrs::is_reparse_point(shortcut_path)? {
        return Err(PortError::Io(format!(
            "{shortcut_path} became a reparse point after scan; refusing to write through it"
        )));
    }
    shell_link::set_icon_location(shortcut_path, icon_path, 0)
}

/// Restores the shortcut by writing its captured original bytes back verbatim
/// (oracle `RestoreOriginalContent`). Durable + atomic so a crash mid-restore can't tear the
/// `.lnk` (P1-9). [WINDOWS-VERIFY] runtime.
pub fn restore_bytes(path: &str, bytes: &[u8]) -> PortResult<()> {
    crate::durable::write_atomic(path, bytes)
}
