//! Mac unit tests for the T8 resident wiring (plan T8, spec 07 §2/§3). Exercises the REAL Mac
//! devhost engine + the driver end to end (no GUI): a synthetic watcher hint reaches a proposal, and
//! a confirm drives `apply_batch` through the actual `TxnDriver` bake path. The tray/window shell
//! itself needs a running Tauri app (no display in the sandbox), so those steps are `[WV]`/manual —
//! see docs/references/windows-wiring-handoff/m7-resident.md.

use std::sync::Arc;

use dm_contracts::IconStyle;
use dm_operations::SettingsStore;
use dm_resident::{
    DriverConfig, MonotonicClock, Proposal, ResidentDriver, ResidentHost, TrayState, WatchEvent,
};
use serde_json::json;

use super::engine::{DevhostResidentEngine, ResidentEngine};

#[derive(Default)]
struct TestHost {
    proposals: Vec<Proposal>,
    last_tray: Option<TrayState>,
}
impl ResidentHost for TestHost {
    fn render_tray(&mut self, state: &TrayState, _pending: usize) {
        self.last_tray = Some(state.clone());
    }
    fn surface_proposal(&mut self, proposal: Proposal) {
        self.proposals.push(proposal);
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

/// An in-memory settings store with a seeded ② saved-style (spec 07 §8), so the reconciler has a
/// style to project — otherwise it is correctly dormant.
fn seeded_settings() -> Arc<SettingsStore> {
    let s = SettingsStore::open_in_memory().unwrap();
    s.set_saved_style(Some(&style())).unwrap();
    Arc::new(s)
}

#[test]
fn a_synthetic_watcher_event_reaches_a_proposal_and_confirm_applies() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = DevhostResidentEngine::new(seeded_settings(), dir.path());
    let mut driver = ResidentDriver::new(MonotonicClock::new(), DriverConfig::default());
    let mut host = TestHost::default();
    driver.set_enabled(true, &mut host);
    assert_eq!(host.last_tray, Some(TrayState::Watching));

    // Cycle 1: a synthetic hint triggers a reconcile; the devhost newcomers are first-seen → the
    // settle gate defers them (spec 07 §3), nothing proposed yet.
    driver.note_events(&[WatchEvent::Created("C:/Users/Dev/Desktop/edge".into())]);
    let r = driver.tick(&mut engine, &mut host);
    assert!(r.reconciled && host.proposals.is_empty(), "first sight is held by the settle gate");

    // Cycle 2: the newcomers have settled → they are PROPOSED (v1 never auto-applies, spec 07 §2).
    driver.request_reconcile();
    driver.tick(&mut engine, &mut host);
    assert!(!host.proposals.is_empty(), "a synthetic watcher event reaches a proposal within a cycle");

    // Confirm → apply_batch writes store ① through the REAL TxnDriver bake path (dm-icon-core).
    let batch = host.proposals.pop().unwrap().candidates;
    assert!(!batch.is_empty());
    let out = engine.apply_batch(batch).expect("apply_batch runs the real bake+apply");
    assert!(!out.applied.is_empty(), "the confirmed batch applies at least one candidate to store ①");
}

#[test]
fn an_empty_saved_style_keeps_the_resident_dormant() {
    // spec 07 §8.3: a None ② means "nothing to project" — the loop proposes nothing.
    let dir = tempfile::tempdir().unwrap();
    let settings = Arc::new(SettingsStore::open_in_memory().unwrap()); // ② empty
    let mut engine = DevhostResidentEngine::new(settings, dir.path());
    let mut driver = ResidentDriver::new(MonotonicClock::new(), DriverConfig::default());
    let mut host = TestHost::default();
    driver.set_enabled(true, &mut host);
    driver.note_events(&[WatchEvent::Created("C:/Users/Dev/Desktop/edge".into())]);
    driver.tick(&mut engine, &mut host);
    driver.request_reconcile();
    driver.tick(&mut engine, &mut host);
    assert!(host.proposals.is_empty(), "an empty ② keeps the resident dormant");
}
