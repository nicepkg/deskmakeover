//! The incremental ledger: the single undo surface both the manual and the (future)
//! background flow append to (ADR-0020 §2, spec 07 §5).

pub mod entry;
pub mod history;
pub mod store;

pub use entry::{LedgerEntry, TxnState};
pub use history::{LookHistoryStore, LookVersion, PinResult, PushOutcome};
pub use store::{JsonLedgerStore, LedgerStore, MemLedgerStore};
