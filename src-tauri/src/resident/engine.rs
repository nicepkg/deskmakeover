//! The resident reconcile ENGINE (spec 07 §3 host side, plan T8): builds the platform ports and
//! drives `Reconciler::reconcile` / `apply_batch`. This is the cfg-selected-adapter seam the
//! icon/wallpaper/tweaks hosts already use — the Windows engine wires the real `dm-windows` adapters
//! + the `notify` watcher (`[WINDOWS-VERIFY]`, cannot be msvc-cross-checked here because `src-tauri`
//! pulls `rusqlite`'s C build); the Mac dev host reuses the icon devhost fakes so the whole loop is
//! REAL and unit-testable without a Windows box. The driver ([`dm_resident::ResidentDriver`]) owns
//! the *when*; this owns the *how*.
//!
//! ② saved-style (spec 07 §8) is re-read from the SHARED [`SettingsStore`] every cycle, so the
//! resident and the foreground never drift and a `None` ② keeps the resident dormant (spec 07 §8.3).

use std::sync::Arc;

use dm_operations::icons::scope::ScopeRoots;
use dm_operations::{JournalSink, LedgerStore, SettingsStore, TxnIdAllocator};
use dm_resident::{
    FreshnessInputs, ReconcileContext, ReconcileEngine, ReconcileOutcome, Reconciler,
    ReconcilerPorts, RestoreBatchOutcome, TrustState, UndoTarget, VettedCandidate,
};

/// The host-side engine the resident loop drives: [`ReconcileEngine::reconcile`] (the driver's
/// propose path) PLUS `apply_batch` (the host's confirm/2h-timeout path — spec 07 §2 item 4).
pub trait ResidentEngine: ReconcileEngine {
    /// Applies a confirmed / timed-out proposal (spec 07 §5): the SAME `TxnDriver::apply` the manual
    /// flow uses, writing ONLY store ①. A busy desktop or a since-propose hand-edit is handled inside
    /// the reconciler (CAS + activity re-check) — the host just hands the batch over.
    fn apply_batch(&mut self, candidates: Vec<VettedCandidate>) -> Result<ReconcileOutcome, String>;

    /// Undoes the last applied batch (spec 07 §13 level 2 — the tray 「撤销最近一次整理」):
    /// snapshot-CAS-gated ledger-anchor restores through [`Reconciler::restore_batch`].
    fn restore_batch(&mut self, targets: &[UndoTarget]) -> Result<RestoreBatchOutcome, String>;
}

/// Wall-clock seconds for the freshness signal (spec 07 §2 item 8 — dormant in v1, which always
/// proposes; supplied so the ctx is well-formed).
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Builds the per-cycle ctx from the SHARED stores and runs one reconcile. Platform-agnostic — both
/// engines build their own `ports`, then delegate here so the ctx assembly + ② read live once.
fn reconcile_with(
    rec: &mut Reconciler,
    ports: &ReconcilerPorts<'_>,
    settings: &SettingsStore,
    scope: &ScopeRoots,
    trust: &TrustState,
    txn: &mut TxnIdAllocator,
    journal: &mut dyn JournalSink,
    ledger: &mut dyn LedgerStore,
) -> Result<ReconcileOutcome, String> {
    let saved = settings.get_saved_style().map_err(|e| e.to_string())?;
    let ctx = ReconcileContext {
        saved_style: saved.as_ref(),
        trust,
        freshness: FreshnessInputs { last_apply_at: None, partial_reversion: false, now: now_secs() },
        scope,
    };
    rec.reconcile(ports, &ctx, txn, journal, ledger).map_err(|e| e.to_string())
}

/// The `apply_batch` twin of [`reconcile_with`].
fn apply_with(
    rec: &mut Reconciler,
    ports: &ReconcilerPorts<'_>,
    settings: &SettingsStore,
    scope: &ScopeRoots,
    trust: &TrustState,
    candidates: Vec<VettedCandidate>,
    txn: &mut TxnIdAllocator,
    journal: &mut dyn JournalSink,
    ledger: &mut dyn LedgerStore,
) -> Result<ReconcileOutcome, String> {
    let saved = settings.get_saved_style().map_err(|e| e.to_string())?;
    let ctx = ReconcileContext {
        saved_style: saved.as_ref(),
        trust,
        freshness: FreshnessInputs { last_apply_at: None, partial_reversion: false, now: now_secs() },
        scope,
    };
    rec.apply_batch(ports, &ctx, candidates, txn, journal, ledger).map_err(|e| e.to_string())
}

// ============================ Mac dev host (verified) =========================================
#[cfg(not(windows))]
mod devhost {
    use super::*;
    use std::path::Path;

    use dm_domain::{ActivityMonitor, PortResult};
    use dm_operations::{FsAssetStore, MemLedgerStore, VecJournal};
    use dm_resident::{StabilityReader, StabilitySnapshot};

    use crate::devhost_icons::{
        DevDesktopScanner, DevIconApplier, DevIconDesktop, DevIconReader, DevIconSourceExtractor,
    };

    /// Always-idle activity (spec 07 §11) — the dev host has no cursor to watch.
    struct DevActivity;
    impl ActivityMonitor for DevActivity {
        fn is_desktop_busy(&self) -> PortResult<bool> {
            Ok(false)
        }
    }

    /// Always-settled stability (spec 07 §3): the synthetic dev files never move, so a newcomer
    /// settles after the standard two quiet cycles the [`dm_resident::SettleProbe`] enforces.
    struct DevStability;
    impl StabilityReader for DevStability {
        fn snapshot(&self, _path: &str) -> StabilitySnapshot {
            StabilitySnapshot { size: 1, mtime_nanos: 1, readable: true }
        }
    }

    /// The Mac dev engine: a real [`Reconciler`] over the shared icon devhost desktop with in-memory
    /// stores (deliberately NOT the foreground's `ledger.json` — this demonstrates the loop without
    /// contending for the icon host's transaction lock; the shared-store integration is
    /// `[WINDOWS-VERIFY]`/follow-up). ② + the enabled flag come from the SHARED settings, so a fresh
    /// boot (empty ②) is correctly dormant.
    pub struct DevhostResidentEngine {
        rec: Reconciler,
        desk: Arc<DevIconDesktop>,
        settings: Arc<SettingsStore>,
        assets: FsAssetStore,
        scope: ScopeRoots,
        trust: TrustState,
        journal: VecJournal,
        ledger: MemLedgerStore,
        txn: TxnIdAllocator,
    }

    impl DevhostResidentEngine {
        pub fn new(settings: Arc<SettingsStore>, data_dir: &Path) -> Self {
            Self {
                rec: Reconciler::new(),
                desk: DevIconDesktop::new(),
                settings,
                assets: FsAssetStore::new(data_dir.join("resident-assets")),
                // The dev host has no shared/privileged desktop scope — nothing is scope-excluded.
                scope: ScopeRoots::Unprivileged,
                trust: TrustState::default(),
                journal: VecJournal::default(),
                ledger: MemLedgerStore::default(),
                txn: TxnIdAllocator::starting_at(1),
            }
        }

    }

    impl ReconcileEngine for DevhostResidentEngine {
        fn reconcile(&mut self) -> Result<ReconcileOutcome, String> {
            let scanner = DevDesktopScanner;
            let extractor = DevIconSourceExtractor(self.desk.clone());
            let reader = DevIconReader(self.desk.clone());
            let applier = DevIconApplier(self.desk.clone());
            let (activity, stability) = (DevActivity, DevStability);
            // Inline so `ports` borrows only `&self.assets` (disjoint from the &mut txn/journal/ledger).
            let ports = ReconcilerPorts {
                scanner: &scanner,
                extractor: &extractor,
                reader: &reader,
                applier: &applier,
                assets: &self.assets,
                activity: &activity,
                stability: &stability,
            };
            reconcile_with(
                &mut self.rec,
                &ports,
                &self.settings,
                &self.scope,
                &self.trust,
                &mut self.txn,
                &mut self.journal,
                &mut self.ledger,
            )
        }
    }

    impl ResidentEngine for DevhostResidentEngine {
        fn apply_batch(&mut self, candidates: Vec<VettedCandidate>) -> Result<ReconcileOutcome, String> {
            let scanner = DevDesktopScanner;
            let extractor = DevIconSourceExtractor(self.desk.clone());
            let reader = DevIconReader(self.desk.clone());
            let applier = DevIconApplier(self.desk.clone());
            let (activity, stability) = (DevActivity, DevStability);
            let ports = ReconcilerPorts {
                scanner: &scanner,
                extractor: &extractor,
                reader: &reader,
                applier: &applier,
                assets: &self.assets,
                activity: &activity,
                stability: &stability,
            };
            apply_with(
                &mut self.rec,
                &ports,
                &self.settings,
                &self.scope,
                &self.trust,
                candidates,
                &mut self.txn,
                &mut self.journal,
                &mut self.ledger,
            )
        }

        fn restore_batch(&mut self, targets: &[UndoTarget]) -> Result<RestoreBatchOutcome, String> {
            let scanner = DevDesktopScanner;
            let extractor = DevIconSourceExtractor(self.desk.clone());
            let reader = DevIconReader(self.desk.clone());
            let applier = DevIconApplier(self.desk.clone());
            let (activity, stability) = (DevActivity, DevStability);
            let ports = ReconcilerPorts {
                scanner: &scanner,
                extractor: &extractor,
                reader: &reader,
                applier: &applier,
                assets: &self.assets,
                activity: &activity,
                stability: &stability,
            };
            self.rec
                .restore_batch(&ports, targets, &mut self.journal, &mut self.ledger)
                .map_err(|e| e.to_string())
        }
    }
}
#[cfg(not(windows))]
pub use devhost::DevhostResidentEngine;

// ============================ Windows (blind, [WINDOWS-VERIFY]) ================================
// The real engine mirrors `build_icon_host`'s adapter set + adds the activity monitor, the fs
// stability reader, and the durable stores, all on ONE STA executor. It CANNOT be compiled or
// msvc-cross-checked from the Mac host (src-tauri pulls rusqlite's C build), so every line here is
// blind + folds into `docs/references/windows-wiring-handoff/m7-resident.md`. Scope stays
// `Unresolved` (fail-closed, spec 07 §14) until `SHGetKnownFolderPath` resolves the real roots.
#[cfg(windows)]
mod win {
    use super::*;
    use std::path::Path;

    use dm_operations::{FileJournal, FsAssetStore, JsonLedgerStore};
    use dm_resident::FsStabilityReader;
    use dm_windows::{
        StaExecutor, WindowsActivityMonitor, WindowsIconApplier, WindowsIconSourceExtractor,
        WindowsScanner, WindowsStateReader,
    };

    /// The real resident engine. [WINDOWS-VERIFY] end to end (activity hook, COM writers, durable
    /// journal/ledger shared with the foreground).
    pub struct WindowsResidentEngine {
        rec: Reconciler,
        exec: Arc<StaExecutor>,
        settings: Arc<SettingsStore>,
        assets: FsAssetStore,
        stability: FsStabilityReader,
        scope: ScopeRoots,
        trust: TrustState,
        journal: FileJournal,
        ledger: JsonLedgerStore,
        txn: TxnIdAllocator,
    }

    impl WindowsResidentEngine {
        /// Builds the engine on a fresh STA executor. Shares the foreground's `ledger.json` / `txn.log`
        /// so a background apply and a foreground apply recover from the SAME journal (spec 07 §1).
        pub fn new(settings: Arc<SettingsStore>, data_dir: &Path) -> Result<Self, String> {
            let exec = Arc::new(StaExecutor::spawn().map_err(|e| e.to_string())?);
            Ok(Self {
                rec: Reconciler::new(),
                exec,
                settings,
                assets: FsAssetStore::new(data_dir.join("icon-assets")),
                stability: FsStabilityReader,
                // §14 scope via the shared SHGetKnownFolderPath resolver (identical to the
                // foreground icon host's gate); fail-closed to Unresolved. [WINDOWS-VERIFY] runtime.
                scope: crate::resolve_scope_roots(),
                trust: TrustState::default(),
                journal: FileJournal::new(data_dir.join("txn.log")),
                ledger: JsonLedgerStore::new(data_dir.join("ledger.json")),
                txn: TxnIdAllocator::starting_at(1),
            })
        }
    }

    impl ReconcileEngine for WindowsResidentEngine {
        fn reconcile(&mut self) -> Result<ReconcileOutcome, String> {
            let scanner = WindowsScanner::new(self.exec.clone());
            let extractor = WindowsIconSourceExtractor::new(self.exec.clone());
            let reader = WindowsStateReader::new(self.exec.clone());
            let applier = WindowsIconApplier::new(self.exec.clone());
            let activity = WindowsActivityMonitor::new();
            let ports = ReconcilerPorts {
                scanner: &scanner,
                extractor: &extractor,
                reader: &reader,
                applier: &applier,
                assets: &self.assets,
                activity: &activity,
                stability: &self.stability,
            };
            reconcile_with(
                &mut self.rec,
                &ports,
                &self.settings,
                &self.scope,
                &self.trust,
                &mut self.txn,
                &mut self.journal,
                &mut self.ledger,
            )
        }
    }

    impl ResidentEngine for WindowsResidentEngine {
        fn apply_batch(&mut self, candidates: Vec<VettedCandidate>) -> Result<ReconcileOutcome, String> {
            let scanner = WindowsScanner::new(self.exec.clone());
            let extractor = WindowsIconSourceExtractor::new(self.exec.clone());
            let reader = WindowsStateReader::new(self.exec.clone());
            let applier = WindowsIconApplier::new(self.exec.clone());
            let activity = WindowsActivityMonitor::new();
            let ports = ReconcilerPorts {
                scanner: &scanner,
                extractor: &extractor,
                reader: &reader,
                applier: &applier,
                assets: &self.assets,
                activity: &activity,
                stability: &self.stability,
            };
            apply_with(
                &mut self.rec,
                &ports,
                &self.settings,
                &self.scope,
                &self.trust,
                candidates,
                &mut self.txn,
                &mut self.journal,
                &mut self.ledger,
            )
        }

        fn restore_batch(&mut self, targets: &[UndoTarget]) -> Result<RestoreBatchOutcome, String> {
            let scanner = WindowsScanner::new(self.exec.clone());
            let extractor = WindowsIconSourceExtractor::new(self.exec.clone());
            let reader = WindowsStateReader::new(self.exec.clone());
            let applier = WindowsIconApplier::new(self.exec.clone());
            let activity = WindowsActivityMonitor::new();
            let ports = ReconcilerPorts {
                scanner: &scanner,
                extractor: &extractor,
                reader: &reader,
                applier: &applier,
                assets: &self.assets,
                activity: &activity,
                stability: &self.stability,
            };
            self.rec
                .restore_batch(&ports, targets, &mut self.journal, &mut self.ledger)
                .map_err(|e| e.to_string())
        }
    }
}
#[cfg(windows)]
pub use win::WindowsResidentEngine;
