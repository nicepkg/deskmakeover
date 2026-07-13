//! The trust/consent ladder (spec 07 §2): batched proposals by default, an earned silent tier,
//! and the intent-freshness health check that downgrades a would-be-silent batch back to a
//! proposal — never suppresses it.

use serde::{Deserialize, Serialize};

/// Proposal auto-apply timeout (spec 07 §2 item 4): the batch applies if the user takes no
/// action for this long. Default 2h.
pub const PROPOSAL_TIMEOUT_SECS: i64 = 2 * 60 * 60;

/// Consecutive un-undone automatic batches required before the toast tier drops and a batch may
/// apply silently (spec 07 §2 item 7).
const SILENT_TIER_BATCHES: u32 = 3;

/// Staleness horizon for the saved style (spec 07 §2 item 8): older than this → downgrade.
const STALE_INTENT_SECS: i64 = 60 * 24 * 60 * 60;

/// The per-user trust counter (spec 07 §2 item 7). Persisted by the host as JSON; an undo of any
/// automatic batch resets it, so the toast tier returns.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustState {
    /// Consecutive automatic batches the user did NOT undo.
    pub batches_without_undo: u32,
}

impl TrustState {
    /// Whether the silent tier is earned (toast tier dropped, silent batches allowed subject to
    /// the freshness check).
    pub fn silent_earned(&self) -> bool {
        self.batches_without_undo >= SILENT_TIER_BATCHES
    }

    /// Whether the trust-building toast (with inline undo) still accompanies a batch.
    pub fn toast_tier_active(&self) -> bool {
        !self.silent_earned()
    }

    /// Whether the host MUST show the toast for a batch/proposal (codex m7b-🟠1): an ANOMALY (a
    /// conflict, a partial failure, an item that needed elevation) ALWAYS surfaces the toast tier
    /// regardless of the trust counter (spec §2 item 7 "anomalies never silently degrade");
    /// otherwise the toast rides only while the trust-building tier is active. This is a separate
    /// axis from silent-apply (which v1 never does) — it governs the NOTIFICATION, not the write.
    pub fn toast_required(&self, anomaly: bool) -> bool {
        anomaly || self.toast_tier_active()
    }

    /// Records a batch outcome: an undo resets the ladder (the toast tier returns); a kept batch
    /// climbs it. Saturating — the counter never wraps.
    pub fn record_batch(&mut self, undone: bool) {
        if undone {
            self.batches_without_undo = 0;
        } else {
            self.batches_without_undo = self.batches_without_undo.saturating_add(1);
        }
    }
}

/// The two staleness signals of the intent-freshness health check (spec 07 §2 item 8), supplied
/// by the host. Either one downgrades a would-be-silent batch to a proposal FOR THAT RUN ONLY.
/// **v1 status:** dormant — v1 ALWAYS proposes (silent mode is a v1.1 opt-in), so nothing consumes
/// this yet. **v1.1 wiring note (codex m7b-🟡6):** `last_apply_at` MUST come from ②'s OWN write
/// timestamp (spec §2 item 8 = "saved-style's write timestamp"), NOT from ③'s look-history head —
/// the two lifecycles diverge (switching an old look, or a ③ write fault). ② currently has no
/// timestamp column; add an `updated_at` written atomically with `set_saved_style` when silent
/// mode ships, and feed it here.
#[derive(Debug, Clone, Copy)]
pub struct FreshnessInputs {
    /// When ② saved-style was last written (its own timestamp, per the v1.1 note above); `None`
    /// = never / unknown.
    pub last_apply_at: Option<i64>,
    /// Whether the user individually reverted previously-styled icons since the last global
    /// Apply — read as stepping away from the style, not as noise.
    pub partial_reversion: bool,
    /// The evaluation clock (unix seconds).
    pub now: i64,
}

impl FreshnessInputs {
    /// Whether a would-be-silent batch must downgrade to a proposal this run.
    pub fn downgrades(&self) -> bool {
        let stale = match self.last_apply_at {
            // No recorded Apply time while a style exists = provenance unknown → conservative.
            None => true,
            Some(t) => self.now.saturating_sub(t) > STALE_INTENT_SECS,
        };
        stale || self.partial_reversion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_silent_tier_is_earned_after_three_kept_batches_and_an_undo_resets_it() {
        let mut t = TrustState::default();
        assert!(!t.silent_earned() && t.toast_tier_active());
        t.record_batch(false);
        t.record_batch(false);
        assert!(!t.silent_earned(), "two batches are not enough");
        t.record_batch(false);
        assert!(t.silent_earned() && !t.toast_tier_active());
        // An undo anywhere resets the ladder — the toast tier returns.
        t.record_batch(true);
        assert!(!t.silent_earned());
        assert_eq!(t.batches_without_undo, 0);
    }

    #[test]
    fn an_anomaly_always_forces_the_toast_even_past_the_earned_tier() {
        let earned = TrustState { batches_without_undo: 5 };
        assert!(!earned.toast_tier_active(), "the toast tier is dropped once earned");
        assert!(earned.toast_required(true), "but an anomaly always toasts");
        assert!(!earned.toast_required(false), "a clean batch past the tier is silent-notification");
        let building = TrustState::default();
        assert!(building.toast_required(false), "the trust-building tier toasts every batch");
    }

    #[test]
    fn freshness_downgrades_on_stale_intent_or_partial_reversion() {
        let now = 1_800_000_000;
        let fresh = FreshnessInputs { last_apply_at: Some(now - 86_400), partial_reversion: false, now };
        assert!(!fresh.downgrades());
        let stale = FreshnessInputs { last_apply_at: Some(now - 61 * 86_400), partial_reversion: false, now };
        assert!(stale.downgrades(), "older than 60 days downgrades");
        let reverted = FreshnessInputs { last_apply_at: Some(now - 86_400), partial_reversion: true, now };
        assert!(reverted.downgrades(), "a partial reversion downgrades");
        let unknown = FreshnessInputs { last_apply_at: None, partial_reversion: false, now };
        assert!(unknown.downgrades(), "unknown provenance is conservative");
    }
}
