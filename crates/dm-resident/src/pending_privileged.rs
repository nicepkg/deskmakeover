//! The pending-privileged queue (spec 07 §14): items the reconciler routed AWAY from every write
//! path because they would need elevation (Public Desktop / ProgramData). The background never
//! elevates — this queue is drained by ONE batched UAC when the user opens the window; its length
//! backs the tray's "待处理特权项 (N)" line.
//!
//! Scope classification is the SHARED `dm_operations::icons::scope::privileged_scope` (a proper
//! path-ancestry test), so the reconciler and the operations-layer version switch classify
//! identically.

use dm_domain::{ItemId, ItemTarget};
pub use dm_operations::icons::scope::{privileged_scope, PrivilegedScope, ScopeRoots};

/// Back-compat alias: the queue's reason is the shared scope classification.
pub type PendingReason = PrivilegedScope;

#[derive(Debug, Clone)]
struct Pending {
    target: ItemTarget,
    reason: PrivilegedScope,
}

/// FIFO, deduplicated by item id (an item observed across many reconcile cycles queues once).
#[derive(Debug, Default)]
pub struct PendingPrivilegedQueue {
    items: Vec<Pending>,
}

impl PendingPrivilegedQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueues, deduplicated by item id. A REPEAT observation UPDATES the target + reason in
    /// place while keeping its FIFO position (codex m7b-🟠8: an item renamed while queued must
    /// hand the batched-UAC drain its CURRENT path, not the first-seen one — file identity, not
    /// the stale locator).
    pub fn push(&mut self, target: ItemTarget, reason: PrivilegedScope) {
        if let Some(existing) = self.items.iter_mut().find(|p| p.target.id == target.id) {
            existing.target = target;
            existing.reason = reason;
            return;
        }
        self.items.push(Pending { target, reason });
    }

    /// Hands every queued target to the one batched-UAC drain (window-open path, spec 07 §14),
    /// emptying the queue.
    pub fn drain_for_elevation(&mut self) -> Vec<ItemTarget> {
        std::mem::take(&mut self.items).into_iter().map(|p| p.target).collect()
    }

    /// Drops one queued item (it vanished from the desktop, or moved out of privileged scope).
    pub fn remove(&mut self, id: &ItemId) {
        self.items.retain(|p| &p.target.id != id);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The queued reasons, for the review UI (id → reason).
    pub fn reasons(&self) -> impl Iterator<Item = (&ItemId, PrivilegedScope)> {
        self.items.iter().map(|p| (&p.target.id, p.reason))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_domain::ItemKind;

    fn target(id: &str, path: &str) -> ItemTarget {
        ItemTarget::new(ItemId::from_raw(id), ItemKind::Shortcut, path)
    }

    #[test]
    fn push_dedups_by_id_but_updates_the_target_in_place() {
        let mut q = PendingPrivilegedQueue::new();
        q.push(target("a", "C:/Users/Public/Desktop/a.lnk"), PrivilegedScope::PublicDesktop);
        q.push(target("b", "C:/ProgramData/x/b.lnk"), PrivilegedScope::ProgramData);
        // A rename of `a` while queued updates its path, does NOT add a second entry.
        q.push(target("a", "C:/Users/Public/Desktop/a-renamed.lnk"), PrivilegedScope::PublicDesktop);
        assert_eq!(q.len(), 2, "repeat observations queue once");
        let drained = q.drain_for_elevation();
        assert_eq!(drained.len(), 2);
        let a = drained.iter().find(|t| t.id == ItemId::from_raw("a")).unwrap();
        assert_eq!(a.path, "C:/Users/Public/Desktop/a-renamed.lnk", "the drain sees the current path");
        assert!(q.is_empty());
    }

    #[test]
    fn remove_drops_a_vanished_item() {
        let mut q = PendingPrivilegedQueue::new();
        q.push(target("a", "C:/Users/Public/Desktop/a.lnk"), PrivilegedScope::PublicDesktop);
        q.remove(&ItemId::from_raw("a"));
        assert!(q.is_empty());
    }
}
