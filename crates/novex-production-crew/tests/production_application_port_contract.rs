use async_trait::async_trait;
use novex_production_crew::{
    durable::{
        canonical_digest,
        package::{
            ArtifactPackageSnapshot, ArtifactRef, PackageType, ProductionCharacterRef,
            ProductionPackageMetadata, ProductionPerformanceRef, ProductionSceneRef,
            ProductionShotRef, ProductionSoundRef,
        },
        production_input::{
            AppliedSuggestionResolution, FormalSceneInput, FormalScriptInput,
            ProductionPackageContent, ProductionPackageInput, VersionedProductionArtifact,
        },
    },
    gates::GateRegistry,
    orchestrator::application_port::{
        ProductionWorkPlanRequest, ProductionWorkPlanSettings, SceneVisualManifestPort,
        SceneVisualManifestReference, SceneVisualReference, WorkGenerationPlanningPort,
        WorkGenerationRunDisposition, WorkGenerationRunReference, WorkGenerationRunStatus,
        WorkPlanReference, WorkVersionReworkKind, WorkVersionReworkReference,
        WorkVersionReworkRequest,
    },
    orchestrator::ProductionOrchestrator,
    roles::RoleRegistry,
    state::artifacts::output_contract::{
        DirectorialTreatmentOutput, PerformanceBriefOutput, PerformanceSceneOutput,
        SceneSoundNoteOutput, ShotContractOutput, SoundPlanOutput,
    },
    ProductionResult,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Default)]
struct CapturingManifestPort {
    inputs: Arc<Mutex<Vec<ProductionPackageInput>>>,
}

#[derive(Clone)]
struct CapturingWorkPort {
    inputs: Arc<Mutex<Vec<ProductionWorkPlanRequest>>>,
    result: WorkPlanReference,
}

#[async_trait]
impl WorkGenerationPlanningPort for CapturingWorkPort {
    async fn create_work_plan(
        &self,
        input: ProductionWorkPlanRequest,
    ) -> ProductionResult<WorkPlanReference> {
        self.inputs.lock().unwrap().push(input);
        Ok(self.result.clone())
    }
}

#[async_trait]
impl SceneVisualManifestPort for CapturingManifestPort {
    async fn prepare_scene_visual_manifest(
        &self,
        input: ProductionPackageInput,
    ) -> ProductionResult<SceneVisualManifestReference> {
        self.inputs.lock().unwrap().push(input.clone());
        Ok(SceneVisualManifestReference::build(
            input.script.script_id,
            input.script.script_version.clone(),
            "manifest-v1".into(),
            vec![SceneVisualReference {
                scene_id: input.scenes[0].scene_id,
                scene_version: input.scenes[0].scene_version.clone(),
                candidate_id: Uuid::new_v4(),
                material_id: Uuid::new_v4(),
            }],
        )?)
    }
}

fn versioned<T>(item: &ArtifactRef, content: T) -> VersionedProductionArtifact<T> {
    VersionedProductionArtifact {
        artifact_id: item.artifact_id,
        artifact_version: item.version,
        content_digest: item.content_digest.clone(),
        source_step_id: item.source_step_id,
        source_attempt: item.source_attempt,
        content,
    }
}

fn production_input() -> ProductionPackageInput {
    let run_id = Uuid::new_v4();
    let script_id = Uuid::new_v4();
    let scene_id = Uuid::new_v4();
    let character_bible_id = Uuid::new_v4();
    let source_step_id = Uuid::new_v4();
    let treatment_item = ArtifactRef {
        run_id,
        artifact_type: "directorial_treatment".into(),
        artifact_id: Uuid::new_v4(),
        version: 2,
        content_digest: format!("{:064x}", 11),
        source_step_id,
        source_attempt: 2,
    };
    let shot_item = ArtifactRef {
        artifact_type: "shot_contract".into(),
        artifact_id: Uuid::new_v4(),
        content_digest: format!("{:064x}", 12),
        ..treatment_item.clone()
    };
    let performance_item = ArtifactRef {
        artifact_type: "performance_brief".into(),
        artifact_id: Uuid::new_v4(),
        content_digest: format!("{:064x}", 13),
        ..treatment_item.clone()
    };
    let sound_item = ArtifactRef {
        artifact_type: "sound_plan".into(),
        artifact_id: Uuid::new_v4(),
        content_digest: format!("{:064x}", 14),
        ..treatment_item.clone()
    };
    let suggestion_id = Uuid::new_v4();
    let applied_suggestion = AppliedSuggestionResolution {
        suggestion_id,
        owner_role: "director".into(),
        artifact_id: shot_item.artifact_id,
        artifact_version: shot_item.version,
        content_digest: shot_item.content_digest.clone(),
    };
    let metadata = ProductionPackageMetadata {
        script_id,
        script_version: "script-v1".into(),
        script_digest: format!("{:064x}", 1),
        scenes: vec![ProductionSceneRef {
            scene_id,
            scene_version: "scene-v1".into(),
            scene_digest: format!("{:064x}", 2),
            sequence: 1,
            duration_sec: 5,
            character_bible_ids: vec![character_bible_id],
        }],
        characters: vec![ProductionCharacterRef {
            character_bible_id,
            character_id: "lead".into(),
        }],
        shots: vec![ProductionShotRef {
            artifact_id: shot_item.artifact_id,
            shot_id: "shot-1".into(),
            sequence: 1,
            scene_id,
            duration_sec: 5,
            character_bible_ids: vec![character_bible_id],
        }],
        performance_briefs: vec![ProductionPerformanceRef {
            artifact_id: performance_item.artifact_id,
            script_id,
            character_bible_id,
            character_id: "lead".into(),
            scene_ids: vec![scene_id],
        }],
        sound_plan: ProductionSoundRef {
            artifact_id: sound_item.artifact_id,
            script_id,
            scene_ids: vec![scene_id],
        },
        suggestion_resolutions: vec![serde_json::to_value(&applied_suggestion).unwrap()],
    };
    let package = ArtifactPackageSnapshot::build(
        PackageType::Production,
        run_id,
        source_step_id,
        2,
        1,
        3,
        vec![
            treatment_item.clone(),
            shot_item.clone(),
            performance_item.clone(),
            sound_item.clone(),
        ],
        serde_json::to_value(metadata).unwrap(),
    )
    .unwrap();
    let content = ProductionPackageContent {
        script: FormalScriptInput {
            script_id,
            script_version: "script-v1".into(),
            script_digest: format!("{:064x}", 1),
            title: "精确制作输入".into(),
            hook: "每一项都能追溯".into(),
        },
        scenes: vec![FormalSceneInput {
            scene_id,
            scene_version: "scene-v1".into(),
            scene_digest: format!("{:064x}", 2),
            sequence: 1,
            narration: "正式旁白".into(),
            visual_description: "正式画面描述".into(),
            emotion: "专注".into(),
            duration_sec: 5,
            character_bible_ids: vec![character_bible_id],
        }],
        directorial_treatment: versioned(
            &treatment_item,
            DirectorialTreatmentOutput {
                visual_style: "写实".into(),
                pacing: "紧凑".into(),
                emotional_arc: "由疑问到确定".into(),
                color_palette: vec!["neutral".into()],
                reference_works: vec!["reference".into()],
            },
        ),
        shot_contracts: vec![versioned(
            &shot_item,
            ShotContractOutput {
                shot_id: "shot-1".into(),
                sequence: 1,
                scene_id,
                shot_type: "medium".into(),
                camera_movement: "push_in".into(),
                duration_sec: 5,
                description: "推进到控制台".into(),
                character_ids: vec!["lead".into()],
            },
        )],
        performance_briefs: vec![versioned(
            &performance_item,
            PerformanceBriefOutput {
                character_bible_id,
                character_id: "lead".into(),
                script_id,
                emotional_arc: vec![PerformanceSceneOutput {
                    sequence: 1,
                    scene_id,
                    emotion: "专注".into(),
                    intensity: 3,
                    notes: "克制".into(),
                }],
                body_language: "稳定".into(),
                vocal_direction: "清晰".into(),
            },
        )],
        sound_plan: versioned(
            &sound_item,
            SoundPlanOutput {
                script_id,
                music_style: "minimal".into(),
                scene_sound_notes: vec![SceneSoundNoteOutput {
                    sequence: 1,
                    scene_id,
                    music_cue: "low pulse".into(),
                    sfx_notes: vec!["keyboard".into()],
                    dialogue_direction: "dry".into(),
                }],
            },
        ),
        applied_suggestions: vec![applied_suggestion],
    };
    ProductionPackageInput::from_approved_package(&package, content).unwrap()
}

#[tokio::test]
async fn fake_manifest_port_receives_complete_typed_production_package_input() {
    let input = production_input();
    let expected_digest = input.package_digest.clone();
    let port = CapturingManifestPort::default();
    let manifest = port
        .prepare_scene_visual_manifest(input.clone())
        .await
        .unwrap();

    assert_eq!(manifest.script_id, input.script.script_id);
    assert_eq!(manifest.script_version, input.script.script_version);
    assert_eq!(manifest.scenes.len(), 1);
    assert_eq!(port.inputs.lock().unwrap().as_slice(), &[input.clone()]);
    assert_eq!(input.package_digest, expected_digest);
    assert_eq!(input.scenes[0].narration, "正式旁白");
    assert_eq!(
        input.shot_contracts[0].content.scene_id,
        input.scenes[0].scene_id
    );
    assert_eq!(
        input.performance_briefs[0].content.character_bible_id,
        input.scenes[0].character_bible_ids[0]
    );
    assert_eq!(
        input.sound_plan.content.scene_sound_notes[0].scene_id,
        input.scenes[0].scene_id
    );
    assert_eq!(input.applied_suggestions.len(), 1);
    assert_eq!(
        input.input_digest,
        canonical_digest(&json!({
            "package_id": input.package_id,
            "package_digest": input.package_digest,
            "script": input.script,
            "scenes": input.scenes,
            "directorial_treatment": input.directorial_treatment,
            "shot_contracts": input.shot_contracts,
            "performance_briefs": input.performance_briefs,
            "sound_plan": input.sound_plan,
            "applied_suggestions": input.applied_suggestions,
        }))
        .unwrap()
    );
}

#[test]
fn typed_input_rejects_content_that_does_not_match_package_provenance() {
    let input = production_input();
    let package = input.package_snapshot().unwrap();
    let mut content = input.into_content();
    content.shot_contracts[0].content_digest = format!("{:064x}", 99);

    assert!(ProductionPackageInput::from_approved_package(&package, content).is_err());
}

#[tokio::test]
async fn orchestrator_calls_typed_ports_once_and_accepts_only_formal_references() {
    let production = production_input();
    let manifest_port = Arc::new(CapturingManifestPort::default());
    let work_inputs = Arc::new(Mutex::new(Vec::new()));
    let work_result = WorkPlanReference::build(
        Uuid::new_v4(),
        Uuid::new_v4(),
        1,
        format!("{:064x}", 31),
        Uuid::new_v4(),
        1,
        format!("{:064x}", 32),
    )
    .unwrap();
    let work_port = Arc::new(CapturingWorkPort {
        inputs: work_inputs.clone(),
        result: work_result.clone(),
    });
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://postgres:postgres@localhost/unused")
        .unwrap();
    let mut orchestrator = ProductionOrchestrator::new(
        pool,
        Arc::new(RoleRegistry::new()),
        Arc::new(GateRegistry::new()),
    );
    orchestrator.scene_visual_manifest_port = Some(manifest_port.clone());
    orchestrator.work_generation_planning_port = Some(work_port);

    let manifest = orchestrator
        .prepare_scene_visual_manifest(production.clone())
        .await
        .unwrap();
    let request = ProductionWorkPlanRequest {
        production: production.clone(),
        manifest: manifest.clone(),
        operator_settings: ProductionWorkPlanSettings {
            llm_model_id: Uuid::new_v4(),
            video_model_id: Uuid::new_v4(),
            tts_model_id: None,
            tts_voice_type: None,
            duration_strategy: "script_total".into(),
            duration_seconds: None,
            aspect_ratio: "9:16".into(),
            resolution: "1080p".into(),
            audio_mode: "silent".into(),
            narration_override: None,
            audio_material_ids: vec![],
            burn_subtitles: false,
            overrides: Default::default(),
        },
    };
    let result = orchestrator
        .create_work_plan(request.clone())
        .await
        .unwrap();

    assert_eq!(result, work_result);
    assert_eq!(
        manifest_port.inputs.lock().unwrap().as_slice(),
        &[production]
    );
    assert_eq!(work_inputs.lock().unwrap().as_slice(), &[request]);
}

#[tokio::test]
async fn stale_or_cross_script_manifest_never_reaches_work_planning_port() {
    for case in ["stale", "cross_script"] {
        let production = production_input();
        let valid_manifest = CapturingManifestPort::default()
            .prepare_scene_visual_manifest(production.clone())
            .await
            .unwrap();
        let mut invalid_manifest = valid_manifest;
        if case == "stale" {
            invalid_manifest.scenes[0].scene_version = "stale-scene-version".into();
        } else {
            invalid_manifest.script_id = Uuid::new_v4();
        }
        let work_inputs = Arc::new(Mutex::new(Vec::new()));
        let work_port = Arc::new(CapturingWorkPort {
            inputs: work_inputs.clone(),
            result: WorkPlanReference::build(
                Uuid::new_v4(),
                Uuid::new_v4(),
                1,
                format!("{:064x}", 41),
                Uuid::new_v4(),
                1,
                format!("{:064x}", 42),
            )
            .unwrap(),
        });
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost/unused")
            .unwrap();
        let mut orchestrator = ProductionOrchestrator::new(
            pool,
            Arc::new(RoleRegistry::new()),
            Arc::new(GateRegistry::new()),
        );
        orchestrator.work_generation_planning_port = Some(work_port);
        let request = ProductionWorkPlanRequest {
            production,
            manifest: invalid_manifest,
            operator_settings: ProductionWorkPlanSettings {
                llm_model_id: Uuid::new_v4(),
                video_model_id: Uuid::new_v4(),
                tts_model_id: None,
                tts_voice_type: None,
                duration_strategy: "script_total".into(),
                duration_seconds: None,
                aspect_ratio: "9:16".into(),
                resolution: "1080p".into(),
                audio_mode: "silent".into(),
                narration_override: None,
                audio_material_ids: vec![],
                burn_subtitles: false,
                overrides: Default::default(),
            },
        };

        let error = orchestrator.create_work_plan(request).await.unwrap_err();
        assert_eq!(error.code(), "transition_conflict", "case={case}");
        assert!(work_inputs.lock().unwrap().is_empty(), "case={case}");
    }
}

#[test]
fn work_generation_terminal_state_mapping_is_fail_closed_until_media_evidence_is_complete() {
    let build =
        |status, error_code: Option<&str>, retryable, final_media_ready, take_inventory_ready| {
            WorkGenerationRunReference::build(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                status,
                error_code.map(|_| "provider".into()),
                error_code.map(str::to_string),
                error_code.map(|_| "脱敏错误摘要".into()),
                retryable,
                final_media_ready,
                take_inventory_ready,
            )
            .unwrap()
        };

    let cases = [
        (
            build(WorkGenerationRunStatus::Queued, None, false, false, false),
            false,
            WorkGenerationRunDisposition::ExternalWait,
        ),
        (
            build(WorkGenerationRunStatus::Running, None, false, false, false),
            false,
            WorkGenerationRunDisposition::ExternalWait,
        ),
        (
            build(
                WorkGenerationRunStatus::Failed,
                Some("provider_failed"),
                true,
                false,
                false,
            ),
            false,
            WorkGenerationRunDisposition::FailedBlocker,
        ),
        (
            build(
                WorkGenerationRunStatus::WaitingManual,
                Some("unknown_submission"),
                false,
                false,
                false,
            ),
            false,
            WorkGenerationRunDisposition::AttentionRequired,
        ),
        (
            build(
                WorkGenerationRunStatus::Cancelling,
                None,
                false,
                false,
                false,
            ),
            true,
            WorkGenerationRunDisposition::Cancelling,
        ),
        (
            build(
                WorkGenerationRunStatus::Cancelled,
                None,
                false,
                false,
                false,
            ),
            false,
            WorkGenerationRunDisposition::ExternalCancelConflict,
        ),
        (
            build(
                WorkGenerationRunStatus::Cancelled,
                None,
                false,
                false,
                false,
            ),
            true,
            WorkGenerationRunDisposition::Cancelled,
        ),
        (
            build(WorkGenerationRunStatus::Succeeded, None, false, true, false),
            false,
            WorkGenerationRunDisposition::EvidenceBlocker,
        ),
        (
            build(WorkGenerationRunStatus::Succeeded, None, false, true, true),
            false,
            WorkGenerationRunDisposition::ReadyForMediaReview,
        ),
    ];

    for (external, cancellation_requested, expected) in cases {
        assert_eq!(external.disposition(cancellation_requested), expected);
    }
}

fn rework_request(kind: WorkVersionReworkKind) -> WorkVersionReworkRequest {
    WorkVersionReworkRequest {
        production_run_id: Uuid::new_v4(),
        revision_epoch: 2,
        work_id: Uuid::new_v4(),
        source_work_version_id: Uuid::new_v4(),
        inventory_id: Uuid::new_v4(),
        inventory_digest: format!("{:064x}", 51),
        evidence_snapshot_id: Uuid::new_v4(),
        evidence_digest: format!("{:064x}", 52),
        kind,
        rejected_take_ids: vec![Uuid::new_v4()],
        affected_shot_contract_ids: vec![Uuid::new_v4()],
        reason: "当前成片未满足质量要求".into(),
        actor: novex_production_crew::durable::command_store::ProductionActor::local_operator(),
        idempotency_key: "quality-rework-contract".into(),
    }
}

#[test]
fn work_version_rework_reference_is_typed_digest_bound_and_requires_confirmation() {
    let edit = rework_request(WorkVersionReworkKind::Edit);
    let reused_artifact_id = Uuid::new_v4();
    let edit_reference = WorkVersionReworkReference::build(
        &edit,
        Uuid::new_v4(),
        3,
        Uuid::new_v4(),
        4,
        format!("{:064x}", 53),
        Uuid::new_v4(),
        1,
        format!("{:064x}", 54),
        format!("{:064x}", 55),
        vec!["video_segment:scene-a".into(), "compose".into()],
        vec![reused_artifact_id],
        json!({"video_task_count": 1, "video_seconds": 5}),
    )
    .unwrap();
    edit_reference.validate_for(&edit).unwrap();
    assert_eq!(edit_reference.kind, WorkVersionReworkKind::Edit);
    assert_eq!(edit_reference.reused_artifact_ids, [reused_artifact_id]);
    assert!(edit_reference.requires_confirmation);

    let full = rework_request(WorkVersionReworkKind::FullRegeneration);
    let full_reference = WorkVersionReworkReference::build(
        &full,
        Uuid::new_v4(),
        3,
        Uuid::new_v4(),
        4,
        format!("{:064x}", 56),
        Uuid::new_v4(),
        1,
        format!("{:064x}", 57),
        format!("{:064x}", 58),
        vec!["video_segment:scene-a".into(), "compose".into()],
        vec![],
        json!({"video_task_count": 2, "video_seconds": 10}),
    )
    .unwrap();
    full_reference.validate_for(&full).unwrap();
    assert_eq!(full_reference.kind, WorkVersionReworkKind::FullRegeneration);
    assert!(full_reference.reused_artifact_ids.is_empty());
    assert!(full_reference.requires_confirmation);
    assert_ne!(
        edit_reference.reference_digest,
        full_reference.reference_digest
    );

    assert!(WorkVersionReworkReference::build(
        &full,
        Uuid::new_v4(),
        3,
        Uuid::new_v4(),
        4,
        format!("{:064x}", 59),
        Uuid::new_v4(),
        1,
        format!("{:064x}", 60),
        format!("{:064x}", 61),
        vec!["compose".into()],
        vec![Uuid::new_v4()],
        json!({"video_task_count": 2}),
    )
    .is_err());
}
