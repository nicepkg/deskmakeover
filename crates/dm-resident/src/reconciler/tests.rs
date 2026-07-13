//! The spec 07 §7 M7 gates that own the reconciler: classification, the stability gate, the
//! trust/freshness ladder, mid-batch activity suppression, and the §14 privileged red line —
//! all over fakes on a virtual desktop (the same discipline as the txn driver's own tests).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use dm_contracts::IconStyle;
use dm_domain::{
    ActivityMonitor, ApplyAssets, AssetRef, AssetStore, DecodedImage, DesktopItem, DesktopScanner,
    Fingerprint, IconApplier, IconSourceExtractor, ItemId, ItemKind, ItemState, ItemStateReader,
    ItemTarget, PortError, PortResult, RestoreAnchor,
};
use dm_operations::{MemLedgerStore, TxnIdAllocator, VecJournal};
use serde_json::json;

use super::*;
use crate::consent::{FreshnessInputs, TrustState};
use crate::stability::{StabilityReader, StabilitySnapshot};

// ---- Fakes -----------------------------------------------------------------------------------

/// The shared virtual desktop: path → surface bytes. Seeded `original:<id>`.
#[derive(Default)]
struct Desk {
    surfaces: RefCell<HashMap<String, Vec<u8>>>,
    /// Every path the applier styled, in order — the §14 red-line assertion reads this.
    applied_paths: RefCell<Vec<String>>,
}

impl Desk {
    fn seed(&self, items: &[DesktopItem]) {
        for it in items {
            self.surfaces
                .borrow_mut()
                .insert(it.path.clone(), format!("original:{}", it.id.as_str()).into_bytes());
        }
    }
}

struct FakeScanner(Vec<DesktopItem>);
impl DesktopScanner for FakeScanner {
    fn scan(&self) -> PortResult<Vec<DesktopItem>> {
        Ok(self.0.clone())
    }
}

struct FakeExtractor;
impl IconSourceExtractor for FakeExtractor {
    fn extract(
        &self,
        item: &DesktopItem,
        _original: Option<&RestoreAnchor>,
    ) -> PortResult<Vec<DecodedImage>> {
        let mut sources = vec![source_png(item.id.as_str())];
        if item.kind == ItemKind::RecycleBin {
            sources.push(source_png("empty"));
        }
        Ok(sources)
    }
}

struct DeskReader<'a>(&'a Desk);
impl ItemStateReader for DeskReader<'_> {
    fn read_fingerprint(&self, target: &ItemTarget) -> PortResult<Fingerprint> {
        self.0
            .surfaces
            .borrow()
            .get(&target.path)
            .map(|b| Fingerprint::of_bytes(b))
            .ok_or_else(|| PortError::NotFound(target.path.clone()))
    }

    fn capture_anchor(&self, target: &ItemTarget) -> PortResult<RestoreAnchor> {
        self.0
            .surfaces
            .borrow()
            .get(&target.path)
            .map(|bytes| RestoreAnchor::FileBytes { bytes: bytes.clone() })
            .ok_or_else(|| PortError::NotFound(target.path.clone()))
    }
}

struct DeskApplier<'a>(&'a Desk);
impl IconApplier for DeskApplier<'_> {
    fn apply(&self, target: &ItemTarget, assets: &ApplyAssets) -> PortResult<Fingerprint> {
        let styled = format!("styled:{}", assets.primary.hash).into_bytes();
        self.0.surfaces.borrow_mut().insert(target.path.clone(), styled.clone());
        self.0.applied_paths.borrow_mut().push(target.path.clone());
        Ok(Fingerprint::of_bytes(&styled))
    }

    fn restore(&self, target: &ItemTarget, anchor: &RestoreAnchor) -> PortResult<()> {
        if let RestoreAnchor::FileBytes { bytes } = anchor {
            self.0.surfaces.borrow_mut().insert(target.path.clone(), bytes.clone());
        }
        Ok(())
    }
}

#[derive(Default)]
struct MemAssets(RefCell<HashMap<String, Vec<u8>>>);
impl AssetStore for MemAssets {
    fn put(&self, hash: &str, bytes: &[u8]) -> PortResult<AssetRef> {
        self.0.borrow_mut().insert(hash.to_string(), bytes.to_vec());
        Ok(AssetRef::new(hash, format!("assets/{hash}.ico")))
    }

    fn put_empty_variant(&self, primary: &AssetRef, bytes: &[u8]) -> PortResult<AssetRef> {
        let hash = format!("{}-empty", primary.hash);
        self.0.borrow_mut().insert(hash.clone(), bytes.to_vec());
        Ok(AssetRef::new(&hash, format!("assets/{hash}.ico")))
    }

    fn exists(&self, asset: &AssetRef) -> PortResult<bool> {
        Ok(self.0.borrow().contains_key(&asset.hash))
    }

    fn gc(&self, live: &[String]) -> PortResult<()> {
        self.0.borrow_mut().retain(|k, _| live.iter().any(|l| l == k));
        Ok(())
    }
}

/// Scripted busy flags: each `is_desktop_busy` call pops the next script entry; an exhausted
/// script reads idle.
struct ScriptedActivity {
    script: RefCell<Vec<bool>>,
    calls: Cell<usize>,
}

impl ScriptedActivity {
    fn idle() -> Self {
        Self { script: RefCell::new(Vec::new()), calls: Cell::new(0) }
    }

    fn script(seq: &[bool]) -> Self {
        Self { script: RefCell::new(seq.to_vec()), calls: Cell::new(0) }
    }
}

impl ActivityMonitor for ScriptedActivity {
    fn is_desktop_busy(&self) -> PortResult<bool> {
        self.calls.set(self.calls.get() + 1);
        let mut s = self.script.borrow_mut();
        Ok(if s.is_empty() { false } else { s.remove(0) })
    }
}

/// Scripted stability: paths in `unstable` report a moving size each call; others settle.
#[derive(Default)]
struct ScriptedStability {
    unstable: RefCell<HashMap<String, u64>>,
}

impl ScriptedStability {
    fn unstable(paths: &[&str]) -> Self {
        let s = Self::default();
        for p in paths {
            s.unstable.borrow_mut().insert((*p).to_string(), 0);
        }
        s
    }
}

impl StabilityReader for ScriptedStability {
    fn snapshot(&self, path: &str) -> StabilitySnapshot {
        let mut u = self.unstable.borrow_mut();
        if let Some(counter) = u.get_mut(path) {
            *counter += 1;
            StabilitySnapshot { size: 100 + *counter, mtime_nanos: 1, readable: true }
        } else {
            StabilitySnapshot { size: 100, mtime_nanos: 1, readable: true }
        }
    }
}

// ---- Harness ---------------------------------------------------------------------------------

fn source_png(seed: &str) -> DecodedImage {
    use image::ImageEncoder;
    let tone = seed.bytes().fold(0u8, |a, b| a.wrapping_add(b));
    let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([tone, 120, 200, 255]));
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&img, 256, 256, image::ExtendedColorType::Rgba8)
        .unwrap();
    DecodedImage { width: 256, height: 256, png }
}

fn item(id: &str, kind: ItemKind, path: &str) -> DesktopItem {
    DesktopItem {
        id: ItemId::from_raw(id),
        name: id.to_string(),
        path: path.to_string(),
        kind,
        icon: None,
        state: ItemState::Ready,
        requires_explicit_consent: false,
        status_message: None,
    }
}

fn user_item(id: &str) -> DesktopItem {
    item(id, ItemKind::Shortcut, &format!("C:/Users/Dev/Desktop/{id}.lnk"))
}

fn style() -> IconStyle {
    IconStyle::from_value(json!({
        "config": {
            "shape": "Circle", "subject": "Original", "tint": "#FF6F5E",
            "monoStyle": "Tonal", "plateBand": "Vivid", "shortcutShape": null,
            "distinction": "None", "markStyle": "Glass", "markColor": null,
            "size": "Mid", "filter": "None", "plateColor": null, "plateFallback": "derived"
        },
        "kindPolicy": {},
        "typeOverrides": {}
    }))
    .unwrap()
}

fn silent_trust() -> TrustState {
    TrustState { batches_without_undo: 3 }
}

fn fresh(now: i64) -> FreshnessInputs {
    FreshnessInputs { last_apply_at: Some(now - 3600), partial_reversion: false, now }
}

const NOW: i64 = 1_800_000_000;
const PUBLIC_ROOT: &str = "C:/Users/Public/Desktop";

struct World {
    desk: Rc<Desk>,
    journal: VecJournal,
    ledger: MemLedgerStore,
    txn: TxnIdAllocator,
    assets: MemAssets,
}

impl World {
    fn new(items: &[DesktopItem]) -> Self {
        let desk = Rc::new(Desk::default());
        desk.seed(items);
        Self {
            desk,
            journal: VecJournal::default(),
            ledger: MemLedgerStore::default(),
            txn: TxnIdAllocator::starting_at(1),
            assets: MemAssets::default(),
        }
    }
}

/// Runs one reconcile cycle (which in v1 PROPOSES — never applies) with the given knobs.
fn cycle(
    w: &mut World,
    rec: &mut Reconciler,
    scanner: &FakeScanner,
    activity: &ScriptedActivity,
    stability: &ScriptedStability,
    style: &IconStyle,
    trust: &TrustState,
    freshness: FreshnessInputs,
) -> ReconcileOutcome {
    let desk = w.desk.clone();
    let reader = DeskReader(&desk);
    let applier = DeskApplier(&desk);
    let ports = ReconcilerPorts {
        scanner,
        extractor: &FakeExtractor,
        reader: &reader,
        applier: &applier,
        assets: &w.assets,
        activity,
        stability,
    };
    let scope_roots = ScopeRoots::resolved(
        vec![PUBLIC_ROOT.to_string()],
        vec!["C:/ProgramData".to_string()],
    )
    .unwrap();
    let ctx = ReconcileContext {
        saved_style: Some(style),
        trust,
        freshness,
        scope: &scope_roots,
    };
    rec.reconcile(&ports, &ctx, &mut w.txn, &mut w.journal, &mut w.ledger).unwrap()
}

/// Applies a vetted candidate batch (the host's confirm/timeout entry) with the given activity.
fn apply(
    w: &mut World,
    rec: &mut Reconciler,
    activity: &ScriptedActivity,
    style: &IconStyle,
    candidates: Vec<VettedCandidate>,
) -> ReconcileOutcome {
    let desk = w.desk.clone();
    let reader = DeskReader(&desk);
    let applier = DeskApplier(&desk);
    let ports = ReconcilerPorts {
        scanner: &FakeScanner(Vec::new()), // apply_batch never scans
        extractor: &FakeExtractor,
        reader: &reader,
        applier: &applier,
        assets: &w.assets,
        activity,
        stability: &ScriptedStability::default(),
    };
    let scope_roots = ScopeRoots::resolved(
        vec![PUBLIC_ROOT.to_string()],
        vec!["C:/ProgramData".to_string()],
    )
    .unwrap();
    let ctx = ReconcileContext {
        saved_style: Some(style),
        trust: &silent_trust(),
        freshness: fresh(NOW),
        scope: &scope_roots,
    };
    rec.apply_batch(&ports, &ctx, candidates, &mut w.txn, &mut w.journal, &mut w.ledger).unwrap()
}

/// The propose→apply round-trip: cycle to a proposal, then apply the whole batch (idle).
fn cycle_then_apply(
    w: &mut World,
    rec: &mut Reconciler,
    scanner: &FakeScanner,
    stability: &ScriptedStability,
    style: &IconStyle,
) -> ReconcileOutcome {
    let proposed = cycle(w, rec, scanner, &ScriptedActivity::idle(), stability, style, &silent_trust(), fresh(NOW)).proposed;
    apply(w, rec, &ScriptedActivity::idle(), style, proposed)
}

// ---- Gates -----------------------------------------------------------------------------------

#[test]
fn an_empty_saved_style_keeps_the_resident_dormant() {
    let items = vec![user_item("a")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let desk = w.desk.clone();
    let reader = DeskReader(&desk);
    let applier = DeskApplier(&desk);
    let ports = ReconcilerPorts {
        scanner: &FakeScanner(items.clone()),
        extractor: &FakeExtractor,
        reader: &reader,
        applier: &applier,
        assets: &w.assets,
        activity: &ScriptedActivity::idle(),
        stability: &ScriptedStability::default(),
    };
    let scope_roots = ScopeRoots::Unprivileged;
    let ctx = ReconcileContext {
        saved_style: None,
        trust: &silent_trust(),
        freshness: fresh(NOW),
        scope: &scope_roots,
    };
    let out = rec.reconcile(&ports, &ctx, &mut w.txn, &mut w.journal, &mut w.ledger).unwrap();
    assert_eq!(out, ReconcileOutcome::default(), "② empty → nothing proposed, nothing applied");
}

#[test]
fn a_new_item_defers_until_stable_then_proposes_and_apply_writes_only_store_one() {
    let items = vec![user_item("fresh")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();
    let trust = silent_trust();

    // Cycle 1: first sight — the settle gate holds it back (proposes nothing).
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    assert_eq!(out.deferred_unstable, vec![ItemId::from_raw("fresh")]);
    assert!(out.applied.is_empty() && out.proposed.is_empty());

    // Cycle 2: settled → PROPOSED (v1 never auto-applies), store ① untouched by the proposal.
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    assert_eq!(out.proposed.len(), 1, "the settled newcomer is proposed");
    assert_eq!(out.proposed[0].item.id, ItemId::from_raw("fresh"));
    assert!(out.applied.is_empty() && w.ledger.all().unwrap().is_empty());

    // The host confirms (or the 2h timeout fires) → apply writes ONLY store ① via the driver.
    let applied = apply(&mut w, &mut rec, &ScriptedActivity::idle(), &style(), out.proposed);
    assert_eq!(applied.applied, vec![ItemId::from_raw("fresh")]);
    let entry = w.ledger.get(&ItemId::from_raw("fresh")).unwrap().expect("ledger row");
    assert!(entry.state.is_committed());
    assert!(w.desk.surfaces.borrow()[&items[0].path].starts_with(b"styled:"), "surface styled");

    // A later cycle: owned + unmodified → self-write suppression, nothing re-proposed.
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    assert!(out.proposed.is_empty() && out.conflicts.is_empty());
}

#[test]
fn reconcile_always_proposes_regardless_of_the_trust_tier() {
    // codex m7b-🟠1: v1 NEVER auto-applies; the trust counter only affects the HOST's toast, not
    // whether the batch applies. So an "earned" tier proposes exactly like an unearned one.
    let items = vec![user_item("new1"), user_item("new2")];
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();

    for trust in [TrustState::default(), silent_trust()] {
        let mut rec = Reconciler::new();
        let mut w = World::new(&items);
        cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
        let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
        assert_eq!(out.proposed.len(), 2, "both tiers propose the batch");
        assert!(out.applied.is_empty(), "reconcile never applies in v1");
        assert!(w.ledger.all().unwrap().is_empty(), "store ① untouched by a proposal");
    }
}

#[test]
fn apply_uses_the_propose_time_snapshot_so_a_hand_edit_since_propose_is_not_overwritten() {
    // codex m7a-🔴1: a hand-edit in the window between propose and confirm/timeout must survive —
    // the snapshot fingerprint is the CAS anchor, so the driver skips the changed item.
    let items = vec![user_item("edited"), user_item("kept")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
    assert_eq!(out.proposed.len(), 2);

    // The user customizes `edited` AFTER the proposal but BEFORE the apply.
    w.desk.surfaces.borrow_mut().insert(items[0].path.clone(), b"user-custom-icon".to_vec());
    let applied = apply(&mut w, &mut rec, &ScriptedActivity::idle(), &style(), out.proposed);
    assert!(applied.conflicts.contains(&ItemId::from_raw("edited")), "the since-propose edit fails CAS");
    assert_eq!(w.desk.surfaces.borrow()[&items[0].path], b"user-custom-icon", "hand-edit preserved");
    assert!(applied.applied.contains(&ItemId::from_raw("kept")), "the unchanged item still applies");
}

#[test]
fn a_busy_desktop_defers_the_wave_and_a_mid_batch_busy_aborts_the_whole_apply() {
    let items = vec![user_item("a"), user_item("b"), user_item("c")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();

    // Wave-start busy: nothing classified/proposed, nothing lost.
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::script(&[true]), &stable, &style(), &silent_trust(), fresh(NOW));
    assert!(out.deferred_busy && out.proposed.is_empty());

    // Settle + propose (idle).
    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
    let proposed = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW)).proposed;
    assert_eq!(proposed.len(), 3);

    // Apply where the desktop goes busy after the first per-icon check: the WHOLE batch aborts —
    // NOTHING is written while the user is active (codex m7a-🟠4), not a partial apply.
    let out = apply(&mut w, &mut rec, &ScriptedActivity::script(&[false, true]), &style(), proposed.clone());
    assert!(out.deferred_busy);
    assert!(out.applied.is_empty(), "no writes land during activity — the whole batch aborts");
    assert!(w.ledger.all().unwrap().is_empty());

    // Idle → the same batch applies in full.
    let out = apply(&mut w, &mut rec, &ScriptedActivity::idle(), &style(), proposed);
    assert_eq!(out.applied.len(), 3, "the deferred batch applies once idle");
}

#[test]
fn an_externally_modified_owned_item_is_flagged_and_never_touched() {
    let items = vec![user_item("mine")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
    cycle_then_apply(&mut w, &mut rec, &scanner, &stable, &style());
    assert!(w.ledger.get(&ItemId::from_raw("mine")).unwrap().is_some());

    // The user hand-edits the styled icon.
    w.desk.surfaces.borrow_mut().insert(items[0].path.clone(), b"hand-edited".to_vec());
    let before = w.desk.surfaces.borrow()[&items[0].path].clone();
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
    assert_eq!(out.conflicts, vec![ItemId::from_raw("mine")]);
    assert_eq!(w.desk.surfaces.borrow()[&items[0].path], before, "user/installer wins — untouched");
}

#[test]
fn a_proposal_goes_stale_if_a_ledger_row_appears_before_apply() {
    // codex r1-🟠1: the proposal was for a FRESH (un-ledgered) item. If something styles it (a
    // committed row appears) between propose and apply, the old proposal must NOT apply on top —
    // it is skipped as a conflict, not silently re-styled via the ledger's last_applied CAS.
    let items = vec![user_item("x")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
    let proposed = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW)).proposed;
    assert_eq!(proposed.len(), 1);

    // Something styles `x` first (a committed row appears) — e.g. a prior apply_batch or the
    // foreground. Apply the SAME batch: it must be a conflict, not a re-style.
    apply(&mut w, &mut rec, &ScriptedActivity::idle(), &style(), proposed.clone());
    let styled_once = w.desk.surfaces.borrow()[&items[0].path].clone();
    let out = apply(&mut w, &mut rec, &ScriptedActivity::idle(), &style(), proposed);
    assert!(out.conflicts.contains(&ItemId::from_raw("x")), "the stale proposal is a conflict");
    assert!(out.applied.is_empty(), "the stale proposal applies nothing");
    assert_eq!(w.desk.surfaces.borrow()[&items[0].path], styled_once, "no re-style on top");
}

#[test]
fn a_busy_starting_during_the_last_bake_still_aborts_before_the_write() {
    // codex r1-🟠2: the final fail-closed activity check before the driver apply — a busy that
    // begins during the LAST candidate's extract/bake (after the per-item check) must still abort.
    let items = vec![user_item("only")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
    let proposed = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW)).proposed;
    assert_eq!(proposed.len(), 1);

    // Per-item check (call 1) = idle → the item bakes; the FINAL pre-write check (call 2) = busy →
    // abort. Nothing is written.
    let out = apply(&mut w, &mut rec, &ScriptedActivity::script(&[false, true]), &style(), proposed);
    assert!(out.deferred_busy && out.applied.is_empty(), "the final gate aborts the write");
    assert!(w.ledger.all().unwrap().is_empty(), "nothing landed");
}

#[test]
fn a_manually_restored_item_is_silently_skipped_not_flagged_forever() {
    // codex m7a-🟡7: an owned item the user manually restored to its exact original (the
    // current==original poison tuple) is ALREADY at its original — the reconciler silently skips
    // it, never level-triggering a permanent conflict flag, never healing-then-restyling (ABA).
    let items = vec![user_item("restored")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
    cycle_then_apply(&mut w, &mut rec, &scanner, &stable, &style());

    // The user manually restores the icon to its exact original (outside the app).
    w.desk.surfaces.borrow_mut().insert(items[0].path.clone(), b"original:restored".to_vec());
    for _ in 0..3 {
        let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
        assert!(out.conflicts.is_empty(), "the poison tuple is silently skipped, never re-flagged");
        assert!(out.proposed.is_empty(), "never restyled over the manual restore (no ABA)");
        assert_eq!(w.desk.surfaces.borrow()[&items[0].path], b"original:restored", "left at the user's original");
    }
}

#[test]
fn kind_policy_opt_out_and_unstyleable_items_never_enter_a_batch() {
    let mut folder = item("docs", ItemKind::Folder, "C:/Users/Dev/Desktop/docs");
    folder.state = ItemState::Ready;
    let mut broken = user_item("broken");
    broken.state = ItemState::Error;
    let items = vec![folder, broken, user_item("app")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();
    let style = IconStyle::from_value(json!({
        "config": {
            "shape": "Circle", "subject": "Original", "tint": "#FF6F5E",
            "monoStyle": "Tonal", "plateBand": "Vivid", "shortcutShape": null,
            "distinction": "None", "markStyle": "Glass", "markColor": null,
            "size": "Mid", "filter": "None", "plateColor": null, "plateFallback": "derived"
        },
        "kindPolicy": { "App": true, "Folder": false, "File": true, "System": true },
        "typeOverrides": {}
    }))
    .unwrap();

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style, &silent_trust(), fresh(NOW));
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style, &silent_trust(), fresh(NOW));
    let proposed: Vec<_> = out.proposed.iter().map(|c| c.item.id.clone()).collect();
    assert_eq!(proposed, vec![ItemId::from_raw("app")], "only the participating styleable item");
    let applied = apply(&mut w, &mut rec, &ScriptedActivity::idle(), &style, out.proposed);
    assert_eq!(applied.applied, vec![ItemId::from_raw("app")]);
    assert!(w.ledger.get(&ItemId::from_raw("docs")).unwrap().is_none(), "opted-out bucket untouched");
    assert!(w.ledger.get(&ItemId::from_raw("broken")).unwrap().is_none(), "unstyleable untouched");
}

/// The §14 release-gating red line (T12): a privileged-scope item is enqueued BEFORE any write
/// path — never proposed, never applied. The deeper guarantee is structural: this crate has no
/// OverlayControl/dm-elevated dependency, so an elevation call from the reconciler cannot even
/// be expressed; this test pins the queue routing half — at BOTH classify and apply_batch (the
/// apply-time re-check, codex m7a-🔴2).
#[test]
fn the_privileged_red_line_public_desktop_items_are_enqueued_never_applied() {
    let public = item("tool", ItemKind::Shortcut, "C:/Users/Public/Desktop/Tool.lnk");
    let mine = user_item("mine");
    let items = vec![public.clone(), mine];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &silent_trust(), fresh(NOW));
    assert_eq!(out.pending_privileged, 1, "the tray count line sees the queued item");
    let proposed: Vec<_> = out.proposed.iter().map(|c| c.item.id.clone()).collect();
    assert_eq!(proposed, vec![ItemId::from_raw("mine")], "the public item is never proposed");

    // Even if a caller smuggles the public item into apply_batch (a stale proposal), the apply-time
    // scope re-check routes it to the queue, never to the applier.
    let smuggled = vec![VettedCandidate {
        item: public.clone(),
        fingerprint: dm_domain::Fingerprint::of_bytes(b"original:tool"),
    }];
    let applied = apply(&mut w, &mut rec, &ScriptedActivity::idle(), &style(), smuggled);
    assert!(applied.applied.is_empty(), "the apply-time scope gate refuses the privileged item");
    let touched = w.desk.applied_paths.borrow();
    assert!(
        touched.iter().all(|p| !p.starts_with("C:/Users/Public")),
        "the applier NEVER received a privileged path: {touched:?}"
    );
    assert!(w.ledger.get(&ItemId::from_raw("tool")).unwrap().is_none());
    let drained = rec.pending_privileged.drain_for_elevation();
    assert_eq!(drained.len(), 1, "the one batched-UAC drain hands the item over");
}

#[test]
fn unstable_items_keep_retrying_until_their_bytes_settle() {
    let items = vec![user_item("dl")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let moving = ScriptedStability::unstable(&[&items[0].path]);

    for _ in 0..3 {
        let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &moving, &style(), &silent_trust(), fresh(NOW));
        assert_eq!(out.deferred_unstable, vec![ItemId::from_raw("dl")], "still writing → deferred");
        assert!(out.proposed.is_empty());
    }
    // The write finishes: two quiet cycles later it is proposed, then applies.
    moving.unstable.borrow_mut().clear();
    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &moving, &style(), &silent_trust(), fresh(NOW));
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &moving, &style(), &silent_trust(), fresh(NOW));
    assert_eq!(out.proposed.len(), 1);
    let applied = apply(&mut w, &mut rec, &ScriptedActivity::idle(), &style(), out.proposed);
    assert_eq!(applied.applied, vec![ItemId::from_raw("dl")]);
}

#[test]
fn a_prior_crash_is_recovered_unconditionally_every_cycle_even_with_no_new_candidates() {
    // codex m7a-🟠3: a crash-left journal must reconcile on the NEXT cycle regardless of whether a
    // new item happens by. Here an INTERRUPTED txn (Prepared + Applied, no Commit) mutated the
    // desktop; recovery aborts it (restores) and the reconcile stands the cycle down rather than
    // proposing/applying on top of a just-recovered desktop.
    use dm_operations::{JournalRecord, JournalSink};
    let items = vec![user_item("a")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    // The desktop was mutated (styled) by the interrupted txn.
    w.desk.surfaces.borrow_mut().insert(items[0].path.clone(), b"styled:crash".to_vec());
    let orig = Fingerprint::of_bytes(b"original:a");
    w.journal.append(&JournalRecord::TxnBegin { txn: 1, items: vec![ItemId::from_raw("a")] }).unwrap();
    w.journal
        .append(&JournalRecord::ItemPrepared {
            txn: 1,
            item: ItemId::from_raw("a"),
            target: items[0].target(),
            anchor: RestoreAnchor::FileBytes { bytes: b"original:a".to_vec() },
            original_fingerprint: orig.clone(),
            expected_fingerprint: orig,
            asset_hash: "h".into(),
            owned: dm_domain::OwnedFields::icon_only(),
            pinned_seed: None,
        })
        .unwrap();
    w.journal
        .append(&JournalRecord::ItemApplied {
            txn: 1,
            item: ItemId::from_raw("a"),
            new_fingerprint: Fingerprint::of_bytes(b"styled:crash"),
        })
        .unwrap();
    // No TxnCommitted → interrupted → recovery restores.

    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &ScriptedStability::default(), &style(), &silent_trust(), fresh(NOW));
    assert!(out.deferred_recovery, "an unrecovered crash defers the cycle as a re-sync, not busy");
    assert!(!out.deferred_busy, "recovery re-sync is distinct from activity (codex r1-🟡3)");
    assert!(!out.errors.is_empty(), "the recovery is surfaced");
    // Recovery ran unconditionally: the interrupted mutation was walked back to the original.
    assert_eq!(w.desk.surfaces.borrow()[&items[0].path], b"original:a", "recovery restored the desktop");
}
