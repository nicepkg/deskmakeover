//! `dm-domain` — the platform-agnostic kernel shared by the operations and platform layers.
//!
//! Contains only data types and port traits: item identity ([`item`]), content fingerprints
//! ([`fingerprint`]), exact-restore anchors ([`restore`]), the opaque generated-asset
//! reference ([`asset`]), the platform port traits ([`ports`]), and typed cross-boundary
//! errors ([`error`]). No I/O and no C-compiling dependencies, so this crate cross-checks
//! cleanly for `x86_64-pc-windows-msvc`.

pub mod asset;
pub mod error;
pub mod fingerprint;
pub mod item;
pub mod ports;
pub mod restore;

pub use asset::{AssetRef, OwnedFields};
pub use error::{PortError, PortResult};
pub use fingerprint::Fingerprint;
pub use item::{DesktopItem, IconRef, IconSourceKind, ItemId, ItemKind, ItemState, ItemTarget};
pub use ports::{
    AssetStore, DesktopScanner, ExplorerRefresher, IconApplier, ItemStateReader, OverlayControl,
    OverlayOutcome, OverlayStyle,
};
pub use restore::{
    DesktopIniAnchor, RecycleBinAnchor, RegistryValue, RestoreAnchor, WrapperAnchor,
};
