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
    ActivityMonitor, AssetStore, DesktopItem, DesktopScanner, IconApplier, IconSourceExtractor,
    ItemId, ItemStateReader, OwnedFields, PortError,
};
use dm_icon_core::render_session::RenderSession;
use dm_operations::icons::native_bake::bake_master_png;
use dm_operations::icons::package_masters;
use dm_operations::icons::style_resolve::StyleRecipe;
use dm_operations::{
    recover_from_journal, ApplyRequest, BufferedMaster, JournalSink, LedgerStore, Result,
    TxnDriver, TxnIdAllocator,
};

use crate::consent::{FreshnessInputs, TrustState};
use crate::pending_privileged::{privileged_scope, PendingPrivilegedQueue};
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
    /// The earned trust tier (spec 07 §2 item 7).
    pub trust: &'a TrustState,
    /// The intent-freshness signals (spec 07 §2 item 8).
    pub freshness: FreshnessInputs,
    /// The privileged roots (`Public Desktop`), resolved by the host via known folders.
    pub public_roots: &'a [String],
}

/// What one reconcile cycle did / decided.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconcileOutcome {
    /// Candidates surfaced as a batched PROPOSAL (not applied) — the host owns the confirm /
    /// 2h-timeout surface and calls [`Reconciler::apply_batch`] with them on confirm/timeout.
    pub proposed: Vec<ItemId>,
    /// Items committed by a silent-tier apply THIS cycle.
    pub applied: Vec<ItemId>,
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
    /// Whether the silent tier was downgraded to a proposal by the freshness check this run.
    pub freshness_downgraded: bool,
    /// Per-item degradations (extract/bake/read faults) — the rest of the cycle proceeds.
    pub errors: Vec<String>,
}

/// The long-lived reconciler: settle-probe memory, the pending-privileged queue, and the warm
/// render session (profile analysis persists across cycles, spec 07 §5).
pub struct Reconciler {
    settle: SettleProbe,
    pub pending_privileged: PendingPrivilegedQueue,
    session: RenderSession,
}

impl Default for Reconciler {
    fn default() -> Self {
        Self::new()
    }
}

impl Reconciler {
    pub fn new() -> Self {
        Self {
            settle: SettleProbe::new(),
            pending_privileged: PendingPrivilegedQueue::new(),
            session: RenderSession::new(),
        }
    }

    /// One reconcile cycle: classify the live desktop, queue privileged scopes, gate unstable
    /// newcomers, flag conflicts, and either silently apply the candidate batch (earned tier +
    /// fresh intent) or surface it as a proposal.
    pub fn reconcile(
        &mut self,
        ports: &ReconcilerPorts<'_>,
        ctx: &ReconcileContext<'_>,
        txn: &mut TxnIdAllocator,
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
    ) -> Result<ReconcileOutcome> {
        let mut out = ReconcileOutcome::default();
        // ② empty → dormant: nothing to project (spec 07 §8.3 — no special-case styling paths).
        let Some(style) = ctx.saved_style else {
            out.pending_privileged = self.pending_privileged.len();
            return Ok(out);
        };
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

        let mut candidates: Vec<DesktopItem> = Vec::new();
        for item in items {
            // §14 red line FIRST: privileged scope routes to the queue before ANY other path.
            if let Some(reason) = privileged_scope(&item.path, ctx.public_roots) {
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
                            // Ambiguous poison/manual-restore tuple: the background never
                            // resolves ambiguity (the foreground heal + fence own it) — flag.
                            out.conflicts.push(item.id.clone());
                        }
                        Ok(_) => out.conflicts.push(item.id.clone()),
                        Err(PortError::NotFound(_)) => {} // vanished mid-cycle; next scan settles it
                        Err(e) => out.errors.push(format!("read {}: {e}", item.id.as_str())),
                    }
                }
                // A non-committed row is an in-flight/recovering txn — recovery owns it; the
                // background stays out.
                Some(_) => {}
                None => {
                    // A NEW item: it formats only once its bytes have settled (spec 07 §3).
                    let snap = ports.stability.snapshot(&item.path);
                    if !self.settle.observe(&item.path, snap) {
                        out.deferred_unstable.push(item.id.clone());
                        continue;
                    }
                    candidates.push(item);
                }
            }
        }

        out.pending_privileged = self.pending_privileged.len();
        if candidates.is_empty() {
            return Ok(out);
        }
        // Batch decision (spec 07 §2): silent only under an earned tier AND fresh intent; the
        // freshness check downgrades to a proposal for THIS run, never suppresses.
        if ctx.trust.silent_earned() {
            if ctx.freshness.downgrades() {
                out.freshness_downgraded = true;
                out.proposed = candidates.into_iter().map(|i| i.id).collect();
                return Ok(out);
            }
            let applied = self.apply_batch(ports, &recipe, candidates, txn, journal, ledger)?;
            merge(&mut out, applied);
        } else {
            out.proposed = candidates.into_iter().map(|i| i.id).collect();
        }
        Ok(out)
    }

    /// Applies one candidate batch through the shared driver — also the host's entry point when
    /// a PROPOSAL is confirmed (or its timeout auto-applies). Re-checks the activity monitor
    /// between every icon's bake (spec 07 §11); items past the busy point return to the pending
    /// pool simply by not being applied (the next cycle re-derives them).
    pub fn apply_batch(
        &mut self,
        ports: &ReconcilerPorts<'_>,
        recipe: &StyleRecipe,
        items: Vec<DesktopItem>,
        txn: &mut TxnIdAllocator,
        journal: &mut dyn JournalSink,
        ledger: &mut dyn LedgerStore,
    ) -> Result<ReconcileOutcome> {
        let mut out = ReconcileOutcome::default();
        let mut masters: Vec<BufferedMaster> = Vec::new();
        let mut anchors: Vec<(DesktopItem, dm_domain::Fingerprint)> = Vec::new();
        for item in items {
            // The per-icon activity re-check: a user who starts interacting mid-batch stops the
            // batch mid-batch; the un-baked remainder defers to the next cycle.
            if ports.activity.is_desktop_busy().unwrap_or(true) {
                out.deferred_busy = true;
                break;
            }
            let cfg = match recipe.effective_config(item.kind, item.kind.is_shortcut()) {
                Ok(Some(c)) => c,
                Ok(None) => continue,
                Err(e) => {
                    out.errors.push(format!("resolve {}: {e}", item.id.as_str()));
                    continue;
                }
            };
            // The CAS anchor is the fingerprint read NOW, before the bake — a hand-edit between
            // this read and the driver's prepare fails CAS and is skipped, never overwritten.
            let fingerprint = match ports.reader.read_fingerprint(&item.target()) {
                Ok(f) => f,
                Err(e) => {
                    out.errors.push(format!("anchor {}: {e}", item.id.as_str()));
                    continue;
                }
            };
            let sources = match ports.extractor.extract(&item, None) {
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
            let mut ok = true;
            let mut item_masters = Vec::with_capacity(sources.len());
            for (slot, src) in sources.iter().enumerate() {
                let bake_id = if slot == 0 {
                    item.id.as_str().to_string()
                } else {
                    format!("{}#{slot}", item.id.as_str())
                };
                match bake_master_png(
                    &mut self.session,
                    &bake_id,
                    &src.png,
                    &cfg,
                    item.kind.is_shortcut(),
                    // Hue continuity: new icons should allocate against pinned existing seeds
                    // (spec 07 §5). v1 lets the kernel derive per source; the pinned-seed feed
                    // is a tracked follow-up, not silently dropped — see the plan's residuals.
                    None,
                ) {
                    Ok(png) => item_masters.push(BufferedMaster {
                        item_id: item.id.as_str().to_string(),
                        source_index: slot as u32,
                        png_base64: png,
                    }),
                    Err(e) => {
                        out.errors.push(format!("bake {}: {e}", item.id.as_str()));
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                masters.extend(item_masters);
                anchors.push((item, fingerprint));
            }
        }
        if anchors.is_empty() {
            return Ok(out);
        }
        let packaged = package_masters(&masters)?;
        // Same discipline as the foreground: reconcile any prior crash BEFORE stacking a new
        // apply; a recovery that moved or could not verify the desktop defers this batch.
        let recovery = recover_from_journal(journal, ports.reader, ports.applier, ledger)?;
        if !recovery.degraded.is_empty() || !recovery.aborted.is_empty() {
            out.errors.push("recovery pending — batch deferred to the next cycle".into());
            return Ok(out);
        }
        let by_id: std::collections::HashMap<&str, &dm_domain::Fingerprint> =
            anchors.iter().map(|(i, f)| (i.id.as_str(), f)).collect();
        let mut requests = Vec::with_capacity(packaged.len());
        for pkg in &packaged {
            let Some((item, _)) = anchors.iter().find(|(i, _)| i.id.as_str() == pkg.item_id) else {
                continue;
            };
            requests.push(ApplyRequest {
                target: item.target(),
                expected_fingerprint: (*by_id[pkg.item_id.as_str()]).clone(),
                owned: OwnedFields::icon_only(),
                asset_hash: pkg.primary.content_hash.clone(),
                asset_bytes: pkg.primary.bytes.clone(),
                empty_asset_bytes: pkg.empty.as_ref().map(|e| e.bytes.clone()),
                pinned_seed: None,
            });
        }
        let outcome = TxnDriver::new(ports.reader, ports.applier, ports.assets)
            .apply(txn.next_id(), requests, journal, ledger)?;
        out.applied = outcome.committed;
        out.conflicts.extend(outcome.conflicts);
        if let Some(e) = outcome.error {
            out.errors.push(e);
        }
        Ok(out)
    }
}

fn merge(into: &mut ReconcileOutcome, from: ReconcileOutcome) {
    into.applied.extend(from.applied);
    into.conflicts.extend(from.conflicts);
    into.errors.extend(from.errors);
    into.deferred_busy |= from.deferred_busy;
}

fn op_err(e: PortError) -> dm_operations::OperationError {
    dm_operations::OperationError::InvalidPayload(e.to_string())
}
