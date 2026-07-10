//! The durable transaction layer: write-ahead journal, apply driver, and crash recovery.

pub mod driver;
pub mod journal;
pub mod recovery;

pub use driver::{ApplyOutcome, ApplyRequest, TxnDriver};
pub use journal::{FileJournal, JournalRecord, JournalSink, VecJournal};
pub use recovery::{recover, RecoveryOutcome};

#[cfg(test)]
mod fakes;
#[cfg(test)]
mod tests;
