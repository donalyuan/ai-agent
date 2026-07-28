//! Full Crew 到既有画面与作品 Application Service 的适配器。

use super::{
    asset_generation::{AssetGenerationApplicationError, AssetGenerationService},
    work_generation::{WorkGenerationApplicationError, WorkGenerationService},
};
use crate::repositories::{PostgresWorkLibraryRepository, WorkLibraryRepositoryError};
use async_trait::async_trait;
use novex_production_crew::{
    durable::production_input::ProductionPackageInput,
    orchestrator::application_port::{
        ProductionWorkPlanRequest, SceneVisualManifestPort, SceneVisualManifestReference,
        SceneVisualReference, WorkGenerationPlanningPort, WorkGenerationRunPort,
        WorkGenerationRunReference, WorkPlanReference, WorkVersionReworkKind,
        WorkVersionReworkPort, WorkVersionReworkReference, WorkVersionReworkRequest,
    },
    ProductionError, ProductionResult,
};
use serde_json::{json, Map, Value};
use sqlx::Row;

#[derive(Clone)]
pub struct ProductionWorkflowIntegrationService {
    asset_generation: AssetGenerationService,
    work_generation: Option<WorkGenerationService>,
}

impl ProductionWorkflowIntegrationService {
    pub fn new(
        asset_generation: AssetGenerationService,
        work_generation: Option<WorkGenerationService>,
    ) -> Self {
        Self {
            asset_generation,
            work_generation,
        }
    }
}

#[async_trait]
impl WorkGenerationPlanningPort for ProductionWorkflowIntegrationService {
    async fn create_work_plan(
        &self,
        input: ProductionWorkPlanRequest,
    ) -> ProductionResult<WorkPlanReference> {
        input.validate()?;
        let service =
            self.work_generation
                .as_ref()
                .ok_or_else(|| ProductionError::CapabilityMismatch {
                    reason: "WorkGeneration planning service is not configured".into(),
                })?;
        service
            .plan_from_production(input)
            .await
            .map_err(map_work_generation_error)
    }
}

#[async_trait]
impl WorkGenerationRunPort for ProductionWorkflowIntegrationService {
    async fn confirmed_run_for_plan(
        &self,
        plan: WorkPlanReference,
    ) -> ProductionResult<WorkGenerationRunReference> {
        let service =
            self.work_generation
                .as_ref()
                .ok_or_else(|| ProductionError::CapabilityMismatch {
                    reason: "WorkGeneration run service is not configured".into(),
                })?;
        service
            .confirmed_run_reference(&plan)
            .await
            .map_err(map_work_generation_error)
    }

    async fn observe_run(
        &self,
        run_id: uuid::Uuid,
    ) -> ProductionResult<WorkGenerationRunReference> {
        let service =
            self.work_generation
                .as_ref()
                .ok_or_else(|| ProductionError::CapabilityMismatch {
                    reason: "WorkGeneration run service is not configured".into(),
                })?;
        service
            .observe_run_reference(run_id)
            .await
            .map_err(map_work_generation_error)
    }
}

fn map_work_generation_error(error: WorkGenerationApplicationError) -> ProductionError {
    match error {
        WorkGenerationApplicationError::ManifestIncomplete {
            script_id,
            blockers,
        } => ProductionError::ExternalWait {
            reason: "scene_visual_manifest".into(),
            details: json!({"script_id": script_id, "blockers": blockers}),
        },
        error => ProductionError::TransitionConflict {
            reason: error.to_string(),
        },
    }
}

#[async_trait]
impl SceneVisualManifestPort for ProductionWorkflowIntegrationService {
    async fn prepare_scene_visual_manifest(
        &self,
        input: ProductionPackageInput,
    ) -> ProductionResult<SceneVisualManifestReference> {
        input.package_snapshot()?;
        let manifest = match self
            .asset_generation
            .scene_visual_manifest(input.script.script_id)
            .await
        {
            Ok(manifest) => manifest,
            Err(AssetGenerationApplicationError::ManifestIncomplete {
                script_id,
                blockers,
            }) => {
                return Err(ProductionError::ExternalWait {
                    reason: "scene_visual_manifest".into(),
                    details: json!({
                        "script_id": script_id,
                        "blockers": blockers,
                    }),
                })
            }
            Err(AssetGenerationApplicationError::ManifestStale {
                expected_input_version,
                actual_input_version,
            }) => {
                return Err(ProductionError::ExternalWait {
                    reason: "scene_visual_manifest_stale".into(),
                    details: json!({
                        "expected_input_version": expected_input_version,
                        "actual_input_version": actual_input_version,
                    }),
                })
            }
            Err(error) => {
                return Err(ProductionError::TransitionConflict {
                    reason: format!("SceneVisualManifest lookup failed: {error}"),
                })
            }
        };
        if manifest.script_id != input.script.script_id
            || manifest.scenes.len() != input.scenes.len()
            || manifest
                .scenes
                .iter()
                .zip(&input.scenes)
                .any(|(actual, expected)| {
                    actual.scene_id != expected.scene_id
                        || actual.sequence != i32::try_from(expected.sequence).unwrap_or(-1)
                        || actual.narration != expected.narration
                        || actual.visual_description != expected.visual_description
                        || actual.emotion != expected.emotion
                        || actual.duration_sec != i32::try_from(expected.duration_sec).unwrap_or(-1)
                })
        {
            return Err(ProductionError::ExternalWait {
                reason: "scene_visual_manifest_stale".into(),
                details: json!({
                    "script_id": input.script.script_id,
                    "package_digest": input.package_digest,
                    "actual_input_version": manifest.input_version,
                }),
            });
        }
        let scenes = manifest
            .scenes
            .into_iter()
            .zip(&input.scenes)
            .map(|(actual, expected)| SceneVisualReference {
                scene_id: actual.scene_id,
                scene_version: expected.scene_version.clone(),
                candidate_id: actual.candidate_id,
                material_id: actual.material_id,
            })
            .collect();
        SceneVisualManifestReference::build(
            input.script.script_id,
            input.script.script_version,
            manifest.input_version,
            scenes,
        )
    }
}

/// Full Crew 质量返工到既有 Work Library 的独立适配器。
#[derive(Clone)]
pub struct ProductionWorkVersionReworkService {
    repository: PostgresWorkLibraryRepository,
}

impl ProductionWorkVersionReworkService {
    pub fn new(repository: PostgresWorkLibraryRepository) -> Self {
        Self { repository }
    }

    async fn edit_patch(&self, request: &WorkVersionReworkRequest) -> ProductionResult<Value> {
        let source = sqlx::query("SELECT work_id,input_snapshot FROM work_versions WHERE id=$1")
            .bind(request.source_work_version_id)
            .fetch_optional(self.repository.pool())
            .await?
            .ok_or_else(|| ProductionError::TransitionConflict {
                reason: "quality rework source WorkVersion not found".into(),
            })?;
        if source.get::<uuid::Uuid, _>("work_id") != request.work_id {
            return Err(ProductionError::TransitionConflict {
                reason: "quality rework source WorkVersion is cross-Work".into(),
            });
        }
        let affected_scene_ids = sqlx::query_scalar::<_, uuid::Uuid>(
            r#"
            SELECT DISTINCT domain_scene_id FROM shot_contracts
            WHERE run_id=$1 AND id=ANY($2) AND domain_scene_id IS NOT NULL
            ORDER BY domain_scene_id
            "#,
        )
        .bind(request.production_run_id)
        .bind(&request.affected_shot_contract_ids)
        .fetch_all(self.repository.pool())
        .await?;
        if affected_scene_ids.is_empty() {
            return Err(ProductionError::TransitionConflict {
                reason: "quality edit has no formal affected Scene".into(),
            });
        }
        let input_snapshot = source.get::<Value, _>("input_snapshot");
        let scenes = input_snapshot
            .get("scenes")
            .and_then(Value::as_array)
            .ok_or_else(|| ProductionError::TransitionConflict {
                reason: "quality edit source WorkVersion has no Scene snapshot".into(),
            })?;
        let mut scene_patches = Map::new();
        for scene_id in affected_scene_ids {
            let index = scenes
                .iter()
                .position(|scene| {
                    scene
                        .get("id")
                        .or_else(|| scene.get("scene_id"))
                        .and_then(Value::as_str)
                        .and_then(|value| uuid::Uuid::parse_str(value).ok())
                        == Some(scene_id)
                })
                .ok_or_else(|| ProductionError::TransitionConflict {
                    reason: "quality edit Shot maps outside the WorkVersion Scene snapshot".into(),
                })?;
            scene_patches.insert(
                index.to_string(),
                json!({
                    "quality_rework": {
                        "production_run_id": request.production_run_id,
                        "inventory_digest": request.inventory_digest,
                        "evidence_digest": request.evidence_digest,
                        "rejected_take_ids": request.rejected_take_ids,
                        "affected_shot_contract_ids": request.affected_shot_contract_ids,
                        "reason": request.reason,
                    }
                }),
            );
        }
        Ok(json!({"scenes": scene_patches}))
    }
}

#[async_trait]
impl WorkVersionReworkPort for ProductionWorkVersionReworkService {
    async fn create_rework_draft(
        &self,
        request: WorkVersionReworkRequest,
    ) -> ProductionResult<WorkVersionReworkReference> {
        request.validate()?;
        let input_patch = match request.kind {
            WorkVersionReworkKind::Edit => Some(self.edit_patch(&request).await?),
            WorkVersionReworkKind::FullRegeneration => None,
        };
        let none = None;
        let kind = match request.kind {
            WorkVersionReworkKind::Edit => "edit",
            WorkVersionReworkKind::FullRegeneration => "full_regeneration",
        };
        let (draft, _) = self
            .repository
            .derive_version(
                request.source_work_version_id,
                kind,
                [&input_patch, &none, &none, &none, &none],
            )
            .await
            .map_err(map_work_library_error)?;
        let diff = self
            .repository
            .analyze_diff(draft.id)
            .await
            .map_err(map_work_library_error)?;
        if draft.work_id != request.work_id
            || draft.source_version_id != Some(request.source_work_version_id)
            || diff.source_version_id != request.source_work_version_id
            || diff.draft_version_id != draft.id
            || diff.status != "analyzed"
        {
            return Err(ProductionError::TransitionConflict {
                reason: "Work Library returned a stale or cross-Work rework plan".into(),
            });
        }
        let plan = sqlx::query(
            r#"
            SELECT id,plan_version,status,input_fingerprint
            FROM work_plans WHERE work_version_id=$1
            ORDER BY plan_version DESC LIMIT 1
            "#,
        )
        .bind(draft.id)
        .fetch_optional(self.repository.pool())
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "Work Library rework draft has no WorkPlan".into(),
        })?;
        if plan.get::<String, _>("status") != "ready" {
            return Err(ProductionError::TransitionConflict {
                reason: "Work Library rework plan must await operator confirmation".into(),
            });
        }
        WorkVersionReworkReference::build(
            &request,
            draft.id,
            u32::try_from(draft.version_no).map_err(|_| ProductionError::TransitionConflict {
                reason: "Work Library draft version is invalid".into(),
            })?,
            plan.get("id"),
            u32::try_from(plan.get::<i32, _>("plan_version")).map_err(|_| {
                ProductionError::TransitionConflict {
                    reason: "Work Library plan version is invalid".into(),
                }
            })?,
            plan.get("input_fingerprint"),
            diff.id,
            u32::try_from(diff.plan_version).map_err(|_| ProductionError::TransitionConflict {
                reason: "Work Library diff version is invalid".into(),
            })?,
            diff.source_fingerprint,
            diff.draft_fingerprint,
            serde_json::from_value(diff.affected_nodes).map_err(|_| {
                ProductionError::TransitionConflict {
                    reason: "Work Library affected_nodes is invalid".into(),
                }
            })?,
            serde_json::from_value(diff.reused_artifact_ids).map_err(|_| {
                ProductionError::TransitionConflict {
                    reason: "Work Library reused_artifact_ids is invalid".into(),
                }
            })?,
            diff.resource_usage,
        )
    }
}

fn map_work_library_error(error: WorkLibraryRepositoryError) -> ProductionError {
    ProductionError::TransitionConflict {
        reason: error.to_string(),
    }
}
