//! Thin `Get/SetFileAttributesW` wrappers used by the folder + file-wrapper writers, which must
//! set `Hidden`/`System`/`ReadOnly` bits that `std::fs` cannot. Ported from the
//! `File.GetAttributes`/`File.SetAttributes` calls in `FolderIconWriter` /
//! `RegularFileWrapperWriter`.

use dm_domain::{PortError, PortResult};
use windows::core::HSTRING;
use windows::Win32::Foundation::{GetLastError, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND};
use windows::Win32::Storage::FileSystem::{
    GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_SYSTEM, FILE_FLAGS_AND_ATTRIBUTES,
    INVALID_FILE_ATTRIBUTES,
};

pub const HIDDEN: u32 = FILE_ATTRIBUTE_HIDDEN.0;
pub const SYSTEM: u32 = FILE_ATTRIBUTE_SYSTEM.0;
pub const READONLY: u32 = FILE_ATTRIBUTE_READONLY.0;
pub const NORMAL: u32 = FILE_ATTRIBUTE_NORMAL.0;
pub const REPARSE_POINT: u32 = 0x0000_0400; // FILE_ATTRIBUTE_REPARSE_POINT

/// Whether the path is a reparse point (junction/symlink). `GetFileAttributesW` reports the
/// reparse flag on the entry itself without following it. The apply writers re-check this even
/// though `scan` already excluded reparse points: `is_dir()`/`exists()` FOLLOW a junction, so a
/// folder swapped for a junction to elsewhere in the scan→apply window would otherwise be written
/// through (desktop.ini/.lnk landing in the link's target). (APPLY-2) [WINDOWS-VERIFY] runtime.
pub fn is_reparse_point(path: &str) -> PortResult<bool> {
    Ok(get(path)? & REPARSE_POINT != 0)
}

/// Reads a path's raw `FILE_ATTRIBUTE_*` bits.
///
/// `GetFileAttributesW` returns `INVALID_FILE_ATTRIBUTES` for BOTH "the path does not exist" and a
/// real failure (access denied, sharing violation, device error). We distinguish them by
/// `GetLastError` (P2-#3): only file/path-not-found maps to `NotFound` (a benign skip); any other
/// error PROPAGATES as `Io`, so an existing-but-unreadable item is never silently recorded as
/// absent — which, at restore time, could otherwise irreversibly delete a wrapper we wrongly
/// believed never existed. [WINDOWS-VERIFY] runtime.
pub fn get(path: &str) -> PortResult<u32> {
    // SAFETY: reads attributes of a UTF-16 path; no ownership transfer.
    let attrs = unsafe { GetFileAttributesW(&HSTRING::from(path)) };
    if attrs == INVALID_FILE_ATTRIBUTES {
        // SAFETY: reads the calling thread's last-error code set by GetFileAttributesW above.
        let err = unsafe { GetLastError() };
        return match err {
            ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => Err(PortError::NotFound(path.to_string())),
            other => Err(PortError::Io(format!(
                "GetFileAttributesW({path}) failed (win32 error {})",
                other.0
            ))),
        };
    }
    Ok(attrs)
}

/// Writes a path's raw `FILE_ATTRIBUTE_*` bits. [WINDOWS-VERIFY] runtime.
pub fn set(path: &str, attrs: u32) -> PortResult<()> {
    // SAFETY: sets attributes on a UTF-16 path.
    unsafe { SetFileAttributesW(&HSTRING::from(path), FILE_FLAGS_AND_ATTRIBUTES(attrs)) }
        .map_err(|e| PortError::Io(e.to_string()))
}

/// Clears the `ReadOnly` bit if present (folders must be writable before `desktop.ini` edits).
pub fn clear_readonly(path: &str) -> PortResult<()> {
    let attrs = get(path)?;
    if attrs & READONLY != 0 {
        set(path, attrs & !READONLY)?;
    }
    Ok(())
}
