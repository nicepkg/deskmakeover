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
pub mod native_bake;
pub mod scope;
pub mod style_resolve;
pub mod version_switch;

pub use package::{package_masters, BufferedMaster, PackagedItem};

use std::collections::HashSet;

use dm_contracts::IconStyle;
use dm_domain::{
    AssetRef, AssetStore, DesktopItem, ElevatedApplyItem, ElevatedIconApplier, ElevatedOutcome,
    ElevatedRestoreItem, Fingerprint, IconApplier, ItemId, ItemKind, ItemStateReader, ItemTarget,
    OwnedFields, PortError, RestoreAnchor,
};

use crate::error::{OperationError, Result};
use crate::ledger::entry::{LedgerEntry, TxnState};
use crate::ledger::{LedgerStore, LookHistoryStore, LookVersion};
use crate::settings_store::SettingsStore;
use crate::txn::{
    recover_from_journal, ApplyOutcome, ApplyRequest, JournalRecord, JournalSink, TxnDriver,
    TxnIdAllocator,
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
    /// A shortcut's raw icon location `(path, index)` captured from the SAME read as `fingerprint`
    /// (via `read_styleable_surface`), so the two never disagree. It is the elevated helper's
    /// compare-and-swap anchor for a privileged shortcut — sourced here (not from `item.icon`, which
    /// is a SEPARATE scan read that could observe a different state) so a value the preflight rejected
    /// is never handed to the helper as "expected" (§P1-1). `None` for a non-shortcut / icon-less item.
    pub cas_icon: Option<(String, i32)>,
    /// The ONE apply-authority bit (codex icons2-🟠5): false when the scan could not establish a
    /// trustworthy source/state for this item (extraction fault, unreadable fingerprint, or an
    /// unreconciled journal). The commit REFUSES such an item even if a client submits a master
    /// for it — the DTO's `styleable`, this bit, and the restore planner share one definition.
    pub source_ok: bool,
}

/// Untrusted-input ceilings for an apply session — the webview supplies `count` + the masters, so
/// these bound memory before the commit validates (audit F4). A real desktop is a few hundred icons,
/// each master a ~256² PNG (~350 KiB base64); these are generous upper bounds only a hostile/broken
/// caller reaches. The host enforces them at the `applyBaked*` command boundary.
pub const MAX_APPLY_MASTERS: usize = 8192;
/// Never pre-reserve more than this from a caller-supplied `count` hint (the Vec still grows).
pub const MAX_PREALLOC_MASTERS: usize = 4096;
/// Per-master base64 ceiling (a 512² RGBA PNG base64 is ~1.4 MiB; 8 MiB is generous headroom).
pub const MAX_MASTER_B64_BYTES: usize = 8 * 1024 * 1024;
/// Cumulative base64 ceiling across one session's masters.
pub const MAX_SESSION_B64_BYTES: usize = 256 * 1024 * 1024;
/// Ceiling on the commit's styleJson recipe string (a resolved recipe is small JSON).
pub const MAX_STYLE_JSON_BYTES: usize = 1024 * 1024;
/// Ceiling on a caller-supplied look/version label.
pub const MAX_LABEL_BYTES: usize = 4096;

/// The chunk-buffer for one apply (`begin` → `push`\* → commit). Owns no ports; it just
/// accumulates the frontend's baked masters until the commit packages + applies them.
pub struct IconApplySession {
    revision: u32,
    expected: usize,
    masters: Vec<BufferedMaster>,
    /// Running total of buffered base64 bytes, for the host's cumulative-byte cap (audit F4).
    bytes: usize,
}

impl IconApplySession {
    /// Starts a session for scan `revision`, expecting `count` masters (a completeness hint the
    /// host can check; the commit tolerates a short/over buffer and reconciles against the scan).
    /// The pre-reservation is bounded so a hostile `count` hint cannot force a huge up-front alloc.
    pub fn begin(revision: u32, count: usize) -> Self {
        Self {
            revision,
            expected: count,
            masters: Vec::with_capacity(count.min(MAX_PREALLOC_MASTERS)),
            bytes: 0,
        }
    }

    /// Buffers one baked master. `source_index` 0 = primary, 1 = the paired empty (Recycle Bin).
    pub fn push(&mut self, item_id: impl Into<String>, source_index: u32, png_base64: impl Into<String>) {
        let png_base64 = png_base64.into();
        self.bytes = self.bytes.saturating_add(png_base64.len());
        self.masters.push(BufferedMaster {
            item_id: item_id.into(),
            source_index,
            png_base64,
        });
    }

    /// Total buffered base64 bytes so far (the host's cumulative-byte cap reads this).
    pub fn bytes(&self) -> usize {
        self.bytes
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
    /// True when this Apply COMPLETED and persisted its global intent to ②③ (codex R9-#2): it either
    /// had a real desktop effect (`committed`/`reverted` non-empty) OR was a conflict-free zero-target
    /// Apply (a policy-only / intentionally-empty global Apply — no icons to touch, but the
    /// kindPolicy/typeOverrides intent still becomes the saved style, spec 07 §8.2). False for a
    /// zero-effect batch WITH conflicts (nothing landed, something was refused) and for any
    /// error/repair path. The host's verdict consumes THIS flag — the ②③ write and `ok`/dirty-clear
    /// share one predicate and can never disagree (codex R8-#2/#3).
    pub intent_persisted: bool,
    /// True when the cached scan must be FENCED before another apply (codex R9-#1): a stale poison
    /// row was healed (dropped) this round, so a same-revision retry — now row-less — would pass the
    /// ordinary fresh CAS and could silently overwrite a user's manual restore-to-original (the ABA).
    /// Also set on a driver bare-Err (the heal set is unknown there; fencing is the safe side). The
    /// host bumps the scan revision off it, structurally forcing a rescan before the next apply.
    pub requires_rescan: bool,
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
        scope: &scope::ScopeRoots,
        elevated: Option<&dyn ElevatedIconApplier>,
        txn: &mut TxnIdAllocator,
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
        history: &mut LookHistoryStore,
    ) -> Result<IconApplyOutcome> {
        // #5 commit→ledger gap: reconcile any journal-committed-but-unledgered transaction into the
        // ledger (and checkpoint the journal) BEFORE preparing, so the CAS anchors + the GC live set
        // below are computed against a ledger that reflects every durable commit. Idempotent.
        let recovery = recover_from_journal(journal, self.platform.reader, self.platform.applier, ledger, scope)?;
        // Recovery of a PRIOR crash's journal either could not fully reconcile (`degraded`, codex
        // R4-Block 5) OR cleanly ABORTED an interrupted transaction — which RESTORES the desktop
        // (codex R5-#3). In EITHER case do NOT stack a new apply on top: the abort already mutated the
        // desktop, so any bare `?` later in this same call (the master validation below, a store read)
        // would surface as "nothing changed" over a desktop recovery just moved. Return the
        // authoritative state + a repair-required note (never a bare Err) so the UI re-syncs and the
        // user retries; the journal was left intact for the next pass (a clean abort already
        // checkpointed it, so the retry finds an empty journal and proceeds normally). Reconcile-only
        // recovery (ledger gap close, no desktop change) is safe to build on and does not early-return.
        if !recovery.degraded.is_empty() || recovery.moved_or_uncertain() {
            let mut repair = recovery.degraded;
            if !recovery.aborted.is_empty() {
                repair.push(format!(
                    "recovered {} interrupted item(s) from a prior crash — re-syncing before re-applying",
                    recovery.aborted.len()
                ));
            }
            if !recovery.preserved.is_empty() {
                // Never-clobber (recovery:265): items left exactly as found because we could not
                // confirm they were ours — surface for review, never silently overwritten.
                repair.push(format!(
                    "left {} item(s) as found (edited or uncertain since a prior crash) — review before re-applying",
                    recovery.preserved.len()
                ));
            }
            let stores = self.read_state_or_degraded(history, ledger, journal, &mut repair);
            return Ok(IconApplyOutcome {
                committed: Vec::new(),
                reverted: Vec::new(),
                conflicts: Vec::new(),
                desktop_mutated: !recovery.aborted.is_empty(),
                intent_persisted: false,
                // Recovery moved the desktop or left uncertain state → the cached scan is stale; fence it.
                requires_rescan: true,
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
        // The privileged shared items (Public Desktop / ProgramData) that ride ONE elevated batch,
        // each paired with the scan-observed icon location — the trust-first CAS anchor the helper
        // checks before writing (captured at scan, threaded UNCHANGED, never re-read: §P1-1).
        let mut privileged: Vec<(ApplyRequest, (String, i32))> = Vec::new();
        // Hand-edited restore skips count as conflicts so an apply whose only intent was such a revert
        // reports a no-effect, not a clean success (codex R7-#3). Folded in before the driver so both
        // the driver-fault early-return and the normal path carry them.
        let mut conflicts: Vec<ItemId> = std::mem::take(&mut restore_skipped);
        for pkg in &packaged {
            let Some(scanned) = by_id.get(pkg.item_id.as_str()) else {
                conflicts.push(ItemId::from_raw(&pkg.item_id));
                continue;
            };
            if !scanned.item.can_style() || !scanned.source_ok {
                conflicts.push(scanned.item.id.clone());
                continue;
            }
            let req = ApplyRequest {
                target: scanned.item.target(),
                expected_fingerprint: scanned.fingerprint,
                owned: OwnedFields::icon_only(),
                asset_hash: pkg.primary.content_hash.clone(),
                asset_bytes: pkg.primary.bytes.clone(),
                empty_asset_bytes: pkg.empty.as_ref().map(|e| e.bytes.clone()),
                pinned_seed: None,
            };
            // Partition by write scope. A privileged-scope target (Public Desktop / ProgramData) an
            // unelevated write can NEVER touch (Access Denied) routes to the elevated batch IFF a port
            // is wired AND its kind is one the helper writes (a real `.lnk`: Shortcut / AppxShortcut).
            // Any other privileged case is an honest `conflict` (skipped) — never a doomed unelevated
            // write that always fails, then rolls the WHOLE user-desktop batch back (the on-box bug).
            if scope.classify(&req.target.path).is_some() {
                if elevated.is_some() && is_elevatable_kind(req.target.kind) {
                    // The CAS anchor is the icon location captured from the SAME read as the accepted
                    // fingerprint (`cas_icon`, not the separate `item.icon` scan read) — so the helper
                    // is never told to "expect" a value the preflight did not accept (§P1-1). `None`
                    // (icon-less) → ("", 0); the helper's `GetIconLocation` reads "" for the same
                    // target, so the `"" == ""` CAS matches.
                    let expect = scanned.cas_icon.clone().unwrap_or_default();
                    privileged.push((req, expect));
                } else {
                    conflicts.push(req.target.id.clone());
                }
            } else {
                requests.push(req);
            }
        }

        let txn_id = txn.next_id();
        let mut apply = match TxnDriver::new(self.platform.reader, self.platform.applier, self.platform.assets)
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
                    intent_persisted: false,
                    // The heal set is unknown on a bare-Err (a poison row may already have been dropped
                    // in preflight) — fence the scan on the safe side (codex R9-#1).
                    requires_rescan: true,
                    error: None,
                    degraded: Some(repair.join("; ")),
                    stores,
                });
            }
        };
        // The privileged shared items run as ONE elevated batch (one UAC), AFTER the user's own icons
        // (so a UAC cancel still lands the unelevated apply). Its outcome folds into `apply` so every
        // downstream verdict — committed / conflicts / desktop_mutated / error / heal-fence — treats
        // both batches uniformly. A privileged Declined (UAC cancel) reports its items as conflicts (a
        // retryable skip), not an error; a Failed/port-fault surfaces as an error.
        if let (Some(elev), false) = (elevated, privileged.is_empty()) {
            let priv_txn = txn.next_id();
            match self.apply_privileged_batch(privileged, elev, priv_txn, journal, ledger) {
                Ok(mut e) => {
                    apply.committed.append(&mut e.committed);
                    apply.conflicts.append(&mut e.conflicts);
                    apply.healed.append(&mut e.healed);
                    apply.desktop_mutated |= e.desktop_mutated;
                    apply.error = join_errors(apply.error.take(), e.error);
                }
                Err(e) => {
                    // A journal/ledger fault escaped the elevated batch's envelope (bare Err); the
                    // helper may have written before an append faulted, so treat the desktop as
                    // possibly-changed (never "nothing changed") and record the repair note.
                    repair.push(format!("elevated apply driver: {e}"));
                    apply.desktop_mutated = true;
                }
            }
        }
        conflicts.append(&mut apply.conflicts);

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
        // ...AND only for a genuinely COMPLETED Apply (codex R5-#2 / R6-#2 / R7-#2 / R8-#2,#3 /
        // R9-#2). `intent_persisted` is THE single predicate — the host's verdict consumes it, so the
        // ②③ write and `ok:true`/dirty-clear can never disagree. Completed means:
        //   • no batch error and `repair.is_empty()` — no keep-restore fault ran. A partial revert (A
        //     reverted, B's restore faulted → still styled) is NOT complete: writing ② ("everything
        //     original") while B wears the old look would resume from a lie next launch.
        //   • AND (a real desktop effect landed (`committed || reverted`) OR nothing was even refused
        //     (`conflicts.is_empty()`)). The conflict-free zero-target case is a POLICY-ONLY /
        //     intentionally-empty global Apply — no icon currently needs touching, but the new
        //     kindPolicy/typeOverrides intent must still persist to ② (spec 07 §8.2, codex R9-#2) or
        //     it is silently lost on restart. A zero-effect apply WITH conflicts (all-conflicts, or a
        //     restore-only batch whose every opt-out was a hand-edit) did not complete: writing ② would
        //     poison the saved-style with a look the desktop never wears (R8-#2).
        let meaningful_apply = !apply.committed.is_empty() || !reverted.is_empty();
        let intent_persisted =
            apply.error.is_none() && repair.is_empty() && (meaningful_apply || conflicts.is_empty());
        if intent_persisted {
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
            intent_persisted,
            requires_rescan: !apply.healed.is_empty(),
            error: apply.error,
            degraded: (!repair.is_empty()).then(|| repair.join("; ")),
            stores,
        })
    }

    /// Applies the privileged shared items (Public Desktop / ProgramData `.lnk`s) as ONE elevated
    /// batch (one UAC), wrapped in the SAME durable journal + ledger envelope the in-process driver
    /// uses — so a privileged item is exactly as reversible + crash-safe as a user-desktop one. The
    /// caller folds the returned [`ApplyOutcome`] into the main apply outcome.
    ///
    /// Journal order (crash-safety — every window recovers to a consistent, reversible terminal; full
    /// table in `docs/plans/2026-07-16-elevated-desktop-items-wiring.md`): TxnBegin → per item
    /// {ItemPrepared (anchor durable) → assets.put + AssetWritten} → per item ItemApplied{new_fp =
    /// `elevated.plan`} (the DERIVED post-apply fingerprint, written BEFORE the helper so scope-aware
    /// recovery recognises a helper-styled item as ours and adopts it forward) → `elevated.apply` (ONE
    /// UAC). Applied → TxnCommitted + ledger.upsert(all); Declined (UAC cancel) → TxnRolledBack + the
    /// items reported as conflicts (a retryable skip, not a failure); Failed / port-Err → TxnRolledBack
    /// + error. The helper LIFO-rolls-back its OWN writes on any internal failure, so a Failed leaves
    /// the desktop original; a crash mid-helper is adopted forward per item by recovery.
    fn apply_privileged_batch(
        &self,
        requests: Vec<(ApplyRequest, (String, i32))>,
        elevated: &dyn ElevatedIconApplier,
        txn_id: u64,
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
    ) -> Result<ApplyOutcome> {
        let mut outcome = ApplyOutcome::default();
        let driver =
            TxnDriver::new(self.platform.reader, self.platform.applier, self.platform.assets);
        // Split the scan-observed CAS anchors out (keyed by item id) before prepare consumes the
        // requests; they are threaded UNCHANGED into each manifest row (never re-read — §P1-1).
        let mut expects: std::collections::HashMap<String, (String, i32)> =
            std::collections::HashMap::with_capacity(requests.len());
        let mut plain: Vec<ApplyRequest> = Vec::with_capacity(requests.len());
        for (req, expect) in requests {
            expects.insert(req.target.id.as_str().to_string(), expect);
            plain.push(req);
        }
        // Phase 1 (CAS + anchor capture) — the SAME trust-first logic the in-process driver runs.
        let prepared = match driver.prepare_batch(plain, ledger) {
            Ok(p) => p,
            Err(e) => {
                outcome.error = Some(format!("elevated apply preflight failed: {e}"));
                return Ok(outcome);
            }
        };
        outcome.conflicts = prepared.conflicts;
        outcome.healed = prepared.healed;
        if prepared.proceeding.is_empty() {
            return Ok(outcome);
        }

        // Monotonic txn guard (mirrors the driver): a reused id would merge two txns' records into one
        // recovery group. Nothing has been journaled or mutated yet, so a violation aborts cleanly.
        let max_seen = journal.read_all()?.iter().map(|r| r.txn()).max().unwrap_or(0);
        if txn_id <= max_seen {
            return Err(OperationError::Journal(format!(
                "elevated txn id {txn_id} is not monotonic (journal holds up to {max_seen}); ids must never be reused"
            )));
        }
        journal.append(&JournalRecord::TxnBegin {
            txn: txn_id,
            items: prepared.proceeding.iter().map(|(r, ..)| r.target.id.clone()).collect(),
        })?;

        // Per item: journal the restore anchor (ItemPrepared) + stage the styled ICO (AssetWritten),
        // both flushed BEFORE any elevated write, so recovery always has the anchor + asset ref.
        let mut items: Vec<ElevatedApplyItem> = Vec::with_capacity(prepared.proceeding.len());
        let mut rows: Vec<PreparedRow> = Vec::with_capacity(prepared.proceeding.len());
        for (req, anchor, original_fp, expected) in &prepared.proceeding {
            journal.append(&JournalRecord::ItemPrepared {
                txn: txn_id,
                item: req.target.id.clone(),
                target: req.target.clone(),
                anchor: anchor.clone(),
                original_fingerprint: *original_fp,
                expected_fingerprint: *expected,
                asset_hash: req.asset_hash.clone(),
                owned: req.owned,
                pinned_seed: req.pinned_seed,
            })?;
            let asset = self.platform.assets.put(&req.asset_hash, &req.asset_bytes)?;
            journal.append(&JournalRecord::AssetWritten {
                txn: txn_id,
                item: req.target.id.clone(),
                asset: asset.clone(),
                empty: None,
            })?;
            let (expect_icon, expect_index) =
                expects.get(req.target.id.as_str()).cloned().unwrap_or_default();
            items.push(ElevatedApplyItem {
                target: req.target.clone(),
                asset_path: asset.path.clone(),
                expect_icon,
                expect_index,
            });
            rows.push(PreparedRow {
                target: req.target.clone(),
                anchor: anchor.clone(),
                original_fingerprint: *original_fp,
                owned: req.owned,
                pinned_seed: req.pinned_seed,
                asset,
            });
        }

        // The DERIVED post-apply fingerprint per item — identical to what the in-process applier's
        // `expected_after_apply` produces — journaled as ItemApplied BEFORE the helper runs. This is
        // the crux of crash-safety: recovery recognises a helper-styled privileged item (live ==
        // new_fingerprint) as ours and adopts it forward, rather than attempting a doomed unelevated
        // restore. `plan` is pure + read-only (no UAC, no write).
        let planned = elevated.plan(&items)?;
        if planned.len() != items.len() {
            journal.append(&JournalRecord::TxnRolledBack { txn: txn_id })?;
            outcome.error = Some("elevated plan returned a mismatched fingerprint count".into());
            return Ok(outcome);
        }
        for (item, fp) in items.iter().zip(&planned) {
            journal.append(&JournalRecord::ItemApplied {
                txn: txn_id,
                item: item.target.id.clone(),
                new_fingerprint: *fp,
            })?;
        }

        // ONE elevated call (one UAC). The helper independently re-confirms every target + icon,
        // CAS-re-checks, writes, and atomically LIFO-rolls-back its OWN writes on any internal failure.
        match elevated.apply(&items) {
            Ok(ElevatedOutcome::Applied) => {
                journal.append(&JournalRecord::TxnCommitted { txn: txn_id })?;
                for (row, new_fp) in rows.into_iter().zip(planned) {
                    let version = ledger.next_version()?;
                    let id = row.target.id.clone();
                    ledger.upsert(LedgerEntry {
                        item: id.clone(),
                        target: row.target,
                        original_fingerprint: row.original_fingerprint,
                        original_anchor: row.anchor,
                        last_applied_fingerprint: new_fp,
                        owned: row.owned,
                        asset: row.asset,
                        empty_asset: None,
                        state: TxnState::Committed,
                        pinned_seed: row.pinned_seed,
                        version,
                    })?;
                    outcome.committed.push(id);
                }
                outcome.desktop_mutated = true;
            }
            // A UAC cancel is a user CHOICE, and it means the helper process NEVER STARTED
            // (`ShellExecuteEx` → ERROR_CANCELLED), so the desktop is DEFINITELY untouched — a clean
            // `TxnRolledBack` is honest here. Report the items as conflicts (a retryable skip), never a
            // hard error, which would withhold the chosen style's ②③ persistence for the icons that DID
            // apply.
            Ok(ElevatedOutcome::Declined) => {
                journal.append(&JournalRecord::TxnRolledBack { txn: txn_id })?;
                for it in &items {
                    outcome.conflicts.push(it.target.id.clone());
                }
            }
            // The helper RAN and failed. Its internal rollback is BEST-EFFORT (a rollback write can
            // itself fault), so the desktop state is UNCERTAIN — some items may still be styled (§P1-2).
            // Do NOT journal a `TxnRolledBack` terminal (that would tell recovery "cleanly reverted,
            // nothing to do" over possible residue, leaving a styled item untracked → irreversible).
            // Leave the txn TERMINAL-LESS: the next operation's crash recovery inspects each live item
            // and ADOPTS FORWARD any still-styled (privileged) one, dropping the untouched originals.
            Ok(ElevatedOutcome::Failed(e)) => {
                outcome.desktop_mutated = true;
                outcome.error = Some(format!("elevated apply failed: {e}"));
            }
            // The port faulted around the helper (launch/wait/exit-read) — the helper MAY have run and
            // written. Same as Failed: leave the txn terminal-less for recovery, and treat the desktop
            // as possibly-changed.
            Err(e) => {
                outcome.desktop_mutated = true;
                outcome.error = Some(format!("elevated apply port error: {e}"));
            }
        }
        Ok(outcome)
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
        scope: &scope::ScopeRoots,
        elevated: Option<&dyn ElevatedIconApplier>,
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
        let recovery = recover_from_journal(journal, self.platform.reader, self.platform.applier, ledger, scope)?;
        // Recovery of a prior crash either could not fully reconcile (`degraded`, codex R4-Block 5) OR
        // cleanly ABORTED an interrupted transaction — restoring the desktop (codex R5-#3). In either
        // case do NOT reset on top: the strict `journal.checkpoint(&[])?` below is a bare `?` that, if
        // it faulted after a recovery abort already moved the desktop, would surface as a bare Err over
        // a mutated desktop. Return repair-required + the authoritative state; the journal stays for the
        // next pass (a clean abort already checkpointed it, so the retry proceeds normally). A
        // reconcile-only recovery (no desktop change) is safe to reset on and does not early-return.
        if !recovery.degraded.is_empty() || recovery.moved_or_uncertain() {
            let mut repair = recovery.degraded;
            if !recovery.aborted.is_empty() {
                repair.push(format!(
                    "recovered {} interrupted item(s) from a prior crash — re-syncing before resetting",
                    recovery.aborted.len()
                ));
            }
            if !recovery.preserved.is_empty() {
                repair.push(format!(
                    "left {} item(s) as found (edited or uncertain since a prior crash) — review before resetting",
                    recovery.preserved.len()
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
        // A clean recovery already reconciled + emptied the journal above; this strict checkpoint is a
        // belt-and-suspenders guarantee that no committed record can outlive the ledger delete below and
        // resurrect a styled row at next launch. Run it ONLY when records actually remain (codex R8-#4):
        // an EMPTY journal here would otherwise try to `remove_file` a zero-byte log, and an undeletable
        // one (ACL) would bare-Err a reset that has nothing left to checkpoint.
        if !journal.read_all()?.is_empty() {
            journal.checkpoint(&[])?;
        }

        // Best-effort revert: reverting item N mutates the desktop, so a fault on item N+1 must not
        // bail with a bare `Err` over the items already reverted (codex R3-Block 4). Each item is
        // independent + CAS-gated; a fault records a repair note and the loop presses on. An item
        // whose revert faulted stays styled (its ledger row stays → desktop == ledger, self-heals).
        let mut repair: Vec<String> = Vec::new();
        let mut restored = Vec::new();
        let mut skipped = Vec::new();
        // Privileged (Public Desktop / ProgramData) rows still wearing our style, collected here and
        // reverted as ONE elevated batch after the walk (one UAC, not one per item). Each carries its
        // true-original fingerprint so, after the batch, a per-item re-read can CONFIRM the revert
        // actually landed before its ledger row is dropped (§P2-1 — the helper silently skips an item
        // the user re-edited during the UAC prompt, and its exit code alone can't say which).
        let mut privileged_restores: Vec<(ElevatedRestoreItem, Fingerprint)> = Vec::new();
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
                    // §14 (audit F2b): a privileged-scope target (Public Desktop / ProgramData) still in
                    // OUR applied style cannot be reverted by the NON-elevated applier. Route it to the
                    // elevated restore BATCH (one UAC, run after the walk) when a port is wired AND the
                    // kind + anchor are ones the helper replays (a real `.lnk` FileBytes anchor — arm 3
                    // above already filtered a user re-edit, so a collected row is genuinely still ours).
                    // Otherwise — no port (unwired / `Unresolved` scope), or a kind/anchor the helper
                    // does not write — leave the row AND the desktop untouched and count it `skipped` (an
                    // accurate "left this item alone", never a doomed unelevated restore). The gate is
                    // DEEP (this arm only), so the SAFE ledger-healing arms above (deleted-row drop,
                    // already-original heal-remove) still run for a privileged row: they touch only the
                    // local ledger, never the privileged desktop (codex F2b-review).
                    if scope.classify(&entry.target.path).is_some() {
                        match (elevated, is_elevatable_kind(entry.target.kind), &entry.original_anchor) {
                            (Some(_), true, RestoreAnchor::FileBytes { bytes }) => {
                                privileged_restores.push((
                                    ElevatedRestoreItem {
                                        target: entry.target.clone(),
                                        original_bytes: bytes.clone(),
                                        applied_icon: entry.asset.path.clone(),
                                    },
                                    entry.original_fingerprint,
                                ));
                            }
                            _ => skipped.push(entry.item),
                        }
                        continue;
                    }
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
        // Revert the privileged rows through ONE elevated call (one UAC), replaying each captured
        // original `.lnk` bytes (CAS-guarded in the helper). No journal needed: the LEDGER holds the
        // durable anchor and the replay is idempotent, so a crash mid-restore self-heals on the next
        // reset via the already-original heal-remove arm — the SAME crash model as the unelevated reset
        // above. Applied → drop each row + count restored; Declined (UAC cancel) → keep the rows + count
        // them skipped (retryable, no fault); Failed / port-Err → keep the rows + skipped + a note.
        if let (Some(elev), false) = (elevated, privileged_restores.is_empty()) {
            let items: Vec<ElevatedRestoreItem> =
                privileged_restores.iter().map(|(it, _)| it.clone()).collect();
            match elev.restore(&items) {
                Ok(ElevatedOutcome::Applied) => {
                    // The helper reverted everything it COULD (it skips an item the user re-edited during
                    // the UAC prompt), but its exit code doesn't say which. CONFIRM each per item by a
                    // fresh (unprivileged) fingerprint read before dropping its row: back-to-original →
                    // drop + restored; anything else (a skipped UAC-window re-edit) → keep the row +
                    // skipped, so a user's edit is never left untracked (§P2-1).
                    for (it, original_fp) in &privileged_restores {
                        match self.platform.reader.read_fingerprint(&it.target) {
                            Ok(fp) if fp == *original_fp => {
                                if let Err(e) = ledger.remove(&it.target.id) {
                                    repair.push(format!("reset ledger remove {}: {e}", it.target.id.as_str()));
                                }
                                restored.push(it.target.id.clone());
                            }
                            Ok(_) => skipped.push(it.target.id.clone()),
                            Err(e) => {
                                // Can't confirm → keep the row (still tracked) + count it skipped.
                                repair.push(format!("reset confirm read {}: {e}", it.target.id.as_str()));
                                skipped.push(it.target.id.clone());
                            }
                        }
                    }
                }
                Ok(ElevatedOutcome::Declined) => {
                    for (it, _) in &privileged_restores {
                        skipped.push(it.target.id.clone());
                    }
                }
                Ok(ElevatedOutcome::Failed(e)) => {
                    repair.push(format!("elevated reset failed: {e}"));
                    for (it, _) in &privileged_restores {
                        skipped.push(it.target.id.clone());
                    }
                }
                Err(e) => {
                    repair.push(format!("elevated reset port error: {e}"));
                    for (it, _) in &privileged_restores {
                        skipped.push(it.target.id.clone());
                    }
                }
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
        // Reset coupling (spec 07 §10 ★): a reset is not just a ledger walk — to read as "as if
        // never modified" it MUST, in the SAME operation, clear ② saved-style AND turn OFF the
        // auto-format toggle. ONE atomic transaction (codex m7b-🟠5) so a crash between them can't
        // leave `②=NULL, toggle=true` (dormant, but the UI lies + a later Apply could revive it).
        // (The arrow-overlay restore is the host's — it owns the elevated OverlayControl.)
        if let Err(e) = self.settings.reset_style_and_autoformat() {
            repair.push(format!("reset clear ②+toggle: {e}"));
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

/// One privileged item's captured apply material, carried from Phase-1 prepare through the
/// post-helper ledger upsert (the elevated batch cannot interleave apply+upsert per item the way the
/// in-process driver's `Prepared` does — the whole batch styles in one helper call).
struct PreparedRow {
    target: ItemTarget,
    anchor: RestoreAnchor,
    original_fingerprint: Fingerprint,
    owned: OwnedFields,
    pinned_seed: Option<u32>,
    asset: AssetRef,
}

/// Whether a privileged-scope item of this kind can be styled through the elevated helper today. v1
/// covers the real `.lnk` kinds (Shortcut / AppxShortcut — the overwhelming majority of Public
/// Desktop items, e.g. Chrome), which the helper writes via COM `SetIconLocation`. Privileged `.url`
/// / folder / loose-file kinds use a different write mechanism and stay honest conflicts (§follow-ups
/// in the wiring plan), never a doomed unelevated write.
fn is_elevatable_kind(kind: ItemKind) -> bool {
    matches!(kind, ItemKind::Shortcut | ItemKind::AppxShortcut)
}

/// Joins the in-process and elevated batches' failure reasons into one message (either alone passes
/// through; both present are joined so neither is swallowed).
fn join_errors(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) => Some(format!("{a}; {b}")),
        (Some(a), None) => Some(a),
        (None, b) => b,
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
