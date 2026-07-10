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
    /// The generated `.ico` was written to the content-addressed store. `empty` carries the paired
    /// empty-state asset (the Recycle Bin's empty ICO) so recovery can rebuild the ledger's exact
    /// empty reference for a future GC (new-P1); `#[serde(default)]` keeps older journals readable.
    AssetWritten {
        txn: u64,
        item: ItemId,
        asset: AssetRef,
        #[serde(default)]
        empty: Option<AssetRef>,
    },
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

    /// Atomically retains only the records whose transaction is in `active_txns`, dropping the
    /// rest (P2-5 checkpoint). Passing an empty slice truncates the journal to nothing. A
    /// checkpoint after a clean commit — when no transaction is in-flight — empties the journal
    /// because committed state then lives in the ledger and recovery no longer needs the history.
    /// The default is a no-op, for in-memory test doubles that need not model truncation.
    fn checkpoint(&mut self, active_txns: &[u64]) -> Result<()> {
        let _ = active_txns;
        Ok(())
    }
}

/// The transactions in `records` with NO terminal record (`TxnCommitted` / `TxnRolledBack`) — the
/// still-in-flight set a checkpoint must retain, in first-seen order. Committed and rolled-back
/// transactions are safe to drop: their state is durable in the ledger or already restored.
pub fn active_txns(records: &[JournalRecord]) -> Vec<u64> {
    use std::collections::BTreeSet;
    let mut terminal = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    for record in records {
        let txn = record.txn();
        if seen.insert(txn) {
            order.push(txn);
        }
        if matches!(record, JournalRecord::TxnCommitted { .. } | JournalRecord::TxnRolledBack { .. }) {
            terminal.insert(txn);
        }
    }
    order.into_iter().filter(|txn| !terminal.contains(txn)).collect()
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

impl FileJournal {
    /// The raw append: create the parent, serialize, append the line, and fsync. Any step can fail
    /// with an I/O or serialization error; [`append`](FileJournal::append) reclassifies all of them
    /// as `OperationError::Journal` (see there).
    fn try_append(&self, record: &JournalRecord) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Fence a torn tail from a previously-crashed append (a fragment with no trailing newline)
        // BEFORE writing: otherwise this record concatenates onto it, and the combined line — plus
        // the next record — reads as mid-file corruption at startup even though `read_all` tolerated
        // the fragment on its own (P1, wave-2R). Truncating back to the last newline boundary makes
        // every appended record start on a clean line.
        truncate_torn_tail(&self.path)?;
        let mut line = serde_json::to_vec(record)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        line.push(b'\n');
        let mut file = fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        file.write_all(&line)?;
        // Durability before the mutation this record guards.
        file.sync_all()?;
        Ok(())
    }
}

/// Removes a torn trailing fragment (bytes after the last newline) so the next append starts on a
/// clean line. A file that is empty, missing, or already newline-terminated is left untouched — the
/// common case costs only a single-byte tail read.
fn truncate_torn_tail(path: &std::path::Path) -> std::io::Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = match fs::OpenOptions::new().read(true).write(true).open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(());
    }
    // Fast path: a clean journal ends with a newline.
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last)?;
    if last[0] == b'\n' {
        return Ok(());
    }
    // Torn fragment present: keep everything through the last newline (or nothing if there is none).
    file.seek(SeekFrom::Start(0))?;
    let mut buf = Vec::with_capacity(len as usize);
    file.read_to_end(&mut buf)?;
    let keep = buf.iter().rposition(|&b| b == b'\n').map(|pos| pos + 1).unwrap_or(0);
    file.set_len(keep as u64)?;
    file.sync_all()?;
    Ok(())
}

impl JournalSink for FileJournal {
    fn append(&mut self, record: &JournalRecord) -> Result<()> {
        // A failed append means the record did NOT durably land, and the log may be torn — classify
        // EVERY failure (I/O, fsync, serialization) as `OperationError::Journal`, never a bare `Io`
        // or `Serde`. The driver keys its crash-safety on this: `is_journal_error` recognizes only
        // `OperationError::Journal`, and it is the signal to ABANDON (restore from anchors WITHOUT
        // journaling) rather than journal a rollback into a possibly-corrupt file (P1-5). A journal
        // write failure IS, definitionally, a journal error; leaving it as `Io` sent real production
        // outages down the normal rollback path while `Journal`-injecting tests stayed falsely green.
        self.try_append(record)
            .map_err(|e| OperationError::Journal(format!("append to {}: {e}", self.path.display())))
    }

    fn read_all(&self) -> Result<Vec<JournalRecord>> {
        let file = match fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(OperationError::Io(e.to_string())),
        };
        let lines: Vec<String> = std::io::BufReader::new(file).lines().collect::<std::io::Result<_>>()?;
        let mut records = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str(line) {
                Ok(record) => records.push(record),
                Err(e) => {
                    // A parse failure is tolerated ONLY on the final content line: a crash mid-append
                    // leaves a torn tail whose `sync_all` never returned, so the mutation that record
                    // would guard never happened — dropping it is safe and recovery stays consistent.
                    // A corrupt line with any well-formed line after it is mid-file damage and stays
                    // fatal: recovery must never be trusted to a journal it can only partially parse.
                    if lines[idx + 1..].iter().all(|l| l.trim().is_empty()) {
                        break;
                    }
                    return Err(OperationError::Journal(format!("corrupt journal line {}: {e}", idx + 1)));
                }
            }
        }
        Ok(records)
    }

    fn checkpoint(&mut self, active_txns: &[u64]) -> Result<()> {
        let kept: Vec<JournalRecord> =
            self.read_all()?.into_iter().filter(|r| active_txns.contains(&r.txn())).collect();
        if kept.is_empty() {
            // Truncate: a missing journal reads as empty, so removing the file is the empty state.
            return match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(OperationError::Io(e.to_string())),
            };
        }
        // Atomically rewrite the retained records: temp file + fsync + rename. A crash before the
        // rename leaves the old (complete) journal for recovery; after it, the retained set — either
        // way the durable state is consistent.
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("log.tmp");
        {
            let mut file = fs::File::create(&tmp)?;
            for record in &kept {
                let mut line = serde_json::to_vec(record)?;
                line.push(b'\n');
                file.write_all(&line)?;
            }
            file.sync_all()?;
        }
        fs::rename(&tmp, &self.path)?;
        Ok(())
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

    fn checkpoint(&mut self, active_txns: &[u64]) -> Result<()> {
        self.records.retain(|r| active_txns.contains(&r.txn()));
        Ok(())
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
    fn a_real_file_journal_append_failure_is_a_journal_error_not_io() {
        // P1-5: the driver's abandon path recognizes ONLY `OperationError::Journal`. A real
        // FileJournal write failure must therefore surface as that variant, or production would take
        // the normal rollback path onto a possibly-torn log while `Journal`-injecting doubles stayed
        // falsely green. Force a genuine outage by pointing the journal under a path whose parent is
        // a regular file, so `create_dir_all` + open cannot succeed.
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("i-am-a-file");
        fs::write(&blocker, b"x").unwrap();
        let mut j = FileJournal::new(blocker.join("txn.log")); // parent is a file, not a dir
        let err = j.append(&begin(1)).unwrap_err();
        assert!(
            matches!(err, OperationError::Journal(_)),
            "a real FileJournal outage must classify as Journal (so the driver abandons), got {err:?}"
        );
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

    #[test]
    fn mid_file_corruption_is_an_error_not_a_silent_drop() {
        // A garbage line with a well-formed record AFTER it is mid-file damage, not a torn tail —
        // recovery cannot be trusted to a journal it can only partially parse, so it stays fatal.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("txn.log");
        let mut j = FileJournal::new(&path);
        j.append(&begin(1)).unwrap();
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{ this is not a record }\n").unwrap();
        drop(f);
        // A later, well-formed record proves the garbage line is NOT the tail.
        j.append(&JournalRecord::TxnCommitted { txn: 1 }).unwrap();
        assert!(matches!(j.read_all(), Err(OperationError::Journal(_))));
    }

    #[test]
    fn a_torn_tail_is_fenced_before_the_next_same_process_append() {
        // P1 (wave-2R): read_all tolerates a torn final fragment, but a subsequent append must not
        // concatenate a valid record onto it — the combined line, plus the record after it, would
        // read as mid-file corruption at startup. The append path truncates the torn fragment first.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("txn.log");
        let mut j = FileJournal::new(&path);
        j.append(&begin(1)).unwrap();
        // Simulate a crashed append: a partial line with no trailing newline.
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"rec\":\"ItemApp").unwrap();
        drop(f);
        // read_all tolerates the torn tail on its own (drops it).
        assert_eq!(j.read_all().unwrap().len(), 1);
        // A same-process retry appends two more valid records...
        j.append(&JournalRecord::TxnCommitted { txn: 1 }).unwrap();
        j.append(&begin(2)).unwrap();
        // ...and the whole journal is still parseable (the fragment was fenced, not concatenated).
        let read = j.read_all().unwrap();
        assert_eq!(read.len(), 3, "the torn fragment must be fenced so later appends stay parseable");
        assert_eq!(read[0], begin(1));
        assert_eq!(read[1], JournalRecord::TxnCommitted { txn: 1 });
        assert_eq!(read[2], begin(2));
    }

    #[test]
    fn a_torn_tail_line_is_dropped_not_fatal() {
        // A crash mid-append leaves a partial final line whose fsync never returned; recovery must
        // read the durable prefix and drop the tail, not brick on it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("txn.log");
        let mut j = FileJournal::new(&path);
        j.append(&begin(1)).unwrap();
        j.append(&JournalRecord::TxnCommitted { txn: 1 }).unwrap();
        // Simulate a torn write: a partial JSON line with no trailing newline.
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"rec\":\"ItemApp").unwrap();
        drop(f);
        let read = j.read_all().unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0], begin(1));
        assert_eq!(read[1], JournalRecord::TxnCommitted { txn: 1 });
    }

    #[test]
    fn a_torn_tail_after_trailing_blank_lines_is_dropped() {
        // Trailing blank lines before the torn fragment must not turn tail tolerance into a
        // mid-file verdict.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("txn.log");
        let mut j = FileJournal::new(&path);
        j.append(&begin(1)).unwrap();
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"\n{ torn").unwrap();
        drop(f);
        assert_eq!(j.read_all().unwrap().len(), 1);
    }

    #[test]
    fn blank_lines_in_the_journal_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("txn.log");
        let mut j = FileJournal::new(&path);
        j.append(&begin(1)).unwrap();
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"\n\n").unwrap();
        drop(f);
        assert_eq!(j.read_all().unwrap().len(), 1);
    }

    #[test]
    fn active_txns_excludes_committed_and_rolled_back() {
        let records = vec![
            begin(1),
            JournalRecord::TxnCommitted { txn: 1 },
            begin(2),
            JournalRecord::TxnRolledBack { txn: 2 },
            begin(3), // in-flight — no terminal record
        ];
        assert_eq!(active_txns(&records), vec![3]);
        assert!(active_txns(&[begin(1), JournalRecord::TxnCommitted { txn: 1 }]).is_empty());
    }

    #[test]
    fn checkpoint_to_no_active_txns_truncates_the_file_journal() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = FileJournal::new(dir.path().join("txn.log"));
        j.append(&begin(1)).unwrap();
        j.append(&JournalRecord::TxnCommitted { txn: 1 }).unwrap();
        j.checkpoint(&[]).unwrap();
        assert!(j.read_all().unwrap().is_empty());
        // A reopen sees the truncation (durable).
        let reopened = FileJournal::new(dir.path().join("txn.log"));
        assert!(reopened.read_all().unwrap().is_empty());
    }

    #[test]
    fn checkpoint_retains_only_the_in_flight_transaction() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = FileJournal::new(dir.path().join("txn.log"));
        j.append(&begin(1)).unwrap();
        j.append(&JournalRecord::TxnCommitted { txn: 1 }).unwrap();
        j.append(&begin(2)).unwrap(); // txn 2 crashed in-flight (no terminal)
        j.checkpoint(&active_txns(&j.read_all().unwrap())).unwrap();
        let kept = j.read_all().unwrap();
        assert_eq!(kept.len(), 1);
        assert!(kept.iter().all(|r| r.txn() == 2));
    }

    #[test]
    fn vec_journal_checkpoint_retains_the_active_set() {
        let mut j = VecJournal::new();
        j.append(&begin(1)).unwrap();
        j.append(&JournalRecord::TxnCommitted { txn: 1 }).unwrap();
        j.append(&begin(2)).unwrap();
        j.checkpoint(&active_txns(&j.records().to_vec())).unwrap();
        assert!(j.records().iter().all(|r| r.txn() == 2));
        j.checkpoint(&[]).unwrap();
        assert!(j.records().is_empty());
    }
}
