//! Desktop change watcher — `ReadDirectoryChangesW` hints per spec 07 §3.
//!
//! [WINDOWS-VERIFY] — M7 SKELETON. The full watcher (debounce 2–10s, file-stability probe,
//! source-fingerprint-beyond-`.lnk`-bytes identity, self-write suppression, buffer-overflow →
//! full rescan, catch-up on resume/Explorer-restart) is spec-07 M7 work. This module fixes the
//! event vocabulary and the entry-point shape so the resident crate can depend on it now;
//! `events are hints, reconciliation is truth`, so the hint stream deliberately carries no
//! decisions.

use std::path::PathBuf;

/// A raw change hint from the filesystem watcher (spec 07 §3: hints, not decisions). Installers
/// commonly write-temp then rename, so `Renamed` and `Created` both occur for a new item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    Created(PathBuf),
    Changed(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
    Deleted(PathBuf),
    /// The watch buffer overflowed — the consumer MUST fall back to a full reconcile (spec 07 §3).
    Overflow,
}

/// Starts watching the user + public desktop roots, delivering [`WatchEvent`] hints to `on_event`.
///
/// [WINDOWS-VERIFY] M7 STUB: the `ReadDirectoryChangesW` overlapped-I/O loop lands with the
/// resident reconciler. The signature is fixed here so downstream code can be written against it.
pub fn watch_desktops<F>(_roots: Vec<PathBuf>, _on_event: F) -> Result<(), String>
where
    F: FnMut(WatchEvent) + Send + 'static,
{
    Err("desktop watcher lands in M7 (spec 07 §3)".to_string())
}
