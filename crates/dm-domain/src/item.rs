//! Desktop item identity, kind, and the minimal addressing record the operations and
//! platform layers share.
//!
//! Harvested from the frozen C# oracle: `DeskMakeover.Core/DesktopItem.cs`
//! (`DesktopItem`, `DesktopItemKind`, `DesktopItemState`) and `IconSource.cs`, plus the
//! stable-id derivation in `DeskMakeover.Shell/DesktopScanner.cs`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A stable desktop-item id: the lowercased hex of the first 8 bytes of
/// `SHA-256("<source>:<UPPERCASE(path)>")`.
///
/// Ported verbatim from `FileSystemDesktopItemSource.StableId` so a Rust scan produces the
/// same ids the frozen oracle did (ledger entries stay addressable across the port). Windows
/// paths are case-insensitive, hence the uppercasing before hashing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ItemId(String);

impl ItemId {
    /// Wraps an already-computed id string (e.g. read back from the ledger).
    pub fn from_raw(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Derives the stable id for a filesystem item exactly as the oracle scanner did.
    pub fn from_source_path(source: &str, path: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(format!("{source}:{}", path.to_uppercase()).as_bytes());
        let digest = hasher.finalize();
        let hex = digest[..8].iter().fold(String::with_capacity(16), |mut acc, b| {
            acc.push_str(&format!("{b:02x}"));
            acc
        });
        Self(hex)
    }

    /// The underlying id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The kind of desktop item, mirroring `DesktopItemKind`. Each kind maps to a reversible
/// write mechanism (or to `Unsupported`, which is never touched).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    /// A `.lnk` shell shortcut — icon via `IShellLink::SetIconLocation`.
    Shortcut,
    /// A `.url` internet shortcut — icon via the `[InternetShortcut]` `IconFile`/`IconIndex`.
    UrlShortcut,
    /// A Store/UWP shortcut. Its desktop entry is an ordinary `.lnk`, so it is styled exactly
    /// like any shortcut — `IconLocation` write + full-bytes restore. Only the PACKAGE asset is
    /// immutable, which does not block masking the shortcut (spec 06 §6, owner-prototype-proven
    /// 2026-07-09).
    AppxShortcut,
    /// The Recycle Bin virtual item — icon via per-user `DefaultIcon` registry values.
    RecycleBin,
    /// A folder — icon via a `desktop.ini` `IconResource`.
    Folder,
    /// A loose file — styled by a companion wrapper `.lnk` with the original hidden.
    RegularFile,
    /// A system virtual item (This PC / Network / User Files / Control Panel) styled via the
    /// per-user CLSID `DefaultIcon` values — the same HKCU mechanism the Recycle Bin uses, hence
    /// styleable (spec 06 §6; an early dev-mock that classified these Unsupported was a mistake).
    System,
    /// Anything that could not be read or has no reversible write.
    Unsupported,
}

impl ItemKind {
    /// Whether this kind has a reversible styling write (mirrors `DesktopBakeService.CanStyle`
    /// minus the state check, which lives on [`DesktopItem`]). Per spec 06 §6 nothing on the
    /// desktop is un-styleable except genuinely broken/[`Unsupported`](ItemKind::Unsupported)
    /// items: AppxShortcut is an ordinary `.lnk`, and System uses the same HKCU `DefaultIcon`
    /// mechanism as the Recycle Bin.
    pub fn is_styleable(self) -> bool {
        !matches!(self, ItemKind::Unsupported)
    }

    /// Shortcuts (`.lnk`/`.url`/UWP) carry the arrow/mark; everything else is styled without one.
    /// A UWP shortcut's desktop entry is an ordinary `.lnk`, so it must wear the mark too
    /// (spec 06 §6.5).
    pub fn is_shortcut(self) -> bool {
        matches!(self, ItemKind::Shortcut | ItemKind::UrlShortcut | ItemKind::AppxShortcut)
    }
}

/// The readiness of an item for styling, mirroring `DesktopItemState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemState {
    Ready,
    PreviewOnly,
    RequiresConsent,
    Unsupported,
    Error,
}

/// Where an item's original icon comes from, mirroring `Core/IconSource.cs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconRef {
    pub kind: IconSourceKind,
    pub location: String,
    pub index: i32,
}

/// The provenance of an icon, mirroring `IconSourceKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconSourceKind {
    File,
    ExecutableResource,
    UrlShortcut,
    AppxAsset,
    SystemIcon,
    Fallback,
}

/// The minimal addressing record for one item, threaded through the transaction journal and
/// the platform ports. Kept small on purpose — the pixel/name/state details live on
/// [`DesktopItem`] and never enter the durable journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemTarget {
    pub id: ItemId,
    pub kind: ItemKind,
    pub path: String,
}

impl ItemTarget {
    pub fn new(id: ItemId, kind: ItemKind, path: impl Into<String>) -> Self {
        Self { id, kind, path: path.into() }
    }
}

/// A scanned desktop item, mirroring `Core/DesktopItem.cs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopItem {
    pub id: ItemId,
    pub name: String,
    pub path: String,
    pub kind: ItemKind,
    pub icon: Option<IconRef>,
    pub state: ItemState,
    pub requires_explicit_consent: bool,
    pub status_message: Option<String>,
}

impl DesktopItem {
    /// The addressing view of this item.
    pub fn target(&self) -> ItemTarget {
        ItemTarget::new(self.id.clone(), self.kind, self.path.clone())
    }

    /// Whether this item may be styled right now: kind is reversible, state is ready, AND it does not
    /// still need explicit consent (audit F11) — a `Ready` item flagged `requires_explicit_consent`
    /// must go through the consent path first, never be reported auto-styleable.
    pub fn can_style(&self) -> bool {
        self.state == ItemState::Ready
            && self.kind.is_styleable()
            && !self.requires_explicit_consent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_id_matches_oracle_derivation() {
        // Independently reproduce sha256("filesystem:C:\\USERS\\X\\DESKTOP\\APP.LNK")[..8].
        let id = ItemId::from_source_path("filesystem", r"C:\Users\x\Desktop\App.lnk");
        let mut hasher = Sha256::new();
        hasher.update(br"filesystem:C:\USERS\X\DESKTOP\APP.LNK");
        let digest = hasher.finalize();
        let expected: String =
            digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(id.as_str(), expected);
        assert_eq!(id.as_str().len(), 16);
    }

    #[test]
    fn stable_id_is_case_insensitive_on_path() {
        let a = ItemId::from_source_path("filesystem", r"C:\Desktop\App.lnk");
        let b = ItemId::from_source_path("filesystem", r"c:\desktop\app.LNK");
        assert_eq!(a, b);
    }

    #[test]
    fn styleable_kinds_match_oracle_can_style() {
        // Spec 06 §6: nothing on the desktop is un-styleable except Unsupported. AppxShortcut is an
        // ordinary `.lnk`, and System uses the same HKCU DefaultIcon mechanism as the Recycle Bin
        // (an early dev-mock that classified either as un-styleable was corrected 2026-07-09).
        for kind in [
            ItemKind::Shortcut,
            ItemKind::UrlShortcut,
            ItemKind::Folder,
            ItemKind::RegularFile,
            ItemKind::RecycleBin,
            ItemKind::AppxShortcut,
            ItemKind::System,
        ] {
            assert!(kind.is_styleable(), "{kind:?} should be styleable");
        }
        assert!(!ItemKind::Unsupported.is_styleable(), "only genuinely broken items are un-styleable");
    }

    #[test]
    fn can_style_requires_ready_state() {
        let mut item = DesktopItem {
            id: ItemId::from_raw("abc"),
            name: "App".into(),
            path: r"C:\Desktop\App.lnk".into(),
            kind: ItemKind::Shortcut,
            icon: None,
            state: ItemState::Ready,
            requires_explicit_consent: false,
            status_message: None,
        };
        assert!(item.can_style());
        item.state = ItemState::Error;
        assert!(!item.can_style());
    }

    #[test]
    fn item_id_is_deterministic_for_unicode_and_very_long_paths() {
        let unicode = r"C:\Users\李明\桌面\日本語 📁.lnk";
        assert_eq!(ItemId::from_source_path("filesystem", unicode), ItemId::from_source_path("filesystem", unicode));
        // A pathological 4k path must still produce a fixed 16-hex-char id.
        let long = format!(r"C:\{}\x.lnk", "a".repeat(4000));
        let id = ItemId::from_source_path("filesystem", &long);
        assert_eq!(id.as_str().len(), 16);
        // Different source namespaces never collide for the same path.
        assert_ne!(
            ItemId::from_source_path("filesystem", unicode),
            ItemId::from_source_path("shell", unicode)
        );
    }

    #[test]
    fn item_target_and_icon_ref_round_trip() {
        let target = ItemTarget::new(ItemId::from_raw("abc"), ItemKind::Folder, r"C:\D\Reports");
        let back: ItemTarget =
            serde_json::from_str(&serde_json::to_string(&target).unwrap()).unwrap();
        assert_eq!(target, back);

        for kind in [
            IconSourceKind::File,
            IconSourceKind::ExecutableResource,
            IconSourceKind::UrlShortcut,
            IconSourceKind::AppxAsset,
            IconSourceKind::SystemIcon,
            IconSourceKind::Fallback,
        ] {
            let icon = IconRef { kind, location: "x.ico".into(), index: -3 };
            let back: IconRef = serde_json::from_str(&serde_json::to_string(&icon).unwrap()).unwrap();
            assert_eq!(icon, back);
        }
    }

    #[test]
    fn can_style_covers_every_kind_at_ready() {
        let styleable = [
            ItemKind::Shortcut,
            ItemKind::UrlShortcut,
            ItemKind::Folder,
            ItemKind::RegularFile,
            ItemKind::RecycleBin,
            ItemKind::AppxShortcut,
            ItemKind::System,
        ];
        let not_styleable = [ItemKind::Unsupported];
        for kind in styleable {
            assert!(mk(kind, ItemState::Ready).can_style(), "{kind:?} should style when ready");
        }
        for kind in not_styleable {
            assert!(!mk(kind, ItemState::Ready).can_style(), "{kind:?} must not style");
        }
        // Even a styleable kind cannot style outside the Ready state.
        for state in [ItemState::PreviewOnly, ItemState::RequiresConsent, ItemState::Unsupported, ItemState::Error] {
            assert!(!mk(ItemKind::Shortcut, state).can_style());
        }
    }

    #[test]
    fn appx_and_system_are_styleable_and_appx_wears_the_mark() {
        // P1-12 / spec 06 §6+§6.5: the dev-mock wrongly treated AppxShortcut and System as
        // un-styleable, and AppxShortcut as a non-shortcut. Both are styleable now, and a UWP
        // shortcut wears the mark like any other shortcut.
        assert!(ItemKind::AppxShortcut.is_styleable(), "a UWP shortcut is an ordinary .lnk");
        assert!(ItemKind::System.is_styleable(), "System uses the HKCU DefaultIcon mechanism");
        assert!(ItemKind::AppxShortcut.is_shortcut(), "UWP shortcuts must wear the mark");
        // System is styleable but is NOT a shortcut (a virtual item, no arrow).
        assert!(!ItemKind::System.is_shortcut());
    }

    #[test]
    fn is_shortcut_covers_lnk_url_and_uwp() {
        // Spec 06 §6.5: a UWP shortcut's desktop entry is an ordinary `.lnk`, so it must wear the
        // mark too — is_shortcut includes AppxShortcut.
        for kind in [ItemKind::Shortcut, ItemKind::UrlShortcut, ItemKind::AppxShortcut] {
            assert!(kind.is_shortcut(), "{kind:?} carries the shortcut mark");
        }
        for kind in [ItemKind::Folder, ItemKind::RegularFile, ItemKind::RecycleBin, ItemKind::System] {
            assert!(!kind.is_shortcut(), "{kind:?} is not a shortcut");
        }
    }

    fn mk(kind: ItemKind, state: ItemState) -> DesktopItem {
        DesktopItem {
            id: ItemId::from_raw("id"),
            name: "n".into(),
            path: "p".into(),
            kind,
            icon: None,
            state,
            requires_explicit_consent: false,
            status_message: None,
        }
    }
}
