//! The per-item ledger entry and its transaction state machine.
//!
//! Formalizes ADR-0020 §2 (incremental ledger) and spec 07 §5: every applied item is one
//! entry carrying its original fingerprint + restore anchor, its last-applied fingerprint,
//! the fields DeskMakeover owns, the content-addressed generated asset, and the transaction
//! state. Each background apply appends to the SAME history the manual flow uses (one undo
//! surface); the state machine is what recovery uses to decide roll-forward vs roll-back.

use dm_domain::{AssetRef, Fingerprint, ItemId, ItemTarget, OwnedFields, RestoreAnchor};
use serde::{Deserialize, Serialize};

/// The durable state of one item's transaction (spec 07 §5:
/// prepared → asset-written → applied → verified → committed). `RolledBack` is the terminal
/// undo state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TxnState {
    /// Restore anchor captured and journaled; nothing mutated yet.
    Prepared,
    /// The generated `.ico` has been written to the content-addressed store.
    AssetWritten,
    /// The external mutation (icon location swap) has been performed.
    Applied,
    /// The applied state has been read back and confirmed.
    Verified,
    /// The transaction committed; the entry is the live styled state.
    Committed,
    /// The item was walked back to its captured original; no residue.
    RolledBack,
}

impl TxnState {
    /// Whether the item is in the live styled state.
    pub fn is_committed(self) -> bool {
        self == TxnState::Committed
    }

    /// Whether this state is terminal (no further transitions).
    pub fn is_terminal(self) -> bool {
        matches!(self, TxnState::Committed | TxnState::RolledBack)
    }
}

/// One item's ledger entry. Ordered in history by [`version`](LedgerEntry::version) — a
/// monotonic counter rather than wall-clock so the pure core stays deterministic in tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub item: ItemId,
    pub target: ItemTarget,
    /// The fingerprint of the user's true original (the restore anchor's fingerprint).
    pub original_fingerprint: Fingerprint,
    /// The exact-restore material for the true original.
    pub original_anchor: RestoreAnchor,
    /// The fingerprint of the state DeskMakeover last applied (spec 07 §5 CAS anchor).
    pub last_applied_fingerprint: Fingerprint,
    /// Which fields DeskMakeover owns on this item.
    pub owned: OwnedFields,
    /// The content-addressed generated asset currently applied.
    pub asset: AssetRef,
    /// The transaction state.
    pub state: TxnState,
    /// The pinned hue seed this item was allocated (ADR-0020 §2: background additions allocate
    /// against pinned existing seeds; existing icons never reflow). `None` for the foreground
    /// flow, which owns the global rebalance.
    pub pinned_seed: Option<u32>,
    /// Monotonic version for newest-first history ordering.
    pub version: u64,
}

impl LedgerEntry {
    /// Whether the current live state matches our last-applied fingerprint — i.e. the item is
    /// still ours and unmodified. A mismatch is an external modification (conflict) per
    /// ADR-0020 §2 / spec 07 §5.
    pub fn is_unmodified(&self, current: &Fingerprint) -> bool {
        self.state.is_committed() && &self.last_applied_fingerprint == current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_and_committed_predicates() {
        use TxnState::*;
        assert!(Committed.is_committed());
        assert!(!Applied.is_committed());
        // Only Committed and RolledBack are terminal.
        assert!(Committed.is_terminal());
        assert!(RolledBack.is_terminal());
        for state in [Prepared, AssetWritten, Applied, Verified] {
            assert!(!state.is_terminal(), "{state:?} is not terminal");
        }
    }
}
