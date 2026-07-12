//! Operations (ADR-0019): durable transaction ledger, journal, and compare-and-swap
//! restore — incremental owned-field apply, no restore-first port.
//!
//! Layers:
//! * [`settings_store`] — the rusqlite-backed settings row (M2).
//! * [`ledger`] — the incremental per-item ledger, the one undo surface (ADR-0020 §2).
//! * [`txn`] — the durable transaction machinery: a write-ahead [`txn::journal`], the apply
//!   [`txn::driver`], and crash [`txn::recovery`]. All pure Rust, no `cfg(windows)`, no C
//!   dependency — it drives the platform through the `dm-domain` port traits, so the entire
//!   state machine (including the kill-point battery) is unit-tested on the Mac host.
//! * [`wallpaper`] — the thin wallpaper orchestration (M6-WIRE, owner ruling D1): the durable
//!   pre-first-apply snapshot (snapshot-once) + baked-PNG materialization + apply/restore
//!   driving through the wallpaper ports. Pure Rust, Mac-tested with fakes.

mod error;
mod fs_atomic;
pub mod ledger;
mod settings_store;
pub mod txn;
pub mod wallpaper;

pub use error::{OperationError, Result};
pub use ledger::{
    JsonLedgerStore, LedgerEntry, LedgerStore, LookHistoryStore, LookVersion, MemLedgerStore,
    PinResult, PushOutcome, TxnState,
};
pub use settings_store::SettingsStore;
pub use wallpaper::{RustImageDecoder, SnapshotStore, WallpaperOps, WallpaperOutcome};
pub use txn::{
    recover, recover_from_journal, ApplyOutcome, ApplyRequest, FileJournal, FsAssetStore,
    JournalRecord, JournalSink, RecoveryOutcome, TxnDriver, TxnIdAllocator, VecJournal,
};
