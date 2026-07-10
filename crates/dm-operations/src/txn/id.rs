//! Monotonic transaction-id allocation.
//!
//! A transaction id must never be reused: recovery groups journal records by id, so a reissued id
//! merges two transactions' records into one group and misclassifies them — a rolled-back reuse of
//! a committed id would otherwise drop the committed work. The allocator seeds from the highest id
//! still in the durable journal and only ever counts up, so ids never regress across a crash
//! (P1-7). The composition root owns one allocator per process and feeds its ids to the driver.

use crate::error::Result;
use crate::txn::journal::JournalSink;

/// Hands out strictly-increasing transaction ids. Build one per process at the composition root
/// with [`from_journal`](TxnIdAllocator::from_journal) so it resumes past whatever the last run
/// reached; a fresh (empty-journal) install starts at 1.
#[derive(Debug)]
pub struct TxnIdAllocator {
    next: u64,
}

impl TxnIdAllocator {
    /// Seeds the allocator one past the highest txn id currently in the journal, so no run can
    /// reissue an id whose records recovery still holds — even after a crash.
    pub fn from_journal(journal: &dyn JournalSink) -> Result<Self> {
        let max = journal.read_all()?.iter().map(|r| r.txn()).max().unwrap_or(0);
        Ok(Self { next: max + 1 })
    }

    /// Starts allocating from an explicit id; `first` is the next id handed out. Mainly for tests
    /// and for resuming from a checkpoint whose max id is tracked outside the journal.
    pub fn starting_at(first: u64) -> Self {
        Self { next: first }
    }

    /// The next monotonic id. Never returns a value it has already returned.
    pub fn next_id(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }

    /// The id that will be handed out next, without consuming it.
    pub fn peek(&self) -> u64 {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::txn::journal::{JournalRecord, VecJournal};
    use dm_domain::ItemId;

    fn begin(txn: u64) -> JournalRecord {
        JournalRecord::TxnBegin { txn, items: vec![ItemId::from_raw("a")] }
    }

    #[test]
    fn fresh_install_starts_at_one() {
        let journal = VecJournal::new();
        let mut alloc = TxnIdAllocator::from_journal(&journal).unwrap();
        assert_eq!(alloc.next_id(), 1);
        assert_eq!(alloc.next_id(), 2);
    }

    #[test]
    fn resumes_past_the_highest_journal_id() {
        let mut journal = VecJournal::new();
        journal.append(&begin(1)).unwrap();
        journal.append(&begin(7)).unwrap();
        journal.append(&begin(3)).unwrap();
        let mut alloc = TxnIdAllocator::from_journal(&journal).unwrap();
        assert_eq!(alloc.next_id(), 8, "must resume one past the max, not the last-seen");
    }

    #[test]
    fn never_regresses_across_a_restart() {
        // A crash + restart re-seeds from the journal; the id must not fall back to 1.
        let mut journal = VecJournal::new();
        journal.append(&begin(42)).unwrap();
        let resumed = TxnIdAllocator::from_journal(&journal).unwrap();
        assert_eq!(resumed.peek(), 43);
    }
}
