//! The reconciler decision core (spec 07 §3-§6, §11): "events are hints, reconciliation is
//! truth." Each cycle re-derives the world from a full `DesktopScanner::scan()` — watch events
//! only decide WHEN a cycle runs (and back it up with the periodic full reconcile the Windows
//! watcher's silent-overflow limitation requires), so a deferred/busy/unstable item is never
//! lost: it simply classifies again next cycle.
//!
//! Write discipline: an incremental background apply goes through `TxnDriver::apply` — the SAME
//! entry point the manual flow uses — and writes ONLY store ① (spec 07 §5/§8: never ② saved-style,
//! never ③ look-history). The §14 red line is structural: this crate has no elevation dependency;
//! a privileged-scope item is enqueued to [`PendingPrivilegedQueue`] before ANY write path.

#[cfg(test)]
mod tests;

use dm_contracts::IconStyle;
use dm_domain::{
    ActivityMonitor, AssetStore, DesktopItem, Fingerprint, IconApplier, IconSourceExtractor,
    ItemId, ItemStateReader, OwnedFields, PortError,
};
use dm_domain::DesktopScanner;
use dm_operations::icons::native_bake::{bake_masters_par, BakeJob};
use dm_operations::icons::package_masters;
use dm_operations::icons::scope::ScopeRoots;
use dm_operations::icons::style_resolve::StyleRecipe;
use dm_operations::{
    recover_from_journal, ApplyRequest, BufferedMaster, JournalSink, LedgerStore, Result,
    TxnDriver, TxnIdAllocator,
};
use serde::{Deserialize, Serialize};

use crate::consent::{FreshnessInputs, TrustState};
use crate::pending_privileged::PendingPrivilegedQueue;
use crate::stability::{SettleProbe, StabilityReader};

/// The platform ports one cycle drives — all shared with the foreground stack, so the background
/// path cannot drift from it.
pub struct ReconcilerPorts<'a> {
    pub scanner: &'a dyn DesktopScanner,
    pub extractor: &'a dyn IconSourceExtractor,
    pub reader: &'a dyn ItemStateReader,
    pub applier: &'a dyn IconApplier,
    pub assets: &'a dyn AssetStore,
    pub activity: &'a dyn ActivityMonitor,
    pub stability: &'a dyn StabilityReader,
}

/// One cycle's policy inputs, supplied by the host (it owns the stores they come from).
pub struct ReconcileContext<'a> {
    /// ② saved-style. `None` = dormant — nothing to project (spec 07 §8.3).
    pub saved_style: Option<&'a IconStyle>,
    /// The earned trust tier (spec 07 §2 item 7) — consumed by the HOST to decide whether a
    /// proposal rides a toast; the reconciler no longer gates its own apply on it.
    pub trust: &'a TrustState,
    /// The intent-freshness signals (spec 07 §2 item 8) — a v1.1 silent-mode signal; dormant in
    /// v1 (which always proposes).
    pub freshness: FreshnessInputs,
    /// The privileged-scope roots (§6/§14). `Unresolved` on a Windows host that has not resolved its
    /// known folders classifies EVERY item privileged, so the reconciler routes them all to the
    /// pending-privileged queue rather than auto-styling machine-wide state (fail closed).
    pub scope: &'a ScopeRoots,
}

/// A candidate that has PASSED the full reconcile gate — scope-vetted (not privileged),
/// participation-vetted (styleable + kind-policy in), stability-settled — with the CAS
/// fingerprint SNAPSHOT captured at propose time. `apply_batch` uses this snapshot as the driver's
/// `expected_fingerprint`, so any change between propose and confirm/timeout (a user hand-edit
/// during the ≤2h window) fails CAS and is skipped, never overwritten (codex m7a-🔴1). Serializable
/// so the host can persist a pending proposal durably across the timeout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VettedCandidate {
    pub item: DesktopItem,
    /// The item's fingerprint at propose time — the CAS anchor for the eventual apply.
    pub fingerprint: Fingerprint,
}

/// One undoable item of an applied batch: the id plus the fingerprint THE BATCH left on it —
/// [`Reconciler::restore_batch`]'s CAS anchor. Anchoring on this snapshot (never on the ledger's
/// CURRENT `last_applied`) is load-bearing: a foreground re-style after the batch bumps the
/// ledger fingerprint, and an undo must then treat the item as superseded — restoring it to
/// original would wipe the user's newer look (codex 2026-07-16 P1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoTarget {
    pub id: ItemId,
    pub applied_fingerprint: Fingerprint,
}

/// What one reconcile cycle did / decided.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconcileOutcome {
    /// Candidates surfaced as a batched PROPOSAL (not applied) — the host owns the confirm /
    /// 2h-timeout surface and calls [`Reconciler::apply_batch`] with them on confirm/timeout. Each
    /// carries its snapshot CAS fingerprint (codex m7a-🔴1).
    pub proposed: Vec<VettedCandidate>,
    /// Items committed by `apply_batch` (the host's confirm/timeout entry). `reconcile` never
    /// populates this in v1 — it only proposes (spec §2 item 4).
    pub applied: Vec<ItemId>,
    /// The committed items WITH the fingerprint the batch left on each — the host arms the tray
    /// 「撤销最近一次整理」 with these (spec 07 §13 level 2; see [`UndoTarget`]).
    pub applied_snapshot: Vec<UndoTarget>,
    /// Items flagged, never touched: externally modified vs our ledger row, or ambiguous
    /// poison/manual-restore tuples (the background NEVER resolves ambiguity — the foreground
    /// heal path owns that).
    pub conflicts: Vec<ItemId>,
    /// The pending-privileged queue depth after this cycle (the tray "待处理特权项(N)" line).
    pub pending_privileged: usize,
    /// New items still failing the stability gate — retried next cycle, never dropped.
    pub deferred_unstable: Vec<ItemId>,
    /// The whole wave deferred because the desktop was busy (spec 07 §11) — nothing was scanned
    /// past the gate; the next cycle re-reconciles.
    pub deferred_busy: bool,
    /// A prior crash was recovered this cycle so the reconcile stood down to re-sync (codex
    /// r1-🟡3): distinct from `deferred_busy` (activity) so the host/UI reads the right reason.
    pub deferred_recovery: bool,
    /// Per-item degradations (extract/bake/read faults) — the rest of the cycle proceeds.
    pub errors: Vec<String>,
}

/// The long-lived reconciler: settle-probe memory + the pending-privileged queue. (Baking holds
/// no render state across `apply_batch` calls — the batch renders through the stateless
/// `bake_masters_par`, so an aborted batch can never leave registered-but-uncommitted sources
/// behind, codex r1-🟠3; a one-shot batch gains little from a cross-call warm cache anyway.)
pub struct Reconciler {
    settle: SettleProbe,
    pub pending_privileged: PendingPrivilegedQueue,
}

impl Default for Reconciler {
    fn default() -> Self {
        Self::new()
    }
}

impl Reconciler {
    pub fn new() -> Self {
        Self { settle: SettleProbe::new(), pending_privileged: PendingPrivilegedQueue::new() }
    }

    /// One reconcile cycle: recover any prior crash, classify the live desktop, queue privileged
    /// scopes, gate unstable newcomers, flag genuine conflicts, and surface the styleable
    /// newcomers as a batched PROPOSAL (v1 never auto-applies — spec §2 item 4). The host owns the
    /// confirm/2h-timeout surface and calls [`apply_batch`] with the returned candidates.
    pub fn reconcile(
        &mut self,
        ports: &ReconcilerPorts<'_>,
        ctx: &ReconcileContext<'_>,
        _txn: &mut TxnIdAllocator,
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
    ) -> Result<ReconcileOutcome> {
        let mut out = ReconcileOutcome::default();
        // Recover UNCONDITIONALLY, first thing, every cycle (codex m7a-🟠3): a crash-left
        // non-committed txn must not sit as a permanent black hole waiting for some unrelated new
        // item to happen along — the reconcile loop itself closes the recovery state machine. A
        // recovery that moved or could not verify the desktop stands the cycle down (don't stack
        // classification/proposals on a just-recovered desktop).
        let recovery = recover_from_journal(journal, ports.reader, ports.applier, ledger, ctx.scope)?;
        if !recovery.degraded.is_empty() || recovery.moved_or_uncertain() {
            out.errors.push("recovered a prior crash — re-syncing before the next cycle".into());
            out.deferred_recovery = true; // a re-sync, NOT activity (codex r1-🟡3)
            out.pending_privileged = self.pending_privileged.len();
            return Ok(out);
        }
        // ② empty → dormant: nothing to project (spec 07 §8.3 — no special-case styling paths).
        let Some(style) = ctx.saved_style else {
            out.pending_privileged = self.pending_privileged.len();
            return Ok(out);
        };
        // Scope unresolved (a Windows host before known-folder resolution) → DEFER the whole cycle.
        // We cannot tell a privileged item from a per-user one, so styling nothing AND queueing
        // nothing is the only safe move: treating every item as privileged would flood the pending
        // queue with the entire desktop and hand a later UAC batch ordinary user items (codex B1-🟠).
        if !ctx.scope.is_resolved() {
            out.errors.push("privileged-scope roots unresolved — deferring, styles nothing".into());
            out.pending_privileged = self.pending_privileged.len();
            return Ok(out);
        }
        // Busy desktop → the whole wave defers; events are hints, the next cycle reconciles
        // (spec 07 §3/§11). An activity-read fault reads as busy — err on the quiet side.
        if ports.activity.is_desktop_busy().unwrap_or(true) {
            out.deferred_busy = true;
            out.pending_privileged = self.pending_privileged.len();
            return Ok(out);
        }
        let recipe = StyleRecipe::parse(style)?;
        let items = ports.scanner.scan().map_err(op_err)?;
        self.settle.retain_paths(items.iter().map(|i| i.path.as_str()));

        let mut candidates: Vec<VettedCandidate> = Vec::new();
        for item in items {
            // §14 red line FIRST: privileged scope routes to the queue before ANY other path.
            if let Some(reason) = ctx.scope.classify(&item.path) {
                self.pending_privileged.push(item.target(), reason);
                continue;
            }
            if !item.can_style() {
                continue;
            }
            // Kind-policy opt-out / non-participating bucket → keep original (spec 07 §6).
            match recipe.effective_config(item.kind, item.kind.is_shortcut()) {
                Ok(Some(_)) => {}
                Ok(None) => continue,
                Err(e) => {
                    out.errors.push(format!("resolve {}: {e}", item.id.as_str()));
                    continue;
                }
            }
            match ledger.get(&item.id)? {
                Some(entry) if entry.state.is_committed() => {
                    match ports.reader.read_fingerprint(&item.target()) {
                        Ok(cur) if entry.is_unmodified(&cur) => {} // ours + intact — nothing to do
                        Ok(cur) if cur == entry.original_fingerprint => {
                            // Poison/manual-restore tuple (current == original != last_applied): the
                            // item is ALREADY at its true original, so there is nothing wrong for the
                            // user to act on — SILENTLY skip (codex m7a-🟡7). Flagging it as a
                            // conflict every cycle would re-alarm the tray forever (level-triggered);
                            // healing it (drop the row) is worse — next cycle it becomes a `None`
                            // new-item that, once settled, would restyle over a manual restore (the
                            // ABA). The foreground heal+fence owns resolving the stale row.
                        }
                        // A genuine external modification (current is neither ours nor the original):
                        // flag it — the user/installer changed a styled icon; never touch it.
                        Ok(_) => out.conflicts.push(item.id.clone()),
                        Err(PortError::NotFound(_)) => {} // vanished mid-cycle; next scan settles it
                        Err(e) => out.errors.push(format!("read {}: {e}", item.id.as_str())),
                    }
                }
                // A non-committed row is an in-flight/recovering txn — the unconditional recovery
                // above owns it; the classification stays out.
                Some(_) => {}
                None => {
                    // A NEW item: it formats only once its bytes have settled (spec 07 §3). The
                    // snapshot fingerprint captured HERE is the CAS anchor for the eventual apply.
                    let snap = ports.stability.snapshot(&item.path);
                    if !self.settle.observe(&item.path, snap) {
                        out.deferred_unstable.push(item.id.clone());
                        continue;
                    }
                    let fingerprint = match ports.reader.read_fingerprint(&item.target()) {
                        Ok(f) => f,
                        // Vanished between scan and the anchor read → benign, like the committed
                        // NotFound arm (codex r1-🟡2), not a fake error alarm.
                        Err(PortError::NotFound(_)) => continue,
                        Err(e) => {
                            out.errors.push(format!("anchor {}: {e}", item.id.as_str()));
                            continue;
                        }
                    };
                    candidates.push(VettedCandidate { item, fingerprint });
                }
            }
        }

        out.pending_privileged = self.pending_privileged.len();
        // v1 ALWAYS proposes (spec §2 item 4, codex m7b-🟠1): the host surfaces the confirmation +
        // 2h timeout and calls [`apply_batch`]. Pure silent mode is an explicit v1.1 opt-in
        // (item 5, out of scope); the 3-batch trust counter (`ctx.trust`) only decides whether the
        // host's proposal notification rides a toast (item 7) — never whether the batch applies.
        out.proposed = candidates;
        Ok(out)
    }

    /// Applies a vetted candidate batch — the host's entry point when a PROPOSAL is confirmed (or
    /// its 2h timeout fires). Uses each candidate's SNAPSHOT fingerprint as the driver's CAS anchor
    /// (codex m7a-🔴1), so a hand-edit between propose and here fails CAS and is skipped, never
    /// overwritten. Re-checks BOTH the §14 privileged scope (codex m7a-🔴2 — a scope that changed
    /// while the proposal waited routes to the queue, never to a write) AND the activity monitor
    /// (spec §11); a busy desktop ABORTS the WHOLE batch (codex m7a-🟠4 — no writes land while the
    /// user is active), applying nothing, so the host retries the whole batch once idle.
    pub fn apply_batch(
        &mut self,
        ports: &ReconcilerPorts<'_>,
        ctx: &ReconcileContext<'_>,
        candidates: Vec<VettedCandidate>,
        txn: &mut TxnIdAllocator,
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
    ) -> Result<ReconcileOutcome> {
        let mut out = ReconcileOutcome::default();
        out.pending_privileged = self.pending_privileged.len();
        let recipe = match ctx.saved_style {
            Some(style) => StyleRecipe::parse(style)?,
            None => return Ok(out), // ② cleared while the proposal waited → nothing to apply
        };
        // A crash could have happened between propose and confirm — reconcile it before stacking a
        // new apply; a recovery that moved/could-not-verify the desktop defers the whole batch.
        let recovery = recover_from_journal(journal, ports.reader, ports.applier, ledger, ctx.scope)?;
        if !recovery.degraded.is_empty() || recovery.moved_or_uncertain() {
            out.errors.push("recovery pending — batch deferred".into());
            out.deferred_recovery = true;
            return Ok(out);
        }
        // One vetted candidate carried from PHASE A (vet) into PHASE B (batch bake): its anchor +
        // resolved config + extracted sources, holding the owned sources/config alive for the
        // borrowing `BakeJob`s. `field_seed` stays None (the pinned-seed feed is a tracked follow-up,
        // spec 07 §5 — v1 lets the kernel derive per source).
        struct Prepared {
            candidate: VettedCandidate,
            cfg: dm_icon_core::config::Config,
            is_shortcut: bool,
            sources: Vec<dm_domain::DecodedImage>,
        }

        // ── Phase A: vet every candidate (scope / stale / config / extract) — pure reads ──────────
        // The per-candidate busy check is a FAIL-FAST: a busy desktop aborts BEFORE we bake anything.
        // The DEFINITIVE gate is still the final `is_desktop_busy` immediately before `apply` below,
        // so nothing is ever written while busy regardless of when the busy began (§11).
        let mut prepared: Vec<Prepared> = Vec::new();
        for candidate in candidates {
            let item = &candidate.item;
            // Busy → ABORT the whole batch: bake nothing, apply nothing (§11).
            if ports.activity.is_desktop_busy().unwrap_or(true) {
                out.deferred_busy = true;
                return Ok(out);
            }
            // Re-check the §14 scope: a path that became privileged while the proposal waited
            // routes to the queue, never to the write path.
            if let Some(reason) = ctx.scope.classify(&item.path) {
                self.pending_privileged.push(item.target(), reason);
                out.pending_privileged = self.pending_privileged.len();
                continue;
            }
            // Stale-proposal guard (codex r1-🟠1): the proposal was for a FRESH (un-ledgered) item.
            // If a committed row appeared while the proposal waited (something styled it since),
            // the proposal is stale — skip it as a conflict rather than re-styling with the driver
            // silently switching to the ledger's `last_applied` CAS anchor (which an old proposal
            // could pass and overwrite a newer application).
            if ledger.get(&item.id)?.is_some() {
                out.conflicts.push(item.id.clone());
                continue;
            }
            let cfg = match recipe.effective_config(item.kind, item.kind.is_shortcut()) {
                Ok(Some(c)) => c,
                Ok(None) => continue,
                Err(e) => {
                    out.errors.push(format!("resolve {}: {e}", item.id.as_str()));
                    continue;
                }
            };
            let sources = match ports.extractor.extract(item, None) {
                Ok(s) if !s.is_empty() => s,
                Ok(_) => {
                    out.errors.push(format!("extract {}: no sources", item.id.as_str()));
                    continue;
                }
                Err(e) => {
                    out.errors.push(format!("extract {}: {e}", item.id.as_str()));
                    continue;
                }
            };
            let is_shortcut = item.kind.is_shortcut();
            prepared.push(Prepared { candidate, cfg, is_shortcut, sources });
        }

        // ── Phase B: bake every (candidate, slot) in ONE parallel batch (M6 Phase 3), distribute ──
        // Byte-identical to a serial `bake_master_png` per slot (native_bake tests). The `jobs`
        // borrow of `prepared` ends with the inner scope, so `prepared` can then be consumed to move
        // each `VettedCandidate` anchor out.
        let baked = {
            let jobs: Vec<BakeJob> = prepared
                .iter()
                .flat_map(|p| {
                    p.sources
                        .iter()
                        .map(move |src| BakeJob { source_png: &src.png, config: &p.cfg, is_shortcut: p.is_shortcut })
                })
                .collect();
            // `None` cache (M6 Phase 4): reconciler candidates are NEW items — near-always misses — so
            // the content-addressed output cache adds no value here; the cache is wired to version-switch only.
            bake_masters_par(&jobs, None)
        };
        let mut masters: Vec<BufferedMaster> = Vec::new();
        let mut anchors: Vec<VettedCandidate> = Vec::new();
        let mut k = 0usize;
        for p in prepared {
            let mut item_masters = Vec::with_capacity(p.sources.len());
            let mut ok = true;
            for slot in 0..p.sources.len() {
                match &baked[k] {
                    Ok(png) => item_masters.push(BufferedMaster {
                        item_id: p.candidate.item.id.as_str().to_string(),
                        source_index: slot as u32,
                        png_base64: png.clone(),
                    }),
                    // Record only the FIRST fault per item (mirrors the old break) and skip it whole.
                    Err(e) => {
                        if ok {
                            out.errors.push(format!("bake {}: {e}", p.candidate.item.id.as_str()));
                        }
                        ok = false;
                    }
                }
                k += 1;
            }
            if ok {
                masters.extend(item_masters);
                anchors.push(p.candidate);
            }
        }
        out.pending_privileged = self.pending_privileged.len();
        if anchors.is_empty() {
            return Ok(out);
        }
        // Package + build the requests (pure computation, no desktop writes) BEFORE the final gate.
        let packaged = package_masters(&masters)?;
        let by_id: std::collections::HashMap<&str, &Fingerprint> =
            anchors.iter().map(|c| (c.item.id.as_str(), &c.fingerprint)).collect();
        let mut requests = Vec::with_capacity(packaged.len());
        for pkg in &packaged {
            let Some(candidate) = anchors.iter().find(|c| c.item.id.as_str() == pkg.item_id) else {
                continue;
            };
            requests.push(ApplyRequest {
                target: candidate.item.target(),
                // The SNAPSHOT fingerprint (codex m7a-🔴1): a change since propose fails CAS.
                expected_fingerprint: (*by_id[pkg.item_id.as_str()]).clone(),
                owned: OwnedFields::icon_only(),
                asset_hash: pkg.primary.content_hash.clone(),
                asset_bytes: pkg.primary.bytes.clone(),
                empty_asset_bytes: pkg.empty.as_ref().map(|e| e.bytes.clone()),
                pinned_seed: None,
            });
        }
        // FINAL fail-closed activity check IMMEDIATELY before the write (codex r1-🟠2 + r4-🟠): the
        // per-item check misses a busy that began during the last bake, and packaging/request
        // building above is a further window — so re-check right here, the last statement before
        // `apply`. A busy desktop aborts the whole batch, writing nothing.
        if ports.activity.is_desktop_busy().unwrap_or(true) {
            out.deferred_busy = true;
            return Ok(out);
        }
        let outcome = TxnDriver::new(ports.reader, ports.applier, ports.assets)
            .apply(txn.next_id(), requests, journal, ledger)?;
        out.applied = outcome.committed;
        // Snapshot each committed row's just-written fingerprint — the undo CAS anchor. Read
        // straight off the ledger the driver just upserted, so the anchor is EXACTLY what this
        // batch left behind (a later foreground re-style bumps the row and undo then skips).
        for id in &out.applied {
            if let Some(entry) = ledger.get(id)? {
                out.applied_snapshot.push(UndoTarget {
                    id: id.clone(),
                    applied_fingerprint: entry.last_applied_fingerprint.clone(),
                });
            }
        }
        out.conflicts.extend(outcome.conflicts);
        if let Some(e) = outcome.error {
            out.errors.push(e);
        }
        Ok(out)
    }
}

/// What [`Reconciler::restore_batch`] did — the tray 「撤销最近一次整理」 outcome (spec 07 §13
/// level 2).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RestoreBatchOutcome {
    /// Items whose bytes were restored to the true original.
    pub restored: Vec<ItemId>,
    /// Items skipped because there was nothing safe to undo: no ledger row, already at the
    /// original, vanished, or an in-flight row recovery owns.
    pub skipped: Vec<ItemId>,
    /// Items whose live state matched NEITHER our last-applied nor the original — a user hand-edit
    /// since the batch. Never clobbered (recovery:265 never-clobber law).
    pub conflicts: Vec<ItemId>,
    /// The whole undo deferred because the desktop was busy (spec 07 §11) — nothing restored;
    /// the host keeps the batch and the user can retry.
    pub deferred_busy: bool,
    /// A prior crash was recovered first — the undo stands down this pass (retry after re-sync).
    pub deferred_recovery: bool,
    /// Per-item read/restore faults — the rest of the batch proceeds.
    pub errors: Vec<String>,
}

impl Reconciler {
    /// Undoes the last auto-format batch (spec 07 §13 level 2 — the tray 「撤销最近一次整理」):
    /// restores each item's bytes from its ledger anchor, CAS-gated on THE BATCH's snapshot
    /// fingerprint ([`UndoTarget`]) so neither a user hand-edit NOR a newer foreground re-style
    /// since the batch is ever clobbered (both read as superseded → conflict, untouched).
    ///
    /// The ledger row is deliberately KEPT (state unchanged): after the restore the live bytes
    /// equal the original, which reads as the poison/manual-restore tuple `reconcile` already
    /// silently skips — so the resident never re-proposes the undone item (dropping the row would
    /// make it a fresh newcomer next cycle: restyle-after-undo, the exact ABA `reconcile`'s
    /// comment forbids). The foreground heal+fence owns resolving the stale row, same as a manual
    /// restore. Restores are idempotent byte-writes off a durable anchor, so this path takes no
    /// journal transaction — a crash mid-batch leaves each item individually consistent (either
    /// still styled-ours or exactly original), mirroring `reset_to_original`'s documented model.
    /// The read→restore pair is check-then-act like every platform write path (the known F1
    /// platform-CAS limitation, recovery:265) — the §11 idle gate bounds, not eliminates, it.
    pub fn restore_batch(
        &mut self,
        ports: &ReconcilerPorts<'_>,
        scope: &ScopeRoots,
        targets: &[UndoTarget],
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
    ) -> Result<RestoreBatchOutcome> {
        let mut out = RestoreBatchOutcome::default();
        // A crash between the batch and this undo must reconcile first (same law as apply_batch).
        let recovery = recover_from_journal(journal, ports.reader, ports.applier, ledger, scope)?;
        if !recovery.degraded.is_empty() || recovery.moved_or_uncertain() {
            out.errors.push("recovery pending — undo deferred".into());
            out.deferred_recovery = true;
            return Ok(out);
        }
        // No writes while the user is active (spec 07 §11) — an undo is still a desktop write wave.
        if ports.activity.is_desktop_busy().unwrap_or(true) {
            out.deferred_busy = true;
            return Ok(out);
        }
        for target in targets {
            let id = &target.id;
            let Some(entry) = ledger.get(id)? else {
                out.skipped.push(id.clone()); // row already healed/removed — nothing to undo
                continue;
            };
            if !entry.state.is_committed() {
                out.skipped.push(id.clone()); // in-flight/recovering row — recovery owns it
                continue;
            }
            // A ledger row newer than the batch (a foreground Apply / version switch re-styled
            // this item since) SUPERSEDES the undo — restoring to original here would wipe the
            // user's newer look (codex 2026-07-16 P1).
            if entry.last_applied_fingerprint != target.applied_fingerprint {
                out.conflicts.push(id.clone());
                continue;
            }
            let current = match ports.reader.read_fingerprint(&entry.target) {
                Ok(f) => f,
                Err(PortError::NotFound(_)) => {
                    out.skipped.push(id.clone()); // vanished since the batch — nothing to restore
                    continue;
                }
                Err(e) => {
                    out.errors.push(format!("read {}: {e}", id.as_str()));
                    continue;
                }
            };
            if current == entry.original_fingerprint {
                out.skipped.push(id.clone()); // already at the true original — idempotent no-op
                continue;
            }
            if current != target.applied_fingerprint {
                out.conflicts.push(id.clone()); // hand-edited since the batch — never clobber
                continue;
            }
            // A4 (owner 2026-07-16): re-read immediately before the write to shrink the check-then-act
            // window as far as a non-atomic shell API allows. A true CAS across the icon file + the
            // registry is infeasible, so an external write in the microseconds between this re-read and
            // the restore below is a documented, owner-accepted residual — but this closes the wider
            // window between the decision reads above and the mutation.
            match ports.reader.read_fingerprint(&entry.target) {
                Ok(f) if f != target.applied_fingerprint => {
                    out.conflicts.push(id.clone()); // changed since the first read — never clobber
                    continue;
                }
                Ok(_) => {}
                // Vanished/unreadable inside the window — do not restore over an unknown state.
                Err(_) => {
                    out.skipped.push(id.clone());
                    continue;
                }
            }
            match ports.applier.restore(&entry.target, &entry.original_anchor) {
                Ok(()) => out.restored.push(id.clone()),
                Err(e) => out.errors.push(format!("restore {}: {e}", id.as_str())),
            }
        }
        Ok(out)
    }
}

fn op_err(e: PortError) -> dm_operations::OperationError {
    dm_operations::OperationError::InvalidPayload(e.to_string())
}
