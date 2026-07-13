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
use crate::icons::style_resolve::StyleRecipe;
use crate::icons::{package_masters, BufferedMaster};
use crate::ledger::{LedgerStore, LookHistoryStore};
use crate::settings_store::SettingsStore;
use crate::txn::{recover_from_journal, ApplyOutcome, ApplyRequest, JournalSink, TxnDriver, TxnIdAllocator};

/// The platform ports a version switch drives — the same set the foreground apply uses.
pub struct VersionSwitchPorts<'a> {
    pub scanner: &'a dyn DesktopScanner,
    pub extractor: &'a dyn IconSourceExtractor,
    pub reader: &'a dyn ItemStateReader,
    pub applier: &'a dyn IconApplier,
    pub assets: &'a dyn AssetStore,
}

/// Switches the live desktop to a saved appearance version. Returns the driver's [`ApplyOutcome`]
/// (committed / conflicts / rolled_back) plus a `deferred` flag when a prior crash's recovery ran
/// and this switch stood down for the next pass. The look-history entry's recipe becomes ② in the
/// same call (spec §8.4). A missing version id is an error (the UI must not offer a stale id).
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

    // Reconcile any prior crash BEFORE anything — a recovery that moved or could not verify the
    // desktop defers this switch, and crucially BEFORE ② is promoted so a deferred switch does not
    // leave ② pointing at a style the desktop never adopted (codex-guard: ②/desktop consistency).
    let recovery = recover_from_journal(journal, ports.reader, ports.applier, ledger)?;
    if !recovery.degraded.is_empty() || !recovery.aborted.is_empty() {
        return Ok(SwitchOutcome { outcome: ApplyOutcome::default(), deferred: true });
    }

    // ② becomes the new current global style (spec §8.3: switching sets ③'s recipe as ②). A
    // one-shot switch creates its own RenderSession (no cross-call warm cache to preserve).
    settings.set_saved_style(Some(&style))?;
    let mut session = RenderSession::new();
    let items = ports.scanner.scan().map_err(|e| OperationError::InvalidPayload(e.to_string()))?;
    let outcome = bake_and_apply(&items, &recipe, ports, &mut session, txn, journal, ledger)?;
    Ok(SwitchOutcome { outcome, deferred: false })
}

/// The result of a version switch.
#[derive(Debug, Clone)]
pub struct SwitchOutcome {
    pub outcome: ApplyOutcome,
    /// A prior crash's recovery ran; the switch stood down for the next pass (nothing applied).
    pub deferred: bool,
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
) -> Result<ApplyOutcome> {
    let mut masters: Vec<BufferedMaster> = Vec::new();
    let mut anchors: Vec<(DesktopItem, dm_domain::Fingerprint)> = Vec::new();
    for item in items {
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
            Err(_) => continue, // vanished/unreadable → not projected this switch
        };
        let sources = match ports.extractor.extract(item, None) {
            Ok(s) if !s.is_empty() => s,
            _ => continue,
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
                Err(_) => {
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
    let packaged = package_masters(&masters)?;
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
    TxnDriver::new(ports.reader, ports.applier, ports.assets).apply(txn.next_id(), requests, journal, ledger)
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
        let ports = VersionSwitchPorts {
            scanner: &FakeScanner(items.clone()),
            extractor: &FakeExtractor,
            reader: &reader,
            applier: &applier,
            assets: &assets,
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
    }

    #[test]
    fn switching_to_an_unknown_version_errors() {
        let dir = tempfile::tempdir().unwrap();
        let desk = Rc::new(Desk::default());
        let reader = DeskReader(desk.clone());
        let applier = DeskApplier(desk.clone());
        let assets = MemAssets::default();
        let ports = VersionSwitchPorts {
            scanner: &FakeScanner(vec![]),
            extractor: &FakeExtractor,
            reader: &reader,
            applier: &applier,
            assets: &assets,
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
