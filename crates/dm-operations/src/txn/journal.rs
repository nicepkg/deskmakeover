//! The write-ahead transaction journal.
//!
//! This is the durability the frozen oracle lacked: `JournaledOperationRunner` was an
//! in-memory rollback stack (ADR-0019 explicitly: "not crash-durable"). Here every step is
//! appended and flushed to disk BEFORE the corresponding external mutation, so a crash at any
//! point can be recovered. Records are append-only JSON lines; the sink is a trait so the
//! driver and recovery are testable with an in-memory [`VecJournal`] and the kill-point
//! battery can replay any truncation of the log.

use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;

use dm_domain::{AssetRef, Fingerprint, ItemId, ItemTarget, OwnedFields, RestoreAnchor};
use serde::{Deserialize, Serialize};

use crate::error::{OperationError, Result};

/// One durable journal record. The ordering guarantee: the record is flushed to disk BEFORE
/// the mutation it describes is attempted (`ItemPrepared`/`AssetWritten` precede their writes),
/// so recovery always has the restore anchor before anything on the desktop can change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "rec")]
pub enum JournalRecord {
    /// A transaction started over these items.
    TxnBegin { txn: u64, items: Vec<ItemId> },
    /// Restore anchor + CAS anchors captured; nothing mutated yet. Written before any write.
    ItemPrepared {
        txn: u64,
        item: ItemId,
        target: ItemTarget,
        anchor: RestoreAnchor,
        /// Fingerprint of the true original (equals the anchor's state).
        original_fingerprint: Fingerprint,
        /// What the caller believed the current state was (CAS conflict check).
        expected_fingerprint: Fingerprint,
        asset_hash: String,
        owned: OwnedFields,
        pinned_seed: Option<u32>,
    },
    /// The generated `.ico` was written to the content-addressed store.
    AssetWritten { txn: u64, item: ItemId, asset: AssetRef },
    /// The icon location was swapped; `new_fingerprint` is the confirmed applied state.
    ItemApplied { txn: u64, item: ItemId, new_fingerprint: Fingerprint },
    /// The applied state was read back and verified.
    ItemVerified { txn: u64, item: ItemId },
    /// The item was walked back to its captured original.
    ItemRolledBack { txn: u64, item: ItemId },
    /// The transaction committed (all items live).
    TxnCommitted { txn: u64 },
    /// The transaction was fully rolled back.
    TxnRolledBack { txn: u64 },
}

impl JournalRecord {
    /// The transaction id this record belongs to.
    pub fn txn(&self) -> u64 {
        match self {
            JournalRecord::TxnBegin { txn, .. }
            | JournalRecord::ItemPrepared { txn, .. }
            | JournalRecord::AssetWritten { txn, .. }
            | JournalRecord::ItemApplied { txn, .. }
            | JournalRecord::ItemVerified { txn, .. }
            | JournalRecord::ItemRolledBack { txn, .. }
            | JournalRecord::TxnCommitted { txn }
            | JournalRecord::TxnRolledBack { txn } => *txn,
        }
    }
}

/// The durable append-only journal port. `append` must not return until the record is durable.
pub trait JournalSink {
    /// Appends and durably flushes one record.
    fn append(&mut self, record: &JournalRecord) -> Result<()>;

    /// Reads every record in append order.
    fn read_all(&self) -> Result<Vec<JournalRecord>>;
}

/// A JSON-lines file journal: each `append` writes one line and `fsync`s it.
#[derive(Debug, Clone)]
pub struct FileJournal {
    path: PathBuf,
}

impl FileJournal {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl JournalSink for FileJournal {
    fn append(&mut self, record: &JournalRecord) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut line = serde_json::to_vec(record)?;
        line.push(b'\n');
        let mut file = fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        file.write_all(&line)?;
        // Durability before the mutation this record guards.
        file.sync_all()?;
        Ok(())
    }

    fn read_all(&self) -> Result<Vec<JournalRecord>> {
        let file = match fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(OperationError::Io(e.to_string())),
        };
        let mut records = Vec::new();
        for line in std::io::BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            records.push(
                serde_json::from_str(&line)
                    .map_err(|e| OperationError::Journal(format!("corrupt journal line: {e}")))?,
            );
        }
        Ok(records)
    }
}

/// An in-memory journal for tests. It models "what has been durably written so far", so a
/// kill point is just a truncation of [`records`](VecJournal::records).
#[derive(Debug, Default, Clone)]
pub struct VecJournal {
    records: Vec<JournalRecord>,
}

impl VecJournal {
    pub fn new() -> Self {
        Self::default()
    }

    /// The records appended so far, in order.
    pub fn records(&self) -> &[JournalRecord] {
        &self.records
    }
}

impl JournalSink for VecJournal {
    fn append(&mut self, record: &JournalRecord) -> Result<()> {
        self.records.push(record.clone());
        Ok(())
    }

    fn read_all(&self) -> Result<Vec<JournalRecord>> {
        Ok(self.records.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn begin(txn: u64) -> JournalRecord {
        JournalRecord::TxnBegin { txn, items: vec![ItemId::from_raw("a")] }
    }

    #[test]
    fn file_journal_appends_and_reads_back_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = FileJournal::new(dir.path().join("txn.log"));
        j.append(&begin(1)).unwrap();
        j.append(&JournalRecord::TxnCommitted { txn: 1 }).unwrap();

        let read = j.read_all().unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0], begin(1));
        assert_eq!(read[1], JournalRecord::TxnCommitted { txn: 1 });

        // Survives reopen (durability).
        let reopened = FileJournal::new(dir.path().join("txn.log"));
        assert_eq!(reopened.read_all().unwrap().len(), 2);
    }

    #[test]
    fn missing_journal_reads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let j = FileJournal::new(dir.path().join("absent.log"));
        assert!(j.read_all().unwrap().is_empty());
    }

    #[test]
    fn vec_journal_records_are_the_truncation_surface() {
        let mut j = VecJournal::new();
        j.append(&begin(7)).unwrap();
        j.append(&JournalRecord::TxnCommitted { txn: 7 }).unwrap();
        assert_eq!(j.records().len(), 2);
        assert_eq!(j.records()[0].txn(), 7);
    }
}
