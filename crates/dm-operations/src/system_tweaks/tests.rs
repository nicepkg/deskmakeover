//! End-to-end driver tests over the in-memory ports: the honest-state and reversibility
//! contract (spec 08 §3-§7) proven on Mac. A certified manifest makes the starter slice writable;
//! the fakes inject CAS failures, effect failures, external edits, and process interruptions.

use dm_domain::system_tweaks::{
    ApplyOutcome, ProbeOutcome, RawRegistryValue, RegistryAddress, RegistryBackend, RegistryHive,
    RegistrySnapshot, RegistryView, RestoreOutcome, SettingId, WindowsEdition, WindowsEnvironment,
};

use super::capability::{
    StandardVerification, VerificationManifest, VerificationRule, VerifiedBuildFamily,
};
use super::catalog::{first_batch, TweakCatalog};
use super::driver::{DriverError, TweakDriver};
use super::fakes::{MemoryProfileProbe, MemoryRegistry};
use super::journal::{JournalStore, MemoryJournal, TransactionValue};
use super::verify::MemoryVerifier;

const EXPLORER_ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const SEARCH: &str = r"Software\Microsoft\Windows\CurrentVersion\Search";
const CONTENT_DELIVERY: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
const USER_PROFILE_ENGAGEMENT: &str =
    r"Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement";
const SEARCH_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\SearchSettings";

fn env() -> WindowsEnvironment {
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

fn addr(key: &str, value: &str) -> RegistryAddress {
    RegistryAddress::new(RegistryHive::CurrentUser, RegistryView::Registry64, key, value)
}

/// A manifest that certifies EVERY automatic candidate on `env()` (so the driver may write).
fn certified_manifest() -> VerificationManifest {
    let rule = VerificationRule::Standard(StandardVerification {
        families: vec![VerifiedBuildFamily {
            build: 26_100,
            min_ubr: 8_000,
            max_ubr: Some(9_000),
        }],
        profiles: vec![env()],
    });
    VerificationManifest::new(
        first_batch()
            .into_iter()
            .filter(|d| matches!(d.tier, super::catalog::TweakTier::AutomaticCandidate))
            .map(|d| (d.id, rule.clone())),
    )
}

/// A registry pre-seeded with every calm target key present and each recipe value at "on" (1),
/// i.e. the surface is currently pushing content.
fn pushing_registry() -> MemoryRegistry {
    let mut registry = MemoryRegistry::new();
    let on = RawRegistryValue::dword(1);
    for (key, value) in [
        (EXPLORER_ADVANCED, "Start_IrisRecommendations"),
        (EXPLORER_ADVANCED, "ShowTaskViewButton"),
        (EXPLORER_ADVANCED, "ShowSyncProviderNotifications"),
        (SEARCH, "SearchboxTaskbarMode"),
        (SEARCH_SETTINGS, "IsDynamicSearchBoxEnabled"),
        (CONTENT_DELIVERY, "SubscribedContent-338389Enabled"),
        (CONTENT_DELIVERY, "SubscribedContent-310093Enabled"),
        (CONTENT_DELIVERY, "SubscribedContent-338393Enabled"),
        (USER_PROFILE_ENGAGEMENT, "ScoobeSystemSettingEnabled"),
    ] {
        registry.set_value(addr(key, value), on.clone());
    }
    registry
}

type Driver = TweakDriver<MemoryRegistry, MemoryJournal, MemoryVerifier, MemoryProfileProbe>;

fn driver_with(registry: MemoryRegistry, verifier: MemoryVerifier) -> Driver {
    TweakDriver::new(
        TweakCatalog::first_batch().unwrap(),
        certified_manifest(),
        registry,
        MemoryJournal::new(),
        verifier,
        MemoryProfileProbe::new(env()),
    )
}

fn driver() -> Driver {
    driver_with(pushing_registry(), MemoryVerifier::new())
}

fn search() -> SettingId {
    SettingId::new("taskbar.search")
}

#[test]
fn a_certified_apply_verifies_and_owns_the_feature() {
    let mut driver = driver();
    assert_eq!(driver.inspect(&search()).unwrap(), ProbeOutcome::Pushing);
    assert_eq!(driver.apply(&search()).unwrap(), ApplyOutcome::Verified);
    // The live value is now 0, and a re-probe reports it owned-quiet.
    assert_eq!(driver.inspect(&search()).unwrap(), ProbeOutcome::OwnedQuiet);
}

#[test]
fn apply_is_idempotent_for_an_owned_feature() {
    let mut driver = driver();
    assert_eq!(driver.apply(&search()).unwrap(), ApplyOutcome::Verified);
    assert_eq!(driver.apply(&search()).unwrap(), ApplyOutcome::Verified);
}

#[test]
fn an_uncertified_environment_is_fail_closed() {
    // The default (initial) manifest grants no write.
    let mut driver = TweakDriver::new(
        TweakCatalog::first_batch().unwrap(),
        VerificationManifest::initial(&first_batch()),
        pushing_registry(),
        MemoryJournal::new(),
        MemoryVerifier::new(),
        MemoryProfileProbe::new(env()),
    );
    assert!(matches!(
        driver.apply(&search()),
        Err(DriverError::Unavailable(_))
    ));
}

#[test]
fn a_guided_feature_is_never_written() {
    let mut driver = driver();
    assert_eq!(
        driver.apply(&SettingId::new("widgets.feed")),
        Err(DriverError::Guided(SettingId::new("widgets.feed")))
    );
}

#[test]
fn an_effect_failure_rolls_the_write_back_to_the_original() {
    let mut verifier = MemoryVerifier::new();
    verifier.fail_next_effect("start did not reload");
    let mut driver = driver_with(pushing_registry(), verifier);
    let start = SettingId::new("start.recommendations");
    assert_eq!(driver.apply(&start).unwrap(), ApplyOutcome::Reverted);
    // The original value is back and the feature is NOT owned.
    assert!(driver.managed_for_test(&start).is_none());
    assert_eq!(driver.inspect(&start).unwrap(), ProbeOutcome::Pushing);
}

#[test]
fn a_cas_conflict_rolls_back_and_reverts() {
    let mut registry = pushing_registry();
    registry.fail_compare_exchange_at(1);
    let mut driver = driver_with(registry, MemoryVerifier::new());
    assert_eq!(driver.apply(&search()).unwrap(), ApplyOutcome::Reverted);
}

#[test]
fn an_externally_changed_base_value_is_skipped_not_clobbered() {
    let mut registry = pushing_registry();
    // The live base value is a String, not the accepted DWORD → skip.
    registry.set_value(
        addr(SEARCH, "SearchboxTaskbarMode"),
        RawRegistryValue::new(dm_domain::system_tweaks::RegistryValueKind::String, b"x".to_vec()),
    );
    let mut driver = driver_with(registry, MemoryVerifier::new());
    assert_eq!(
        driver.apply(&search()).unwrap(),
        ApplyOutcome::Skipped(dm_domain::system_tweaks::SkipReason::Changed)
    );
}

#[test]
fn a_present_policy_guard_reports_managed_and_is_never_written() {
    // taskbar.search carries an HKLM guard at Windows Search\SearchOnTaskbarMode. Its presence
    // means the feature is managed by policy.
    let guard = RegistryAddress::new(
        RegistryHive::LocalMachine,
        RegistryView::Registry64,
        r"Software\Policies\Microsoft\Windows\Windows Search",
        "SearchOnTaskbarMode",
    );
    let mut registry = pushing_registry();
    registry.set_value(guard, RawRegistryValue::dword(1));
    let mut driver = driver_with(registry, MemoryVerifier::new());
    assert_eq!(driver.inspect(&search()).unwrap(), ProbeOutcome::Managed);
    assert!(matches!(
        driver.apply(&search()),
        Err(DriverError::Unavailable(_))
    ));
}

#[test]
fn restore_returns_the_true_original_and_disowns() {
    let mut driver = driver();
    driver.apply(&search()).unwrap();
    assert_eq!(driver.restore(&search()).unwrap(), RestoreOutcome::Restored);
    // Back to pushing (the original 1) and no longer owned.
    assert_eq!(driver.inspect(&search()).unwrap(), ProbeOutcome::Pushing);
    assert!(driver.managed_for_test(&search()).is_none());
}

#[test]
fn restore_refuses_to_overwrite_an_external_edit() {
    let mut driver = driver();
    driver.apply(&search()).unwrap();
    // The user re-enables the search box themselves after our write.
    driver.set_live_for_test(addr(SEARCH, "SearchboxTaskbarMode"), RawRegistryValue::dword(1));
    assert_eq!(
        driver.restore(&search()).unwrap(),
        RestoreOutcome::SkippedExternalConflict
    );
    // The user's value stands; we disowned rather than clobbered.
    assert!(driver.managed_for_test(&search()).is_none());
}

#[test]
fn restore_of_an_unowned_feature_is_not_managed() {
    let mut driver = driver();
    assert_eq!(
        driver.restore(&search()),
        Err(DriverError::NotManaged(search()))
    );
}

#[test]
fn a_drifted_owned_value_probes_as_owned_drifted() {
    let mut driver = driver();
    driver.apply(&search()).unwrap();
    driver.set_live_for_test(addr(SEARCH, "SearchboxTaskbarMode"), RawRegistryValue::dword(1));
    assert_eq!(driver.inspect(&search()).unwrap(), ProbeOutcome::OwnedDrifted);
}

#[test]
fn a_crash_mid_apply_leaves_a_recoverable_prepared_transaction() {
    let mut registry = pushing_registry();
    // Interrupt AFTER the first successful write (settings.suggestions has one leaf).
    registry.interrupt_after_writes(1);
    let mut driver = driver_with(registry, MemoryVerifier::new());
    let feature = SettingId::new("settings.suggestions");
    // The interrupted apply surfaces an error and leaves the transaction prepared.
    assert!(driver.apply(&feature).is_err());
    // A fresh write is blocked until recovery runs.
    assert!(matches!(
        driver.apply(&search()),
        Err(DriverError::RecoveryRequired(_))
    ));
    // Recovery rolls the interrupted apply back to its original and clears the block.
    let report = driver.recover().unwrap();
    assert_eq!(report.recovered.len(), 1);
    assert!(report.conflicts.is_empty());
    assert_eq!(driver.apply(&search()).unwrap(), ApplyOutcome::Verified);
    // The interrupted feature was rolled back, never owned.
    assert!(driver.managed_for_test(&feature).is_none());
}

// Small test-only accessors, kept here so the driver's production surface stays minimal.
impl Driver {
    fn managed_for_test(&self, feature: &SettingId) -> Option<super::journal::ManagedSetting> {
        self.journal_ref().managed(feature).unwrap()
    }

    fn set_live_for_test(&mut self, address: RegistryAddress, value: RawRegistryValue) {
        self.backend_mut().set_value(address, value);
    }
}

// Reuse the transaction-value builder indirectly to keep the type imported (documentation of the
// leaf shape the journal persists).
#[allow(dead_code)]
fn _leaf_shape(value: &TransactionValue) -> &RegistrySnapshot {
    &value.original
}
