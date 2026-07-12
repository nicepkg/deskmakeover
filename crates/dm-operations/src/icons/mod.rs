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
    /// True when the styling batch entered its mutation phase and then rolled back / abandoned — the
    /// desktop WAS touched even though nothing committed (codex R5-#1). The host uses it so a rollback
    /// is never reported as "nothing changed"; a clean preflight failure leaves it false.
    pub desktop_mutated: bool,
    pub error: Option<String>,
    /// A finalize step that ran AFTER the desktop was already mutated failed at runtime (a keep-revert
    /// I/O fault, a ②/③ write, GC, or the state read-back). The desktop DID change, so the op must
    /// never return a bare `Err` (the UI would report "nothing changed" over a mutated desktop —
    /// codex R3-Block 4); it returns `ok:false` + the authoritative persisted state + this repair
    /// note so the UI re-syncs and the store keeps the draft dirty for a retry. `None` = clean finalize.
    pub degraded: Option<String>,
    pub stores: IconStoreState,
}

/// The outcome of a reset-to-original: items reverted, items left alone because the user
/// hand-edited them since (spec 07 §10 ★, trust-first), and the fresh store snapshot.
#[derive(Debug, Clone)]
pub struct IconResetOutcome {
    pub restored: Vec<ItemId>,
    pub skipped: Vec<ItemId>,
    /// A finalize step that ran AFTER a revert already landed failed at runtime (a later item's
    /// revert I/O fault, GC, ② clear, or the state read-back). Same contract as the apply path
    /// (codex R3-Block 4): never a bare `Err` over an already-mutated desktop — `ok:false` + the
    /// authoritative persisted state + this note. `None` = clean.
    pub degraded: Option<String>,
    /// True when the reset did NOT run its ledger revert because up-front recovery had to heal a prior
    /// crash first (degraded, or a clean abort that already moved the desktop — codex R6-#4). The host
    /// must then SKIP the reset-only finalizers (disabling auto-format, lifting the arrow overlay):
    /// the reset has not actually happened, so applying its side effects would leave a partial state
    /// (arrow native + auto-format off, yet icons still styled). The user re-syncs and retries.
    pub deferred: bool,
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

    /// Reads stores ②③ + the active ledger for the persisted snapshot the host reports. `applied`
    /// keys the restore affordance: it is true when the ledger has any styled row OR a prior crash's
    /// recovery left repair pending (an in-flight or committed-but-unreconciled txn in the journal),
    /// so a styled desktop the ledger does not yet reflect never hides the only way back (codex R6-#6).
    pub fn read_state(
        &self,
        history: &LookHistoryStore,
        ledger: &dyn LedgerStore,
        journal: &dyn JournalSink,
    ) -> Result<IconStoreState> {
        let applied = !ledger.all()?.is_empty()
            || crate::txn::repair_pending(&journal.read_all()?, ledger)?;
        Ok(IconStoreState {
            saved_style: self.settings.get_saved_style()?,
            history: history.all(),
            applied,
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
        let recovery = recover_from_journal(journal, self.platform.reader, self.platform.applier, ledger)?;
        // Recovery of a PRIOR crash's journal either could not fully reconcile (`degraded`, codex
        // R4-Block 5) OR cleanly ABORTED an interrupted transaction — which RESTORES the desktop
        // (codex R5-#3). In EITHER case do NOT stack a new apply on top: the abort already mutated the
        // desktop, so any bare `?` later in this same call (the master validation below, a store read)
        // would surface as "nothing changed" over a desktop recovery just moved. Return the
        // authoritative state + a repair-required note (never a bare Err) so the UI re-syncs and the
        // user retries; the journal was left intact for the next pass (a clean abort already
        // checkpointed it, so the retry finds an empty journal and proceeds normally). Reconcile-only
        // recovery (ledger gap close, no desktop change) is safe to build on and does not early-return.
        if !recovery.degraded.is_empty() || !recovery.aborted.is_empty() {
            let mut repair = recovery.degraded;
            if !recovery.aborted.is_empty() {
                repair.push(format!(
                    "recovered {} interrupted item(s) from a prior crash — re-syncing before re-applying",
                    recovery.aborted.len()
                ));
            }
            let stores = self.read_state_or_degraded(history, ledger, journal, &mut repair);
            return Ok(IconApplyOutcome {
                committed: Vec::new(),
                reverted: Vec::new(),
                conflicts: Vec::new(),
                desktop_mutated: !recovery.aborted.is_empty(),
                error: None,
                degraded: Some(repair.join("; ")),
                stores,
            });
        }

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
        // Repair notes: any failure of a step that runs AFTER the desktop is (or may be) already
        // mutated is recorded here, NEVER surfaced as a bare `Err` (codex R3-Block 4). Joined into
        // `degraded` at the end; the host turns it into `ok:false` + a repair toast + the real state.
        let mut repair: Vec<String> = Vec::new();

        // Best-effort keep-restore: each iteration is CAS-gated + item-independent, so a fault on one
        // item records a note and moves on — the ledger row it couldn't revert simply stays (desktop ==
        // ledger for that item, self-heals on the next reset), rather than bailing after having already
        // reverted earlier items and leaving the caller a bare `Err` over a half-changed desktop.
        let mut reverted: Vec<ItemId> = Vec::new();
        // A restore the user asked for that could NOT be performed because they hand-edited the icon
        // since the scan (trust-first: we never clobber their edit). It is a "couldn't do what you
        // asked" outcome, so it counts toward `conflicts` — otherwise an apply whose ONLY intent was
        // such a revert would report a clean success over a no-op (codex R7-#3).
        let mut restore_skipped: Vec<ItemId> = Vec::new();
        for id in restore_ids {
            if packaged_ids.contains(id.as_str()) {
                continue;
            }
            let item_id = ItemId::from_raw(id.as_str());
            let entry = match ledger.get(&item_id) {
                Ok(Some(e)) => e,
                Ok(None) => continue, // not in the ledger → never styled, nothing to revert
                Err(e) => {
                    repair.push(format!("keep-restore ledger read {id}: {e}"));
                    continue;
                }
            };
            match self.platform.reader.read_fingerprint(&entry.target) {
                Ok(cur) if cur == entry.last_applied_fingerprint => {
                    if let Err(e) = self.platform.applier.restore(&entry.target, &entry.original_anchor) {
                        // Desktop unchanged for THIS item (restore faulted); its ledger row stays →
                        // still consistent. Record + continue.
                        repair.push(format!("keep-restore {id}: {e}"));
                        continue;
                    }
                    // Reverted on disk; a remove fault leaves a lingering row — recorded, and the next
                    // reset/apply now HEALS it via the original-fingerprint arm below (codex R4-Block 2).
                    if let Err(e) = ledger.remove(&item_id) {
                        repair.push(format!("keep-restore ledger remove {id}: {e}"));
                    }
                    reverted.push(item_id);
                }
                // Already pristine (a prior revert landed, its row lingered): heal the row so it can't
                // poison a future reset/re-apply (codex R4-Block 2). NOT a hand-edit — checked before it.
                Ok(cur) if cur == entry.original_fingerprint => {
                    if let Err(e) = ledger.remove(&item_id) {
                        repair.push(format!("keep-restore heal-remove {id}: {e}"));
                    }
                }
                // Hand-edited since the scan → leave it (trust-first), but record it as a skip so an
                // apply whose only effect would have been this revert is not reported as a clean no-op.
                Ok(_) => restore_skipped.push(item_id),
                Err(PortError::NotFound(_)) => {
                    if let Err(e) = ledger.remove(&item_id) {
                        repair.push(format!("keep-restore ledger drop {id}: {e}"));
                    }
                }
                Err(e) => repair.push(format!("keep-restore read {id}: {e}")),
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
        // Hand-edited restore skips count as conflicts so an apply whose only intent was such a revert
        // reports a no-effect, not a clean success (codex R7-#3). Folded in before the driver so both
        // the driver-fault early-return and the normal path carry them.
        let mut conflicts: Vec<ItemId> = std::mem::take(&mut restore_skipped);
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
        let apply = match TxnDriver::new(self.platform.reader, self.platform.applier, self.platform.assets)
            .apply(txn_id, requests, journal, ledger)
        {
            Ok(a) => a,
            Err(e) => {
                // The driver faulted OUTSIDE its transactional envelope (a batch rollback returns
                // Ok-with-error, NOT Err). Keep-reverts above may already have touched the desktop, so
                // return the authoritative state + a repair note rather than a bare Err over a
                // half-changed desktop (codex R3-Block 4).
                repair.push(format!("apply driver: {e}"));
                let stores = self.read_state_or_degraded(history, ledger, journal, &mut repair);
                return Ok(IconApplyOutcome {
                    committed: Vec::new(),
                    reverted,
                    conflicts,
                    // A driver bare-Err can escape only from a journal-append fault, which may be AFTER
                    // the desktop mutated — treat it as possibly-changed so the host never says
                    // "nothing changed" (codex R5-#1). The `degraded` path already routes it there.
                    desktop_mutated: true,
                    error: None,
                    degraded: Some(repair.join("; ")),
                    stores,
                });
            }
        };
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
        // The ①→②→③ finalize below runs AFTER the driver already committed the desktop. A runtime
        // failure of any step must NOT bubble a bare Err (the UI would say "nothing changed" over a
        // freshly-styled desktop — codex R3-Block 4): record it into `repair` and press on, so the op
        // returns the authoritative persisted state + ok:false. (A crash BETWEEN these writes is the
        // separately-documented, self-healing finalize crash-window — a distinct, accepted gap.)
        // ...AND only for a genuinely COMPLETED Apply (codex R5-#2 / R6-#2 / R7-#2). Two guards:
        //   • `repair.is_empty()` — no keep-restore fault ran. A partial revert (icon A reverted, icon
        //     B's restore faulted → still styled) is NOT complete: writing ② ("everything original")
        //     while B still wears the old look would resume from a lie next launch.
        //   • NOT an all-styling-attempts-failed batch — masters were sent but every one CAS-conflicted
        //     (`packaged` non-empty, `committed` empty): writing ② would poison the saved-style with a
        //     look the desktop never wears. A batch with NO masters (a pure revert-only or a policy-only
        //     Apply that intentionally styles nothing) still writes ②③ — that is the completed Apply's
        //     saved style, and ③ records it (spec 07 §8.2).
        let all_styling_attempts_failed = !packaged.is_empty() && apply.committed.is_empty();
        if apply.error.is_none() && repair.is_empty() && !all_styling_attempts_failed {
            if let Err(e) = self.settings.set_saved_style(Some(&style)) {
                repair.push(format!("save ② style: {e}"));
            }
            if let Err(e) = history.push(LookVersion {
                id: look_id.into(),
                created_at,
                label,
                pinned: false,
                icon_style: style,
            }) {
                repair.push(format!("push ③ history: {e}"));
            }
        }

        // Collect assets orphaned by this apply (an item's superseded ICO). Live = the ledger's
        // referenced assets UNION the in-flight journal's, so nothing a durable-but-unreconciled
        // record still points at is dropped (the lock keeps a CONCURRENT apply out; this union
        // covers the same-call in-flight window). A GC fault only strands orphan bytes (disk waste
        // that the next GC reclaims) — never fail a committed apply over it (codex R3-Block 4).
        match live_asset_hashes(ledger, journal) {
            Ok(live) => {
                if let Err(e) = self.platform.assets.gc(&live) {
                    repair.push(format!("gc: {e}"));
                }
            }
            Err(e) => repair.push(format!("gc live-set: {e}")),
        }

        let stores = self.read_state_or_degraded(history, ledger, journal, &mut repair);
        Ok(IconApplyOutcome {
            committed: apply.committed,
            reverted,
            conflicts,
            desktop_mutated: apply.desktop_mutated,
            error: apply.error,
            degraded: (!repair.is_empty()).then(|| repair.join("; ")),
            stores,
        })
    }

    /// Reads stores ②③ + the ledger for the finalize read-back; on a runtime read fault (after the
    /// desktop already committed) it records a repair note and returns a minimal safe snapshot rather
    /// than bubbling a bare Err (codex R3-Block 4). `history.all()` is an in-memory clone (infallible).
    fn read_state_or_degraded(
        &self,
        history: &LookHistoryStore,
        ledger: &dyn LedgerStore,
        journal: &dyn JournalSink,
        repair: &mut Vec<String>,
    ) -> IconStoreState {
        match self.read_state(history, ledger, journal) {
            Ok(s) => s,
            Err(e) => {
                repair.push(format!("state read-back: {e}"));
                // A read fault means the applied-state is UNKNOWN, not authoritatively false. This
                // runs only AFTER a mutation landed, so fail CLOSED toward "possibly applied"
                // (`applied: true`) — the restore affordance keys off it, and hiding it over a mere
                // read fault would strand the user with a changed desktop and no way back (codex
                // R4-Block 3). `history.all()` is an in-memory clone (infallible); saved_style falls
                // back to None (unknown) — a benign default the frontend treats as "not the current".
                IconStoreState { saved_style: None, history: history.all(), applied: true }
            }
        }
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
        let recovery = recover_from_journal(journal, self.platform.reader, self.platform.applier, ledger)?;
        // Recovery of a prior crash either could not fully reconcile (`degraded`, codex R4-Block 5) OR
        // cleanly ABORTED an interrupted transaction — restoring the desktop (codex R5-#3). In either
        // case do NOT reset on top: the strict `journal.checkpoint(&[])?` below is a bare `?` that, if
        // it faulted after a recovery abort already moved the desktop, would surface as a bare Err over
        // a mutated desktop. Return repair-required + the authoritative state; the journal stays for the
        // next pass (a clean abort already checkpointed it, so the retry proceeds normally). A
        // reconcile-only recovery (no desktop change) is safe to reset on and does not early-return.
        if !recovery.degraded.is_empty() || !recovery.aborted.is_empty() {
            let mut repair = recovery.degraded;
            if !recovery.aborted.is_empty() {
                repair.push(format!(
                    "recovered {} interrupted item(s) from a prior crash — re-syncing before resetting",
                    recovery.aborted.len()
                ));
            }
            let stores = self.read_state_or_degraded(history, ledger, journal, &mut repair);
            return Ok(IconResetOutcome {
                restored: Vec::new(),
                skipped: Vec::new(),
                degraded: Some(repair.join("; ")),
                deferred: true,
                stores,
            });
        }
        journal.checkpoint(&[])?;

        // Best-effort revert: reverting item N mutates the desktop, so a fault on item N+1 must not
        // bail with a bare `Err` over the items already reverted (codex R3-Block 4). Each item is
        // independent + CAS-gated; a fault records a repair note and the loop presses on. An item
        // whose revert faulted stays styled (its ledger row stays → desktop == ledger, self-heals).
        let mut repair: Vec<String> = Vec::new();
        let mut restored = Vec::new();
        let mut skipped = Vec::new();
        for entry in ledger.all()? {
            match self.platform.reader.read_fingerprint(&entry.target) {
                // The user deleted the icon: clear its row (its ICO becomes collectable below).
                Err(PortError::NotFound(_)) => {
                    if let Err(e) = ledger.remove(&entry.item) {
                        repair.push(format!("reset ledger drop {}: {e}", entry.item.as_str()));
                    }
                }
                // Already the pristine ORIGINAL: a prior revert landed but its ledger row lingered
                // (e.g. a `ledger.remove` fault, codex R4-Block 2). This is NOT a hand-edit — heal the
                // row so it can't later poison a reset (false "hand-edit") or a re-apply (stale CAS
                // anchor). Checked BEFORE the hand-edit arm since `original != last_applied`.
                Ok(cur) if cur == entry.original_fingerprint => {
                    if let Err(e) = ledger.remove(&entry.item) {
                        repair.push(format!("reset heal-remove {}: {e}", entry.item.as_str()));
                    }
                }
                // ★ Trust-first: the current state matches neither what we applied NOR the original, so
                // the user hand-edited it — leave it, count it toward "已跳过 N 项(你自己改过)".
                Ok(cur) if cur != entry.last_applied_fingerprint => skipped.push(entry.item),
                // Still exactly our applied state: revert to the true original and drop the row.
                Ok(_) => {
                    if let Err(e) = self.platform.applier.restore(&entry.target, &entry.original_anchor) {
                        repair.push(format!("reset {}: {e}", entry.item.as_str()));
                        continue;
                    }
                    if let Err(e) = ledger.remove(&entry.item) {
                        repair.push(format!("reset ledger remove {}: {e}", entry.item.as_str()));
                    }
                    restored.push(entry.item);
                }
                // An infrastructure fault (locked file, COM/registry) reading ONE item is recorded —
                // the operator still learns the restore path is compromised (via `degraded`), but the
                // items already reverted are not abandoned to a bare Err (spec 07 §10 / codex R3-Block 4).
                Err(e) => repair.push(format!("reset read {}: {e}", entry.item.as_str())),
            }
        }
        // Collect every asset the now-shrunk ledger no longer references. Skipped rows keep theirs;
        // an empty ledger collects everything. A GC fault only strands orphan bytes — never fail a
        // reset that already reverted the desktop over it.
        match ledger_asset_hashes(ledger) {
            Ok(live) => {
                if let Err(e) = self.platform.assets.gc(&live) {
                    repair.push(format!("reset gc: {e}"));
                }
            }
            Err(e) => repair.push(format!("reset gc live-set: {e}")),
        }
        // ② cleared: after a reset there is no current global style, so the resident is dormant.
        if let Err(e) = self.settings.set_saved_style(None) {
            repair.push(format!("reset clear ②: {e}"));
        }

        let stores = self.read_state_or_degraded(history, ledger, journal, &mut repair);
        Ok(IconResetOutcome {
            restored,
            skipped,
            degraded: (!repair.is_empty()).then(|| repair.join("; ")),
            deferred: false,
            stores,
        })
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
