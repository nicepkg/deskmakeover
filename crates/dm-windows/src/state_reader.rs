//! The [`ItemStateReader`] for every desktop item kind: the CAS fingerprint and the exact-restore
//! anchor. Ported from `DeskMakeover.Shell/RestoreMetadataCollector.cs` (per-kind capture) and the
//! `SequenceEqual` preflight (`DesktopIconApplyOperations.cs`).
//!
//! * file kinds (`.lnk`/`.url`/loose file): fingerprint = SHA-256 of the bytes; anchor = the bytes;
//! * folder: fingerprint = SHA-256 of `desktop.ini` bytes + folder attributes; anchor = both;
//! * Recycle Bin: fingerprint + anchor = the per-user `DefaultIcon` registry values.
//!
//! [WINDOWS-VERIFY] runtime (filesystem/registry semantics).

use std::path::Path;

use dm_domain::{
    DesktopIniAnchor, Fingerprint, ItemKind, ItemStateReader, ItemTarget, PortError, PortResult,
    RestoreAnchor, WrapperAnchor,
};

use crate::apply::{file_wrapper, recyclebin};
use crate::shell::attrs;

/// Reads fingerprints and anchors across all item kinds.
pub struct WindowsStateReader;

impl ItemStateReader for WindowsStateReader {
    fn read_fingerprint(&self, target: &ItemTarget) -> PortResult<Fingerprint> {
        match target.kind {
            // `.lnk`/`.url` apply rewrites the file's own bytes, so the file bytes ARE the surface.
            ItemKind::Shortcut | ItemKind::UrlShortcut => {
                Ok(Fingerprint::of_bytes(&read_bytes(&target.path)?))
            }
            // A loose file is styled by a sibling wrapper `.lnk` + hiding the original; the file's
            // own bytes are never touched. Fingerprint what apply actually changes — the wrapper's
            // presence/bytes and the original's attribute bits — so styled ≠ unstyled and the item
            // can commit (P1-10). A missing original still errors NotFound (via `attrs::get`), so a
            // deleted file is skipped, not misread as an unchanged surface.
            ItemKind::RegularFile => {
                let wrapper = file_wrapper::wrapper_path(&target.path);
                let wrapper_bytes = match std::fs::read(&wrapper) {
                    Ok(bytes) => Some(bytes),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => return Err(PortError::Io(e.to_string())),
                };
                let file_attributes = attrs::get(&target.path)?;
                Ok(crate::fingerprint_surface::regular_file(
                    wrapper_bytes.as_deref(),
                    file_attributes,
                ))
            }
            ItemKind::Folder => {
                let ini = ini_bytes(&target.path);
                let folder_attrs = attrs::get(&target.path)?;
                Ok(Fingerprint::of_parts(&[&ini, &folder_attrs.to_le_bytes()]))
            }
            ItemKind::RecycleBin => {
                let a = recyclebin::read_current();
                Ok(Fingerprint::of_parts(&[
                    a.default.as_ref().map(|v| v.raw.as_bytes()).unwrap_or_default(),
                    a.empty.as_ref().map(|v| v.raw.as_bytes()).unwrap_or_default(),
                    a.full.as_ref().map(|v| v.raw.as_bytes()).unwrap_or_default(),
                ]))
            }
            other => Err(PortError::Unsupported(format!("fingerprint for {other:?}"))),
        }
    }

    fn capture_anchor(&self, target: &ItemTarget) -> PortResult<RestoreAnchor> {
        match target.kind {
            ItemKind::Shortcut | ItemKind::UrlShortcut => {
                Ok(RestoreAnchor::FileBytes { bytes: read_bytes(&target.path)? })
            }
            ItemKind::Folder => Ok(capture_folder(&target.path)?),
            ItemKind::RegularFile => Ok(capture_file(&target.path)?),
            ItemKind::RecycleBin => Ok(RestoreAnchor::RecycleBin(recyclebin::read_current())),
            other => Err(PortError::Unsupported(format!("anchor for {other:?}"))),
        }
    }
}

fn capture_folder(folder_path: &str) -> PortResult<RestoreAnchor> {
    let attributes = attrs::get(folder_path)?;
    let ini = Path::new(folder_path).join("desktop.ini");
    let desktop_ini = if ini.exists() {
        let content = std::fs::read(&ini).map_err(|e| PortError::Io(e.to_string()))?;
        let ini_attrs = attrs::get(&ini.to_string_lossy())?;
        Some(DesktopIniAnchor { content, attributes: ini_attrs })
    } else {
        None
    };
    Ok(RestoreAnchor::Folder { attributes, desktop_ini })
}

fn capture_file(file_path: &str) -> PortResult<RestoreAnchor> {
    let file_attributes = attrs::get(file_path)?;
    // Capture the wrapper if a same-named `.lnk` already exists (our apply would overwrite it).
    let wrapper = file_wrapper::wrapper_path(file_path);
    let wrapper_existed = Path::new(&wrapper).exists();
    let wrapper_content = if wrapper_existed {
        Some(std::fs::read(&wrapper).map_err(|e| PortError::Io(e.to_string()))?)
    } else {
        None
    };
    Ok(RestoreAnchor::RegularFile(WrapperAnchor {
        file_attributes,
        wrapper_existed,
        wrapper_content,
    }))
}

fn ini_bytes(folder_path: &str) -> Vec<u8> {
    std::fs::read(Path::new(folder_path).join("desktop.ini")).unwrap_or_default()
}

fn read_bytes(path: &str) -> PortResult<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(PortError::NotFound(path.to_string()))
        }
        Err(e) => Err(PortError::Io(e.to_string())),
    }
}
