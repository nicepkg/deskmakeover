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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use dm_contracts::{
    ArrowOverlayDto, GridMetricsDto, IconChunkItemDto, IconItemDto, IconKindDto, IconOpResultDto,
    IconPersistedDto, IconScanDto, IconStyle, LookVersionDto, SettingsPatch, ToastDto,
};
use dm_domain::{
    DecodedImage, DesktopScanner, ExplorerRefresher, IconApplier, IconSourceExtractor, ItemKind,
    ItemStateReader, OverlayControl, OverlayOutcome,
};
use dm_operations::{
    FsAssetStore, IconApplySession, IconOps, IconPlatform, IconStoreState, JsonLedgerStore,
    LookHistoryStore, LookVersion, ScannedItem, SettingsStore, TxnIdAllocator,
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
    /// A monotonic token identifying the CURRENT session. Begin returns it; Chunk + Commit must
    /// present the matching token, so a stale async request (or a second WebView) whose token is
    /// older than the current session is rejected — masters + styleJson can never cross applies
    /// (codex R3-Block 1). 0 means "no session".
    session_id: u64,
    /// The op-epoch captured when the current session began; the commit rejects if the epoch moved
    /// since (any mutation — Apply, Reset, or restoreOverlay — landed during this apply's bake, so
    /// committing would write over the user's newer intent — codex Block 3 / R2-Block 3).
    session_epoch: u64,
    /// The scan snapshot (items + scan-time fingerprints) the CURRENT session was begun against —
    /// captured at Begin so an intervening rescan cannot swap the CAS anchors out from under a
    /// commit (codex R2-Block 1). The commit resolves against THIS, never the live `scan`.
    session_scan: Vec<ScannedItem>,
    /// The last scan's items WITH their scan-time fingerprints — the CAS anchor for a fresh apply is
    /// captured HERE, not re-read at commit (codex Block 2). Copied into `session_scan` at Begin.
    scan: Vec<ScannedItem>,
    /// The revision of the last scan; a Begin whose revision differs is a stale apply and is rejected.
    scan_revision: u32,
    /// Monotonic epoch bumped by EVERY user mutation (a committed Apply, a full Reset, AND a
    /// restoreOverlay) so an in-flight apply that began before any of them rejects at commit.
    op_epoch: u64,
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
    /// Serializes an ENTIRE desktop-mutating verb — apply-commit, full restore, and arrow restore —
    /// including its overlay call + `set_arrow`, which run OUTSIDE `mut_state` (a slow elevated
    /// round-trip must not hold the transaction lock that scans need). Without this gate two verbs'
    /// overlay helpers could interleave and leave marker ≠ desktop ≠ UI (codex R3-Block 2). Reads
    /// (scan / get_persisted) never take it, so a mutation-in-flight never blocks a refresh. Lock
    /// order is ALWAYS op_gate → mut_state; no reader ever takes op_gate, so there is no cycle.
    op_gate: Mutex<()>,
    /// The `dmicon://` protocol cache: `"<itemId>/<slot>"` → the extracted PNG bytes (overwritten
    /// each scan; the URL's `?rev` — not this map — cache-busts the webview).
    sources: Mutex<HashMap<String, Vec<u8>>>,
    /// Monotonic revision: bumps each scan, cache-busting every source URL.
    revision: AtomicU32,
    /// The native shortcut-arrow overlay state (ADR-0021), persisted to `arrow_marker` so it
    /// survives a restart (codex Block 5 — the overlay is machine-wide + outlives the process).
    arrow_overlay: Mutex<ArrowOverlayDto>,
    /// The file the arrow state is persisted to (a tiny marker, no schema migration needed).
    arrow_marker: PathBuf,
    /// The materialized transparent overlay ICO the elevated helper points the registry at (the
    /// helper rejects an empty/absent path — codex Block 5).
    overlay_ico: PathBuf,
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
        // Materialize the transparent overlay ICO once (best-effort) so the elevated helper always
        // has a real path to validate + copy into ProgramData (codex Block 5).
        let overlay_ico = data_dir.join("overlay-transparent.ico");
        let _ = std::fs::write(&overlay_ico, dm_icon_codec::transparent_ico().bytes);
        // Resume the persisted arrow state (default Native) — the overlay is machine-wide + survives
        // process restarts, so a fresh process must not forget an installed overlay.
        let arrow_marker = data_dir.join("arrow-overlay.txt");
        let arrow = match std::fs::read_to_string(&arrow_marker).ok().as_deref().map(str::trim) {
            Some("hidden") => ArrowOverlayDto::Hidden,
            _ => ArrowOverlayDto::Native,
        };
        Self {
            scanner: ports.scanner,
            extractor: ports.extractor,
            reader: ports.reader,
            applier: ports.applier,
            overlay: ports.overlay,
            refresher: ports.refresher,
            assets: FsAssetStore::new(data_dir.join("icon-assets")),
            settings,
            op_gate: Mutex::new(()),
            mut_state: Mutex::new(IconMutState {
                ledger: JsonLedgerStore::new(data_dir.join("ledger.json")),
                journal: FileJournal::new(data_dir.join("txn.log")),
                history: LookHistoryStore::new(data_dir.join("look-history.json")),
                txn: TxnIdAllocator::starting_at(1),
                session: None,
                session_id: 0,
                session_epoch: 0,
                session_scan: Vec::new(),
                scan: Vec::new(),
                scan_revision: 0,
                op_epoch: 0,
            }),
            sources: Mutex::new(HashMap::new()),
            revision: AtomicU32::new(0),
            arrow_overlay: Mutex::new(arrow),
            arrow_marker,
            overlay_ico,
            active_user_profiles,
        }
    }

    /// Sets + persists the arrow-overlay state (survives a restart; codex Block 5). Best-effort
    /// persistence — a failed write leaves the in-memory truth authoritative for this session.
    fn set_arrow(&self, arrow: ArrowOverlayDto) {
        *self.arrow_overlay.lock().unwrap() = arrow;
        let text = match arrow {
            ArrowOverlayDto::Native => "native",
            ArrowOverlayDto::Hidden => "hidden",
        };
        let _ = std::fs::write(&self.arrow_marker, text);
    }

    /// `icons.scan`: enumerate + classify + extract 256px sources (served over `dmicon://`) into raw
    /// items. NO embedded state (D1: the frontend assembles it).
    pub fn scan(&self) -> Result<IconScanDto, String> {
        let items = self.scanner.scan().map_err(|e| e.to_string())?;
        let rev = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        // Build the new content-addressed source cache in a LOCAL map, then atomically swap it in only
        // after EVERY extract succeeds (codex R2-Major 3): a mid-scan extract failure must not leave
        // the previous scan's still-displayed URLs 404-ing against a half-cleared cache.
        let mut next_sources: HashMap<String, Vec<u8>> = HashMap::new();
        let mut dtos = Vec::with_capacity(items.len());
        let mut scanned = Vec::with_capacity(items.len());
        for (i, item) in items.into_iter().enumerate() {
            let sources = self.extractor.extract(&item).map_err(|e| e.to_string())?;
            let mut urls = Vec::with_capacity(sources.len());
            for (slot, src) in sources.iter().enumerate() {
                urls.push(cache_source_into(&mut next_sources, item.id.as_str(), slot as u32, src));
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
            // Capture the CAS anchor AT SCAN TIME (codex Block 2): a hand-edit during the bake then
            // fails the driver's CAS instead of being silently overwritten. An unreadable surface
            // gets a sentinel fingerprint, so a fresh apply of it conflicts (skipped), never forced.
            let fingerprint = self
                .reader
                .read_fingerprint(&item.target())
                .unwrap_or_else(|_| dm_domain::Fingerprint::of_bytes(b""));
            scanned.push(ScannedItem { item, fingerprint });
        }
        // Every extract succeeded → atomically publish the new cache + scan snapshot.
        *self.sources.lock().unwrap() = next_sources;
        {
            let mut st = self.mut_state.lock().unwrap();
            st.scan = scanned;
            st.scan_revision = rev;
        }
        // Observed desktop metrics (the frontend assembles its grid from these, never fabricated
        // dims). [WINDOWS-VERIFY] real SPI_GETWORKAREA + shell icon metrics; the dev host reports a
        // plausible 1080p work area matching `synthetic_layout`.
        let grid = GridMetricsDto { screen_width: 1920, screen_height: 1080, taskbar_height: 48 };
        Ok(IconScanDto { revision: rev, items: dtos, grid })
    }

    /// `icons.getPersisted`: the ②③ + native bits the frontend overlays onto its assembled state.
    pub fn get_persisted(&self) -> Result<IconPersistedDto, String> {
        let st = self.mut_state.lock().unwrap();
        let stores =
            self.ops().read_state(&st.history, &st.ledger).map_err(|e| e.to_string())?;
        drop(st);
        Ok(self.finish_persisted(self.to_persisted_dto_locked(&stores)))
    }

    /// `icons.applyBakedBegin`: open a chunk-buffer session for scan `revision`, returning a fresh
    /// session token. Rejects a stale apply whose revision no longer matches the current scan (codex
    /// Block 2), and captures the op-epoch so the commit can detect an intervening mutation.
    pub fn apply_baked_begin(&self, revision: u32, count: u32) -> Result<String, String> {
        let mut st = self.mut_state.lock().unwrap();
        if revision != st.scan_revision {
            return Err(format!(
                "stale apply: begin revision {revision} does not match the current scan {}",
                st.scan_revision
            ));
        }
        // A fresh Begin ABANDONS any prior in-flight session (a bake that errored mid-stream and never
        // committed must not strand the session and deadlock every future apply). Mixing is prevented
        // by the SESSION TOKEN: the new session gets a new monotonic id, so any still-in-flight Chunk/
        // Commit carrying the OLD token is rejected — masters never cross applies (codex R3-Block 1).
        if st.session.is_some() {
            log::warn!("icons.applyBakedBegin abandoned a prior uncommitted apply session");
        }
        st.session_id += 1;
        st.session = Some(IconApplySession::begin(revision, count as usize));
        st.session_epoch = st.op_epoch;
        st.session_scan = st.scan.clone();
        Ok(st.session_id.to_string())
    }

    /// `icons.applyBakedChunk`: buffer a batch of baked masters into the open session, validating the
    /// session token so a stale/foreign chunk can never land in the wrong buffer (codex R3-Block 1).
    pub fn apply_baked_chunk(&self, session_id: &str, items: Vec<IconChunkItemDto>) -> Result<(), String> {
        let mut st = self.mut_state.lock().unwrap();
        if session_id != st.session_id.to_string() {
            return Err("apply session token mismatch (a newer apply superseded this one)".into());
        }
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
        session_id: &str,
        style_json: String,
        restore_ids: Vec<String>,
        label: Option<String>,
    ) -> Result<IconOpResultDto, String> {
        let style = parse_style(&style_json)?;
        // Hold the mutation gate for the WHOLE verb — the ledger commit AND the overlay install +
        // set_arrow below (which run after `mut_state` is dropped) — so no concurrent restore /
        // arrow-restore can interleave its overlay helper with this one (codex R3-Block 2).
        let _op_gate = self.op_gate.lock().unwrap();
        let mut st = self.mut_state.lock().unwrap();
        // Reject a commit whose token no longer matches the current session — a newer Begin
        // superseded it, so its buffer/styleJson belong to a stale apply (codex R3-Block 1).
        if session_id != st.session_id.to_string() {
            let stores = self.ops().read_state(&st.history, &st.ledger).map_err(|e| e.to_string())?;
            let dto = self.to_persisted_dto_locked(&stores);
            drop(st);
            return Ok(IconOpResultDto {
                ok: false,
                toast: Some(ToastDto { key: "Toast_ApplySuperseded".into(), arg: None }),
                persisted: self.finish_persisted(dto),
            });
        }
        let session = st.session.take().ok_or("no apply session to commit")?;
        // Reject a malformed buffer (short/over) — a stale scan or a dropped chunk (codex Block 2).
        if session.len() != session.expected() {
            return Err(format!(
                "incomplete apply buffer: {} masters of {} promised",
                session.len(),
                session.expected()
            ));
        }
        // Reject a SUPERSEDED apply: ANY mutation (Restore, another Apply, or a restoreOverlay)
        // landed during this apply's bake, so committing now would write a stale look OVER the
        // user's newer intent on the real desktop (codex Block 3 / R2-Block 3). Fail closed WITHOUT
        // mutating — the store keeps the draft dirty + rescans.
        if st.op_epoch != st.session_epoch {
            let stores = self.ops().read_state(&st.history, &st.ledger).map_err(|e| e.to_string())?;
            let dto = self.to_persisted_dto_locked(&stores);
            drop(st);
            return Ok(IconOpResultDto {
                ok: false,
                toast: Some(ToastDto { key: "Toast_ApplySuperseded".into(), arg: None }),
                persisted: self.finish_persisted(dto),
            });
        }
        // Resolve against the session's OWN scan snapshot (bound at Begin), never the live `scan`,
        // so an intervening rescan cannot swap the CAS anchors (codex R2-Block 1). Split the guard
        // into disjoint field borrows so the ops call can hold &mut of several at once.
        let IconMutState { ledger, journal, history, txn, session_scan, op_epoch, .. } = &mut *st;
        let look_id = format!("look-{}", txn.peek());
        let created_at = now_secs();
        let outcome = self
            .ops()
            .commit_apply(session, style, label, look_id, created_at, session_scan, &restore_ids, txn, journal, ledger, history)
            .map_err(|e| e.to_string())?;
        // This apply mutated the desktop → bump the epoch so any concurrent stale apply rejects.
        *op_epoch += 1;
        session_scan.clear();
        let committed_any = !outcome.committed.is_empty();
        let dto = self.to_persisted_dto_locked(&outcome.stores);
        drop(st);

        // A global apply that styled at least one icon installs the machine-wide transparent overlay
        // (native arrow hidden, ADR-0021), pointing the elevated helper at a real transparent ICO
        // (the helper rejects an empty path — codex Block 5). The elevated verb is the host's; on the
        // dev host it succeeds. A pure revert-only apply leaves the overlay state untouched.
        if committed_any {
            if let Ok(OverlayOutcome::Applied) = self
                .overlay
                .apply(dm_domain::OverlayStyle::Transparent, &self.overlay_ico.to_string_lossy())
            {
                self.set_arrow(ArrowOverlayDto::Hidden);
            }
            let _ = self.refresher.notify_icons_changed();
        }
        // A finalize step failed AFTER the desktop committed (codex R3-Block 4): log the detail for
        // the operator, surface a generic "applied but finalize incomplete" toast, and return ok:false
        // with the authoritative persisted state — the store then keeps the draft dirty for a retry,
        // never a bare bridge error that reads as "nothing changed".
        if let Some(reason) = &outcome.degraded {
            log::warn!("icons apply finalize degraded: {reason}");
        }
        let (ok, toast) = if let Some(e) = outcome.error {
            (false, Some(ToastDto { key: "Toast_ApplyFailed".into(), arg: Some(e) }))
        } else if outcome.degraded.is_some() {
            (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
        } else {
            (true, None)
        };
        Ok(IconOpResultDto { ok, toast, persisted: self.finish_persisted(dto) })
    }

    /// `icons.restore`: full reset — revert every styled icon to its true original (trust-first,
    /// spec 07 §10) AND lift the arrow overlay (icons + arrow back to native).
    pub fn restore(&self) -> Result<IconOpResultDto, String> {
        // Hold the mutation gate across the ledger reset AND the arrow lift below (codex R3-Block 2).
        let _op_gate = self.op_gate.lock().unwrap();
        let mut st = self.mut_state.lock().unwrap();
        let IconMutState { ledger, history, journal, op_epoch, .. } = &mut *st;
        let outcome = self
            .ops()
            .reset_to_original(journal, ledger, history)
            .map_err(|e| e.to_string())?;
        // A reset is a mutation → bump the epoch so a concurrent in-flight apply rejects.
        *op_epoch += 1;
        let dto = self.to_persisted_dto_locked(&outcome.stores);
        let skipped = outcome.skipped.len();
        let degraded = outcome.degraded;
        drop(st);

        // §10 three-part coupling (spec 07 §8.4): clearing ② (done in the ops) is paired with
        // turning auto-format OFF so the resident stays dormant after a reset.
        let _ = self
            .settings
            .set(&SettingsPatch { keep_new_icons_styled: Some(false), ..Default::default() });
        // Lift the overlay if it was installed. A helper FAILURE is surfaced (codex R2-Block 3): the
        // icons reverted but the machine-wide arrow is still hidden, so the op is NOT a clean success.
        let mut overlay_failed = false;
        if *self.arrow_overlay.lock().unwrap() == ArrowOverlayDto::Hidden {
            match self.overlay.restore() {
                Ok(OverlayOutcome::Applied) => self.set_arrow(ArrowOverlayDto::Native),
                _ => overlay_failed = true,
            }
        }
        let _ = self.refresher.notify_icons_changed();
        // A finalize step failed after some icons already reverted (codex R3-Block 4): log the detail,
        // return ok:false + a repair toast + the authoritative state. Surface the trust-first skips, or
        // the arrow-restore failure — never a blanket ok:true. Priority: arrow fault → finalize
        // degraded → trust-first skips.
        if let Some(reason) = &degraded {
            log::warn!("icons reset finalize degraded: {reason}");
        }
        let (ok, toast) = if overlay_failed {
            (false, Some(ToastDto { key: "Toast_RestoreArrowFailed".into(), arg: None }))
        } else if degraded.is_some() {
            (false, Some(ToastDto { key: "Toast_ResetDegraded".into(), arg: None }))
        } else if skipped > 0 {
            (true, Some(ToastDto { key: "Toast_ResetSkipped".into(), arg: Some(skipped.to_string()) }))
        } else {
            (true, None)
        };
        Ok(IconOpResultDto { ok, toast, persisted: self.finish_persisted(dto) })
    }

    /// `icons.restoreOverlay`: keep-beautification restore — lift ONLY the arrow overlay (the icon
    /// look stays). Faithful to the elevated Applied|Declined|Failed contract; the OBSERVED
    /// post-op arrow state is authoritative.
    pub fn restore_overlay(&self) -> Result<IconOpResultDto, String> {
        // Hold the mutation gate across the overlay helper + epoch bump + set_arrow so a concurrent
        // apply-commit / full-restore can never interleave its own overlay call (codex R3-Block 2).
        let _op_gate = self.op_gate.lock().unwrap();
        let outcome = self.overlay.restore().map_err(|e| e.to_string())?;
        let (arrow, ok, toast_key) = match outcome {
            OverlayOutcome::Applied => (ArrowOverlayDto::Native, true, "Toast_ArrowRestored"),
            // Declined/Failed leave the arrow hidden so the affordance stays for a retry.
            OverlayOutcome::Declined => (ArrowOverlayDto::Hidden, false, "Toast_ArrowRestoreDeclined"),
            OverlayOutcome::Failed => (ArrowOverlayDto::Hidden, false, "Toast_RestoreArrowFailed"),
        };
        // Bump the op-epoch ONLY when the arrow actually flipped to native (a real machine-wide
        // mutation): an in-flight apply that began before it then rejects rather than re-hiding the
        // arrow the user just lifted (codex R2-Block 3). A Declined/Failed changed nothing, so it must
        // NOT invalidate an in-flight apply.
        if outcome == OverlayOutcome::Applied {
            self.mut_state.lock().unwrap().op_epoch += 1;
        }
        self.set_arrow(arrow);
        let persisted = self.get_persisted()?;
        Ok(IconOpResultDto {
            ok,
            toast: Some(ToastDto { key: toast_key.into(), arg: None }),
            persisted,
        })
    }

    /// `icons.exportCompare`: render + save a before/after compare sheet. Not yet implemented — the
    /// sheet compositor is a future deliverable, so this reports `ok:false` with an "unavailable"
    /// toast rather than falsely claiming success with no artifact on disk (codex Major 5).
    pub fn export_compare(&self) -> Result<IconOpResultDto, String> {
        let persisted = self.get_persisted()?;
        Ok(IconOpResultDto {
            ok: false,
            toast: Some(ToastDto { key: "Toast_CompareFailed".into(), arg: None }),
            persisted,
        })
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
fn icon_protocol_url(key: &str) -> String {
    if cfg!(windows) {
        format!("http://dmicon.localhost/{key}")
    } else {
        format!("dmicon://localhost/{key}")
    }
}

/// Inserts an extracted source into `sources` under a CONTENT-ADDRESSED key
/// `"<itemId>/<slot>/<hash>"` and returns its protocol URL. Content-addressing makes the
/// `immutable` Cache-Control header honest (codex Major 4): identical pixels → identical URL (a
/// legitimate cache hit); changed pixels → a new URL (never a stale reuse of a prior process's
/// bytes). Written into a caller-owned local map so the whole cache swaps atomically per scan.
fn cache_source_into(
    sources: &mut HashMap<String, Vec<u8>>,
    item_id: &str,
    slot: u32,
    src: &DecodedImage,
) -> String {
    let hash = &dm_icon_codec::content_hash(&src.png)[..16];
    let key = format!("{item_id}/{slot}/{hash}");
    sources.insert(key.clone(), src.png.clone());
    icon_protocol_url(&key)
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
        // The contract-required 256×256 master size (the host packages exactly this).
        let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([120, 90, 200, 255]));
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&img, 256, 256, image::ExtendedColorType::Rgba8)
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

        let sid = h.apply_baked_begin(scan.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto {
            id: edge.id.clone(),
            source_index: 0,
            master_png: tiny_master(),
        }])
        .unwrap();
        let res = h.apply_baked_commit(&sid, style_json(1), vec![], Some("第一版".into())).unwrap();
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
        let sid = h.apply_baked_begin(scan.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        h.apply_baked_commit(&sid, style_json(1), vec![], Some("A".into())).unwrap();

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
        let sid = h.apply_baked_begin(scan.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        h.apply_baked_commit(&sid, style_json(1), vec![], Some("A".into())).unwrap();

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
        // No Begin ⇒ session token "0"; any presented token mismatches ⇒ rejected.
        assert!(h
            .apply_baked_chunk("1", vec![IconChunkItemDto { id: "edge".into(), source_index: 0, master_png: tiny_master() }])
            .is_err());
    }

    #[test]
    fn a_chunk_or_commit_with_a_stale_token_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();
        // Begin A gets token "1"; Begin B abandons it + gets "2".
        let sid_a = h.apply_baked_begin(scan.revision, 1).unwrap();
        let sid_b = h.apply_baked_begin(scan.revision, 1).unwrap();
        assert_ne!(sid_a, sid_b, "each Begin mints a fresh token");
        // A's stale chunk is rejected; only B's token is live.
        assert!(h
            .apply_baked_chunk(&sid_a, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }])
            .is_err());
        h.apply_baked_chunk(&sid_b, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }])
            .unwrap();
        // A's stale COMMIT is rejected (ok:false), never mutating with B's buffer.
        let stale = h.apply_baked_commit(&sid_a, style_json(1), vec![], Some("A".into())).unwrap();
        assert!(!stale.ok, "a stale-token commit must not succeed");
    }
}
