//! Version switching (spec 07 §9) — the native projection of a saved appearance recipe onto the
//! CURRENT desktop. NOT "replay what was recorded": store ③ deliberately holds no icon list
//! (§8.2), so switching means "make the current desktop match the recorded style." It reads ③,
//! promotes the recipe to ② (the new current global style), then drives EVERY current item
//! through the same resolve→bake→`TxnDriver::apply` path auto-format uses — "version switch and
//! incremental auto-format are the same primitive; only which items go into `requests` differs"
//! (spec §9). CAS is the free safety net: an item the user hand-edited since fails
//! `prepare_item`'s CAS and is skipped, exactly as in a normal apply.
//!
//! Native (webview-less) so the resident/tray can invoke it; the foreground webview reaches the
//! same end state through its existing apply pipeline.

use dm_domain::{
    AssetStore, DesktopItem, DesktopScanner, IconApplier, IconSourceExtractor, ItemStateReader,
    OwnedFields,
};
use dm_icon_core::render_session::RenderSession;

use crate::error::{OperationError, Result};
use crate::icons::native_bake::bake_master_png;
use crate::icons::scope;
use crate::icons::style_resolve::StyleRecipe;
use crate::icons::{package_masters, BufferedMaster};
use crate::ledger::{LedgerStore, LookHistoryStore};
use crate::settings_store::SettingsStore;
use crate::txn::{recover_from_journal, ApplyOutcome, ApplyRequest, JournalSink, TxnDriver, TxnIdAllocator};

/// The platform ports a version switch drives — the same set the foreground apply uses. The
/// privileged roots (resolved by the host via `SHGetKnownFolderPath`) let the projection SKIP
/// `Public Desktop`/`ProgramData` items (spec §6: version switching never touches them, §14 the
/// background never elevates — codex m7a-🔴2).
///
/// ⚠️ **[WINDOWS-VERIFY] fail-closed root resolution (codex r2-🔴):** `Public Desktop` and
/// `ProgramData` ALWAYS exist on Windows, so the host MUST resolve non-empty roots there. Empty
/// roots mean "no privileged scope" — correct on the dev host (which has no such items) but a
/// §14 FAIL-OPEN if a real Windows `SHGetKnownFolderPath` failed. The host must therefore refuse
/// to run version-switch / the reconciler with empty roots WHEN ON WINDOWS (fail closed), never
/// silently proceed. (Recovery in this function reconciles only OUR OWN journal, which never
/// contains a privileged target — we never style one — so recovery cannot restore a privileged
/// item we styled; the fail-open risk is purely the projection loop, gated by these roots.)
pub struct VersionSwitchPorts<'a> {
    pub scanner: &'a dyn DesktopScanner,
    pub extractor: &'a dyn IconSourceExtractor,
    pub reader: &'a dyn ItemStateReader,
    pub applier: &'a dyn IconApplier,
    pub assets: &'a dyn AssetStore,
    /// The privileged-scope roots (spec §6/§14). `Unresolved` on Windows before known-folder
    /// resolution makes the projection loop FAIL CLOSED (styles nothing); `Unprivileged` on the dev
    /// host excludes nothing. The type makes "Windows with empty roots" (fail-open) unrepresentable.
    pub scope: &'a scope::ScopeRoots,
}

/// Switches the live desktop to a saved appearance version. A missing version id is an error (the
/// UI must not offer a stale id). Ordering closes the ②-consistency + partial-commit gaps (codex
/// m7a-🟠5): (1) recover any prior crash and (2) scan — both BEFORE ② is promoted — so a deferred
/// switch never leaves ② pointing at a style the desktop never adopted; then (3) promote ② and
/// project. A projection fault AFTER promotion returns STRUCTURED (`promoted:true` + `errors`),
/// never a bare `Err` that hides that ② already moved.
pub fn switch_to_version(
    version_id: &str,
    ports: &VersionSwitchPorts<'_>,
    settings: &SettingsStore,
    history: &LookHistoryStore,
    txn: &mut TxnIdAllocator,
    journal: &mut dyn JournalSink,
    ledger: &mut dyn LedgerStore,
) -> Result<SwitchOutcome> {
    let version = history
        .all()
        .into_iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| OperationError::InvalidPayload(format!("unknown appearance version {version_id:?}")))?;
    let style = version.icon_style;
    let recipe = StyleRecipe::parse(&style)?;

    // (1) Recover any prior crash BEFORE promoting ② — a recovery that moved or could not verify
    // the desktop defers this switch WITHOUT touching ② (②/desktop consistency).
    let recovery = recover_from_journal(journal, ports.reader, ports.applier, ledger)?;
    if !recovery.degraded.is_empty() || recovery.moved_or_uncertain() {
        return Ok(SwitchOutcome { outcome: ApplyOutcome::default(), promoted: false, deferred: true, errors: Vec::new() });
    }
    // (2) Scan — a pure read, no persistent side effect — before promotion.
    let items = ports.scanner.scan().map_err(|e| OperationError::InvalidPayload(e.to_string()))?;

    // (3) Promote ② (spec §8.3: switching sets ③'s recipe as ②), then project. A one-shot switch
    // creates its own RenderSession (no cross-call warm cache to preserve).
    settings.set_saved_style(Some(&style))?;
    let mut session = RenderSession::new();
    let mut errors: Vec<String> = Vec::new();
    let outcome =
        match bake_and_apply(&items, &recipe, ports, &mut session, txn, journal, ledger, &mut errors) {
            Ok(o) => o,
            // A projection fault AFTER ② moved: surface it structured, never a bare Err (the
            // caller would otherwise see "failed" while ② is already the new style).
            Err(e) => {
                errors.push(format!("projection: {e}"));
                ApplyOutcome::default()
            }
        };
    Ok(SwitchOutcome { outcome, promoted: true, deferred: false, errors })
}

/// The result of a version switch.
#[derive(Debug, Clone)]
pub struct SwitchOutcome {
    pub outcome: ApplyOutcome,
    /// Whether ② was promoted to the switched recipe (false only when a prior-crash recovery
    /// deferred the switch before promotion).
    pub promoted: bool,
    /// A prior crash's recovery ran; the switch stood down for the next pass (② NOT promoted).
    pub deferred: bool,
    /// Per-item / projection faults (extract/bake/package/driver) — the switch still promoted ②;
    /// these tell the caller the projection was partial so it can surface a degraded result.
    pub errors: Vec<String>,
}

/// Resolve → extract → native-bake → package → `TxnDriver::apply` over `items` under `recipe`.
/// The shared primitive of the version switch and the resident's batch apply (spec §9: same
/// primitive). Non-participating (kind-policy opt-out / unstyleable / extract-fault) items are
/// silently skipped from the request set — the driver's CAS then skips any hand-edited item too.
fn bake_and_apply(
    items: &[DesktopItem],
    recipe: &StyleRecipe,
    ports: &VersionSwitchPorts<'_>,
    session: &mut RenderSession,
    txn: &mut TxnIdAllocator,
    journal: &mut dyn JournalSink,
    ledger: &mut dyn LedgerStore,
    errors: &mut Vec<String>,
) -> Result<ApplyOutcome> {
    let mut masters: Vec<BufferedMaster> = Vec::new();
    let mut anchors: Vec<(DesktopItem, dm_domain::Fingerprint)> = Vec::new();
    for item in items {
        // §6/§14: version switching never touches Public Desktop / ProgramData (codex m7a-🔴2).
        // An `Unresolved` scope classifies EVERY item privileged, so a Windows host that has not
        // resolved its known folders styles nothing here rather than failing open (codex F-scope).
        if ports.scope.classify(&item.path).is_some() {
            continue;
        }
        if !item.can_style() {
            continue;
        }
        let cfg = match recipe.effective_config(item.kind, item.kind.is_shortcut())? {
            Some(c) => c,
            None => continue, // kind-policy opt-out → keep original
        };
        // CAS anchor = the fingerprint read NOW; the driver skips an item hand-edited since.
        let fingerprint = match ports.reader.read_fingerprint(&item.target()) {
            Ok(f) => f,
            Err(e) => {
                errors.push(format!("read {}: {e}", item.id.as_str()));
                continue;
            }
        };
        // Extract from the TRUE ORIGINAL, not the live surface: a version switch operates on
        // already-STYLED items, so the live icon is our current look — baking the new style over it
        // would compound `Style(Style(original))` (codex System-review 🟠). Pass the ledger's pinned
        // original anchor so extraction resolves the user's real source; an un-ledgered (fresh) item
        // has no anchor → `None` → the live surface IS its original (unlike the resident's batch,
        // which is already guarded to fresh items, this path routinely re-styles owned ones).
        let original = ledger.get(&item.id)?.map(|e| e.original_anchor);
        let sources = match ports.extractor.extract(item, original.as_ref()) {
            Ok(s) if !s.is_empty() => s,
            Ok(_) => {
                errors.push(format!("extract {}: no sources", item.id.as_str()));
                continue;
            }
            Err(e) => {
                errors.push(format!("extract {}: {e}", item.id.as_str()));
                continue;
            }
        };
        let mut ok = true;
        let mut item_masters = Vec::with_capacity(sources.len());
        for (slot, src) in sources.iter().enumerate() {
            let bake_id = if slot == 0 {
                item.id.as_str().to_string()
            } else {
                format!("{}#{slot}", item.id.as_str())
            };
            match bake_master_png(session, &bake_id, &src.png, &cfg, item.kind.is_shortcut(), None) {
                Ok(png) => item_masters.push(BufferedMaster {
                    item_id: item.id.as_str().to_string(),
                    source_index: slot as u32,
                    png_base64: png,
                }),
                Err(e) => {
                    errors.push(format!("bake {}: {e}", item.id.as_str()));
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            masters.extend(item_masters);
            anchors.push((item.clone(), fingerprint));
        }
    }
    if anchors.is_empty() {
        return Ok(ApplyOutcome::default());
    }
    // Distinguish a PRE-apply package fault from a driver fault (codex r2-🟠). A package error is
    // a pure pre-mutation failure (nothing touched the desktop) → surface it, empty outcome. The
    // driver returns Ok-with-error for a rollback (its `ApplyOutcome` carries committed/rolled_back
    // + `error`), and a bare `Err` only on a durable journal fault — either way we return the
    // outcome (or record the driver error) so the transaction's real state is never silently
    // folded into "nothing happened".
    let packaged = match package_masters(&masters) {
        Ok(p) => p,
        Err(e) => {
            errors.push(format!("package: {e}"));
            return Ok(ApplyOutcome::default());
        }
    };
    let by_id: std::collections::HashMap<&str, &dm_domain::Fingerprint> =
        anchors.iter().map(|(i, f)| (i.id.as_str(), f)).collect();
    let mut requests = Vec::with_capacity(packaged.len());
    for pkg in &packaged {
        let Some((item, _)) = anchors.iter().find(|(i, _)| i.id.as_str() == pkg.item_id) else {
            continue;
        };
        requests.push(ApplyRequest {
            target: item.target(),
            expected_fingerprint: (*by_id[pkg.item_id.as_str()]).clone(),
            owned: OwnedFields::icon_only(),
            asset_hash: pkg.primary.content_hash.clone(),
            asset_bytes: pkg.primary.bytes.clone(),
            empty_asset_bytes: pkg.empty.as_ref().map(|e| e.bytes.clone()),
            pinned_seed: None,
        });
    }
    match TxnDriver::new(ports.reader, ports.applier, ports.assets).apply(txn.next_id(), requests, journal, ledger) {
        Ok(outcome) => Ok(outcome), // committed/rolled_back/error all carried on the outcome
        Err(e) => {
            // A bare driver Err = a durable journal fault; there is no ApplyOutcome to carry. Record
            // it distinctly (not "package") so the caller knows the desktop MAY have been mutated.
            errors.push(format!("driver: {e}"));
            Ok(ApplyOutcome { desktop_mutated: true, ..Default::default() })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_contracts::IconStyle;
    use dm_domain::{
        ApplyAssets, AssetRef, DecodedImage, Fingerprint, ItemId, ItemKind, ItemState, ItemTarget,
        PortError, PortResult, RestoreAnchor,
    };
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use crate::ledger::{LookHistoryStore, LookVersion, MemLedgerStore};
    use crate::txn::VecJournal;

    #[derive(Default)]
    struct Desk {
        surfaces: RefCell<HashMap<String, Vec<u8>>>,
    }

    struct FakeScanner(Vec<DesktopItem>);
    impl DesktopScanner for FakeScanner {
        fn scan(&self) -> PortResult<Vec<DesktopItem>> {
            Ok(self.0.clone())
        }
    }

    struct FakeExtractor;
    impl IconSourceExtractor for FakeExtractor {
        fn extract(&self, item: &DesktopItem, _: Option<&RestoreAnchor>) -> PortResult<Vec<DecodedImage>> {
            use image::ImageEncoder;
            let tone = item.id.as_str().bytes().fold(0u8, |a, b| a.wrapping_add(b));
            let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([tone, 90, 200, 255]));
            let mut png = Vec::new();
            image::codecs::png::PngEncoder::new(&mut png)
                .write_image(&img, 256, 256, image::ExtendedColorType::Rgba8)
                .unwrap();
            Ok(vec![DecodedImage { width: 256, height: 256, png }])
        }
    }

    struct DeskReader(Rc<Desk>);
    impl ItemStateReader for DeskReader {
        fn read_fingerprint(&self, t: &ItemTarget) -> PortResult<Fingerprint> {
            self.0
                .surfaces
                .borrow()
                .get(&t.path)
                .map(|b| Fingerprint::of_bytes(b))
                .ok_or_else(|| PortError::NotFound(t.path.clone()))
        }
        fn capture_anchor(&self, t: &ItemTarget) -> PortResult<RestoreAnchor> {
            self.0
                .surfaces
                .borrow()
                .get(&t.path)
                .map(|b| RestoreAnchor::FileBytes { bytes: b.clone() })
                .ok_or_else(|| PortError::NotFound(t.path.clone()))
        }
    }

    struct DeskApplier(Rc<Desk>);
    impl IconApplier for DeskApplier {
        fn apply(&self, t: &ItemTarget, a: &ApplyAssets) -> PortResult<Fingerprint> {
            let styled = format!("styled:{}", a.primary.hash).into_bytes();
            self.0.surfaces.borrow_mut().insert(t.path.clone(), styled.clone());
            Ok(Fingerprint::of_bytes(&styled))
        }
        fn restore(&self, t: &ItemTarget, anchor: &RestoreAnchor) -> PortResult<()> {
            if let RestoreAnchor::FileBytes { bytes } = anchor {
                self.0.surfaces.borrow_mut().insert(t.path.clone(), bytes.clone());
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct MemAssets(RefCell<HashMap<String, Vec<u8>>>);
    impl AssetStore for MemAssets {
        fn put(&self, hash: &str, bytes: &[u8]) -> PortResult<AssetRef> {
            self.0.borrow_mut().insert(hash.into(), bytes.to_vec());
            Ok(AssetRef::new(hash, format!("assets/{hash}.ico")))
        }
        fn put_empty_variant(&self, primary: &AssetRef, bytes: &[u8]) -> PortResult<AssetRef> {
            let h = format!("{}-e", primary.hash);
            self.0.borrow_mut().insert(h.clone(), bytes.to_vec());
            Ok(AssetRef::new(&h, format!("assets/{h}.ico")))
        }
        fn exists(&self, a: &AssetRef) -> PortResult<bool> {
            Ok(self.0.borrow().contains_key(&a.hash))
        }
        fn gc(&self, live: &[String]) -> PortResult<()> {
            self.0.borrow_mut().retain(|k, _| live.iter().any(|l| l == k));
            Ok(())
        }
    }

    fn shortcut(id: &str) -> DesktopItem {
        DesktopItem {
            id: ItemId::from_raw(id),
            name: id.into(),
            path: format!("C:/Users/Dev/Desktop/{id}.lnk"),
            kind: ItemKind::Shortcut,
            icon: None,
            state: ItemState::Ready,
            requires_explicit_consent: false,
            status_message: None,
        }
    }

    fn style(shape: &str) -> IconStyle {
        IconStyle::from_value(serde_json::json!({
            "config": { "shape": shape, "subject": "Original", "tint": "#FF6F5E",
                "monoStyle": "Tonal", "plateBand": "Vivid", "shortcutShape": null,
                "distinction": "None", "markStyle": "Glass", "markColor": null, "size": "Mid",
                "filter": "None", "plateColor": null, "plateFallback": "derived" },
            "kindPolicy": {}, "typeOverrides": {}
        }))
        .unwrap()
    }

    #[test]
    fn switching_projects_the_recipe_onto_the_live_scan_and_promotes_it_to_saved_style() {
        let dir = tempfile::tempdir().unwrap();
        let desk = Rc::new(Desk::default());
        let items = vec![shortcut("a"), shortcut("b")];
        for it in &items {
            desk.surfaces.borrow_mut().insert(it.path.clone(), format!("orig:{}", it.id.as_str()).into_bytes());
        }
        let reader = DeskReader(desk.clone());
        let applier = DeskApplier(desk.clone());
        let assets = MemAssets::default();
        let scope_roots = scope::ScopeRoots::Unprivileged;
        let ports = VersionSwitchPorts {
            scanner: &FakeScanner(items.clone()),
            extractor: &FakeExtractor,
            reader: &reader,
            applier: &applier,
            assets: &assets,
            scope: &scope_roots,
        };
        let settings = SettingsStore::open_in_memory().unwrap();
        let mut history = LookHistoryStore::new(dir.path().join("history.json"));
        history
            .push(LookVersion {
                id: "look-1".into(),
                created_at: 1,
                label: Some("圆角".into()),
                pinned: false,
                icon_style: style("Circle"),
            })
            .unwrap();
        let mut txn = TxnIdAllocator::starting_at(1);
        let mut journal = VecJournal::default();
        let mut ledger = MemLedgerStore::default();

        let out = switch_to_version(
            "look-1", &ports, &settings, &history, &mut txn, &mut journal, &mut ledger,
        )
        .unwrap();
        assert!(!out.deferred);
        assert_eq!(out.outcome.committed.len(), 2, "every current item is projected");
        // ② promoted to the switched recipe (spec §8.4).
        assert!(settings.get_saved_style().unwrap().is_some());
        // Both surfaces are now styled.
        for it in &items {
            assert!(desk.surfaces.borrow()[&it.path].starts_with(b"styled:"));
        }

        // A hand-edited item is CAS-skipped on the next switch, exactly like a normal apply.
        desk.surfaces.borrow_mut().insert(items[0].path.clone(), b"hand-edited".to_vec());
        history
            .push(LookVersion {
                id: "look-2".into(),
                created_at: 2,
                label: None,
                pinned: false,
                icon_style: style("Diamond"),
            })
            .unwrap();
        let out2 = switch_to_version(
            "look-2", &ports, &settings, &history, &mut txn, &mut journal, &mut ledger,
        )
        .unwrap();
        assert!(out2.outcome.conflicts.contains(&ItemId::from_raw("a")), "hand-edited item is skipped");
        assert_eq!(desk.surfaces.borrow()[&items[0].path], b"hand-edited", "never overwritten");
        assert!(out2.outcome.committed.contains(&ItemId::from_raw("b")), "the intact item re-projects");
        assert!(out2.promoted, "the switch promoted ②");
    }

    #[test]
    fn a_public_desktop_item_is_never_projected_by_a_version_switch() {
        // codex m7a-🔴2 / spec §6: version switching never touches Public Desktop / ProgramData.
        let dir = tempfile::tempdir().unwrap();
        let desk = Rc::new(Desk::default());
        let public = DesktopItem {
            id: ItemId::from_raw("pub"),
            name: "pub".into(),
            path: "C:/Users/Public/Desktop/Tool.lnk".into(),
            kind: ItemKind::Shortcut,
            icon: None,
            state: ItemState::Ready,
            requires_explicit_consent: false,
            status_message: None,
        };
        let mine = shortcut("mine");
        let items = vec![public.clone(), mine.clone()];
        for it in &items {
            desk.surfaces.borrow_mut().insert(it.path.clone(), format!("orig:{}", it.id.as_str()).into_bytes());
        }
        let reader = DeskReader(desk.clone());
        let applier = DeskApplier(desk.clone());
        let assets = MemAssets::default();
        let scope_roots = scope::ScopeRoots::resolved(
            vec!["C:/Users/Public/Desktop".to_string()],
            vec!["C:/ProgramData".to_string()],
        )
        .unwrap();
        let ports = VersionSwitchPorts {
            scanner: &FakeScanner(items.clone()),
            extractor: &FakeExtractor,
            reader: &reader,
            applier: &applier,
            assets: &assets,
            scope: &scope_roots,
        };
        let settings = SettingsStore::open_in_memory().unwrap();
        let mut history = LookHistoryStore::new(dir.path().join("h.json"));
        history
            .push(LookVersion { id: "v".into(), created_at: 1, label: None, pinned: false, icon_style: style("Circle") })
            .unwrap();
        let mut txn = TxnIdAllocator::starting_at(1);
        let mut journal = VecJournal::default();
        let mut ledger = MemLedgerStore::default();

        let out = switch_to_version("v", &ports, &settings, &history, &mut txn, &mut journal, &mut ledger).unwrap();
        assert_eq!(out.outcome.committed, vec![ItemId::from_raw("mine")], "only the user's own item");
        assert!(!desk.surfaces.borrow()[&public.path].starts_with(b"styled:"), "public item untouched");
    }

    #[test]
    fn switching_to_an_unknown_version_errors() {
        let dir = tempfile::tempdir().unwrap();
        let desk = Rc::new(Desk::default());
        let reader = DeskReader(desk.clone());
        let applier = DeskApplier(desk.clone());
        let assets = MemAssets::default();
        let scope_roots = scope::ScopeRoots::Unprivileged;
        let ports = VersionSwitchPorts {
            scanner: &FakeScanner(vec![]),
            extractor: &FakeExtractor,
            reader: &reader,
            applier: &applier,
            assets: &assets,
            scope: &scope_roots,
        };
        let settings = SettingsStore::open_in_memory().unwrap();
        let history = LookHistoryStore::new(dir.path().join("h.json"));
        let mut txn = TxnIdAllocator::starting_at(1);
        let mut journal = VecJournal::default();
        let mut ledger = MemLedgerStore::default();
        assert!(switch_to_version(
            "nope", &ports, &settings, &history, &mut txn, &mut journal, &mut ledger
        )
        .is_err());
    }
}
