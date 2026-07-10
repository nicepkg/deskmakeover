//! The port traits that separate the pure operations layer from the platform layer.
//!
//! The transaction driver (`dm-operations`) never touches COM, the registry, or the
//! filesystem directly — it drives these traits. `dm-windows` implements them with real
//! `windows-rs` COM behind its STA actor; Mac unit tests implement them with in-memory fakes
//! over a virtual desktop, which is what makes the whole state machine (including kill-point
//! recovery) testable without Windows.

use crate::asset::AssetRef;
use crate::error::PortResult;
use crate::fingerprint::Fingerprint;
use crate::item::{DesktopItem, ItemTarget};
use crate::restore::RestoreAnchor;

/// Reads the current on-disk/registry state of an item: its fingerprint (the CAS anchor) and
/// its exact-restore material. Both are captured against the *live* item, so the driver can
/// detect external modification and can always walk back to the true original.
pub trait ItemStateReader {
    /// The fingerprint of the item's current state (spec 07 §4 identity).
    fn read_fingerprint(&self, target: &ItemTarget) -> PortResult<Fingerprint>;

    /// Captures the exact-restore anchor for the item BEFORE any mutation
    /// (oracle: `RestoreMetadataCollector`).
    fn capture_anchor(&self, target: &ItemTarget) -> PortResult<RestoreAnchor>;
}

/// Applies owned fields to one item (points it at a generated asset) and restores it from a
/// previously captured anchor. Dispatch by [`ItemTarget::kind`] happens inside the impl.
///
/// Ported from the per-kind writers (`ShellLinkShortcutIconWriter`, `UrlShortcutIconWriter`,
/// `FolderIconWriter`, `RegularFileWrapperWriter`, `RecycleBinIconWriter`) unified behind the
/// journaled-operation contract (`IJournaledOperation.Apply`/`Rollback`).
pub trait IconApplier {
    /// Points `target` at `asset` (e.g. `IShellLink::SetIconLocation` + `IPersistFile::Save`) and
    /// returns the fingerprint the item's styleable surface should now carry for THIS asset — the
    /// achieved-state fingerprint the driver's verify compares the live re-read against (spec 07
    /// §5). Returning the achieved fingerprint (not `()`) is what lets the driver confirm the apply
    /// matched the *requested* asset, rather than merely observing "the state changed" (P1-4).
    fn apply(&self, target: &ItemTarget, asset: &AssetRef) -> PortResult<Fingerprint>;

    /// Restores `target` to the captured original (e.g. replay the original `.lnk` bytes).
    fn restore(&self, target: &ItemTarget, anchor: &RestoreAnchor) -> PortResult<()>;
}

/// Materializes generated `.ico` bytes into a content-addressed store and garbage-collects
/// entries no ledger references (spec 07 §5). `put` is idempotent: an identical asset is
/// reused rather than rewritten.
pub trait AssetStore {
    /// Writes `bytes` under `hash`, returning the reference (new-file-first semantics).
    fn put(&self, hash: &str, bytes: &[u8]) -> PortResult<AssetRef>;

    /// Deletes any stored asset whose hash is not in `live`.
    fn gc(&self, live: &[String]) -> PortResult<()>;
}

/// Enumerates the desktop (user + public) into classified items (oracle: `DesktopScanner`).
pub trait DesktopScanner {
    fn scan(&self) -> PortResult<Vec<DesktopItem>>;
}

/// Nudges Explorer to re-read icons without a disruptive restart
/// (oracle: `ExplorerRefresh.NotifyIconsChanged` → `SHChangeNotify`).
pub trait ExplorerRefresher {
    fn notify_icons_changed(&self) -> PortResult<()>;
}

/// The privileged global shortcut-overlay verb pair (ADR-0021), invoked out-of-process via the
/// elevated helper. The background process NEVER calls this (ADR-0020 §4).
pub trait OverlayControl {
    /// Applies the overlay (one batched UAC). `ico_path` is the rendered overlay `.ico` (the
    /// caller owns the icon core); the helper validates it and copies it into ProgramData before
    /// the registry ever references it (LPE guard, ADR-0021 §4).
    fn apply(&self, style: OverlayStyle, ico_path: &str) -> PortResult<OverlayOutcome>;
    /// Restores the exact original overlay registry state.
    fn restore(&self) -> PortResult<OverlayOutcome>;
}

/// The overlay styles the helper accepts (oracle: `OverlayCommands.Apply` style whitelist).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayStyle {
    Refined,
    Transparent,
    Custom,
}

/// The result of an overlay verb; `Declined` maps a UAC cancel (oracle: `OverlayOutcome`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayOutcome {
    Applied,
    Declined,
    Failed,
}
