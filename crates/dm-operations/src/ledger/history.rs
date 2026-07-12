//! Store ③ of the appearance model (spec 07 §8): the advisory "10 newest looks" a user can
//! switch back to.
//!
//! Corruption-**tolerant** — the mirror-opposite fail-safety of the active ledger (store ①): a
//! missing OR unreadable file reads as an empty history and NEVER blocks apply/restore, which is
//! exactly why it MUST be its own physically separate file (`look-history.json`). It stores
//! appearance RECIPES only (`icon_style`, the opaque `{config, kindPolicy, typeOverrides}` blob
//! from `src/bridge/types.ts`), never an icon list (§8.2) — switching a look projects the recipe
//! onto the LIVE scan (§9), so a recorded icon list would only go stale.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// How many looks the history keeps (spec 07 §17 — THIS store only; never the active ledger,
/// whose fail-safety direction is deliberately opposite).
const CAP: usize = 10;

/// How many entries may be pinned (exempt from FIFO eviction) at once (spec 07 §17).
const MAX_PINS: usize = 2;

/// One saved appearance recipe. `icon_style` is the `{config, kindPolicy, typeOverrides}` blob
/// (spec 07 §8.2) — opaque to Rust; only the native bake path (dm-icon-core) ever reads inside it.
/// `id` + `created_at` are caller-stamped so the store stays deterministic in tests (the Tauri
/// layer supplies real ids + time), matching the ledger's wall-clock-free discipline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LookVersion {
    pub id: String,
    pub created_at: i64,
    /// A user-chosen name; also what a pinned favourite is remembered by. `None` = unnamed.
    #[serde(default)]
    pub label: Option<String>,
    /// Exempt from FIFO eviction while set (spec 07 §17). Older builds without this field read as
    /// unpinned.
    #[serde(default)]
    pub pinned: bool,
    pub icon_style: serde_json::Value,
}

/// What a [`LookHistoryStore::push`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    /// A new entry was prepended and the cap enforced.
    Added,
    /// The recipe was field-for-field identical to the current head, so only the head's timestamp
    /// was bumped — no new entry (spec 07 §17 dedup-before-cap).
    BumpedHead,
}

/// The result of a [`LookHistoryStore::set_pinned`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinResult {
    /// The entry's pin state was updated.
    Updated,
    /// No entry with that id exists.
    NotFound,
    /// Pinning would exceed [`MAX_PINS`] and this id is not already pinned.
    LimitReached,
}

/// The look-history file (store ③). Every read is tolerant; every write is crash-atomic.
pub struct LookHistoryStore {
    path: PathBuf,
}

impl LookHistoryStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The backing file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every saved look, newest-first. Tolerant: a missing OR unreadable/corrupt file reads as an
    /// empty history (spec 07 §8.2) — this store must never block the apply/restore path.
    pub fn all(&self) -> Vec<LookVersion> {
        self.load()
    }

    /// The saved look with `id`, if any.
    pub fn get(&self, id: &str) -> Option<LookVersion> {
        self.load().into_iter().find(|v| v.id == id)
    }

    /// Pushes a look (one per global Apply). Dedup-before-cap (spec 07 §17): a recipe
    /// field-for-field identical to the current head only bumps that head's timestamp — so
    /// clicking Apply a few times in a row does not burn through the cap. Otherwise the entry is
    /// prepended and the oldest NON-pinned entries are evicted down to the cap.
    pub fn push(&mut self, entry: LookVersion) -> Result<PushOutcome> {
        let mut all = self.load();
        if let Some(head) = all.first_mut() {
            if head.icon_style == entry.icon_style {
                head.created_at = entry.created_at;
                self.store(&all)?;
                return Ok(PushOutcome::BumpedHead);
            }
        }
        all.insert(0, entry);
        evict_to_cap(&mut all);
        self.store(&all)?;
        Ok(PushOutcome::Added)
    }

    /// Pins/unpins an entry. Pinned entries survive FIFO eviction (spec 07 §17). Pinning is
    /// refused past [`MAX_PINS`] so a `LimitReached` is a clear signal, not a silent no-op.
    pub fn set_pinned(&mut self, id: &str, pinned: bool) -> Result<PinResult> {
        let mut all = self.load();
        // Existence first: a pin request for an unknown id is NotFound, never LimitReached.
        let Some(pos) = all.iter().position(|v| v.id == id) else {
            return Ok(PinResult::NotFound);
        };
        // Refuse pinning past the cap — but an already-pinned entry re-pinning itself is idempotent,
        // not a breach (it consumes no new slot).
        if pinned && !all[pos].pinned && all.iter().filter(|v| v.pinned).count() >= MAX_PINS {
            return Ok(PinResult::LimitReached);
        }
        all[pos].pinned = pinned;
        self.store(&all)?;
        Ok(PinResult::Updated)
    }

    /// Renames an entry (or clears its name with `None`). Returns whether the entry existed.
    pub fn set_label(&mut self, id: &str, label: Option<String>) -> Result<bool> {
        let mut all = self.load();
        match all.iter_mut().find(|v| v.id == id) {
            Some(v) => {
                v.label = label;
                self.store(&all)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Tolerant load: any read/parse failure yields an empty history (see the type docs). Newest-
    /// first is the on-disk order, preserved verbatim.
    fn load(&self) -> Vec<LookVersion> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    fn store(&self, all: &[LookVersion]) -> Result<()> {
        crate::fs_atomic::write_atomic(&self.path, &serde_json::to_vec_pretty(all)?)
    }
}

/// Evicts the oldest NON-pinned entries until at most [`CAP`] remain. Pinned entries are exempt
/// from FIFO eviction (spec 07 §17), so a deliberately-kept favourite is never silently dropped —
/// even if that means momentarily keeping more than the cap when the whole tail is pinned (pins are
/// bounded by [`MAX_PINS`] < [`CAP`], so in practice the history still converges to the cap).
fn evict_to_cap(all: &mut Vec<LookVersion>) {
    while all.len() > CAP {
        match all.iter().rposition(|v| !v.pinned) {
            Some(idx) => {
                all.remove(idx);
            }
            None => break, // everything left is pinned — never evict a pin
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store() -> (tempfile::TempDir, LookHistoryStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = LookHistoryStore::new(dir.path().join("look-history.json"));
        (dir, store)
    }

    /// A distinct recipe per `n`, shaped like the real `{config, kindPolicy, typeOverrides}` blob.
    fn style(n: i64) -> serde_json::Value {
        json!({ "config": { "seed": n }, "kindPolicy": {}, "typeOverrides": {} })
    }

    fn ver(id: &str, ts: i64, n: i64) -> LookVersion {
        LookVersion { id: id.into(), created_at: ts, label: None, pinned: false, icon_style: style(n) }
    }

    #[test]
    fn push_prepends_newest_first_and_persists() {
        let (_dir, mut store) = store();
        store.push(ver("a", 1, 1)).unwrap();
        store.push(ver("b", 2, 2)).unwrap();
        let all = store.all();
        assert_eq!(all.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
        // Survives reopen (its own file).
        let reopened = LookHistoryStore::new(store.path());
        assert_eq!(reopened.all().len(), 2);
        assert_eq!(store.get("a").unwrap().icon_style, style(1));
    }

    #[test]
    fn identical_to_head_bumps_timestamp_instead_of_growing() {
        let (_dir, mut store) = store();
        assert_eq!(store.push(ver("a", 1, 7)).unwrap(), PushOutcome::Added);
        // Same recipe as head → dedup, only the timestamp changes.
        assert_eq!(store.push(ver("b", 99, 7)).unwrap(), PushOutcome::BumpedHead);
        let all = store.all();
        assert_eq!(all.len(), 1, "dedup does not add a second entry");
        assert_eq!(all[0].id, "a", "the existing head is kept");
        assert_eq!(all[0].created_at, 99, "its timestamp was bumped");
    }

    #[test]
    fn a_different_recipe_adds_a_new_head() {
        let (_dir, mut store) = store();
        store.push(ver("a", 1, 1)).unwrap();
        assert_eq!(store.push(ver("b", 2, 2)).unwrap(), PushOutcome::Added);
        assert_eq!(store.all()[0].id, "b");
    }

    #[test]
    fn caps_at_ten_evicting_the_oldest() {
        let (_dir, mut store) = store();
        for n in 0..12 {
            store.push(ver(&format!("v{n}"), n, n)).unwrap();
        }
        let all = store.all();
        assert_eq!(all.len(), CAP);
        // Newest (v11) is head; the two oldest (v0, v1) were evicted.
        assert_eq!(all.first().unwrap().id, "v11");
        assert!(all.iter().all(|v| v.id != "v0" && v.id != "v1"));
    }

    #[test]
    fn pinned_entries_are_exempt_from_eviction() {
        let (_dir, mut store) = store();
        store.push(ver("keep", 0, 100)).unwrap();
        assert_eq!(store.set_pinned("keep", true).unwrap(), PinResult::Updated);
        // Fill well past the cap with distinct recipes.
        for n in 1..15 {
            store.push(ver(&format!("v{n}"), n, n)).unwrap();
        }
        let all = store.all();
        assert!(all.iter().any(|v| v.id == "keep"), "a pinned favourite survives eviction");
        assert!(all.len() <= CAP + 1, "history still converges near the cap");
    }

    #[test]
    fn pinning_is_bounded_and_reports_the_limit() {
        let (_dir, mut store) = store();
        for n in 0..3 {
            store.push(ver(&format!("v{n}"), n, n)).unwrap();
        }
        assert_eq!(store.set_pinned("v0", true).unwrap(), PinResult::Updated);
        assert_eq!(store.set_pinned("v1", true).unwrap(), PinResult::Updated);
        // A third pin is refused, clearly, not silently.
        assert_eq!(store.set_pinned("v2", true).unwrap(), PinResult::LimitReached);
        // Re-pinning an already-pinned id is idempotent, not a limit breach.
        assert_eq!(store.set_pinned("v0", true).unwrap(), PinResult::Updated);
        // Unpinning frees a slot.
        assert_eq!(store.set_pinned("v0", false).unwrap(), PinResult::Updated);
        assert_eq!(store.set_pinned("v2", true).unwrap(), PinResult::Updated);
        assert_eq!(store.set_pinned("missing", true).unwrap(), PinResult::NotFound);
    }

    #[test]
    fn set_label_round_trips() {
        let (_dir, mut store) = store();
        store.push(ver("a", 1, 1)).unwrap();
        assert!(store.set_label("a", Some("我的最爱".into())).unwrap());
        assert_eq!(store.get("a").unwrap().label.as_deref(), Some("我的最爱"));
        assert!(!store.set_label("missing", Some("x".into())).unwrap());
    }

    #[test]
    fn corrupt_or_missing_file_reads_as_empty_never_errors() {
        // Missing file → empty (spec 07 §8.2: never blocks apply/restore).
        let (_dir, store) = store();
        assert!(store.all().is_empty());
        // A present-but-garbage file ALSO reads as empty — the opposite of the fail-closed ledger.
        std::fs::write(store.path(), b"{ not json ]").unwrap();
        assert!(store.all().is_empty());
        assert!(store.get("anything").is_none());
    }
}
