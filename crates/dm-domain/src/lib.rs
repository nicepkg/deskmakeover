//! `dm-domain` — the platform-agnostic kernel shared by the operations and platform layers.
//!
//! Contains only data types and port traits: item identity ([`item`]), content fingerprints
//! ([`fingerprint`]), exact-restore anchors ([`restore`]), the opaque generated-asset
//! reference ([`asset`]), the platform port traits ([`ports`]), and typed cross-boundary
//! errors ([`error`]). No I/O and no C-compiling dependencies, so this crate cross-checks
//! cleanly for `x86_64-pc-windows-msvc`.

pub mod asset;
pub mod elevated;
pub mod error;
pub mod fingerprint;
pub mod item;
pub mod ports;
pub mod restore;
pub mod source;
pub mod system_tweaks;
pub mod wallpaper;

pub use asset::{ApplyAssets, AssetRef, OwnedFields};
pub use elevated::{
    ElevatedApplyItem, ElevatedIconApplier, ElevatedOutcome, ElevatedRestoreItem,
};
pub use error::{PortError, PortResult};
pub use fingerprint::Fingerprint;
pub use item::{DesktopItem, IconRef, IconSourceKind, ItemId, ItemKind, ItemState, ItemTarget};
pub use source::{
    FileId, IconLocationState, PackageState, SourceFingerprint, SourceIdentity, TargetState,
};
pub use ports::{
    ActivityMonitor, AssetStore, DesktopGeometry, DesktopGeometryReader, DesktopIconGrid, DesktopIconSlot,
    DesktopScanner,
    ExplorerRefresher, IconApplier, IconSourceExtractor, ImageDecoder, ItemStateReader,
    MonitorTopology, OverlayControl, OverlayOutcome, OverlayStyle, WallpaperApplier,
};
pub use restore::{
    DesktopIniAnchor, PriorWrapper, RecycleBinAnchor, RegistryValue, RestoreAnchor,
    SystemIconAnchor, WrapperAnchor,
};
pub use wallpaper::{
    DecodedImage, MonitorInfo, MonitorRect, MonitorWallpaper, Orientation, WallpaperPosition,
    WallpaperSnapshot, WallpaperTopology,
};
