//! Loose-file styling by wrapping it in a companion `.lnk` and hiding the original. Ported from
//! `DeskMakeover.Shell/RegularFileWrapperWriter.cs`. Structural (creates a sibling file + hides
//! the original), so ADR-0020 §5 keeps it in the proposal queue for background automation.
//! [WINDOWS-VERIFY] runtime.

use std::path::Path;

use dm_domain::{PortError, PortResult, WrapperAnchor};

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
    let working_dir = Path::new(file_path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    shell_link::create_shortcut(&wrapper_path(file_path), file_path, &working_dir, icon_path)?;
    let current = attrs::get(file_path)?;
    attrs::set(file_path, current | attrs::HIDDEN | attrs::SYSTEM)
}

/// Removes the wrapper (or resurrects the pre-existing one) and restores the file's attributes.
/// Mirrors `Unwrap`. No COM — plain filesystem.
pub fn restore(file_path: &str, anchor: &WrapperAnchor) -> PortResult<()> {
    let wrapper = wrapper_path(file_path);
    if !anchor.wrapper_existed {
        if Path::new(&wrapper).exists() {
            std::fs::remove_file(&wrapper).map_err(|e| PortError::Io(e.to_string()))?;
        }
    } else if let Some(bytes) = &anchor.wrapper_content {
        // The apply overwrote a user-made shortcut of the same name — put its bytes back.
        std::fs::write(&wrapper, bytes).map_err(|e| PortError::Io(e.to_string()))?;
    }
    if Path::new(file_path).exists() {
        attrs::set(file_path, anchor.file_attributes)?;
    }
    Ok(())
}
