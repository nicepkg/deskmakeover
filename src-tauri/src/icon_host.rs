//! Icon host glue (M6-WIRE B4): assembles the THIN `icons.*` command results from the ports +
//! ops (D1-thin boundary — Rust scans/packages/applies/restores + persists ②③; the frontend
//! assembles `IconsStateDto` from these thin results + its own presets/palette/grid) and serves
//! extracted 256px sources over the `dmicon://` custom protocol so icon pixels never ride the JSON
//! bridge (the same discipline as wallpaper's `dmwallpaper://`).
//!
//! ALL mutable transaction state (ledger, journal, look-history, txn allocator, the chunk-buffer
//! session, and the last-scan cache) lives under ONE mutex — the B2 apply/GC lifecycle-lock's
//! runtime half — so apply and GC never interleave.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use dm_contracts::{
    ArrowOverlayDto, IconChunkItemDto, IconItemDto, IconKindDto, IconOpResultDto, IconPersistedDto,
    IconScanDto, IconStyle, LookVersionDto, ToastDto,
};
use dm_domain::{
    DecodedImage, DesktopItem, DesktopScanner, ExplorerRefresher, IconApplier, IconSourceExtractor,
    ItemKind, ItemStateReader, OverlayControl, OverlayOutcome,
};
use dm_operations::{
    FsAssetStore, IconApplySession, IconOps, IconPlatform, IconStoreState, JsonLedgerStore,
    LookHistoryStore, LookVersion, SettingsStore, TxnIdAllocator,
};
use dm_operations::txn::FileJournal;

/// The mutable transaction state, all under the host's apply/GC lock.
struct IconMutState {
    ledger: JsonLedgerStore,
    journal: FileJournal,
    history: LookHistoryStore,
    txn: TxnIdAllocator,
    /// The in-flight chunk-buffer session (begin → chunk → commit); `None` between applies.
    session: Option<IconApplySession>,
    /// The last scan's items, kept so a commit resolves master ids → targets against the exact
    /// desktop the user styled (a stale master is skipped, not misapplied).
    scan: Vec<DesktopItem>,
}

/// The platform ports the host drives, bundled for construction.
pub struct IconHostPorts {
    pub scanner: Arc<dyn DesktopScanner + Send + Sync>,
    pub extractor: Arc<dyn IconSourceExtractor + Send + Sync>,
    pub reader: Arc<dyn ItemStateReader + Send + Sync>,
    pub applier: Arc<dyn IconApplier + Send + Sync>,
    pub overlay: Arc<dyn OverlayControl + Send + Sync>,
    pub refresher: Arc<dyn ExplorerRefresher + Send + Sync>,
}

pub struct IconHost {
    scanner: Arc<dyn DesktopScanner + Send + Sync>,
    extractor: Arc<dyn IconSourceExtractor + Send + Sync>,
    reader: Arc<dyn ItemStateReader + Send + Sync>,
    applier: Arc<dyn IconApplier + Send + Sync>,
    overlay: Arc<dyn OverlayControl + Send + Sync>,
    refresher: Arc<dyn ExplorerRefresher + Send + Sync>,
    assets: FsAssetStore,
    settings: Arc<SettingsStore>,
    mut_state: Mutex<IconMutState>,
    /// The `dmicon://` protocol cache: `"<itemId>/<slot>"` → the extracted PNG bytes (overwritten
    /// each scan; the URL's `?rev` — not this map — cache-busts the webview).
    sources: Mutex<HashMap<String, Vec<u8>>>,
    /// Monotonic revision: bumps each scan, cache-busting every source URL.
    revision: AtomicU32,
    /// The native shortcut-arrow overlay state (ADR-0021).
    arrow_overlay: Mutex<ArrowOverlayDto>,
    /// Active user-profile count (host truth; >1 makes the machine-wide arrow disclosure
    /// non-skippable). The dev host reports a single user.
    active_user_profiles: u32,
}

impl IconHost {
    pub fn new(
        ports: IconHostPorts,
        settings: Arc<SettingsStore>,
        data_dir: &Path,
        active_user_profiles: u32,
    ) -> Self {
        Self {
            scanner: ports.scanner,
            extractor: ports.extractor,
            reader: ports.reader,
            applier: ports.applier,
            overlay: ports.overlay,
            refresher: ports.refresher,
            assets: FsAssetStore::new(data_dir.join("icon-assets")),
            settings,
            mut_state: Mutex::new(IconMutState {
                ledger: JsonLedgerStore::new(data_dir.join("ledger.json")),
                journal: FileJournal::new(data_dir.join("txn.log")),
                history: LookHistoryStore::new(data_dir.join("look-history.json")),
                txn: TxnIdAllocator::starting_at(1),
                session: None,
                scan: Vec::new(),
            }),
            sources: Mutex::new(HashMap::new()),
            revision: AtomicU32::new(0),
            arrow_overlay: Mutex::new(ArrowOverlayDto::Native),
            active_user_profiles,
        }
    }

    /// `icons.scan`: enumerate + classify + extract 256px sources (served over `dmicon://`) into raw
    /// items. NO embedded state (D1: the frontend assembles it).
    pub fn scan(&self) -> Result<IconScanDto, String> {
        let items = self.scanner.scan().map_err(|e| e.to_string())?;
        let rev = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        let mut dtos = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            let sources = self.extractor.extract(item).map_err(|e| e.to_string())?;
            let mut urls = Vec::with_capacity(sources.len());
            for (slot, src) in sources.iter().enumerate() {
                urls.push(self.cache_source(item.id.as_str(), slot as u32, src, rev));
            }
            let (x, y) = synthetic_layout(i);
            dtos.push(IconItemDto {
                id: item.id.as_str().into(),
                label: item.name.clone(),
                kind: map_kind(item.kind),
                is_shortcut: item.kind.is_shortcut(),
                styleable: item.can_style(),
                status_reason: item.status_message.clone(),
                x,
                y,
                source_urls: urls,
            });
        }
        self.mut_state.lock().unwrap().scan = items;
        Ok(IconScanDto { revision: rev, items: dtos })
    }

    /// `icons.getPersisted`: the ②③ + native bits the frontend overlays onto its assembled state.
    pub fn get_persisted(&self) -> Result<IconPersistedDto, String> {
        let st = self.mut_state.lock().unwrap();
        let stores =
            self.ops().read_state(&st.history, &st.ledger).map_err(|e| e.to_string())?;
        drop(st);
        Ok(self.finish_persisted(self.to_persisted_dto_locked(&stores)))
    }

    /// `icons.applyBakedBegin`: open a chunk-buffer session for scan `revision`.
    pub fn apply_baked_begin(&self, revision: u32, count: u32) -> Result<(), String> {
        self.mut_state.lock().unwrap().session =
            Some(IconApplySession::begin(revision, count as usize));
        Ok(())
    }

    /// `icons.applyBakedChunk`: buffer a batch of baked masters into the open session.
    pub fn apply_baked_chunk(&self, items: Vec<IconChunkItemDto>) -> Result<(), String> {
        let mut st = self.mut_state.lock().unwrap();
        let session = st
            .session
            .as_mut()
            .ok_or("no apply session; call applyBakedBegin first")?;
        for it in items {
            session.push(it.id, it.source_index, it.master_png);
        }
        Ok(())
    }

    /// `icons.applyBakedCommit`: package + apply the buffered masters, persist ②③, install the
    /// arrow overlay. Serialized under the mut lock (the apply/GC lifecycle-lock).
    pub fn apply_baked_commit(
        &self,
        style_json: String,
        label: Option<String>,
    ) -> Result<IconOpResultDto, String> {
        let style = parse_style(&style_json)?;
        let mut st = self.mut_state.lock().unwrap();
        let session = st.session.take().ok_or("no apply session to commit")?;
        // Split the guard into disjoint field borrows so the ops call can hold &mut of several at once.
        let IconMutState { ledger, journal, history, txn, scan, .. } = &mut *st;
        let look_id = format!("look-{}", txn.peek());
        let created_at = now_secs();
        let outcome = self
            .ops()
            .commit_apply(session, style, label, look_id, created_at, scan, txn, journal, ledger, history)
            .map_err(|e| e.to_string())?;
        let committed_any = !outcome.committed.is_empty();
        let dto = self.to_persisted_dto_locked(&outcome.stores);
        drop(st);

        // A successful global apply installs the machine-wide transparent overlay (native arrow
        // hidden, ADR-0021). The elevated verb is the host's; on the dev host it succeeds.
        if committed_any {
            if let Ok(OverlayOutcome::Applied) =
                self.overlay.apply(dm_domain::OverlayStyle::Transparent, "")
            {
                *self.arrow_overlay.lock().unwrap() = ArrowOverlayDto::Hidden;
            }
            let _ = self.refresher.notify_icons_changed();
        }
        Ok(IconOpResultDto {
            ok: outcome.error.is_none(),
            toast: outcome.error.map(|e| ToastDto { key: "icons.applyPartial".into(), arg: Some(e) }),
            persisted: self.finish_persisted(dto),
        })
    }

    /// `icons.restore`: full reset — revert every styled icon to its true original (trust-first,
    /// spec 07 §10) AND lift the arrow overlay (icons + arrow back to native).
    pub fn restore(&self) -> Result<IconOpResultDto, String> {
        let mut st = self.mut_state.lock().unwrap();
        let IconMutState { ledger, history, .. } = &mut *st;
        let outcome = self
            .ops()
            .reset_to_original(ledger, history)
            .map_err(|e| e.to_string())?;
        let dto = self.to_persisted_dto_locked(&outcome.stores);
        drop(st);

        // Lift the overlay if it was installed (best-effort; the observed arrow state is authority).
        if *self.arrow_overlay.lock().unwrap() == ArrowOverlayDto::Hidden {
            if let Ok(OverlayOutcome::Applied) = self.overlay.restore() {
                *self.arrow_overlay.lock().unwrap() = ArrowOverlayDto::Native;
            }
        }
        let _ = self.refresher.notify_icons_changed();
        Ok(IconOpResultDto { ok: true, toast: None, persisted: self.finish_persisted(dto) })
    }

    /// `icons.restoreOverlay`: keep-beautification restore — lift ONLY the arrow overlay (the icon
    /// look stays). Faithful to the elevated Applied|Declined|Failed contract; the OBSERVED
    /// post-op arrow state is authoritative.
    pub fn restore_overlay(&self) -> Result<IconOpResultDto, String> {
        let outcome = self.overlay.restore().map_err(|e| e.to_string())?;
        let (arrow, ok, toast_key) = match outcome {
            OverlayOutcome::Applied => (ArrowOverlayDto::Native, true, "icons.arrowRestored"),
            // Declined/Failed leave the arrow hidden so the affordance stays for a retry.
            OverlayOutcome::Declined => (ArrowOverlayDto::Hidden, false, "icons.restoreDeclined"),
            OverlayOutcome::Failed => (ArrowOverlayDto::Hidden, false, "icons.restoreFailed"),
        };
        *self.arrow_overlay.lock().unwrap() = arrow;
        let persisted = self.get_persisted()?;
        Ok(IconOpResultDto {
            ok,
            toast: Some(ToastDto { key: toast_key.into(), arg: None }),
            persisted,
        })
    }

    /// `icons.exportCompare`: render a before/after compare sheet. The sheet rendering is a
    /// frontend/future concern; the host op is a no-op that returns the current state.
    pub fn export_compare(&self) -> Result<IconOpResultDto, String> {
        let persisted = self.get_persisted()?;
        Ok(IconOpResultDto { ok: true, toast: None, persisted })
    }

    /// Protocol lookup: the PNG bytes for `dmicon://…/<itemId>/<slot>?rev=N`.
    pub fn png_for(&self, key: &str) -> Option<Vec<u8>> {
        self.sources.lock().unwrap().get(key).cloned()
    }

    fn ops(&self) -> IconOps<'_> {
        IconOps::new(
            IconPlatform::new(&*self.reader, &*self.applier, &self.assets),
            &self.settings,
        )
    }

    /// Caches an extracted source under `"<itemId>/<slot>"` and returns its protocol URL.
    fn cache_source(&self, item_id: &str, slot: u32, src: &DecodedImage, rev: u32) -> String {
        let key = format!("{item_id}/{slot}");
        self.sources.lock().unwrap().insert(key.clone(), src.png.clone());
        icon_protocol_url(&key, rev)
    }

    /// Maps the ops-layer store snapshot to the wire DTO (recipe as opaque JSON string). Safe to
    /// call while the mut lock is held: it does NOT touch the arrow lock — the caller stamps the
    /// live arrow via [`finish_persisted`] after releasing the mut lock.
    fn to_persisted_dto_locked(&self, stores: &IconStoreState) -> IconPersistedDto {
        IconPersistedDto {
            saved_style_json: stores.saved_style.as_ref().map(style_to_json),
            history: stores.history.iter().map(look_to_dto).collect(),
            applied: stores.applied,
            // Placeholder arrow; `finish_persisted` stamps the live value (avoids a nested lock while
            // the mut lock is held).
            arrow_overlay: ArrowOverlayDto::Native,
            active_user_profiles: self.active_user_profiles,
        }
    }

    /// Stamps the live arrow-overlay state onto a persisted DTO built while the mut lock was held.
    fn finish_persisted(&self, mut dto: IconPersistedDto) -> IconPersistedDto {
        dto.arrow_overlay = *self.arrow_overlay.lock().unwrap();
        dto
    }
}

/// Parses + validates the opaque recipe string into an `IconStyle` (rejects a malformed envelope).
fn parse_style(style_json: &str) -> Result<IconStyle, String> {
    serde_json::from_str::<IconStyle>(style_json)
        .map_err(|e| format!("invalid icon style: {e}"))
}

fn style_to_json(style: &IconStyle) -> String {
    serde_json::to_string(style).expect("a validated IconStyle always serializes")
}

fn look_to_dto(v: &LookVersion) -> LookVersionDto {
    LookVersionDto {
        id: v.id.clone(),
        created_at: v.created_at as f64,
        label: v.label.clone(),
        pinned: v.pinned,
        style_json: style_to_json(&v.icon_style),
    }
}

fn map_kind(k: ItemKind) -> IconKindDto {
    match k {
        ItemKind::Shortcut => IconKindDto::Shortcut,
        ItemKind::UrlShortcut => IconKindDto::UrlShortcut,
        ItemKind::AppxShortcut => IconKindDto::AppxShortcut,
        ItemKind::RecycleBin => IconKindDto::RecycleBin,
        ItemKind::Folder => IconKindDto::Folder,
        ItemKind::RegularFile => IconKindDto::RegularFile,
        ItemKind::System => IconKindDto::SystemIcon,
        ItemKind::Unsupported => IconKindDto::Unsupported,
    }
}

/// Synthetic desktop-grid positions for the dev host (a fixed column-major layout). On Windows the
/// real observed `IFolderView2` positions replace these ([WINDOWS-VERIFY]).
fn synthetic_layout(i: usize) -> (i32, i32) {
    const ROWS: usize = 6;
    let col = (i / ROWS) as i32;
    let row = (i % ROWS) as i32;
    (24 + col * 104, 24 + row * 116)
}

/// The platform-correct custom-protocol URL for an icon source (mirrors the wallpaper protocol).
/// [WINDOWS-VERIFY] the `http://dmicon.localhost` WebView2 form on the real box.
fn icon_protocol_url(key: &str, rev: u32) -> String {
    if cfg!(windows) {
        format!("http://dmicon.localhost/{key}?rev={rev}")
    } else {
        format!("dmicon://localhost/{key}?rev={rev}")
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use crate::devhost_icons::{
        DevDesktopScanner, DevExplorerRefresher, DevIconApplier, DevIconDesktop,
        DevIconReader, DevIconSourceExtractor, DevOverlayControl,
    };
    use serde_json::json;

    fn host(dir: &std::path::Path) -> IconHost {
        let desk = DevIconDesktop::new();
        let settings = Arc::new(SettingsStore::open(&dir.join("settings.sqlite3")).unwrap());
        IconHost::new(
            IconHostPorts {
                scanner: Arc::new(DevDesktopScanner),
                extractor: Arc::new(DevIconSourceExtractor),
                reader: Arc::new(DevIconReader(desk.clone())),
                applier: Arc::new(DevIconApplier(desk)),
                overlay: Arc::new(DevOverlayControl),
                refresher: Arc::new(DevExplorerRefresher),
            },
            settings,
            dir,
            1,
        )
    }

    fn style_json(seed: i64) -> String {
        json!({ "config": { "seed": seed }, "kindPolicy": {}, "typeOverrides": {} }).to_string()
    }

    /// Bakes an item by streaming its scanned sources back as chunk masters (a 1×1 stand-in PNG per
    /// slot), mirroring what the frontend does after a scan.
    fn tiny_master() -> String {
        use base64::Engine;
        use image::ImageEncoder;
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([120, 90, 200, 255]));
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&img, 8, 8, image::ExtendedColorType::Rgba8)
            .unwrap();
        base64::engine::general_purpose::STANDARD.encode(png)
    }

    #[test]
    fn scan_serves_every_source_over_the_protocol() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan = h.scan().unwrap();
        assert!(scan.revision >= 1 && !scan.items.is_empty());
        // Every advertised source URL resolves through the protocol handler.
        for item in &scan.items {
            for url in &item.source_urls {
                let key = url.split("localhost/").nth(1).unwrap().split('?').next().unwrap();
                assert!(h.png_for(key).is_some(), "protocol miss for {url}");
            }
        }
        // The Recycle Bin advertises two sources (primary + empty).
        let bin = scan.items.iter().find(|i| i.kind == IconKindDto::RecycleBin).unwrap();
        assert_eq!(bin.source_urls.len(), 2);
    }

    #[test]
    fn apply_then_get_persisted_reads_back_applied_with_saved_style_and_arrow_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();

        h.apply_baked_begin(scan.revision, 1).unwrap();
        h.apply_baked_chunk(vec![IconChunkItemDto {
            id: edge.id.clone(),
            source_index: 0,
            master_png: tiny_master(),
        }])
        .unwrap();
        let res = h.apply_baked_commit(style_json(1), Some("第一版".into())).unwrap();
        assert!(res.ok);
        assert!(res.persisted.applied);
        assert_eq!(res.persisted.arrow_overlay, ArrowOverlayDto::Hidden);
        assert!(res.persisted.saved_style_json.is_some());
        assert_eq!(res.persisted.history.len(), 1);

        // getPersisted reads the same truth on a cold call.
        let p = h.get_persisted().unwrap();
        assert!(p.applied && p.saved_style_json.is_some());
    }

    #[test]
    fn restore_reverts_and_returns_arrow_native_no_saved_style() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();
        h.apply_baked_begin(scan.revision, 1).unwrap();
        h.apply_baked_chunk(vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        h.apply_baked_commit(style_json(1), Some("A".into())).unwrap();

        let res = h.restore().unwrap();
        assert!(res.ok);
        assert!(!res.persisted.applied, "everything reverted");
        assert!(res.persisted.saved_style_json.is_none(), "saved-style cleared");
        assert_eq!(res.persisted.arrow_overlay, ArrowOverlayDto::Native);
    }

    #[test]
    fn restore_overlay_keeps_the_look_and_only_lifts_the_arrow() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();
        h.apply_baked_begin(scan.revision, 1).unwrap();
        h.apply_baked_chunk(vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        h.apply_baked_commit(style_json(1), Some("A".into())).unwrap();

        let res = h.restore_overlay().unwrap();
        assert!(res.ok);
        assert_eq!(res.persisted.arrow_overlay, ArrowOverlayDto::Native, "arrow lifted");
        assert!(res.persisted.applied, "the icon look is UNTOUCHED (keep-beautify)");
        assert!(res.persisted.saved_style_json.is_some());
    }

    #[test]
    fn a_chunk_without_a_session_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        assert!(h
            .apply_baked_chunk(vec![IconChunkItemDto { id: "edge".into(), source_index: 0, master_png: tiny_master() }])
            .is_err());
    }
}
