//! The [`ItemStateReader`] for every desktop item kind: the CAS fingerprint and the exact-restore
//! anchor. Ported from `DeskMakeover.Shell/RestoreMetadataCollector.cs` (per-kind capture) and the
//! `SequenceEqual` preflight (`DesktopIconApplyOperations.cs`).
//!
//! The **fingerprint** covers only the icon reference the writer sets (the styleable surface),
//! derivable from the asset so the applier's `expected` needs no self-re-read (P1-1) and an
//! unrelated byte/attribute is not a false conflict (P2-2). The **anchor** still captures the full
//! original state needed for an exact restore.
//!
//! [WINDOWS-VERIFY] runtime (filesystem/registry/COM semantics).

use std::path::Path;
use std::sync::Arc;

use dm_domain::{
    DesktopIniAnchor, Fingerprint, ItemKind, ItemStateReader, ItemTarget, PortError, PortResult,
    RestoreAnchor, WrapperAnchor,
};

use crate::apply::{file_wrapper, recyclebin};
use crate::com::StaExecutor;
use crate::fingerprint_surface::{self as fp, SurfaceState};
use crate::shell::{attrs, shell_link};
use crate::textfmt;

/// Reads fingerprints and anchors across all item kinds.
///
/// Holds the shared [`StaExecutor`] because reading a `.lnk`'s icon location is COM
/// (`IShellLinkW`), which — like every shell function ([`crate::shell`]) — MUST run on the STA
/// apartment thread. The applier already marshals its `.lnk` writes onto this executor; the reader
/// must route its `.lnk` reads through the SAME discipline, or a preflight/read-back can fail with
/// an uninitialized or wrong COM apartment. Non-COM reads (file attributes, `desktop.ini` text,
/// registry values) stay inline.
pub struct WindowsStateReader {
    exec: Arc<StaExecutor>,
}

impl WindowsStateReader {
    /// Builds a reader that marshals its COM reads onto `exec` — pass the SAME executor the
    /// [`crate::apply::WindowsIconApplier`] uses so all shell COM shares one apartment thread.
    pub fn new(exec: Arc<StaExecutor>) -> Self {
        Self { exec }
    }
}

impl ItemStateReader for WindowsStateReader {
    fn read_fingerprint(&self, target: &ItemTarget) -> PortResult<Fingerprint> {
        match target.kind {
            // The surface is the icon LOCATION the writer sets (path + index), read straight from
            // the live `.lnk` — a COM read, so it runs on the STA thread (outer `?` = thread-join
            // error, inner `?` = the read's own error). A missing shortcut is NotFound (a deleted
            // item is skipped, not read as an empty surface).
            ItemKind::Shortcut => {
                require_exists(&target.path)?;
                let p = target.path.clone();
                let (path, index) =
                    self.exec.run(move || shell_link::read_icon_location(&p))??.unwrap_or_default();
                Ok(SurfaceState::IconRef { path, index }.fingerprint())
            }
            // `.url`: the `[InternetShortcut]` `IconFile`/`IconIndex`.
            ItemKind::UrlShortcut => {
                let (path, index) =
                    textfmt::parse_internet_shortcut_icon(&read_text(&target.path)?).unwrap_or_default();
                Ok(SurfaceState::IconRef { path, index }.fingerprint())
            }
            // Folder: the `desktop.ini` `IconResource` path/index (BOM-tolerant parse).
            ItemKind::Folder => {
                require_dir(&target.path)?;
                // An absent desktop.ini is an unstyled folder (empty surface); a present-but-
                // unreadable one is a real error, not silently "unstyled" (P2-3).
                let (path, index) = textfmt::parse_desktop_ini_icon(&read_desktop_ini(&target.path)?).unwrap_or_default();
                Ok(SurfaceState::IconRef { path, index }.fingerprint())
            }
            // Loose file: the companion wrapper's icon location + ONLY the owned attribute bits
            // (Hidden|System). The file's own bytes are never touched by apply, so they are not the
            // surface (P1-10); unrelated bits (ARCHIVE, offline) are masked off (P2-2). A missing
            // original is NotFound via `attrs::get`, so a deleted file is skipped.
            ItemKind::RegularFile => {
                let owned_attr_bits = attrs::get(&target.path)? & fp::OWNED_ATTR_BITS;
                let wrapper = file_wrapper::wrapper_path(&target.path);
                // The wrapper's icon location is a COM read (`IShellLinkW`) → STA thread.
                let wrapper_icon = if Path::new(&wrapper).exists() {
                    let w = wrapper.clone();
                    self.exec.run(move || shell_link::read_icon_location(&w))??
                } else {
                    None
                };
                Ok(SurfaceState::RegularFile { wrapper_icon, owned_attr_bits }.fingerprint())
            }
            // Recycle Bin: the empty + full icon paths the per-user `DefaultIcon` values point at
            // (stripping the trailing `,index`), so the surface matches the paired assets.
            ItemKind::RecycleBin => {
                let a = recyclebin::read_current();
                let empty = a.empty.as_ref().map(|v| strip_icon_index(&v.raw)).unwrap_or_default().to_string();
                let full = a.full.as_ref().map(|v| strip_icon_index(&v.raw)).unwrap_or_default().to_string();
                Ok(SurfaceState::RecycleBin { empty, full }.fingerprint())
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

/// The `desktop.ini` bytes for a folder: absent ⇒ empty (an unstyled folder); a present-but-
/// unreadable file ⇒ a propagated I/O error rather than a silent "unstyled" reading (P2-3).
fn read_desktop_ini(folder_path: &str) -> PortResult<Vec<u8>> {
    match std::fs::read(Path::new(folder_path).join("desktop.ini")) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(PortError::Io(e.to_string())),
    }
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

fn read_text(path: &str) -> PortResult<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(PortError::NotFound(path.to_string()))
        }
        Err(e) => Err(PortError::Io(e.to_string())),
    }
}

/// Fails with `NotFound` when a shortcut path is gone, so a deleted item is skipped rather than
/// surfacing as a COM load error (which the driver would treat as a real infrastructure failure).
fn require_exists(path: &str) -> PortResult<()> {
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err(PortError::NotFound(path.to_string()))
    }
}

fn require_dir(path: &str) -> PortResult<()> {
    if Path::new(path).is_dir() {
        Ok(())
    } else {
        Err(PortError::NotFound(path.to_string()))
    }
}

/// Strips the trailing `,index` from a `DefaultIcon` registry value (`C:\x.ico,0` → `C:\x.ico`),
/// splitting on the LAST comma so icon paths containing commas survive.
fn strip_icon_index(value: &str) -> &str {
    match value.rfind(',') {
        Some(comma) => &value[..comma],
        None => value,
    }
}
