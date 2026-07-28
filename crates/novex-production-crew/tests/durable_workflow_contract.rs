use chrono::{Duration, TimeZone, Utc};
use novex_production_crew::durable::{
    canonical_digest,
    command_store::{ProductionAggregateType, ProductionCommandStore, ProductionCommandType},
    media::{
        build_required_take_inventory, media_review_readiness, quality_coverage, ComposeInput,
        ContinuityEvidence, FinalMediaAsset, MediaCapability, MediaEvidence, MediaEvidenceSnapshot,
        RequiredTakeInventorySnapshot, TakeEvidence, TakeReviewEvidence,
    },
    package::{
        ArtifactPackageSnapshot, ArtifactRef, GateDecision, GateDecisionBook, PackageType,
        ProductionCharacterRef, ProductionPackageMetadata, ProductionPerformanceRef,
        ProductionSceneRef, ProductionShotRef, ProductionSoundRef,
    },
    plan::{FullCrewPlanRegistry, ResourceLimits, StepKind},
    resource::{ResourceRequest, ResourceSafetyGate, ResourceUsageLedger},
    script::{map_script_draft, ScriptDraftInput, ScriptSceneInput},
    state_machine::{
        allowed_commands, derive_revision, external_work_decision, external_work_state,
        unlockable_steps, validate_intent_command, validate_workflow_command, CommandEnvelope,
        IntentCommand, IntentHistory, RevisionCause, RevisionOutcome, RunState, RunStatus,
        SideEffectState, StepState, StepStatus, WaitingReason, WorkGenerationStatus,
        WorkflowCommand, WorkflowCommandKind, WorkflowSnapshot,
    },
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn production_command_digest_and_names_are_canonical_and_explicit() {
    let first = json!({
        "run_id": Uuid::nil(),
        "package_digest": format!("{:064x}", 1),
        "reason": "same request",
    });
    let second = serde_json::from_str::<serde_json::Value>(&format!(
        r#"{{"reason":"same request","package_digest":"{:064x}","run_id":"{}"}}"#,
        1,
        Uuid::nil()
    ))
    .unwrap();

    assert_eq!(
        ProductionCommandStore::canonical_request_digest(&first).unwrap(),
        ProductionCommandStore::canonical_request_digest(&second).unwrap(),
    );
    assert_eq!(
        ProductionCommandType::ApprovePackage.as_str(),
        "approve_package"
    );
    assert_eq!(
        ProductionCommandType::RejectPackage.as_str(),
        "reject_package"
    );
    assert_eq!(ProductionCommandType::CancelRun.as_str(), "cancel_run");
    assert_eq!(
        ProductionCommandType::PromoteScript.as_str(),
        "promote_script"
    );
    assert_eq!(
        ProductionCommandType::QualityRework.as_str(),
        "quality_rework"
    );
    assert_eq!(
        ProductionAggregateType::ProductionRun.as_str(),
        "production_run"
    );
}

fn digest(seed: &str) -> String {
    canonical_digest(&json!({"seed": seed})).unwrap()
}

fn production_package_fixture(
    run_id: Uuid,
    step_id: Uuid,
) -> (Vec<ArtifactRef>, ProductionPackageMetadata) {
    let script_id = Uuid::new_v5(&run_id, b"script");
    let scene_a = Uuid::new_v5(&run_id, b"scene-a");
    let scene_b = Uuid::new_v5(&run_id, b"scene-b");
    let character_a = Uuid::new_v5(&run_id, b"character-a");
    let character_b = Uuid::new_v5(&run_id, b"character-b");
    let treatment_id = Uuid::new_v5(&run_id, b"treatment");
    let shot_a = Uuid::new_v5(&run_id, b"shot-a");
    let shot_b = Uuid::new_v5(&run_id, b"shot-b");
    let performance_a = Uuid::new_v5(&run_id, b"performance-a");
    let performance_b = Uuid::new_v5(&run_id, b"performance-b");
    let sound_id = Uuid::new_v5(&run_id, b"sound");
    let artifact = |artifact_type: &str, artifact_id: Uuid, seed: &str| ArtifactRef {
        run_id,
        artifact_type: artifact_type.into(),
        artifact_id,
        version: 1,
        content_digest: digest(seed),
        source_step_id: step_id,
        source_attempt: 1,
    };
    let items = vec![
        artifact("directorial_treatment", treatment_id, "treatment"),
        artifact("shot_contract", shot_a, "shot-a"),
        artifact("shot_contract", shot_b, "shot-b"),
        artifact("performance_brief", performance_a, "performance-a"),
        artifact("performance_brief", performance_b, "performance-b"),
        artifact("sound_plan", sound_id, "sound"),
    ];
    let metadata = ProductionPackageMetadata {
        script_id,
        script_version: script_id.to_string(),
        script_digest: digest("script"),
        scenes: vec![
            ProductionSceneRef {
                scene_id: scene_a,
                scene_version: scene_a.to_string(),
                scene_digest: digest("scene-a"),
                sequence: 1,
                duration_sec: 5,
                character_bible_ids: vec![character_a],
            },
            ProductionSceneRef {
                scene_id: scene_b,
                scene_version: scene_b.to_string(),
                scene_digest: digest("scene-b"),
                sequence: 2,
                duration_sec: 5,
                character_bible_ids: vec![character_a, character_b],
            },
        ],
        characters: vec![
            ProductionCharacterRef {
                character_bible_id: character_a,
                character_id: "lead".into(),
            },
            ProductionCharacterRef {
                character_bible_id: character_b,
                character_id: "mentor".into(),
            },
        ],
        shots: vec![
            ProductionShotRef {
                artifact_id: shot_a,
                shot_id: "shot-1".into(),
                sequence: 1,
                scene_id: scene_a,
                duration_sec: 5,
                character_bible_ids: vec![character_a],
            },
            ProductionShotRef {
                artifact_id: shot_b,
                shot_id: "shot-2".into(),
                sequence: 2,
                scene_id: scene_b,
                duration_sec: 5,
                character_bible_ids: vec![character_a, character_b],
            },
        ],
        performance_briefs: vec![
            ProductionPerformanceRef {
                artifact_id: performance_a,
                script_id,
                character_bible_id: character_a,
                character_id: "lead".into(),
                scene_ids: vec![scene_a, scene_b],
            },
            ProductionPerformanceRef {
                artifact_id: performance_b,
                script_id,
                character_bible_id: character_b,
                character_id: "mentor".into(),
                scene_ids: vec![scene_b],
            },
        ],
        sound_plan: ProductionSoundRef {
            artifact_id: sound_id,
            script_id,
            scene_ids: vec![scene_a, scene_b],
        },
        suggestion_resolutions: vec![],
    };
    (items, metadata)
}

fn fixed_plan() -> novex_production_crew::durable::plan::PlanSnapshot {
    FullCrewPlanRegistry::snapshot_v1(
        false,
        json!({"producer": {"definition": "production.producer@1"}}),
        ResourceLimits::strict_default(),
    )
    .unwrap()
}

#[test]
fn full_crew_v1_plan_is_fixed_versioned_and_deterministic() {
    let limits = ResourceLimits::strict_default();
    let bindings = json!({
        "producer": {"definition": "production.producer@1", "model_id": Uuid::nil()},
        "screenwriter": {"definition": "production.screenwriter@1", "model_id": Uuid::nil()}
    });
    let first = FullCrewPlanRegistry::snapshot_v1(true, bindings.clone(), limits.clone()).unwrap();
    let second = FullCrewPlanRegistry::snapshot_v1(true, bindings, limits).unwrap();

    assert_eq!(first.plan_key, "full_crew");
    assert_eq!(first.plan_version, "1.0.0");
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.steps.first().unwrap().key, "validate_source");
    assert_eq!(first.steps.last().unwrap().key, "quality_gate");
    assert!(first.steps.iter().any(|step| {
        step.key == "character_critic" && step.kind == StepKind::Role && step.optional
    }));

    let performance = first.step("performance_director").unwrap();
    let sound = first.step("sound_director").unwrap();
    assert_eq!(performance.dependencies, sound.dependencies);
    assert_eq!(performance.dependencies, vec!["director_revision"]);
    assert!(first.max_package_revisions.values().all(|value| *value > 0));
    assert!(first.max_quality_reworks > 0);

    let without_critic = FullCrewPlanRegistry::snapshot_v1(
        false,
        json!({"producer": {"definition": "production.producer@1"}}),
        ResourceLimits::strict_default(),
    )
    .unwrap();
    assert!(!without_critic
        .steps
        .iter()
        .any(|step| step.key == "character_critic"));
    assert_ne!(without_critic.digest, first.digest);

    let mut tampered = first.clone();
    tampered.steps.swap(0, 1);
    assert!(tampered.validate_frozen().is_err());
}

#[test]
fn state_machine_rejects_skips_and_unknown_side_effect_reclaim() {
    let now = Utc.with_ymd_and_hms(2026, 7, 27, 8, 0, 0).unwrap();
    let mut step = StepState::queued(Uuid::new_v4(), "producer", StepKind::Role, 0);
    assert!(step.transition(StepStatus::Succeeded).is_err());

    step.claim("worker-a", now, Duration::seconds(30)).unwrap();
    assert_eq!(step.status, StepStatus::Running);
    assert!(step.claim("worker-b", now, Duration::seconds(30)).is_err());
    step.side_effect_state = SideEffectState::Unknown;
    assert!(step
        .reclaim_expired(
            "worker-b",
            now + Duration::seconds(31),
            Duration::seconds(30)
        )
        .is_err());
    assert_eq!(step.status, StepStatus::AttentionRequired);

    let waiting =
        StepState::waiting_approval(Uuid::new_v4(), "script_package_approval", StepKind::Gate, 0);
    assert!(waiting.clone().transitioned(StepStatus::Running).is_err());
    assert!(waiting.transitioned(StepStatus::Queued).is_ok());
}

#[test]
fn step_transitions_are_kind_specific_and_terminal_states_are_immutable() {
    let cases = [
        (
            StepKind::Role,
            StepStatus::Queued,
            StepStatus::Running,
            true,
        ),
        (
            StepKind::Role,
            StepStatus::Queued,
            StepStatus::WaitingApproval,
            false,
        ),
        (
            StepKind::Gate,
            StepStatus::Queued,
            StepStatus::WaitingApproval,
            true,
        ),
        (
            StepKind::Gate,
            StepStatus::WaitingApproval,
            StepStatus::Succeeded,
            true,
        ),
        (
            StepKind::DomainCommand,
            StepStatus::Queued,
            StepStatus::Running,
            true,
        ),
        (
            StepKind::ExternalWait,
            StepStatus::Queued,
            StepStatus::ExternalWait,
            true,
        ),
        (
            StepKind::ExternalWait,
            StepStatus::ExternalWait,
            StepStatus::Succeeded,
            true,
        ),
        (
            StepKind::Role,
            StepStatus::Succeeded,
            StepStatus::Queued,
            false,
        ),
        (
            StepKind::DomainCommand,
            StepStatus::Cancelled,
            StepStatus::Queued,
            false,
        ),
    ];

    for (kind, current, target, expected) in cases {
        let mut step = StepState::queued(Uuid::new_v4(), "table", kind, 0);
        step.status = current;
        assert_eq!(
            step.transition(target).is_ok(),
            expected,
            "unexpected transition for {kind:?}: {current:?} -> {target:?}"
        );
    }
}

#[test]
fn run_transitions_reject_skips_and_cancellation_cannot_claim_success_early() {
    let cases = [
        (RunStatus::Created, RunStatus::Queued, true),
        (RunStatus::Created, RunStatus::Completed, false),
        (RunStatus::Queued, RunStatus::Running, true),
        (RunStatus::Running, RunStatus::WaitingApproval, true),
        (RunStatus::WaitingApproval, RunStatus::Queued, true),
        (RunStatus::ExternalWait, RunStatus::AttentionRequired, true),
        (RunStatus::Running, RunStatus::Cancelling, true),
        (RunStatus::Cancelling, RunStatus::Cancelled, true),
        (RunStatus::Cancelling, RunStatus::Completed, false),
        (RunStatus::Completed, RunStatus::Running, false),
        (RunStatus::Cancelled, RunStatus::Queued, false),
    ];

    for (current, target, expected) in cases {
        let mut run = RunState::new(current, 0);
        assert_eq!(
            run.transition(target).is_ok(),
            expected,
            "unexpected run transition: {current:?} -> {target:?}"
        );
    }
}

#[test]
fn workflow_commands_validate_frozen_plan_dependencies_epoch_and_idempotency() {
    let plan = fixed_plan();
    let mut validate_source = StepState::queued(
        Uuid::new_v4(),
        "validate_source",
        StepKind::DomainCommand,
        0,
    );
    validate_source.status = StepStatus::Succeeded;
    let mut producer = StepState::queued(Uuid::new_v4(), "producer", StepKind::Role, 0);
    producer.status = StepStatus::Blocked;
    let snapshot = WorkflowSnapshot::new(
        RunState::new(RunStatus::Queued, 0),
        vec![validate_source.clone(), producer.clone()],
    );

    assert_eq!(
        unlockable_steps(&plan, &snapshot).unwrap(),
        vec!["producer"]
    );
    let unlocked = snapshot.with_dependency_unlocks(&plan).unwrap();
    let envelope = CommandEnvelope {
        plan_key: plan.plan_key.clone(),
        plan_version: plan.plan_version.clone(),
        plan_digest: plan.digest.clone(),
        idempotency_key: "execute-producer".into(),
        command: WorkflowCommand::step(WorkflowCommandKind::ExecuteStep, "producer"),
    };
    validate_workflow_command(&plan, &unlocked, &envelope).unwrap();
    assert!(allowed_commands(&plan, &unlocked)
        .unwrap()
        .contains(&envelope.command));

    let mut old_producer = producer.clone();
    old_producer.status = StepStatus::Succeeded;
    let mut current_validate = validate_source.clone();
    current_validate.revision_epoch = 1;
    let mut current_producer = producer.clone();
    current_producer.revision_epoch = 1;
    let revised = WorkflowSnapshot::new(
        RunState::new(RunStatus::Queued, 1),
        vec![
            validate_source.clone(),
            old_producer,
            current_validate,
            current_producer,
        ],
    )
    .with_dependency_unlocks(&plan)
    .unwrap();
    assert_eq!(revised.steps[1].status, StepStatus::Succeeded);
    assert_eq!(revised.steps[3].status, StepStatus::Queued);

    let mut bad_plan = envelope.clone();
    bad_plan.plan_digest = digest("other-plan");
    assert!(validate_workflow_command(&plan, &unlocked, &bad_plan).is_err());
    let mut blank_key = envelope.clone();
    blank_key.idempotency_key = "  ".into();
    assert!(validate_workflow_command(&plan, &unlocked, &blank_key).is_err());
    let stale_epoch = WorkflowSnapshot::new(
        RunState::new(RunStatus::Queued, 1),
        vec![validate_source, producer],
    );
    assert!(validate_workflow_command(&plan, &stale_epoch, &envelope).is_err());

    let cancelling = WorkflowSnapshot::new(RunState::cancelling(0), unlocked.steps.clone());
    assert!(unlockable_steps(&plan, &cancelling).unwrap().is_empty());
    assert!(validate_workflow_command(&plan, &cancelling, &envelope).is_err());
    assert_eq!(
        allowed_commands(&plan, &cancelling).unwrap(),
        Vec::<WorkflowCommand>::new()
    );
}

#[test]
fn waiting_reasons_and_commands_are_derived_without_io() {
    let plan = fixed_plan();
    let mut gate =
        StepState::waiting_approval(Uuid::new_v4(), "script_package_approval", StepKind::Gate, 0);
    gate.waiting_reason = Some(WaitingReason::PackageApproval);
    let mut visual = StepState::queued(
        Uuid::new_v4(),
        "wait_scene_visual_manifest",
        StepKind::ExternalWait,
        0,
    );
    visual.status = StepStatus::ExternalWait;
    visual.waiting_reason = Some(WaitingReason::SceneVisualManifest);
    let snapshot = WorkflowSnapshot::new(
        RunState::new(RunStatus::WaitingApproval, 0),
        vec![gate.clone(), visual.clone()],
    );

    assert_eq!(
        gate.derived_waiting_reason(),
        Some(WaitingReason::PackageApproval)
    );
    assert_eq!(
        visual.derived_waiting_reason(),
        Some(WaitingReason::SceneVisualManifest)
    );
    let commands = allowed_commands(&plan, &snapshot).unwrap();
    assert!(commands.contains(&WorkflowCommand::step(
        WorkflowCommandKind::ApprovePackage,
        "script_package_approval"
    )));
    assert!(commands.contains(&WorkflowCommand::step(
        WorkflowCommandKind::RejectPackage,
        "script_package_approval"
    )));
    assert!(commands.contains(&WorkflowCommand::step(
        WorkflowCommandKind::Resume,
        "wait_scene_visual_manifest"
    )));
    assert!(commands.contains(&WorkflowCommand::run(WorkflowCommandKind::CancelRun)));
}

#[test]
fn revision_directives_are_fixed_bounded_and_keep_old_epochs_immutable() {
    let plan = fixed_plan();
    let cases = [
        (
            RevisionCause::BriefRejected,
            vec!["producer"],
            "brief",
            false,
        ),
        (
            RevisionCause::ScriptRejected,
            vec!["screenwriter"],
            "script",
            false,
        ),
        (
            RevisionCause::ProductionRejected,
            vec!["director", "sound_director"],
            "production",
            false,
        ),
        (
            RevisionCause::ScriptSemanticChange,
            vec!["screenwriter"],
            "script",
            true,
        ),
    ];

    for (cause, owners, limit_key, invalidates_script) in cases {
        let outcome = derive_revision(&plan, 4, cause, &owners, 0, true).unwrap();
        let RevisionOutcome::Open(directive) = outcome else {
            panic!("revision should remain below limit");
        };
        assert_eq!(directive.next_epoch, 5);
        assert_eq!(directive.reopen_owners, owners);
        assert_eq!(directive.invalidates_formal_script, invalidates_script);

        let limit = plan.max_package_revisions[limit_key];
        assert_eq!(
            derive_revision(&plan, 4, cause, &owners, limit, true).unwrap(),
            RevisionOutcome::LimitReached
        );
    }

    assert!(derive_revision(
        &plan,
        1,
        RevisionCause::ScriptSemanticChange,
        &["screenwriter"],
        0,
        false,
    )
    .is_err());
    assert!(derive_revision(
        &plan,
        1,
        RevisionCause::ProductionRejected,
        &["screenwriter"],
        0,
        true,
    )
    .is_err());
    let expression = derive_revision(
        &plan,
        3,
        RevisionCause::ProductionExpressionChange,
        &["sound_director"],
        0,
        true,
    )
    .unwrap();
    let RevisionOutcome::Open(expression) = expression else {
        panic!("production expression revision should open");
    };
    assert!(!expression.invalidates_formal_script);
}

#[test]
fn external_terminal_mapping_is_fail_closed_and_reports_stable_waiting_reasons() {
    let cases = [
        (
            WorkGenerationStatus::Failed,
            false,
            false,
            false,
            RunStatus::Blocked,
            WaitingReason::ExternalFailure,
        ),
        (
            WorkGenerationStatus::WaitingManual,
            false,
            false,
            false,
            RunStatus::AttentionRequired,
            WaitingReason::ManualAttention,
        ),
        (
            WorkGenerationStatus::Cancelled,
            false,
            false,
            false,
            RunStatus::Blocked,
            WaitingReason::ExternalCancelConflict,
        ),
        (
            WorkGenerationStatus::Cancelling,
            false,
            false,
            true,
            RunStatus::Cancelling,
            WaitingReason::CancellationPending,
        ),
        (
            WorkGenerationStatus::Succeeded,
            false,
            false,
            false,
            RunStatus::Blocked,
            WaitingReason::EvidenceIncomplete,
        ),
    ];
    for (status, media, inventory, cancellation, expected_status, expected_reason) in cases {
        let decision = external_work_decision(status, media, inventory, cancellation);
        assert_eq!(decision.run_status, expected_status);
        assert_eq!(decision.waiting_reason, Some(expected_reason));
        assert!(!decision.unlocks_editor);
    }
    let success = external_work_decision(WorkGenerationStatus::Succeeded, true, true, false);
    assert_eq!(success.run_status, RunStatus::Running);
    assert!(success.waiting_reason.is_none());
    assert!(success.unlocks_editor);
}

#[test]
fn intent_delete_and_archive_commands_preserve_history() {
    let blank = IntentHistory::default();
    validate_intent_command(IntentCommand::Delete, &blank).unwrap();
    assert!(validate_intent_command(IntentCommand::Archive, &blank).is_err());

    let historical = IntentHistory {
        has_run: true,
        has_artifacts: true,
        has_domain_links: true,
        run_terminal: true,
    };
    assert!(validate_intent_command(IntentCommand::Delete, &historical).is_err());
    validate_intent_command(IntentCommand::Archive, &historical).unwrap();

    let active = IntentHistory {
        run_terminal: false,
        ..historical
    };
    assert!(validate_intent_command(IntentCommand::Archive, &active).is_err());
}

#[test]
fn external_work_states_never_treat_http_acceptance_or_missing_evidence_as_success() {
    assert_eq!(
        external_work_state(WorkGenerationStatus::Queued, false, false, false),
        RunStatus::ExternalWait
    );
    assert_eq!(
        external_work_state(WorkGenerationStatus::Succeeded, false, false, false),
        RunStatus::Blocked
    );
    assert_eq!(
        external_work_state(WorkGenerationStatus::WaitingManual, false, false, false),
        RunStatus::AttentionRequired
    );
    assert_eq!(
        external_work_state(WorkGenerationStatus::Cancelled, false, false, false),
        RunStatus::Blocked
    );
    assert_eq!(
        external_work_state(WorkGenerationStatus::Cancelled, false, false, true),
        RunStatus::Cancelled
    );
    assert_eq!(
        external_work_state(WorkGenerationStatus::Succeeded, true, true, false),
        RunStatus::Running
    );
}

#[test]
fn package_digest_is_stable_and_gate_is_bound_to_current_digest() {
    let run_id = Uuid::new_v4();
    let step_id = Uuid::new_v4();
    let a = ArtifactRef {
        run_id,
        artifact_type: "character_bible".into(),
        artifact_id: Uuid::new_v4(),
        version: 1,
        content_digest: digest("character"),
        source_step_id: step_id,
        source_attempt: 1,
    };
    let b = ArtifactRef {
        run_id,
        artifact_type: "script_draft".into(),
        artifact_id: Uuid::new_v4(),
        version: 1,
        content_digest: digest("script"),
        source_step_id: step_id,
        source_attempt: 1,
    };
    let c = ArtifactRef {
        run_id,
        artifact_type: "story_bible".into(),
        artifact_id: Uuid::new_v4(),
        version: 1,
        content_digest: digest("story"),
        source_step_id: step_id,
        source_attempt: 1,
    };

    let first = ArtifactPackageSnapshot::build(
        PackageType::Script,
        run_id,
        step_id,
        1,
        0,
        1,
        vec![b.clone(), a.clone(), c.clone()],
        json!({}),
    )
    .unwrap();
    let second = ArtifactPackageSnapshot::build(
        PackageType::Script,
        run_id,
        step_id,
        1,
        0,
        1,
        vec![c, a, b],
        json!({}),
    )
    .unwrap();
    assert_eq!(first.package_digest, second.package_digest);
    assert_eq!(first.items[0].artifact_type, "character_bible");

    let actor = "local_operator";
    let mut decisions = GateDecisionBook::default();
    let approved = decisions
        .decide(
            &first,
            &first.package_digest,
            GateDecision::Approve,
            actor,
            None,
            vec![],
        )
        .unwrap();
    assert_eq!(
        decisions
            .decide(
                &first,
                &first.package_digest,
                GateDecision::Approve,
                actor,
                None,
                vec![],
            )
            .unwrap()
            .id,
        approved.id
    );
    assert!(decisions
        .decide(
            &first,
            &first.package_digest,
            GateDecision::Reject,
            actor,
            Some("需要重写".into()),
            vec!["screenwriter".into()],
        )
        .is_err());
    assert!(decisions
        .decide(
            &first,
            &digest("stale"),
            GateDecision::Approve,
            actor,
            None,
            vec![],
        )
        .is_err());
}

#[test]
fn all_package_schemas_have_canonical_order_and_reject_incomplete_references() {
    let run_id = Uuid::new_v4();
    let step_id = Uuid::new_v4();
    let artifact = |artifact_type: &str, seed: &str| ArtifactRef {
        run_id,
        artifact_type: artifact_type.into(),
        artifact_id: Uuid::new_v5(&run_id, seed.as_bytes()),
        version: 1,
        content_digest: digest(seed),
        source_step_id: step_id,
        source_attempt: 1,
    };
    let (production_items, production_metadata) = production_package_fixture(run_id, step_id);
    let fixtures = vec![
        (
            PackageType::Brief,
            vec![artifact("creative_brief", "brief")],
            json!({}),
        ),
        (
            PackageType::Script,
            vec![
                artifact("script_draft", "script"),
                artifact("character_bible", "character"),
                artifact("story_bible", "story"),
            ],
            json!({}),
        ),
        (
            PackageType::Production,
            production_items,
            serde_json::to_value(production_metadata).unwrap(),
        ),
        (
            PackageType::Quality,
            vec![
                artifact("take_review", "review"),
                artifact("continuity_ledger", "ledger"),
                artifact("media_evidence", "evidence"),
                artifact("required_take_inventory", "inventory"),
            ],
            json!({
                "work_version_id": Uuid::new_v5(&run_id, b"work-version"),
                "inventory_digest": digest("inventory-snapshot")
            }),
        ),
    ];
    for (package_type, items, metadata) in fixtures {
        let first = ArtifactPackageSnapshot::build(
            package_type,
            run_id,
            step_id,
            1,
            0,
            1,
            items.clone(),
            metadata.clone(),
        )
        .unwrap();
        let second = ArtifactPackageSnapshot::build(
            package_type,
            run_id,
            step_id,
            1,
            0,
            1,
            items.into_iter().rev().collect(),
            metadata,
        )
        .unwrap();
        assert_eq!(first.package_digest, second.package_digest);
        assert!(first
            .items
            .windows(2)
            .all(|pair| pair[0].artifact_type <= pair[1].artifact_type));
    }

    assert!(ArtifactPackageSnapshot::build(
        PackageType::Brief,
        run_id,
        step_id,
        1,
        0,
        1,
        vec![],
        json!({}),
    )
    .is_err());
    let mut cross_run = artifact("creative_brief", "cross-run");
    cross_run.run_id = Uuid::new_v4();
    assert!(ArtifactPackageSnapshot::build(
        PackageType::Brief,
        run_id,
        step_id,
        1,
        0,
        1,
        vec![cross_run],
        json!({}),
    )
    .is_err());
    assert!(ArtifactPackageSnapshot::build(
        PackageType::Script,
        run_id,
        step_id,
        1,
        0,
        1,
        vec![
            artifact("character_bible", "missing-story-character"),
            artifact("script_draft", "missing-story-script"),
        ],
        json!({}),
    )
    .is_err());
}

#[test]
fn production_package_requires_closed_typed_scene_character_shot_sets() {
    let run_id = Uuid::new_v4();
    let step_id = Uuid::new_v4();
    let build = |items: Vec<ArtifactRef>, metadata: ProductionPackageMetadata| {
        ArtifactPackageSnapshot::build(
            PackageType::Production,
            run_id,
            step_id,
            1,
            0,
            1,
            items,
            serde_json::to_value(metadata).unwrap(),
        )
    };
    let (items, metadata) = production_package_fixture(run_id, step_id);
    build(items.clone(), metadata.clone()).unwrap();

    let mut no_shot_for_scene = metadata.clone();
    let removed_shot = no_shot_for_scene.shots.pop().unwrap();
    let without_removed_shot = items
        .iter()
        .filter(|item| item.artifact_id != removed_shot.artifact_id)
        .cloned()
        .collect();
    assert!(build(without_removed_shot, no_shot_for_scene)
        .unwrap_err()
        .to_string()
        .contains("Scene has no ShotContract"));

    let mut no_character_brief = metadata.clone();
    let removed_brief = no_character_brief.performance_briefs.pop().unwrap();
    let without_removed_brief = items
        .iter()
        .filter(|item| item.artifact_id != removed_brief.artifact_id)
        .cloned()
        .collect();
    assert!(build(without_removed_brief, no_character_brief)
        .unwrap_err()
        .to_string()
        .contains("Character has no PerformanceBrief"));

    let mut orphan_sound_scene = metadata.clone();
    orphan_sound_scene.sound_plan.scene_ids[1] = Uuid::new_v4();
    assert!(build(items.clone(), orphan_sound_scene)
        .unwrap_err()
        .to_string()
        .contains("SoundPlan Scene set"));

    let mut cross_script = metadata.clone();
    cross_script.performance_briefs[0].script_id = Uuid::new_v4();
    assert!(build(items.clone(), cross_script)
        .unwrap_err()
        .to_string()
        .contains("cross-Script"));

    let mut duplicate_shot = metadata.clone();
    duplicate_shot.shots[1].shot_id = duplicate_shot.shots[0].shot_id.clone();
    assert!(build(items.clone(), duplicate_shot)
        .unwrap_err()
        .to_string()
        .contains("duplicate Shot"));

    let mut invalid_order = metadata.clone();
    invalid_order.scenes[1].sequence = 3;
    assert!(build(items.clone(), invalid_order)
        .unwrap_err()
        .to_string()
        .contains("Scene sequence"));

    let mut invalid_duration = metadata.clone();
    invalid_duration.shots[1].duration_sec = 4;
    assert!(build(items.clone(), invalid_duration)
        .unwrap_err()
        .to_string()
        .contains("Shot duration"));

    let mut free_string = serde_json::to_value(metadata).unwrap();
    free_string["scenes"][0]["scene_id"] = json!("scene-1");
    assert!(ArtifactPackageSnapshot::build(
        PackageType::Production,
        run_id,
        step_id,
        1,
        0,
        1,
        items,
        free_string,
    )
    .unwrap_err()
    .to_string()
    .contains("typed metadata"));
}

#[test]
fn resource_gate_reserves_before_side_effect_and_holds_unknown_usage() {
    let limits = ResourceLimits {
        max_role_calls: 2,
        max_input_tokens: 100,
        max_output_tokens: 50,
        max_role_retries: 1,
        max_quality_reworks: 1,
        max_video_tasks: 2,
        max_video_duration_sec: 60,
        max_tts_characters: 1000,
        max_asr_tasks: 2,
        max_concurrency: 1,
        max_provider_retries: 1,
    };
    let mut ledger = ResourceUsageLedger::new(limits);
    let request = ResourceRequest::role_call(60, 20);
    let reservation = ResourceSafetyGate::reserve(&mut ledger, request.clone()).unwrap();
    assert_eq!(ledger.reserved("input_tokens"), 60);
    assert!(ResourceSafetyGate::reserve(&mut ledger, request).is_err());

    ResourceSafetyGate::settle(&mut ledger, reservation.id, Some(55), false).unwrap();
    assert_eq!(ledger.actual("input_tokens"), 55);

    let unknown =
        ResourceSafetyGate::reserve(&mut ledger, ResourceRequest::video_generation(1, 30)).unwrap();
    ResourceSafetyGate::settle(&mut ledger, unknown.id, None, true).unwrap();
    assert_eq!(ledger.held_uncertain("video_tasks"), 1);

    let boundary_limits = ResourceLimits {
        max_role_calls: 2,
        max_input_tokens: 100,
        max_output_tokens: 50,
        max_role_retries: 1,
        max_quality_reworks: 1,
        max_video_tasks: 2,
        max_video_duration_sec: 60,
        max_tts_characters: 1000,
        max_asr_tasks: 2,
        max_concurrency: 1,
        max_provider_retries: 1,
    };
    for resource_key in [
        "role_calls",
        "input_tokens",
        "output_tokens",
        "role_retries",
        "quality_reworks",
        "video_tasks",
        "video_duration_sec",
        "tts_characters",
        "asr_tasks",
        "concurrency",
        "provider_retries",
    ] {
        let limit = boundary_limits.value(resource_key).unwrap();
        let mut boundary_ledger = ResourceUsageLedger::new(boundary_limits.clone());
        let error = ResourceSafetyGate::reserve(
            &mut boundary_ledger,
            ResourceRequest {
                values: [(resource_key.into(), limit + 1)].into_iter().collect(),
            },
        )
        .unwrap_err();
        assert_eq!(error.code(), "resource_limit");
        assert!(error.to_string().contains(resource_key));
    }

    let resource_json = serde_json::to_value(&boundary_limits).unwrap();
    for forbidden in [
        "price",
        "currency",
        "amount",
        "api_key",
        "authorization",
        "credential",
    ] {
        assert!(!resource_json.as_object().unwrap().contains_key(forbidden));
    }
}

#[test]
fn orchestration_resource_limit_blocks_model_side_effect_before_invocation() {
    let mut limits = ResourceLimits::strict_default();
    limits.max_role_calls = 0;
    let mut ledger = ResourceUsageLedger::new(limits);
    let mut model_invocations = 0;

    let result =
        ResourceSafetyGate::reserve(&mut ledger, ResourceRequest::role_call(1, 1)).map(|_| {
            model_invocations += 1;
        });

    let error = result.unwrap_err();
    assert_eq!(error.code(), "resource_limit");
    assert!(error.to_string().contains("role_calls"));
    assert_eq!(model_invocations, 0);
}

#[test]
fn script_mapper_is_zero_call_deterministic_and_requires_complete_domain_fields() {
    let draft = ScriptDraftInput {
        title: "测试脚本".into(),
        hook: "三秒抓住注意力".into(),
        scenes: vec![
            ScriptSceneInput {
                sequence: 1,
                narration: "第一幕".into(),
                visual_description: "真实画面一".into(),
                emotion: "紧张".into(),
                duration_sec: 8,
                character_ids: vec!["lead".into()],
            },
            ScriptSceneInput {
                sequence: 2,
                narration: "第二幕".into(),
                visual_description: "真实画面二".into(),
                emotion: "释然".into(),
                duration_sec: 9,
                character_ids: vec!["lead".into()],
            },
        ],
    };
    let mapped = map_script_draft(&draft, &["lead".into()]).unwrap();
    assert_eq!(mapped.scenes.len(), 2);
    assert_eq!(mapped.scenes[0].sequence, 1);
    assert_eq!(mapped.scenes[1].sequence, 2);
    assert_eq!(
        mapped.digest,
        map_script_draft(&draft, &["lead".into()]).unwrap().digest
    );

    let mut invalid = draft.clone();
    invalid.scenes[1].sequence = 3;
    assert!(map_script_draft(&invalid, &["lead".into()]).is_err());
    assert!(map_script_draft(&draft, &[]).is_err());
}

#[test]
fn media_inventory_and_quality_gate_require_exact_current_coverage() {
    let work_version_id = Uuid::new_v4();
    let scene_a = Uuid::new_v4();
    let scene_b = Uuid::new_v4();
    let shot_a = Uuid::new_v4();
    let shot_b = Uuid::new_v4();
    let attempt_id = Uuid::new_v4();
    let input = ComposeInput {
        generation_step_id: Uuid::new_v4(),
        generation_attempt_id: attempt_id,
        output_artifact_id: Uuid::new_v4(),
        segment_key: "segment-1".into(),
        scene_ids: vec![scene_a, scene_b],
        shot_contracts: vec![(scene_a, vec![shot_a]), (scene_b, vec![shot_b])],
        consumed_by_final_compose: true,
        generation_succeeded: true,
    };
    let inventory = build_required_take_inventory(work_version_id, vec![input]).unwrap();
    assert_eq!(inventory.takes.len(), 1);
    assert_eq!(inventory.takes[0].scene_ids, vec![scene_a, scene_b]);
    let mut expected_shots = vec![shot_a, shot_b];
    expected_shots.sort();
    assert_eq!(inventory.required_shot_ids(), expected_shots);

    let evidence = MediaEvidence {
        work_version_id,
        inventory_digest: inventory.digest.clone(),
        final_asset_id: Uuid::new_v4(),
        asset_hash: digest("media"),
        mime_type: "video/mp4".into(),
        duration_ms: 17_000,
        vision: MediaCapability::available("vision-analyzer@1"),
        audio: MediaCapability::available("asr@1"),
        redacted_analysis: json!({"summary": "已检查真实媒体"}),
    };
    evidence.validate().unwrap();

    let ledgers = vec![
        ContinuityEvidence::new(work_version_id, inventory.digest.clone(), shot_a),
        ContinuityEvidence::new(work_version_id, inventory.digest.clone(), shot_b),
    ];
    let reviews = vec![TakeReviewEvidence::new(
        work_version_id,
        inventory.digest.clone(),
        inventory.takes[0].take_id,
        TakeEvidence::Approved,
    )];
    quality_coverage(&inventory, &evidence, &ledgers, &reviews).unwrap();
    assert!(quality_coverage(&inventory, &evidence, &ledgers[..1], &reviews).is_err());
    assert!(quality_coverage(&inventory, &evidence, &ledgers, &[]).is_err());

    let mut stale_evidence = evidence;
    stale_evidence.work_version_id = Uuid::new_v4();
    assert!(quality_coverage(&inventory, &stale_evidence, &ledgers, &reviews).is_err());
}

#[test]
fn media_snapshots_bind_exact_generation_provenance_without_inventing_take_shot_pairs() {
    let inventory_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let source_step_id = Uuid::new_v4();
    let work_id = Uuid::new_v4();
    let work_version_id = Uuid::new_v4();
    let generation_run_id = Uuid::new_v4();
    let final_asset = FinalMediaAsset {
        artifact_id: Uuid::new_v4(),
        sha256: digest("final-media"),
        mime_type: "video/mp4".into(),
        duration_ms: 17_000,
    };
    let scene_a = Uuid::new_v4();
    let scene_b = Uuid::new_v4();
    let shot_a = Uuid::new_v4();
    let shot_b = Uuid::new_v4();
    let compose_input = ComposeInput {
        generation_step_id: Uuid::new_v4(),
        generation_attempt_id: Uuid::new_v4(),
        output_artifact_id: Uuid::new_v4(),
        segment_key: "segment-1".into(),
        scene_ids: vec![scene_a, scene_b],
        shot_contracts: vec![(scene_a, vec![shot_a]), (scene_b, vec![shot_b])],
        consumed_by_final_compose: true,
        generation_succeeded: true,
    };
    let snapshot = RequiredTakeInventorySnapshot::build(
        inventory_id,
        run_id,
        source_step_id,
        1,
        0,
        work_id,
        work_version_id,
        generation_run_id,
        final_asset.clone(),
        digest("work-version"),
        vec![compose_input.clone()],
    )
    .unwrap();
    let replay = RequiredTakeInventorySnapshot::build(
        inventory_id,
        run_id,
        source_step_id,
        1,
        0,
        work_id,
        work_version_id,
        generation_run_id,
        final_asset.clone(),
        digest("work-version"),
        vec![compose_input],
    )
    .unwrap();

    assert_eq!(snapshot.inventory_digest, replay.inventory_digest);
    assert_eq!(snapshot.takes.len(), 1);
    assert_eq!(snapshot.takes[0].scene_ids, vec![scene_a, scene_b]);
    assert_eq!(snapshot.takes[0].scene_shot_map[&scene_a], vec![shot_a]);
    assert_eq!(snapshot.takes[0].scene_shot_map[&scene_b], vec![shot_b]);
    assert_ne!(snapshot.takes[0].take_id, shot_a);
    assert_ne!(snapshot.takes[0].take_id, shot_b);

    let evidence = MediaEvidenceSnapshot::build(
        Uuid::new_v4(),
        run_id,
        source_step_id,
        1,
        0,
        work_version_id,
        inventory_id,
        snapshot.inventory_digest.clone(),
        final_asset,
        "vision-analyzer@1".into(),
        "audio-asr@1".into(),
        json!({
            "final_media": {"motion_continuity": "pass", "audio_present": true},
            "takes": [{"take_id": snapshot.takes[0].take_id, "result": "pass"}],
        }),
    )
    .unwrap();
    assert_eq!(evidence.inventory_digest, snapshot.inventory_digest);
    assert_eq!(evidence.mime_type, "video/mp4");
    assert_eq!(evidence.duration_ms, 17_000);
    assert!(!serde_json::to_string(&evidence)
        .unwrap()
        .contains("signed_url"));
    media_review_readiness(Some(snapshot.clone()), Some(evidence.clone())).unwrap();
    assert_eq!(
        media_review_readiness(None, Some(evidence.clone()))
            .unwrap_err()
            .code(),
        "evidence_blocker"
    );
    assert_eq!(
        media_review_readiness(Some(snapshot.clone()), None)
            .unwrap_err()
            .code(),
        "evidence_blocker"
    );
    let mut no_vision = evidence.clone();
    no_vision.vision_capability_version.clear();
    assert_eq!(
        media_review_readiness(Some(snapshot.clone()), Some(no_vision))
            .unwrap_err()
            .code(),
        "capability_mismatch"
    );
    let mut no_audio = evidence.clone();
    no_audio.audio_capability_version.clear();
    assert_eq!(
        media_review_readiness(Some(snapshot.clone()), Some(no_audio))
            .unwrap_err()
            .code(),
        "capability_mismatch"
    );
    let mut missing_take = evidence.clone();
    missing_take.redacted_analysis["takes"] = json!([]);
    missing_take.evidence_digest = canonical_digest(&json!({"tampered": true})).unwrap();
    assert_eq!(
        media_review_readiness(Some(snapshot.clone()), Some(missing_take))
            .unwrap_err()
            .code(),
        "evidence_blocker"
    );

    assert!(MediaEvidenceSnapshot::build(
        Uuid::new_v4(),
        run_id,
        source_step_id,
        1,
        0,
        work_version_id,
        inventory_id,
        snapshot.inventory_digest,
        FinalMediaAsset {
            artifact_id: Uuid::new_v4(),
            sha256: digest("other"),
            mime_type: "video/mp4".into(),
            duration_ms: 1,
        },
        "vision-analyzer@1".into(),
        "audio-asr@1".into(),
        json!({"signed_url": "https://example.invalid/secret"}),
    )
    .is_err());
}
