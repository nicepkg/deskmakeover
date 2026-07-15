//! Host for the 清爽 (calm-Windows) settings decision core: it holds the `TweakDriver` behind one
//! mutex (the host-side op lock — the driver's own writer lease + this mutex serialize every
//! mutation), maps the frontend's `CalmBackend` verbs onto driver calls, and converts the richer
//! domain outcomes to the thin wire DTOs.
//!
//! Two stacks behind one type-erased driver (`CalmDriverOps`):
//! - **Windows** (`new_windows`): the real `WinregBackend` + `WindowsSystemProfileProbe` (Wave 2
//!   ports) under the FAIL-CLOSED manifest — no recipe is certified until the W3 certification
//!   lab enumerates the write allowlist (ADR-0023 D2), so every automatic candidate surfaces as
//!   held and NO registry write can happen; the guided rows' 「带我去关」 REALLY launches the
//!   catalog's `ms-settings:` route (owner report 2026-07-16: the devhost stub made the button a
//!   silent no-op on the shipped app).
//! - **Devhost** (`new_devhost`, every other platform + tests): in-memory fakes certifying the
//!   starter slice on a synthetic Windows 11 profile so the whole probe→apply→verify→restore
//!   pipeline runs end to end on Mac, exactly as the browser mock does.

use std::collections::HashSet;
use std::sync::Mutex;

use dm_contracts::{
    CalmApplyOutcomeDto, CalmApplyRowDto, CalmGuidedProbeDto, CalmProbeRowDto, CalmProbeStateDto,
    CalmRestoreOutcomeDto, CalmRestoreRowDto, CalmSkipReasonDto,
};
use dm_domain::system_tweaks::{
    ApplyOutcome, ProbeOutcome, RawRegistryValue, RegistryBackend, RestoreOutcome, SettingId,
    SkipReason, SystemProfileProbe, WindowsEdition, WindowsEnvironment,
};
use dm_operations::system_tweaks::{
    first_batch, JournalStore, ManualRoute, MemoryJournal, MemoryProfileProbe, MemoryRegistry,
    MemoryVerifier, StandardVerification, TweakCatalog, TweakDriver, TweakTier,
    VerificationBackend, VerificationManifest, VerificationRule, VerifiedBuildFamily,
};

/// The type-erased driver verbs the host needs — one `Box` holds either stack.
trait CalmDriverOps: Send {
    fn recover(&mut self) -> Result<(), String>;
    fn inspect(&self, feature: &SettingId) -> Result<ProbeOutcome, String>;
    fn apply(&mut self, feature: &SettingId) -> Result<ApplyOutcome, String>;
    fn restore(&mut self, feature: &SettingId) -> Result<RestoreOutcome, String>;
}

impl<B, J, V, P> CalmDriverOps for TweakDriver<B, J, V, P>
where
    B: RegistryBackend + Send,
    J: JournalStore + Send,
    V: VerificationBackend<B> + Send,
    P: SystemProfileProbe + Send,
{
    fn recover(&mut self) -> Result<(), String> {
        TweakDriver::recover(self).map(|_| ()).map_err(|e| e.to_string())
    }
    fn inspect(&self, feature: &SettingId) -> Result<ProbeOutcome, String> {
        TweakDriver::inspect(self, feature).map_err(|e| e.to_string())
    }
    fn apply(&mut self, feature: &SettingId) -> Result<ApplyOutcome, String> {
        TweakDriver::apply(self, feature).map_err(|e| e.to_string())
    }
    fn restore(&mut self, feature: &SettingId) -> Result<RestoreOutcome, String> {
        TweakDriver::restore(self, feature).map_err(|e| e.to_string())
    }
}

/// The synthetic-but-plausible Windows 11 profile the devhost certifies (build 26100.8737, Pro,
/// US, x64). It matches a real "24H2" fingerprint shape so the certification path is exercised.
fn devhost_environment() -> WindowsEnvironment {
    WindowsEnvironment {
        major: 10,
        minor: 0,
        build: 26_100,
        ubr: 8_737,
        display_version: "24H2".into(),
        edition_id: "Professional".into(),
        edition: WindowsEdition::Pro,
        installation_type: "Client".into(),
        product_type: 48,
        is_workstation: true,
        region: "US".into(),
        native_architecture: "x64".into(),
        process_architecture: "x64".into(),
        packaged: false,
    }
}

/// The starter-slice ids the devhost certifies as writable (mirrors the frontend catalog's
/// `starterSlice`). Every other automatic candidate stays fail-closed (surfaces as held).
const STARTER_SLICE: &[&str] = &["start.recommendations", "taskbar.search", "taskbar.taskview"];

/// The tier-shaped fail-closed rule: automatic/advanced candidates carry EMPTY certification
/// (no build family, no profile — nothing can ever pass), guided rows are ManualOnly.
fn fail_closed_rule(tier: TweakTier) -> VerificationRule {
    match tier {
        TweakTier::AutomaticCandidate => VerificationRule::Standard(StandardVerification {
            families: Vec::new(),
            profiles: Vec::new(),
        }),
        TweakTier::Advanced => VerificationRule::Advanced(Vec::new()),
        TweakTier::Guided => VerificationRule::ManualOnly,
    }
}

/// The REAL-Windows manifest until the W3 certification lab lands (ADR-0023 D2): every recipe
/// fail-closed — the app can probe honestly and walk guided routes, but no registry write is
/// reachable. The lab's output replaces this with per-recipe certified families.
fn fail_closed_manifest() -> VerificationManifest {
    VerificationManifest::new(
        first_batch()
            .into_iter()
            .map(|descriptor| (descriptor.id, fail_closed_rule(descriptor.tier))),
    )
}

/// A manifest certifying ONLY the starter slice on the devhost profile; all other automatic
/// candidates keep the empty (fail-closed) rule, and guided rows are ManualOnly.
fn devhost_manifest() -> VerificationManifest {
    let certified = VerificationRule::Standard(StandardVerification {
        families: vec![VerifiedBuildFamily {
            build: 26_100,
            min_ubr: 8_000,
            max_ubr: Some(9_000),
        }],
        profiles: vec![devhost_environment()],
    });
    VerificationManifest::new(first_batch().into_iter().map(|descriptor| {
        let rule = if STARTER_SLICE.contains(&descriptor.id.as_str()) {
            certified.clone()
        } else {
            fail_closed_rule(descriptor.tier)
        };
        (descriptor.id, rule)
    }))
}

/// A registry pre-seeded with every writable recipe's target key present and its value "on" (1),
/// i.e. the surface is currently pushing content.
fn devhost_registry() -> MemoryRegistry {
    let mut registry = MemoryRegistry::new();
    let on = RawRegistryValue::dword(1);
    for descriptor in first_batch() {
        for mutation in &descriptor.mutations {
            registry.set_value(mutation.address.clone(), on.clone());
        }
    }
    registry
}

/// The mutable host state under one lock.
struct HostState {
    driver: Box<dyn CalmDriverOps>,
    /// Guided rows whose route the user has opened this session (drives the devhost return-probe;
    /// bookkeeping-only on real Windows, where the return-probe never fabricates an answer).
    walked: HashSet<String>,
}

/// How `open_route` launches a `ms-settings:` URI. Injected so the devhost (and every test,
/// including on a Windows dev box) never pops a real Settings window.
type RouteLauncher = fn(&str) -> Result<(), String>;

/// The calm settings host managed by Tauri.
pub struct TweaksHost {
    state: Mutex<HostState>,
    /// True only for the real-Windows stack: the guided return-probe answers Unreadable (the app
    /// has no real read path for guided rows yet — auto-confirming a mere walk would lie).
    real_os: bool,
    /// `Some` only for the real-Windows stack: 「带我去关」 launches the catalog route.
    launcher: Option<RouteLauncher>,
}

impl TweaksHost {
    /// Build the devhost host (non-Windows platforms + tests): in-memory fakes, synthetic profile.
    pub fn new_devhost() -> Self {
        let driver = TweakDriver::new(
            TweakCatalog::first_batch().expect("the first-batch catalog is valid"),
            devhost_manifest(),
            devhost_registry(),
            MemoryJournal::new(),
            MemoryVerifier::new(),
            MemoryProfileProbe::new(devhost_environment()),
        );
        Self::with_driver(Box::new(driver), false, None)
    }

    /// Build the real-Windows host: real registry + profile ports (Wave 2) under the fail-closed
    /// manifest (no writes until W3 certifies recipes). The journal stays in-memory BECAUSE the
    /// manifest is fail-closed — no transaction can exist to survive a restart; the W3 write
    /// slice must land a durable journal together with its certified manifest.
    #[cfg(windows)]
    pub fn new_windows() -> Self {
        let driver = TweakDriver::new(
            TweakCatalog::first_batch().expect("the first-batch catalog is valid"),
            fail_closed_manifest(),
            dm_windows::system_tweaks::WinregBackend::new(),
            MemoryJournal::new(),
            MemoryVerifier::new(),
            dm_windows::system_tweaks::WindowsSystemProfileProbe::new(),
        );
        Self::with_driver(
            Box::new(driver),
            true,
            Some(dm_windows::system_tweaks::launch::open_settings_page as RouteLauncher),
        )
    }

    fn with_driver(driver: Box<dyn CalmDriverOps>, real_os: bool, launcher: Option<RouteLauncher>) -> Self {
        Self {
            state: Mutex::new(HostState {
                driver,
                walked: HashSet::new(),
            }),
            real_os,
            launcher,
        }
    }

    /// Probe every catalog row into its wire state. Recovery runs first so a crash-left-prepared
    /// transaction self-heals before the frontend reads state (mirrors the icon host discipline).
    pub fn probe(&self) -> Result<Vec<CalmProbeRowDto>, String> {
        let mut state = self.lock()?;
        state.driver.recover()?;
        let mut rows = Vec::new();
        for descriptor in first_batch() {
            let outcome = state.driver.inspect(&descriptor.id)?;
            rows.push(probe_row(descriptor.id.as_str(), outcome));
        }
        Ok(rows)
    }

    /// Apply the given ids (the one-click package), returning each row's outcome.
    pub fn apply(&self, ids: Vec<String>) -> Result<Vec<CalmApplyRowDto>, String> {
        let mut state = self.lock()?;
        let mut rows = Vec::with_capacity(ids.len());
        for id in ids {
            let feature = SettingId::new(&id);
            let outcome = state.driver.apply(&feature)?;
            rows.push(apply_row(&id, outcome));
        }
        Ok(rows)
    }

    /// Restore every currently-owned writable row toward its original.
    pub fn restore(&self) -> Result<Vec<CalmRestoreRowDto>, String> {
        let mut state = self.lock()?;
        let owned = owned_ids(state.driver.as_ref())?;
        let mut rows = Vec::with_capacity(owned.len());
        for id in owned {
            let outcome = state.driver.restore(&SettingId::new(&id))?;
            rows.push(restore_row(&id, outcome));
        }
        Ok(rows)
    }

    /// Restore ONE owned row toward its original.
    pub fn restore_one(&self, id: String) -> Result<CalmRestoreRowDto, String> {
        let mut state = self.lock()?;
        let outcome = state.driver.restore(&SettingId::new(&id))?;
        Ok(restore_row(&id, outcome))
    }

    /// Open a guided row's documented route. Real Windows launches the catalog's `ms-settings:`
    /// page (spec 08 §5); `WidgetsBoardSettings` has no launchable URI (the Win+W board), so its
    /// row caption carries the walk. The devhost only records the walk for its return-probe.
    pub fn open_route(&self, id: String) -> Result<(), String> {
        let mut state = self.lock()?;
        if let Some(launch) = self.launcher {
            let descriptor = first_batch()
                .into_iter()
                .find(|d| d.id.as_str() == id)
                .ok_or_else(|| format!("unknown calm control: {id}"))?;
            if let Some(ManualRoute::SettingsPage(uri)) = descriptor.manual_route {
                launch(uri)?;
            }
        }
        state.walked.insert(id);
        Ok(())
    }

    /// A guided row's return-probe. Real Windows: always Unreadable — guided descriptors carry
    /// no probe path yet, and reporting Off because the user merely OPENED the page would be a
    /// fabricated confirmation (the page then asks 关好了吗 and records the user's attestation).
    /// Devhost: walked readable rows report Off so the Mac dev loop exercises the confirm path.
    pub fn re_probe_guided(&self, id: String) -> Result<CalmGuidedProbeDto, String> {
        if self.real_os {
            return Ok(CalmGuidedProbeDto::Unreadable);
        }
        let state = self.lock()?;
        let readable = first_batch()
            .into_iter()
            .find(|descriptor| descriptor.id.as_str() == id)
            .and_then(|descriptor| descriptor.readable_state)
            .unwrap_or(false);
        Ok(if !readable {
            CalmGuidedProbeDto::Unreadable
        } else if state.walked.contains(&id) {
            CalmGuidedProbeDto::Off
        } else {
            CalmGuidedProbeDto::StillOn
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HostState>, String> {
        self.state
            .lock()
            .map_err(|_| "calm host lock poisoned".to_string())
    }
}

/// Every writable feature the driver currently owns (for a global restore).
fn owned_ids(driver: &dyn CalmDriverOps) -> Result<Vec<String>, String> {
    let mut owned = Vec::new();
    for descriptor in first_batch() {
        if descriptor.tier == TweakTier::Guided {
            continue;
        }
        match driver.inspect(&descriptor.id)? {
            ProbeOutcome::OwnedQuiet | ProbeOutcome::OwnedDrifted => {
                owned.push(descriptor.id.as_str().to_string());
            }
            _ => {}
        }
    }
    Ok(owned)
}

fn probe_row(id: &str, outcome: ProbeOutcome) -> CalmProbeRowDto {
    let (state, owned_by_us, drifted_from_us) = match outcome {
        ProbeOutcome::AlreadyQuiet => (CalmProbeStateDto::Quiet, false, false),
        ProbeOutcome::Pushing => (CalmProbeStateDto::Pushing, false, false),
        ProbeOutcome::OwnedQuiet => (CalmProbeStateDto::Quiet, true, false),
        ProbeOutcome::OwnedDrifted => (CalmProbeStateDto::Pushing, false, true),
        ProbeOutcome::Unsupported(_) => (CalmProbeStateDto::Unsupported, false, false),
        ProbeOutcome::Managed => (CalmProbeStateDto::Managed, false, false),
        ProbeOutcome::NeedsReconfirm => (CalmProbeStateDto::NeedsReconfirm, false, false),
    };
    CalmProbeRowDto {
        id: id.to_string(),
        state,
        owned_by_us,
        drifted_from_us,
    }
}

fn apply_row(id: &str, outcome: ApplyOutcome) -> CalmApplyRowDto {
    let (outcome, reason) = match outcome {
        ApplyOutcome::Verified => (CalmApplyOutcomeDto::Verified, None),
        ApplyOutcome::SetAwaiting => (CalmApplyOutcomeDto::SetAwaiting, None),
        ApplyOutcome::Reverted => (CalmApplyOutcomeDto::Reverted, None),
        ApplyOutcome::Skipped(SkipReason::Changed) => {
            (CalmApplyOutcomeDto::Skipped, Some(CalmSkipReasonDto::Changed))
        }
    };
    CalmApplyRowDto {
        id: id.to_string(),
        outcome,
        reason,
    }
}

fn restore_row(id: &str, outcome: RestoreOutcome) -> CalmRestoreRowDto {
    let outcome = match outcome {
        RestoreOutcome::Restored => CalmRestoreOutcomeDto::Restored,
        RestoreOutcome::SkippedExternalConflict => CalmRestoreOutcomeDto::SkippedDrift,
    };
    CalmRestoreRowDto {
        id: id.to_string(),
        outcome,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host wired exactly like `new_windows` minus the real OS ports: the fail-closed manifest
    /// over the devhost fakes. Lets the ADR-0023 D2 gate be asserted on every platform.
    fn fail_closed_host() -> TweaksHost {
        let driver = TweakDriver::new(
            TweakCatalog::first_batch().expect("the first-batch catalog is valid"),
            fail_closed_manifest(),
            devhost_registry(),
            MemoryJournal::new(),
            MemoryVerifier::new(),
            MemoryProfileProbe::new(devhost_environment()),
        );
        TweaksHost::with_driver(Box::new(driver), false, None)
    }

    #[test]
    fn the_fail_closed_manifest_certifies_no_write_anywhere() {
        // ADR-0023 D2 (owner 2026-07-16 ship round): until the W3 lab certifies recipes, the
        // real-Windows stack must hold every automatic candidate as held — even with every
        // surface pushing, apply must be unreachable for every id, starter slice included.
        let host = fail_closed_host();
        for row in host.probe().expect("probe runs") {
            let control = first_batch()
                .into_iter()
                .find(|d| d.id.as_str() == row.id)
                .expect("probe rows come from the catalog");
            if control.tier != TweakTier::Guided {
                assert_eq!(
                    row.state,
                    CalmProbeStateDto::Unsupported,
                    "{} must surface fail-closed (held), got {:?}",
                    row.id,
                    row.state
                );
            }
        }
        // Every non-guided recipe (not just the starter slice) must refuse a single-id apply —
        // the fail-closed gate is per-recipe, so assert it per-recipe (codex P3 test strength).
        for descriptor in first_batch() {
            if descriptor.tier == TweakTier::Guided {
                continue;
            }
            host.apply(vec![descriptor.id.as_str().to_string()])
                .expect_err(&format!("{} must refuse a write pre-W3", descriptor.id.as_str()));
        }
    }

    #[test]
    fn every_settings_page_route_is_a_wellformed_ms_settings_uri() {
        // open_route hands these literals to ShellExecuteW on the real box — a typo'd scheme
        // would silently open nothing (the exact owner report this round fixed).
        let mut seen = 0;
        for descriptor in first_batch() {
            if let Some(ManualRoute::SettingsPage(uri)) = descriptor.manual_route {
                assert!(
                    uri.starts_with("ms-settings:") && uri.len() > "ms-settings:".len(),
                    "{}: bad settings route {uri:?}",
                    descriptor.id.as_str()
                );
                seen += 1;
            }
        }
        assert!(seen >= 3, "the catalog's guided rows carry settings routes");
    }

    #[test]
    fn the_real_os_return_probe_never_fabricates_a_confirmation() {
        // Walking a route on real Windows must NOT auto-confirm the row as off — the host has
        // no read path for guided rows, so the only honest answer is Unreadable (ask the user).
        // Launcher = a recording stub, so this never pops a Settings window on a dev box.
        fn record_launch(_uri: &str) -> Result<(), String> {
            Ok(())
        }
        let driver = TweakDriver::new(
            TweakCatalog::first_batch().expect("valid catalog"),
            fail_closed_manifest(),
            devhost_registry(),
            MemoryJournal::new(),
            MemoryVerifier::new(),
            MemoryProfileProbe::new(devhost_environment()),
        );
        let host = TweaksHost::with_driver(Box::new(driver), true, Some(record_launch));
        host.open_route("taskbar.widgetsButton".into()).expect("route walk records");
        assert_eq!(
            host.re_probe_guided("taskbar.widgetsButton".into()).unwrap(),
            CalmGuidedProbeDto::Unreadable,
            "a walk alone must never read as a confirmation"
        );
    }
}
