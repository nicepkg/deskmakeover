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
    PriorWrapper, RestoreAnchor, WrapperAnchor,
};

use crate::apply::{file_wrapper, recyclebin};
use crate::classify::parse_icon_location;
use crate::com::StaExecutor;
use crate::fingerprint_surface::{self as fp, SurfaceState, WrapperSurface};
use crate::pathcheck;
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
            // A UWP shortcut's desktop entry is an ordinary `.lnk`, so it reads through the same
            // IconRef surface as a Shortcut (spec 06 §6, P1-12).
            ItemKind::Shortcut | ItemKind::AppxShortcut => {
                pathcheck::require_exists(&target.path)?;
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
            // Folder: the `desktop.ini` `IconResource` path/index (BOM-tolerant parse) PLUS the
            // folder attribute bits apply owns (READONLY — Explorer needs it to honour desktop.ini),
            // masked so unrelated bits are not a false conflict (P1-#1/P2-2).
            ItemKind::Folder => {
                pathcheck::require_dir(&target.path)?;
                // An absent desktop.ini is an unstyled folder (empty surface); a present-but-
                // unreadable one is a real error, not silently "unstyled" (P2-3).
                let icon = textfmt::parse_desktop_ini_icon(&read_desktop_ini(&target.path)?).unwrap_or_default();
                let owned_attr_bits = attrs::get(&target.path)? & fp::FOLDER_OWNED_ATTR_BITS;
                Ok(SurfaceState::Folder { icon, owned_attr_bits }.fingerprint())
            }
            // Loose file: the companion wrapper's FULL identity (icon location + target + working
            // dir) + ONLY the owned attribute bits (Hidden|System) on the original. The file's own
            // bytes are never touched by apply, so they are not the surface (P1-10); unrelated bits
            // (ARCHIVE, offline) are masked off (P2-2); the wrapper target/workdir are covered so a
            // partial write is caught (P1-#1). A missing original is NotFound via `attrs::get`.
            ItemKind::RegularFile => {
                let owned_attr_bits = attrs::get(&target.path)? & fp::OWNED_ATTR_BITS;
                let wrapper_path = file_wrapper::wrapper_path(&target.path);
                // The wrapper's identity is a COM read (`IShellLinkW`) → STA thread. `path_exists`
                // propagates a metadata error rather than reading it as "no wrapper" (P2, wave-2R).
                let wrapper = if pathcheck::path_exists(&wrapper_path)? {
                    let w = wrapper_path.clone();
                    let id = self.exec.run(move || shell_link::read_wrapper_identity(&w))??;
                    Some(WrapperSurface {
                        icon: id.icon.unwrap_or_default(),
                        target: id.target,
                        working_dir: id.working_dir,
                    })
                } else {
                    None
                };
                Ok(SurfaceState::RegularFile { wrapper, owned_attr_bits }.fingerprint())
            }
            // Recycle Bin: the default/empty/full icon values the per-user `DefaultIcon` points at,
            // each parsed into (path, index) so the surface matches the paired assets AND a wrong
            // index or a stale `default` is caught (P1-#1).
            ItemKind::RecycleBin => {
                // `parse_icon_location` only treats a trailing segment as the index when it parses as
                // an integer, so a malformed value (`…\full.ico,garbage`) stays whole and cannot be
                // mistaken for a valid `(path, 0)` — nor can two distinct comma-bearing paths collide
                // (wave-2R P1-#2). Reuses the host-tested classifier parser.
                let a = recyclebin::read_current()?;
                let default = a.default.as_ref().map(|v| parse_icon_location(&v.raw)).unwrap_or_default();
                let empty = a.empty.as_ref().map(|v| parse_icon_location(&v.raw)).unwrap_or_default();
                let full = a.full.as_ref().map(|v| parse_icon_location(&v.raw)).unwrap_or_default();
                Ok(SurfaceState::RecycleBin { default, empty, full }.fingerprint())
            }
            // System is styleable per spec 06 §6 but its HKCU CLSID reader is a Windows-scoped
            // follow-up — an honest labelled pending error, not the generic Unsupported (P1-12).
            ItemKind::System => Err(PortError::Unsupported(
                "[WINDOWS-VERIFY] System DefaultIcon read is not yet wired (HKCU CLSID reader pending)".into(),
            )),
            ItemKind::Unsupported => {
                Err(PortError::Unsupported(format!("fingerprint for {:?}", target.kind)))
            }
        }
    }

    fn capture_anchor(&self, target: &ItemTarget) -> PortResult<RestoreAnchor> {
        match target.kind {
            // AppxShortcut is an ordinary `.lnk`: byte-replay restore like a Shortcut (P1-12).
            ItemKind::Shortcut | ItemKind::UrlShortcut | ItemKind::AppxShortcut => {
                Ok(RestoreAnchor::FileBytes { bytes: read_bytes(&target.path)? })
            }
            ItemKind::Folder => Ok(capture_folder(&target.path)?),
            ItemKind::RegularFile => Ok(capture_file(&target.path)?),
            ItemKind::RecycleBin => Ok(RestoreAnchor::RecycleBin(recyclebin::read_current()?)),
            ItemKind::System => Err(PortError::Unsupported(
                "[WINDOWS-VERIFY] System DefaultIcon anchor is not yet wired (HKCU CLSID reader pending)".into(),
            )),
            ItemKind::Unsupported => {
                Err(PortError::Unsupported(format!("anchor for {:?}", target.kind)))
            }
        }
    }
}

fn capture_folder(folder_path: &str) -> PortResult<RestoreAnchor> {
    let attributes = attrs::get(folder_path)?;
    let ini = Path::new(folder_path).join("desktop.ini");
    // `try_exists` propagates a metadata error (e.g. access denied) instead of `exists`, which
    // reports `false` on any failure — so a present-but-unreadable `desktop.ini` is never captured
    // as "absent" and then wrongly removed on restore (P2-#3).
    let desktop_ini = if ini.try_exists().map_err(|e| PortError::Io(e.to_string()))? {
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
    // `try_exists` propagates a metadata error rather than reporting `false`: an existing-but-
    // unreadable wrapper recorded as `wrapper_existed:false` would be irreversibly DELETED on
    // restore (the "not existed → remove it" branch). Fail closed instead (P2-#3).
    let wrapper = file_wrapper::wrapper_path(file_path);
    let prior_wrapper = if Path::new(&wrapper).try_exists().map_err(|e| PortError::Io(e.to_string()))? {
        // A present wrapper ALWAYS captures its bytes — the enum makes "existed but no content"
        // (which restore could not undo) unrepresentable (audit A1-🔴). A read fault propagates
        // (fail closed) rather than recording an unrestorable present-with-no-bytes anchor.
        let content = std::fs::read(&wrapper).map_err(|e| PortError::Io(e.to_string()))?;
        PriorWrapper::Present { content }
    } else {
        PriorWrapper::Absent
    };
    Ok(RestoreAnchor::RegularFile(WrapperAnchor { file_attributes, prior_wrapper }))
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

