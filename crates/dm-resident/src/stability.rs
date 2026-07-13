//! The retryable file-stability gate (spec 07 §3): a new item is processed only once its
//! observable state has SETTLED across two reconcile cycles. The watcher owns debounce only —
//! this gate lives in the reconciler (codex B10-review: a blocking probe in the debouncer worker
//! would stall the event stream), as a pure non-blocking state machine: observe now, compare to
//! the last cycle's observation, settle when nothing moved.
//!
//! The snapshot is deliberately abstract (`u64` size + `i64` mtime + readable flag) so the Mac
//! tests drive it directly; the host feeds real `std::fs::metadata` values. The `.lnk`-specific
//! readiness half (IShellLink parses, target + IconLocation populated) is a `[WINDOWS-VERIFY]`
//! extra the Windows host layers on top — a `.lnk` that fails it simply reads `readable: false`
//! here and retries next cycle.

use std::collections::HashMap;

/// One cycle's observation of a path's externally-visible write state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StabilitySnapshot {
    pub size: u64,
    /// mtime in NANOSECONDS (codex m7b-🟠4): NTFS mtime resolution is sub-second, so a
    /// whole-second stamp would let a same-size rewrite within one second read as "settled" when
    /// it is still being written. Nanosecond precision catches it.
    pub mtime_nanos: u128,
    /// Whether the file opened readably without an exclusive-lock conflict this cycle.
    pub readable: bool,
}

/// Tracks per-path snapshots across cycles. `settled` = the current observation is readable AND
/// byte-identical to the previous cycle's. First sight is never settled (one full cycle of quiet
/// is the minimum bar), and paths that vanish are forgotten.
#[derive(Debug, Default)]
pub struct SettleProbe {
    seen: HashMap<String, StabilitySnapshot>,
}

impl SettleProbe {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `now` for `path` and reports whether the path has settled. An unreadable
    /// observation NEVER settles (and still replaces the record, so the quiet clock restarts).
    pub fn observe(&mut self, path: &str, now: StabilitySnapshot) -> bool {
        let settled = now.readable && self.seen.get(path) == Some(&now);
        self.seen.insert(path.to_string(), now);
        settled
    }

    /// Drops paths no longer on the desktop so the map cannot grow unboundedly.
    pub fn retain_paths<'a>(&mut self, live: impl Iterator<Item = &'a str>) {
        let keep: std::collections::HashSet<&str> = live.collect();
        self.seen.retain(|k, _| keep.contains(k.as_str()));
    }
}

/// Where a cycle's snapshots come from — the port that keeps the probe Mac-testable. The real
/// reader hits the filesystem; tests inject scripted observations.
pub trait StabilityReader {
    fn snapshot(&self, path: &str) -> StabilitySnapshot;
}

/// The filesystem reader: size + whole-second mtime from metadata; `readable` = a plain open
/// succeeds (a writer holding an exclusive share on Windows fails this — `[WINDOWS-VERIFY]`
/// share-mode semantics; on the Mac host it still catches vanished/permission-blocked files).
pub struct FsStabilityReader;

impl StabilityReader for FsStabilityReader {
    fn snapshot(&self, path: &str) -> StabilitySnapshot {
        let meta = std::fs::metadata(path).ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let mtime_nanos = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let readable = meta.map(|m| m.is_dir()).unwrap_or(false)
            || std::fs::File::open(path).is_ok();
        StabilitySnapshot { size, mtime_nanos, readable }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(size: u64, mtime_nanos: u128, readable: bool) -> StabilitySnapshot {
        StabilitySnapshot { size, mtime_nanos, readable }
    }

    #[test]
    fn first_sight_never_settles_and_a_quiet_second_cycle_does() {
        let mut p = SettleProbe::new();
        assert!(!p.observe("a.lnk", snap(100, 5, true)), "first sight is not settled");
        assert!(p.observe("a.lnk", snap(100, 5, true)), "unchanged second cycle settles");
    }

    #[test]
    fn a_still_moving_or_locked_file_keeps_retrying() {
        let mut p = SettleProbe::new();
        p.observe("b.tmp", snap(100, 5, true));
        assert!(!p.observe("b.tmp", snap(220, 6, true)), "size/mtime moved → not settled");
        assert!(!p.observe("b.tmp", snap(220, 6, false)), "locked → not settled");
        // The unreadable observation replaced the record: the next readable identical one is the
        // FIRST quiet cycle (readable flag differs from the locked record), so it settles only
        // after one more quiet cycle.
        assert!(!p.observe("b.tmp", snap(220, 6, true)));
        assert!(p.observe("b.tmp", snap(220, 6, true)));
    }

    #[test]
    fn a_sub_second_rewrite_at_the_same_size_does_not_settle() {
        // codex m7b-🟠4: same size, different sub-second mtime → still being written, NOT settled.
        let mut p = SettleProbe::new();
        p.observe("x.tmp", snap(100, 1_000_000_000, true));
        assert!(
            !p.observe("x.tmp", snap(100, 1_000_000_500, true)),
            "a same-size sub-second rewrite is caught by nanosecond mtime"
        );
        // Truly quiet (identical nanos) → settles the next cycle.
        assert!(p.observe("x.tmp", snap(100, 1_000_000_500, true)));
    }

    #[test]
    fn vanished_paths_are_forgotten() {
        let mut p = SettleProbe::new();
        p.observe("gone.lnk", snap(1, 1, true));
        p.retain_paths(["stay.lnk"].into_iter());
        // Re-observed after forgetting → first sight again.
        assert!(!p.observe("gone.lnk", snap(1, 1, true)));
    }
}
