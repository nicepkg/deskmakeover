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
    DesktopGeometryReader, DesktopScanner, ElevatedIconApplier, ExplorerRefresher, IconApplier,
    IconSourceExtractor, ItemStateReader, OverlayControl,
};
use dm_operations::icons::scope::ScopeRoots;
use dm_operations::icons::version_switch::OutputCache;
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
    /// The content-addressed OUTPUT cache (M6 Phase 4). A repeat `switchVersion` to an
    /// already-rendered look clones its stored master instead of recomputing it — a pure memo of
    /// `render_tile` (a hit is byte-identical to a fresh render). Lives under `mut_state` so the
    /// switch's `&mut` access is already serialized; 64 MiB byte-budget LRU. Inert on the scalar
    /// build (no get/insert), so it can never change an output byte off the fast path.
    output_cache: OutputCache,
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
    /// Styles privileged shared items (Public Desktop / ProgramData `.lnk`s) via the elevated helper
    /// (one UAC per batch). `None` where no privileged scope exists — the dev host's virtual desktop
    /// still wires a fake so the elevated apply/reset paths are exercised off-Windows.
    pub elevated: Option<Arc<dyn ElevatedIconApplier + Send + Sync>>,
}

pub struct IconHost {
    scanner: Arc<dyn DesktopScanner + Send + Sync>,
    extractor: Arc<dyn IconSourceExtractor + Send + Sync>,
    reader: Arc<dyn ItemStateReader + Send + Sync>,
    applier: Arc<dyn IconApplier + Send + Sync>,
    overlay: Arc<dyn OverlayControl + Send + Sync>,
    refresher: Arc<dyn ExplorerRefresher + Send + Sync>,
    geometry: Arc<dyn DesktopGeometryReader + Send + Sync>,
    /// The elevated desktop-item applier (privileged shared items). See [`IconHostPorts::elevated`].
    elevated: Option<Arc<dyn ElevatedIconApplier + Send + Sync>>,
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
    /// The content hash of `overlay_ico` — the overlay's install signature. On apply the effective
    /// overlay is `hidden:<sha>`; if it differs from what was last installed (a native→hidden
    /// transition OR an asset content change across an app update), Explorer is reloaded so the
    /// `Shell Icons\29` overlay — which Explorer caches at startup — actually takes visual effect.
    overlay_ico_sha: String,
    /// Persists the last-installed overlay signature (`hidden:<sha>` / `native`), so a repeat apply
    /// of the SAME overlay never flickers the desktop, yet a changed one always refreshes.
    overlay_install_marker: PathBuf,
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
        // has a real path to validate + copy into ProgramData (codex Block 5). Its content hash is the
        // overlay's install signature: if it changes across an app update (e.g. the 2026-07-16 AND-mask
        // black-block fix), the next apply reloads Explorer even without a native↔hidden transition.
        let overlay_ico = data_dir.join("overlay-transparent.ico");
        let overlay_asset = dm_icon_codec::transparent_ico();
        let overlay_ico_sha = overlay_asset.content_hash;
        let _ = std::fs::write(&overlay_ico, &overlay_asset.bytes);
        let overlay_install_marker = data_dir.join("overlay-installed.txt");
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
            elevated: ports.elevated,
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
                output_cache: OutputCache::new(),
            }),
            sources: Mutex::new(SourceCache::new(SOURCE_CACHE_CAP)),
            revision: AtomicU32::new(0),
            arrow_overlay: Mutex::new(arrow),
            arrow_marker,
            overlay_ico,
            overlay_ico_sha,
            overlay_install_marker,
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

    /// Reloads the machine-wide shortcut-overlay icon (`HKLM ...\Shell Icons\29`) after it TRANSITIONS
    /// between the transparent overlay and the native arrow. Explorer caches that value at STARTUP and
    /// `SHChangeNotify` does NOT reload it, so a fresh apply writes the transparent `.ico` yet the ugly
    /// native arrow keeps showing until Explorer restarts (owner report 2026-07-16). This is the
    /// standard shortcut-arrow-tweak refresh; it runs ONLY on a native↔transparent transition (≈once
    /// per makeover / reset), never on a repeat apply, so the desktop flickers at most once. Best-effort
    /// — a refresh fault never fails the op. The conditional restart avoids a stray Explorer window if
    /// the shell auto-respawns. [WINDOWS-VERIFY] runtime.
    pub(super) fn refresh_shell_icon_overlay(&self) {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue; \
                     Start-Sleep -Milliseconds 700; \
                     if (-not (Get-Process -Name explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
        }
    }

    /// Records the effective overlay signature just installed (`hidden:<sha>` / `native`) and returns
    /// whether it DIFFERS from the previously-installed one — i.e. whether Explorer must reload for
    /// the change to become visible. A repeat apply of the SAME overlay returns false (no flicker); a
    /// native↔hidden transition OR an overlay-asset content change (e.g. the black-block fix) returns
    /// true. Best-effort persistence: a lost marker only costs one extra (idempotent) refresh.
    pub(super) fn overlay_install_changed(&self, sig: &str) -> bool {
        let prev = std::fs::read_to_string(&self.overlay_install_marker).unwrap_or_default();
        let changed = prev.trim() != sig;
        if changed {
            let _ = std::fs::write(&self.overlay_install_marker, sig);
        }
        changed
    }

    pub(super) fn ops(&self) -> IconOps<'_> {
        IconOps::new(
            IconPlatform::new(&*self.reader, &*self.applier, &self.assets),
            &self.settings,
        )
    }

    /// The elevated applier as a bare trait ref for the ops apply/reset calls (`None` on a host with
    /// no privileged scope — the ops then leave privileged items as honest skips, fail-closed).
    pub(super) fn elevated(&self) -> Option<&dyn ElevatedIconApplier> {
        // Drop the `+ Send + Sync` marker bounds the storage carries — the ops take a plain trait ref.
        self.elevated.as_deref().map(|e| e as &dyn ElevatedIconApplier)
    }
}

mod dto;
// pub(crate): preset_store shares the civil-date stamp helpers (spec 09).
pub(crate) mod export;
mod mutations;
mod scan;
mod source_cache;

use source_cache::{SourceCache, SOURCE_CACHE_CAP};

#[cfg(all(test, not(windows)))]
mod tests;
