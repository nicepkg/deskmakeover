//! Folder styling via `desktop.ini`. Ported from `DeskMakeover.Shell/FolderIconWriter.cs`. The
//! `desktop.ini` content is pure (unit-tested on the host); the attribute juggling is
//! `[WINDOWS-VERIFY]`.

use std::path::Path;

use dm_domain::{DesktopIniAnchor, PortError, PortResult};

use crate::shell::attrs;
use crate::textfmt::desktop_ini_bytes;

/// Writes `desktop.ini` (Hidden+System) and marks the folder ReadOnly so Explorer honours it.
/// Mirrors `FolderIconWriter.Apply`. [WINDOWS-VERIFY] runtime.
pub fn apply(folder_path: &str, icon_path: &str) -> PortResult<()> {
    if !Path::new(folder_path).is_dir() {
        return Err(PortError::NotFound(folder_path.to_string()));
    }
    // Re-check reparse safety at apply time: is_dir() FOLLOWS a junction, so a folder swapped for
    // a junction since scan would send desktop.ini into the link's target. (APPLY-2)
    if attrs::is_reparse_point(folder_path)? {
        return Err(PortError::Io(format!(
            "{folder_path} became a reparse point after scan; refusing to write desktop.ini through it"
        )));
    }
    let ini = ini_path(folder_path);
    attrs::clear_readonly(folder_path)?;
    if Path::new(&ini).exists() {
        attrs::set(&ini, attrs::NORMAL)?;
    }
    // Durable + atomic so a crash mid-write can't tear `desktop.ini` (P1-9).
    crate::durable::write_atomic(&ini, &desktop_ini_bytes(icon_path))?;
    attrs::set(&ini, attrs::HIDDEN | attrs::SYSTEM)?;
    let folder_attrs = attrs::get(folder_path)?;
    attrs::set(folder_path, folder_attrs | attrs::READONLY)
}

/// Restores the folder's original `desktop.ini` (or deletes ours) and its original attributes.
/// Mirrors `FolderIconWriter.RestoreOriginal`. [WINDOWS-VERIFY] runtime.
pub fn restore(
    folder_path: &str,
    folder_attributes: u32,
    desktop_ini: Option<&DesktopIniAnchor>,
) -> PortResult<()> {
    // Fallible metadata, not is_dir() (audit F3): a permission/sharing/IO error must NOT read as
    // "folder gone → nothing to revert" (fail-open success that strands styled state); only a genuine
    // NotFound or a replaced-by-non-directory is a benign no-op.
    match std::fs::metadata(folder_path) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => return Ok(()), // replaced by a non-directory — our folder styling is moot
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(PortError::Io(format!("cannot stat {folder_path}: {e}"))),
    }
    let ini = ini_path(folder_path);
    attrs::clear_readonly(folder_path)?;
    if Path::new(&ini).exists() {
        attrs::set(&ini, attrs::NORMAL)?;
    }

    match desktop_ini {
        Some(anchor) => {
            crate::durable::write_atomic(&ini, &anchor.content)?;
            attrs::set(&ini, anchor.attributes)?;
        }
        None if Path::new(&ini).exists() => {
            std::fs::remove_file(&ini).map_err(|e| PortError::Io(e.to_string()))?;
        }
        None => {}
    }

    attrs::set(folder_path, folder_attributes)
}

fn ini_path(folder_path: &str) -> String {
    Path::new(folder_path).join("desktop.ini").to_string_lossy().into_owned()
}
