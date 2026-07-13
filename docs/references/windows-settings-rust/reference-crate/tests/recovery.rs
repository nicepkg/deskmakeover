mod common;

use common::*;
use deskmakeover_windows_settings_reference::*;

#[test]
fn interrupted_apply_has_prepared_journal_and_recovers_to_original() {
    let feature = "taskbarWidgets";
    let target = address("TaskbarDa");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    backend.interrupt_after_additional_writes(1);
    let mut engine = engine(
        definition(feature, vec![("TaskbarDa", dword(0))]),
        standard_rule(),
        backend,
    );

    let error = engine.apply(apply_request(&engine, feature)).unwrap_err();
    assert!(matches!(error, EngineError::Interrupted { transaction: 1 }));
    // Returning immediately after the backend's post-write interruption proves prepare preceded it.
    assert_eq!(engine.fake_journal().prepared_count(), 1);
    assert_eq!(engine.fake_registry().snapshot(&target), dword(0));
    let writes_before_retry = engine.fake_registry().successful_write_count();
    assert!(matches!(
        engine.apply(apply_request(&engine, feature)),
        Err(EngineError::RecoveryRequired(transactions)) if transactions == vec![1]
    ));
    assert_eq!(
        engine.fake_registry().successful_write_count(),
        writes_before_retry
    );

    let entry = engine.fake_journal().entry(1).expect("prepared entry");
    assert_eq!(entry.recipe_version, 1);
    assert_eq!(entry.environment, environment());
    assert_eq!(
        entry.verification,
        VerificationPlan::new(EffectVerifier::DelayedReadBackAndSettingsUi)
    );

    engine.fake_registry_mut().clear_faults();
    let report = engine.recover().unwrap();
    assert_eq!(report.recovered, vec![1]);
    assert!(report.conflicts.is_empty());
    assert_eq!(engine.fake_registry().snapshot(&target), dword(1));
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_none());
}

#[test]
fn interrupted_before_creation_confirmation_restores_leaf_but_conservatively_keeps_key() {
    let feature = "createdKeyCrashRecovery";
    let root_path = r"Software\DeskMakeoverReference\ExistingRoot";
    let owned_path = format!(r"{root_path}\CreatedByApply");
    let target = address_in(&owned_path, "Enabled");
    let owned_key = key(&owned_path);
    let mut backend = MemoryRegistry::default();
    backend.ensure_key(key(root_path));
    backend.interrupt_after_additional_writes(1);
    let mut engine = engine(
        definition_at(feature, vec![(target.clone(), dword(0))]),
        standard_rule(),
        backend,
    );

    assert!(matches!(
        engine.apply(apply_request(&engine, feature)),
        Err(EngineError::Interrupted { transaction: 1 })
    ));
    let entry = engine.fake_journal().entry(1).unwrap();
    assert_eq!(entry.candidate_keys, vec![owned_key.clone()]);
    assert!(entry.confirmed_created_keys.is_empty());
    assert!(engine.fake_registry().key_is_present(&owned_key));
    assert_eq!(engine.fake_registry().snapshot(&target), dword(0));

    engine.fake_registry_mut().clear_faults();
    let report = engine.recover().unwrap();
    assert_eq!(report.recovered, vec![1]);
    assert!(report.conflicts.is_empty());
    assert!(engine.fake_registry().key_is_present(&owned_key));
    assert_eq!(
        engine.fake_registry().snapshot(&target),
        RegistrySnapshot::ValueMissing
    );
    assert_eq!(engine.fake_journal().prepared_count(), 0);
}

#[test]
fn recovery_keeps_prepared_barrier_when_terminal_effect_verification_fails() {
    let feature = "taskbarWidgets";
    let target = address("TaskbarDa");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    backend.interrupt_after_additional_writes(1);
    let mut engine = engine(
        definition(feature, vec![("TaskbarDa", dword(0))]),
        standard_rule(),
        backend,
    );
    assert!(matches!(
        engine.apply(apply_request(&engine, feature)),
        Err(EngineError::Interrupted { transaction: 1 })
    ));

    engine.fake_registry_mut().clear_faults();
    engine
        .fake_verifier_mut()
        .fail_next_effect("shell still exposed the applied state");
    let first_recovery = engine.recover().unwrap();
    assert!(first_recovery.recovered.is_empty());
    assert_eq!(first_recovery.conflicts.len(), 1);
    assert_eq!(engine.fake_registry().snapshot(&target), dword(1));
    assert_eq!(engine.fake_journal().prepared_count(), 1);
    assert_eq!(
        engine.fake_journal().entry(1).unwrap().state,
        TransactionState::Prepared
    );
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_none());

    let second_recovery = engine.recover().unwrap();
    assert_eq!(second_recovery.recovered, vec![1]);
    assert!(second_recovery.conflicts.is_empty());
    assert_eq!(engine.fake_journal().prepared_count(), 0);
    assert_eq!(
        engine.fake_journal().entry(1).unwrap().state,
        TransactionState::RolledBack
    );
    assert_eq!(
        engine
            .fake_verifier()
            .effect_invocations()
            .iter()
            .map(|invocation| invocation.phase)
            .collect::<Vec<_>>(),
        vec![
            VerificationPhase::ApplyRollback,
            VerificationPhase::ApplyRollback
        ]
    );
}

#[test]
fn failed_atomic_commit_leaves_prepared_barrier_and_recovers_original() {
    let feature = "searchHighlights";
    let target = address("SearchHighlights");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(target.clone(), dword(1));
    let mut engine = engine(
        definition(feature, vec![("SearchHighlights", dword(0))]),
        standard_rule(),
        backend,
    );
    engine.fake_journal_mut().fail_next_atomic_commit();

    assert!(matches!(
        engine.apply(apply_request(&engine, feature)),
        Err(EngineError::Journal(_))
    ));
    assert_eq!(engine.fake_registry().snapshot(&target), dword(0));
    assert_eq!(engine.fake_journal().prepared_count(), 1);
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_none());
    assert!(matches!(
        engine.apply(apply_request(&engine, feature)),
        Err(EngineError::RecoveryRequired(_))
    ));

    engine.recover().unwrap();
    assert_eq!(engine.fake_registry().snapshot(&target), dword(1));
    assert_eq!(engine.fake_journal().prepared_count(), 0);
}

#[test]
fn prepare_rejects_a_stale_managed_generation_without_creating_an_entry() {
    let feature = "searchHighlights";
    let mut engine = engine(
        definition(feature, vec![("SearchHighlights", dword(0))]),
        standard_rule(),
        MemoryRegistry::default(),
    );
    engine.apply(apply_request(&engine, feature)).unwrap();
    let (_, _, mut journal, _, _) = engine.into_fake_parts();
    let current = journal.managed(&id(feature)).unwrap().unwrap();
    let mut stale = current.clone();
    stale.last_transaction += 1;

    let lease = journal.acquire_writer_lease().unwrap();
    let error = journal
        .prepare(
            &lease,
            id(feature),
            current.recipe_version,
            environment(),
            current.verification.clone(),
            VerificationReceipt::NoBaseline,
            TransactionIntent::Restore,
            Vec::new(),
            Vec::new(),
            current.cleanup_owned_keys.clone(),
            Some(stale),
        )
        .unwrap_err();

    assert!(error.0.contains("generation changed"));
    assert!(journal.incomplete(&lease).unwrap().is_empty());
    assert_eq!(journal.managed(&id(feature)).unwrap(), Some(current));
}

#[test]
fn interrupted_restore_is_forward_recovered_to_exact_original() {
    let feature = "startPromotedRecommendations";
    let first = address("PromotedA");
    let second = address("PromotedB");
    let mut backend = MemoryRegistry::default();
    backend.set_snapshot(first.clone(), RegistrySnapshot::ValueMissing);
    backend.set_snapshot(second.clone(), dword(9));
    let mut engine = engine(
        definition(
            feature,
            vec![("PromotedA", dword(0)), ("PromotedB", dword(0))],
        ),
        standard_rule(),
        backend,
    );
    engine.apply(apply_request(&engine, feature)).unwrap();
    engine
        .fake_registry_mut()
        .interrupt_after_additional_writes(1);

    let error = engine
        .restore(restore_request(&engine, feature))
        .unwrap_err();
    assert!(matches!(error, EngineError::Interrupted { transaction: 2 }));
    assert_eq!(engine.fake_journal().prepared_count(), 1);

    engine.fake_registry_mut().clear_faults();
    let report = engine.recover().unwrap();
    assert_eq!(report.recovered, vec![2]);
    assert_eq!(
        engine.fake_registry().snapshot(&first),
        RegistrySnapshot::ValueMissing
    );
    assert_eq!(engine.fake_registry().snapshot(&second), dword(9));
    assert!(engine
        .fake_journal()
        .managed(&id(feature))
        .unwrap()
        .is_none());
    assert_eq!(
        engine
            .fake_verifier()
            .effect_invocations()
            .iter()
            .map(|invocation| invocation.phase)
            .collect::<Vec<_>>(),
        vec![
            VerificationPhase::ApplyDesired,
            VerificationPhase::RestoreOriginal
        ]
    );
}
