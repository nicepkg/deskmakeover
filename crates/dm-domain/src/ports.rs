//! The port traits that separate the pure operations layer from the platform layer.
//!
//! The transaction driver (`dm-operations`) never touches COM, the registry, or the
//! filesystem directly — it drives these traits. `dm-windows` implements them with real
//! `windows-rs` COM behind its STA actor; Mac unit tests implement them with in-memory fakes
//! over a virtual desktop, which is what makes the whole state machine (including kill-point
//! recovery) testable without Windows.

use crate::asset::{ApplyAssets, AssetRef};
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
    /// Points `target` at `assets` (e.g. `IShellLink::SetIconLocation` + `IPersistFile::Save`) and
    /// returns the fingerprint the item's styleable surface SHOULD now carry — derived from the
    /// asset the apply was asked to point at, INDEPENDENTLY of re-reading the just-written state.
    /// The driver compares this against an independent read-back (spec 07 §5), so a COM write that
    /// reports success but silently leaves the old icon is caught (P1-1/P1-4), not merely a "state
    /// changed" check. The Recycle Bin's paired empty ref travels in [`ApplyAssets::empty`], so the
    /// applier references the exact asset the driver materialized, never a guessed path (P2-1).
    fn apply(&self, target: &ItemTarget, assets: &ApplyAssets) -> PortResult<Fingerprint>;

    /// Restores `target` to the captured original (e.g. replay the original `.lnk` bytes).
    fn restore(&self, target: &ItemTarget, anchor: &RestoreAnchor) -> PortResult<()>;
}

/// Materializes generated `.ico` bytes into a content-addressed store and garbage-collects
/// entries no ledger references (spec 07 §5). `put` is idempotent: an identical asset is
/// reused rather than rewritten.
pub trait AssetStore {
    /// Writes `bytes` under `hash`, returning the reference (new-file-first semantics).
    fn put(&self, hash: &str, bytes: &[u8]) -> PortResult<AssetRef>;

    /// Materializes the paired empty-state variant of `primary` and returns its reference. Some
    /// items have two visual states the registry references together (the Recycle Bin's
    /// full/empty icons); the empty asset is addressed relative to the full one by the store's
    /// convention, so the applier can reference it without guessing an unwritten path (P1-14).
    fn put_empty_variant(&self, primary: &AssetRef, bytes: &[u8]) -> PortResult<AssetRef>;

    /// Whether `asset` has actually been materialized in the store. The driver verifies a paired
    /// asset exists before the mutation points a registry value at it (P1-14).
    fn exists(&self, asset: &AssetRef) -> PortResult<bool>;

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
