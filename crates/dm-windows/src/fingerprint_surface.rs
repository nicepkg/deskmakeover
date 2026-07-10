//! The **styleable surface** — the icon reference an apply actually writes — and its fingerprint.
//!
//! One host-tested source of truth consumed by BOTH sides of the apply/verify handshake:
//! * [`WindowsStateReader::read_fingerprint`] gathers the live surface off disk/registry into a
//!   [`SurfaceState`] and fingerprints it (the CAS anchor + the driver's read-back);
//! * [`WindowsIconApplier::apply`] builds the surface it INTENDS from the asset via
//!   [`expected_after_apply`] and fingerprints that (the driver's `expected`).
//!
//! Because both call [`SurfaceState::fingerprint`], the two can never disagree on the *computation*
//! — only on the raw values they feed, which is the `[WINDOWS-VERIFY]` I/O. This closes several
//! holes at once: the fingerprint is derivable from the asset, so the applier's `expected` needs no
//! tautological self-re-read (P1-1); it covers **everything apply changes and nothing else**, so a
//! partial writer that sets the icon but omits the coupled state (a folder's `READONLY` bit, the
//! wrapper's target/working-dir, the Recycle Bin's `default` value or icon indices) fingerprints
//! differently and is caught (P1-1), while an unrelated attribute flip is not a false conflict
//! (P2-2). The per-kind dispatch and the fingerprint math are unit-tested on the Mac host (P3-1);
//! only the disk/registry reads are deferred to Windows.

use std::path::Path;

use dm_domain::{Fingerprint, ItemKind};

/// An icon location: the path plus its resource index (`file.ico,0`).
pub type IconLoc = (String, i32);

/// The full identity of a loose-file wrapper `.lnk` that apply establishes: where it points, where
/// it runs, and which icon it shows. A partial writer that sets the icon but leaves the target or
/// working directory wrong makes a broken wrapper — including all three in the surface catches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperSurface {
    pub icon: IconLoc,
    pub target: String,
    pub working_dir: String,
}

/// The exact state an icon writer establishes — the field(s) it owns, nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceState {
    /// `.lnk`/`.url`: the icon location (path + index) the writer points the item at.
    IconRef { path: String, index: i32 },
    /// Folder: the `desktop.ini` `IconResource` location PLUS the folder attribute bits apply owns
    /// (`READONLY`, which Explorer requires before it honours `desktop.ini`). Masked to the owned
    /// bits so an unrelated attribute flip is not a false conflict; a writer that sets the icon but
    /// omits `READONLY` fingerprints differently, so the read-back fails verify (P1-1).
    Folder { icon: IconLoc, owned_attr_bits: u32 },
    /// Loose file: the companion wrapper's full identity (icon + target + working dir), `None` if no
    /// wrapper, plus the original file's owned attribute bits (`HIDDEN | SYSTEM`) — masked so a
    /// backup tool clearing `ARCHIVE` or an OS offline bit is not read as an external edit (P2-2).
    RegularFile { wrapper: Option<WrapperSurface>, owned_attr_bits: u32 },
    /// Recycle Bin: the `default`/`empty`/`full` `DefaultIcon` values (path + index each) — all
    /// three the apply writes, so a writer that lands the paths but a wrong index or a stale
    /// `default` fingerprints differently.
    RecycleBin { default: IconLoc, empty: IconLoc, full: IconLoc },
}

impl SurfaceState {
    /// The fingerprint of this surface. The single computation both the reader and the applier use,
    /// so a genuine full write makes their fingerprints agree and a no-op/wrong/partial write makes
    /// them differ.
    pub fn fingerprint(&self) -> Fingerprint {
        match self {
            SurfaceState::IconRef { path, index } => {
                Fingerprint::of_parts(&[b"iconref", path.as_bytes(), &index.to_le_bytes()])
            }
            SurfaceState::Folder { icon, owned_attr_bits } => Fingerprint::of_parts(&[
                b"folder",
                icon.0.as_bytes(),
                &icon.1.to_le_bytes(),
                &owned_attr_bits.to_le_bytes(),
            ]),
            SurfaceState::RegularFile { wrapper, owned_attr_bits } => {
                let present = [u8::from(wrapper.is_some())];
                match wrapper {
                    Some(w) => Fingerprint::of_parts(&[
                        b"regfile",
                        &present,
                        w.icon.0.as_bytes(),
                        &w.icon.1.to_le_bytes(),
                        w.target.as_bytes(),
                        w.working_dir.as_bytes(),
                        &owned_attr_bits.to_le_bytes(),
                    ]),
                    None => Fingerprint::of_parts(&[b"regfile", &present, &owned_attr_bits.to_le_bytes()]),
                }
            }
            SurfaceState::RecycleBin { default, empty, full } => Fingerprint::of_parts(&[
                b"recyclebin",
                default.0.as_bytes(),
                &default.1.to_le_bytes(),
                empty.0.as_bytes(),
                &empty.1.to_le_bytes(),
                full.0.as_bytes(),
                &full.1.to_le_bytes(),
            ]),
        }
    }
}

/// The `FILE_ATTRIBUTE_*` bits the RegularFile wrapper writer owns on the ORIGINAL file; the reader
/// masks the live attributes to these before building [`SurfaceState::RegularFile`] (P2-2).
pub const OWNED_ATTR_BITS: u32 = 0x02 /* HIDDEN */ | 0x04 /* SYSTEM */;

/// The `FILE_ATTRIBUTE_*` bits the folder writer owns on the FOLDER itself: `READONLY`, which
/// Explorer requires before it honours a `desktop.ini` (`FolderIconWriter` sets it). The reader
/// masks the live folder attributes to this before building [`SurfaceState::Folder`].
pub const FOLDER_OWNED_ATTR_BITS: u32 = 0x01 /* READONLY */;

/// The working directory a loose-file wrapper `.lnk` is given: the file's parent directory. The
/// SINGLE derivation both the applier ([`crate::apply::file_wrapper`]) and this module's
/// [`expected_after_apply`] use, so the intended working dir can never drift between what apply
/// writes and what the driver expects to read back.
pub fn wrapper_working_dir(file_path: &str) -> String {
    Path::new(file_path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The surface an apply SHOULD establish for `kind`, given the item's own path (needed for the
/// RegularFile wrapper's target/working-dir) and the primary icon path (and, for the Recycle Bin,
/// the paired empty icon). This is the applier's `expected`; the reader must read a [`SurfaceState`]
/// equal to it after a genuine write. Host-tested so the apply-side dispatch is pinned (P1-1/P3-1).
pub fn expected_after_apply(
    kind: ItemKind,
    item_path: &str,
    primary_icon: &str,
    empty_icon: Option<&str>,
) -> SurfaceState {
    match kind {
        ItemKind::Folder => SurfaceState::Folder {
            icon: (primary_icon.to_string(), 0),
            owned_attr_bits: FOLDER_OWNED_ATTR_BITS,
        },
        ItemKind::RegularFile => SurfaceState::RegularFile {
            wrapper: Some(WrapperSurface {
                icon: (primary_icon.to_string(), 0),
                target: item_path.to_string(),
                working_dir: wrapper_working_dir(item_path),
            }),
            owned_attr_bits: OWNED_ATTR_BITS,
        },
        ItemKind::RecycleBin => SurfaceState::RecycleBin {
            default: (primary_icon.to_string(), 0),
            empty: (empty_icon.unwrap_or_default().to_string(), 0),
            full: (primary_icon.to_string(), 0),
        },
        // Shortcut, UrlShortcut, AppxShortcut (an ordinary `.lnk`) and any other icon-location kind.
        _ => SurfaceState::IconRef { path: primary_icon.to_string(), index: 0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARCHIVE: u32 = 0x20;

    fn fp(kind: ItemKind, item_path: &str, icon: &str, empty: Option<&str>) -> Fingerprint {
        expected_after_apply(kind, item_path, icon, empty).fingerprint()
    }

    #[test]
    fn icon_ref_is_asset_derivable_and_distinguishes_paths() {
        // The applier derives this from the asset; the reader from the live location. A stale/other
        // icon must fingerprint differently, so a read-back that still shows the original fails
        // verify against the asset-derived expected (P1-1).
        let a = fp(ItemKind::Shortcut, r"C:\Desktop\App.lnk", r"C:\gen\styleA.ico", None);
        assert_eq!(a, fp(ItemKind::Shortcut, r"C:\Desktop\App.lnk", r"C:\gen\styleA.ico", None));
        assert_ne!(a, fp(ItemKind::Shortcut, r"C:\Desktop\App.lnk", r"C:\gen\styleB.ico", None));
        assert_ne!(a, SurfaceState::IconRef { path: r"C:\gen\styleA.ico".into(), index: 1 }.fingerprint());
        assert_ne!(a, SurfaceState::IconRef { path: r"C:\Windows\System32\imageres.dll".into(), index: 3 }.fingerprint());
    }

    #[test]
    fn appx_shortcut_uses_the_icon_ref_surface_like_any_lnk() {
        // Spec 06 §6: a UWP shortcut's desktop entry is an ordinary `.lnk`, styled exactly like a
        // Shortcut — so it dispatches to the IconRef surface, not a rejected/other one.
        assert!(matches!(
            expected_after_apply(ItemKind::AppxShortcut, r"C:\Desktop\Store.lnk", "a.ico", None),
            SurfaceState::IconRef { .. }
        ));
        assert_eq!(
            fp(ItemKind::AppxShortcut, r"C:\Desktop\Store.lnk", "a.ico", None),
            fp(ItemKind::Shortcut, r"C:\Desktop\Store.lnk", "a.ico", None),
        );
    }

    #[test]
    fn dispatch_picks_the_right_surface_per_kind() {
        // P3-1: the per-kind choice is host-tested, so reverting a dispatch arm goes red.
        assert!(matches!(expected_after_apply(ItemKind::Shortcut, "p", "a.ico", None), SurfaceState::IconRef { .. }));
        assert!(matches!(expected_after_apply(ItemKind::UrlShortcut, "p", "a.ico", None), SurfaceState::IconRef { .. }));
        assert!(matches!(expected_after_apply(ItemKind::Folder, "p", "a.ico", None), SurfaceState::Folder { .. }));
        assert!(matches!(expected_after_apply(ItemKind::RegularFile, "p", "a.ico", None), SurfaceState::RegularFile { .. }));
        assert!(matches!(expected_after_apply(ItemKind::RecycleBin, "p", "f.ico", Some("e.ico")), SurfaceState::RecycleBin { .. }));
    }

    #[test]
    fn folder_surface_covers_the_readonly_bit_explorer_needs() {
        // P1-#1: a writer that sets the desktop.ini icon but omits the folder READONLY bit leaves an
        // icon Explorer will not display. The surface includes the owned folder attr, so that partial
        // write fingerprints differently from the expected (READONLY set).
        let expected = fp(ItemKind::Folder, r"C:\Desktop\Reports", r"C:\gen\a.ico", None);
        // A partial write: right icon, but READONLY not set.
        let partial = SurfaceState::Folder { icon: (r"C:\gen\a.ico".into(), 0), owned_attr_bits: 0 }.fingerprint();
        assert_ne!(expected, partial, "omitting the folder READONLY bit must fingerprint differently (P1-#1)");
        // The full write matches.
        let full = SurfaceState::Folder { icon: (r"C:\gen\a.ico".into(), 0), owned_attr_bits: FOLDER_OWNED_ATTR_BITS }.fingerprint();
        assert_eq!(expected, full);
    }

    #[test]
    fn folder_surface_masks_unowned_attribute_bits() {
        // P2-2: only the owned (READONLY) bit reaches the hash, so an unrelated bit flip is not a
        // false conflict.
        let a = SurfaceState::Folder { icon: (r"x.ico".into(), 0), owned_attr_bits: (FOLDER_OWNED_ATTR_BITS | ARCHIVE) & FOLDER_OWNED_ATTR_BITS };
        let b = SurfaceState::Folder { icon: (r"x.ico".into(), 0), owned_attr_bits: FOLDER_OWNED_ATTR_BITS };
        assert_eq!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn regular_file_surface_covers_wrapper_target_and_working_dir() {
        // P1-#1: a writer that sets the wrapper icon but points it at the wrong target (or wrong
        // working dir) makes a broken wrapper. Both are in the surface, so a mismatch is caught.
        let expected = fp(ItemKind::RegularFile, r"C:\Desktop\report.pdf", r"C:\gen\a.ico", None);
        // Same icon + owned attrs, but the wrapper points somewhere else.
        let wrong_target = SurfaceState::RegularFile {
            wrapper: Some(WrapperSurface {
                icon: (r"C:\gen\a.ico".into(), 0),
                target: r"C:\Desktop\SOMETHING-ELSE.pdf".into(),
                working_dir: wrapper_working_dir(r"C:\Desktop\report.pdf"),
            }),
            owned_attr_bits: OWNED_ATTR_BITS,
        }
        .fingerprint();
        assert_ne!(expected, wrong_target, "a wrong wrapper target must fingerprint differently (P1-#1)");
    }

    #[test]
    fn regular_file_styled_differs_from_unstyled() {
        let unstyled = SurfaceState::RegularFile { wrapper: None, owned_attr_bits: 0 }.fingerprint();
        let styled = fp(ItemKind::RegularFile, r"C:\Desktop\a.txt", r"C:\gen\a.ico", None);
        assert_ne!(unstyled, styled, "styled surface must fingerprint differently (P1-10)");
    }

    #[test]
    fn regular_file_ignores_bits_it_does_not_own() {
        // P2-2: the reader masks to OWNED_ATTR_BITS, so ARCHIVE never reaches the hash.
        let w = WrapperSurface { icon: ("x.ico".into(), 0), target: "t".into(), working_dir: "d".into() };
        let masked = SurfaceState::RegularFile { wrapper: Some(w.clone()), owned_attr_bits: (OWNED_ATTR_BITS | ARCHIVE) & OWNED_ATTR_BITS };
        let owned = SurfaceState::RegularFile { wrapper: Some(w.clone()), owned_attr_bits: OWNED_ATTR_BITS };
        assert_eq!(masked.fingerprint(), owned.fingerprint(), "masking off ARCHIVE leaves the fingerprint unchanged");
        let unmasked = SurfaceState::RegularFile { wrapper: Some(w), owned_attr_bits: OWNED_ATTR_BITS | ARCHIVE };
        assert_ne!(owned.fingerprint(), unmasked.fingerprint());
    }

    #[test]
    fn recyclebin_covers_default_both_paths_and_indices() {
        let base = fp(ItemKind::RecycleBin, "p", r"C:\gen\full.ico", Some(r"C:\gen\empty.ico"));
        assert_eq!(base, fp(ItemKind::RecycleBin, "p", r"C:\gen\full.ico", Some(r"C:\gen\empty.ico")));
        assert_ne!(base, fp(ItemKind::RecycleBin, "p", r"C:\gen\full.ico", Some(r"C:\gen\OTHER-empty.ico")));
        assert_ne!(base, fp(ItemKind::RecycleBin, "p", r"C:\gen\OTHER-full.ico", Some(r"C:\gen\empty.ico")));
        // P1-#1: a wrong index on any value must fingerprint differently, so a partial write that
        // lands the path but a stale index is caught.
        let wrong_index = SurfaceState::RecycleBin {
            default: (r"C:\gen\full.ico".into(), 0),
            empty: (r"C:\gen\empty.ico".into(), 7),
            full: (r"C:\gen\full.ico".into(), 0),
        }
        .fingerprint();
        assert_ne!(base, wrong_index);
        // A stale `default` (not repointed at the new full icon) must also differ.
        let stale_default = SurfaceState::RecycleBin {
            default: (r"C:\gen\STALE.ico".into(), 0),
            empty: (r"C:\gen\empty.ico".into(), 0),
            full: (r"C:\gen\full.ico".into(), 0),
        }
        .fingerprint();
        assert_ne!(base, stale_default);
    }

    #[test]
    fn wrapper_working_dir_is_the_parent_directory() {
        // Cross-platform note: on Windows this splits the backslash path; the host uses POSIX
        // semantics. Consistency is what matters — the applier and expected share THIS function, and
        // the reader reads back what the applier wrote — so we pin the shared derivation, not the OS
        // path grammar (that is `[WINDOWS-VERIFY]`).
        assert_eq!(wrapper_working_dir("/a/b/c.txt"), "/a/b");
        assert_eq!(wrapper_working_dir("nodir"), "");
    }
}
