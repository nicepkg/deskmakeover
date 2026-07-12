//! The durable transaction layer: write-ahead journal, apply driver, and crash recovery.

pub mod asset_store;
pub mod driver;
pub mod id;
pub mod journal;
pub mod recovery;

pub use asset_store::FsAssetStore;
pub use driver::{ApplyOutcome, ApplyRequest, TxnDriver};
pub use id::TxnIdAllocator;
pub use journal::{FileJournal, JournalRecord, JournalSink, VecJournal};
pub use recovery::{recover, recover_from_journal, repair_pending, RecoveryOutcome};

// Crate-visible in test builds so the icon-ops tests reuse this virtual-desktop fake instead of
// duplicating it (the driver tests use it as `super::fakes`).
#[cfg(test)]
pub(crate) mod fakes;
#[cfg(test)]
mod tests;
