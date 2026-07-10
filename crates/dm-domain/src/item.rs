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
    /// A Store/AppX shortcut — no safe reversible write, never styled.
    AppxShortcut,
    /// The Recycle Bin virtual item — icon via per-user `DefaultIcon` registry values.
    RecycleBin,
    /// A folder — icon via a `desktop.ini` `IconResource`.
    Folder,
    /// A loose file — styled by a companion wrapper `.lnk` with the original hidden.
    RegularFile,
    /// A generic system virtual item styled through registry `DefaultIcon` values.
    System,
    /// Anything that could not be read or has no reversible write.
    Unsupported,
}

impl ItemKind {
    /// Whether this kind has a reversible styling write (mirrors `DesktopBakeService.CanStyle`
    /// minus the state check, which lives on [`DesktopItem`]).
    pub fn is_styleable(self) -> bool {
        matches!(
            self,
            ItemKind::Shortcut
                | ItemKind::UrlShortcut
                | ItemKind::Folder
                | ItemKind::RegularFile
                | ItemKind::RecycleBin
        )
    }

    /// Real shortcuts (`.lnk`/`.url`) carry the arrow/mark; everything else is styled without one.
    pub fn is_shortcut(self) -> bool {
        matches!(self, ItemKind::Shortcut | ItemKind::UrlShortcut)
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

    /// Whether this item may be styled right now (kind is reversible AND state is ready).
    pub fn can_style(&self) -> bool {
        self.state == ItemState::Ready && self.kind.is_styleable()
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
        for kind in [
            ItemKind::Shortcut,
            ItemKind::UrlShortcut,
            ItemKind::Folder,
            ItemKind::RegularFile,
            ItemKind::RecycleBin,
        ] {
            assert!(kind.is_styleable(), "{kind:?} should be styleable");
        }
        assert!(!ItemKind::AppxShortcut.is_styleable());
        assert!(!ItemKind::Unsupported.is_styleable());
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
}
