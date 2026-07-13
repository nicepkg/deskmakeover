//! Icon host glue (M6-WIRE B4): assembles the THIN `icons.*` command results from the ports +
//! ops (D1-thin boundary — Rust scans/packages/applies/restores + persists ②③; the frontend
//! assembles `IconsStateDto` from these thin results + its own presets/palette/grid) and serves
//! extracted 256px sources over the `dmicon://` custom protocol so icon pixels never ride the JSON
//! bridge (the same discipline as wallpaper's `dmwallpaper://`).
//!
//! ALL mutable transaction state (ledger, journal, look-history, txn allocator, the chunk-buffer
//! session, and the last-scan cache) lives under ONE mutex — the B2 apply/GC lifecycle-lock's
//! runtime half — so apply and GC never interleave.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use dm_contracts::{
    ArrowOverlayDto, GridMetricsDto, IconChunkItemDto, IconItemDto, IconKindDto, IconOpResultDto,
    IconPersistedDto, IconScanDto, IconStyle, LookVersionDto, SettingsPatch, ToastDto,
};
use dm_domain::{
    DecodedImage, DesktopGeometryReader, DesktopScanner, ExplorerRefresher, IconApplier,
    IconSourceExtractor, ItemKind, ItemStateReader, OverlayControl, OverlayOutcome,
};
use dm_operations::{
    FsAssetStore, IconApplySession, IconOps, IconPlatform, IconStoreState, JsonLedgerStore,
    LedgerStore, LookHistoryStore, LookVersion, ScannedItem, SettingsStore, TxnIdAllocator,
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
    /// Whether `scan`/`scan_revision` describe a REAL, still-valid scan an apply may bind. False
    /// before the first scan and after a heal FENCE (whose revision is synthetic). A genuinely EMPTY
    /// desktop scan is VALID (`scan` empty, `scan_valid` true) — an emptiness test cannot express
    /// this, and rejecting it would break the zero-target policy-only Apply (codex R11-#2).
    scan_valid: bool,
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
    pub geometry: Arc<dyn DesktopGeometryReader + Send + Sync>,
}

pub struct IconHost {
    scanner: Arc<dyn DesktopScanner + Send + Sync>,
    extractor: Arc<dyn IconSourceExtractor + Send + Sync>,
    reader: Arc<dyn ItemStateReader + Send + Sync>,
    applier: Arc<dyn IconApplier + Send + Sync>,
    overlay: Arc<dyn OverlayControl + Send + Sync>,
    refresher: Arc<dyn ExplorerRefresher + Send + Sync>,
    geometry: Arc<dyn DesktopGeometryReader + Send + Sync>,
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
    /// The `dmicon://` protocol cache: `"<itemId>/<slot>/<hash>"` → the extracted PNG bytes, kept as
    /// TWO generations so an old frame's in-flight request survives a scan swap (codex R3-Major 5).
    sources: Mutex<SourceCache>,
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
    /// Where `exportCompare` saves when the platform offers no Pictures known-folder.
    export_fallback_dir: PathBuf,
    /// `Public Desktop` known-folder roots (spec §6/§14 privileged-scope exclusion). Empty on the
    /// dev host; [WINDOWS-VERIFY] resolved via `SHGetKnownFolderPath` on the box.
    public_desktop_roots: Vec<String>,
    /// `ProgramData` known-folder roots. Empty on the dev host; [WINDOWS-VERIFY] on the box.
    programdata_roots: Vec<String>,
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
            geometry: ports.geometry,
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
                scan_valid: false,
                op_epoch: 0,
            }),
            sources: Mutex::new(SourceCache::new(SOURCE_CACHE_CAP)),
            revision: AtomicU32::new(0),
            arrow_overlay: Mutex::new(arrow),
            arrow_marker,
            overlay_ico,
            active_user_profiles,
            export_fallback_dir: data_dir.join("exports"),
            // [WINDOWS-VERIFY] resolve the real Public Desktop / ProgramData known folders on the
            // box (SHGetKnownFolderPath FOLDERID_PublicDesktop / FOLDERID_ProgramData). ⚠️ These
            // MUST be non-empty on Windows (both folders always exist) — if resolution FAILS on
            // the box, the wiring must FAIL CLOSED (refuse version-switch / auto-format), never run
            // with empty roots, which would let §14-privileged items be styled (codex r2-🔴). The
            // dev host legitimately has none, so the exclusion is a correct no-op here.
            public_desktop_roots: Vec::new(),
            programdata_roots: Vec::new(),
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
        // Ledger-aware extraction (codex extractor-review 🔴1): for an item WE own whose live
        // surface still equals our last-applied fingerprint, the live icon is this app's styled
        // output — extracting it as "the source" would compound Style(Style(orig)) on every
        // re-scan. Snapshot the committed anchors up front (short lock; extraction below is slow
        // and must not hold `mut_state`). The JOURNAL overlays the ledger (codex icons2-🔴1): a
        // committed txn whose ledger upsert faulted is desktop truth the ledger has not caught up
        // to — its Prepared anchor + Applied fingerprint win over a missing/stale row. An
        // INCOMPLETE txn leaves an item's live provenance unknowable: that item is DEGRADED below
        // (shown from live, never bake-able, never anchor-substituted) until recovery reconciles.
        // The snapshot also pins `op_epoch` so a mutation landing during the slow extraction
        // fails the publish fence instead of publishing just-styled pixels as "the raw source"
        // (codex icons2-🔴2).
        let (anchors, unknown_provenance, epoch_at_snapshot) = {
            use dm_operations::{JournalRecord as JR, JournalSink as _};
            let st = self.mut_state.lock().unwrap();
            let mut anchors: HashMap<String, (dm_domain::Fingerprint, dm_domain::RestoreAnchor)> =
                st.ledger
                    .all()
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .filter(|e| e.state.is_committed())
                    .map(|e| {
                        (
                            e.item.as_str().to_string(),
                            (e.last_applied_fingerprint, e.original_anchor),
                        )
                    })
                    .collect();
            let mut unknown: std::collections::HashSet<String> = std::collections::HashSet::new();
            let records = st.journal.read_all().map_err(|e| e.to_string())?;
            let terminal: HashMap<u64, bool> = records
                .iter()
                .filter_map(|r| match r {
                    JR::TxnCommitted { txn } => Some((*txn, true)),
                    JR::TxnRolledBack { txn } => Some((*txn, false)),
                    _ => None,
                })
                .collect();
            let mut prepared: HashMap<(u64, String), dm_domain::RestoreAnchor> = HashMap::new();
            for r in &records {
                if let JR::ItemPrepared { txn, item, anchor, .. } = r {
                    match terminal.get(txn) {
                        Some(true) => {
                            prepared.insert((*txn, item.as_str().to_string()), anchor.clone());
                        }
                        // Rolled back → the desktop was walked back; ledger/live are authoritative.
                        Some(false) => {}
                        None => {
                            unknown.insert(item.as_str().to_string());
                        }
                    }
                }
            }
            for r in &records {
                if let JR::ItemApplied { txn, item, new_fingerprint } = r {
                    if let Some(anchor) = prepared.get(&(*txn, item.as_str().to_string())) {
                        anchors.insert(
                            item.as_str().to_string(),
                            (new_fingerprint.clone(), anchor.clone()),
                        );
                    }
                }
            }
            (anchors, unknown, st.op_epoch)
        };
        // Build the new content-addressed source cache in a LOCAL map, then atomically swap it in
        // after extraction (codex R2-Major 3): a failed refresh must not leave the previous scan's
        // still-displayed URLs 404-ing against a half-cleared cache. One bad item does NOT fail the
        // whole scan (codex extractor-review 🟠3): it degrades to styleable:false with a reason —
        // one OneDrive placeholder must not blank a 40-icon desktop.
        // Live positions (technique A) matched BY NAME, the oracle's own matching rule; an
        // unreadable layout (headless session, denied QI) or an unmatched item degrades to the
        // synthetic grid slot — positions are a mirror nicety, never fatal.
        let live_slots: HashMap<String, (i32, i32)> = self
            .geometry
            .positions()
            .unwrap_or_default()
            .into_iter()
            .map(|s| (s.name, (s.x, s.y)))
            .collect();
        let mut next_sources: HashMap<String, Vec<u8>> = HashMap::new();
        let mut dtos = Vec::with_capacity(items.len());
        let mut scanned = Vec::with_capacity(items.len());
        for (i, item) in items.into_iter().enumerate() {
            // Capture the CAS anchor AT SCAN TIME (codex Block 2): a hand-edit during the bake then
            // fails the driver's CAS instead of being silently overwritten. Read BEFORE
            // extraction: the fingerprint decides whether the live surface is our own styled
            // output (→ extract from the original anchor instead). An unreadable surface keeps a
            // sentinel fingerprint for display purposes but is stripped of APPLY AUTHORITY
            // (`source_ok:false` → the commit refuses it; codex icons2-🟠5 — the sentinel is a
            // legal CAS value for empty bytes, so it must never carry authority on its own).
            let read = self.reader.read_fingerprint(&item.target());
            let unreadable = read.is_err();
            let fingerprint =
                read.unwrap_or_else(|_| dm_domain::Fingerprint::of_bytes(b""));
            // Journal-incomplete items have unknowable live provenance — never anchor-substitute,
            // never offer for styling; show the live pixels with an honest reason.
            let provenance_unknown = unknown_provenance.contains(item.id.as_str());
            let original = if provenance_unknown || unreadable {
                None
            } else {
                anchors
                    .get(item.id.as_str())
                    .filter(|(last_applied, _)| last_applied == &fingerprint)
                    .map(|(_, anchor)| anchor)
            };
            let (urls, extract_err) = match self.extractor.extract(&item, original) {
                Ok(sources) => {
                    let mut urls = Vec::with_capacity(sources.len());
                    for (slot, src) in sources.iter().enumerate() {
                        urls.push(cache_source_into(
                            &mut next_sources,
                            item.id.as_str(),
                            slot as u32,
                            src,
                        ));
                    }
                    (urls, None)
                }
                Err(e) => (Vec::new(), Some(format!("图标读取失败：{e}"))),
            };
            let degraded_reason = if provenance_unknown {
                Some("待修复：上次操作未完成，刷新后重试".to_string())
            } else if unreadable {
                Some("图标状态读取失败".to_string())
            } else {
                extract_err
            };
            let (x, y) = live_slots.get(&item.name).copied().unwrap_or_else(|| synthetic_layout(i));
            dtos.push(IconItemDto {
                id: item.id.as_str().into(),
                label: item.name.clone(),
                kind: map_kind(item.kind),
                is_shortcut: item.kind.is_shortcut(),
                styleable: item.can_style() && degraded_reason.is_none(),
                status_reason: degraded_reason.clone().or_else(|| item.status_message.clone()),
                x,
                y,
                source_urls: urls,
            });
            // `source_ok` is the ONE apply-authority bit shared with the commit path (codex
            // icons2-🟠5): the DTO's styleable, the commit's acceptance, and the restore planner
            // all derive from it instead of three drifting definitions.
            scanned.push(ScannedItem { item, fingerprint, source_ok: degraded_reason.is_none() });
        }
        // Every extract succeeded → publish atomically, ALL inside one critical section ordered
        // acceptance-check → source cache → snapshot (codex R10-#B + R11-#1). The revision check runs
        // FIRST: this scan allocated its revision BEFORE the (slow) extraction and does not hold the
        // op gate, so a heal FENCE (or a competing scan) may have advanced `scan_revision` past it. A
        // superseded scan must lose WITHOUT side effects — publishing its cache before erroring could
        // evict URLs the live generation still serves (the failed-refresh contract promises "当前画面
        // 保持不变"). Lock order st → sources appears only here and nothing locks sources → st, so no
        // cycle.
        {
            let mut st = self.mut_state.lock().unwrap();
            if rev <= st.scan_revision {
                return Err(format!(
                    "scan superseded (revision {rev} <= current {}): rescan",
                    st.scan_revision
                ));
            }
            // Epoch fence (codex icons2-🔴2): a desktop mutation (apply-commit / restore /
            // overlay) that landed between the anchor snapshot and here means the extraction ran
            // against a MIXED generation — its output could publish just-styled pixels as "the
            // raw source". Lose without side effects; the caller rescans against the new epoch.
            if st.op_epoch != epoch_at_snapshot {
                return Err(format!(
                    "scan raced a desktop mutation (epoch {} -> {}): rescan",
                    epoch_at_snapshot, st.op_epoch
                ));
            }
            self.sources.lock().unwrap().publish(next_sources);
            st.scan = scanned;
            st.scan_revision = rev;
            // A REAL scan (even of a genuinely empty desktop) is a valid apply target (codex R11-#2).
            st.scan_valid = true;
        }
        // Observed desktop metrics (the frontend assembles its grid from these, never fabricated
        // dims). [WINDOWS-VERIFY] real SM_C*SCREEN + SPI_GETWORKAREA; the dev host reports a
        // plausible 1080p work area matching `synthetic_layout`; an unreadable platform degrades
        // to the same shape rather than failing the scan.
        let grid = self
            .geometry
            .geometry()
            .map(|g| GridMetricsDto {
                screen_width: g.screen_width,
                screen_height: g.screen_height,
                taskbar_height: g.taskbar_height,
            })
            .unwrap_or(GridMetricsDto { screen_width: 1920, screen_height: 1080, taskbar_height: 48 });
        Ok(IconScanDto { revision: rev, items: dtos, grid })
    }

    /// `icons.getPersisted`: the ②③ + native bits the frontend overlays onto its assembled state.
    /// `read_state` folds the repair-pending signal into `applied` (codex R6-#6), so a styled desktop
    /// a degraded recovery left un-ledgered keeps its restore affordance reachable here AND on every
    /// apply/reset op-result — the signal lives in ONE place, the shared `read_state`.
    pub fn get_persisted(&self) -> Result<IconPersistedDto, String> {
        let st = self.mut_state.lock().unwrap();
        let stores = self
            .ops()
            .read_state(&st.history, &st.ledger, &st.journal)
            .map_err(|e| e.to_string())?;
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
        // No valid scan snapshot: either nothing has been scanned yet, or a heal FENCED the previous
        // one (the fenced revision is synthetic — codex R10-#B). An apply must bind a REAL,
        // still-valid snapshot's fingerprints; rescan first. Validity is an EXPLICIT flag, not an
        // emptiness test: a genuinely empty desktop scan is valid, and its zero-target policy-only
        // Apply must go through (codex R11-#2).
        if !st.scan_valid {
            return Err("no valid scan to apply against: rescan first".into());
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
            let stores = self.ops().read_state(&st.history, &st.ledger, &st.journal).map_err(|e| e.to_string())?;
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
            let stores = self.ops().read_state(&st.history, &st.ledger, &st.journal).map_err(|e| e.to_string())?;
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
        // FENCE the scan revision when the ops demand it (codex R9-#1): a stale poison row was healed
        // (dropped) this round — or a driver bare-Err left the heal set unknown — so a same-revision
        // retry would find no ledger row and pass the ordinary fresh CAS, silently overwriting what
        // could be the user's manual restore-to-original (the ABA). Advancing `scan_revision` off the
        // shared counter makes every applyBakedBegin carrying the old revision fail "stale apply"
        // until a REAL rescan publishes fresh fingerprints; the frontend's rescan-after-conflict UX is
        // a follow-up, but this fence is the structural safety boundary, not the toast.
        if outcome.requires_rescan {
            st.scan_revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
            // The fenced revision is synthetic — it corresponds to NO real scan. Mark the snapshot
            // INVALID (an explicit flag, not an emptiness sentinel — codex R11-#2) so even a Begin
            // somehow carrying the fenced number cannot bind pre-heal fingerprints; clear the stale
            // items too. Only a REAL rescan (which republishes both) reopens the gate (codex R10-#B).
            st.scan_valid = false;
            st.scan.clear();
        }
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
        let (ok, toast) = if let Some(e) = &outcome.error {
            if outcome.reverted.is_empty() && !outcome.desktop_mutated {
                // The styling batch failed BEFORE touching the desktop AND no keep-revert landed →
                // truly nothing changed (a preflight/CAS failure). Only here is "桌面没有改动" honest.
                (false, Some(ToastDto { key: "Toast_ApplyFailed".into(), arg: Some(e.clone()) }))
            } else {
                // The batch rolled back / abandoned AFTER moving the desktop, or keep-reverts already
                // changed it — NOT "nothing changed" (codex R4-Block 1 + R5-#1): partial → ok:false +
                // repair toast + the real state, so the UI never claims the desktop is untouched.
                (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
            }
        } else if outcome.degraded.is_some() {
            (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
        } else if !outcome.intent_persisted {
            // The ops did NOT persist this Apply's intent — with error/degraded already handled above,
            // this is exactly the zero-effect-WITH-conflicts case (all-conflicts, or a restore-only
            // batch whose every opt-out was a hand-edit): nothing landed, something was refused, ②③
            // untouched. Report a no-effect + keep the draft dirty (codex R8-#2/#3, R9-#2). A
            // conflict-free zero-target (policy-only) Apply has `intent_persisted == true` — its ②③
            // WAS written — and correctly falls through to a clean success.
            (false, Some(ToastDto { key: "Toast_ApplyNoEffect".into(), arg: None }))
        } else if !outcome.conflicts.is_empty() {
            // A PARTIAL success: some icons styled/reverted, others conflicted (changed under the user
            // since the scan) or were left as a trust-first hand-edit skip. ②③ WAS written (a real
            // effect landed). Surface the skipped count so the user knows to rescan + retry them — spec
            // 01 requires skipped items be visible, never silently swallowed (codex R8-#3).
            (
                true,
                Some(ToastDto {
                    key: "Toast_ApplySkipped".into(),
                    arg: Some(outcome.conflicts.len().to_string()),
                }),
            )
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
        // The ops DEFERRED the ledger reset because up-front recovery had to heal a prior crash first
        // (codex R6-#4). The reset did NOT run, so its finalizers (auto-format off + arrow lift) MUST
        // be skipped — running them would leave a partial state (arrow native + resident off, yet icons
        // still styled). The user re-syncs from the returned state and retries.
        let deferred = outcome.deferred;
        drop(st);

        let mut overlay_failed = false;
        let mut autoformat_off_failed = false;
        if !deferred {
            // §10 three-part coupling (spec 07 §8.4): clearing ② (done in the ops) is paired with
            // turning auto-format OFF so the resident stays dormant after a reset. A write fault here is
            // NOT swallowed (codex R7-#4): a reset reporting ok:true while `keep_new_icons_styled` is
            // still true would leave the resident re-styling new icons after a "restore to original".
            if self
                .settings
                .set(&SettingsPatch { keep_new_icons_styled: Some(false), ..Default::default() })
                .is_err()
            {
                autoformat_off_failed = true;
            }
            // Lift the overlay if it was installed. A helper FAILURE is surfaced (codex R2-Block 3): the
            // icons reverted but the machine-wide arrow is still hidden, so the op is NOT a clean success.
            if *self.arrow_overlay.lock().unwrap() == ArrowOverlayDto::Hidden {
                match self.overlay.restore() {
                    Ok(OverlayOutcome::Applied) => self.set_arrow(ArrowOverlayDto::Native),
                    _ => overlay_failed = true,
                }
            }
        }
        let _ = self.refresher.notify_icons_changed();
        // A finalize step failed after some icons already reverted (codex R3-Block 4): log the detail,
        // return ok:false + a repair toast + the authoritative state. Surface the trust-first skips, or
        // the arrow-restore failure — never a blanket ok:true. Priority: arrow fault → finalize
        // degraded (incl. an auto-format-off write fault) → trust-first skips.
        if let Some(reason) = &degraded {
            log::warn!("icons reset finalize degraded: {reason}");
        }
        let (ok, toast) = if overlay_failed {
            (false, Some(ToastDto { key: "Toast_RestoreArrowFailed".into(), arg: None }))
        } else if degraded.is_some() || autoformat_off_failed {
            (false, Some(ToastDto { key: "Toast_ResetDegraded".into(), arg: None }))
        } else if skipped > 0 {
            (true, Some(ToastDto { key: "Toast_ResetSkipped".into(), arg: Some(skipped.to_string()) }))
        } else {
            (true, None)
        };
        Ok(IconOpResultDto { ok, toast, persisted: self.finish_persisted(dto) })
    }

    /// `icons.switchVersion`: switch the desktop to a saved appearance version (spec 07 §9). Reads
    /// the ③ entry, promotes its recipe to ②, and projects it onto the LIVE scan through the same
    /// resolve→bake→driver path auto-format uses. CAS-safe (a hand-edited icon is skipped), fenced
    /// (the scan revision + op-epoch bump so an in-flight apply built on the old desktop rejects).
    pub fn switch_version(&self, version_id: &str) -> Result<IconOpResultDto, String> {
        use dm_operations::icons::version_switch::{switch_to_version, VersionSwitchPorts};
        // Serialize the whole desktop-mutating verb (same discipline as apply-commit / restore).
        let _op_gate = self.op_gate.lock().unwrap();
        let mut st = self.mut_state.lock().unwrap();
        let ports = VersionSwitchPorts {
            scanner: &*self.scanner,
            extractor: &*self.extractor,
            reader: &*self.reader,
            applier: &*self.applier,
            assets: &self.assets,
            // [WINDOWS-VERIFY] the real Public Desktop / ProgramData known folders; the dev host
            // has none, so nothing is scope-excluded there.
            public_roots: &self.public_desktop_roots,
            programdata_roots: &self.programdata_roots,
        };
        let IconMutState { ledger, journal, history, txn, op_epoch, .. } = &mut *st;
        let outcome = switch_to_version(
            version_id, &ports, &self.settings, history, txn, journal, ledger,
        )
        .map_err(|e| e.to_string())?;
        // A switch is a desktop mutation: bump the epoch (an in-flight apply rejects) and FENCE the
        // scan (the CAS anchors the old snapshot holds are stale) so the next apply must rescan.
        *op_epoch += 1;
        st.scan_revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        st.scan_valid = false;
        st.scan.clear();
        let stores = self
            .ops()
            .read_state(&st.history, &st.ledger, &st.journal)
            .map_err(|e| e.to_string())?;
        let dto = self.to_persisted_dto_locked(&stores);
        drop(st);
        let _ = self.refresher.notify_icons_changed();

        let (ok, toast) = if outcome.deferred {
            // A prior crash's recovery ran; the switch stood down BEFORE ② was promoted, so
            // nothing changed. The UI re-syncs + retries — honest, never a phantom success.
            (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
        } else if outcome.outcome.error.is_some() {
            (false, Some(ToastDto { key: "Toast_ApplyDegraded".into(), arg: None }))
        } else if !outcome.outcome.conflicts.is_empty() && outcome.outcome.committed.is_empty() {
            (true, Some(ToastDto {
                key: "Toast_ApplySkipped".into(),
                arg: Some(outcome.outcome.conflicts.len().to_string()),
            }))
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
        // Read the authoritative ②③ persisted state BEFORE the machine-level overlay mutation, so a
        // ledger/settings I/O fault fails the op with the desktop UNCHANGED — never a bare Err AFTER
        // the arrow already flipped (codex R4-Block 4). This op only touches the arrow overlay, so
        // the ②③ half of the snapshot is still exact post-op; we overwrite just the arrow field.
        let mut persisted = self.get_persisted()?;
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
        persisted.arrow_overlay = arrow; // the one field this op mutated; ②③ carried from the pre-read
        Ok(IconOpResultDto {
            ok,
            toast: Some(ToastDto { key: toast_key.into(), arg: None }),
            persisted,
        })
    }

    /// `icons.exportCompare`: save the webview-composed before/after sheet. Composition lives in
    /// the frontend (it owns the fonts, the CJK stack, and both image states — oracle
    /// `ComparisonImageExporter`); this side validates the payload IS a decodable PNG and writes
    /// it to the Pictures folder (fallback: the app's own exports dir). Failure stays honest —
    /// `ok:false` + the failed toast, never a phantom success with no artifact on disk.
    pub fn export_compare(
        &self,
        png_base64: &str,
        pictures: Option<PathBuf>,
    ) -> Result<IconOpResultDto, String> {
        let persisted = self.get_persisted()?;
        let saved = (|| -> Result<PathBuf, String> {
            use base64::Engine;
            // Bounded input (codex icons2-🟠8): the sheet is a ~1200x660 PNG — cap the encoded
            // and decoded sizes so a hostile payload cannot balloon memory, and accept ONLY a
            // real PNG (the image crate would happily decode other formats that then land on
            // disk under a lying `.png` name).
            if png_base64.len() > 12_000_000 {
                return Err("compare sheet: payload too large".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(png_base64.trim())
                .map_err(|e| format!("compare sheet: bad base64: {e}"))?;
            if bytes.len() > 9_000_000 {
                return Err("compare sheet: decoded payload too large".into());
            }
            if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
                return Err("compare sheet: not a PNG".into());
            }
            let img = image::load_from_memory(&bytes)
                .map_err(|e| format!("compare sheet: not a decodable image: {e}"))?;
            if (img.width() as u64) * (img.height() as u64) > 16_000_000 {
                return Err("compare sheet: dimensions out of range".into());
            }
            let dir = pictures.unwrap_or_else(|| self.export_fallback_dir.clone());
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            create_export_file(&dir, &utc_stamp(now_secs()), &bytes)
        })();
        Ok(match saved {
            Ok(path) => IconOpResultDto {
                ok: true,
                toast: Some(ToastDto {
                    key: "Toast_CompareSaved".into(),
                    arg: Some(path.display().to_string()),
                }),
                persisted,
            },
            Err(e) => {
                log::warn!("exportCompare failed: {e}");
                IconOpResultDto {
                    ok: false,
                    toast: Some(ToastDto { key: "Toast_CompareFailed".into(), arg: None }),
                    persisted,
                }
            }
        })
    }

    /// Protocol lookup: the PNG bytes for `dmicon://…/<itemId>/<slot>/<hash>`, resolved against the
    /// live generation then the prior one so a swap→adopt handoff never 404s (codex R3-Major 5).
    pub fn png_for(&self, key: &str) -> Option<Vec<u8>> {
        self.sources.lock().unwrap().get(key)
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
/// The byte cap for the `dmicon://` source cache — generous enough to hold several scan generations
/// of a large desktop (256px PNGs are small), bounding memory while covering the handoff window.
const SOURCE_CACHE_CAP: usize = 32 * 1024 * 1024;

/// The `dmicon://` source cache. A scan republishes the freshly-extracted generation, but the OLD
/// webview frame keeps requesting the PREVIOUS scan's content-addressed URLs until the frontend
/// re-renders against the new scan DTO — serving only the live generation would 404 those in-flight
/// requests during the swap→adopt handoff (codex R3-Major 5 / R4-Major 2). A fixed two-generation
/// window could still evict a URL the UI had not yet adopted (a scan whose adopt failed, then another
/// scan). Instead this is a byte-bounded, content-keyed LRU: each scan re-inserts its live keys
/// (refreshing their recency), so an unchanged icon never ages out and a CHANGED icon's superseded
/// key survives several more generations before the cap evicts it — covering the handoff generously
/// without unbounded growth. Content addressing dedups (one entry per unique pixel set).
struct SourceCache {
    map: HashMap<String, Vec<u8>>,
    /// Insertion/refresh order, front = oldest — the LRU eviction queue.
    order: VecDeque<String>,
    bytes: usize,
    cap: usize,
}

impl SourceCache {
    fn new(cap_bytes: usize) -> Self {
        Self { map: HashMap::new(), order: VecDeque::new(), bytes: 0, cap: cap_bytes }
    }

    /// Publishes a freshly-extracted generation: inserts every entry (a re-inserted content key
    /// refreshes its recency, so live icons never evict), then trims the oldest HISTORICAL keys past
    /// the byte cap while PINNING this generation's own keys (codex R5-#7). The scan DTO advertises
    /// every key in `next`, so the webview will request each one — evicting a live key mid-publish
    /// would 404 the current desktop. If this one generation alone exceeds the cap (a very large
    /// desktop of high-entropy icons), the cache is left temporarily over the cap holding the full
    /// working set, rather than dropping a live key: the next scan trims the then-historical excess.
    fn publish(&mut self, next: HashMap<String, Vec<u8>>) {
        let pinned: std::collections::HashSet<String> = next.keys().cloned().collect();
        for (k, v) in next {
            self.insert_raw(k, v);
        }
        self.trim(&pinned);
    }

    /// Inserts/refreshes one entry without trimming (a re-inserted key moves to most-recent).
    fn insert_raw(&mut self, key: String, bytes: Vec<u8>) {
        if let Some(old) = self.map.remove(&key) {
            self.bytes -= old.len();
            self.order.retain(|k| k != &key);
        }
        self.bytes += bytes.len();
        self.order.push_back(key.clone());
        self.map.insert(key, bytes);
    }

    /// Evicts the oldest NON-pinned keys until under the cap. `pinned` (the current generation) is
    /// never evicted; once only pinned keys remain the cache stops trimming even if still over cap.
    fn trim(&mut self, pinned: &std::collections::HashSet<String>) {
        let mut idx = 0;
        while self.bytes > self.cap && idx < self.order.len() {
            if pinned.contains(&self.order[idx]) {
                idx += 1; // a live key — skip it, never evict the generation being served
                continue;
            }
            let key = self.order.remove(idx).expect("index in bounds");
            if let Some(v) = self.map.remove(&key) {
                self.bytes -= v.len();
            }
            // `remove(idx)` shifted the tail down, so the next candidate is again at `idx`.
        }
    }

    /// The bytes for a content-addressed key (read-only; recency is refreshed by re-scan, not by get).
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.map.get(key).cloned()
    }
}

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

/// `YYYYMMDD-HHMMSS` (UTC) for export filenames — Howard Hinnant's civil-from-days algorithm,
/// so the host needs no calendar dependency. UTC (not local) keeps it deterministic; the stamp
/// is a filename, not a displayed date.
fn utc_stamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// Writes the export ATOMICALLY under a non-clobbering name: `DeskMakeover-<stamp>.png`,
/// suffixed `-2`, `-3`, … on collision. `create_new` makes claim + create one syscall — no
/// check-then-write window (codex icons2-🟠9) — and candidate exhaustion FAILS rather than
/// falling back to an overwrite.
fn create_export_file(dir: &Path, stamp: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    use std::io::Write;
    for n in 1..100u32 {
        let path = if n == 1 {
            dir.join(format!("DeskMakeover-{stamp}.png"))
        } else {
            dir.join(format!("DeskMakeover-{stamp}-{n}.png"))
        };
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(bytes).map_err(|e| e.to_string())?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("compare sheet: export name space exhausted for this second".into())
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use crate::devhost_icons::{
        DevDesktopGeometry, DevDesktopScanner, DevExplorerRefresher, DevIconApplier,
        DevIconDesktop, DevIconReader, DevIconSourceExtractor, DevOverlayControl,
    };
    use serde_json::json;

    fn host_with_desk(dir: &std::path::Path) -> (IconHost, Arc<DevIconDesktop>) {
        let desk = DevIconDesktop::new();
        let settings = Arc::new(SettingsStore::open(&dir.join("settings.sqlite3")).unwrap());
        let host = IconHost::new(
            IconHostPorts {
                scanner: Arc::new(DevDesktopScanner),
                extractor: Arc::new(DevIconSourceExtractor(desk.clone())),
                reader: Arc::new(DevIconReader(desk.clone())),
                applier: Arc::new(DevIconApplier(desk.clone())),
                overlay: Arc::new(DevOverlayControl),
                refresher: Arc::new(DevExplorerRefresher),
                geometry: Arc::new(DevDesktopGeometry),
            },
            settings,
            dir,
            1,
        );
        (host, desk)
    }

    fn host(dir: &std::path::Path) -> IconHost {
        host_with_desk(dir).0
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

    /// The `dmicon://` PNG a scan currently serves for an item's slot-0 source.
    fn served_png(h: &IconHost, scan: &IconScanDto, id: &str) -> Vec<u8> {
        let item = scan.items.iter().find(|i| i.id == id).unwrap();
        let key =
            item.source_urls[0].split("localhost/").nth(1).unwrap().split('?').next().unwrap();
        h.png_for(key).expect("protocol serves the advertised source")
    }

    #[test]
    fn a_rescan_of_an_owned_unmodified_item_serves_the_original_source_not_the_styled_output() {
        // codex extractor-review 🔴1: after an apply, the LIVE icon is our styled output. A naive
        // re-scan reads it back as "the source", so the next apply styles the styled image —
        // Style(Style(orig)) compounds forever. The ledger-aware scan must serve the ORIGINAL.
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let scan1 = h.scan().unwrap();
        let original_png = served_png(&h, &scan1, "edge");

        let sid = h.apply_baked_begin(scan1.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto {
            id: "edge".into(),
            source_index: 0,
            master_png: tiny_master(),
        }])
        .unwrap();
        assert!(h.apply_baked_commit(&sid, style_json(1), vec![], None).unwrap().ok);

        // Owned + unmodified → the re-scan extracts from the ledger's original anchor.
        let scan2 = h.scan().unwrap();
        assert_eq!(
            served_png(&h, &scan2, "edge"),
            original_png,
            "the re-scan must serve the true original source, not the styled surface"
        );

        // An EXTERNAL hand-edit breaks ownership → the live (foreign) surface is the honest
        // source again; the anchor must NOT shadow the user's own change.
        let second = dir.path().join("second");
        std::fs::create_dir_all(&second).unwrap();
        let (h2, desk2) = host_with_desk(&second);
        let s1 = h2.scan().unwrap();
        let orig2 = served_png(&h2, &s1, "code");
        let sid2 = h2.apply_baked_begin(s1.revision, 1).unwrap();
        h2.apply_baked_chunk(&sid2, vec![IconChunkItemDto {
            id: "code".into(),
            source_index: 0,
            master_png: tiny_master(),
        }])
        .unwrap();
        assert!(h2.apply_baked_commit(&sid2, style_json(2), vec![], None).unwrap().ok);
        desk2.force_foreign("code");
        let s2 = h2.scan().unwrap();
        assert_ne!(
            served_png(&h2, &s2, "code"),
            orig2,
            "a hand-edited surface no longer matches last_applied → live extraction wins"
        );
    }

    #[test]
    fn export_compare_saves_a_validated_png_and_toasts_the_path() {
        use base64::Engine;
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        let out = dir.path().join("pictures");

        // A real PNG payload saves + toasts its path.
        let res = h.export_compare(&tiny_master(), Some(out.clone())).unwrap();
        assert!(res.ok);
        let arg = res.toast.as_ref().unwrap().arg.clone().unwrap();
        assert!(arg.contains("DeskMakeover-"), "toast carries the saved path: {arg}");
        let saved = std::path::Path::new(&arg);
        assert!(saved.exists(), "the artifact is on disk");

        // A second export in the same second gets a suffixed name, never a clobber.
        let res2 = h.export_compare(&tiny_master(), Some(out.clone())).unwrap();
        let arg2 = res2.toast.as_ref().unwrap().arg.clone().unwrap();
        assert_ne!(arg, arg2, "same-second exports never overwrite");

        // A non-PNG payload (real GIF magic, so it is a plausible image but not our format) must
        // be REJECTED by the magic-byte gate before decode (codex icons2-🟠8: PNG-only, so a
        // non-PNG never lands under our `.png` name).
        let gif = base64::engine::general_purpose::STANDARD.encode(b"GIF89a\x01\x00\x01\x00");
        // Garbage / oversize / wrong-format payloads never land on disk — honest ok:false.
        let oversize = "A".repeat(12_000_001);
        for bad in [
            "not-base64!!!",
            &base64::engine::general_purpose::STANDARD.encode(b"nonsense"),
            &gif,
            &oversize,
        ] {
            let res = h.export_compare(bad, Some(out.clone())).unwrap();
            assert!(!res.ok, "rejected payload stays off disk");
            assert_eq!(res.toast.as_ref().unwrap().key, "Toast_CompareFailed");
        }
        assert_eq!(
            std::fs::read_dir(&out).unwrap().count(),
            2,
            "only the two valid exports exist"
        );
    }

    #[test]
    fn utc_stamps_follow_the_civil_calendar() {
        assert_eq!(utc_stamp(0), "19700101-000000");
        // 2026-07-13 00:00:00 UTC = 1783900800.
        assert_eq!(utc_stamp(1_783_900_800), "20260713-000000");
        // Leap-year day: 2024-02-29 12:34:56 UTC = 1709210096.
        assert_eq!(utc_stamp(1_709_210_096), "20240229-123456");
    }

    /// codex icons2-🔴1: a TxnCommitted whose ledger upsert faulted is desktop truth the ledger
    /// hasn't caught up to. The scan must overlay the JOURNAL's Prepared anchor + Applied
    /// fingerprint onto the (missing) ledger row, so it extracts the ORIGINAL — not the styled
    /// surface the committed txn wrote — instead of compounding Style(Style(orig)).
    #[test]
    fn a_committed_but_unledgered_txn_extracts_the_original_via_the_journal_overlay() {
        use dm_operations::txn::{FileJournal, JournalSink};
        use dm_operations::JournalRecord;

        let dir = tempfile::tempdir().unwrap();
        // Baseline: what the ORIGINAL source renders to for edge.
        let baseline_png = {
            let bdir = dir.path().join("baseline");
            std::fs::create_dir_all(&bdir).unwrap();
            let h = host(&bdir);
            let s = h.scan().unwrap();
            served_png(&h, &s, "edge")
        };

        let (h, desk) = host_with_desk(dir.path());
        // Simulate the committed write landing on the desktop (styled bytes) with NO ledger row.
        desk.force_foreign("edge");
        let live_styled = b"styled:foreign-hand-edit:edge".to_vec();
        let new_fp = dm_domain::Fingerprint::of_bytes(&live_styled);
        let original = b"original:edge".to_vec();
        let orig_fp = dm_domain::Fingerprint::of_bytes(&original);
        let target = dm_domain::ItemTarget::new(
            dm_domain::ItemId::from_raw("edge"),
            dm_domain::ItemKind::Shortcut,
            "C:/Users/Dev/Desktop/edge",
        );
        {
            let mut j = FileJournal::new(dir.path().join("txn.log"));
            j.append(&JournalRecord::TxnBegin { txn: 1, items: vec![target.id.clone()] }).unwrap();
            j.append(&JournalRecord::ItemPrepared {
                txn: 1,
                item: target.id.clone(),
                target: target.clone(),
                anchor: dm_domain::RestoreAnchor::FileBytes { bytes: original.clone() },
                original_fingerprint: orig_fp.clone(),
                expected_fingerprint: orig_fp,
                asset_hash: "deadbeef".into(),
                owned: dm_domain::OwnedFields::icon_only(),
                pinned_seed: None,
            })
            .unwrap();
            j.append(&JournalRecord::ItemApplied {
                txn: 1,
                item: target.id.clone(),
                new_fingerprint: new_fp,
            })
            .unwrap();
            j.append(&JournalRecord::TxnCommitted { txn: 1 }).unwrap();
        }

        let scan = h.scan().unwrap();
        // The overlay recovered the original anchor → the served source is the ORIGINAL, not the
        // styled live surface.
        assert_eq!(
            served_png(&h, &scan, "edge"),
            baseline_png,
            "the journal overlay extracts the true original, not Style(orig)"
        );
    }

    /// codex icons2-🔴1 (the incomplete case): a Prepared item with NO terminal record has
    /// unknowable live provenance. The scan must NEVER anchor-substitute or offer it for styling
    /// — it degrades until recovery reconciles it.
    #[test]
    fn an_incomplete_journal_item_degrades_and_is_not_styleable() {
        use dm_operations::txn::{FileJournal, JournalSink};
        use dm_operations::JournalRecord;

        let dir = tempfile::tempdir().unwrap();
        let (h, _desk) = host_with_desk(dir.path());
        let target = dm_domain::ItemTarget::new(
            dm_domain::ItemId::from_raw("edge"),
            dm_domain::ItemKind::Shortcut,
            "C:/Users/Dev/Desktop/edge",
        );
        let orig_fp = dm_domain::Fingerprint::of_bytes(b"original:edge");
        {
            let mut j = FileJournal::new(dir.path().join("txn.log"));
            j.append(&JournalRecord::TxnBegin { txn: 1, items: vec![target.id.clone()] }).unwrap();
            j.append(&JournalRecord::ItemPrepared {
                txn: 1,
                item: target.id.clone(),
                target: target.clone(),
                anchor: dm_domain::RestoreAnchor::FileBytes { bytes: b"original:edge".to_vec() },
                original_fingerprint: orig_fp.clone(),
                expected_fingerprint: orig_fp,
                asset_hash: "deadbeef".into(),
                owned: dm_domain::OwnedFields::icon_only(),
                pinned_seed: None,
            })
            .unwrap();
            // No ItemApplied, no terminal record — an interrupted txn.
        }
        let scan = h.scan().unwrap();
        let edge = scan.items.iter().find(|i| i.id == "edge").unwrap();
        assert!(!edge.styleable, "unknown provenance is never offered for styling");
        assert!(
            edge.status_reason.as_deref().unwrap_or("").contains("待修复"),
            "the degradation reason is honest: {:?}",
            edge.status_reason
        );
    }

    #[test]
    fn one_failing_extract_degrades_that_item_instead_of_failing_the_whole_scan() {
        // codex extractor-review 🟠3: one unreadable icon (OneDrive placeholder, vanished file)
        // must not blank the whole desktop scan — it degrades to styleable:false with a reason.
        struct OneBadExtractor(Arc<DevIconDesktop>);
        impl dm_domain::IconSourceExtractor for OneBadExtractor {
            fn extract(
                &self,
                item: &dm_domain::DesktopItem,
                original: Option<&dm_domain::RestoreAnchor>,
            ) -> dm_domain::PortResult<Vec<dm_domain::DecodedImage>> {
                if item.id.as_str() == "edge" {
                    return Err(dm_domain::PortError::Io("cloud placeholder offline".into()));
                }
                DevIconSourceExtractor(self.0.clone()).extract(item, original)
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let desk = DevIconDesktop::new();
        let settings = Arc::new(SettingsStore::open(&dir.path().join("settings.sqlite3")).unwrap());
        let h = IconHost::new(
            IconHostPorts {
                scanner: Arc::new(DevDesktopScanner),
                extractor: Arc::new(OneBadExtractor(desk.clone())),
                reader: Arc::new(DevIconReader(desk.clone())),
                applier: Arc::new(DevIconApplier(desk)),
                overlay: Arc::new(DevOverlayControl),
                refresher: Arc::new(DevExplorerRefresher),
                geometry: Arc::new(DevDesktopGeometry),
            },
            settings,
            dir.path(),
            1,
        );
        let scan = h.scan().unwrap();
        let bad = scan.items.iter().find(|i| i.id == "edge").unwrap();
        assert!(!bad.styleable, "the unreadable item is not offered for styling");
        assert!(bad.source_urls.is_empty());
        assert!(bad.status_reason.as_deref().unwrap_or("").contains("图标读取失败"));
        // Everyone else still scanned + serves sources.
        let good = scan.items.iter().find(|i| i.id == "code").unwrap();
        assert!(good.styleable && !good.source_urls.is_empty());
        assert!(scan.items.len() >= 7, "the rest of the desktop survives one bad item");
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
    fn a_genuinely_empty_desktop_scan_is_a_valid_apply_target() {
        // codex R11-#2: a real scan of an EMPTY desktop (nothing to style) is still a valid apply
        // target — the user can submit a policy-only global Apply (kindPolicy/typeOverrides) whose
        // intent must persist to ②③. Validity is an explicit flag, not `scan.is_empty()`; only the
        // never-scanned and fenced states reject a Begin.
        struct EmptyScanner;
        impl DesktopScanner for EmptyScanner {
            fn scan(&self) -> dm_domain::PortResult<Vec<dm_domain::DesktopItem>> {
                Ok(Vec::new())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let desk = DevIconDesktop::new();
        let settings = Arc::new(SettingsStore::open(&dir.path().join("settings.sqlite3")).unwrap());
        let h = IconHost::new(
            IconHostPorts {
                scanner: Arc::new(EmptyScanner),
                extractor: Arc::new(DevIconSourceExtractor(desk.clone())),
                reader: Arc::new(DevIconReader(desk.clone())),
                applier: Arc::new(DevIconApplier(desk)),
                overlay: Arc::new(DevOverlayControl),
                refresher: Arc::new(DevExplorerRefresher),
                geometry: Arc::new(DevDesktopGeometry),
            },
            settings,
            dir.path(),
            1,
        );

        // Before ANY scan: no valid snapshot → Begin rejects.
        let err = h.apply_baked_begin(0, 0).unwrap_err();
        assert!(err.contains("no valid scan"), "never-scanned must reject: {err}");

        // A real empty scan: valid target, zero items.
        let scan = h.scan().unwrap();
        assert!(scan.items.is_empty());
        let sid = h.apply_baked_begin(scan.revision, 0).unwrap();
        let res = h.apply_baked_commit(&sid, style_json(5), vec![], Some("策略".into())).unwrap();
        assert!(res.ok, "a conflict-free zero-target (policy-only) Apply is a clean success");
        assert!(res.persisted.saved_style_json.is_some(), "② carries the policy intent");
        assert_eq!(res.persisted.history.len(), 1, "③ recorded the completed Apply");
    }

    #[test]
    fn an_ambiguous_heal_fences_the_scan_revision_until_a_real_rescan() {
        // codex R9-#1: the ABA. scan(O, r1) → apply styles S → the user MANUALLY restores the icon to
        // its exact original outside the app (indistinguishable from a poison row). Apply#2 at r1
        // heals (drops the row) + conflicts — and the host must then FENCE r1: a THIRD apply at the
        // same revision would find no ledger row, pass the ordinary fresh CAS (current O == scan O),
        // and silently overwrite the manual restore. Only a REAL rescan reopens the gate, after which
        // styling is the user's current, unambiguous intent.
        let dir = tempfile::tempdir().unwrap();
        let (h, desk) = host_with_desk(dir.path());
        let scan1 = h.scan().unwrap();
        let edge = scan1.items.iter().find(|i| i.id == "edge").unwrap().clone();

        // Apply #1: style edge (ledger row: original=O, last_applied=S).
        let sid = h.apply_baked_begin(scan1.revision, 1).unwrap();
        h.apply_baked_chunk(&sid, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        assert!(h.apply_baked_commit(&sid, style_json(1), vec![], Some("A".into())).unwrap().ok);

        // The user manually restores edge to its exact original (ledger row lingers → ambiguous tuple).
        desk.force_original("edge");

        // Apply #2 at the SAME revision: the heal drops the row + conflicts — never silently restyles.
        let sid2 = h.apply_baked_begin(scan1.revision, 1).unwrap();
        h.apply_baked_chunk(&sid2, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        let res2 = h.apply_baked_commit(&sid2, style_json(2), vec![], Some("B".into())).unwrap();
        assert!(!res2.ok, "the ambiguous heal must not read as success");
        assert_eq!(res2.toast.unwrap().key, "Toast_ApplyNoEffect");

        // The fence: a THIRD apply at the same stale revision is REJECTED before it can slip through
        // the now-row-less fresh CAS.
        let err = h.apply_baked_begin(scan1.revision, 1).unwrap_err();
        assert!(err.contains("stale apply"), "same-revision retry must be fenced: {err}");
        // And the fenced state means "NO valid scan" (codex R10-#B): the snapshot was cleared, so even
        // a Begin that somehow carries the synthetic fenced revision cannot bind pre-heal fingerprints.
        // Probe the fenced revision by brute force over the small window above scan1.
        for fenced in scan1.revision + 1..scan1.revision + 4 {
            if let Err(e) = h.apply_baked_begin(fenced, 1) {
                assert!(
                    e.contains("stale apply") || e.contains("no valid scan"),
                    "a fenced/unknown revision must never bind a snapshot: {e}"
                );
            } else {
                panic!("Begin({fenced}) bound a snapshot inside the fenced window");
            }
        }

        // A real rescan reopens the gate; styling is now the user's current, unambiguous intent.
        let scan2 = h.scan().unwrap();
        assert!(scan2.revision > scan1.revision);
        let sid3 = h.apply_baked_begin(scan2.revision, 1).unwrap();
        h.apply_baked_chunk(&sid3, vec![IconChunkItemDto { id: edge.id.clone(), source_index: 0, master_png: tiny_master() }]).unwrap();
        assert!(h.apply_baked_commit(&sid3, style_json(2), vec![], Some("B2".into())).unwrap().ok);
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

    #[test]
    fn source_cache_keeps_many_generations_and_never_evicts_a_live_key() {
        // codex R3-Major 5 / R4-Major 2: a changed icon's OLD content-addressed URL must still resolve
        // across the swap→adopt handoff — for MORE than one generation, since a scan whose adopt failed
        // leaves the UI on an even-older generation. The byte-bounded LRU covers it; an unchanged icon
        // that is re-scanned every generation must NEVER age out.
        let mut c = SourceCache::new(1024); // small cap, but each entry is tiny → many survive
        c.publish(HashMap::from([("live/0/h".to_string(), vec![1u8]), ("a/0/h1".to_string(), vec![2u8])]));
        // Several more generations where `live` is unchanged (re-inserted, same key) and `a` changes.
        c.publish(HashMap::from([("live/0/h".to_string(), vec![1u8]), ("a/0/h2".to_string(), vec![3u8])]));
        c.publish(HashMap::from([("live/0/h".to_string(), vec![1u8]), ("a/0/h3".to_string(), vec![4u8])]));
        // The live key survives every generation; the changed key's older versions survive well past a
        // single generation (all under the cap here).
        assert_eq!(c.get("live/0/h"), Some(vec![1]), "an unchanged, re-scanned icon never evicts");
        assert_eq!(c.get("a/0/h1"), Some(vec![2]), "two generations back still resolves");
        assert_eq!(c.get("a/0/h3"), Some(vec![4]), "the live generation resolves");
    }

    #[test]
    fn source_cache_evicts_oldest_past_the_byte_cap() {
        let mut c = SourceCache::new(100);
        // Insert entries that together exceed the cap; the oldest must be evicted, the newest kept.
        c.publish(HashMap::from([("old/0/h".to_string(), vec![0u8; 60])]));
        c.publish(HashMap::from([("new/0/h".to_string(), vec![0u8; 60])])); // now 120 > 100 → evict oldest
        assert_eq!(c.get("old/0/h"), None, "the oldest key was evicted past the cap");
        assert_eq!(c.get("new/0/h").map(|v| v.len()), Some(60), "the newest key is always kept");
    }

    #[test]
    fn source_cache_never_evicts_a_key_of_the_generation_being_published() {
        // codex R5-#7: a single scan whose OWN sources exceed the cap must still resolve EVERY key it
        // advertises — the DTO points the webview at all of them, so evicting one mid-publish would 404
        // the live desktop. The cap bounds HISTORICAL generations, never the current one.
        let mut c = SourceCache::new(100);
        c.publish(HashMap::from([("old/0/h".to_string(), vec![0u8; 90])])); // historical, will be trimmed
        // One generation of three 50-byte icons = 150 bytes > the 100 cap. All three must survive.
        c.publish(HashMap::from([
            ("live/0/h".to_string(), vec![1u8; 50]),
            ("live/1/h".to_string(), vec![2u8; 50]),
            ("live/2/h".to_string(), vec![3u8; 50]),
        ]));
        assert_eq!(c.get("old/0/h"), None, "the prior generation is evicted to make room");
        assert_eq!(c.get("live/0/h").map(|v| v.len()), Some(50), "live key 0 is pinned, never evicted");
        assert_eq!(c.get("live/1/h").map(|v| v.len()), Some(50), "live key 1 is pinned, never evicted");
        assert_eq!(c.get("live/2/h").map(|v| v.len()), Some(50), "live key 2 is pinned, never evicted");
    }

    #[test]
    fn get_persisted_keeps_the_restore_affordance_when_an_in_flight_txn_lingers() {
        // codex R5-#6: a degraded prior-crash recovery leaves an IN-FLIGHT (no-terminal) txn in the
        // journal and can style the desktop with NO ledger row. `applied` off the ledger alone would
        // then be false and HIDE the restore affordance — stranding the user. get_persisted must keep
        // `applied: true` off the retained in-flight journal so the restore path (which re-runs
        // recovery and heals) stays reachable.
        use dm_operations::txn::{journal::JournalRecord, FileJournal, JournalSink};
        let dir = tempfile::tempdir().unwrap();
        let h = host(dir.path());
        assert!(!h.get_persisted().unwrap().applied, "clean start: nothing applied, no in-flight txn");

        // Simulate the degraded recovery's residue: an in-flight txn lingering in the journal.
        let mut j = FileJournal::new(dir.path().join("txn.log"));
        j.append(&JournalRecord::TxnBegin { txn: 1, items: vec![dm_domain::ItemId::from_raw("x")] })
            .unwrap();

        assert!(
            h.get_persisted().unwrap().applied,
            "a lingering in-flight txn forces applied:true so restore stays reachable (ledger is still empty)"
        );

        // A CLEANLY-terminated txn (rolled back) is NOT pending repair — it must NOT trip the signal.
        j.append(&JournalRecord::TxnRolledBack { txn: 1 }).unwrap();
        assert!(
            !h.get_persisted().unwrap().applied,
            "a terminal (rolled-back) txn awaiting checkpoint never spuriously shows restore"
        );
    }
}
