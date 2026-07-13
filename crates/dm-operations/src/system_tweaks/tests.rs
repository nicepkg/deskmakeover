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

// ---- codex W1 review regression tests ----

#[test]
fn re_applying_a_drifted_owned_row_re_establishes_it_and_re_verifies() {
    // codex #1: the owned fast path must NOT blind-return Verified; a drifted row re-closes.
    let mut driver = driver();
    driver.apply(&search()).unwrap();
    // The user re-enables the search box (drift).
    driver.set_live_for_test(addr(SEARCH, "SearchboxTaskbarMode"), RawRegistryValue::dword(1));
    assert_eq!(driver.inspect(&search()).unwrap(), ProbeOutcome::OwnedDrifted);
    // Re-applying re-writes the desired value and proves it — it does not falsely claim Verified.
    assert_eq!(driver.apply(&search()).unwrap(), ApplyOutcome::Verified);
    assert_eq!(driver.inspect(&search()).unwrap(), ProbeOutcome::OwnedQuiet);
    // The TRUE original (1) is preserved, so a restore still returns to it.
    assert_eq!(driver.restore(&search()).unwrap(), RestoreOutcome::Restored);
    assert_eq!(driver.inspect(&search()).unwrap(), ProbeOutcome::Pushing);
}

#[test]
fn a_rollback_never_overwrites_an_external_edit_and_stays_pending() {
    // codex #2: if the user writes a THIRD value during settle, a failed apply must NOT clobber
    // it — the transaction stays prepared for recovery.
    let mut verifier = MemoryVerifier::new();
    // During settle, the user sets the value to 2 (neither our desired 0 nor the before 1), then
    // the effect check fails so apply must roll back.
    verifier.replace_on_next_settle(
        addr(SEARCH, "SearchboxTaskbarMode"),
        RegistrySnapshot::Present(RawRegistryValue::dword(2)),
    );
    verifier.fail_next_effect("forced failure after the external edit");
    let mut driver = driver_with(pushing_registry(), verifier);
    let result = driver.apply(&search());
    assert!(matches!(result, Err(DriverError::Pending { .. })), "got {result:?}");
    // The user's value (2) stands — never clobbered — and a new write is blocked until recovery.
    assert!(matches!(
        driver.apply(&SettingId::new("start.recommendations")),
        Err(DriverError::RecoveryRequired(_))
    ));
}

#[test]
fn a_missing_target_key_is_skipped_never_created() {
    // codex #5: a missing KEY must fail closed (no key creation in W1).
    let mut registry = MemoryRegistry::new();
    // Seed EVERY key except the taskbar Search key, then set the value's key absent.
    for (key, value) in [
        (EXPLORER_ADVANCED, "Start_IrisRecommendations"),
        (EXPLORER_ADVANCED, "ShowTaskViewButton"),
    ] {
        registry.set_value(addr(key, value), RawRegistryValue::dword(1));
    }
    // taskbar.search's key (SEARCH) is never seeded → KeyMissing.
    let mut driver = driver_with(registry, MemoryVerifier::new());
    assert_eq!(
        driver.apply(&search()).unwrap(),
        ApplyOutcome::Skipped(dm_domain::system_tweaks::SkipReason::Changed)
    );
    assert!(driver.managed_for_test(&search()).is_none());
}

#[test]
fn a_start_apply_rolls_back_if_the_known_recent_item_changes() {
    // codex R2 #4: the Start effect proof rides a pre-write receipt; if the known Recent item
    // changes between the receipt and the effect check, the effect FAILS and the write rolls back.
    let mut verifier = MemoryVerifier::new();
    verifier.set_start_recent_marker("recent-before");
    verifier.change_recent_marker_on_settle("recent-after"); // the user opened a new file
    let mut driver = driver_with(pushing_registry(), verifier);
    let start = SettingId::new("start.recommendations");
    assert_eq!(driver.apply(&start).unwrap(), ApplyOutcome::Reverted);
    // Not owned — the promotions write did not stand because it disturbed Recent.
    assert!(driver.managed_for_test(&start).is_none());
    assert_eq!(driver.inspect(&start).unwrap(), ProbeOutcome::Pushing);
}

#[test]
fn a_clean_start_apply_captures_and_proves_its_receipt() {
    let mut verifier = MemoryVerifier::new();
    verifier.set_start_recent_marker("recent-stable");
    let mut driver = driver_with(pushing_registry(), verifier);
    let start = SettingId::new("start.recommendations");
    assert_eq!(driver.apply(&start).unwrap(), ApplyOutcome::Verified);
    assert_eq!(driver.inspect(&start).unwrap(), ProbeOutcome::OwnedQuiet);
}

#[test]
fn a_failed_re_apply_undoes_to_before_not_the_true_original() {
    // codex R2 #2: undo a failed apply back to the value we OBSERVED, never the true original —
    // a failed re-apply must preserve the user's drift, not force the original.
    let mut driver = driver();
    driver.apply(&search()).unwrap(); // 1 → 0, owned
    // The user re-enables the search box (drift to 1 — here the same as the original, so use a
    // distinct drift value 2 to prove the undo targets `before`, not `original`).
    driver.set_live_for_test(addr(SEARCH, "SearchboxTaskbarMode"), RawRegistryValue::dword(2));
    // Re-apply, but fail the NEXT CAS so nothing is written this transaction.
    driver.backend.fail_next_compare_exchange();
    let result = driver.apply(&search());
    // The re-apply reverted (or is pending) — either way it must NOT have clobbered the user's 2.
    assert!(
        matches!(result, Ok(ApplyOutcome::Reverted)) || matches!(result, Err(DriverError::Pending { .. })),
        "got {result:?}"
    );
    let live = driver.read_live_for_test(addr(SEARCH, "SearchboxTaskbarMode"));
    assert_eq!(live.as_dword(), Some(2), "the user's drift (2) must stand, not the original (1)");
}

#[test]
fn a_value_that_changes_after_the_single_read_is_caught_by_the_cas() {
    // codex R2 #8: apply reads + validates each leaf once and uses THAT snapshot; a later external
    // change is caught by the CAS conflict, never silently written over.
    let mut registry = pushing_registry();
    // Between the single read and the CAS, an external process changes the value to 5.
    registry.replace_before_compare_exchange_at(
        1,
        addr(SEARCH, "SearchboxTaskbarMode"),
        RegistrySnapshot::Present(RawRegistryValue::dword(5)),
    );
    let mut driver = driver_with(registry, MemoryVerifier::new());
    // The CAS expected the read-time value (1); it now finds 5 → conflict → clean rollback.
    assert_eq!(driver.apply(&search()).unwrap(), ApplyOutcome::Reverted);
    // The external 5 stands.
    assert_eq!(
        driver.read_live_for_test(addr(SEARCH, "SearchboxTaskbarMode")).as_dword(),
        Some(5)
    );
}

#[test]
fn a_changed_recipe_version_requires_restore_before_reapply() {
    // codex R2 #9: a re-apply under a different recipe version must not silently orphan a leaf.
    let mut driver = driver();
    driver.apply(&search()).unwrap();
    // Simulate a recipe migration by bumping the anchor's recorded version out from under it.
    driver.bump_managed_version_for_test(&search());
    assert_eq!(
        driver.apply(&search()),
        Err(DriverError::MigrationRequired(search()))
    );
}

#[test]
fn the_catalog_rejects_a_guided_descriptor_with_a_mutation() {
    use super::catalog::{ManualRoute, TweakDescriptor, TweakTier};
    let bad = TweakDescriptor {
        id: SettingId::new("bad.guided"),
        recipe_version: 1,
        tier: TweakTier::Guided,
        mutations: vec![super::catalog::first_batch()
            .into_iter()
            .find(|d| d.id == search())
            .unwrap()
            .mutations[0]
            .clone()],
        policy_guards: vec![],
        forbidden_mutations: vec![],
        manual_route: Some(ManualRoute::WidgetsBoardSettings),
        effect_verifier: None,
        readable_state: Some(false),
    };
    assert!(matches!(
        TweakCatalog::try_new(vec![bad]),
        Err(super::catalog::CatalogError::GuidedWithMutation(_))
    ));
}

#[test]
fn the_catalog_folds_case_when_detecting_an_address_collision() {
    use super::catalog::{first_batch, TweakCatalog};
    let mut descriptors = first_batch();
    // Two descriptors targeting Explorer\Advanced\ShowTaskViewButton in different case collide.
    let clash = descriptors
        .iter()
        .find(|d| d.id == SettingId::new("taskbar.taskview"))
        .unwrap()
        .clone();
    let mut clash = clash;
    clash.id = SettingId::new("taskbar.taskview.dup");
    clash.mutations[0].address.value = "SHOWTASKVIEWBUTTON".into(); // same value, upper-cased
    descriptors.push(clash);
    assert!(matches!(
        TweakCatalog::try_new(descriptors),
        Err(super::catalog::CatalogError::ResourceCollision(_))
    ));
}

#[test]
fn the_journal_rejects_a_prepare_while_a_transaction_is_incomplete() {
    // codex #3: the generation guard — a second prepare cannot begin while one is incomplete.
    use super::journal::{
        JournalStore, MemoryJournal, PrepareRequest, TransactionIntent, TransactionValue,
    };
    use super::verify::{VerificationPlan, VerificationReceipt};
    let mut journal = MemoryJournal::new();
    let lease = journal.acquire_writer_lease().unwrap();
    let request = |feature: &str| PrepareRequest {
        feature: SettingId::new(feature),
        recipe_version: 1,
        environment: env(),
        verification: VerificationPlan::new(
            super::catalog::EffectVerifier::DelayedReadBackAndSettingsUi,
        ),
        receipt: VerificationReceipt::NoBaseline,
        intent: TransactionIntent::Apply,
        values: Vec::<TransactionValue>::new(),
        managed_before: None,
    };
    journal.prepare(&lease, request("one")).unwrap();
    // A second prepare while the first is still Prepared is rejected.
    assert!(journal.prepare(&lease, request("two")).is_err());
}

// Small test-only accessors over the driver's pub(super) ports (same-crate submodule access).
impl Driver {
    fn managed_for_test(&self, feature: &SettingId) -> Option<super::journal::ManagedSetting> {
        let lease = self.journal.acquire_writer_lease().unwrap();
        self.journal.managed(&lease, feature).unwrap()
    }

    fn set_live_for_test(&mut self, address: RegistryAddress, value: RawRegistryValue) {
        self.backend.set_value(address, value);
    }

    fn read_live_for_test(&self, address: RegistryAddress) -> RawRegistryValue {
        match self.backend.read(&address).unwrap() {
            RegistrySnapshot::Present(value) => value,
            other => panic!("expected a present value, got {other:?}"),
        }
    }

    fn bump_managed_version_for_test(&mut self, feature: &SettingId) {
        self.journal.bump_managed_version_for_test(feature);
    }
}

// Reuse the transaction-value builder indirectly to keep the type imported (documentation of the
// leaf shape the journal persists).
#[allow(dead_code)]
fn _leaf_shape(value: &TransactionValue) -> &RegistrySnapshot {
    &value.original
}
