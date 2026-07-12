//! The pending-privileged queue (spec 07 §14): items the reconciler routed AWAY from every write
//! path because they would need elevation (Public Desktop / ProgramData). The background never
//! elevates — this queue is drained by ONE batched UAC when the user opens the window; its length
//! backs the tray's "待处理特权项 (N)" line.

use dm_domain::{ItemId, ItemTarget};

/// Why an item is queued instead of formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingReason {
    /// Lives under the shared `Public Desktop` root.
    PublicDesktop,
    /// Lives under `ProgramData` (installer-deployed).
    ProgramData,
}

#[derive(Debug, Clone)]
struct Pending {
    target: ItemTarget,
    reason: PendingReason,
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

    /// Enqueues once per item id; a repeat observation refreshes nothing (first reason wins —
    /// the scope of a path does not change while it sits queued).
    pub fn push(&mut self, target: ItemTarget, reason: PendingReason) {
        if self.items.iter().any(|p| p.target.id == target.id) {
            return;
        }
        self.items.push(Pending { target, reason });
    }

    /// Hands every queued target to the one batched-UAC drain (window-open path, spec 07 §14),
    /// emptying the queue.
    pub fn drain_for_elevation(&mut self) -> Vec<ItemTarget> {
        std::mem::take(&mut self.items).into_iter().map(|p| p.target).collect()
    }

    /// Drops one queued item (it vanished from the desktop before the drain).
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
    pub fn reasons(&self) -> impl Iterator<Item = (&ItemId, PendingReason)> {
        self.items.iter().map(|p| (&p.target.id, p.reason))
    }
}

/// Classifies a path's write scope against the privileged roots (the host resolves the real
/// known-folder paths; tests inject). Comparison is case-insensitive on the ASCII range — NTFS
/// path semantics; the roots come from `SHGetKnownFolderPath`, so casing is already canonical in
/// practice and this guard only covers hand-typed/registry-echoed variants.
pub fn privileged_scope(path: &str, public_roots: &[String]) -> Option<PendingReason> {
    let lower = path.to_ascii_lowercase().replace('\\', "/");
    for root in public_roots {
        let root_l = root.to_ascii_lowercase().replace('\\', "/");
        if !root_l.is_empty() && lower.starts_with(&root_l) {
            return Some(PendingReason::PublicDesktop);
        }
    }
    if lower.contains("/programdata/") {
        return Some(PendingReason::ProgramData);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_domain::ItemKind;

    fn target(id: &str, path: &str) -> ItemTarget {
        ItemTarget::new(ItemId::from_raw(id), ItemKind::Shortcut, path)
    }

    #[test]
    fn push_dedups_by_id_and_drain_empties() {
        let mut q = PendingPrivilegedQueue::new();
        q.push(target("a", "C:/Users/Public/Desktop/a.lnk"), PendingReason::PublicDesktop);
        q.push(target("a", "C:/Users/Public/Desktop/a.lnk"), PendingReason::PublicDesktop);
        q.push(target("b", "C:/ProgramData/x/b.lnk"), PendingReason::ProgramData);
        assert_eq!(q.len(), 2, "repeat observations queue once");
        let drained = q.drain_for_elevation();
        assert_eq!(drained.len(), 2);
        assert!(q.is_empty());
    }

    #[test]
    fn scope_classification_matches_public_roots_and_programdata() {
        let roots = vec![r"C:\Users\Public\Desktop".to_string()];
        assert_eq!(
            privileged_scope(r"C:\Users\Public\Desktop\Tool.lnk", &roots),
            Some(PendingReason::PublicDesktop),
        );
        assert_eq!(
            privileged_scope("c:/users/public/desktop/tool.lnk", &roots),
            Some(PendingReason::PublicDesktop),
            "case-insensitive, separator-agnostic"
        );
        assert_eq!(
            privileged_scope(r"C:\ProgramData\App\i.lnk", &roots),
            Some(PendingReason::ProgramData),
        );
        assert_eq!(privileged_scope(r"C:\Users\Dev\Desktop\mine.lnk", &roots), None);
    }

    #[test]
    fn remove_drops_a_vanished_item() {
        let mut q = PendingPrivilegedQueue::new();
        q.push(target("a", "C:/Users/Public/Desktop/a.lnk"), PendingReason::PublicDesktop);
        q.remove(&ItemId::from_raw("a"));
        assert!(q.is_empty());
    }
}
