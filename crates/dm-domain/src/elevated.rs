//! The elevated desktop-item apply/restore port (M8): the batch-granular privileged sibling of
//! [`IconApplier`](crate::ports::IconApplier).
//!
//! A shared/all-users desktop item (`C:\Users\Public\Desktop\*.lnk`, `ProgramData\...`) is
//! ACL-protected: an unelevated `SetIconLocation`/byte-replay fails with Access Denied. The
//! operations layer routes those items — and ONLY those (`scope.classify(..).is_some()`) — through
//! this port, which drives the single privileged helper (`dm-elevated apply|restore-desktop-items`)
//! via one `runas` per batch (ONE UAC prompt, never one per item).
//!
//! The port is BATCH-granular on purpose: `runas` spawns a UAC prompt, so a per-item port would
//! flood the user with prompts. The operations layer wraps the batch in the same durable journal +
//! ledger + recovery envelope the in-process [`TxnDriver`](../../dm_operations) uses, so a
//! privileged item is exactly as reversible + crash-safe as a user-desktop one.
//!
//! The helper NEVER trusts the manifest: it re-confirms every target is under a privileged root and
//! every icon is a capped local `.ico` before writing (LPE gate, ADR-0021 §4). This port's job is
//! only to STAGE the batch + invoke the (signature-verified) helper + map its exit code.

use crate::error::PortResult;
use crate::fingerprint::Fingerprint;
use crate::item::ItemTarget;

/// One item to style through the elevated helper. `asset_path` is the already-materialized `.ico`
/// (written unelevated into the app's content-addressed store — the helper points the `.lnk` at it;
/// the shell renders that icon in the USER's context, so no ProgramData copy is needed, unlike the
/// machine-wide overlay).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevatedApplyItem {
    pub target: ItemTarget,
    pub asset_path: String,
}

/// One item to revert through the elevated helper. `original_bytes` is the captured original `.lnk`
/// (the [`FileBytes`](crate::restore::RestoreAnchor::FileBytes) anchor) the helper replays verbatim;
/// `applied_icon` is the icon location we styled it with — the helper's compare-and-swap only
/// restores an item STILL wearing our style (a user re-edit is left alone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevatedRestoreItem {
    pub target: ItemTarget,
    pub original_bytes: Vec<u8>,
    pub applied_icon: String,
}

/// The result of one elevated batch. All-or-nothing (the helper LIFO-rolls-back its own writes on
/// any internal failure): `Applied` means every item landed; `Declined` maps a UAC cancel (the user
/// declined elevation — not a failure); `Failed` carries the helper's exit reason. Mirrors
/// [`OverlayOutcome`](crate::ports::OverlayOutcome), which the overlay verb pair already uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElevatedOutcome {
    Applied,
    Declined,
    Failed(String),
}

/// Drives the elevated desktop-item helper. The operations layer holds an `Option<&dyn
/// ElevatedIconApplier>`: `None` on a host with no privileged scope (the dev host) or before the
/// helper is wired, in which case privileged items fail closed (skipped), exactly as they did
/// before this port existed.
pub trait ElevatedIconApplier {
    /// Derives the post-apply fingerprint of each item WITHOUT writing anything — the same styled
    /// surface [`IconApplier::apply`](crate::ports::IconApplier::apply) would produce. The
    /// operations layer journals this as the item's `ItemApplied` fingerprint BEFORE invoking the
    /// helper, so crash recovery can recognise a privileged item that the helper DID style (live ==
    /// this fingerprint) and adopt it forward instead of attempting a doomed unelevated restore.
    /// Pure + read-only: no UAC, no mutation.
    fn plan(&self, items: &[ElevatedApplyItem]) -> PortResult<Vec<Fingerprint>>;

    /// Stages the batch + invokes the signature-verified helper via `runas` (ONE UAC) to style
    /// every item. The helper independently re-confirms each target + icon and atomically rolls back
    /// all of its writes on any failure, so a half-elevated desktop can never result.
    fn apply(&self, items: &[ElevatedApplyItem]) -> PortResult<ElevatedOutcome>;

    /// Stages the batch + invokes the helper via `runas` (ONE UAC) to restore every item to its
    /// captured original bytes, compare-and-swap guarded so an item the user re-styled since is left
    /// untouched.
    fn restore(&self, items: &[ElevatedRestoreItem]) -> PortResult<ElevatedOutcome>;
}
