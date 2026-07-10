//! The **styleable surface** — the icon reference an apply actually writes — and its fingerprint.
//!
//! One host-tested source of truth consumed by BOTH sides of the apply/verify handshake:
//! * [`WindowsStateReader::read_fingerprint`] gathers the live surface off disk/registry into a
//!   [`SurfaceState`] and fingerprints it (the CAS anchor + the driver's read-back);
//! * [`WindowsIconApplier::apply`] builds the surface it INTENDS from the asset via
//!   [`expected_after_apply`] and fingerprints that (the driver's `expected`).
//!
//! Because both call [`SurfaceState::fingerprint`], the two can never disagree on the *computation*
//! — only on the raw values they feed, which is the `[WINDOWS-VERIFY]` I/O. This closes three
//! holes at once: the fingerprint is derivable from the asset, so the applier's `expected` needs no
//! tautological self-re-read (P1-1); it covers what apply changes and nothing else, so a styled
//! surface differs from an unstyled one (P1-10) and an unrelated attribute flip is not a false
//! conflict (P2-2). The per-kind dispatch and the fingerprint math are unit-tested on the Mac host
//! (P3-1); only the disk/registry reads are deferred to Windows.

use dm_domain::{Fingerprint, ItemKind};

/// The exact state an icon writer establishes — the field(s) it owns, nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceState {
    /// `.lnk`/`.url`/folder: the icon location (path + index) the writer points the item at.
    IconRef { path: String, index: i32 },
    /// Loose file: the companion wrapper's icon location (`None` if no wrapper) plus ONLY the
    /// attribute bits apply owns (`HIDDEN | SYSTEM`) — masked so a backup tool clearing `ARCHIVE`
    /// or an OS offline bit is not read as an external edit (P2-2).
    RegularFile { wrapper_icon: Option<(String, i32)>, owned_attr_bits: u32 },
    /// Recycle Bin: the empty + full icon paths the `DefaultIcon` values point at.
    RecycleBin { empty: String, full: String },
}

impl SurfaceState {
    /// The fingerprint of this surface. The single computation both the reader and the applier use,
    /// so a genuine write makes their fingerprints agree and a no-op/wrong-asset write makes them
    /// differ.
    pub fn fingerprint(&self) -> Fingerprint {
        match self {
            SurfaceState::IconRef { path, index } => {
                Fingerprint::of_parts(&[path.as_bytes(), &index.to_le_bytes()])
            }
            SurfaceState::RegularFile { wrapper_icon, owned_attr_bits } => {
                let present = [u8::from(wrapper_icon.is_some())];
                match wrapper_icon {
                    Some((path, index)) => Fingerprint::of_parts(&[
                        &present,
                        path.as_bytes(),
                        &index.to_le_bytes(),
                        &owned_attr_bits.to_le_bytes(),
                    ]),
                    None => Fingerprint::of_parts(&[&present, &owned_attr_bits.to_le_bytes()]),
                }
            }
            SurfaceState::RecycleBin { empty, full } => {
                Fingerprint::of_parts(&[empty.as_bytes(), full.as_bytes()])
            }
        }
    }
}

/// The `FILE_ATTRIBUTE_*` bits the RegularFile wrapper writer owns; the reader masks the live
/// attributes to these before building [`SurfaceState::RegularFile`] (P2-2).
pub const OWNED_ATTR_BITS: u32 = 0x02 /* HIDDEN */ | 0x04 /* SYSTEM */;

/// The surface an apply SHOULD establish for `kind`, given the primary icon path (and, for the
/// Recycle Bin, the paired empty icon). This is the applier's `expected`; the reader must read a
/// [`SurfaceState`] equal to it after a genuine write. Host-tested so the apply-side dispatch is
/// pinned (P1-1/P3-1).
pub fn expected_after_apply(kind: ItemKind, primary_icon: &str, empty_icon: Option<&str>) -> SurfaceState {
    match kind {
        ItemKind::RegularFile => SurfaceState::RegularFile {
            wrapper_icon: Some((primary_icon.to_string(), 0)),
            owned_attr_bits: OWNED_ATTR_BITS,
        },
        ItemKind::RecycleBin => SurfaceState::RecycleBin {
            empty: empty_icon.unwrap_or_default().to_string(),
            full: primary_icon.to_string(),
        },
        // Shortcut, UrlShortcut, Folder (and any other icon-location kind).
        _ => SurfaceState::IconRef { path: primary_icon.to_string(), index: 0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARCHIVE: u32 = 0x20;

    fn fp(kind: ItemKind, icon: &str, empty: Option<&str>) -> Fingerprint {
        expected_after_apply(kind, icon, empty).fingerprint()
    }

    #[test]
    fn icon_ref_is_asset_derivable_and_distinguishes_paths() {
        // The applier derives this from the asset; the reader from the live location. A stale/other
        // icon must fingerprint differently, so a read-back that still shows the original fails
        // verify against the asset-derived expected (P1-1).
        let a = fp(ItemKind::Shortcut, r"C:\gen\styleA.ico", None);
        assert_eq!(a, fp(ItemKind::Shortcut, r"C:\gen\styleA.ico", None));
        assert_ne!(a, fp(ItemKind::Shortcut, r"C:\gen\styleB.ico", None));
        assert_ne!(a, SurfaceState::IconRef { path: r"C:\gen\styleA.ico".into(), index: 1 }.fingerprint());
        assert_ne!(a, SurfaceState::IconRef { path: r"C:\Windows\System32\imageres.dll".into(), index: 3 }.fingerprint());
    }

    #[test]
    fn dispatch_picks_the_right_surface_per_kind() {
        // P3-1: the per-kind choice is host-tested, so reverting a dispatch arm goes red.
        assert!(matches!(expected_after_apply(ItemKind::Shortcut, "a.ico", None), SurfaceState::IconRef { .. }));
        assert!(matches!(expected_after_apply(ItemKind::UrlShortcut, "a.ico", None), SurfaceState::IconRef { .. }));
        assert!(matches!(expected_after_apply(ItemKind::Folder, "a.ico", None), SurfaceState::IconRef { .. }));
        assert!(matches!(expected_after_apply(ItemKind::RegularFile, "a.ico", None), SurfaceState::RegularFile { .. }));
        assert!(matches!(expected_after_apply(ItemKind::RecycleBin, "f.ico", Some("e.ico")), SurfaceState::RecycleBin { .. }));
    }

    #[test]
    fn regular_file_styled_differs_from_unstyled() {
        let unstyled = SurfaceState::RegularFile { wrapper_icon: None, owned_attr_bits: 0 }.fingerprint();
        let styled = fp(ItemKind::RegularFile, r"C:\gen\a.ico", None);
        assert_ne!(unstyled, styled, "styled surface must fingerprint differently (P1-10)");
    }

    #[test]
    fn regular_file_ignores_bits_it_does_not_own() {
        // P2-2: the reader masks to OWNED_ATTR_BITS, so ARCHIVE never reaches the hash.
        let masked_with_archive = (OWNED_ATTR_BITS | ARCHIVE) & OWNED_ATTR_BITS;
        let a = SurfaceState::RegularFile { wrapper_icon: Some(("x.ico".into(), 0)), owned_attr_bits: masked_with_archive };
        let b = SurfaceState::RegularFile { wrapper_icon: Some(("x.ico".into(), 0)), owned_attr_bits: OWNED_ATTR_BITS };
        assert_eq!(a.fingerprint(), b.fingerprint(), "masking off ARCHIVE leaves the fingerprint unchanged");
        // The mask is load-bearing: an unmasked ARCHIVE WOULD differ.
        let unmasked = SurfaceState::RegularFile { wrapper_icon: Some(("x.ico".into(), 0)), owned_attr_bits: OWNED_ATTR_BITS | ARCHIVE };
        assert_ne!(b.fingerprint(), unmasked.fingerprint());
    }

    #[test]
    fn recyclebin_distinguishes_both_icon_paths() {
        let base = fp(ItemKind::RecycleBin, r"C:\gen\full.ico", Some(r"C:\gen\empty.ico"));
        assert_eq!(base, fp(ItemKind::RecycleBin, r"C:\gen\full.ico", Some(r"C:\gen\empty.ico")));
        assert_ne!(base, fp(ItemKind::RecycleBin, r"C:\gen\full.ico", Some(r"C:\gen\OTHER-empty.ico")));
        assert_ne!(base, fp(ItemKind::RecycleBin, r"C:\gen\OTHER-full.ico", Some(r"C:\gen\empty.ico")));
    }
}
