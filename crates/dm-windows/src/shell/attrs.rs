//! Thin `Get/SetFileAttributesW` wrappers used by the folder + file-wrapper writers, which must
//! set `Hidden`/`System`/`ReadOnly` bits that `std::fs` cannot. Ported from the
//! `File.GetAttributes`/`File.SetAttributes` calls in `FolderIconWriter` /
//! `RegularFileWrapperWriter`.

use dm_domain::{PortError, PortResult};
use windows::core::HSTRING;
use windows::Win32::Storage::FileSystem::{
    GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_SYSTEM, FILE_FLAGS_AND_ATTRIBUTES,
    INVALID_FILE_ATTRIBUTES,
};

pub const HIDDEN: u32 = FILE_ATTRIBUTE_HIDDEN.0;
pub const SYSTEM: u32 = FILE_ATTRIBUTE_SYSTEM.0;
pub const READONLY: u32 = FILE_ATTRIBUTE_READONLY.0;
pub const NORMAL: u32 = FILE_ATTRIBUTE_NORMAL.0;

/// Reads a path's raw `FILE_ATTRIBUTE_*` bits. [WINDOWS-VERIFY] runtime.
pub fn get(path: &str) -> PortResult<u32> {
    // SAFETY: reads attributes of a UTF-16 path; no ownership transfer.
    let attrs = unsafe { GetFileAttributesW(&HSTRING::from(path)) };
    if attrs == INVALID_FILE_ATTRIBUTES {
        return Err(PortError::NotFound(path.to_string()));
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
