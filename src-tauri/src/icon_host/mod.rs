//! Icon host glue (M6-WIRE B4): assembles the THIN `icons.*` command results from the ports +
//! ops (D1-thin boundary — Rust scans/packages/applies/restores + persists ②③; the frontend
//! assembles `IconsStateDto` from these thin results + its own presets/palette/grid) and serves
//! extracted 256px sources over the `dmicon://` custom protocol so icon pixels never ride the JSON
//! bridge (the same discipline as wallpaper's `dmwallpaper://`).
//!
//! ALL mutable transaction state (ledger, journal, look-history, txn allocator, the chunk-buffer
//! session, and the last-scan cache) lives under ONE mutex — the B2 apply/GC lifecycle-lock's
//! runtime half — so apply and GC never interleave.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex};

use dm_contracts::ArrowOverlayDto;
use dm_domain::{
    DesktopGeometryReader, DesktopScanner, ExplorerRefresher, IconApplier, IconSourceExtractor,
    ItemStateReader, OverlayControl,
};
use dm_operations::icons::scope::ScopeRoots;
use dm_operations::txn::FileJournal;
use dm_operations::{
    FsAssetStore, IconApplySession, IconOps, IconPlatform, JsonLedgerStore, LookHistoryStore,
    ScannedItem, SettingsStore, TxnIdAllocator,
};


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
    /// The privileged-scope roots (spec §6/§14). `Unprivileged` on the dev host; `Unresolved` on
    /// Windows until `SHGetKnownFolderPath` resolves the real known folders ([WINDOWS-VERIFY]). An
    /// `Unresolved` scope makes version-switch / resident auto-format FAIL CLOSED (styles nothing),
    /// so a blind host can never bypass the §14 red line by shipping empty roots.
    scope_roots: ScopeRoots,
}

impl IconHost {
    pub fn new(
        ports: IconHostPorts,
        settings: Arc<SettingsStore>,
        data_dir: &Path,
        active_user_profiles: u32,
        scope: ScopeRoots,
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
            // The §14 privileged-scope roots (see the field doc). `Unprivileged` on the dev host;
            // `Unresolved` on Windows until the composition resolves the real known folders, which
            // makes automation FAIL CLOSED so a blind host can never style §14-privileged items with
            // empty roots (codex r2-🔴 / F-scope). The type makes the fail-OPEN state unrepresentable.
            scope_roots: scope,
        }
    }

    /// Sets the in-memory arrow-overlay state and persists it so it survives a restart (codex Block 5).
    /// Returns the marker WRITE result: a lost `Hidden` marker is DANGEROUS (on restart the host loads
    /// `native`, skips the elevated restore, and leaves the machine-wide overlay installed as residue —
    /// codex R2 B-3), so a caller recording an INSTALL must surface the failure; a lost `Native` marker
    /// is fail-safe (it only costs an extra idempotent restore next time), so those callers log + carry
    /// on. [WINDOWS-VERIFY]: the complete fix also probes the REAL registry overlay state on startup and
    /// before restore, rather than trusting this marker alone.
    pub(super) fn set_arrow(&self, arrow: ArrowOverlayDto) -> std::io::Result<()> {
        *self.arrow_overlay.lock().unwrap() = arrow;
        let text = match arrow {
            ArrowOverlayDto::Native => "native",
            ArrowOverlayDto::Hidden => "hidden",
        };
        std::fs::write(&self.arrow_marker, text)
    }

    /// Protocol lookup: the PNG bytes for `dmicon://…/<itemId>/<slot>/<hash>`, resolved against the
    /// live generation then the prior one so a swap→adopt handoff never 404s (codex R3-Major 5).
    pub fn png_for(&self, key: &str) -> Option<Vec<u8>> {
        self.sources.lock().unwrap().get(key)
    }

    pub(super) fn ops(&self) -> IconOps<'_> {
        IconOps::new(
            IconPlatform::new(&*self.reader, &*self.applier, &self.assets),
            &self.settings,
        )
    }
}

mod dto;
mod export;
mod mutations;
mod scan;
mod source_cache;

use source_cache::{SourceCache, SOURCE_CACHE_CAP};

#[cfg(all(test, not(windows)))]
mod tests;
