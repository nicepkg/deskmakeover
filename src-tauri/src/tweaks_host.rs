//! Host for the 清爽 (calm-Windows) settings decision core: it holds the `TweakDriver` behind one
//! mutex (the host-side op lock — the driver's own writer lease + this mutex serialize every
//! mutation), maps the frontend's `CalmBackend` verbs onto driver calls, and converts the richer
//! domain outcomes to the thin wire DTOs.
//!
//! W1 uses the devhost fakes on EVERY platform: the real `winreg`/`windows-rs` backend is Wave 2
//! ([WINDOWS-VERIFY]). The devhost certifies the starter slice on a synthetic Windows 11 profile so
//! the whole probe→apply→verify→restore pipeline runs end to end on Mac, exactly as the browser
//! mock does; the non-starter automatic candidates stay fail-closed (they surface as "held").

use std::collections::HashSet;
use std::sync::Mutex;

use dm_contracts::{
    CalmApplyOutcomeDto, CalmApplyRowDto, CalmGuidedProbeDto, CalmProbeRowDto, CalmProbeStateDto,
    CalmRestoreOutcomeDto, CalmRestoreRowDto, CalmSkipReasonDto,
};
use dm_domain::system_tweaks::{
    ApplyOutcome, ProbeOutcome, RawRegistryValue, RestoreOutcome, SettingId, SkipReason,
    WindowsEdition, WindowsEnvironment,
};
use dm_operations::system_tweaks::{
    first_batch, MemoryJournal, MemoryProfileProbe, MemoryRegistry, MemoryVerifier,
    StandardVerification, TweakCatalog, TweakDriver, TweakTier, VerificationManifest,
    VerificationRule, VerifiedBuildFamily,
};

/// The concrete W1 devhost driver: in-memory registry, journal, verifier, and profile probe.
type DevDriver = TweakDriver<MemoryRegistry, MemoryJournal, MemoryVerifier, MemoryProfileProbe>;

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
            match descriptor.tier {
                TweakTier::AutomaticCandidate => VerificationRule::Standard(StandardVerification {
                    families: Vec::new(),
                    profiles: Vec::new(),
                }),
                TweakTier::Advanced => VerificationRule::Advanced(Vec::new()),
                TweakTier::Guided => VerificationRule::ManualOnly,
            }
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
    driver: DevDriver,
    /// Guided rows whose route the user has opened this session (devhost stand-in for the OS walk).
    walked: HashSet<String>,
}

/// The calm settings host managed by Tauri.
pub struct TweaksHost {
    state: Mutex<HostState>,
}

impl TweaksHost {
    /// Build the W1 devhost host (used on every platform until the Wave 2 winreg backend lands).
    pub fn new_devhost() -> Self {
        let driver = TweakDriver::new(
            TweakCatalog::first_batch().expect("the first-batch catalog is valid"),
            devhost_manifest(),
            devhost_registry(),
            MemoryJournal::new(),
            MemoryVerifier::new(),
            MemoryProfileProbe::new(devhost_environment()),
        );
        Self {
            state: Mutex::new(HostState {
                driver,
                walked: HashSet::new(),
            }),
        }
    }

    /// Probe every catalog row into its wire state. Recovery runs first so a crash-left-prepared
    /// transaction self-heals before the frontend reads state (mirrors the icon host discipline).
    pub fn probe(&self) -> Result<Vec<CalmProbeRowDto>, String> {
        let mut state = self.lock()?;
        let _ = state.driver.recover().map_err(|error| error.to_string())?;
        let mut rows = Vec::new();
        for descriptor in first_batch() {
            let outcome = state
                .driver
                .inspect(&descriptor.id)
                .map_err(|error| error.to_string())?;
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
            let outcome = state
                .driver
                .apply(&feature)
                .map_err(|error| error.to_string())?;
            rows.push(apply_row(&id, outcome));
        }
        Ok(rows)
    }

    /// Restore every currently-owned writable row toward its original.
    pub fn restore(&self) -> Result<Vec<CalmRestoreRowDto>, String> {
        let mut state = self.lock()?;
        let owned = owned_ids(&state.driver)?;
        let mut rows = Vec::with_capacity(owned.len());
        for id in owned {
            let outcome = state
                .driver
                .restore(&SettingId::new(&id))
                .map_err(|error| error.to_string())?;
            rows.push(restore_row(&id, outcome));
        }
        Ok(rows)
    }

    /// Restore ONE owned row toward its original.
    pub fn restore_one(&self, id: String) -> Result<CalmRestoreRowDto, String> {
        let mut state = self.lock()?;
        let outcome = state
            .driver
            .restore(&SettingId::new(&id))
            .map_err(|error| error.to_string())?;
        Ok(restore_row(&id, outcome))
    }

    /// Open a guided row's documented route. Real Windows launches the `ms-settings:` page
    /// ([WINDOWS-VERIFY]); the devhost records the walk so the return-probe can answer.
    pub fn open_route(&self, id: String) -> Result<(), String> {
        let mut state = self.lock()?;
        state.walked.insert(id);
        Ok(())
    }

    /// A guided row's return-probe: readable rows report off (walked) / still-on; unreadable rows
    /// report unreadable (the app cannot know, so the user attests).
    pub fn re_probe_guided(&self, id: String) -> Result<CalmGuidedProbeDto, String> {
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
fn owned_ids(driver: &DevDriver) -> Result<Vec<String>, String> {
    let mut owned = Vec::new();
    for descriptor in first_batch() {
        if descriptor.tier == TweakTier::Guided {
            continue;
        }
        match driver.inspect(&descriptor.id).map_err(|e| e.to_string())? {
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
