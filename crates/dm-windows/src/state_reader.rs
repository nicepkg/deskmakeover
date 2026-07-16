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

use crate::apply::{file_wrapper, recyclebin, system};
use crate::classify::parse_icon_location;
use crate::com::StaExecutor;
use crate::durable::{read_capped, SHORTCUT_READ_CAP};
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
            // `.url`: the `[InternetShortcut]` `IconFile`/`IconIndex`. Decoded encoding-aware —
            // Steam writes these as UTF-16 LE, which read_to_string rejected outright, dropping the
            // whole shortcut to non-styleable (owner report 2026-07-15).
            ItemKind::UrlShortcut => {
                let text = textfmt::decode_ini_text_bytes(&read_bytes(&target.path)?);
                let (path, index) =
                    textfmt::parse_internet_shortcut_icon(&text).unwrap_or_default();
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
            // System (This PC / Network / …): the styleable surface is the effective per-CLSID
            // `DefaultIcon` — one icon location, so it fingerprints as an `IconRef` exactly like a
            // shortcut's, and matches `expected_after_apply`'s IconRef for a System item (P1-12).
            ItemKind::System => {
                let a = system::read_current(&system::parse_clsid(&target.path)?)?;
                let (path, index) = a.value.as_ref().map(|v| parse_icon_location(&v.raw)).unwrap_or_default();
                Ok(SurfaceState::IconRef { path, index }.fingerprint())
            }
            ItemKind::Unsupported => {
                Err(PortError::Unsupported(format!("fingerprint for {:?}", target.kind)))
            }
        }
    }

    fn read_styleable_surface(
        &self,
        target: &ItemTarget,
    ) -> PortResult<(Fingerprint, Vec<(String, i32)>)> {
        match target.kind {
            // A shortcut's fingerprint IS its icon location, so read the location ONCE and return BOTH
            // — the elevated CAS anchor and the fingerprint can then never disagree (§P1-1). A UWP
            // shortcut is an ordinary `.lnk`, same surface.
            ItemKind::Shortcut | ItemKind::AppxShortcut => {
                pathcheck::require_exists(&target.path)?;
                let p = target.path.clone();
                let (path, index) =
                    self.exec.run(move || shell_link::read_icon_location(&p))??.unwrap_or_default();
                let fingerprint = SurfaceState::IconRef { path: path.clone(), index }.fingerprint();
                Ok((fingerprint, vec![(path, index)]))
            }
            // The other icon-reference kinds also expose their live icon location(s) — the
            // styled-residue provenance check needs them (a live location inside OUR asset store is
            // this app's output; with no trustworthy anchor it must never be recaptured as "the
            // original" or re-extracted as a bake source — owner report 2026-07-17, a folder
            // compounding Style(Style(orig)) after its ledger row was lost). Each read mirrors the
            // corresponding `read_fingerprint` arm, reading the location(s) ONCE.
            ItemKind::UrlShortcut => {
                let text = textfmt::decode_ini_text_bytes(&read_bytes(&target.path)?);
                let (path, index) =
                    textfmt::parse_internet_shortcut_icon(&text).unwrap_or_default();
                let fingerprint = SurfaceState::IconRef { path: path.clone(), index }.fingerprint();
                Ok((fingerprint, vec![(path, index)]))
            }
            ItemKind::Folder => {
                pathcheck::require_dir(&target.path)?;
                let icon = textfmt::parse_desktop_ini_icon(&read_desktop_ini(&target.path)?).unwrap_or_default();
                let owned_attr_bits = attrs::get(&target.path)? & fp::FOLDER_OWNED_ATTR_BITS;
                let location = icon.clone();
                Ok((SurfaceState::Folder { icon, owned_attr_bits }.fingerprint(), vec![location]))
            }
            ItemKind::System => {
                let a = system::read_current(&system::parse_clsid(&target.path)?)?;
                let (path, index) = a.value.as_ref().map(|v| parse_icon_location(&v.raw)).unwrap_or_default();
                let fingerprint = SurfaceState::IconRef { path: path.clone(), index }.fingerprint();
                Ok((fingerprint, vec![(path, index)]))
            }
            // RegularFile: the companion WRAPPER's icon is the live location — a wrapper pointing
            // into our asset store with no ledger row is styled residue like any other kind (codex
            // R3 P1: the guard was blind here). No wrapper → empty location (never "ours").
            ItemKind::RegularFile => {
                let owned_attr_bits = attrs::get(&target.path)? & fp::OWNED_ATTR_BITS;
                let wrapper_path = file_wrapper::wrapper_path(&target.path);
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
                let location = wrapper.as_ref().map(|w| w.icon.clone()).unwrap_or_default();
                Ok((SurfaceState::RegularFile { wrapper, owned_attr_bits }.fingerprint(), vec![location]))
            }
            // RecycleBin: a multi-value surface — ALL THREE registry values are returned, so a
            // PARTIAL write (our asset already in `empty`/`full` while `default` is still the
            // original) is visible to the provenance guards (codex 2026-07-17 P1: a single
            // representative location under-reported exactly that residue).
            ItemKind::RecycleBin => {
                let a = recyclebin::read_current()?;
                let default = a.default.as_ref().map(|v| parse_icon_location(&v.raw)).unwrap_or_default();
                let empty = a.empty.as_ref().map(|v| parse_icon_location(&v.raw)).unwrap_or_default();
                let full = a.full.as_ref().map(|v| parse_icon_location(&v.raw)).unwrap_or_default();
                let locations = vec![default.clone(), empty.clone(), full.clone()];
                Ok((SurfaceState::RecycleBin { default, empty, full }.fingerprint(), locations))
            }
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
            ItemKind::System => {
                Ok(RestoreAnchor::SystemIcon(system::read_current(&system::parse_clsid(&target.path)?)?))
            }
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
        let content = read_capped(&ini, SHORTCUT_READ_CAP).map_err(|e| PortError::Io(e.to_string()))?;
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
        let content = read_capped(&wrapper, SHORTCUT_READ_CAP).map_err(|e| PortError::Io(e.to_string()))?;
        PriorWrapper::Present { content }
    } else {
        PriorWrapper::Absent
    };
    Ok(RestoreAnchor::RegularFile(WrapperAnchor { file_attributes, prior_wrapper }))
}

/// The `desktop.ini` bytes for a folder: absent ⇒ empty (an unstyled folder); a present-but-
/// unreadable file ⇒ a propagated I/O error rather than a silent "unstyled" reading (P2-3).
fn read_desktop_ini(folder_path: &str) -> PortResult<Vec<u8>> {
    match read_capped(Path::new(folder_path).join("desktop.ini"), SHORTCUT_READ_CAP) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(PortError::Io(e.to_string())),
    }
}

fn read_bytes(path: &str) -> PortResult<Vec<u8>> {
    match read_capped(path, SHORTCUT_READ_CAP) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(PortError::NotFound(path.to_string()))
        }
        Err(e) => Err(PortError::Io(e.to_string())),
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::com::StaExecutor;
    use std::sync::Arc;

    fn reader() -> WindowsStateReader {
        WindowsStateReader::new(Arc::new(StaExecutor::spawn().unwrap()))
    }

    fn target(kind: ItemKind, path: &str) -> ItemTarget {
        ItemTarget { id: dm_domain::ItemId::from_raw("t"), kind, path: path.to_string() }
    }

    #[test]
    fn url_styleable_surface_reports_the_live_icon_location_and_a_matching_fingerprint() {
        // The styled-residue provenance check consumes the location; it must be the SAME state the
        // fingerprint hashes (one read, never two disagreeing ones).
        let dir = tempfile::tempdir().unwrap();
        let url = dir.path().join("site.url");
        std::fs::write(&url, "[InternetShortcut]\r\nURL=https://x\r\nIconFile=C:\\gen\\a.ico\r\nIconIndex=3\r\n").unwrap();
        let r = reader();
        let t = target(ItemKind::UrlShortcut, &url.to_string_lossy());
        let (fp, loc) = r.read_styleable_surface(&t).unwrap();
        assert_eq!(loc, vec![(r"C:\gen\a.ico".to_string(), 3)]);
        assert_eq!(fp, r.read_fingerprint(&t).unwrap(), "surface and fingerprint reads agree");
    }

    #[test]
    fn folder_styleable_surface_reports_the_desktop_ini_icon_and_a_matching_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("styled-folder");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(
            folder.join("desktop.ini"),
            "[.ShellClassInfo]\r\nIconResource=C:\\gen\\f.ico,0\r\nConfirmFileOp=0\r\n",
        )
        .unwrap();
        let r = reader();
        let t = target(ItemKind::Folder, &folder.to_string_lossy());
        let (fp, loc) = r.read_styleable_surface(&t).unwrap();
        assert_eq!(loc, vec![(r"C:\gen\f.ico".to_string(), 0)]);
        assert_eq!(fp, r.read_fingerprint(&t).unwrap(), "surface and fingerprint reads agree");
    }

    #[test]
    fn an_unstyled_folder_surface_is_an_empty_location_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("plain");
        std::fs::create_dir(&folder).unwrap();
        let r = reader();
        let t = target(ItemKind::Folder, &folder.to_string_lossy());
        let (fp, loc) = r.read_styleable_surface(&t).unwrap();
        assert_eq!(loc, vec![(String::new(), 0)], "no desktop.ini → empty location");
        assert_eq!(fp, r.read_fingerprint(&t).unwrap());
    }
}

