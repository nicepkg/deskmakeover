//! The resident reconcile-loop driver (spec 07 §1/§3/§11/§12, plan T4/T8).
//!
//! This owns the *WHEN* of reconciliation — the pure loop LOGIC, decoupled from the OS so the whole
//! thing is Mac-unit-testable over fakes. The reconciler decision core ([`crate::reconciler`]) owns
//! the *WHAT*; the platform (watcher, activity hook, COM writers, stores) is injected behind three
//! traits ([`DriverClock`], [`WatchEventSource`], [`ReconcileEngine`]) plus the host surface
//! ([`ResidentHost`]). The driver:
//!
//! - **Coalesces a burst** — many [`WatchEvent`] hints between two heartbeats collapse to ONE
//!   reconcile pass ("events are hints, reconciliation is truth", spec 07 §3). Every reconcile is a
//!   full rescan, so per-event paths are irrelevant here; the driver only cares "were there hints?"
//! - **Runs a PERIODIC full reconcile** as the burst-loss backstop. notify 8.2 drops a Windows
//!   buffer overflow SILENTLY (`dm_windows::watcher` KNOWN LIMITATION), so [`WatchEvent::Overflow`]
//!   alone can NEVER be relied on for burst-loss recovery on Windows — a timer-driven full pass is
//!   mandatory. The injected [`DriverClock`] makes that timer Mac-testable with a fake clock.
//! - **Drives the tray state machine** — maps each [`ReconcileOutcome`] onto [`TrayState`]
//!   transitions (spec 07 §12): a busy-deferred wave → PAUSED, an idle cycle → back to WATCHING, a
//!   hard engine fault → ERROR. Batch WORKING transitions are host-driven (the host owns
//!   confirm/2h-timeout → `apply_batch`), fed back through [`ResidentDriver::on_batch_start`] /
//!   [`ResidentDriver::on_batch_done`].
//! - **Surfaces proposals** — v1 never auto-applies (spec 07 §2 item 4); the reconciler proposes and
//!   the driver hands the batch to the host ([`ResidentHost::surface_proposal`]), which renders the
//!   toast, owns the confirm/2h-timeout, and calls `apply_batch`. The pending-privileged count rides
//!   every tray render for the "待处理特权项(N)" line (spec 07 §12/§14).

#[cfg(test)]
mod tests;

use std::time::{Duration, Instant};

use dm_windows::watcher::WatchEvent;

use crate::reconciler::{ReconcileOutcome, VettedCandidate};
use crate::tray_state::{transition, TrayEvent, TrayState};

/// Monotonic wall-clock in milliseconds — injected so the periodic-reconcile timer (spec 07 §3) is
/// exercised by a fake clock the Mac tests advance, never by real sleeps. Prod uses [`MonotonicClock`].
pub trait DriverClock {
    fn now_ms(&self) -> u64;
}

/// A non-blocking source of coalesced watcher hints. Prod drains the `mpsc::Receiver<WatchEvent>` the
/// `dm_windows::watcher::watch_desktops` callback feeds; tests hand back a scripted `Vec`.
pub trait WatchEventSource {
    /// Takes every currently-buffered hint (non-blocking). The driver coalesces them into ONE pass.
    fn drain(&mut self) -> Vec<WatchEvent>;
}

/// The reconcile ENGINE the driver drives once per cycle. The driver owns the *when*; this owns the
/// *how* — build the real ports, lock the host's stores, and call
/// [`crate::reconciler::Reconciler::reconcile`]. Prod lives in `src-tauri/src/resident.rs` (real
/// `dm-windows` adapters on Windows, a devhost fake elsewhere); the Mac unit tests implement it over
/// the reconciler fakes. `Err` is a HARD fault (a durable journal/ledger write failed) → the driver
/// drives the tray to ERROR (spec 07 §12).
pub trait ReconcileEngine {
    /// Runs ONE full reconcile pass (spec 07 §3). The reconciler's internal stability gate runs
    /// BEFORE it classifies, so an unsettled newcomer is deferred here, never in the watcher.
    fn reconcile(&mut self) -> Result<ReconcileOutcome, String>;
}

/// A batched proposal handed to the host (spec 07 §2 item 4). Carries the vetted candidates — each
/// with its propose-time snapshot CAS fingerprint — for the eventual `apply_batch`, plus whether an
/// ANOMALY forces the toast tier regardless of the trust counter (spec 07 §2 item 7: a conflict, a
/// partial failure, or a newly-queued privileged item never silently degrades).
#[derive(Debug, Clone)]
pub struct Proposal {
    pub candidates: Vec<VettedCandidate>,
    pub anomaly: bool,
}

/// The host surface the driver pushes to. Prod renders the real tray + a toast; the Mac tests record
/// the calls to assert on them.
pub trait ResidentHost {
    /// Renders the tray to `state` (glyph + tooltip per spec 07 §12) with `pending_privileged`
    /// backing the "待处理特权项(N)" line.
    fn render_tray(&mut self, state: &TrayState, pending_privileged: usize);
    /// Surfaces a batched proposal (spec 07 §2 item 4): the host renders the toast, owns the
    /// confirm/2h-timeout, then calls `apply_batch`.
    fn surface_proposal(&mut self, proposal: Proposal);
}

/// Tuning for the loop. `full_reconcile` is the burst-loss backstop cadence (spec 07 §3 — the
/// mandatory belt-and-suspenders for the silent Windows overflow); `heartbeat` is how often the prod
/// [`ResidentDriver::run`] loop wakes to poll the event source.
#[derive(Debug, Clone, Copy)]
pub struct DriverConfig {
    pub full_reconcile: Duration,
    pub heartbeat: Duration,
}

impl Default for DriverConfig {
    fn default() -> Self {
        // 5 min periodic full reconcile (a cheap read_dir + fingerprint pass), polled every 500ms.
        Self { full_reconcile: Duration::from_secs(300), heartbeat: Duration::from_millis(500) }
    }
}

/// What one [`ResidentDriver::tick`] did — for the loop and the tests to observe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickReport {
    /// A reconcile pass ran this tick (a burst was pending OR the periodic timer fired).
    pub reconciled: bool,
    /// Candidates surfaced as a proposal this tick.
    pub proposed: usize,
    /// The engine returned a hard fault this tick (the tray is now ERROR).
    pub errored: bool,
}

/// The reconcile-loop driver. Holds only pure loop state; every OS touchpoint is injected.
pub struct ResidentDriver<C: DriverClock> {
    clock: C,
    config: DriverConfig,
    tray: TrayState,
    /// Whether automation is enabled (spec 07 §2). Distinct from `tray == Off`: a `Failure` can put
    /// an *enabled* driver into ERROR, and a disable can leave a recorded fault (spec 07 §12.1).
    enabled: bool,
    /// Last-known pending-privileged depth (spec 07 §14) — rides every tray render, and is preserved
    /// across an engine `Err` (which carries no fresh count).
    pending: usize,
    /// Coalescing flag: any hint since the last reconcile marks the loop dirty; one pass clears it.
    dirty: bool,
    /// Clock reading of the last reconcile pass; the periodic backstop measures from here.
    last_full_ms: u64,
}

impl<C: DriverClock> ResidentDriver<C> {
    /// A fresh driver: disabled, tray OFF. Enabling arms an immediate startup catch-up reconcile
    /// (spec 07 §3: full reconcile on startup) plus the periodic backstop.
    pub fn new(clock: C, config: DriverConfig) -> Self {
        let now = clock.now_ms();
        Self {
            clock,
            config,
            tray: TrayState::Off,
            enabled: false,
            pending: 0,
            dirty: false,
            last_full_ms: now,
        }
    }

    /// The current tray state (for the host to render + for tests).
    pub fn tray(&self) -> &TrayState {
        &self.tray
    }

    /// Whether automation is enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Enables/disables automation (the tray "☑自动整理新图标" toggle, spec 07 §2/§12.1). Enabling
    /// arms an immediate catch-up reconcile (spec 07 §3) and restarts the periodic clock; disabling
    /// is honoured from ANY state → OFF (owner decision 2026-07-13). NB: the §2 precondition (a
    /// non-empty ② saved-style) is enforced by the settings-patch layer BEFORE this is ever called
    /// with `true`; the driver only reflects the decided state.
    pub fn set_enabled(&mut self, enabled: bool, host: &mut dyn ResidentHost) {
        self.enabled = enabled;
        if enabled {
            self.dirty = true;
            self.last_full_ms = self.clock.now_ms();
        }
        let ev = if enabled { TrayEvent::ToggleOn } else { TrayEvent::ToggleOff };
        self.fire(ev, host);
    }

    /// Records a coalesced burst of watcher hints (spec 07 §3). Any hint marks the loop dirty so the
    /// NEXT [`tick`](Self::tick) runs exactly one full reconcile; an [`WatchEvent::Overflow`] is the
    /// same "re-scan everything" signal (and is NOT relied upon alone — the periodic backstop covers
    /// the silent-Windows-overflow case). No hint is ever dropped: a busy/unstable classification
    /// simply re-runs next cycle.
    pub fn note_events(&mut self, events: &[WatchEvent]) {
        if events.is_empty() {
            return;
        }
        // A burst of N hints coalesces into ONE reconcile — the whole point of the settle window +
        // this flag. Overflow is logged loudly because it means the watch buffer was lost (or, on
        // Windows, would have been silently); the periodic backstop is what actually recovers it.
        if events.iter().any(|e| matches!(e, WatchEvent::Overflow)) {
            log::warn!(
                "resident: watcher overflow hint — relying on the periodic full reconcile backstop \
                 (notify 8.2 drops Windows overflow silently; spec 07 §3)"
            );
        }
        self.dirty = true;
    }

    /// One heartbeat: if a burst is pending OR the periodic backstop is due, run a full reconcile and
    /// map its outcome onto the tray + proposal surface. A disabled driver is inert. Returns a small
    /// report for the loop/tests.
    pub fn tick(&mut self, engine: &mut dyn ReconcileEngine, host: &mut dyn ResidentHost) -> TickReport {
        if !self.enabled {
            return TickReport::default();
        }
        let now = self.clock.now_ms();
        let periodic_due = now.saturating_sub(self.last_full_ms) >= self.config.full_reconcile.as_millis() as u64;
        if !self.dirty && !periodic_due {
            return TickReport::default();
        }
        // Consume the burst + reset the periodic clock BEFORE the (possibly slow) pass, so hints that
        // arrive DURING the pass mark the loop dirty again and are not lost (spec 07 §3).
        self.dirty = false;
        self.last_full_ms = now;
        match engine.reconcile() {
            Ok(out) => self.absorb(out, host),
            Err(reason) => {
                // A hard durable fault (journal/ledger write failed) surfaces the ERROR tray from any
                // state; the pending-privileged count is preserved (the fault carries no fresh one).
                log::error!("resident: reconcile engine fault — {reason}");
                self.fire(TrayEvent::Failure { reason }, host);
                TickReport { reconciled: true, proposed: 0, errored: true }
            }
        }
    }

    /// Maps a successful reconcile outcome onto the tray + proposal surface.
    fn absorb(&mut self, out: ReconcileOutcome, host: &mut dyn ResidentHost) -> TickReport {
        self.pending = out.pending_privileged;
        // Activity (spec 07 §11): a busy-deferred wave pauses the tray; any non-busy cycle ends the
        // pause. Both are no-ops from WORKING/ERROR (the SM table declares them out) so a batch or a
        // recorded fault is never clobbered by a routine reconcile.
        let activity = if out.deferred_busy { TrayEvent::ActivityStart } else { TrayEvent::ActivityEnd };
        self.tray = transition(self.tray.clone(), activity);

        let proposed = out.proposed.len();
        if proposed > 0 {
            // An anomaly (a flagged conflict, or a privileged item routed to the queue this cycle)
            // forces the toast regardless of the earned trust tier (spec 07 §2 item 7).
            let anomaly = !out.conflicts.is_empty() || out.pending_privileged > 0;
            host.surface_proposal(Proposal { candidates: out.proposed, anomaly });
        }
        // Re-render every reconcile so a changed pending-privileged count reaches the tray line even
        // when the tray STATE itself did not move.
        host.render_tray(&self.tray, self.pending);
        TickReport { reconciled: true, proposed, errored: false }
    }

    /// The host tells the driver a batch STARTED formatting (it owns the confirm/2h-timeout →
    /// `apply_batch` path). Drives WATCHING/PAUSED → WORKING (spec 07 §12).
    pub fn on_batch_start(&mut self, count: u32, host: &mut dyn ResidentHost) {
        self.fire(TrayEvent::BatchStart { count }, host);
    }

    /// The host tells the driver the batch COMMITTED clean → WORKING → WATCHING (spec 07 §12).
    pub fn on_batch_done(&mut self, host: &mut dyn ResidentHost) {
        self.fire(TrayEvent::BatchDone, host);
    }

    /// The host reports a durable write / undo-journal failure → any state → ERROR (spec 07 §12).
    pub fn on_failure(&mut self, reason: String, host: &mut dyn ResidentHost) {
        self.fire(TrayEvent::Failure { reason }, host);
    }

    /// The user acknowledged/retried the error → ERROR → WATCHING (spec 07 §12).
    pub fn on_error_acknowledged(&mut self, host: &mut dyn ResidentHost) {
        self.fire(TrayEvent::ErrorAcknowledged, host);
    }

    /// The current pending-privileged depth (spec 07 §14) — the "待处理特权项(N)" count.
    pub fn pending_privileged(&self) -> usize {
        self.pending
    }

    /// The driver's clock reading (ms) — the host uses the SAME clock for the proposal 2h-timeout
    /// deadline math (spec 07 §2 item 4) so the two never drift.
    pub fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    /// Forces the NEXT [`tick`](Self::tick) to run a full reconcile even with no watcher hints — the
    /// tray "立即整理桌面" action (spec 07 §12) and any explicit host-requested rescan.
    pub fn request_reconcile(&mut self) {
        self.dirty = true;
    }

    /// Applies a tray event through the SM and re-renders (with the live pending-privileged count).
    fn fire(&mut self, event: TrayEvent, host: &mut dyn ResidentHost) {
        self.tray = transition(self.tray.clone(), event);
        host.render_tray(&self.tray, self.pending);
    }

    /// The prod propose-loop the host spawns on a background thread: poll the injected event source,
    /// coalesce, tick, sleep, until `shutdown` flips. Confirm/2h-timeout → `apply_batch` is the
    /// HOST's job (it owns the engine + the timeout), interleaved in its own loop; this convenience
    /// covers the pure propose path (and is exercised by the driver tests over fakes).
    pub fn run(
        &mut self,
        source: &mut dyn WatchEventSource,
        engine: &mut dyn ReconcileEngine,
        host: &mut dyn ResidentHost,
        shutdown: &dyn Fn() -> bool,
    ) {
        while !shutdown() {
            let events = source.drain();
            self.note_events(&events);
            self.tick(engine, host);
            std::thread::sleep(self.config.heartbeat);
        }
    }
}

/// The prod monotonic clock (spec 07 §3 periodic timer). Anchored at construction so `now_ms` is a
/// small, always-increasing offset.
pub struct MonotonicClock {
    start: Instant,
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MonotonicClock {
    pub fn new() -> Self {
        Self { start: Instant::now() }
    }
}

impl DriverClock for MonotonicClock {
    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}
