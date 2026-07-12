//! Source identity: what an item's icon SHOULD look like (spec 07 §4).
//!
//! The source fingerprint covers MORE than a shortcut's raw bytes. A target-app update with an
//! unchanged `.lnk` still changes the icon, so identity folds in the target's version/mtime, the
//! state of the file the icon points at, and a UWP package's version + chosen resource variant.
//!
//! This is DISTINCT from the post-apply CAS fingerprint (`LedgerEntry.last_applied_fingerprint`):
//! - the SOURCE fingerprint (here) decides whether the resident watcher must (re)format an item at
//!   all — did what-it-should-look-like change?
//! - the CAS fingerprint decides whether an apply would clobber a user's own edit — did OUR applied
//!   state change out from under us?
//!
//! Self-write suppression — the other half of §4 (our own applies must never re-enter the format
//! queue) — is stateful reconciler logic and lives with the resident reconciler (M7 plan T6), which
//! consumes this fingerprint; it is deliberately not built into this pure identity type.

use serde::{Deserialize, Serialize};

use crate::fingerprint::Fingerprint;
use crate::item::ItemKind;

/// A source fingerprint (spec 07 §4), wrapped in a distinct newtype so it can NEVER be passed where
/// a CAS fingerprint (`ApplyRequest.expected_fingerprint` / `LedgerEntry.last_applied_fingerprint`)
/// is expected. The two answer different questions — "did what the icon should look like change?"
/// vs "did OUR applied state change out from under us?" — and silently mixing them would cause false
/// conflicts or break CAS. Serializes as its lowercase-hex string (for the resident's
/// change-detection cache).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceFingerprint(Fingerprint);

impl SourceFingerprint {
    /// The lowercase-hex rendering.
    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }

    /// Parses a lowercase-hex source fingerprint; `None` if malformed.
    pub fn from_hex(hex: &str) -> Option<Self> {
        Fingerprint::from_hex(hex).map(Self)
    }
}

/// A file's on-volume identity (`volume serial` + `file id`), used to tell a RENAME (same identity,
/// new path) from a REPLACE (new file at the same path). `None` when the platform cannot read it
/// (a virtual item, or a filesystem without stable ids).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileId {
    pub volume: u64,
    pub index: u64,
}

/// The state of the file an item's icon currently points at (path + resource index + size + mtime),
/// so a resource DLL/EXE update — same IconLocation, new bytes — reflows the source identity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IconLocationState {
    pub location: String,
    pub index: i32,
    pub size: u64,
    pub mtime: i64,
}

/// A shortcut's target: its path plus the target file's version + mtime, so a target-app update
/// (the `.lnk` unchanged) still changes the source identity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TargetState {
    pub path: String,
    pub version: String,
    pub mtime: i64,
}

/// A UWP/Appx package's identity: the AUMID (Application User Model ID — distinguishes two apps
/// packaged in the SAME family, which share a family name + version), the package family + version,
/// and the chosen resource variant (scale / theme / contrast). A package update, an app switch
/// within one package, or a variant change all change the icon.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageState {
    pub aumid: String,
    pub family: String,
    pub version: String,
    pub resource_variant: String,
}

/// Everything that determines an item's source appearance (spec 07 §4). The platform scanner
/// populates the fields it can read; a virtual or loose item leaves the shortcut-specific ones
/// `None`. Fingerprint via [`SourceIdentity::fingerprint`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub kind: ItemKind,
    pub file_id: Option<FileId>,
    pub icon_location: Option<IconLocationState>,
    pub target: Option<TargetState>,
    pub package: Option<PackageState>,
    /// Any additional raw identity bytes (the `.url` `IconFile`/`IconIndex` text, a `desktop.ini`
    /// `IconResource`, …) the scanner folds in without a dedicated field.
    pub extra: Vec<u8>,
}

impl SourceIdentity {
    /// A bare identity for `kind` with every optional field absent (the scanner fills in what it
    /// reads). Handy for virtual items and tests.
    pub fn of_kind(kind: ItemKind) -> Self {
        Self {
            kind,
            file_id: None,
            icon_location: None,
            target: None,
            package: None,
            extra: Vec::new(),
        }
    }

    /// The 32-byte source fingerprint: a framed hash over every identity field. Two identities that
    /// agree on all fields fingerprint equal; any change — a target version bump, an icon file's
    /// mtime, a package version, a rename vs a replace — produces a different fingerprint. Each
    /// field is length-prefixed (via [`Fingerprint::of_parts`]) and every `Option` carries an
    /// explicit presence byte, so `None` never collides with `Some(<default>)` and field boundaries
    /// never merge.
    pub fn fingerprint(&self) -> SourceFingerprint {
        let mut parts: Vec<Vec<u8>> = Vec::new();
        parts.push(vec![kind_tag(self.kind)]);

        match &self.file_id {
            Some(f) => {
                parts.push(vec![1]);
                parts.push(f.volume.to_le_bytes().to_vec());
                parts.push(f.index.to_le_bytes().to_vec());
            }
            None => parts.push(vec![0]),
        }

        match &self.icon_location {
            Some(i) => {
                parts.push(vec![1]);
                parts.push(i.location.as_bytes().to_vec());
                parts.push(i.index.to_le_bytes().to_vec());
                parts.push(i.size.to_le_bytes().to_vec());
                parts.push(i.mtime.to_le_bytes().to_vec());
            }
            None => parts.push(vec![0]),
        }

        match &self.target {
            Some(t) => {
                parts.push(vec![1]);
                parts.push(t.path.as_bytes().to_vec());
                parts.push(t.version.as_bytes().to_vec());
                parts.push(t.mtime.to_le_bytes().to_vec());
            }
            None => parts.push(vec![0]),
        }

        match &self.package {
            Some(p) => {
                parts.push(vec![1]);
                parts.push(p.aumid.as_bytes().to_vec());
                parts.push(p.family.as_bytes().to_vec());
                parts.push(p.version.as_bytes().to_vec());
                parts.push(p.resource_variant.as_bytes().to_vec());
            }
            None => parts.push(vec![0]),
        }

        parts.push(self.extra.clone());

        let refs: Vec<&[u8]> = parts.iter().map(|p| p.as_slice()).collect();
        SourceFingerprint(Fingerprint::of_parts(&refs))
    }
}

/// A stable per-kind discriminant byte folded into the source fingerprint. Exhaustive on purpose:
/// adding an [`ItemKind`] variant is a compile error here until it is assigned a byte, so a new
/// kind can never silently share another's identity space.
fn kind_tag(kind: ItemKind) -> u8 {
    match kind {
        ItemKind::Unsupported => 0,
        ItemKind::Shortcut => 1,
        ItemKind::UrlShortcut => 2,
        ItemKind::AppxShortcut => 3,
        ItemKind::RecycleBin => 4,
        ItemKind::Folder => 5,
        ItemKind::RegularFile => 6,
        ItemKind::System => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shortcut() -> SourceIdentity {
        SourceIdentity {
            kind: ItemKind::Shortcut,
            file_id: Some(FileId { volume: 1, index: 42 }),
            icon_location: Some(IconLocationState {
                location: r"C:\Program Files\App\app.exe".into(),
                index: 0,
                size: 1_024,
                mtime: 1_700_000_000,
            }),
            target: Some(TargetState {
                path: r"C:\Program Files\App\app.exe".into(),
                version: "1.2.3".into(),
                mtime: 1_700_000_000,
            }),
            package: None,
            extra: Vec::new(),
        }
    }

    #[test]
    fn identical_identities_fingerprint_equal() {
        assert_eq!(shortcut().fingerprint(), shortcut().fingerprint());
    }

    #[test]
    fn a_target_version_bump_changes_the_fingerprint_even_with_the_same_lnk() {
        // The headline §4 case: the shortcut bytes (path/icon-location) are unchanged, only the
        // target app updated — the source identity MUST differ so the item is re-formatted.
        let before = shortcut();
        let mut after = shortcut();
        after.target.as_mut().unwrap().version = "1.2.4".into();
        assert_ne!(before.fingerprint(), after.fingerprint());
    }

    #[test]
    fn an_icon_file_mtime_change_changes_the_fingerprint() {
        let before = shortcut();
        let mut after = shortcut();
        after.icon_location.as_mut().unwrap().mtime += 1;
        assert_ne!(before.fingerprint(), after.fingerprint());
    }

    #[test]
    fn a_package_version_or_variant_or_aumid_change_changes_the_fingerprint() {
        let base = SourceIdentity {
            package: Some(PackageState {
                aumid: "Contoso.App_8wekyb!App".into(),
                family: "Contoso.App_8wekyb".into(),
                version: "2.0.0.0".into(),
                resource_variant: "scale-200".into(),
            }),
            ..SourceIdentity::of_kind(ItemKind::AppxShortcut)
        };
        let mut newer = base.clone();
        newer.package.as_mut().unwrap().version = "2.0.1.0".into();
        assert_ne!(base.fingerprint(), newer.fingerprint());

        let mut variant = base.clone();
        variant.package.as_mut().unwrap().resource_variant = "scale-400".into();
        assert_ne!(base.fingerprint(), variant.fingerprint());

        // Two apps in the SAME package (same family + version) differ only by AUMID — the
        // fingerprint MUST tell them apart, or one app's icon would mask the other's.
        let mut sibling = base.clone();
        sibling.package.as_mut().unwrap().aumid = "Contoso.App_8wekyb!Helper".into();
        assert_ne!(base.fingerprint(), sibling.fingerprint());
    }

    #[test]
    fn source_fingerprint_hex_round_trips() {
        let fp = shortcut().fingerprint();
        assert_eq!(SourceFingerprint::from_hex(&fp.to_hex()), Some(fp));
        assert!(SourceFingerprint::from_hex("not-hex").is_none());
    }

    #[test]
    fn rename_versus_replace_is_distinguished_by_file_id() {
        // Same path, but a different file identity = a REPLACE (a new file), not the same item.
        let a = SourceIdentity { file_id: Some(FileId { volume: 1, index: 10 }), ..shortcut() };
        let b = SourceIdentity { file_id: Some(FileId { volume: 1, index: 11 }), ..shortcut() };
        assert_ne!(a.fingerprint(), b.fingerprint());
        // A different volume (same index) also differs.
        let c = SourceIdentity { file_id: Some(FileId { volume: 2, index: 10 }), ..shortcut() };
        assert_ne!(a.fingerprint(), c.fingerprint());
    }

    #[test]
    fn kind_participates_in_identity() {
        let a = SourceIdentity::of_kind(ItemKind::Folder);
        let b = SourceIdentity::of_kind(ItemKind::System);
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn absent_field_never_collides_with_a_present_default() {
        // `None` must not hash the same as `Some(<all-zero/empty default>)` — the presence byte +
        // length framing guarantees it.
        let none = SourceIdentity::of_kind(ItemKind::RegularFile);
        let some_default = SourceIdentity {
            icon_location: Some(IconLocationState::default()),
            ..SourceIdentity::of_kind(ItemKind::RegularFile)
        };
        assert_ne!(none.fingerprint(), some_default.fingerprint());

        let some_target = SourceIdentity {
            target: Some(TargetState::default()),
            ..SourceIdentity::of_kind(ItemKind::RegularFile)
        };
        assert_ne!(none.fingerprint(), some_target.fingerprint());
    }

    #[test]
    fn extra_bytes_participate_and_frame_without_collision() {
        let a = SourceIdentity { extra: b"IconFile=a.ico".to_vec(), ..SourceIdentity::of_kind(ItemKind::UrlShortcut) };
        let b = SourceIdentity { extra: b"IconFile=b.ico".to_vec(), ..SourceIdentity::of_kind(ItemKind::UrlShortcut) };
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn every_kind_tag_is_distinct() {
        let tags = [
            ItemKind::Unsupported,
            ItemKind::Shortcut,
            ItemKind::UrlShortcut,
            ItemKind::AppxShortcut,
            ItemKind::RecycleBin,
            ItemKind::Folder,
            ItemKind::RegularFile,
            ItemKind::System,
        ]
        .map(kind_tag);
        let mut sorted = tags.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), tags.len(), "every ItemKind must map to a distinct tag byte");
    }
}
