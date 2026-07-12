//! Durable persistence for the active ledger: the one undo surface both the manual and the
//! (future) background flow append to.
//!
//! Two behaviours are ported from the oracle and are deliberately different from each other:
//! * atomic write (temp file + rename) — `SnapshotStore.Save` / `DesktopBakeService.SaveState`;
//! * **fail-closed** corruption handling — `DesktopBakeService.LoadState` codex B4: a present
//!   but unparseable ledger returns [`OperationError::CorruptLedger`], NEVER an empty list,
//!   so a corrupt file can't masquerade as "nothing applied" and strand the restore path.
//!
//! (The advisory "10 newest looks" history — [`super::history::LookHistoryStore`], which IS
//! corruption-tolerant because it's advisory — is the deliberately-opposite sibling store: its
//! own physically separate file, empty on corruption rather than fail-closed.)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use dm_domain::ItemId;

use crate::error::{OperationError, Result};
use crate::ledger::entry::LedgerEntry;

/// The active-ledger persistence port. The transaction driver and recovery write through this;
/// tests use [`MemLedgerStore`], production uses [`JsonLedgerStore`].
pub trait LedgerStore {
    /// Inserts or replaces the entry for its item.
    fn upsert(&mut self, entry: LedgerEntry) -> Result<()>;

    /// The entry for `item`, if any.
    fn get(&self, item: &ItemId) -> Result<Option<LedgerEntry>>;

    /// Every entry, newest-first by [`LedgerEntry::version`].
    fn all(&self) -> Result<Vec<LedgerEntry>>;

    /// Removes the entry for `item` (used when an item is fully restored to original).
    fn remove(&mut self, item: &ItemId) -> Result<()>;

    /// The next monotonic version to allocate: one past the current maximum.
    fn next_version(&self) -> Result<u64> {
        Ok(self.all()?.iter().map(|e| e.version).max().unwrap_or(0) + 1)
    }
}

/// In-memory ledger for unit tests.
#[derive(Debug, Default)]
pub struct MemLedgerStore {
    entries: BTreeMap<String, LedgerEntry>,
}

impl MemLedgerStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl LedgerStore for MemLedgerStore {
    fn upsert(&mut self, entry: LedgerEntry) -> Result<()> {
        self.entries.insert(entry.item.as_str().to_string(), entry);
        Ok(())
    }

    fn get(&self, item: &ItemId) -> Result<Option<LedgerEntry>> {
        Ok(self.entries.get(item.as_str()).cloned())
    }

    fn all(&self) -> Result<Vec<LedgerEntry>> {
        let mut all: Vec<LedgerEntry> = self.entries.values().cloned().collect();
        all.sort_by(|a, b| b.version.cmp(&a.version));
        Ok(all)
    }

    fn remove(&mut self, item: &ItemId) -> Result<()> {
        self.entries.remove(item.as_str());
        Ok(())
    }
}

/// JSON-file-backed ledger with atomic writes and fail-closed corruption handling.
#[derive(Debug, Clone)]
pub struct JsonLedgerStore {
    path: PathBuf,
}

impl JsonLedgerStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Loads the entry list. Missing file ⇒ empty (a fresh install). Present-but-unparseable ⇒
    /// [`OperationError::CorruptLedger`] (fail closed — never an empty list).
    fn load(&self) -> Result<Vec<LedgerEntry>> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice::<Vec<LedgerEntry>>(&bytes)
                .map_err(|_| OperationError::CorruptLedger),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(OperationError::Io(e.to_string())),
        }
    }

    /// Atomically persists `entries` (temp + fsync + rename, with the Windows sharing-violation
    /// retry) through the crate's shared [`crate::fs_atomic::write_atomic`].
    fn store(&self, entries: &[LedgerEntry]) -> Result<()> {
        crate::fs_atomic::write_atomic(&self.path, &serde_json::to_vec_pretty(entries)?)
    }

    fn rewrite<F: FnOnce(&mut Vec<LedgerEntry>)>(&self, mutate: F) -> Result<()> {
        let mut entries = self.load()?;
        mutate(&mut entries);
        self.store(&entries)
    }

    /// The backing file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl LedgerStore for JsonLedgerStore {
    fn upsert(&mut self, entry: LedgerEntry) -> Result<()> {
        self.rewrite(|entries| {
            if let Some(slot) = entries.iter_mut().find(|e| e.item == entry.item) {
                *slot = entry;
            } else {
                entries.push(entry);
            }
        })
    }

    fn get(&self, item: &ItemId) -> Result<Option<LedgerEntry>> {
        Ok(self.load()?.into_iter().find(|e| &e.item == item))
    }

    fn all(&self) -> Result<Vec<LedgerEntry>> {
        let mut all = self.load()?;
        all.sort_by(|a, b| b.version.cmp(&a.version));
        Ok(all)
    }

    fn remove(&mut self, item: &ItemId) -> Result<()> {
        self.rewrite(|entries| entries.retain(|e| &e.item != item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::entry::TxnState;
    use dm_domain::{AssetRef, Fingerprint, ItemKind, ItemTarget, OwnedFields, RestoreAnchor};

    fn sample(item: &str, version: u64) -> LedgerEntry {
        LedgerEntry {
            item: ItemId::from_raw(item),
            target: ItemTarget::new(ItemId::from_raw(item), ItemKind::Shortcut, format!("C:/{item}.lnk")),
            original_fingerprint: Fingerprint::of_bytes(b"orig"),
            original_anchor: RestoreAnchor::FileBytes { bytes: b"orig".to_vec() },
            last_applied_fingerprint: Fingerprint::of_bytes(b"applied"),
            owned: OwnedFields::icon_only(),
            asset: AssetRef::new("hash", "C:/gen.ico"),
            empty_asset: None,
            state: TxnState::Committed,
            pinned_seed: None,
            version,
        }
    }

    #[test]
    fn json_store_round_trips_and_orders_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = JsonLedgerStore::new(dir.path().join("ledger.json"));
        store.upsert(sample("a", 1)).unwrap();
        store.upsert(sample("b", 3)).unwrap();
        store.upsert(sample("c", 2)).unwrap();

        let all = store.all().unwrap();
        assert_eq!(all.iter().map(|e| e.version).collect::<Vec<_>>(), vec![3, 2, 1]);
        assert_eq!(store.next_version().unwrap(), 4);
        assert!(store.get(&ItemId::from_raw("b")).unwrap().is_some());

        // Survives reopen.
        let reopened = JsonLedgerStore::new(dir.path().join("ledger.json"));
        assert_eq!(reopened.all().unwrap().len(), 3);
    }

    #[test]
    fn upsert_replaces_same_item() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = JsonLedgerStore::new(dir.path().join("ledger.json"));
        store.upsert(sample("a", 1)).unwrap();
        store.upsert(sample("a", 5)).unwrap();
        let all = store.all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].version, 5);
    }

    #[test]
    fn missing_file_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonLedgerStore::new(dir.path().join("nope.json"));
        assert!(store.all().unwrap().is_empty());
        assert_eq!(store.next_version().unwrap(), 1);
    }

    #[test]
    fn corrupt_ledger_never_reads_as_nothing_applied() {
        // The load must FAIL CLOSED: a present-but-garbage ledger is an error, not an empty
        // list that would look like "nothing was ever applied" (oracle codex B4).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.json");
        fs::write(&path, b"{ this is not json ]").unwrap();
        let store = JsonLedgerStore::new(&path);
        assert!(matches!(store.all(), Err(OperationError::CorruptLedger)));
        assert!(matches!(store.get(&ItemId::from_raw("a")), Err(OperationError::CorruptLedger)));
    }

    #[test]
    fn removing_a_missing_item_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = JsonLedgerStore::new(dir.path().join("ledger.json"));
        store.upsert(sample("a", 1)).unwrap();
        store.remove(&ItemId::from_raw("does-not-exist")).unwrap();
        assert_eq!(store.all().unwrap().len(), 1);
        store.remove(&ItemId::from_raw("a")).unwrap();
        assert!(store.all().unwrap().is_empty());
    }

    #[test]
    fn mem_store_next_version_tracks_the_max() {
        let mut store = MemLedgerStore::new();
        assert_eq!(store.next_version().unwrap(), 1);
        store.upsert(sample("a", 4)).unwrap();
        store.upsert(sample("b", 2)).unwrap();
        assert_eq!(store.next_version().unwrap(), 5);
        // get + remove round-trip.
        assert!(store.get(&ItemId::from_raw("a")).unwrap().is_some());
        store.remove(&ItemId::from_raw("a")).unwrap();
        assert!(store.get(&ItemId::from_raw("a")).unwrap().is_none());
    }
}
