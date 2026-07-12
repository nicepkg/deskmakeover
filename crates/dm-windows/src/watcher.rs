//! Desktop change watcher (Wave B B10, spec 07 §3 + §16).
//!
//! Backed by `notify` + `notify-debouncer-full` — the `-full` debouncer tracks file ids and pairs an
//! installer's temp-write→rename into a single `Rename(Both)` (spec 07 §16). It is a best-effort
//! reducer, NOT an exactly-once guarantee: heterogeneous bursts can still yield more than one hint,
//! so EXACTLY-ONCE FORMATTING is the reconciler's job (its pending-set + idempotence), not this
//! stream's. The primitive is cross-platform (ReadDirectoryChangesW on Windows, FSEvents on macOS,
//! inotify on Linux), so the debounce + event-mapping core is unit-testable on the Mac host; only the
//! Windows-runtime desktop semantics below stay `[WINDOWS-VERIFY]`.
//!
//! `events are hints, reconciliation is truth` (spec 07 §3): this stream deliberately carries no
//! decisions — the resident reconciler re-scans + re-fingerprints on every hint and decides what to
//! (re)format. So over-notifying is harmless; under-notifying (a dropped buffer) is not, which is
//! why an overflow/rescan signal is surfaced as [`WatchEvent::Overflow`] for a full reconcile.
//!
//! ⚠️ KNOWN LIMITATION — WINDOWS OVERFLOW IS SILENT WITH notify 8.2 (codex B10-review 🔴). notify's
//! Windows `ReadDirectoryChangesW` backend IGNORES the completion's `bytes_written`: a buffer
//! overflow (Windows signals it as a zero-byte completion, discarding the whole 16 KiB buffer) emits
//! NO event, NO error, and NO `Flag::Rescan`. So `to_watch_event`'s `need_rescan() → Overflow` maps
//! correctly (and FIRES on macOS/inotify), but on Windows a real overflow is dropped silently, which
//! breaks the "under-notifying → full reconcile" fail-safe. Mainline notify still ignores the
//! parameter, so there is no version bump that fixes it. The DURABLE fix (either a patched/forked
//! Windows backend that emits `Rescan` on a zero-byte completion, OR — recommended — a periodic full
//! reconcile in the M7 resident as a belt-and-suspenders backstop) is Windows-runtime work; until
//! then the M7 reconciler MUST NOT rely on `Overflow` alone for burst-loss recovery on Windows.
//!
//! [WINDOWS-VERIFY] — runtime behaviours the Mac host cannot exercise, batched to the owner's
//! Windows box (see `docs/plans/2026-07-10-m34-windows-blind.md` item 9):
//!   1. Self-write suppression: the apply path writes `desktop.ini` / swaps ICOs; the reconciler
//!      must not treat the app's OWN writes as a new-icon event. This is the RECONCILER's job (it
//!      owns the apply epoch); this watcher only supplies raw hints. Confirm on Windows that an apply
//!      does not trigger a self-format loop.
//!   2. Explorer-restart / resume catch-up: after an Explorer crash or a sleep/resume the watch
//!      handle may need re-arming; confirm the reconciler does a full rescan on (re)start so items
//!      that appeared while unwatched are still formatted (spec 07 §3 catch-up).
//!   3. Buffer-overflow → full rescan: see the KNOWN LIMITATION above — with notify 8.2 this does NOT
//!      reach `Overflow` on Windows; verify the M7 periodic-reconcile backstop instead.
//!   4. File-stability gate (size+mtime settle, open/lock probe, `.lnk` readiness — spec 07 §... the
//!      resident section): this is NOT in the watcher (a blocking probe would stall the debouncer
//!      worker). It belongs in the M7 reconciler/job layer as a retryable gate BEFORE formatting; its
//!      pure state machine is Mac-testable, only the real share-mode / `IShellLink` readiness is `[WV]`.

use std::path::PathBuf;
use std::time::Duration;

use notify_debouncer_full::notify::event::{ModifyKind, RenameMode};
use notify_debouncer_full::notify::{Event, EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

/// The default settle window (spec 07 §3/§16: debounce 4s, allowed range 2–10s). Four seconds folds
/// the installer temp-write→rename burst while staying responsive. A config-injected value should be
/// clamped to [2s, 10s] at the wiring layer (M7) — [`watch_desktops_with`] stays unclamped as the
/// test seam.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_secs(4);

/// A raw change hint from the filesystem watcher (spec 07 §3: hints, not decisions). Installers
/// commonly write-temp then rename, so `Renamed` and `Created` both occur for a new item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    Created(PathBuf),
    Changed(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
    Deleted(PathBuf),
    /// The watch buffer overflowed / the backend asked for a rescan — the consumer MUST fall back to
    /// a full reconcile (spec 07 §3). Carries no path: the truth is "re-scan everything".
    Overflow,
}

/// A live desktop watch. Holding it keeps the underlying OS watch armed. Dropping it (or calling
/// [`DesktopWatch::shutdown`]) STOPS the watch and JOINS the debouncer's worker thread — a real
/// quiescence barrier, so no `on_event` callback fires after it returns (the bare `Debouncer::drop`
/// only sets a stop flag and can drain one more tick; `stop()` joins). The resident owns one for the
/// app's lifetime.
pub struct DesktopWatch {
    debouncer: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
}

impl DesktopWatch {
    /// Stops the watch and joins the worker thread NOW (idempotent). Prefer this over `drop` when the
    /// caller needs to KNOW callbacks have quiesced before proceeding.
    pub fn shutdown(&mut self) {
        if let Some(debouncer) = self.debouncer.take() {
            debouncer.stop();
        }
    }
}

impl Drop for DesktopWatch {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Starts watching the user + public desktop `roots`, delivering settled [`WatchEvent`] hints to
/// `on_event`. The returned [`DesktopWatch`] MUST be held for the watch to stay live.
///
/// `on_event` runs on the debouncer's worker thread; keep it cheap (the resident just nudges its
/// reconciler). Uses the [`DEFAULT_DEBOUNCE`] settle window.
pub fn watch_desktops<F>(roots: Vec<PathBuf>, on_event: F) -> Result<DesktopWatch, String>
where
    F: FnMut(WatchEvent) + Send + 'static,
{
    watch_desktops_with(roots, DEFAULT_DEBOUNCE, on_event)
}

/// [`watch_desktops`] with an explicit settle window (spec 07 §3 allows 2–10s; tests use a short
/// one). Watches each root recursively — the reconciler filters, so over-notifying is safe.
pub fn watch_desktops_with<F>(
    roots: Vec<PathBuf>,
    debounce: Duration,
    mut on_event: F,
) -> Result<DesktopWatch, String>
where
    F: FnMut(WatchEvent) + Send + 'static,
{
    if roots.is_empty() {
        return Err("watch_desktops: no desktop roots supplied".to_string());
    }
    let mut debouncer = new_debouncer(debounce, None, move |result: DebounceEventResult| {
        match result {
            Ok(events) => {
                for de in &events {
                    if let Some(hint) = to_watch_event(&de.event) {
                        on_event(hint);
                    }
                }
            }
            // A backend error can mean a lost/overflowed watch: fail SAFE toward a full reconcile
            // (spec 07 §3) rather than silently dropping changes.
            Err(errors) => {
                if !errors.is_empty() {
                    on_event(WatchEvent::Overflow);
                }
            }
        }
    })
    .map_err(|e| format!("watcher init: {e}"))?;

    for root in &roots {
        // NON-recursive: the desktop scanner enumerates only the root's DIRECT children and skips
        // reparse points (shell/scan.rs) — matching that scope avoids walking a code repo / OneDrive
        // folder / junction that happens to sit on the desktop (which a recursive watch would traverse
        // at startup, then churn deep events + file-id memory for, codex B10-review 🟠). The debouncer's
        // file-id cache also walks the tree per root, so this bounds that too.
        debouncer
            .watch(root, RecursiveMode::NonRecursive)
            .map_err(|e| format!("watch {}: {e}", root.display()))?;
    }
    Ok(DesktopWatch { debouncer: Some(debouncer) })
}

/// Maps ONE settled `notify::Event` to at most one hint. A backend rescan flag (buffer overflow /
/// resume) wins over the kind — it means "re-scan everything". Rename-Both carries `[from, to]`;
/// a half-rename (To/From, when the pair straddles the watch boundary) reads as a create/delete.
fn to_watch_event(ev: &Event) -> Option<WatchEvent> {
    if ev.need_rescan() {
        return Some(WatchEvent::Overflow);
    }
    let first = || ev.paths.first().cloned();
    match ev.kind {
        EventKind::Create(_) => first().map(WatchEvent::Created),
        EventKind::Remove(_) => first().map(WatchEvent::Deleted),
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            match (ev.paths.first(), ev.paths.get(1)) {
                (Some(from), Some(to)) => {
                    Some(WatchEvent::Renamed { from: from.clone(), to: to.clone() })
                }
                _ => first().map(WatchEvent::Changed),
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => first().map(WatchEvent::Created),
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => first().map(WatchEvent::Deleted),
        EventKind::Modify(_) => first().map(WatchEvent::Changed),
        // Access / Any / Other are not desktop-content changes → no hint.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify_debouncer_full::notify::event::{
        AccessKind, CreateKind, DataChange, Flag, RemoveKind,
    };

    /// Builds a `notify::Event` via the stable builder API (no reliance on field visibility).
    fn ev(kind: EventKind, paths: &[&str]) -> Event {
        let mut e = Event::new(kind);
        for p in paths {
            e = e.add_path(PathBuf::from(*p));
        }
        e
    }

    #[test]
    fn maps_create_change_delete_and_rename() {
        assert_eq!(
            to_watch_event(&ev(EventKind::Create(CreateKind::File), &["/d/a.lnk"])),
            Some(WatchEvent::Created(PathBuf::from("/d/a.lnk")))
        );
        assert_eq!(
            to_watch_event(&ev(EventKind::Remove(RemoveKind::File), &["/d/a.lnk"])),
            Some(WatchEvent::Deleted(PathBuf::from("/d/a.lnk")))
        );
        assert_eq!(
            to_watch_event(&ev(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &["/d/a.lnk"]
            )),
            Some(WatchEvent::Changed(PathBuf::from("/d/a.lnk")))
        );
        // The installer temp-write→rename that the `-full` debouncer folds into one Rename-Both.
        assert_eq!(
            to_watch_event(&ev(
                EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
                &["/d/tmp123", "/d/a.lnk"]
            )),
            Some(WatchEvent::Renamed {
                from: PathBuf::from("/d/tmp123"),
                to: PathBuf::from("/d/a.lnk")
            })
        );
        // A rename INTO the watched dir (temp elsewhere → desktop) reads as a create.
        assert_eq!(
            to_watch_event(&ev(EventKind::Modify(ModifyKind::Name(RenameMode::To)), &["/d/a.lnk"])),
            Some(WatchEvent::Created(PathBuf::from("/d/a.lnk")))
        );
        // Access events carry no format-relevant signal.
        assert_eq!(
            to_watch_event(&ev(EventKind::Access(AccessKind::Any), &["/d/a.lnk"])),
            None
        );
    }

    #[test]
    fn a_backend_rescan_flag_becomes_overflow() {
        let e = ev(EventKind::Create(CreateKind::Any), &["/d/a.lnk"]).set_flag(Flag::Rescan);
        assert_eq!(to_watch_event(&e), Some(WatchEvent::Overflow));
    }

    #[test]
    fn live_watch_reports_a_new_file_through_the_debouncer() {
        use std::sync::mpsc::channel;
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = channel::<WatchEvent>();
        // A short settle window keeps the test fast while still exercising the real debouncer path.
        let _watch = watch_desktops_with(
            vec![dir.path().to_path_buf()],
            Duration::from_millis(120),
            move |e| {
                let _ = tx.send(e);
            },
        )
        .unwrap();

        // Give the backend a moment to arm before mutating (FSEvents/inotify register async).
        std::thread::sleep(Duration::from_millis(250));
        std::fs::write(dir.path().join("new-icon.lnk"), b"x").unwrap();

        // Collect hints until one names our file (create/change is a valid new-item hint) or we time
        // out. Debounce is 120ms, so ~3s is a generous ceiling for a loaded CI machine.
        let mut saw = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(WatchEvent::Created(p)) | Ok(WatchEvent::Changed(p))
                    if p.ends_with("new-icon.lnk") =>
                {
                    saw = true;
                    break;
                }
                _ => continue,
            }
        }
        assert!(saw, "the debounced watcher must report the newly-created desktop file");
    }
}
