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
            StabilitySnapshot { size: 100 + *counter, mtime: 1, readable: true }
        } else {
            StabilitySnapshot { size: 100, mtime: 1, readable: true }
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

/// Runs one reconcile cycle with the given policy knobs.
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
    let ctx = ReconcileContext {
        saved_style: Some(style),
        trust,
        freshness,
        public_roots: &[PUBLIC_ROOT.to_string()],
    };
    rec.reconcile(&ports, &ctx, &mut w.txn, &mut w.journal, &mut w.ledger).unwrap()
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
    let ctx = ReconcileContext {
        saved_style: None,
        trust: &silent_trust(),
        freshness: fresh(NOW),
        public_roots: &[],
    };
    let out = rec.reconcile(&ports, &ctx, &mut w.txn, &mut w.journal, &mut w.ledger).unwrap();
    assert_eq!(out, ReconcileOutcome::default(), "② empty → nothing proposed, nothing applied");
}

#[test]
fn a_new_item_defers_until_stable_then_applies_silently_under_the_earned_tier() {
    let items = vec![user_item("fresh")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();
    let trust = silent_trust();

    // Cycle 1: first sight — the settle gate holds it back.
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    assert_eq!(out.deferred_unstable, vec![ItemId::from_raw("fresh")]);
    assert!(out.applied.is_empty() && out.proposed.is_empty());

    // Cycle 2: unchanged bytes — settled → silent apply writes ONLY store ① via the driver.
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    assert_eq!(out.applied, vec![ItemId::from_raw("fresh")]);
    let entry = w.ledger.get(&ItemId::from_raw("fresh")).unwrap().expect("ledger row");
    assert!(entry.state.is_committed());
    assert!(
        w.desk.surfaces.borrow()[&items[0].path].starts_with(b"styled:"),
        "the desktop surface is styled"
    );

    // Cycle 3: owned + unmodified — nothing to do, nothing re-proposed (self-write suppression
    // at the reconcile level: our own output is not a change).
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    assert!(out.applied.is_empty() && out.proposed.is_empty() && out.conflicts.is_empty());
}

#[test]
fn an_unearned_tier_proposes_instead_of_applying() {
    let items = vec![user_item("new1"), user_item("new2")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();
    let trust = TrustState { batches_without_undo: 2 }; // one short of the tier

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    assert_eq!(out.proposed.len(), 2, "the batch surfaces as one proposal");
    assert!(out.applied.is_empty(), "nothing applies without confirm/timeout");
    assert!(w.ledger.all().unwrap().is_empty(), "store ① untouched by a proposal");
}

#[test]
fn freshness_downgrades_a_would_be_silent_batch_to_a_proposal() {
    let items = vec![user_item("stale")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();
    let trust = silent_trust();
    let stale = FreshnessInputs {
        last_apply_at: Some(NOW - 61 * 86_400),
        partial_reversion: false,
        now: NOW,
    };

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, stale);
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, stale);
    assert!(out.freshness_downgraded);
    assert_eq!(out.proposed, vec![ItemId::from_raw("stale")]);
    assert!(out.applied.is_empty());
}

#[test]
fn a_busy_desktop_defers_the_whole_wave_and_a_mid_batch_interruption_stops_the_batch() {
    let items = vec![user_item("a"), user_item("b"), user_item("c")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();
    let trust = silent_trust();

    // Wave-start busy: nothing classified, nothing lost.
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::script(&[true]), &stable, &style(), &trust, fresh(NOW));
    assert!(out.deferred_busy && out.applied.is_empty() && out.proposed.is_empty());

    // Settle pass (idle), then a batch whose activity flips busy after the first icon's check:
    // call 1 = wave gate (idle), calls 2.. = per-icon checks.
    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    let activity = ScriptedActivity::script(&[false, false, true]);
    let out = cycle(&mut w, &mut rec, &scanner, &activity, &stable, &style(), &trust, fresh(NOW));
    assert!(out.deferred_busy, "the interruption is visible");
    assert_eq!(out.applied.len(), 1, "only the icon baked before the interruption applied");

    // The user goes idle → the next cycle picks up the remainder — suppressed, never dropped.
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    assert_eq!(out.applied.len(), 2, "the deferred remainder applies once idle");
}

#[test]
fn an_externally_modified_owned_item_is_flagged_and_never_touched() {
    let items = vec![user_item("mine")];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();
    let trust = silent_trust();

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    assert_eq!(out.applied.len(), 1);

    // The user hand-edits the styled icon.
    w.desk.surfaces.borrow_mut().insert(items[0].path.clone(), b"hand-edited".to_vec());
    let before = w.desk.surfaces.borrow()[&items[0].path].clone();
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    assert_eq!(out.conflicts, vec![ItemId::from_raw("mine")]);
    assert_eq!(w.desk.surfaces.borrow()[&items[0].path], before, "user/installer wins — untouched");
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
    let trust = silent_trust();
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

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style, &trust, fresh(NOW));
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style, &trust, fresh(NOW));
    assert_eq!(out.applied, vec![ItemId::from_raw("app")], "only the participating styleable item");
    assert!(w.ledger.get(&ItemId::from_raw("docs")).unwrap().is_none(), "opted-out bucket untouched");
    assert!(w.ledger.get(&ItemId::from_raw("broken")).unwrap().is_none(), "unstyleable untouched");
}

/// The §14 release-gating red line (T12): a privileged-scope item is enqueued BEFORE any write
/// path — the applier never sees it. The deeper guarantee is structural: this crate has no
/// OverlayControl/dm-elevated dependency, so an elevation call from the reconciler cannot even
/// be expressed; this test pins the queue routing half.
#[test]
fn the_privileged_red_line_public_desktop_items_are_enqueued_never_applied() {
    let public = item("tool", ItemKind::Shortcut, "C:/Users/Public/Desktop/Tool.lnk");
    let mine = user_item("mine");
    let items = vec![public, mine];
    let mut w = World::new(&items);
    let mut rec = Reconciler::new();
    let scanner = FakeScanner(items.clone());
    let stable = ScriptedStability::default();
    let trust = silent_trust();

    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &stable, &style(), &trust, fresh(NOW));

    assert_eq!(out.pending_privileged, 1, "the tray count line sees the queued item");
    assert_eq!(out.applied, vec![ItemId::from_raw("mine")]);
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
    let trust = silent_trust();

    for _ in 0..3 {
        let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &moving, &style(), &trust, fresh(NOW));
        assert_eq!(out.deferred_unstable, vec![ItemId::from_raw("dl")], "still writing → deferred");
        assert!(out.applied.is_empty());
    }
    // The write finishes: two quiet cycles later it formats.
    moving.unstable.borrow_mut().clear();
    cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &moving, &style(), &trust, fresh(NOW));
    let out = cycle(&mut w, &mut rec, &scanner, &ScriptedActivity::idle(), &moving, &style(), &trust, fresh(NOW));
    assert_eq!(out.applied, vec![ItemId::from_raw("dl")]);
}
