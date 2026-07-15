//! Resident auto-format (spec 07, ADR-0019/0020/0022): the background engine that keeps new
//! desktop icons styled per the user's saved appearance.
//!
//! Layering: this crate is the DECISION core — pure orchestration over the `dm-domain` ports and
//! `dm-operations` primitives, fully unit-tested on the Mac host over fakes. The platform bodies
//! it drives (desktop watcher, activity WinEventHook, COM writers) live in `dm-windows`; the tray
//! rendering + notifications live in `src-tauri`. Two structural guarantees ride in the crate
//! graph itself:
//!
//! - **The §14 red line**: no `dm-elevated`/`OverlayControl` dependency exists here, so the
//!   background CANNOT elevate — privileged work is only ever enqueued
//!   ([`PendingPrivilegedQueue`]) for the one batched UAC when the user opens the window.
//! - **One undo surface** (ADR-0020 §2): every background apply goes through the SAME
//!   `TxnDriver::apply` + ledger the manual flow uses; an incremental run writes ONLY store ①
//!   (never ② saved-style, never ③ look-history — spec 07 §5/§8).

pub mod consent;
pub mod driver;
pub mod pending_privileged;
pub mod reconciler;
pub mod stability;
pub mod tray_state;

pub use consent::{FreshnessInputs, TrustState, PROPOSAL_TIMEOUT_SECS};
pub use driver::{
    DriverClock, DriverConfig, MonotonicClock, Proposal, ReconcileEngine, ResidentDriver,
    ResidentHost, TickReport, WatchEventSource,
};
pub use pending_privileged::{PendingPrivilegedQueue, PendingReason};
pub use reconciler::{
    ReconcileContext, ReconcileOutcome, Reconciler, ReconcilerPorts, RestoreBatchOutcome,
    UndoTarget, VettedCandidate,
};
pub use stability::{FsStabilityReader, SettleProbe, StabilityReader, StabilitySnapshot};
pub use tray_state::{transition, TrayEvent, TrayState};
// Re-export the watcher hint type the driver consumes so composition roots (src-tauri) need no
// direct dm-windows dependency on non-Windows hosts (spec 07 §3, plan T4).
pub use dm_windows::watcher::WatchEvent;
