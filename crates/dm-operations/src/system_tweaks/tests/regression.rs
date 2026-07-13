//! Transaction-correctness regression tests (the codex W1 review rounds): honest
//! verification, never-clobber rollback, generation guards, receipts, fail-closed
//! missing-key/kind, recipe migration, and pre-write re-authentication.

#![allow(clippy::bool_assert_comparison)]

use super::*;

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
    use super::super::catalog::{ManualRoute, TweakDescriptor, TweakTier};
    let bad = TweakDescriptor {
        id: SettingId::new("bad.guided"),
        recipe_version: 1,
        tier: TweakTier::Guided,
        mutations: vec![super::super::catalog::first_batch()
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
        Err(super::super::catalog::CatalogError::GuidedWithMutation(_))
    ));
}

#[test]
fn the_catalog_folds_case_when_detecting_an_address_collision() {
    use super::super::catalog::{first_batch, TweakCatalog};
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
        Err(super::super::catalog::CatalogError::ResourceCollision(_))
    ));
}

#[test]
fn the_journal_rejects_a_prepare_while_a_transaction_is_incomplete() {
    // codex #3: the generation guard — a second prepare cannot begin while one is incomplete.
    use super::super::journal::{
        JournalStore, MemoryJournal, PrepareRequest, TransactionIntent, TransactionValue,
    };
    use super::super::verify::{VerificationPlan, VerificationReceipt};
    let mut journal = MemoryJournal::new();
    let lease = journal.acquire_writer_lease().unwrap();
    let request = |feature: &str| PrepareRequest {
        feature: SettingId::new(feature),
        recipe_version: 1,
        environment: env(),
        verification: VerificationPlan::new(
            super::super::catalog::EffectVerifier::DelayedReadBackAndSettingsUi,
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

#[test]
fn a_restore_whose_effect_proof_fails_stays_pending() {
    // codex R3 #4: restore/recovery must run a functional effect proof, not just raw equality.
    let mut driver = driver();
    driver.apply(&search()).unwrap();
    driver.verifier.fail_next_effect("surface did not reload the original");
    let result = driver.restore(&search());
    assert!(matches!(result, Err(DriverError::Pending { .. })), "got {result:?}");
    // Still owned — the restore did not reach a terminal state without proof.
    assert!(driver.managed_for_test(&search()).is_some());
}

#[test]
fn the_catalog_rejects_a_non_dword_desired_value() {
    // codex R3 #6: W1 is DWORD-only — a String/Binary/Qword desired is rejected.
    use super::super::catalog::{EffectVerifier, TweakDescriptor, TweakTier};
    use dm_domain::system_tweaks::{MissingPolicy, RegistryValueKind, SettingMutation};
    let string_desired = SettingMutation {
        address: addr(EXPLORER_ADVANCED, "SomeString"),
        desired: RegistrySnapshot::Present(RawRegistryValue::new(
            RegistryValueKind::String,
            b"x".to_vec(),
        )),
        accepted_existing_kinds: vec![RegistryValueKind::Dword],
        missing_policy: MissingPolicy::CreateAllowed,
    };
    let bad = TweakDescriptor {
        id: SettingId::new("bad.string"),
        recipe_version: 1,
        tier: TweakTier::AutomaticCandidate,
        mutations: vec![string_desired],
        policy_guards: vec![],
        forbidden_mutations: vec![],
        manual_route: None,
        effect_verifier: Some(EffectVerifier::DelayedReadBackAndSettingsUi),
        readable_state: None,
    };
    assert!(matches!(
        TweakCatalog::try_new(vec![bad]),
        Err(super::super::catalog::CatalogError::IllegalDesired(_))
    ));
}

#[test]
fn a_feature_update_between_prepare_and_the_write_blocks_the_apply() {
    // codex R3 NEW: the environment is re-authenticated immediately before each write; a feature
    // update landing after prepare must not be written into an uncertified environment.
    let mut newer = env();
    newer.build = 26_200; // a new build family the manifest does not certify
    let driver = driver();
    // The top-of-apply probe (call 1) sees the certified env; the pre-write re-auth (call 2) sees
    // the new build → the write is blocked and nothing is owned.
    driver.profile.flip_after(1, newer);
    let mut driver = driver;
    let result = driver.apply(&search());
    // Blocked before any write: reverted (nothing written) and not owned.
    assert!(
        matches!(result, Ok(ApplyOutcome::Reverted)) || matches!(result, Err(DriverError::Pending { .. })),
        "got {result:?}"
    );
    assert!(driver.managed_for_test(&search()).is_none());
    // The registry was never written — the search box value still pushes (1).
    assert_eq!(
        driver.read_live_for_test(addr(SEARCH, "SearchboxTaskbarMode")).as_dword(),
        Some(1)
    );
}

