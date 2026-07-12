//! The icon apply-session + persisted-state orchestration (Wave B B2).
//!
//! D1-consistent THIN boundary: Rust does the genuine platform work — package the frontend's
//! baked masters into laddered ICOs, drive the durable transaction engine, persist stores ②③
//! (spec 07 §8), reset to original — and returns thin data. It does NOT assemble `IconsStateDto`
//! (presets/palette/grid/`activePresetId`/the config draft are frontend concerns).
//!
//! [`IconOps`] bundles the always-needed immutable collaborators (the platform ports + the
//! settings store, store ②); each method takes only the mutable stores it touches (journal,
//! ledger, look-history, txn allocator). The apply path is safety-critical — it is the one
//! transaction that can leave a desktop wrong — so it reconciles the journal into the ledger
//! before preparing (spec 07 §5 / the #5 commit→ledger gap), and its caller MUST serialize apply
//! and GC under one lock (the B2 apply/GC lifecycle-lock; the host owns that mutex, mirroring
//! `WallpaperHost`).

mod package;

pub use package::{package_masters, BufferedMaster, PackagedItem};

use std::collections::HashSet;

use dm_contracts::IconStyle;
use dm_domain::{
    AssetStore, DesktopItem, Fingerprint, IconApplier, ItemId, ItemStateReader, OwnedFields,
    PortError,
};

use crate::error::Result;
use crate::ledger::{LedgerStore, LookHistoryStore, LookVersion};
use crate::settings_store::SettingsStore;
use crate::txn::{
    recover_from_journal, ApplyRequest, JournalRecord, JournalSink, TxnDriver, TxnIdAllocator,
};

/// The immutable platform ports one apply/reset drives.
#[derive(Clone, Copy)]
pub struct IconPlatform<'a> {
    pub reader: &'a dyn ItemStateReader,
    pub applier: &'a dyn IconApplier,
    pub assets: &'a dyn AssetStore,
}

/// One item as the scan observed it, WITH the fingerprint captured AT SCAN TIME. The apply's CAS
/// anchor for a fresh item is this scan-time fingerprint (not a re-read at commit): if the user
/// hand-edits the icon during the — potentially slow, chunked — bake, `current != scan_fingerprint`
/// fails the driver's CAS and the edit is left untouched, never overwritten (the driver's documented
/// "fresh apply uses the scan observation" contract). The host captures it once per scan.
#[derive(Debug, Clone)]
pub struct ScannedItem {
    pub item: DesktopItem,
    pub fingerprint: Fingerprint,
}

/// The chunk-buffer for one apply (`begin` → `push`\* → commit). Owns no ports; it just
/// accumulates the frontend's baked masters until the commit packages + applies them.
pub struct IconApplySession {
    revision: u32,
    expected: usize,
    masters: Vec<BufferedMaster>,
}

impl IconApplySession {
    /// Starts a session for scan `revision`, expecting `count` masters (a completeness hint the
    /// host can check; the commit tolerates a short/over buffer and reconciles against the scan).
    pub fn begin(revision: u32, count: usize) -> Self {
        Self { revision, expected: count, masters: Vec::with_capacity(count) }
    }

    /// Buffers one baked master. `source_index` 0 = primary, 1 = the paired empty (Recycle Bin).
    pub fn push(&mut self, item_id: impl Into<String>, source_index: u32, png_base64: impl Into<String>) {
        self.masters.push(BufferedMaster {
            item_id: item_id.into(),
            source_index,
            png_base64: png_base64.into(),
        });
    }

    /// The scan revision this apply was built against.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// How many masters the `begin` call promised.
    pub fn expected(&self) -> usize {
        self.expected
    }

    /// How many masters have actually been buffered.
    pub fn len(&self) -> usize {
        self.masters.len()
    }

    /// Whether no master has been buffered (an apply of nothing).
    pub fn is_empty(&self) -> bool {
        self.masters.is_empty()
    }
}

/// The persisted store snapshot the host maps to `IconPersistedDto` (adding the native arrow +
/// profile bits). `applied` is whether the active ledger tracks any styled item — the restore
/// affordance's authority across a cold start.
#[derive(Debug, Clone, PartialEq)]
pub struct IconStoreState {
    pub saved_style: Option<IconStyle>,
    pub history: Vec<LookVersion>,
    pub applied: bool,
}

/// The outcome of a commit: which items the transaction styled, which were skipped (stale,
/// un-styleable, gone, or an external-modification CAS conflict), any batch error, and the fresh
/// store snapshot.
#[derive(Debug, Clone)]
pub struct IconApplyOutcome {
    pub committed: Vec<ItemId>,
    /// Currently-styled icons the user set to 「保留原样」 that this apply reverted to their original.
    pub reverted: Vec<ItemId>,
    pub conflicts: Vec<ItemId>,
    pub error: Option<String>,
    pub stores: IconStoreState,
}

/// The outcome of a reset-to-original: items reverted, items left alone because the user
/// hand-edited them since (spec 07 §10 ★, trust-first), and the fresh store snapshot.
#[derive(Debug, Clone)]
pub struct IconResetOutcome {
    pub restored: Vec<ItemId>,
    pub skipped: Vec<ItemId>,
    pub stores: IconStoreState,
}

impl<'a> IconPlatform<'a> {
    pub fn new(
        reader: &'a dyn ItemStateReader,
        applier: &'a dyn IconApplier,
        assets: &'a dyn AssetStore,
    ) -> Self {
        Self { reader, applier, assets }
    }
}

/// The icon operations, bound to the immutable platform ports + the settings store (② saved-style).
pub struct IconOps<'a> {
    platform: IconPlatform<'a>,
    settings: &'a SettingsStore,
}

impl<'a> IconOps<'a> {
    pub fn new(platform: IconPlatform<'a>, settings: &'a SettingsStore) -> Self {
        Self { platform, settings }
    }

    /// Reads stores ②③ + the active ledger for the persisted snapshot the host reports.
    pub fn read_state(
        &self,
        history: &LookHistoryStore,
        ledger: &dyn LedgerStore,
    ) -> Result<IconStoreState> {
        Ok(IconStoreState {
            saved_style: self.settings.get_saved_style()?,
            history: history.all(),
            applied: !ledger.all()?.is_empty(),
        })
    }

    /// Packages the session's masters, applies them as one durable transaction, persists ②③, and
    /// GCs orphaned assets. `scan_items` is the host's last-scan cache (id → target). `look_id` +
    /// `created_at` are caller-stamped (the host supplies real values; tests pass fixed ones),
    /// matching the ledger's wall-clock-free discipline.
    ///
    /// The caller MUST hold the apply/GC lock across this call (the B2 lifecycle-lock): the GC at
    /// the end computes its live set from the ledger UNION the in-flight journal, so a concurrent
    /// apply's not-yet-committed asset is never collected — but that guard only holds if apply and
    /// GC do not interleave, which is the host mutex's job.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_apply(
        &self,
        session: IconApplySession,
        style: IconStyle,
        label: Option<String>,
        look_id: impl Into<String>,
        created_at: i64,
        scan: &[ScannedItem],
        restore_ids: &[String],
        txn: &mut TxnIdAllocator,
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
        history: &mut LookHistoryStore,
    ) -> Result<IconApplyOutcome> {
        // #5 commit→ledger gap: reconcile any journal-committed-but-unledgered transaction into the
        // ledger (and checkpoint the journal) BEFORE preparing, so the CAS anchors + the GC live set
        // below are computed against a ledger that reflects every durable commit. Idempotent.
        recover_from_journal(journal, self.platform.reader, self.platform.applier, ledger)?;

        // Package + VALIDATE every master FIRST, before touching the desktop (codex R2-Block 2): a
        // malformed PNG must fail the whole commit while the desktop is still untouched, never after
        // the keep-restore has already reverted some icons.
        let packaged = package_masters(&session.masters)?;
        let packaged_ids: std::collections::HashSet<&str> =
            packaged.iter().map(|p| p.item_id.as_str()).collect();

        // The user's 「保留原样」 / kindPolicy opt-out for a CURRENTLY-styled icon is a RESTORE, not a
        // no-op: the frontend excludes it from the bake set, so without this step the icon keeps its
        // old applied style on the real desktop while the UI shows the original (spec 06 §2 breach,
        // codex 2026-07-12). CAS-gated + trust-first: an icon the user hand-edited since is left
        // alone. An id not in the ledger was never styled — nothing to revert. An id ALSO in the bake
        // set is being re-styled, so the apply wins — skip its revert (codex R2-Block 2 double-touch).
        let mut reverted: Vec<ItemId> = Vec::new();
        for id in restore_ids {
            if packaged_ids.contains(id.as_str()) {
                continue;
            }
            let item_id = ItemId::from_raw(id.as_str());
            let Some(entry) = ledger.get(&item_id)? else { continue };
            match self.platform.reader.read_fingerprint(&entry.target) {
                Ok(cur) if cur == entry.last_applied_fingerprint => {
                    self.platform.applier.restore(&entry.target, &entry.original_anchor)?;
                    ledger.remove(&item_id)?;
                    reverted.push(item_id);
                }
                Ok(_) => {} // hand-edited since → leave it (trust-first)
                Err(PortError::NotFound(_)) => ledger.remove(&item_id)?, // deleted → drop the row
                Err(e) => return Err(e.into()),
            }
        }

        // Resolve each packaged item against the live scan; build the driver's requests. An item no
        // longer in the scan, or not styleable, or already gone is a benign conflict (skipped, never
        // forced). The CAS anchor for a FRESH apply is the fingerprint captured AT SCAN TIME (so a
        // hand-edit during the bake fails CAS and is left untouched); a RE-APPLY's anchor is the
        // ledger's last-applied, which the driver enforces itself.
        let by_id: std::collections::HashMap<&str, &ScannedItem> =
            scan.iter().map(|s| (s.item.id.as_str(), s)).collect();
        let mut requests = Vec::with_capacity(packaged.len());
        let mut conflicts: Vec<ItemId> = Vec::new();
        for pkg in &packaged {
            let Some(scanned) = by_id.get(pkg.item_id.as_str()) else {
                conflicts.push(ItemId::from_raw(&pkg.item_id));
                continue;
            };
            if !scanned.item.can_style() {
                conflicts.push(scanned.item.id.clone());
                continue;
            }
            requests.push(ApplyRequest {
                target: scanned.item.target(),
                expected_fingerprint: scanned.fingerprint,
                owned: OwnedFields::icon_only(),
                asset_hash: pkg.primary.content_hash.clone(),
                asset_bytes: pkg.primary.bytes.clone(),
                empty_asset_bytes: pkg.empty.as_ref().map(|e| e.bytes.clone()),
                pinned_seed: None,
            });
        }

        let txn_id = txn.next_id();
        let apply = TxnDriver::new(self.platform.reader, self.platform.applier, self.platform.assets)
            .apply(txn_id, requests, journal, ledger)?;
        conflicts.extend(apply.conflicts);

        // Persist ② (saved-style) + push ③ (look-history) ONLY on a clean apply. A batch that
        // preflight-failed or rolled back reports `Ok(ApplyOutcome { error: Some(..) })` with the
        // desktop reverted (driver P2-5); writing ② then would poison the saved-style with a look
        // the desktop never actually wears and grow ③ with a phantom entry — the next launch would
        // resume from (and the resident project) a failed style. A completed global Apply is the
        // ONLY writer of ② (spec 07 §8.2), carrying the apply's name (dedup + force-unpinned handled
        // by the store). set_saved_style borrows the style; the push then consumes it.
        // [FOLLOW-UP] the ①→②→③ finalize is three separate writes — a crash between them is not yet
        // journaled (documented alongside the reset crash-window; both want a finalize record).
        if apply.error.is_none() {
            self.settings.set_saved_style(Some(&style))?;
            history.push(LookVersion {
                id: look_id.into(),
                created_at,
                label,
                pinned: false,
                icon_style: style,
            })?;
        }

        // Collect assets orphaned by this apply (an item's superseded ICO). Live = the ledger's
        // referenced assets UNION the in-flight journal's, so nothing a durable-but-unreconciled
        // record still points at is dropped (the lock keeps a CONCURRENT apply out; this union
        // covers the same-call in-flight window).
        let live = live_asset_hashes(ledger, journal)?;
        self.platform.assets.gc(&live)?;

        let stores = self.read_state(history, ledger)?;
        Ok(IconApplyOutcome {
            committed: apply.committed,
            reverted,
            conflicts,
            error: apply.error,
            stores,
        })
    }

    /// Reverts every styled item to the user's true original (spec 07 §10), CAS-gated so an item
    /// the user hand-edited since is LEFT ALONE (trust-first, ★): a byte-literal revert would
    /// destroy the user's own change. A deleted item's row is dropped; a restored item's row is
    /// dropped and its ICO becomes collectable. Clears ② (the saved-style), so the resident stays
    /// dormant afterwards (spec 07 §8.4). The arrow overlay + auto-format toggle are the host's to
    /// reset — this owns only the icon ledger + assets + ②.
    ///
    /// Crash-window (documented, self-healing): a crash strictly between `applier.restore` and the
    /// ledger `remove` leaves the item correctly reverted on disk but its row lingering; the desktop
    /// is never wrong (the anchor is durable + restore is idempotent), and a subsequent reset's CAS
    /// resolves it. Journaling the reset as its own transaction (so recovery finishes it) is a
    /// tracked follow-up, not a correctness gap in the desktop state.
    pub fn reset_to_original(
        &self,
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
        history: &LookHistoryStore,
    ) -> Result<IconResetOutcome> {
        // Reconcile any committed-but-unledgered txn into the ledger, THEN empty the journal, BEFORE
        // deleting any ledger row (codex 2026-07-12). Without this, an Apply's `TxnCommitted` records
        // outlive the reset: on the next launch, startup recovery re-upserts the rows this reset just
        // deleted, resurrecting a styled ledger entry that points at a GC'd ICO while the desktop is
        // original — and its stale fingerprint then reads as a user hand-edit forever. The checkpoint
        // is STRICT here (not the recovery path's best-effort): if the journal cannot be emptied we
        // abort before touching the ledger, so a restart never revives a half-reset state.
        recover_from_journal(journal, self.platform.reader, self.platform.applier, ledger)?;
        journal.checkpoint(&[])?;

        let mut restored = Vec::new();
        let mut skipped = Vec::new();
        for entry in ledger.all()? {
            match self.platform.reader.read_fingerprint(&entry.target) {
                // The user deleted the icon: clear its row (its ICO becomes collectable below).
                Err(PortError::NotFound(_)) => ledger.remove(&entry.item)?,
                // ★ Trust-first: the current state no longer matches what we applied, so the user
                // hand-edited it — leave it, count it toward "已跳过 N 项(你自己改过)".
                Ok(cur) if cur != entry.last_applied_fingerprint => skipped.push(entry.item),
                // Still exactly our applied state: revert to the true original and drop the row.
                Ok(_) => {
                    self.platform.applier.restore(&entry.target, &entry.original_anchor)?;
                    ledger.remove(&entry.item)?;
                    restored.push(entry.item);
                }
                // An infrastructure error (locked file, COM/registry fault) is NOT a benign skip —
                // abort so the operator learns the restore path may be compromised (spec 07 §10).
                Err(e) => return Err(e.into()),
            }
        }
        // Collect every asset the now-shrunk ledger no longer references. Skipped rows keep theirs;
        // an empty ledger collects everything.
        let live = ledger_asset_hashes(ledger)?;
        self.platform.assets.gc(&live)?;
        // ② cleared: after a reset there is no current global style, so the resident is dormant.
        self.settings.set_saved_style(None)?;

        let stores = self.read_state(history, ledger)?;
        Ok(IconResetOutcome { restored, skipped, stores })
    }
}

/// The set of asset hashes the ledger currently references (primary + paired empty).
fn ledger_asset_hashes(ledger: &dyn LedgerStore) -> Result<Vec<String>> {
    let mut live: HashSet<String> = HashSet::new();
    for e in ledger.all()? {
        live.insert(e.asset.hash);
        if let Some(empty) = e.empty_asset {
            live.insert(empty.hash);
        }
    }
    Ok(live.into_iter().collect())
}

/// The GC live set for the apply path: the ledger's referenced assets UNION the in-flight
/// journal's `AssetWritten` refs, so an asset written by a durable-but-not-yet-reconciled record
/// is never collected before its commit references it.
fn live_asset_hashes(ledger: &dyn LedgerStore, journal: &dyn JournalSink) -> Result<Vec<String>> {
    let mut live: HashSet<String> = ledger_asset_hashes(ledger)?.into_iter().collect();
    for rec in journal.read_all()? {
        if let JournalRecord::AssetWritten { asset, empty, .. } = rec {
            live.insert(asset.hash);
            if let Some(empty) = empty {
                live.insert(empty.hash);
            }
        }
    }
    Ok(live.into_iter().collect())
}

#[cfg(all(test, not(windows)))]
mod tests;
