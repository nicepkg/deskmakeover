//! Mac unit tests for the reconcile-loop driver (plan T4/T8, spec 07 §3/§11/§12). The pure loop
//! LOGIC — coalescing, the periodic backstop, tray mapping, proposal surfacing — is exercised over a
//! `ScriptedEngine` (isolates the driver from the reconciler); one INTEGRATION test drives a REAL
//! [`Reconciler`] over the reconciler fakes (incl. a settable `FakeActivityMonitor`) so "a busy
//! desktop defers" flows end to end through the driver → tray. No sleeps: a fake clock is advanced.

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

use dm_contracts::IconStyle;
use dm_domain::{
    ActivityMonitor, ApplyAssets, AssetRef, AssetStore, DecodedImage, DesktopItem, DesktopScanner,
    Fingerprint, IconApplier, IconSourceExtractor, ItemId, ItemKind, ItemState, ItemStateReader,
    ItemTarget, PortError, PortResult, RestoreAnchor,
};
use dm_operations::icons::scope::ScopeRoots;
use dm_operations::{MemLedgerStore, TxnIdAllocator, VecJournal};
use dm_windows::watcher::WatchEvent;
use serde_json::json;

use super::*;
use crate::consent::{FreshnessInputs, TrustState};
use crate::reconciler::{
    ReconcileContext, ReconcileOutcome, Reconciler, ReconcilerPorts, VettedCandidate,
};
use crate::stability::{StabilityReader, StabilitySnapshot};

// ---- injected-dependency fakes ---------------------------------------------------------------

/// A test clock the test advances by hand (shared with the driver via `Rc`).
#[derive(Clone)]
struct FakeClock(Rc<Cell<u64>>);
impl FakeClock {
    fn new() -> Self {
        Self(Rc::new(Cell::new(0)))
    }
    fn advance(&self, ms: u64) {
        self.0.set(self.0.get() + ms);
    }
}
impl DriverClock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0.get()
    }
}

/// Records every tray render + surfaced proposal so tests can assert on them.
#[derive(Default)]
struct RecordingHost {
    renders: Vec<(TrayState, usize)>,
    proposals: Vec<Proposal>,
}
impl ResidentHost for RecordingHost {
    fn render_tray(&mut self, state: &TrayState, pending_privileged: usize) {
        self.renders.push((state.clone(), pending_privileged));
    }
    fn surface_proposal(&mut self, proposal: Proposal) {
        self.proposals.push(proposal);
    }
}
impl RecordingHost {
    fn last_tray(&self) -> TrayState {
        self.renders.last().expect("a render happened").0.clone()
    }
}

/// A scripted engine: pops the next queued outcome per `reconcile`, counting calls. An exhausted
/// script returns a clean empty outcome.
struct ScriptedEngine {
    outcomes: VecDeque<Result<ReconcileOutcome, String>>,
    calls: usize,
}
impl ScriptedEngine {
    fn new(seq: Vec<Result<ReconcileOutcome, String>>) -> Self {
        Self { outcomes: seq.into(), calls: 0 }
    }
}
impl ReconcileEngine for ScriptedEngine {
    fn reconcile(&mut self) -> Result<ReconcileOutcome, String> {
        self.calls += 1;
        self.outcomes.pop_front().unwrap_or_else(|| Ok(ReconcileOutcome::default()))
    }
}

fn outcome_with_proposals(n: usize) -> ReconcileOutcome {
    ReconcileOutcome {
        proposed: (0..n)
            .map(|i| VettedCandidate {
                item: user_item(&format!("i{i}")),
                fingerprint: Fingerprint::of_bytes(b"x"),
            })
            .collect(),
        ..Default::default()
    }
}

fn busy_outcome() -> ReconcileOutcome {
    ReconcileOutcome { deferred_busy: true, ..Default::default() }
}

fn user_item(id: &str) -> DesktopItem {
    DesktopItem {
        id: ItemId::from_raw(id),
        name: id.to_string(),
        path: format!("C:/Users/Dev/Desktop/{id}.lnk"),
        kind: ItemKind::Shortcut,
        icon: None,
        state: ItemState::Ready,
        requires_explicit_consent: false,
        status_message: None,
    }
}

// ---- driver-logic tests (scripted engine) ----------------------------------------------------

fn driver(clock: &FakeClock) -> ResidentDriver<FakeClock> {
    ResidentDriver::new(clock.clone(), DriverConfig { full_reconcile: Duration::from_secs(300), heartbeat: Duration::from_millis(1) })
}

#[test]
fn a_disabled_driver_never_reconciles() {
    let clock = FakeClock::new();
    let mut d = driver(&clock);
    let mut eng = ScriptedEngine::new(vec![]);
    let mut host = RecordingHost::default();
    // No enable → no reconcile even with hints pending and the clock well past the periodic horizon.
    d.note_events(&[WatchEvent::Created("/d/a.lnk".into())]);
    clock.advance(10 * 60 * 1000);
    let r = d.tick(&mut eng, &mut host);
    assert!(!r.reconciled && eng.calls == 0, "OFF is inert");
    assert_eq!(*d.tray(), TrayState::Off);
}

#[test]
fn a_burst_of_hints_coalesces_into_exactly_one_reconcile() {
    let clock = FakeClock::new();
    let mut d = driver(&clock);
    let mut host = RecordingHost::default();
    // Enabling arms ONE startup catch-up reconcile (spec 07 §3).
    let mut eng = ScriptedEngine::new(vec![Ok(ReconcileOutcome::default()), Ok(outcome_with_proposals(2))]);
    d.set_enabled(true, &mut host);
    assert_eq!(*d.tray(), TrayState::Watching);
    let r = d.tick(&mut eng, &mut host); // startup catch-up
    assert!(r.reconciled && eng.calls == 1);

    // A BURST of three hints between heartbeats must collapse to ONE reconcile (spec 07 §3).
    d.note_events(&[
        WatchEvent::Created("/d/a.lnk".into()),
        WatchEvent::Changed("/d/a.lnk".into()),
        WatchEvent::Renamed { from: "/d/tmp".into(), to: "/d/b.lnk".into() },
    ]);
    let r = d.tick(&mut eng, &mut host);
    assert!(r.reconciled && r.proposed == 2, "the coalesced burst proposed its batch");
    assert_eq!(eng.calls, 2, "three hints → ONE additional reconcile");

    // No further hints + clock not past the periodic horizon → no more reconciles.
    let r = d.tick(&mut eng, &mut host);
    assert!(!r.reconciled && eng.calls == 2, "an idle tick with no dirty flag is a no-op");
}

#[test]
fn the_periodic_full_reconcile_fires_with_no_events_as_the_burst_loss_backstop() {
    // spec 07 §3 / watcher KNOWN LIMITATION: notify 8.2 drops a Windows overflow silently, so the
    // timer-driven full pass is the ONLY reliable burst-loss recovery — it MUST fire without hints.
    let clock = FakeClock::new();
    let mut d = driver(&clock);
    let mut eng = ScriptedEngine::new(vec![Ok(ReconcileOutcome::default()); 4]);
    let mut host = RecordingHost::default();
    d.set_enabled(true, &mut host);
    d.tick(&mut eng, &mut host); // startup pass consumes the enable-armed dirty flag
    assert_eq!(eng.calls, 1);

    // Not yet due → no reconcile.
    clock.advance(299_000);
    assert!(!d.tick(&mut eng, &mut host).reconciled, "before the horizon: no pass");
    assert_eq!(eng.calls, 1);

    // Cross the 300s horizon with ZERO hints → the periodic backstop fires.
    clock.advance(1_500);
    assert!(d.tick(&mut eng, &mut host).reconciled, "the periodic full reconcile fires with no events");
    assert_eq!(eng.calls, 2);

    // The horizon resets from each pass; another full interval with no hints fires again.
    clock.advance(300_000);
    assert!(d.tick(&mut eng, &mut host).reconciled);
    assert_eq!(eng.calls, 3);
}

#[test]
fn an_outcome_drives_the_right_tray_transition() {
    let clock = FakeClock::new();
    let mut d = driver(&clock);
    let mut host = RecordingHost::default();
    let mut eng = ScriptedEngine::new(vec![
        Ok(busy_outcome()),               // busy → PAUSED
        Ok(outcome_with_proposals(1)),    // idle + proposal → back to WATCHING, proposal surfaced
        Err("journal write denied".into()), // hard fault → ERROR
    ]);
    d.set_enabled(true, &mut host);
    assert_eq!(*d.tray(), TrayState::Watching);

    // 1) A busy-deferred wave pauses the tray (spec 07 §11/§12) and surfaces nothing.
    d.note_events(&[WatchEvent::Created("/d/a.lnk".into())]);
    d.tick(&mut eng, &mut host);
    assert_eq!(host.last_tray(), TrayState::Paused);
    assert!(host.proposals.is_empty(), "a busy wave proposes nothing");

    // 2) An idle cycle with a proposal ends the pause (PAUSED→WATCHING) and surfaces the batch.
    d.note_events(&[WatchEvent::Created("/d/a.lnk".into())]);
    d.tick(&mut eng, &mut host);
    assert_eq!(host.last_tray(), TrayState::Watching);
    assert_eq!(host.proposals.len(), 1, "the idle cycle surfaced its proposal");

    // 3) A hard engine fault surfaces ERROR from any state (spec 07 §12).
    d.note_events(&[WatchEvent::Created("/d/a.lnk".into())]);
    let r = d.tick(&mut eng, &mut host);
    assert!(r.errored);
    assert!(matches!(host.last_tray(), TrayState::Error { .. }));
}

#[test]
fn host_driven_batch_and_error_transitions_move_the_tray() {
    let clock = FakeClock::new();
    let mut d = driver(&clock);
    let mut host = RecordingHost::default();
    d.set_enabled(true, &mut host);

    // The host owns confirm/timeout → apply_batch; it reports the batch lifecycle to the tray.
    d.on_batch_start(3, &mut host);
    assert_eq!(host.last_tray(), TrayState::Working { count: 3 });
    d.on_batch_done(&mut host);
    assert_eq!(host.last_tray(), TrayState::Watching);

    // A durable write / undo-journal failure → ERROR; the user acknowledging returns to WATCHING.
    d.on_failure("ledger flush failed".into(), &mut host);
    assert!(matches!(host.last_tray(), TrayState::Error { .. }));
    d.on_error_acknowledged(&mut host);
    assert_eq!(host.last_tray(), TrayState::Watching);

    // Disable from ANY state → OFF (spec 07 §12.1).
    d.on_batch_start(2, &mut host);
    d.set_enabled(false, &mut host);
    assert_eq!(host.last_tray(), TrayState::Off);
    assert!(!d.enabled());
}

#[test]
fn a_pending_privileged_count_rides_every_tray_render() {
    // spec 07 §14: the "待处理特权项(N)" line reflects the reconciler's queue depth each cycle.
    let clock = FakeClock::new();
    let mut d = driver(&clock);
    let mut host = RecordingHost::default();
    let queued = ReconcileOutcome { pending_privileged: 2, ..Default::default() };
    let mut eng = ScriptedEngine::new(vec![Ok(queued)]);
    d.set_enabled(true, &mut host);
    d.tick(&mut eng, &mut host);
    assert_eq!(host.renders.last().unwrap().1, 2, "the render carried the pending-privileged count");
    assert_eq!(d.pending_privileged(), 2);
}

#[test]
fn run_loops_until_shutdown_then_stops() {
    // Exercises the prod propose-loop over fakes: it drains → coalesces → ticks each iteration.
    let clock = FakeClock::new();
    let mut d = driver(&clock);
    d.set_enabled(true, &mut RecordingHost::default());
    let mut eng = ScriptedEngine::new(vec![Ok(outcome_with_proposals(1)); 8]);
    let mut host = RecordingHost::default();
    let mut source = ScriptedSource(vec![vec![WatchEvent::Created("/d/a.lnk".into())]].into());
    let ticks = Cell::new(0u32);
    let shutdown = || {
        let n = ticks.get() + 1;
        ticks.set(n);
        n > 3 // run three iterations then stop
    };
    d.run(&mut source, &mut eng, &mut host, &shutdown);
    assert!(eng.calls >= 1, "the loop ran at least the initial burst reconcile");
}

/// A scripted event source: each `drain` pops the next batch of hints (empty once exhausted).
struct ScriptedSource(VecDeque<Vec<WatchEvent>>);
impl WatchEventSource for ScriptedSource {
    fn drain(&mut self) -> Vec<WatchEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

// ---- integration: a REAL Reconciler over the fakes (a busy desktop defers) --------------------

struct AlwaysSettle;
impl StabilityReader for AlwaysSettle {
    fn snapshot(&self, _path: &str) -> StabilitySnapshot {
        StabilitySnapshot { size: 100, mtime_nanos: 1, readable: true }
    }
}

/// The reconciler-test `FakeActivityMonitor` (spec 07 §11): an always-idle default with a settable
/// busy flag, shared with the test via `Rc`.
struct FakeActivityMonitor {
    busy: Cell<bool>,
}
impl FakeActivityMonitor {
    fn new() -> Rc<Self> {
        Rc::new(Self { busy: Cell::new(false) })
    }
    fn set_busy(&self, busy: bool) {
        self.busy.set(busy);
    }
}
impl ActivityMonitor for FakeActivityMonitor {
    fn is_desktop_busy(&self) -> PortResult<bool> {
        Ok(self.busy.get())
    }
}

/// A one-item virtual desktop backing the reader (reconcile only reads fingerprints; the extractor /
/// applier / assets exist to satisfy the port bundle but are never hit on the propose path).
struct Desk;
impl ItemStateReader for Desk {
    fn read_fingerprint(&self, target: &ItemTarget) -> PortResult<Fingerprint> {
        Ok(Fingerprint::of_bytes(format!("original:{}", target.path).as_bytes()))
    }
    fn capture_anchor(&self, target: &ItemTarget) -> PortResult<RestoreAnchor> {
        Ok(RestoreAnchor::FileBytes { bytes: format!("original:{}", target.path).into_bytes() })
    }
}
struct OneScanner(Vec<DesktopItem>);
impl DesktopScanner for OneScanner {
    fn scan(&self) -> PortResult<Vec<DesktopItem>> {
        Ok(self.0.clone())
    }
}
struct StubExtractor;
impl IconSourceExtractor for StubExtractor {
    fn extract(&self, _i: &DesktopItem, _o: Option<&RestoreAnchor>) -> PortResult<Vec<DecodedImage>> {
        Err(PortError::Unsupported("not exercised on the propose path".into()))
    }
}
struct StubApplier;
impl IconApplier for StubApplier {
    fn apply(&self, _t: &ItemTarget, _a: &ApplyAssets) -> PortResult<Fingerprint> {
        Err(PortError::Unsupported("not exercised on the propose path".into()))
    }
    fn restore(&self, _t: &ItemTarget, _a: &RestoreAnchor) -> PortResult<()> {
        Ok(())
    }
}
struct StubAssets;
impl AssetStore for StubAssets {
    fn put(&self, hash: &str, _b: &[u8]) -> PortResult<AssetRef> {
        Ok(AssetRef::new(hash, format!("assets/{hash}.ico")))
    }
    fn put_empty_variant(&self, primary: &AssetRef, _b: &[u8]) -> PortResult<AssetRef> {
        Ok(AssetRef::new(&primary.hash, "assets/x.ico".to_string()))
    }
    fn exists(&self, _a: &AssetRef) -> PortResult<bool> {
        Ok(true)
    }
    fn gc(&self, _l: &[String]) -> PortResult<()> {
        Ok(())
    }
}

fn style() -> IconStyle {
    IconStyle::from_value(json!({
        "config": {
            "shape": "Circle", "subject": "Original", "tint": "#FF6F5E",
            "monoStyle": "Tonal", "plateBand": "Vivid", "shortcutShape": null,
            "distinction": "None", "markStyle": "Glass", "markColor": null,
            "size": "Mid", "filter": "None", "plateColor": null, "plateFallback": "derived"
        },
        "kindPolicy": {}, "typeOverrides": {}
    }))
    .unwrap()
}

/// A REAL reconcile engine over the fakes — the driver drives `Reconciler::reconcile` for real, with
/// a settable `FakeActivityMonitor` so a busy desktop's `deferred_busy` flows through to the tray.
struct RealEngine {
    rec: Reconciler,
    items: Vec<DesktopItem>,
    activity: Rc<FakeActivityMonitor>,
    style: IconStyle,
    trust: TrustState,
    scope: ScopeRoots,
    journal: VecJournal,
    ledger: MemLedgerStore,
    txn: TxnIdAllocator,
}
impl RealEngine {
    fn new(activity: Rc<FakeActivityMonitor>) -> Self {
        Self {
            rec: Reconciler::new(),
            items: vec![user_item("fresh")],
            activity,
            style: style(),
            trust: TrustState { batches_without_undo: 3 },
            scope: ScopeRoots::resolved(
                vec!["C:/Users/Public/Desktop".into()],
                vec!["C:/ProgramData".into()],
            )
            .unwrap(),
            journal: VecJournal::default(),
            ledger: MemLedgerStore::default(),
            txn: TxnIdAllocator::starting_at(1),
        }
    }
}
impl ReconcileEngine for RealEngine {
    fn reconcile(&mut self) -> Result<ReconcileOutcome, String> {
        let desk = Desk;
        let scanner = OneScanner(self.items.clone());
        let stability = AlwaysSettle;
        let ports = ReconcilerPorts {
            scanner: &scanner,
            extractor: &StubExtractor,
            reader: &desk,
            applier: &StubApplier,
            assets: &StubAssets,
            activity: &*self.activity,
            stability: &stability,
        };
        let ctx = ReconcileContext {
            saved_style: Some(&self.style),
            trust: &self.trust,
            freshness: FreshnessInputs { last_apply_at: Some(0), partial_reversion: false, now: 0 },
            scope: &self.scope,
        };
        self.rec
            .reconcile(&ports, &ctx, &mut self.txn, &mut self.journal, &mut self.ledger)
            .map_err(|e| e.to_string())
    }
}

#[test]
fn a_busy_desktop_defers_through_a_real_reconciler_and_pauses_the_tray() {
    let clock = FakeClock::new();
    let mut d = driver(&clock);
    let mut host = RecordingHost::default();
    let activity = FakeActivityMonitor::new();
    let mut eng = RealEngine::new(activity.clone());
    d.set_enabled(true, &mut host);

    // Cycle 1 (idle): first sight of the newcomer — the reconciler's own stability gate defers it,
    // nothing proposed yet.
    d.note_events(&[WatchEvent::Created("/d/fresh.lnk".into())]);
    let r = d.tick(&mut eng, &mut host);
    assert!(r.reconciled && r.proposed == 0, "first sight is held by the settle gate");

    // Now the desktop goes BUSY under the user's cursor (spec 07 §11): the wave defers → tray PAUSED,
    // still nothing proposed — driven by the FakeActivityMonitor through the real reconciler.
    activity.set_busy(true);
    d.note_events(&[WatchEvent::Created("/d/fresh.lnk".into())]);
    let r = d.tick(&mut eng, &mut host);
    assert!(r.reconciled && r.proposed == 0, "a busy desktop defers the wave");
    assert_eq!(host.last_tray(), TrayState::Paused);

    // Idle again: the settled newcomer is now proposed and the pause ends (PAUSED→WATCHING).
    activity.set_busy(false);
    d.note_events(&[WatchEvent::Created("/d/fresh.lnk".into())]);
    let r = d.tick(&mut eng, &mut host);
    assert_eq!(r.proposed, 1, "the deferred newcomer is not lost — it proposes once idle");
    assert_eq!(host.last_tray(), TrayState::Watching);
    assert_eq!(host.proposals.len(), 1);
}
