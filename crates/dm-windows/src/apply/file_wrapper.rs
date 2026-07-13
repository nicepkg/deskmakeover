//! Loose-file styling by wrapping it in a companion `.lnk` and hiding the original. Ported from
//! `DeskMakeover.Shell/RegularFileWrapperWriter.cs`. Structural (creates a sibling file + hides
//! the original), so ADR-0020 §5 keeps it in the proposal queue for background automation.
//! [WINDOWS-VERIFY] runtime.

use std::path::Path;

use dm_domain::{PortError, PortResult, WrapperAnchor};

use crate::fingerprint_surface::wrapper_working_dir;
use crate::shell::{attrs, shell_link};

/// The companion wrapper path for a loose file (`<file>.lnk`), oracle `WrapperPathFor`.
pub fn wrapper_path(file_path: &str) -> String {
    format!("{file_path}.lnk")
}

/// Creates the styled wrapper `.lnk` and hides the original (Hidden+System). Runs on the STA
/// thread (the `.lnk` creation is COM). Mirrors `Wrap`.
pub fn apply(file_path: &str, icon_path: &str) -> PortResult<()> {
    if !Path::new(file_path).is_file() {
        return Err(PortError::NotFound(file_path.to_string()));
    }
    // is_file() FOLLOWS a reparse point; a file swapped for a symlink since scan would be hidden
    // (and its wrapper aimed) through the link. Refuse. (APPLY-2)
    if attrs::is_reparse_point(file_path)? {
        return Err(PortError::Io(format!(
            "{file_path} became a reparse point after scan; refusing to wrap it"
        )));
    }
    // The SAME derivation the surface's `expected_after_apply` uses, so the working dir the wrapper
    // is given can never drift from what the driver expects to read back (P1-#1).
    let working_dir = wrapper_working_dir(file_path);
    shell_link::create_shortcut(&wrapper_path(file_path), file_path, &working_dir, icon_path)?;
    let current = attrs::get(file_path)?;
    attrs::set(file_path, current | attrs::HIDDEN | attrs::SYSTEM)
}

/// Removes the wrapper (or resurrects the pre-existing one) and restores the file's attributes.
/// Mirrors `Unwrap`. No COM — plain filesystem.
pub fn restore(file_path: &str, anchor: &WrapperAnchor) -> PortResult<()> {
    let wrapper = wrapper_path(file_path);
    if !anchor.wrapper_existed {
        // try_exists(), not exists() (audit F3): a metadata error must not silently skip removing
        // OUR wrapper (leaving styled state); only a genuine absence is a no-op.
        match Path::new(&wrapper).try_exists() {
            Ok(true) => std::fs::remove_file(&wrapper).map_err(|e| PortError::Io(e.to_string()))?,
            Ok(false) => {}
            Err(e) => return Err(PortError::Io(format!("cannot access {wrapper}: {e}"))),
        }
    } else if let Some(bytes) = &anchor.wrapper_content {
        // The apply overwrote a user-made shortcut of the same name — put its bytes back, durably
        // and atomically so a crash mid-restore can't tear it (P1-9).
        crate::durable::write_atomic(&wrapper, bytes)?;
    }
    // try_exists() (audit F3): a metadata error must not silently skip restoring the file's original
    // attributes (leaving it Hidden+System); only a genuine absence is a no-op.
    match Path::new(file_path).try_exists() {
        Ok(true) => attrs::set(file_path, anchor.file_attributes)?,
        Ok(false) => {}
        Err(e) => return Err(PortError::Io(format!("cannot access {file_path}: {e}"))),
    }
    Ok(())
}
