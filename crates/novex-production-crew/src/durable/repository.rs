//! Durable Full Crew PostgreSQL repository。
//!
//! 每个命令在同一事务中写入命令幂等事实和领域状态；查询只从
//! PostgreSQL 重建，不读取 Redis payload 或进程内流程对象。

use super::{
    canonical_digest,
    command_store::{
        ProductionAggregateType, ProductionCommandScope, ProductionCommandStore,
        ProductionCommandType,
    },
    media::{
        media_review_readiness, ComposeInput, FinalMediaAsset, MediaEvidenceSnapshot,
        MediaReviewInput, RequiredTake, RequiredTakeInventorySnapshot,
    },
    package::{
        ArtifactPackageSnapshot, ArtifactRef, GateDecision, PackageType, ProductionCharacterRef,
        ProductionPackageMetadata, ProductionPerformanceRef, ProductionSceneRef, ProductionShotRef,
        ProductionSoundRef,
    },
    plan::{PlanSnapshot, ResourceLimits, StepKind},
    production_input::{
        FormalSceneInput, FormalScriptInput, ProductionPackageContent, ProductionPackageInput,
        VersionedProductionArtifact,
    },
    resource::ResourceRequest,
    state_machine::{
        allowed_commands, validate_intent_command, validate_workflow_command, CommandEnvelope,
        IntentCommand, IntentHistory, RunState, RunStatus, SideEffectState, StepState, StepStatus,
        WorkflowCommand, WorkflowCommandKind, WorkflowSnapshot,
    },
};
use crate::gates::quality_gate::{
    QualityContinuityLedger, QualityGate, QualityGateInput, QualityGateOutcome,
    QualityReviewStatus, QualityTakeReview,
};
use crate::orchestrator::application_port::{
    ProductionWorkPlanRequest, SceneVisualManifestReference, WorkGenerationRunDisposition,
    WorkGenerationRunReference, WorkPlanReference, WorkVersionReworkKind,
    WorkVersionReworkReference, WorkVersionReworkRequest,
};
use crate::state::artifacts::output_contract::{
    CharacterBibleOutput, DirectorialTreatmentOutput, PerformanceBriefOutput, ScriptDraftOutput,
    ShotContractOutput, SoundPlanOutput, ValidatedRoleOutput,
};
use crate::{ProductionError, ProductionResult};
use chrono::{DateTime, Utc};
use novex_agent::{AuditedTerminalStatus, FinishAuditedCall};
use novex_ai_core::{redact_audit_value, validate_audit_payload};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use uuid::Uuid;

pub use super::command_store::ProductionActor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIntentCommand {
    pub project_id: Uuid,
    pub topic_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub initial_input: Value,
    pub actor: ProductionActor,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct StartRunCommand {
    pub intent_id: Uuid,
    pub plan: PlanSnapshot,
    pub actor: ProductionActor,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct ResumeRunCommand {
    pub run_id: Uuid,
    pub actor: ProductionActor,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct RetryStepCommand {
    pub run_id: Uuid,
    pub step_id: Uuid,
    pub actor: ProductionActor,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcceptedProductionCommand {
    pub run_id: Uuid,
    pub step_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct PackageDecisionCommand {
    pub run_id: Uuid,
    pub package_digest: String,
    pub decision: GateDecision,
    pub reason: Option<String>,
    pub affected_owners: Vec<String>,
    pub actor: ProductionActor,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollaborationSuggestionCommand {
    pub run_id: Uuid,
    pub source_step_id: Uuid,
    pub source_attempt: i32,
    pub source_model_call_id: Uuid,
    pub from_role: String,
    pub to_role: String,
    pub target_artifact_type: String,
    pub target_artifact_id: Uuid,
    pub target_artifact_version: i32,
    pub target_content_digest: String,
    pub suggestion_type: String,
    pub content: Value,
    pub blocking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionDecision {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductionIntentRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub topic_id: Uuid,
    pub title: String,
    pub status: String,
    pub source_snapshot: Value,
    pub source_fingerprint: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductionRunRecord {
    pub id: Uuid,
    pub production_project_id: Uuid,
    pub plan_snapshot_id: Uuid,
    pub status: String,
    pub quality_status: String,
    pub current_revision_epoch: i32,
    pub resource_limits: Value,
    pub binding_snapshot: Value,
    pub source_snapshot: Value,
    pub cancellation_intent: Option<Value>,
    pub error_code: Option<String>,
    pub error_details: Option<Value>,
    pub actor_type: String,
    pub actor_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductionStepRecord {
    pub id: Uuid,
    pub run_id: Uuid,
    pub revision_epoch: i32,
    pub plan_order: i32,
    pub step_key: String,
    pub step_type: String,
    pub role_key: Option<String>,
    pub dependencies: Value,
    pub status: String,
    pub waiting_reason: Option<String>,
    pub error_code: Option<String>,
    pub error_details: Option<Value>,
    pub retryable: bool,
    pub attempt: i32,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub side_effect_state: String,
    pub agent_run_id: Option<Uuid>,
    pub model_call_id: Option<Uuid>,
    pub context_snapshot_id: Option<Uuid>,
}

/// Role prepare 读取的精确 package item，不包含按项目动态查询得到的内容。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RoleInputArtifactRef {
    pub artifact_type: String,
    pub artifact_id: Uuid,
    pub artifact_version: i32,
    pub content_digest: String,
    pub source_step_id: Uuid,
    pub source_attempt: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleInputPackage {
    pub id: Uuid,
    pub package_type: String,
    pub digest: String,
    pub revision_epoch: i32,
    pub items: Vec<RoleInputArtifactRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDependencyAnchor {
    pub step_id: Uuid,
    pub step_key: String,
    pub step_type: String,
    pub role_key: Option<String>,
    pub output_digest: String,
}

/// 计划声明的修订命令为当前 owner 保存的追加用户指令。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RoleRevisionInstruction {
    pub id: Uuid,
    pub revision_epoch: i32,
    pub owner_role: String,
    pub actor_type: String,
    pub actor_id: String,
    pub source: String,
    pub trust: String,
    pub instruction: String,
    pub instruction_digest: String,
}

/// 在任何模型 provider 调用前读取并校验的 durable role 输入快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePrepareSnapshot {
    pub run_id: Uuid,
    pub production_project_id: Uuid,
    pub project_id: Uuid,
    pub step_id: Uuid,
    pub attempt: i32,
    pub revision_epoch: i32,
    pub role_key: String,
    pub source_snapshot: Value,
    pub role_binding: Value,
    pub input_packages: Vec<RoleInputPackage>,
    pub dependency_anchors: Vec<RoleDependencyAnchor>,
    pub revision_instruction: Option<RoleRevisionInstruction>,
    pub media_review: Option<MediaReviewInput>,
}

/// 已经由 DefinitionRegistry 和模型 resolver 校验过、可写入 agent_run_bindings 的证据。
#[derive(Debug, Clone)]
pub struct PreparedAgentBindingInput {
    pub agent_key: String,
    pub agent_version: String,
    pub agent_digest: String,
    pub prompt_bindings: Value,
    pub context_policy_bindings: Value,
    pub registry_digest: String,
    pub model_id: Uuid,
    pub behavior_fingerprint: String,
    pub model_capabilities: Value,
    pub tokenizer_profile_key: String,
    pub tokenizer_profile_version: String,
    pub tokenizer_profile_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoleFinalizeFailure {
    pub code: String,
    pub message: String,
    pub result_uncertain: bool,
}

#[derive(Clone)]
pub struct RoleFinalizeCommand {
    pub run_id: Uuid,
    pub production_project_id: Uuid,
    pub step_id: Uuid,
    pub attempt: i32,
    pub revision_epoch: i32,
    pub role_key: String,
    pub agent_run_id: Uuid,
    pub model_call_id: Uuid,
    pub context_snapshot_id: Uuid,
    pub input_packages: Vec<RoleInputPackage>,
    pub output: Option<Value>,
    pub validated_output: Option<ValidatedRoleOutput>,
    pub output_digest: Option<String>,
    pub failure: Option<RoleFinalizeFailure>,
    pub model_call_finish: FinishAuditedCall,
    pub execution_time_ms: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedRoleArtifact {
    pub artifact_type: String,
    pub id: Uuid,
    pub version: i32,
    pub character_id: Option<String>,
    pub shot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoleFinalizeRecord {
    pub role: String,
    pub status: String,
    pub output_artifacts: Vec<PersistedRoleArtifact>,
    pub model_call_id: Uuid,
    pub execution_time_ms: u64,
    pub error: Option<RoleFinalizeFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionRunView {
    pub run: ProductionRunRecord,
    pub steps: Vec<ProductionStepRecord>,
    pub packages: Vec<Value>,
    pub gate_decisions: Vec<Value>,
    pub domain_links: Vec<Value>,
    pub resource_summary: Vec<Value>,
    pub allowed_commands: Vec<WorkflowCommand>,
}

/// PostgreSQL 从唯一当前质量作用域构建出的不可变包及 Gate 输入。
#[derive(Debug, Clone)]
pub struct QualityPackageBuild {
    pub package: ArtifactPackageSnapshot,
    pub gate_input: QualityGateInput,
    pub outcome: QualityGateOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PersistedGateDecision {
    pub id: Uuid,
    pub run_id: Uuid,
    pub gate_step_id: Uuid,
    pub package_id: Uuid,
    pub package_digest: String,
    pub revision_epoch: i32,
    pub decision: String,
    pub reason: Option<String>,
    pub affected_owners: Value,
    pub actor_type: String,
    pub actor_id: String,
    pub decided_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PersistedResourceReservation {
    pub id: Uuid,
    pub run_id: Uuid,
    pub step_id: Uuid,
    pub attempt_no: i32,
    pub resource_key: String,
    pub reserved_value: i64,
    pub actual_value: Option<i64>,
    pub status: String,
    pub request_digest: String,
    pub created_at: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductionWakeupRecord {
    pub id: Uuid,
    pub run_id: Uuid,
    pub step_id: Uuid,
    pub status: String,
    pub delivery_attempts: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalCancellationState {
    Cancelling,
    Cancelled,
    AttentionRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancellationContext {
    pub run: ProductionRunRecord,
    pub external_run_ids: Vec<Uuid>,
    pub external_results: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PersistedCollaborationSuggestion {
    pub id: Uuid,
    pub production_project_id: Uuid,
    pub run_id: Uuid,
    pub source_step_id: Uuid,
    pub source_attempt: i32,
    pub revision_epoch: i32,
    pub source_model_call_id: Uuid,
    pub from_role: String,
    pub to_role: String,
    pub artifact_type: String,
    pub artifact_id: Uuid,
    pub target_artifact_version: i32,
    pub target_content_digest: String,
    pub suggestion_type: String,
    pub content: Value,
    pub blocking: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PersistedSuggestionResponse {
    pub id: Uuid,
    pub suggestion_id: Uuid,
    pub decision: String,
    pub reason: Option<String>,
    pub actor_type: String,
    pub actor_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PackageIdentity {
    id: Uuid,
    package_type: String,
    revision_epoch: i32,
    metadata: Value,
}

#[derive(FromRow)]
struct SuggestionResolutionRequirement {
    id: Uuid,
    to_role: String,
    artifact_type: String,
    target_artifact_version: i32,
    target_content_digest: String,
    decision: Option<String>,
}

#[derive(FromRow)]
struct RolePrepareHeadRow {
    run_id: Uuid,
    production_project_id: Uuid,
    project_id: Uuid,
    current_revision_epoch: i32,
    run_status: String,
    cancellation_intent: Option<Value>,
    binding_snapshot: Value,
    source_snapshot: Value,
    step_id: Uuid,
    revision_epoch: i32,
    step_type: String,
    role_key: Option<String>,
    dependencies: Value,
    step_status: String,
    attempt: i32,
    lease_owner: Option<String>,
    lease_expires_at: Option<DateTime<Utc>>,
    side_effect_state: String,
}

#[derive(Clone, FromRow)]
struct RolePrepareStepRow {
    id: Uuid,
    plan_order: i32,
    step_key: String,
    step_type: String,
    role_key: Option<String>,
    dependencies: Value,
    status: String,
    input_package_id: Option<Uuid>,
    input_digest: Option<String>,
    output_digest: Option<String>,
}

#[derive(FromRow)]
struct RoleInputPackageRow {
    id: Uuid,
    package_type: String,
    package_digest: String,
    revision_epoch: i32,
}

#[derive(FromRow)]
struct MediaInventoryRow {
    inventory_id: Uuid,
    run_id: Uuid,
    source_step_id: Uuid,
    source_attempt: i32,
    revision_epoch: i32,
    work_id: Uuid,
    work_version_id: Uuid,
    work_generation_run_id: Uuid,
    final_artifact_id: Uuid,
    work_version_hash: String,
    inventory_digest: String,
}

#[derive(FromRow)]
struct RequiredTakeRow {
    take_id: Uuid,
    ordinal: i32,
    generation_step_id: Uuid,
    generation_attempt_id: Uuid,
    output_artifact_id: Uuid,
    segment_key: String,
    scene_ids: Value,
    scene_shot_map: Value,
    generation_run_id: Uuid,
    generation_step_status: String,
    generation_step_type: String,
    attempt_step_id: Uuid,
    attempt_status: String,
    output_work_version_id: Uuid,
    output_generation_step_id: Option<Uuid>,
    output_role: String,
}

#[derive(FromRow)]
struct MediaEvidenceRow {
    evidence_id: Uuid,
    run_id: Uuid,
    source_step_id: Uuid,
    source_attempt: i32,
    revision_epoch: i32,
    work_version_id: Uuid,
    inventory_id: Uuid,
    inventory_digest: String,
    final_artifact_id: Uuid,
    asset_hash: String,
    mime_type: String,
    duration_ms: i64,
    vision_capability_version: String,
    audio_capability_version: String,
    redacted_analysis: Value,
    evidence_digest: String,
}

#[derive(FromRow)]
struct QualityLedgerRow {
    id: Uuid,
    step_id: Uuid,
    attempt: i32,
    revision_epoch: i32,
    work_version_id: Uuid,
    inventory_id: Uuid,
    evidence_snapshot_id: Uuid,
    shot_contract_id: Uuid,
    version: i32,
    content_digest: String,
    content: Value,
}

#[derive(FromRow)]
struct QualityReviewRow {
    id: Uuid,
    step_id: Uuid,
    attempt: i32,
    revision_epoch: i32,
    work_version_id: Uuid,
    inventory_id: Uuid,
    evidence_snapshot_id: Uuid,
    required_take_id: Uuid,
    version: i32,
    content_digest: String,
    status: String,
    content: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
struct ApplicableLedgerVersionRow {
    continuity_ledger_id: Uuid,
    shot_contract_id: Uuid,
    ledger_version: i32,
    content_digest: String,
}

#[derive(FromRow)]
struct RequiredTakeBuildScopeRow {
    work_generation_run_id: Uuid,
    work_id: Uuid,
    work_version_id: Uuid,
    generation_status: String,
    script_id: Uuid,
    input_snapshot: Value,
    plan_prompt_snapshot: Value,
    generation_prompt_snapshot: Value,
    work_version_snapshot: Value,
}

#[derive(FromRow)]
struct RequiredTakePackageRow {
    id: Uuid,
    source_step_id: Uuid,
    source_attempt: i32,
    package_version: i32,
    package_digest: String,
    revision_epoch: i32,
    metadata: Value,
    decision: String,
}

#[derive(FromRow)]
struct RequiredTakeGenerationStepRow {
    id: Uuid,
    step_type: String,
    status: String,
    depends_on: Value,
    input_snapshot: Value,
}

#[derive(Debug, Deserialize)]
struct RequiredTakePlanSegment {
    sequence: usize,
    scene_ids: Vec<Uuid>,
}

#[derive(Clone, FromRow)]
struct ReworkStepRow {
    plan_order: i32,
    step_key: String,
    step_type: String,
    role_key: Option<String>,
    dependencies: Value,
    attempt: i32,
    input_package_id: Option<Uuid>,
    input_digest: Option<String>,
    output_digest: Option<String>,
}

#[derive(FromRow)]
struct ProductionPackageRunScope {
    production_project_id: Uuid,
    current_revision_epoch: i32,
}

#[derive(FromRow)]
struct ProductionPackageScriptRow {
    id: Uuid,
    target_version: String,
    target_digest: String,
    source_artifacts: Value,
}

#[derive(FromRow)]
struct ProductionPackageSceneRow {
    id: Uuid,
    sequence: i32,
    duration_sec: i32,
    target_version: String,
    target_digest: String,
}

#[derive(Deserialize)]
struct ProductionPackageScriptArtifact {
    artifact_type: String,
    artifact_id: Uuid,
    artifact_version: i32,
    content_digest: String,
    source_step_id: Uuid,
    source_attempt: i32,
}

#[derive(FromRow)]
struct ProductionPackageCharacterRow {
    id: Uuid,
    character_id: String,
    version: i32,
    content: Value,
    content_digest: String,
    step_id: Uuid,
    attempt: i32,
}

#[derive(FromRow)]
struct ProductionPackageArtifactRow {
    id: Uuid,
    version: i32,
    content: Value,
    content_digest: String,
    step_id: Uuid,
    attempt: i32,
}

#[derive(FromRow)]
struct ApprovedProductionPackageRow {
    id: Uuid,
    source_step_id: Uuid,
    source_attempt: i32,
    revision_epoch: i32,
    package_version: i32,
    package_digest: String,
    metadata: Value,
}

#[derive(FromRow)]
struct ApprovedProductionPackageItemRow {
    artifact_type: String,
    artifact_id: Uuid,
    artifact_version: i32,
    content_digest: String,
    source_step_id: Uuid,
    source_attempt: i32,
}

#[derive(FromRow)]
struct FormalScriptInputRow {
    title: String,
    hook: String,
}

#[derive(FromRow)]
struct FormalSceneInputRow {
    id: Uuid,
    sequence: i32,
    narration: String,
    visual_description: String,
    emotion: String,
    duration_sec: i32,
}

#[derive(FromRow)]
struct ProductionPackageContributorStep {
    id: Uuid,
    role_key: String,
    plan_order: i32,
    attempt: i32,
}

#[derive(Clone)]
pub struct DurableProductionRepository {
    pool: PgPool,
}

impl DurableProductionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_intent(
        &self,
        command: CreateIntentCommand,
    ) -> ProductionResult<ProductionIntentRecord> {
        command.actor.validate()?;
        validate_safe_object(&command.initial_input)?;
        if command.title.trim().is_empty() {
            return Err(ProductionError::SourceInvalid {
                reason: "production title must not be blank".into(),
            });
        }
        let request_digest = ProductionCommandStore::canonical_request_digest(&json!({
            "project_id": command.project_id,
            "topic_id": command.topic_id,
            "title": command.title,
            "description": command.description,
            "initial_input": command.initial_input,
        }))?;
        let command_scope = ProductionCommandScope::new(
            command.actor.clone(),
            ProductionCommandType::CreateIntent,
            ProductionAggregateType::Topic,
            command.topic_id,
            &command.idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        if let Some(result) =
            ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest).await?
        {
            let intent_id = result
                .get("intent_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(|| ProductionError::TransitionConflict {
                    reason: "stored create_intent result is invalid".into(),
                })?;
            tx.commit().await?;
            return self.get_intent(intent_id).await;
        }

        let source_snapshot = sqlx::query_scalar::<_, Value>(
            r#"
            SELECT jsonb_build_object(
                'project', jsonb_build_object(
                    'id', project.id, 'name', project.name,
                    'positioning', project.positioning, 'description', project.description,
                    'status', project.status, 'updated_at', project.updated_at
                ),
                'topic', jsonb_build_object(
                    'id', topic.id, 'project_id', topic.project_id, 'title', topic.title,
                    'angle', topic.angle, 'target_audience', topic.target_audience,
                    'hook_points', topic.hook_points, 'content_type', topic.content_type,
                    'tags', topic.tags, 'status', topic.status,
                    'updated_at', topic.updated_at
                ),
                'initial_input', $3::jsonb
            )
            FROM projects project
            JOIN content_topics topic ON topic.id = $2 AND topic.project_id = project.id
            WHERE project.id = $1
              AND project.status = 'active'
              AND topic.status = 'approved'
              AND topic.deleted_at IS NULL
            FOR UPDATE OF topic
            "#,
        )
        .bind(command.project_id)
        .bind(command.topic_id)
        .bind(&command.initial_input)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::SourceInvalid {
            reason: "project/topic must be active, same-project, approved, and not deleted".into(),
        })?;
        let source_fingerprint = canonical_digest(&source_snapshot)?;

        let insert = sqlx::query_as::<_, ProductionIntentRecord>(
            r#"
            INSERT INTO production_projects (
                title, description, project_type, status, user_id, metadata,
                project_id, topic_id, source_snapshot, source_fingerprint, source_locked_at
            )
            VALUES ($1, $2, 'full_crew', 'created', $3, $4, $5, $6, $7, $8, NOW())
            RETURNING id, project_id, topic_id, title, status, source_snapshot,
                      source_fingerprint, archived_at, created_at
            "#,
        )
        .bind(&command.title)
        .bind(&command.description)
        .bind(Uuid::nil())
        .bind(&command.initial_input)
        .bind(command.project_id)
        .bind(command.topic_id)
        .bind(&source_snapshot)
        .bind(&source_fingerprint)
        .fetch_one(&mut *tx)
        .await;
        let intent = match insert {
            Ok(intent) => intent,
            Err(error)
                if is_constraint(&error, "production_projects_one_active_intent_per_topic") =>
            {
                return Err(ProductionError::ActiveIntentConflict)
            }
            Err(error) => return Err(error.into()),
        };
        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            json!({"intent_id": intent.id}),
        )
        .await?;
        tx.commit().await?;
        Ok(intent)
    }

    pub async fn get_intent(&self, id: Uuid) -> ProductionResult<ProductionIntentRecord> {
        sqlx::query_as::<_, ProductionIntentRecord>(
            r#"
            SELECT id, project_id, topic_id, title, status, source_snapshot,
                   source_fingerprint, archived_at, created_at
            FROM production_projects
            WHERE id = $1 AND status <> 'legacy_unbound' AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ProductionError::ProjectNotFound { project_id: id })
    }

    pub async fn delete_intent(
        &self,
        intent_id: Uuid,
        actor: ProductionActor,
        idempotency_key: &str,
    ) -> ProductionResult<()> {
        let request_digest =
            ProductionCommandStore::canonical_request_digest(&json!({"intent_id": intent_id}))?;
        let command_scope = ProductionCommandScope::new(
            actor,
            ProductionCommandType::DeleteIntent,
            ProductionAggregateType::ProductionIntent,
            intent_id,
            idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        if ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest)
            .await?
            .is_some()
        {
            tx.commit().await?;
            return Ok(());
        }
        lock_full_crew_intent(&mut tx, intent_id).await?;
        let history = load_intent_history(&mut tx, intent_id).await?;
        validate_intent_command(IntentCommand::Delete, &history)?;
        let deleted = sqlx::query("DELETE FROM production_projects WHERE id=$1")
            .bind(intent_id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() != 1 {
            return Err(ProductionError::ProjectNotFound {
                project_id: intent_id,
            });
        }
        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            json!({"intent_id": intent_id, "status": "deleted"}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn archive_intent(
        &self,
        intent_id: Uuid,
        actor: ProductionActor,
        idempotency_key: &str,
    ) -> ProductionResult<ProductionIntentRecord> {
        let request_digest =
            ProductionCommandStore::canonical_request_digest(&json!({"intent_id": intent_id}))?;
        let command_scope = ProductionCommandScope::new(
            actor,
            ProductionCommandType::ArchiveIntent,
            ProductionAggregateType::ProductionIntent,
            intent_id,
            idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        if ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest)
            .await?
            .is_some()
        {
            tx.commit().await?;
            return self.get_intent(intent_id).await;
        }
        lock_full_crew_intent(&mut tx, intent_id).await?;
        let history = load_intent_history(&mut tx, intent_id).await?;
        validate_intent_command(IntentCommand::Archive, &history)?;
        sqlx::query(
            "UPDATE production_projects SET status='archived',archived_at=NOW(),updated_at=NOW() WHERE id=$1",
        )
        .bind(intent_id)
        .execute(&mut *tx)
        .await?;
        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            json!({"intent_id": intent_id, "status": "archived"}),
        )
        .await?;
        tx.commit().await?;
        self.get_intent(intent_id).await
    }

    pub async fn start_run(
        &self,
        command: StartRunCommand,
    ) -> ProductionResult<ProductionRunRecord> {
        command.actor.validate()?;
        let request_digest = start_run_request_digest(command.intent_id)?;
        let command_scope = ProductionCommandScope::new(
            command.actor.clone(),
            ProductionCommandType::StartRun,
            ProductionAggregateType::ProductionIntent,
            command.intent_id,
            &command.idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        if let Some(result) =
            ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest).await?
        {
            let run_id = uuid_from_result(&result, "run_id")?;
            tx.commit().await?;
            return self.get_run_record(run_id).await;
        }
        command.plan.validate_frozen()?;
        validate_public_binding_snapshot(&command.plan.role_bindings)?;
        validate_active_bindings(&command.plan)?;

        let source_snapshot = sqlx::query_scalar::<_, Value>(
            r#"
            SELECT source_snapshot
            FROM production_projects
            WHERE id = $1 AND project_type = 'full_crew'
              AND status IN ('created', 'active')
              AND project_id IS NOT NULL AND topic_id IS NOT NULL
            FOR UPDATE
            "#,
        )
        .bind(command.intent_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "intent is legacy, terminal, archived, or otherwise not startable".into(),
        })?;
        let existing = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM production_runs WHERE production_project_id = $1",
        )
        .bind(command.intent_id)
        .fetch_optional(&mut *tx)
        .await?;
        if existing.is_some() {
            return Err(ProductionError::RunAlreadyExists);
        }

        let plan_json = serde_json::to_value(&command.plan)?;
        let resource_limits = serde_json::to_value(&command.plan.resource_limits)?;
        sqlx::query(
            r#"
            INSERT INTO production_plan_snapshots (
                plan_key, plan_version, plan_digest, plan, role_bindings, resource_limits
            ) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (plan_key, plan_version, plan_digest) DO NOTHING
            "#,
        )
        .bind(&command.plan.plan_key)
        .bind(&command.plan.plan_version)
        .bind(&command.plan.digest)
        .bind(&plan_json)
        .bind(&command.plan.role_bindings)
        .bind(&resource_limits)
        .execute(&mut *tx)
        .await?;
        let plan_snapshot_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM production_plan_snapshots
            WHERE plan_key = $1 AND plan_version = $2 AND plan_digest = $3
            "#,
        )
        .bind(&command.plan.plan_key)
        .bind(&command.plan.plan_version)
        .bind(&command.plan.digest)
        .fetch_one(&mut *tx)
        .await?;

        let run = sqlx::query_as::<_, ProductionRunRecord>(
            r#"
            INSERT INTO production_runs (
                production_project_id, plan_snapshot_id, status, resource_limits,
                binding_snapshot, source_snapshot, actor_type, actor_id
            ) VALUES ($1, $2, 'queued', $3, $4, $5, $6, $7)
            RETURNING id, production_project_id, plan_snapshot_id, status, quality_status,
                      current_revision_epoch, resource_limits, binding_snapshot, source_snapshot,
                      cancellation_intent, error_code, error_details, actor_type, actor_id,
                      created_at, updated_at
            "#,
        )
        .bind(command.intent_id)
        .bind(plan_snapshot_id)
        .bind(&resource_limits)
        .bind(&command.plan.role_bindings)
        .bind(&source_snapshot)
        .bind(&command.actor.actor_type)
        .bind(&command.actor.actor_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO production_revision_epochs (
                run_id, epoch, reason_type, reason, affected_owners, actor_type, actor_id
            ) VALUES ($1, 0, 'initial', 'initial fixed plan execution', '[]'::jsonb, $2, $3)
            "#,
        )
        .bind(run.id)
        .bind(&command.actor.actor_type)
        .bind(&command.actor.actor_id)
        .execute(&mut *tx)
        .await?;

        for (plan_order, step) in command.plan.steps.iter().enumerate() {
            let step_type = serde_json::to_value(step.kind)?
                .as_str()
                .unwrap_or_default()
                .to_string();
            let status = if step.dependencies.is_empty() {
                "queued"
            } else {
                "blocked"
            };
            sqlx::query(
                r#"
                INSERT INTO production_steps (
                    run_id, revision_epoch, plan_order, step_key, step_type,
                    role_key, dependencies, status
                ) VALUES ($1, 0, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(run.id)
            .bind(plan_order as i32)
            .bind(&step.key)
            .bind(step_type)
            .bind(&step.role_key)
            .bind(serde_json::to_value(&step.dependencies)?)
            .bind(status)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            "UPDATE production_projects SET status = 'active', updated_at = NOW() WHERE id = $1",
        )
        .bind(command.intent_id)
        .execute(&mut *tx)
        .await?;
        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            json!({"run_id": run.id}),
        )
        .await?;
        tx.commit().await?;
        Ok(run)
    }

    /// 在解析当前默认模型和 active Definition 前重放已持久化的 start 命令。
    pub async fn replay_start_run(
        &self,
        intent_id: Uuid,
        actor: ProductionActor,
        idempotency_key: &str,
    ) -> ProductionResult<Option<ProductionRunRecord>> {
        let request_digest = start_run_request_digest(intent_id)?;
        let command_scope = ProductionCommandScope::new(
            actor,
            ProductionCommandType::StartRun,
            ProductionAggregateType::ProductionIntent,
            intent_id,
            idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        let Some(result) =
            ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest).await?
        else {
            tx.commit().await?;
            return Ok(None);
        };
        let run_id = uuid_from_result(&result, "run_id")?;
        tx.commit().await?;
        self.get_run_record(run_id).await.map(Some)
    }

    pub async fn get_run(&self, id: Uuid) -> ProductionResult<ProductionRunView> {
        let run = self.get_run_record(id).await?;
        let steps = sqlx::query_as::<_, ProductionStepRecord>(
            r#"
            SELECT id, run_id, revision_epoch, plan_order, step_key, step_type, role_key,
                   dependencies, status, waiting_reason, error_code, error_details, retryable, attempt,
                   lease_owner, lease_expires_at, side_effect_state,
                   agent_run_id, model_call_id, context_snapshot_id
            FROM production_steps WHERE run_id = $1
            ORDER BY revision_epoch, plan_order
            "#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let packages = json_rows(
            &self.pool,
            "SELECT to_jsonb(item) FROM artifact_package_snapshots item WHERE run_id = $1 ORDER BY created_at",
            id,
        )
        .await?;
        let gate_decisions = json_rows(
            &self.pool,
            "SELECT to_jsonb(item) FROM production_gate_decisions item WHERE run_id = $1 ORDER BY decided_at",
            id,
        )
        .await?;
        let domain_links = json_rows(
            &self.pool,
            "SELECT to_jsonb(item) FROM production_domain_links item WHERE run_id = $1 ORDER BY created_at",
            id,
        )
        .await?;
        let resource_summary = sqlx::query_scalar::<_, Value>(
            r#"
            SELECT jsonb_build_object(
                'resource_key', resource_key,
                'reserved', COALESCE(SUM(reserved_value) FILTER (WHERE status = 'reserved'), 0),
                'held_uncertain', COALESCE(SUM(reserved_value) FILTER (WHERE status = 'held_uncertain'), 0),
                -- actual usage is append-only in the usage ledger.  In particular,
                -- concurrency reservations are released after recording usage so
                -- summing only settled reservations would silently report zero.
                'actual', COALESCE((
                    SELECT SUM(usage.used_value)
                    FROM production_resource_usage usage
                    WHERE usage.run_id = reservation.run_id
                      AND usage.resource_key = reservation.resource_key
                ), 0)
            )
            FROM production_resource_reservations reservation
            WHERE reservation.run_id = $1
            GROUP BY reservation.run_id, reservation.resource_key
            ORDER BY reservation.resource_key
            "#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let plan_value = sqlx::query_scalar::<_, Value>(
            "SELECT plan FROM production_plan_snapshots WHERE id=$1",
        )
        .bind(run.plan_snapshot_id)
        .fetch_one(&self.pool)
        .await?;
        let plan: PlanSnapshot = serde_json::from_value(plan_value)?;
        let current_revision_epoch = u32::try_from(run.current_revision_epoch).map_err(|_| {
            ProductionError::TransitionConflict {
                reason: "production Run has an invalid revision epoch".into(),
            }
        })?;
        let mut run_state = RunState::new(parse_run_status(&run.status)?, current_revision_epoch);
        run_state.cancellation_requested = run.cancellation_intent.is_some();
        let workflow_steps = steps
            .iter()
            .cloned()
            .map(step_state_from_record)
            .collect::<ProductionResult<Vec<_>>>()?;
        let workflow = WorkflowSnapshot::new(run_state, workflow_steps);
        let mut allowed_commands = allowed_commands(&plan, &workflow)?;
        allowed_commands.retain(|command| {
            if command.kind != WorkflowCommandKind::RetryStep {
                return true;
            }
            command.step_key.as_deref().is_some_and(|step_key| {
                steps.iter().any(|step| {
                    step.revision_epoch == run.current_revision_epoch
                        && step.step_key == step_key
                        && step.retryable
                        && matches!(step.side_effect_state.as_str(), "none" | "prepared")
                })
            })
        });
        Ok(ProductionRunView {
            run,
            steps,
            packages,
            gate_decisions,
            domain_links,
            resource_summary,
            allowed_commands,
        })
    }

    /// 持久化一次 Run 唤醒命令，并只为当前 revision 中真正可执行的 queued step 写 outbox。
    pub async fn resume_run(
        &self,
        command: ResumeRunCommand,
    ) -> ProductionResult<AcceptedProductionCommand> {
        command.actor.validate()?;
        let request_digest =
            ProductionCommandStore::canonical_request_digest(&json!({"run_id": command.run_id}))?;
        let command_scope = ProductionCommandScope::new(
            command.actor.clone(),
            ProductionCommandType::ResumeRun,
            ProductionAggregateType::ProductionRun,
            command.run_id,
            &command.idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        if let Some(result) =
            ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest).await?
        {
            let accepted = serde_json::from_value(result).map_err(|_| {
                ProductionError::TransitionConflict {
                    reason: "stored resume_run result is invalid".into(),
                }
            })?;
            tx.commit().await?;
            return Ok(accepted);
        }

        let run = sqlx::query_as::<_, (String, i32, Option<Value>)>(
            r#"
            SELECT status,current_revision_epoch,cancellation_intent
            FROM production_runs WHERE id=$1 FOR UPDATE
            "#,
        )
        .bind(command.run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production run not found".into(),
        })?;
        if run.2.is_some()
            || matches!(
                run.0.as_str(),
                "cancelling" | "cancelled" | "failed" | "completed"
            )
        {
            return Err(ProductionError::TransitionConflict {
                reason: "terminal or cancelling run cannot be resumed".into(),
            });
        }

        let step_ids = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT step.id
            FROM production_steps step
            WHERE step.run_id=$1 AND step.revision_epoch=$2 AND step.status='queued'
              AND NOT EXISTS (
                  SELECT 1
                  FROM jsonb_array_elements_text(step.dependencies) dependency(step_key)
                  WHERE NOT EXISTS (
                      SELECT 1 FROM production_steps completed
                      WHERE completed.run_id=step.run_id
                        AND completed.revision_epoch=step.revision_epoch
                        AND completed.step_key=dependency.step_key
                        AND completed.status='succeeded'
                  )
              )
            ORDER BY step.plan_order
            FOR UPDATE
            "#,
        )
        .bind(command.run_id)
        .bind(run.1)
        .fetch_all(&mut *tx)
        .await?;
        if step_ids.is_empty() {
            return Err(ProductionError::TransitionConflict {
                reason: "run has no queued step whose dependencies are satisfied".into(),
            });
        }
        for step_id in &step_ids {
            ensure_wakeup_in_transaction(&mut tx, command.run_id, *step_id).await?;
        }
        let accepted = AcceptedProductionCommand {
            run_id: command.run_id,
            step_ids,
        };
        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            serde_json::to_value(&accepted)?,
        )
        .await?;
        tx.commit().await?;
        Ok(accepted)
    }

    /// 将当前 revision 中可重试且没有不确定副作用的失败 step 重新排队。
    pub async fn retry_step(
        &self,
        command: RetryStepCommand,
    ) -> ProductionResult<AcceptedProductionCommand> {
        command.actor.validate()?;
        let request_digest = ProductionCommandStore::canonical_request_digest(&json!({
            "run_id": command.run_id,
            "step_id": command.step_id,
        }))?;
        let command_scope = ProductionCommandScope::new(
            command.actor.clone(),
            ProductionCommandType::RetryStep,
            ProductionAggregateType::ProductionStep,
            command.step_id,
            &command.idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        if let Some(result) =
            ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest).await?
        {
            let accepted = serde_json::from_value(result).map_err(|_| {
                ProductionError::TransitionConflict {
                    reason: "stored retry_step result is invalid".into(),
                }
            })?;
            tx.commit().await?;
            return Ok(accepted);
        }

        let current = sqlx::query_as::<_, ProductionStepRecord>(
            r#"
            SELECT id,run_id,revision_epoch,plan_order,step_key,step_type,role_key,
                   dependencies,status,waiting_reason,error_code,error_details,retryable,attempt,
                   lease_owner,lease_expires_at,side_effect_state,
                   agent_run_id,model_call_id,context_snapshot_id
            FROM production_steps WHERE id=$1 AND run_id=$2 FOR UPDATE
            "#,
        )
        .bind(command.step_id)
        .bind(command.run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production step does not belong to the requested run".into(),
        })?;
        let current_epoch = sqlx::query_scalar::<_, i32>(
            r#"
            SELECT current_revision_epoch FROM production_runs
            WHERE id=$1 AND cancellation_intent IS NULL
              AND status NOT IN ('cancelling','cancelled','failed','completed')
            FOR UPDATE
            "#,
        )
        .bind(command.run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "terminal or cancelling run cannot retry a step".into(),
        })?;
        if current.revision_epoch != current_epoch
            || !matches!(current.status.as_str(), "failed" | "attention_required")
            || !current.retryable
            || !matches!(current.side_effect_state.as_str(), "none" | "prepared")
        {
            return Err(ProductionError::TransitionConflict {
                reason:
                    "step is not a retryable current-revision failure with certain side effects"
                        .into(),
            });
        }
        validate_persisted_workflow_command(
            &mut tx,
            command.run_id,
            &command.idempotency_key,
            WorkflowCommand::step(WorkflowCommandKind::RetryStep, &current.step_key),
        )
        .await?;
        if current.step_type == "role" {
            let next_attempt = current.attempt.checked_add(1).ok_or_else(|| {
                ProductionError::TransitionConflict {
                    reason: "role retry attempt exceeds PostgreSQL range".into(),
                }
            })?;
            Self::reserve_integrated_resources(
                &mut tx,
                command.run_id,
                command.step_id,
                next_attempt,
                ResourceRequest::role_retry(),
                &request_digest,
            )
            .await?;
            Self::settle_integrated_resources(
                &mut tx,
                command.run_id,
                &request_digest,
                BTreeMap::from([("role_retries".into(), 1)]),
                false,
            )
            .await?;
        }
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status='queued',waiting_reason=NULL,error_code=NULL,error_details=NULL,
                retryable=FALSE,lease_owner=NULL,lease_expires_at=NULL,
                side_effect_state='none',agent_run_id=NULL,model_call_id=NULL,
                context_snapshot_id=NULL,started_at=NULL,completed_at=NULL,updated_at=NOW()
            WHERE id=$1
            "#,
        )
        .bind(command.step_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE production_runs SET status='queued',error_code=NULL,error_details=NULL,updated_at=NOW() WHERE id=$1",
        )
        .bind(command.run_id)
        .execute(&mut *tx)
        .await?;
        ensure_wakeup_in_transaction(&mut tx, command.run_id, command.step_id).await?;
        let accepted = AcceptedProductionCommand {
            run_id: command.run_id,
            step_ids: vec![command.step_id],
        };
        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            serde_json::to_value(&accepted)?,
        )
        .await?;
        tx.commit().await?;
        Ok(accepted)
    }

    /// 读取当前 lease 对应的不可漂移 role 输入；任何缺失 package 或审计 digest 都 fail-closed。
    pub async fn load_role_prepare_snapshot(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
    ) -> ProductionResult<RolePrepareSnapshot> {
        if lease_owner.trim().is_empty() || attempt <= 0 {
            return Err(ProductionError::TransitionConflict {
                reason: "role prepare requires a lease owner and positive attempt".into(),
            });
        }
        let mut tx = self.pool.begin().await?;
        let head = sqlx::query_as::<_, RolePrepareHeadRow>(
            r#"
            SELECT run.id AS run_id, run.production_project_id, project.project_id,
                   run.current_revision_epoch, run.status AS run_status,
                   run.cancellation_intent, run.binding_snapshot, run.source_snapshot,
                   step.id AS step_id, step.revision_epoch, step.step_type, step.role_key,
                   step.dependencies, step.status AS step_status, step.attempt,
                   step.lease_owner, step.lease_expires_at, step.side_effect_state
            FROM production_steps step
            JOIN production_runs run ON run.id = step.run_id
            JOIN production_projects project ON project.id = run.production_project_id
            WHERE step.id = $1
            FOR SHARE OF step, run, project
            "#,
        )
        .bind(step_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production role step not found".into(),
        })?;
        if head.step_type != "role"
            || head.role_key.as_deref().is_none_or(str::is_empty)
            || head.step_status != "running"
            || head.attempt != attempt
            || head.lease_owner.as_deref() != Some(lease_owner)
            || head
                .lease_expires_at
                .is_none_or(|expires_at| expires_at < Utc::now())
            || !matches!(head.side_effect_state.as_str(), "none" | "prepared")
        {
            return Err(ProductionError::TransitionConflict {
                reason: "role prepare does not own the current live step lease".into(),
            });
        }
        if head.current_revision_epoch != head.revision_epoch
            || head.cancellation_intent.is_some()
            || !matches!(head.run_status.as_str(), "queued" | "running")
        {
            return Err(ProductionError::TransitionConflict {
                reason: "role step is outside the current runnable ProductionRun".into(),
            });
        }
        let role_key = head.role_key.clone().unwrap_or_default();
        let role_binding = head
            .binding_snapshot
            .get(&role_key)
            .cloned()
            .ok_or_else(|| ProductionError::CapabilityMismatch {
                reason: format!("run has no frozen binding for role {role_key}"),
            })?;
        let revision_instruction = sqlx::query_as::<_, RoleRevisionInstruction>(
            r#"
            SELECT id, revision_epoch, owner_role, actor_type, actor_id,
                   source, trust, instruction, instruction_digest
            FROM production_revision_instructions
            WHERE run_id = $1 AND revision_epoch = $2 AND owner_role = $3
            "#,
        )
        .bind(head.run_id)
        .bind(head.revision_epoch)
        .bind(&role_key)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(instruction) = &revision_instruction {
            if instruction.actor_type != "local_operator"
                || instruction.actor_id.trim().is_empty()
                || instruction.source != "script_revision_command"
                || instruction.trust != "user_instruction"
                || instruction.instruction.trim().is_empty()
                || canonical_digest(&instruction.instruction)? != instruction.instruction_digest
            {
                return Err(ProductionError::TransitionConflict {
                    reason: "revision instruction audit fields or digest are invalid".into(),
                });
            }
        }

        let rows = sqlx::query_as::<_, RolePrepareStepRow>(
            r#"
            SELECT id, plan_order, step_key, step_type, role_key, dependencies, status,
                   input_package_id, input_digest, output_digest
            FROM production_steps
            WHERE run_id = $1 AND revision_epoch = $2
            ORDER BY plan_order
            "#,
        )
        .bind(head.run_id)
        .bind(head.revision_epoch)
        .fetch_all(&mut *tx)
        .await?;
        let by_key: BTreeMap<_, _> = rows
            .iter()
            .cloned()
            .map(|row| (row.step_key.clone(), row))
            .collect();
        let mut stack = json_string_array(&head.dependencies)?;
        let mut ancestors = BTreeMap::<i32, RolePrepareStepRow>::new();
        while let Some(dependency_key) = stack.pop() {
            let dependency = by_key.get(&dependency_key).cloned().ok_or_else(|| {
                ProductionError::TransitionConflict {
                    reason: format!(
                        "role dependency {dependency_key} is absent from the frozen plan"
                    ),
                }
            })?;
            if dependency.status != "succeeded" {
                return Err(ProductionError::TransitionConflict {
                    reason: format!("role dependency {dependency_key} is not succeeded"),
                });
            }
            if ancestors.contains_key(&dependency.plan_order) {
                continue;
            }
            stack.extend(json_string_array(&dependency.dependencies)?);
            ancestors.insert(dependency.plan_order, dependency);
        }

        let source_digest = canonical_digest(&head.source_snapshot)?;
        let mut packages = BTreeMap::<Uuid, RoleInputPackage>::new();
        let mut dependency_anchors = Vec::with_capacity(ancestors.len());
        for dependency in ancestors.values() {
            let output_digest = dependency.output_digest.clone().or_else(|| {
                (dependency.step_key == "validate_source").then(|| source_digest.clone())
            });
            let output_digest =
                output_digest.ok_or_else(|| ProductionError::TransitionConflict {
                    reason: format!(
                        "role dependency {} has no deterministic output digest",
                        dependency.step_key
                    ),
                })?;
            validate_digest(&output_digest)?;
            dependency_anchors.push(RoleDependencyAnchor {
                step_id: dependency.id,
                step_key: dependency.step_key.clone(),
                step_type: dependency.step_type.clone(),
                role_key: dependency.role_key.clone(),
                output_digest,
            });

            if dependency.step_type == "gate" {
                let package_id = dependency.input_package_id.ok_or_else(|| {
                    ProductionError::TransitionConflict {
                        reason: format!(
                            "approved gate {} has no exact input package",
                            dependency.step_key
                        ),
                    }
                })?;
                let input_digest = dependency.input_digest.as_deref().ok_or_else(|| {
                    ProductionError::TransitionConflict {
                        reason: format!(
                            "approved gate {} has no exact input digest",
                            dependency.step_key
                        ),
                    }
                })?;
                let package = load_role_input_package(
                    &mut tx,
                    head.run_id,
                    head.revision_epoch,
                    package_id,
                    input_digest,
                    dependency.id,
                )
                .await?;
                packages.insert(package.id, package);
            }
        }
        if let Some(package_id) = by_key
            .values()
            .find(|row| row.id == head.step_id)
            .and_then(|row| row.input_package_id)
        {
            if !packages.contains_key(&package_id) {
                return Err(ProductionError::TransitionConflict {
                    reason: "role input_package_id is not backed by an approved ancestor Gate"
                        .into(),
                });
            }
        }
        let media_review = if matches!(role_key.as_str(), "editor" | "qc") {
            Some(load_media_review_input(&mut tx, head.run_id, head.revision_epoch).await?)
        } else {
            None
        };
        tx.commit().await?;
        Ok(RolePrepareSnapshot {
            run_id: head.run_id,
            production_project_id: head.production_project_id,
            project_id: head.project_id,
            step_id: head.step_id,
            attempt: head.attempt,
            revision_epoch: head.revision_epoch,
            role_key,
            source_snapshot: head.source_snapshot,
            role_binding,
            input_packages: packages.into_values().collect(),
            dependency_anchors,
            revision_instruction,
            media_review,
        })
    }

    /// 查询 Editor/QC 当前 revision 的唯一完整媒体输入，供合同测试和应用查询复用。
    pub async fn media_review_input(
        &self,
        run_id: Uuid,
        revision_epoch: i32,
    ) -> ProductionResult<MediaReviewInput> {
        let mut tx = self.pool.begin().await?;
        let input = load_media_review_input(&mut tx, run_id, revision_epoch).await?;
        tx.commit().await?;
        Ok(input)
    }

    /// 从当前 revision 的唯一媒体、Editor 和 QC attempt 构建 QualityPackage。
    ///
    /// 历史 WorkVersion、旧 inventory/evidence、legacy 行和旧 attempt 只保留审计，
    /// 不参与当前包；当前 attempt 内出现重复 shot/take 时直接 fail-closed。
    pub async fn build_quality_package(
        &self,
        run_id: Uuid,
        package_version: u32,
    ) -> ProductionResult<QualityPackageBuild> {
        if package_version == 0 {
            return Err(quality_package_blocker("quality_package_version_invalid"));
        }
        let mut tx = self.pool.begin().await?;
        let revision_epoch = sqlx::query_scalar::<_, i32>(
            "SELECT current_revision_epoch FROM production_runs WHERE id=$1 FOR SHARE",
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| quality_package_blocker("quality_run_missing"))?;
        let media = load_media_review_input(&mut tx, run_id, revision_epoch).await?;
        let editor =
            load_current_quality_role_step(&mut tx, run_id, revision_epoch, "editor").await?;
        let qc = load_current_quality_role_step(&mut tx, run_id, revision_epoch, "qc").await?;

        let ledger_rows = sqlx::query_as::<_, QualityLedgerRow>(
            r#"
            SELECT id,step_id,attempt,revision_epoch,work_version_id,inventory_id,
                   evidence_snapshot_id,shot_contract_id,version,content_digest,content
            FROM continuity_ledgers
            WHERE run_id=$1 AND step_id=$2 AND attempt=$3 AND revision_epoch=$4
              AND work_version_id=$5 AND inventory_id=$6 AND evidence_snapshot_id=$7
              AND audit_status='complete'
            ORDER BY shot_contract_id,version,id
            FOR SHARE
            "#,
        )
        .bind(run_id)
        .bind(editor.0)
        .bind(editor.1)
        .bind(revision_epoch)
        .bind(media.inventory.work_version_id)
        .bind(media.inventory.inventory_id)
        .bind(media.evidence.evidence_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut ledger_refs = BTreeMap::new();
        let mut continuity_ledgers = Vec::with_capacity(ledger_rows.len());
        let mut package_items = Vec::with_capacity(ledger_rows.len() + 4);
        package_items.push(ArtifactRef {
            run_id,
            artifact_type: "required_take_inventory".into(),
            artifact_id: media.inventory.inventory_id,
            version: 1,
            content_digest: media.inventory.inventory_digest.clone(),
            source_step_id: media.inventory.source_step_id,
            source_attempt: media.inventory.source_attempt,
        });
        package_items.push(ArtifactRef {
            run_id,
            artifact_type: "media_evidence".into(),
            artifact_id: media.evidence.evidence_id,
            version: 1,
            content_digest: media.evidence.evidence_digest.clone(),
            source_step_id: media.evidence.source_step_id,
            source_attempt: media.evidence.source_attempt,
        });
        for row in ledger_rows {
            let output: crate::state::artifacts::output_contract::ContinuityLedgerOutput =
                serde_json::from_value(row.content.clone())
                    .map_err(|_| quality_package_blocker("continuity_content_invalid"))?;
            if row.step_id != editor.0
                || row.attempt != editor.1
                || row.revision_epoch != revision_epoch
                || row.work_version_id != media.inventory.work_version_id
                || row.inventory_id != media.inventory.inventory_id
                || row.evidence_snapshot_id != media.evidence.evidence_id
                || row.shot_contract_id != output.shot_contract_id
                || output.work_version_id != row.work_version_id
                || output.inventory_id != row.inventory_id
                || output.evidence_snapshot_id != row.evidence_snapshot_id
                || row.version <= 0
                || canonical_digest(&row.content)? != row.content_digest
            {
                return Err(quality_package_blocker("continuity_scope_stale"));
            }
            let version = u32::try_from(row.version)
                .map_err(|_| quality_package_blocker("continuity_version_invalid"))?;
            if ledger_refs
                .insert(
                    row.shot_contract_id,
                    ApplicableLedgerVersionRow {
                        continuity_ledger_id: row.id,
                        shot_contract_id: row.shot_contract_id,
                        ledger_version: row.version,
                        content_digest: row.content_digest.clone(),
                    },
                )
                .is_some()
            {
                return Err(quality_package_blocker("continuity_current_ambiguous"));
            }
            continuity_ledgers.push(QualityContinuityLedger {
                id: row.id,
                run_id,
                revision_epoch: revision_epoch as u32,
                work_version_id: row.work_version_id,
                inventory_id: row.inventory_id,
                inventory_digest: media.inventory.inventory_digest.clone(),
                evidence_snapshot_id: row.evidence_snapshot_id,
                shot_contract_id: row.shot_contract_id,
                version,
            });
            package_items.push(ArtifactRef {
                run_id,
                artifact_type: "continuity_ledger".into(),
                artifact_id: row.id,
                version,
                content_digest: row.content_digest,
                source_step_id: row.step_id,
                source_attempt: row.attempt as u32,
            });
        }

        let review_rows = sqlx::query_as::<_, QualityReviewRow>(
            r#"
            SELECT id,step_id,attempt,revision_epoch,work_version_id,inventory_id,
                   evidence_snapshot_id,required_take_id,version,content_digest,status,content
            FROM take_reviews
            WHERE run_id=$1 AND step_id=$2 AND attempt=$3 AND revision_epoch=$4
              AND work_version_id=$5 AND inventory_id=$6 AND evidence_snapshot_id=$7
              AND audit_status='complete'
            ORDER BY required_take_id,version,id
            FOR SHARE
            "#,
        )
        .bind(run_id)
        .bind(qc.0)
        .bind(qc.1)
        .bind(revision_epoch)
        .bind(media.inventory.work_version_id)
        .bind(media.inventory.inventory_id)
        .bind(media.evidence.evidence_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut take_reviews = Vec::with_capacity(review_rows.len());
        let mut current_takes = BTreeSet::new();
        for row in review_rows {
            let output: crate::state::artifacts::output_contract::TakeReviewOutput =
                serde_json::from_value(row.content.clone())
                    .map_err(|_| quality_package_blocker("take_review_content_invalid"))?;
            if row.step_id != qc.0
                || row.attempt != qc.1
                || row.revision_epoch != revision_epoch
                || row.work_version_id != media.inventory.work_version_id
                || row.inventory_id != media.inventory.inventory_id
                || row.evidence_snapshot_id != media.evidence.evidence_id
                || row.required_take_id != output.required_take_id
                || output.work_version_id != row.work_version_id
                || output.inventory_id != row.inventory_id
                || output.evidence_snapshot_id != row.evidence_snapshot_id
                || row.status != output.review_status
                || row.version <= 0
                || canonical_digest(&row.content)? != row.content_digest
            {
                return Err(quality_package_blocker("take_review_scope_stale"));
            }
            if !current_takes.insert(row.required_take_id) {
                return Err(quality_package_blocker("take_review_current_ambiguous"));
            }
            let mappings = sqlx::query_as::<_, ApplicableLedgerVersionRow>(
                r#"
                SELECT continuity_ledger_id,shot_contract_id,ledger_version,content_digest
                FROM take_review_ledger_versions
                WHERE take_review_id=$1
                ORDER BY ordinal
                "#,
            )
            .bind(row.id)
            .fetch_all(&mut *tx)
            .await?;
            let take = media
                .inventory
                .takes
                .iter()
                .find(|take| take.take_id == row.required_take_id)
                .ok_or_else(|| quality_package_blocker("take_review_required_take_stale"))?;
            let mut ordered_shots = Vec::new();
            let mut seen_shots = BTreeSet::new();
            for scene_id in &take.scene_ids {
                for shot_id in &take.scene_shot_map[scene_id] {
                    if seen_shots.insert(*shot_id) {
                        ordered_shots.push(*shot_id);
                    }
                }
            }
            let expected = ordered_shots
                .iter()
                .map(|shot_id| ledger_refs.get(shot_id).cloned())
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| quality_package_blocker("take_review_ledger_missing"))?;
            if mappings != expected {
                return Err(quality_package_blocker("take_review_ledger_version_stale"));
            }
            let status = parse_quality_review_status(&row.status)?;
            let version = u32::try_from(row.version)
                .map_err(|_| quality_package_blocker("take_review_version_invalid"))?;
            take_reviews.push(QualityTakeReview {
                id: row.id,
                run_id,
                revision_epoch: revision_epoch as u32,
                work_version_id: row.work_version_id,
                inventory_id: row.inventory_id,
                inventory_digest: media.inventory.inventory_digest.clone(),
                evidence_snapshot_id: row.evidence_snapshot_id,
                required_take_id: row.required_take_id,
                applicable_shot_contract_ids: output.applicable_shot_contract_ids,
                status,
                version,
            });
            package_items.push(ArtifactRef {
                run_id,
                artifact_type: "take_review".into(),
                artifact_id: row.id,
                version,
                content_digest: row.content_digest,
                source_step_id: row.step_id,
                source_attempt: row.attempt as u32,
            });
        }
        let gate_input = QualityGateInput {
            media_review: media,
            continuity_ledgers,
            take_reviews,
        };
        let outcome = QualityGate::evaluate(&gate_input)?;
        let metadata = json!({
            "work_version_id": gate_input.media_review.inventory.work_version_id,
            "inventory_id": gate_input.media_review.inventory.inventory_id,
            "inventory_digest": gate_input.media_review.inventory.inventory_digest,
            "evidence_snapshot_id": gate_input.media_review.evidence.evidence_id,
            "evidence_digest": gate_input.media_review.evidence.evidence_digest,
            "quality_gate_input": gate_input,
        });
        let package = ArtifactPackageSnapshot::build(
            PackageType::Quality,
            run_id,
            qc.0,
            qc.1 as u32,
            revision_epoch as u32,
            package_version,
            package_items,
            metadata,
        )?;
        tx.commit().await?;
        Ok(QualityPackageBuild {
            package,
            gate_input,
            outcome,
        })
    }

    /// 为 role attempt 创建 `agent_runs` 与不可变 binding，并先关联回 ProductionStep。
    pub async fn create_role_agent_run(
        &self,
        snapshot: &RolePrepareSnapshot,
        lease_owner: &str,
        binding: &PreparedAgentBindingInput,
        input_snapshot: &Value,
    ) -> ProductionResult<Uuid> {
        validate_safe_object(input_snapshot)?;
        let mut tx = self.pool.begin().await?;
        let step =
            lock_owned_step(&mut tx, snapshot.step_id, lease_owner, snapshot.attempt).await?;
        if step.run_id != snapshot.run_id
            || step.revision_epoch != snapshot.revision_epoch
            || step.role_key.as_deref() != Some(snapshot.role_key.as_str())
            || step.agent_run_id.is_some()
            || step.model_call_id.is_some()
            || step.context_snapshot_id.is_some()
        {
            return Err(ProductionError::TransitionConflict {
                reason: "role attempt audit anchor already exists or no longer matches".into(),
            });
        }
        let agent_run_id = Uuid::new_v4();
        let model_snapshot = json!({
            "model_id": binding.model_id,
            "behavior_fingerprint": binding.behavior_fingerprint,
            "capabilities": binding.model_capabilities,
            "tokenizer_profile": {
                "key": binding.tokenizer_profile_key,
                "version": binding.tokenizer_profile_version,
                "digest": binding.tokenizer_profile_digest,
            }
        });
        sqlx::query(
            r#"
            INSERT INTO agent_runs (
                id, project_id, agent_type, status, input, model_id, model_snapshot
            ) VALUES ($1, $2, 'production', 'running', $3, $4, $5)
            "#,
        )
        .bind(agent_run_id)
        .bind(snapshot.project_id)
        .bind(input_snapshot)
        .bind(binding.model_id)
        .bind(model_snapshot)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO agent_run_bindings (
                agent_run_id, agent_key, agent_version, agent_digest, prompt_bindings,
                context_policy_bindings, registry_digest, model_id, behavior_fingerprint,
                model_capabilities, tokenizer_profile_key, tokenizer_profile_version,
                tokenizer_profile_digest, legacy_partial_audit
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, FALSE
            )
            "#,
        )
        .bind(agent_run_id)
        .bind(&binding.agent_key)
        .bind(&binding.agent_version)
        .bind(&binding.agent_digest)
        .bind(&binding.prompt_bindings)
        .bind(&binding.context_policy_bindings)
        .bind(&binding.registry_digest)
        .bind(binding.model_id)
        .bind(&binding.behavior_fingerprint)
        .bind(&binding.model_capabilities)
        .bind(&binding.tokenizer_profile_key)
        .bind(&binding.tokenizer_profile_version)
        .bind(&binding.tokenizer_profile_digest)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE production_steps SET agent_run_id=$2, updated_at=NOW() WHERE id=$1")
            .bind(snapshot.step_id)
            .bind(agent_run_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            UPDATE production_step_attempts SET agent_run_id=$3
            WHERE step_id=$1 AND attempt_no=$2 AND status IN ('running', 'prepared')
            "#,
        )
        .bind(snapshot.step_id)
        .bind(snapshot.attempt)
        .bind(agent_run_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE production_runs SET status='running', updated_at=NOW() WHERE id=$1 AND status='queued'",
        )
        .bind(snapshot.run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(agent_run_id)
    }

    /// 将成功 ContextSnapshot 与 prepared ModelCall 原子关联到当前 ProductionStep/attempt。
    #[allow(clippy::too_many_arguments)]
    pub async fn attach_role_prepare_audit(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
        agent_run_id: Uuid,
        model_call_id: Uuid,
        context_snapshot_id: Uuid,
    ) -> ProductionResult<()> {
        let mut tx = self.pool.begin().await?;
        let step = lock_owned_step(&mut tx, step_id, lease_owner, attempt).await?;
        if step.agent_run_id != Some(agent_run_id)
            || step.model_call_id.is_some()
            || step.context_snapshot_id.is_some()
        {
            return Err(ProductionError::TransitionConflict {
                reason: "role prepare audit owner is inconsistent".into(),
            });
        }
        let anchors_match = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM model_calls call
                JOIN context_snapshots context ON context.id = call.context_snapshot_id
                WHERE call.id=$1 AND call.agent_run_id=$2 AND call.status='prepared'
                  AND context.id=$3 AND context.agent_run_id=$2
            )
            "#,
        )
        .bind(model_call_id)
        .bind(agent_run_id)
        .bind(context_snapshot_id)
        .fetch_one(&mut *tx)
        .await?;
        if !anchors_match {
            return Err(ProductionError::TransitionConflict {
                reason: "prepared ModelCall and ContextSnapshot audit anchors are incomplete"
                    .into(),
            });
        }
        sqlx::query(
            r#"
            UPDATE production_steps
            SET model_call_id=$2, context_snapshot_id=$3, side_effect_state='prepared',
                updated_at=NOW()
            WHERE id=$1
            "#,
        )
        .bind(step_id)
        .bind(model_call_id)
        .bind(context_snapshot_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_step_attempts
            SET status='prepared', side_effect_state='prepared', model_call_id=$3,
                context_snapshot_id=$4
            WHERE step_id=$1 AND attempt_no=$2 AND agent_run_id=$5
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .bind(model_call_id)
        .bind(context_snapshot_id)
        .bind(agent_run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// prepare 在 provider 前失败时闭合 step/attempt/agent_run，并释放未使用预占。
    pub async fn fail_role_prepare(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
        agent_run_id: Option<Uuid>,
        error_code: &str,
        message: &str,
    ) -> ProductionResult<()> {
        validate_stable_error_code(error_code)?;
        let mut tx = self.pool.begin().await?;
        let step = lock_owned_step(&mut tx, step_id, lease_owner, attempt).await?;
        if let Some(agent_run_id) = agent_run_id {
            if step.agent_run_id != Some(agent_run_id) {
                return Err(ProductionError::TransitionConflict {
                    reason: "failed role prepare does not own the AgentRun".into(),
                });
            }
            sqlx::query(
                r#"
                UPDATE agent_runs SET status='failed', error_message=$2, ended_at=NOW()
                WHERE id=$1 AND status IN ('pending', 'running')
                "#,
            )
            .bind(agent_run_id)
            .bind(message)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                UPDATE model_calls
                SET status='aborted', error_snapshot=$2, completed_at=NOW()
                WHERE agent_run_id=$1 AND status='prepared'
                "#,
            )
            .bind(agent_run_id)
            .bind(json!({"kind": error_code, "message": message}))
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            r#"
            UPDATE production_resource_reservations
            SET status='released', settled_at=NOW()
            WHERE step_id=$1 AND attempt_no=$2 AND status='reserved'
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .execute(&mut *tx)
        .await?;
        let details = json!({"code": error_code, "message": message});
        sqlx::query(
            r#"
            UPDATE production_step_attempts
            SET status='failed', side_effect_state='none', error_details=$3,
                completed_at=NOW()
            WHERE step_id=$1 AND attempt_no=$2 AND status IN ('running', 'prepared')
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .bind(&details)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status='failed', error_code=$2, error_details=$3, retryable=FALSE,
                lease_owner=NULL, lease_expires_at=NULL, side_effect_state='none',
                completed_at=NOW(), updated_at=NOW()
            WHERE id=$1
            "#,
        )
        .bind(step_id)
        .bind(error_code)
        .bind(&details)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_runs
            SET status='blocked', error_code=$2, error_details=$3, updated_at=NOW()
            WHERE id=$1 AND status IN ('queued', 'running')
            "#,
        )
        .bind(step.run_id)
        .bind(error_code)
        .bind(details)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// 将 typed role 输出、资源、ModelCall、AgentRun 和 Step 原子提交。
    pub async fn finalize_role_execution(
        &self,
        lease_owner: &str,
        command: RoleFinalizeCommand,
    ) -> ProductionResult<RoleFinalizeRecord> {
        validate_role_finalize_command(&command)?;
        if let Some(record) = self.load_role_finalize_replay(&command).await? {
            return Ok(record);
        }

        let finish = sanitize_model_call_finish(&command.model_call_finish)?;
        let mut tx = self.pool.begin().await?;
        let step = lock_owned_step(&mut tx, command.step_id, lease_owner, command.attempt).await?;
        if step.run_id != command.run_id
            || step.revision_epoch != command.revision_epoch
            || step.role_key.as_deref() != Some(command.role_key.as_str())
            || step.agent_run_id != Some(command.agent_run_id)
            || step.model_call_id != Some(command.model_call_id)
            || step.context_snapshot_id != Some(command.context_snapshot_id)
            || step.side_effect_state != "prepared"
        {
            return Err(ProductionError::TransitionConflict {
                reason: "role finalize does not match the prepared step anchors".into(),
            });
        }
        let project_matches = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM production_runs run
                WHERE run.id=$1 AND run.production_project_id=$2
                  AND run.current_revision_epoch=$3
                  AND run.status IN ('queued', 'running')
            )
            "#,
        )
        .bind(command.run_id)
        .bind(command.production_project_id)
        .bind(command.revision_epoch)
        .fetch_one(&mut *tx)
        .await?;
        if !project_matches {
            return Err(ProductionError::TransitionConflict {
                reason: "role finalize run/project/revision is stale".into(),
            });
        }
        let audit_matches = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM agent_runs agent
                JOIN model_calls call ON call.agent_run_id=agent.id
                JOIN context_snapshots context ON context.id=call.context_snapshot_id
                WHERE agent.id=$1 AND agent.status='running'
                  AND call.id=$2 AND call.status='prepared'
                  AND context.id=$3 AND context.agent_run_id=agent.id
            )
            "#,
        )
        .bind(command.agent_run_id)
        .bind(command.model_call_id)
        .bind(command.context_snapshot_id)
        .fetch_one(&mut *tx)
        .await?;
        if !audit_matches {
            return Err(ProductionError::TransitionConflict {
                reason: "role finalize audit anchors are not prepared".into(),
            });
        }
        sqlx::query("SELECT id FROM production_projects WHERE id=$1 FOR UPDATE")
            .bind(command.production_project_id)
            .fetch_one(&mut *tx)
            .await?;

        let output_artifacts = if let Some(validated) = &command.validated_output {
            persist_validated_role_output(&mut tx, &command, validated).await?
        } else {
            Vec::new()
        };
        settle_role_resources(&mut tx, &command).await?;
        finish_prepared_model_call(&mut tx, command.model_call_id, &finish).await?;

        let record = RoleFinalizeRecord {
            role: command.role_key.clone(),
            status: if command.failure.is_some() {
                "failed".into()
            } else {
                "succeeded".into()
            },
            output_artifacts,
            model_call_id: command.model_call_id,
            execution_time_ms: command.execution_time_ms,
            error: command.failure.clone(),
        };
        let result = serde_json::to_value(&record)?;
        if let Some(failure) = &command.failure {
            let step_status = if failure.result_uncertain {
                "attention_required"
            } else {
                "failed"
            };
            let side_effect_state = if failure.result_uncertain {
                "unknown"
            } else {
                "confirmed"
            };
            let details = json!({
                "code": failure.code,
                "message": failure.message,
                "result_uncertain": failure.result_uncertain,
            });
            sqlx::query(
                r#"
                UPDATE agent_runs
                SET status='failed', output=$2, error_message=$3, ended_at=NOW()
                WHERE id=$1 AND status='running'
                "#,
            )
            .bind(command.agent_run_id)
            .bind(&command.output)
            .bind(&failure.message)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                UPDATE production_step_attempts
                SET status=$3, side_effect_state=$4, result=$5, error_details=$6,
                    completed_at=NOW()
                WHERE step_id=$1 AND attempt_no=$2 AND status='prepared'
                "#,
            )
            .bind(command.step_id)
            .bind(command.attempt)
            .bind(step_status)
            .bind(side_effect_state)
            .bind(&result)
            .bind(&details)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                UPDATE production_steps
                SET status=$2, side_effect_state=$3, error_code=$4, error_details=$5,
                    retryable=FALSE, lease_owner=NULL, lease_expires_at=NULL,
                    completed_at=NOW(), updated_at=NOW()
                WHERE id=$1
                "#,
            )
            .bind(command.step_id)
            .bind(step_status)
            .bind(side_effect_state)
            .bind(&failure.code)
            .bind(&details)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE production_runs SET status=$2, error_code=$3, error_details=$4, updated_at=NOW() WHERE id=$1",
            )
            .bind(command.run_id)
            .bind(if failure.result_uncertain {
                "attention_required"
            } else {
                "blocked"
            })
            .bind(&failure.code)
            .bind(details)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE agent_runs
                SET status='succeeded', output=$2, error_message=NULL, ended_at=NOW()
                WHERE id=$1 AND status='running'
                "#,
            )
            .bind(command.agent_run_id)
            .bind(&command.output)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                UPDATE production_step_attempts
                SET status='succeeded', side_effect_state='confirmed', result=$3,
                    error_details=NULL, completed_at=NOW()
                WHERE step_id=$1 AND attempt_no=$2 AND status='prepared'
                "#,
            )
            .bind(command.step_id)
            .bind(command.attempt)
            .bind(&result)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                UPDATE production_steps
                SET status='succeeded', side_effect_state='confirmed', output_digest=$2,
                    error_code=NULL, error_details=NULL, retryable=FALSE,
                    lease_owner=NULL, lease_expires_at=NULL, completed_at=NOW(), updated_at=NOW()
                WHERE id=$1
                "#,
            )
            .bind(command.step_id)
            .bind(&command.output_digest)
            .execute(&mut *tx)
            .await?;
            unlock_ready_steps(&mut tx, command.run_id, command.revision_epoch).await?;
            sqlx::query(
                "UPDATE production_runs SET status='queued', error_code=NULL, error_details=NULL, updated_at=NOW() WHERE id=$1",
            )
            .bind(command.run_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(record)
    }

    async fn load_role_finalize_replay(
        &self,
        command: &RoleFinalizeCommand,
    ) -> ProductionResult<Option<RoleFinalizeRecord>> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                Option<Value>,
                Option<Uuid>,
                Option<Uuid>,
                Option<Uuid>,
            ),
        >(
            r#"
            SELECT status, result, agent_run_id, model_call_id, context_snapshot_id
            FROM production_step_attempts
            WHERE step_id=$1 AND attempt_no=$2
            "#,
        )
        .bind(command.step_id)
        .bind(command.attempt)
        .fetch_optional(&self.pool)
        .await?;
        let Some((status, result, agent_run_id, model_call_id, context_snapshot_id)) = row else {
            return Ok(None);
        };
        if !matches!(
            status.as_str(),
            "succeeded" | "failed" | "attention_required"
        ) {
            return Ok(None);
        }
        if agent_run_id != Some(command.agent_run_id)
            || model_call_id != Some(command.model_call_id)
            || context_snapshot_id != Some(command.context_snapshot_id)
        {
            return Err(ProductionError::IdempotencyConflict);
        }
        let result = result.ok_or_else(|| ProductionError::TransitionConflict {
            reason: "terminal role attempt is missing its immutable finalize result".into(),
        })?;
        Ok(Some(serde_json::from_value(result)?))
    }

    /// 业务事务失败后，在新事务中闭合 prepared 审计，且不保留部分产物。
    pub async fn fail_role_finalize_database(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
        agent_run_id: Uuid,
        model_call_id: Uuid,
        message: &str,
    ) -> ProductionResult<()> {
        let mut tx = self.pool.begin().await?;
        let step = lock_owned_step(&mut tx, step_id, lease_owner, attempt).await?;
        if step.agent_run_id != Some(agent_run_id) || step.model_call_id != Some(model_call_id) {
            return Err(ProductionError::TransitionConflict {
                reason: "database finalize failure does not own the prepared audit".into(),
            });
        }
        let failure = RoleFinalizeFailure {
            code: "database_error".into(),
            message: message.into(),
            result_uncertain: false,
        };
        let record = RoleFinalizeRecord {
            role: step.role_key.clone().unwrap_or_default(),
            status: "failed".into(),
            output_artifacts: vec![],
            model_call_id,
            execution_time_ms: 0,
            error: Some(failure.clone()),
        };
        let details = json!({"code": failure.code, "message": failure.message});
        settle_reserved_resources_after_database_failure(&mut tx, step_id, attempt).await?;
        sqlx::query(
            r#"
            UPDATE model_calls
            SET status='failed', error_snapshot=$2, structured_parse_status='succeeded',
                completed_at=NOW()
            WHERE id=$1 AND status='prepared'
            "#,
        )
        .bind(model_call_id)
        .bind(json!({"kind": "database_finalize", "message": message}))
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE agent_runs SET status='failed', error_message=$2, ended_at=NOW() WHERE id=$1 AND status='running'",
        )
        .bind(agent_run_id)
        .bind(message)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_step_attempts
            SET status='failed', side_effect_state='confirmed', result=$3,
                error_details=$4, completed_at=NOW()
            WHERE step_id=$1 AND attempt_no=$2 AND status='prepared'
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .bind(serde_json::to_value(record)?)
        .bind(&details)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status='failed', side_effect_state='confirmed', error_code='database_error',
                error_details=$2, retryable=FALSE, lease_owner=NULL, lease_expires_at=NULL,
                completed_at=NOW(), updated_at=NOW()
            WHERE id=$1
            "#,
        )
        .bind(step_id)
        .bind(&details)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE production_runs SET status='blocked', error_code='database_error', error_details=$2, updated_at=NOW() WHERE id=$1",
        )
        .bind(step.run_id)
        .bind(details)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// 从当前 epoch 的唯一 Producer attempt 构建 BriefPackage。
    pub async fn build_brief_package(
        &self,
        run_id: Uuid,
        package_version: u32,
    ) -> ProductionResult<ArtifactPackageSnapshot> {
        self.build_role_package(
            run_id,
            package_version,
            PackageType::Brief,
            "producer",
            &[("creative_briefs", "creative_brief", 1, 1)],
        )
        .await
    }

    /// 从当前 epoch 的唯一 Screenwriter attempt 构建 ScriptPackage。
    pub async fn build_script_package(
        &self,
        run_id: Uuid,
        package_version: u32,
    ) -> ProductionResult<ArtifactPackageSnapshot> {
        self.build_role_package(
            run_id,
            package_version,
            PackageType::Script,
            "screenwriter",
            &[
                ("story_bibles", "story_bible", 1, 1),
                ("character_bibles", "character_bible", 1, usize::MAX),
                ("script_drafts", "script_draft", 1, 1),
            ],
        )
        .await
    }

    async fn build_role_package(
        &self,
        run_id: Uuid,
        package_version: u32,
        package_type: PackageType,
        role_key: &str,
        artifact_specs: &[(&'static str, &'static str, usize, usize)],
    ) -> ProductionResult<ArtifactPackageSnapshot> {
        if package_version == 0 {
            return Err(ProductionError::TransitionConflict {
                reason: "package version must be positive".into(),
            });
        }
        let mut tx = self.pool.begin().await?;
        let scope = sqlx::query_as::<_, (Uuid, i32)>(
            "SELECT production_project_id,current_revision_epoch FROM production_runs WHERE id=$1 FOR SHARE",
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production Run does not exist".into(),
        })?;
        let steps = sqlx::query_as::<_, ProductionPackageContributorStep>(
            r#"
            SELECT id,role_key,plan_order,attempt
            FROM production_steps
            WHERE run_id=$1 AND revision_epoch=$2 AND role_key=$3
              AND status='succeeded' AND attempt > 0
            ORDER BY plan_order,id
            "#,
        )
        .bind(run_id)
        .bind(scope.1)
        .bind(role_key)
        .fetch_all(&mut *tx)
        .await?;
        if steps.len() != 1 {
            return Err(ProductionError::TransitionConflict {
                reason: format!(
                    "{package_type:?}Package requires one current succeeded {role_key} step"
                ),
            });
        }
        let source_step = &steps[0];
        let mut items = Vec::new();
        for (table, artifact_type, minimum, maximum) in artifact_specs {
            let rows = load_scoped_process_artifacts(
                &mut tx,
                table,
                scope.0,
                run_id,
                scope.1,
                source_step,
            )
            .await?;
            if rows.len() < *minimum || rows.len() > *maximum {
                return Err(ProductionError::TransitionConflict {
                    reason: format!(
                        "{package_type:?}Package has invalid {artifact_type} cardinality"
                    ),
                });
            }
            for row in rows {
                items.push(process_artifact_ref(run_id, artifact_type, &row)?);
            }
        }
        let package = ArtifactPackageSnapshot::build(
            package_type,
            run_id,
            source_step.id,
            positive_u32(source_step.attempt, "package source attempt")?,
            u32::try_from(scope.1).map_err(|_| ProductionError::TransitionConflict {
                reason: "package revision epoch is invalid".into(),
            })?,
            package_version,
            items,
            json!({}),
        )?;
        tx.commit().await?;
        Ok(package)
    }

    /// 只从当前 Run、正式 Script 和精确成功 attempt 构建 ProductionPackage。
    pub async fn build_production_package(
        &self,
        run_id: Uuid,
        package_version: u32,
    ) -> ProductionResult<ArtifactPackageSnapshot> {
        if package_version == 0 {
            return Err(production_package_error(
                "production package version must be positive",
            ));
        }
        let mut tx = self.pool.begin().await?;
        let scope = sqlx::query_as::<_, ProductionPackageRunScope>(
            r#"
            SELECT production_project_id, current_revision_epoch
            FROM production_runs WHERE id=$1 FOR SHARE
            "#,
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| production_package_error("production Run does not exist"))?;
        let scripts = sqlx::query_as::<_, ProductionPackageScriptRow>(
            r#"
            SELECT script.id, link.target_version, link.target_digest,
                   script.source_artifacts
            FROM scripts script
            JOIN production_domain_links link
              ON link.run_id=$1 AND link.link_type='script' AND link.script_id=script.id
            WHERE script.production_run_id=$1 AND script.status='approved'
              AND script.source_revision_epoch=(
                  SELECT MAX(candidate.source_revision_epoch)
                  FROM scripts candidate
                  JOIN production_domain_links candidate_link
                    ON candidate_link.run_id=$1 AND candidate_link.link_type='script'
                   AND candidate_link.script_id=candidate.id
                  WHERE candidate.production_run_id=$1 AND candidate.status='approved'
                    AND candidate.source_revision_epoch <= $2
              )
            ORDER BY script.id
            "#,
        )
        .bind(run_id)
        .bind(scope.current_revision_epoch)
        .fetch_all(&mut *tx)
        .await?;
        if scripts.len() != 1 {
            return Err(production_package_error(
                "current Run must have exactly one current formal Script",
            ));
        }
        let script = &scripts[0];
        let scenes = sqlx::query_as::<_, ProductionPackageSceneRow>(
            r#"
            SELECT scene.id, scene.sequence, scene.duration_sec,
                   link.target_version, link.target_digest
            FROM scenes scene
            JOIN production_domain_links link
              ON link.run_id=$1 AND link.link_type='scene' AND link.scene_id=scene.id
            WHERE scene.script_id=$2
            ORDER BY scene.sequence, scene.id
            "#,
        )
        .bind(run_id)
        .bind(script.id)
        .fetch_all(&mut *tx)
        .await?;
        let formal_scene_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scenes WHERE script_id=$1")
                .bind(script.id)
                .fetch_one(&mut *tx)
                .await?;
        if scenes.is_empty() || scenes.len() as i64 != formal_scene_count {
            return Err(production_package_error(
                "formal Script Scene domain links are incomplete",
            ));
        }

        let source_artifacts: Vec<ProductionPackageScriptArtifact> =
            serde_json::from_value(script.source_artifacts.clone()).map_err(|error| {
                production_package_error(format!("Script source_artifacts are invalid: {error}"))
            })?;
        let draft_sources = source_artifacts
            .iter()
            .filter(|artifact| artifact.artifact_type == "script_draft")
            .collect::<Vec<_>>();
        let character_sources = source_artifacts
            .iter()
            .filter(|artifact| artifact.artifact_type == "character_bible")
            .collect::<Vec<_>>();
        if draft_sources.len() != 1 || character_sources.is_empty() {
            return Err(production_package_error(
                "formal Script must retain one ScriptDraft and its CharacterBible sources",
            ));
        }
        let draft_source = draft_sources[0];
        let draft =
            load_exact_script_artifact(&mut tx, "script_drafts", run_id, draft_source).await?;
        let draft_output: ScriptDraftOutput =
            serde_json::from_value(draft.content).map_err(|error| {
                production_package_error(format!("formal ScriptDraft source is invalid: {error}"))
            })?;
        if draft_output.scenes.len() != scenes.len() {
            return Err(production_package_error(
                "formal Scene set does not match the promoted ScriptDraft",
            ));
        }

        let mut characters = Vec::with_capacity(character_sources.len());
        let mut character_id_map = BTreeMap::new();
        for source in character_sources {
            let row = sqlx::query_as::<_, ProductionPackageCharacterRow>(
                r#"
                SELECT id, character_id, version, content, content_digest, step_id, attempt
                FROM character_bibles
                WHERE id=$1 AND version=$2 AND content_digest=$3
                  AND run_id=$4 AND step_id=$5 AND attempt=$6
                  AND audit_status='complete'
                "#,
            )
            .bind(source.artifact_id)
            .bind(source.artifact_version)
            .bind(&source.content_digest)
            .bind(run_id)
            .bind(source.source_step_id)
            .bind(source.source_attempt)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| {
                production_package_error(
                    "formal Script CharacterBible source provenance is incomplete",
                )
            })?;
            let content: CharacterBibleOutput =
                serde_json::from_value(row.content).map_err(|error| {
                    production_package_error(format!(
                        "formal CharacterBible source is invalid: {error}"
                    ))
                })?;
            if row.character_id != content.character_id
                || row.id != source.artifact_id
                || row.version != source.artifact_version
                || row.content_digest.trim() != source.content_digest
                || row.step_id != source.source_step_id
                || row.attempt != source.source_attempt
                || character_id_map
                    .insert(content.character_id.clone(), row.id)
                    .is_some()
            {
                return Err(production_package_error(
                    "formal CharacterBible identity or provenance is inconsistent",
                ));
            }
            characters.push(ProductionCharacterRef {
                character_bible_id: row.id,
                character_id: content.character_id,
            });
        }
        characters.sort_by(|left, right| {
            (&left.character_id, left.character_bible_id)
                .cmp(&(&right.character_id, right.character_bible_id))
        });

        let mut scene_refs = Vec::with_capacity(scenes.len());
        for (index, scene) in scenes.iter().enumerate() {
            let draft_scene = &draft_output.scenes[index];
            if scene.sequence != i32::try_from(draft_scene.sequence).unwrap_or(-1)
                || scene.duration_sec != i32::try_from(draft_scene.duration_sec).unwrap_or(-1)
            {
                return Err(production_package_error(
                    "formal Scene sequence/duration differs from its promoted ScriptDraft",
                ));
            }
            let character_bible_ids = draft_scene
                .character_ids
                .iter()
                .map(|character_id| {
                    character_id_map.get(character_id).copied().ok_or_else(|| {
                        production_package_error(
                            "ScriptDraft Scene references an unknown CharacterBible",
                        )
                    })
                })
                .collect::<ProductionResult<Vec<_>>>()?;
            scene_refs.push(ProductionSceneRef {
                scene_id: scene.id,
                scene_version: scene.target_version.clone(),
                scene_digest: scene.target_digest.trim().into(),
                sequence: u32::try_from(scene.sequence)
                    .map_err(|_| production_package_error("formal Scene sequence is invalid"))?,
                duration_sec: u32::try_from(scene.duration_sec)
                    .map_err(|_| production_package_error("formal Scene duration is invalid"))?,
                character_bible_ids,
            });
        }

        let contributors = sqlx::query_as::<_, ProductionPackageContributorStep>(
            r#"
            SELECT id, role_key, plan_order, attempt
            FROM production_steps
            WHERE run_id=$1 AND revision_epoch=$2 AND status='succeeded'
              AND attempt > 0
              AND role_key IN ('director', 'performance_director', 'sound_director')
            ORDER BY plan_order, id
            "#,
        )
        .bind(run_id)
        .bind(scope.current_revision_epoch)
        .fetch_all(&mut *tx)
        .await?;
        let contributor_by_role = contributors
            .iter()
            .map(|step| (step.role_key.as_str(), step))
            .collect::<BTreeMap<_, _>>();
        if contributors.len() != 3 || contributor_by_role.len() != 3 {
            return Err(production_package_error(
                "ProductionPackage requires exact succeeded Director, Performance and Sound steps",
            ));
        }
        let director = contributor_by_role["director"];
        let performance = contributor_by_role["performance_director"];
        let sound = contributor_by_role["sound_director"];
        let treatments = load_scoped_process_artifacts(
            &mut tx,
            "directorial_treatments",
            scope.production_project_id,
            run_id,
            scope.current_revision_epoch,
            director,
        )
        .await?;
        let shot_rows = load_scoped_process_artifacts(
            &mut tx,
            "shot_contracts",
            scope.production_project_id,
            run_id,
            scope.current_revision_epoch,
            director,
        )
        .await?;
        let performance_rows = load_scoped_process_artifacts(
            &mut tx,
            "performance_briefs",
            scope.production_project_id,
            run_id,
            scope.current_revision_epoch,
            performance,
        )
        .await?;
        let sound_rows = load_scoped_process_artifacts(
            &mut tx,
            "sound_plans",
            scope.production_project_id,
            run_id,
            scope.current_revision_epoch,
            sound,
        )
        .await?;
        if treatments.len() != 1
            || shot_rows.is_empty()
            || performance_rows.is_empty()
            || sound_rows.len() != 1
        {
            return Err(production_package_error(
                "ProductionPackage process artifact cardinality is incomplete",
            ));
        }

        let mut items = Vec::new();
        items.push(process_artifact_ref(
            run_id,
            "directorial_treatment",
            &treatments[0],
        )?);
        let mut shots = Vec::with_capacity(shot_rows.len());
        for row in &shot_rows {
            let output: ShotContractOutput =
                serde_json::from_value(row.content.clone()).map_err(|error| {
                    production_package_error(format!("ShotContract schema is invalid: {error}"))
                })?;
            let character_bible_ids = output
                .character_ids
                .iter()
                .map(|character_id| {
                    character_id_map.get(character_id).copied().ok_or_else(|| {
                        production_package_error(
                            "ShotContract references an unknown CharacterBible",
                        )
                    })
                })
                .collect::<ProductionResult<Vec<_>>>()?;
            shots.push(ProductionShotRef {
                artifact_id: row.id,
                shot_id: output.shot_id,
                sequence: output.sequence,
                scene_id: output.scene_id,
                duration_sec: output.duration_sec,
                character_bible_ids,
            });
            items.push(process_artifact_ref(run_id, "shot_contract", row)?);
        }
        shots.sort_by_key(|shot| (shot.sequence, shot.artifact_id));

        let mut performance_briefs = Vec::with_capacity(performance_rows.len());
        for row in &performance_rows {
            let output: PerformanceBriefOutput = serde_json::from_value(row.content.clone())
                .map_err(|error| {
                    production_package_error(format!("PerformanceBrief schema is invalid: {error}"))
                })?;
            performance_briefs.push(ProductionPerformanceRef {
                artifact_id: row.id,
                script_id: output.script_id,
                character_bible_id: output.character_bible_id,
                character_id: output.character_id,
                scene_ids: output
                    .emotional_arc
                    .into_iter()
                    .map(|scene| scene.scene_id)
                    .collect(),
            });
            items.push(process_artifact_ref(run_id, "performance_brief", row)?);
        }
        performance_briefs.sort_by_key(|brief| (brief.character_bible_id, brief.artifact_id));

        let sound_output: SoundPlanOutput = serde_json::from_value(sound_rows[0].content.clone())
            .map_err(|error| {
            production_package_error(format!("SoundPlan schema is invalid: {error}"))
        })?;
        let sound_plan = ProductionSoundRef {
            artifact_id: sound_rows[0].id,
            script_id: sound_output.script_id,
            scene_ids: sound_output
                .scene_sound_notes
                .into_iter()
                .map(|note| note.scene_id)
                .collect(),
        };
        items.push(process_artifact_ref(run_id, "sound_plan", &sound_rows[0])?);

        let source_step = contributors
            .iter()
            .max_by_key(|step| (step.plan_order, step.id))
            .ok_or_else(|| production_package_error("ProductionPackage has no source step"))?;
        let metadata = ProductionPackageMetadata {
            script_id: script.id,
            script_version: script.target_version.clone(),
            script_digest: script.target_digest.trim().into(),
            scenes: scene_refs,
            characters,
            shots,
            performance_briefs,
            sound_plan,
            suggestion_resolutions: vec![],
        };
        let package = ArtifactPackageSnapshot::build(
            PackageType::Production,
            run_id,
            source_step.id,
            u32::try_from(source_step.attempt)
                .map_err(|_| production_package_error("source attempt is invalid"))?,
            u32::try_from(scope.current_revision_epoch)
                .map_err(|_| production_package_error("revision epoch is invalid"))?,
            package_version,
            items,
            serde_json::to_value(metadata)?,
        )?;
        tx.commit().await?;
        Ok(package)
    }

    /// 从当前 epoch 已批准的精确 ProductionPackage 恢复 Application Port 输入。
    pub async fn load_approved_production_input(
        &self,
        run_id: Uuid,
        package_digest: &str,
    ) -> ProductionResult<ProductionPackageInput> {
        validate_digest(package_digest)?;
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_as::<_, ApprovedProductionPackageRow>(
            r#"
            SELECT package.id, package.source_step_id, package.source_attempt,
                   package.revision_epoch, package.package_version,
                   package.package_digest, package.metadata
            FROM artifact_package_snapshots package
            JOIN production_runs run ON run.id=package.run_id
            JOIN production_gate_decisions decision
              ON decision.package_id=package.id AND decision.run_id=package.run_id
             AND decision.package_digest=package.package_digest
             AND decision.decision='approved'
            WHERE package.run_id=$1 AND package.package_type='production'
              AND package.package_digest=$2
              AND package.revision_epoch=run.current_revision_epoch
              AND package.package_version=(
                  SELECT MAX(candidate.package_version)
                  FROM artifact_package_snapshots candidate
                  WHERE candidate.run_id=package.run_id
                    AND candidate.revision_epoch=package.revision_epoch
                    AND candidate.package_type='production'
              )
            FOR SHARE OF package, run, decision
            "#,
        )
        .bind(run_id)
        .bind(package_digest)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ProductionError::StalePackage)?;
        let item_rows = sqlx::query_as::<_, ApprovedProductionPackageItemRow>(
            r#"
            SELECT artifact_type, artifact_id, artifact_version, content_digest,
                   source_step_id, source_attempt
            FROM artifact_package_items WHERE package_id=$1 ORDER BY ordinal
            "#,
        )
        .bind(row.id)
        .fetch_all(&mut *tx)
        .await?;
        let items = item_rows
            .iter()
            .map(|item| {
                Ok(ArtifactRef {
                    run_id,
                    artifact_type: item.artifact_type.clone(),
                    artifact_id: item.artifact_id,
                    version: positive_u32(item.artifact_version, "package artifact version")?,
                    content_digest: item.content_digest.trim().into(),
                    source_step_id: item.source_step_id,
                    source_attempt: positive_u32(
                        item.source_attempt,
                        "package artifact source attempt",
                    )?,
                })
            })
            .collect::<ProductionResult<Vec<_>>>()?;
        let mut package = ArtifactPackageSnapshot::build(
            PackageType::Production,
            run_id,
            row.source_step_id,
            positive_u32(row.source_attempt, "package source attempt")?,
            u32::try_from(row.revision_epoch)
                .map_err(|_| production_package_error("package revision epoch is invalid"))?,
            positive_u32(row.package_version, "package version")?,
            items,
            row.metadata.clone(),
        )?;
        package.id = row.id;
        if package.package_digest != row.package_digest.trim() {
            return Err(production_package_error(
                "persisted ProductionPackage digest is not canonical",
            ));
        }
        let metadata: ProductionPackageMetadata = serde_json::from_value(row.metadata)?;
        let script = sqlx::query_as::<_, FormalScriptInputRow>(
            r#"
            SELECT script.title, script.hook
            FROM scripts script
            JOIN production_domain_links link
              ON link.run_id=$1 AND link.link_type='script' AND link.script_id=script.id
            WHERE script.id=$2 AND script.production_run_id=$1
              AND link.target_version=$3 AND link.target_digest=$4
            "#,
        )
        .bind(run_id)
        .bind(metadata.script_id)
        .bind(&metadata.script_version)
        .bind(&metadata.script_digest)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| production_package_error("formal Script package reference is stale"))?;
        let scene_ids = metadata
            .scenes
            .iter()
            .map(|scene| scene.scene_id)
            .collect::<Vec<_>>();
        let scene_rows = sqlx::query_as::<_, FormalSceneInputRow>(
            r#"
            SELECT id, sequence, narration, visual_description, emotion, duration_sec
            FROM scenes WHERE script_id=$1 AND id=ANY($2) ORDER BY sequence, id
            "#,
        )
        .bind(metadata.script_id)
        .bind(&scene_ids)
        .fetch_all(&mut *tx)
        .await?;
        if scene_rows.len() != metadata.scenes.len() {
            return Err(production_package_error(
                "formal Scene package references are incomplete",
            ));
        }
        let scenes = scene_rows
            .into_iter()
            .zip(&metadata.scenes)
            .map(|(scene, reference)| {
                if scene.id != reference.scene_id
                    || scene.sequence != i32::try_from(reference.sequence).unwrap_or(-1)
                    || scene.duration_sec != i32::try_from(reference.duration_sec).unwrap_or(-1)
                {
                    return Err(production_package_error(
                        "formal Scene order or duration drifted from ProductionPackage",
                    ));
                }
                Ok(FormalSceneInput {
                    scene_id: scene.id,
                    scene_version: reference.scene_version.clone(),
                    scene_digest: reference.scene_digest.clone(),
                    sequence: reference.sequence,
                    narration: scene.narration,
                    visual_description: scene.visual_description,
                    emotion: scene.emotion,
                    duration_sec: reference.duration_sec,
                    character_bible_ids: reference.character_bible_ids.clone(),
                })
            })
            .collect::<ProductionResult<Vec<_>>>()?;

        let treatment_item = only_package_item(&item_rows, "directorial_treatment")?;
        let sound_item = only_package_item(&item_rows, "sound_plan")?;
        let treatment =
            load_exact_process_artifact(&mut tx, "directorial_treatments", run_id, treatment_item)
                .await?;
        let sound = load_exact_process_artifact(&mut tx, "sound_plans", run_id, sound_item).await?;
        let mut shot_contracts = load_typed_process_artifacts::<ShotContractOutput>(
            &mut tx,
            "shot_contracts",
            run_id,
            package_items(&item_rows, "shot_contract"),
        )
        .await?;
        shot_contracts.sort_by_key(|item| (item.content.sequence, item.artifact_id));
        let mut performance_briefs = load_typed_process_artifacts::<PerformanceBriefOutput>(
            &mut tx,
            "performance_briefs",
            run_id,
            package_items(&item_rows, "performance_brief"),
        )
        .await?;
        performance_briefs.sort_by_key(|item| (item.content.character_bible_id, item.artifact_id));
        let content = ProductionPackageContent {
            script: FormalScriptInput {
                script_id: metadata.script_id,
                script_version: metadata.script_version,
                script_digest: metadata.script_digest,
                title: script.title,
                hook: script.hook,
            },
            scenes,
            directorial_treatment: versioned_process_artifact::<DirectorialTreatmentOutput>(
                treatment_item,
                treatment,
            )?,
            shot_contracts,
            performance_briefs,
            sound_plan: versioned_process_artifact::<SoundPlanOutput>(sound_item, sound)?,
            applied_suggestions: package
                .metadata
                .get("suggestion_resolutions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(serde_json::from_value)
                .collect::<Result<Vec<_>, _>>()?,
        };
        let input = ProductionPackageInput::from_approved_package(&package, content)?;
        tx.commit().await?;
        Ok(input)
    }

    /// 保存 SceneVisualManifest 未就绪事实，供重启后的恢复扫描继续观察。
    pub async fn mark_scene_visual_manifest_wait(
        &self,
        input: &ProductionPackageInput,
        details: &Value,
    ) -> ProductionResult<()> {
        if !details.is_object() {
            return Err(ProductionError::TransitionConflict {
                reason: "SceneVisualManifest wait details must be an object".into(),
            });
        }
        let mut tx = self.pool.begin().await?;
        let step_id = lock_scene_visual_wait_step(&mut tx, input, false).await?;
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status='external_wait', waiting_reason='scene_visual_manifest',
                input_package_id=$2, input_digest=$3, error_code='external_wait',
                error_details=$4, retryable=FALSE, updated_at=NOW()
            WHERE id=$1
            "#,
        )
        .bind(step_id)
        .bind(input.package_id)
        .bind(&input.package_digest)
        .bind(details)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE production_runs SET status='external_wait', error_code=NULL, error_details=NULL, updated_at=NOW() WHERE id=$1",
        )
        .bind(input.run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// 原子保存正式 manifest 引用、完成 external wait，并只解锁确定性后继。
    pub async fn complete_scene_visual_manifest_wait(
        &self,
        input: &ProductionPackageInput,
        manifest: &SceneVisualManifestReference,
    ) -> ProductionResult<()> {
        manifest.validate_for(input)?;
        let mut tx = self.pool.begin().await?;
        let step_id = lock_scene_visual_wait_step(&mut tx, input, true).await?;
        let revision_epoch = i32::try_from(input.revision_epoch)
            .map_err(|_| production_package_error("revision epoch is invalid"))?;
        let existing_output = sqlx::query_scalar::<_, Option<String>>(
            "SELECT output_digest FROM production_steps WHERE id=$1",
        )
        .bind(step_id)
        .fetch_one(&mut *tx)
        .await?;
        if let Some(existing_output) = existing_output {
            if existing_output.trim() == manifest.manifest_digest {
                tx.commit().await?;
                return Ok(());
            }
            return Err(ProductionError::TransitionConflict {
                reason: "SceneVisualManifest wait already completed with another digest".into(),
            });
        }
        let manifest_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO production_scene_visual_manifests (
                id, run_id, revision_epoch, package_id, package_digest,
                script_id, script_version, manifest_version, manifest_digest
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            "#,
        )
        .bind(manifest_id)
        .bind(input.run_id)
        .bind(revision_epoch)
        .bind(input.package_id)
        .bind(&input.package_digest)
        .bind(manifest.script_id)
        .bind(&manifest.script_version)
        .bind(&manifest.manifest_version)
        .bind(&manifest.manifest_digest)
        .execute(&mut *tx)
        .await?;
        for (ordinal, scene) in manifest.scenes.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO production_scene_visual_manifest_items (
                    manifest_id, ordinal, scene_id, scene_version, candidate_id, material_id
                ) VALUES ($1,$2,$3,$4,$5,$6)
                "#,
            )
            .bind(manifest_id)
            .bind(ordinal as i32)
            .bind(scene.scene_id)
            .bind(&scene.scene_version)
            .bind(scene.candidate_id)
            .bind(scene.material_id)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status='succeeded', waiting_reason=NULL, input_package_id=$2,
                input_digest=$3, output_digest=$4, error_code=NULL, error_details=NULL,
                completed_at=NOW(), updated_at=NOW()
            WHERE id=$1
            "#,
        )
        .bind(step_id)
        .bind(input.package_id)
        .bind(&input.package_digest)
        .bind(&manifest.manifest_digest)
        .execute(&mut *tx)
        .await?;
        unlock_ready_steps(&mut tx, input.run_id, revision_epoch).await?;
        sqlx::query(
            "UPDATE production_runs SET status='queued', error_code=NULL, error_details=NULL, updated_at=NOW() WHERE id=$1",
        )
        .bind(input.run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// 从 PostgreSQL 恢复当前 revision 的正式 SceneVisualManifest。
    pub async fn load_scene_visual_manifest(
        &self,
        run_id: Uuid,
    ) -> ProductionResult<SceneVisualManifestReference> {
        let mut tx = self.pool.begin().await?;
        let manifest = sqlx::query_as::<_, (Uuid, String, String, String)>(
            r#"
            SELECT script_id, script_version, manifest_version, manifest_digest
            FROM production_scene_visual_manifests manifest
            JOIN production_runs run ON run.id=manifest.run_id
            WHERE manifest.run_id=$1 AND manifest.revision_epoch=run.current_revision_epoch
            ORDER BY manifest.created_at DESC, manifest.id DESC
            LIMIT 1
            FOR SHARE
            "#,
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "current SceneVisualManifest is not persisted".into(),
        })?;
        let scenes = sqlx::query_as::<_, (Uuid, String, Uuid, Uuid)>(
            r#"
            SELECT item.scene_id, item.scene_version, item.candidate_id, item.material_id
            FROM production_scene_visual_manifest_items item
            JOIN production_scene_visual_manifests manifest ON manifest.id=item.manifest_id
            JOIN production_runs run ON run.id=manifest.run_id
            WHERE manifest.run_id=$1 AND manifest.revision_epoch=run.current_revision_epoch
              AND manifest.script_id=$2 AND manifest.manifest_digest=$3
            ORDER BY item.ordinal
            "#,
        )
        .bind(run_id)
        .bind(manifest.0)
        .bind(&manifest.3)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|(scene_id, scene_version, candidate_id, material_id)| {
            crate::orchestrator::application_port::SceneVisualReference {
                scene_id,
                scene_version,
                candidate_id,
                material_id,
            }
        })
        .collect::<Vec<_>>();
        tx.commit().await?;
        let reference = SceneVisualManifestReference {
            script_id: manifest.0,
            script_version: manifest.1,
            manifest_version: manifest.2,
            scenes,
            manifest_digest: manifest.3,
        };
        let rebuilt = SceneVisualManifestReference::build(
            reference.script_id,
            reference.script_version.clone(),
            reference.manifest_version.clone(),
            reference.scenes.clone(),
        )?;
        if rebuilt.manifest_digest != reference.manifest_digest {
            return Err(ProductionError::TransitionConflict {
                reason: "persisted SceneVisualManifest digest is not canonical".into(),
            });
        }
        Ok(reference)
    }

    /// 原子保存既有 Work/WorkVersion/WorkPlan 正式引用并完成规划步骤。
    pub async fn complete_work_plan_creation(
        &self,
        request: &ProductionWorkPlanRequest,
        plan: &WorkPlanReference,
    ) -> ProductionResult<()> {
        request.validate()?;
        plan.validate()?;
        let input = &request.production;
        let revision_epoch = i32::try_from(input.revision_epoch)
            .map_err(|_| production_package_error("revision epoch is invalid"))?;
        let mut tx = self.pool.begin().await?;
        let step = sqlx::query_as::<_, (Uuid, String, Option<String>, i32, Option<String>)>(
            r#"
            SELECT id,status,output_digest,attempt,lease_owner FROM production_steps
            WHERE run_id=$1 AND revision_epoch=$2 AND step_key='create_work_plan'
            FOR UPDATE
            "#,
        )
        .bind(input.run_id)
        .bind(revision_epoch)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "create_work_plan step not found for current revision".into(),
        })?;
        if step.1 == "succeeded" {
            if step.2.as_deref() == Some(plan.plan_digest.as_str()) {
                tx.commit().await?;
                return Ok(());
            }
            return Err(ProductionError::TransitionConflict {
                reason: "create_work_plan already completed with another formal plan".into(),
            });
        }
        if !matches!(step.1.as_str(), "queued" | "running") {
            return Err(ProductionError::TransitionConflict {
                reason: "create_work_plan is not executable".into(),
            });
        }
        let persisted_manifest = sqlx::query_scalar::<_, String>(
            r#"
            SELECT manifest_digest FROM production_scene_visual_manifests
            WHERE run_id=$1 AND revision_epoch=$2 AND package_id=$3
              AND package_digest=$4 AND script_id=$5
            ORDER BY created_at DESC LIMIT 1
            "#,
        )
        .bind(input.run_id)
        .bind(revision_epoch)
        .bind(input.package_id)
        .bind(&input.package_digest)
        .bind(input.script.script_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "formal SceneVisualManifest has not completed its external wait".into(),
        })?;
        if persisted_manifest != request.manifest.manifest_digest {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkPlan does not consume the persisted current manifest".into(),
            });
        }
        let persisted = sqlx::query_as::<_, (Uuid, i32, Value, i32, String, String)>(
            r#"
            SELECT work.script_id,version.version_no,version.input_snapshot,
                   plan.plan_version,plan.input_fingerprint,plan.status
            FROM work_plans plan
            JOIN works work ON work.id=plan.work_id
            JOIN work_versions version ON version.id=plan.work_version_id
            WHERE plan.id=$1 AND plan.work_id=$2 AND plan.work_version_id=$3
            FOR SHARE OF plan,work,version
            "#,
        )
        .bind(plan.work_plan_id)
        .bind(plan.work_id)
        .bind(plan.work_version_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "formal WorkPlan reference does not exist".into(),
        })?;
        let snapshot_run = persisted
            .2
            .pointer("/production_crew/production_run_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        let snapshot_package = persisted
            .2
            .pointer("/production_crew/production_package/digest")
            .and_then(Value::as_str);
        let snapshot_manifest = persisted
            .2
            .pointer("/production_crew/scene_visual_manifest/manifest_digest")
            .and_then(Value::as_str);
        if persisted.0 != input.script.script_id
            || persisted.1 != plan.work_version as i32
            || persisted.3 != plan.plan_version as i32
            || persisted.4 != plan.input_fingerprint
            || persisted.5 != "ready"
            || snapshot_run != Some(input.run_id)
            || snapshot_package != Some(input.package_digest.as_str())
            || snapshot_manifest != Some(request.manifest.manifest_digest.as_str())
        {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkPlan formal reference or Full Crew source snapshot is stale".into(),
            });
        }
        let work_digest = canonical_digest(&json!({
            "work_id": plan.work_id,
            "script_id": input.script.script_id,
            "production_run_id": input.run_id,
        }))?;
        for (link_type, target_id, target_version, target_digest) in [
            ("work", plan.work_id, "1".to_string(), work_digest),
            (
                "work_version",
                plan.work_version_id,
                plan.work_version.to_string(),
                plan.work_version_digest.clone(),
            ),
            (
                "work_plan",
                plan.work_plan_id,
                plan.plan_version.to_string(),
                plan.plan_digest.clone(),
            ),
        ] {
            let (work_id, work_version_id, work_plan_id) = match link_type {
                "work" => (Some(target_id), None, None),
                "work_version" => (None, Some(target_id), None),
                "work_plan" => (None, None, Some(target_id)),
                _ => unreachable!(),
            };
            sqlx::query(
                r#"
                INSERT INTO production_domain_links (
                    run_id,source_step_id,revision_epoch,link_type,
                    work_id,work_version_id,work_plan_id,target_version,target_digest
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                ON CONFLICT (run_id,link_type,target_version,target_digest) DO NOTHING
                "#,
            )
            .bind(input.run_id)
            .bind(step.0)
            .bind(revision_epoch)
            .bind(link_type)
            .bind(work_id)
            .bind(work_version_id)
            .bind(work_plan_id)
            .bind(target_version)
            .bind(target_digest)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status='succeeded',input_package_id=$2,input_digest=$3,output_digest=$4,
                waiting_reason=NULL,error_code=NULL,error_details=NULL,
                lease_owner=NULL,lease_expires_at=NULL,side_effect_state='confirmed',
                completed_at=NOW(),updated_at=NOW()
            WHERE id=$1
            "#,
        )
        .bind(step.0)
        .bind(input.package_id)
        .bind(&input.package_digest)
        .bind(&plan.plan_digest)
        .execute(&mut *tx)
        .await?;
        if step.3 > 0 && step.4.is_some() {
            sqlx::query(
                r#"
                UPDATE production_step_attempts
                SET status='succeeded',side_effect_state='confirmed',
                    result=$3,completed_at=NOW()
                WHERE step_id=$1 AND attempt_no=$2
                  AND status IN ('running','prepared')
                "#,
            )
            .bind(step.0)
            .bind(step.3)
            .bind(json!({
                "plan_id": plan.work_plan_id,
                "plan_digest": plan.plan_digest,
            }))
            .execute(&mut *tx)
            .await?;
        }
        unlock_ready_steps(&mut tx, input.run_id, revision_epoch).await?;
        sqlx::query(
            "UPDATE production_runs SET status='queued',error_code=NULL,error_details=NULL,updated_at=NOW() WHERE id=$1",
        )
        .bind(input.run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// 关联既有人工确认结果，并将作品运行观察步骤置为 external wait。
    pub async fn record_work_generation_confirmation(
        &self,
        run_id: Uuid,
        plan: &WorkPlanReference,
        external: &WorkGenerationRunReference,
    ) -> ProductionResult<()> {
        plan.validate()?;
        external.validate_for(plan)?;
        let mut tx = self.pool.begin().await?;
        let current_epoch = sqlx::query_scalar::<_, i32>(
            "SELECT current_revision_epoch FROM production_runs WHERE id=$1 FOR UPDATE",
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production run not found".into(),
        })?;
        let step = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
            r#"
            SELECT id,status,output_digest FROM production_steps
            WHERE run_id=$1 AND revision_epoch=$2 AND step_key='work_plan_confirmation'
            FOR UPDATE
            "#,
        )
        .bind(run_id)
        .bind(current_epoch)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "work_plan_confirmation step not found".into(),
        })?;
        if step.1 == "succeeded" {
            if step.2.as_deref() == Some(external.run_digest.as_str()) {
                tx.commit().await?;
                return Ok(());
            }
            return Err(ProductionError::TransitionConflict {
                reason: "work plan confirmation already linked another run".into(),
            });
        }
        if !matches!(step.1.as_str(), "queued" | "external_wait") {
            return Err(ProductionError::TransitionConflict {
                reason: "work plan confirmation is not waiting for operator confirmation".into(),
            });
        }
        let formal_plan_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM production_domain_links
                WHERE run_id=$1 AND link_type='work_plan' AND work_plan_id=$2
                  AND target_digest=$3
            )
            "#,
        )
        .bind(run_id)
        .bind(plan.work_plan_id)
        .bind(&plan.plan_digest)
        .fetch_one(&mut *tx)
        .await?;
        if !formal_plan_exists {
            return Err(ProductionError::TransitionConflict {
                reason: "confirmed WorkPlan is not linked to this ProductionRun".into(),
            });
        }
        sqlx::query(
            r#"
            INSERT INTO production_domain_links (
                run_id,source_step_id,revision_epoch,link_type,work_generation_run_id,
                target_version,target_digest
            ) VALUES ($1,$2,$3,'work_generation_run',$4,'1',$5)
            ON CONFLICT (run_id,link_type,target_version,target_digest) DO NOTHING
            "#,
        )
        .bind(run_id)
        .bind(step.0)
        .bind(current_epoch)
        .bind(external.run_id)
        .bind(&external.run_digest)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_steps SET status='succeeded',input_digest=$2,output_digest=$3,
                waiting_reason=NULL,error_code=NULL,error_details=NULL,
                completed_at=NOW(),updated_at=NOW() WHERE id=$1
            "#,
        )
        .bind(step.0)
        .bind(&plan.plan_digest)
        .bind(&external.run_digest)
        .execute(&mut *tx)
        .await?;
        unlock_ready_steps(&mut tx, run_id, current_epoch).await?;
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status='external_wait',waiting_reason='work_generation',
                attempt=GREATEST(attempt,1),input_digest=$3,
                error_code='external_wait',retryable=FALSE,updated_at=NOW()
            WHERE run_id=$1 AND revision_epoch=$2 AND step_key='wait_work_generation'
              AND status='queued'
            "#,
        )
        .bind(run_id)
        .bind(current_epoch)
        .bind(&external.run_digest)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE production_runs SET status='external_wait',error_code=NULL,error_details=NULL,updated_at=NOW() WHERE id=$1",
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn settle_work_generation_resources(
        tx: &mut Transaction<'_, Postgres>,
        production_run_id: Uuid,
        external_run_id: Uuid,
        disposition: WorkGenerationRunDisposition,
    ) -> ProductionResult<()> {
        let run_usage = sqlx::query_as::<_, (Uuid, Value)>(
            "SELECT work_plan_id,resource_usage FROM work_generation_runs WHERE id=$1 FOR SHARE",
        )
        .bind(external_run_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((work_plan_id, usage)) = run_usage else {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkGenerationRun resource snapshot not found".into(),
            });
        };
        let initial_request = work_generation_resource_request(&usage);
        let initial_digest = canonical_digest(&json!({
            "kind": "work_generation_initial",
            "work_generation_run_id": external_run_id,
            "work_plan_id": work_plan_id,
            "resource_request": initial_request,
        }))?;
        let has_initial = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM production_resource_reservations WHERE run_id=$1 AND request_digest=$2)",
        )
        .bind(production_run_id)
        .bind(&initial_digest)
        .fetch_one(&mut **tx)
        .await?;
        if !has_initial {
            return Ok(());
        }

        let attempt_rows = sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                Value,
                Option<Uuid>,
                Option<i32>,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            SELECT step.id,step.step_type,step.resource_usage,attempt.id,attempt.attempt_no,
                   attempt.status,attempt.upstream_task_id
            FROM work_generation_steps step
            LEFT JOIN work_generation_attempts attempt ON attempt.step_id=step.id
            WHERE step.run_id=$1
            ORDER BY step.step_no,attempt.attempt_no
            "#,
        )
        .bind(external_run_id)
        .fetch_all(&mut **tx)
        .await?;
        let uncertain = disposition == WorkGenerationRunDisposition::AttentionRequired;
        let terminal = matches!(
            disposition,
            WorkGenerationRunDisposition::FailedBlocker
                | WorkGenerationRunDisposition::ExternalCancelConflict
                | WorkGenerationRunDisposition::EvidenceBlocker
                | WorkGenerationRunDisposition::ReadyForMediaReview
                | WorkGenerationRunDisposition::Cancelled
        );
        if !uncertain && !terminal {
            return Ok(());
        }

        let mut initial_actual = BTreeMap::from([
            ("video_tasks".into(), 0),
            ("video_duration_sec".into(), 0),
            ("tts_characters".into(), 0),
            ("asr_tasks".into(), 0),
            ("concurrency".into(), 0),
        ]);
        let mut initial_started = false;
        for (step_id, step_type, usage, attempt_id, attempt_no, status, upstream_task_id) in
            &attempt_rows
        {
            let Some(attempt_id) = attempt_id else {
                continue;
            };
            let started = attempt_started(status.as_deref(), upstream_task_id.as_deref());
            if started && is_provider_step(step_type) {
                initial_started = true;
            }
            let attempt_no = attempt_no.unwrap_or(0);
            if attempt_no == 1 {
                add_attempt_usage(&mut initial_actual, step_type, usage, started);
            } else if attempt_no > 1 {
                let Some(request) = provider_retry_resource_request(step_type, usage) else {
                    continue;
                };
                let digest = canonical_digest(&json!({
                    "kind": "work_generation_provider_retry",
                    "work_generation_attempt_id": attempt_id,
                    "work_generation_step_id": step_id,
                    "resource_request": request,
                }))?;
                let exists = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM production_resource_reservations WHERE run_id=$1 AND request_digest=$2)",
                )
                .bind(production_run_id)
                .bind(&digest)
                .fetch_one(&mut **tx)
                .await?;
                if exists {
                    let actual = provider_retry_actual(step_type, usage, started);
                    Self::settle_integrated_resources(
                        tx,
                        production_run_id,
                        &digest,
                        actual,
                        uncertain,
                    )
                    .await?;
                }
            }
        }
        initial_actual.insert("concurrency".into(), u64::from(initial_started));
        Self::settle_integrated_resources(
            tx,
            production_run_id,
            &initial_digest,
            initial_actual,
            uncertain,
        )
        .await
    }

    /// 将既有作品运行的真实状态映射到 Full Crew，不发起 retry 或 provider 调用。
    pub async fn sync_work_generation_state(
        &self,
        run_id: Uuid,
        external: &WorkGenerationRunReference,
    ) -> ProductionResult<WorkGenerationRunDisposition> {
        let mut tx = self.pool.begin().await?;
        let run = sqlx::query_as::<_, (i32, Option<Value>)>(
            "SELECT current_revision_epoch,cancellation_intent FROM production_runs WHERE id=$1 FOR UPDATE",
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production run not found".into(),
        })?;
        let linked = sqlx::query_as::<_, (Uuid, i32)>(
            r#"
            SELECT source_step_id,revision_epoch
            FROM production_domain_links
            WHERE run_id=$1 AND link_type='work_generation_run'
              AND work_generation_run_id=$2
            ORDER BY created_at DESC,id DESC LIMIT 1
            "#,
        )
        .bind(run_id)
        .bind(external.run_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(_linked) = linked else {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkGenerationRun is not linked to this ProductionRun".into(),
            });
        };
        let step_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM production_steps
            WHERE run_id=$1 AND revision_epoch=$2 AND step_key='wait_work_generation'
            FOR UPDATE
            "#,
        )
        .bind(run_id)
        .bind(run.0)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "wait_work_generation step not found".into(),
        })?;
        let cancellation_requested = run
            .1
            .as_ref()
            .and_then(Value::as_object)
            .is_some_and(|intent| !intent.is_empty());
        let disposition = external.disposition(cancellation_requested);
        let details = serde_json::to_value(external)?;
        if disposition == WorkGenerationRunDisposition::Cancelled {
            tx.rollback().await?;
            self.reconcile_external_cancellation(
                run_id,
                external.run_id,
                ExternalCancellationState::Cancelled,
                None,
            )
            .await?;
            let mut tx = self.pool.begin().await?;
            sqlx::query(
                r#"
                UPDATE production_steps SET status='cancelled',waiting_reason=NULL,
                    input_digest=$2,output_digest=$2,error_code=NULL,error_details=$3,
                    retryable=FALSE,completed_at=COALESCE(completed_at,NOW()),updated_at=NOW()
                WHERE id=$1
                "#,
            )
            .bind(step_id)
            .bind(&external.run_digest)
            .bind(&details)
            .execute(&mut *tx)
            .await?;
            Self::settle_work_generation_resources(&mut tx, run_id, external.run_id, disposition)
                .await?;
            tx.commit().await?;
            return Ok(disposition);
        }
        if matches!(
            disposition,
            WorkGenerationRunDisposition::FailedBlocker
                | WorkGenerationRunDisposition::AttentionRequired
                | WorkGenerationRunDisposition::ExternalCancelConflict
                | WorkGenerationRunDisposition::EvidenceBlocker
                | WorkGenerationRunDisposition::ReadyForMediaReview
        ) {
            Self::settle_work_generation_resources(&mut tx, run_id, external.run_id, disposition)
                .await?;
        }
        match disposition {
            WorkGenerationRunDisposition::ExternalWait => {
                sqlx::query(
                    r#"
                    UPDATE production_steps SET status='external_wait',
                        waiting_reason='work_generation',input_digest=$2,
                        error_code='external_wait',error_details=$3,retryable=FALSE,updated_at=NOW()
                    WHERE id=$1
                    "#,
                )
                .bind(step_id)
                .bind(&external.run_digest)
                .bind(&details)
                .execute(&mut *tx)
                .await?;
                set_production_run_state(&mut tx, run_id, "external_wait", None, None).await?;
            }
            WorkGenerationRunDisposition::FailedBlocker => {
                sqlx::query(
                    r#"
                    UPDATE production_steps SET status='blocked',waiting_reason=NULL,
                        error_code='work_generation_failed',error_details=$2,retryable=$3,
                        updated_at=NOW() WHERE id=$1
                    "#,
                )
                .bind(step_id)
                .bind(&details)
                .bind(external.retryable)
                .execute(&mut *tx)
                .await?;
                set_production_run_state(
                    &mut tx,
                    run_id,
                    "blocked",
                    Some("work_generation_failed"),
                    Some(&details),
                )
                .await?;
            }
            WorkGenerationRunDisposition::AttentionRequired => {
                let error_code = external
                    .error_code
                    .as_deref()
                    .unwrap_or("work_generation_waiting_manual");
                sqlx::query(
                    r#"
                    UPDATE production_steps SET status='attention_required',waiting_reason=NULL,
                        error_code=$2,error_details=$3,retryable=FALSE,updated_at=NOW()
                    WHERE id=$1
                    "#,
                )
                .bind(step_id)
                .bind(error_code)
                .bind(&details)
                .execute(&mut *tx)
                .await?;
                set_production_run_state(
                    &mut tx,
                    run_id,
                    "attention_required",
                    Some(error_code),
                    Some(&details),
                )
                .await?;
            }
            WorkGenerationRunDisposition::Cancelling => {
                sqlx::query(
                    r#"
                    UPDATE production_steps SET status='cancelling',waiting_reason=NULL,
                        error_code='work_generation_cancelling',error_details=$2,
                        retryable=FALSE,updated_at=NOW() WHERE id=$1
                    "#,
                )
                .bind(step_id)
                .bind(&details)
                .execute(&mut *tx)
                .await?;
                set_production_run_state(
                    &mut tx,
                    run_id,
                    "cancelling",
                    Some("work_generation_cancelling"),
                    Some(&details),
                )
                .await?;
            }
            WorkGenerationRunDisposition::ExternalCancelConflict => {
                sqlx::query(
                    r#"
                    UPDATE production_steps SET status='blocked',waiting_reason=NULL,
                        error_code='external_cancel_conflict',error_details=$2,
                        retryable=FALSE,updated_at=NOW() WHERE id=$1
                    "#,
                )
                .bind(step_id)
                .bind(&details)
                .execute(&mut *tx)
                .await?;
                set_production_run_state(
                    &mut tx,
                    run_id,
                    "blocked",
                    Some("external_cancel_conflict"),
                    Some(&details),
                )
                .await?;
            }
            WorkGenerationRunDisposition::EvidenceBlocker => {
                sqlx::query(
                    r#"
                    UPDATE production_steps SET status='external_wait',
                        waiting_reason='work_generation_evidence',input_digest=$2,
                        error_code='evidence_blocker',error_details=$3,
                        retryable=FALSE,updated_at=NOW() WHERE id=$1
                    "#,
                )
                .bind(step_id)
                .bind(&external.run_digest)
                .bind(&details)
                .execute(&mut *tx)
                .await?;
                set_production_run_state(
                    &mut tx,
                    run_id,
                    "external_wait",
                    Some("evidence_blocker"),
                    Some(&details),
                )
                .await?;
            }
            WorkGenerationRunDisposition::ReadyForMediaReview => {
                sqlx::query(
                    r#"
                    UPDATE production_steps SET status='succeeded',waiting_reason=NULL,
                        input_digest=$2,output_digest=$2,error_code=NULL,error_details=NULL,
                        retryable=FALSE,completed_at=NOW(),updated_at=NOW() WHERE id=$1
                    "#,
                )
                .bind(step_id)
                .bind(&external.run_digest)
                .execute(&mut *tx)
                .await?;
                unlock_ready_steps(&mut tx, run_id, run.0).await?;
                sqlx::query(
                    "UPDATE production_runs SET quality_status='reviewing',updated_at=NOW() WHERE id=$1",
                )
                .bind(run_id)
                .execute(&mut *tx)
                .await?;
                set_production_run_state(&mut tx, run_id, "queued", None, None).await?;
            }
            WorkGenerationRunDisposition::Cancelled => unreachable!("handled before match"),
        }
        tx.commit().await?;
        Ok(disposition)
    }

    /// 在调用 Work Library 前校验当前媒体 scope 和固定返工上限。
    pub async fn ensure_quality_rework_allowed(
        &self,
        request: &WorkVersionReworkRequest,
    ) -> ProductionResult<()> {
        request.validate()?;
        let mut tx = self.pool.begin().await?;
        let (plan, rework_count, production_project_id) =
            validate_quality_rework_scope(&mut tx, request).await?;
        let limit = u64::from(plan.max_quality_reworks);
        if rework_count >= limit {
            let details = json!({
                "resource": "quality_reworks",
                "current": rework_count,
                "limit": limit,
                "allowed_commands": ["cancel", "new_production_intent_after_termination"],
            });
            sqlx::query(
                r#"
                UPDATE production_runs
                SET status='attention_required',error_code='quality_rework_limit_reached',
                    error_details=$2,updated_at=NOW()
                WHERE id=$1
                "#,
            )
            .bind(request.production_run_id)
            .bind(&details)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE production_projects SET status='attention_required',updated_at=NOW() WHERE id=$1",
            )
            .bind(production_project_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Err(ProductionError::ResourceLimit {
                resource: "quality_reworks".into(),
                current: rework_count,
                requested: 1,
                limit,
            });
        }
        tx.commit().await?;
        Ok(())
    }

    /// 在调用 Work Library 前预占一次质量返工额度；同请求重放返回原预占。
    pub async fn reserve_quality_rework(
        &self,
        request: &WorkVersionReworkRequest,
    ) -> ProductionResult<()> {
        request.validate()?;
        let request_digest = request.digest()?;
        let mut tx = self.pool.begin().await?;
        let step = sqlx::query_as::<_, (Uuid, i32)>(
            r#"
            SELECT step.id,GREATEST(step.attempt,1)
            FROM production_runs run
            JOIN production_steps step
              ON step.run_id=run.id AND step.revision_epoch=run.current_revision_epoch
             AND step.step_key='quality_gate'
            WHERE run.id=$1 AND run.current_revision_epoch=$2
            FOR UPDATE OF run,step
            "#,
        )
        .bind(request.production_run_id)
        .bind(i32::try_from(request.revision_epoch).map_err(|_| {
            ProductionError::TransitionConflict {
                reason: "quality rework revision exceeds PostgreSQL range".into(),
            }
        })?)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "quality rework has no current QualityGate step".into(),
        })?;
        Self::reserve_integrated_resources(
            &mut tx,
            request.production_run_id,
            step.0,
            step.1,
            ResourceRequest::quality_rework(),
            &request_digest,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Work Library 明确拒绝且未产生副作用时释放质量返工预占。
    pub async fn release_quality_rework(
        &self,
        request: &WorkVersionReworkRequest,
    ) -> ProductionResult<()> {
        request.validate()?;
        let mut tx = self.pool.begin().await?;
        Self::release_integrated_resources(&mut tx, request.production_run_id, &request.digest()?)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// 在调用 Work Library 前返回已持久化的质量返工结果，避免重放外部领域命令。
    pub async fn quality_rework_replay(
        &self,
        request: &WorkVersionReworkRequest,
    ) -> ProductionResult<Option<WorkVersionReworkReference>> {
        request.validate()?;
        let request_digest = request.digest()?;
        let command_scope = ProductionCommandScope::new(
            request.actor.clone(),
            ProductionCommandType::QualityRework,
            ProductionAggregateType::ProductionRun,
            request.production_run_id,
            &request.idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        let replay: Option<WorkVersionReworkReference> =
            ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest)
                .await?
                .map(serde_json::from_value)
                .transpose()?;
        if let Some(reference) = &replay {
            reference.validate_for(request)?;
        }
        tx.commit().await?;
        Ok(replay)
    }

    /// 记录既有 Work Library 返回的草稿/差异计划，并开启新的质量 revision epoch。
    pub async fn record_quality_rework(
        &self,
        request: &WorkVersionReworkRequest,
        reference: &WorkVersionReworkReference,
    ) -> ProductionResult<WorkVersionReworkReference> {
        request.validate()?;
        reference.validate_for(request)?;
        let request_digest = request.digest()?;
        let command_scope = ProductionCommandScope::new(
            request.actor.clone(),
            ProductionCommandType::QualityRework,
            ProductionAggregateType::ProductionRun,
            request.production_run_id,
            &request.idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        if let Some(result) =
            ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest).await?
        {
            let stored: WorkVersionReworkReference = serde_json::from_value(result)?;
            stored.validate_for(request)?;
            tx.commit().await?;
            return Ok(stored);
        }
        let (plan, rework_count, production_project_id) =
            validate_quality_rework_scope(&mut tx, request).await?;
        if rework_count >= u64::from(plan.max_quality_reworks) {
            return Err(ProductionError::ResourceLimit {
                resource: "quality_reworks".into(),
                current: rework_count,
                requested: 1,
                limit: u64::from(plan.max_quality_reworks),
            });
        }
        let current_epoch = i32::try_from(request.revision_epoch).map_err(|_| {
            ProductionError::TransitionConflict {
                reason: "quality rework revision exceeds PostgreSQL range".into(),
            }
        })?;
        let next_epoch = current_epoch + 1;
        let old_steps = sqlx::query_as::<_, ReworkStepRow>(
            r#"
            SELECT plan_order,step_key,step_type,role_key,dependencies,attempt,
                   input_package_id,input_digest,output_digest
            FROM production_steps
            WHERE run_id=$1 AND revision_epoch=$2
            ORDER BY plan_order
            "#,
        )
        .bind(request.production_run_id)
        .bind(current_epoch)
        .fetch_all(&mut *tx)
        .await?;
        let confirmation_order = old_steps
            .iter()
            .find(|step| step.step_key == "work_plan_confirmation")
            .map(|step| step.plan_order)
            .ok_or_else(|| ProductionError::TransitionConflict {
                reason: "fixed plan has no work_plan_confirmation step".into(),
            })?;
        sqlx::query(
            r#"
            INSERT INTO production_revision_epochs (
                run_id,epoch,reason_type,reason,affected_owners,
                actor_type,actor_id,instruction_digest
            ) VALUES ($1,$2,'quality_rework',$3,$4,'local_operator','local_operator',$5)
            "#,
        )
        .bind(request.production_run_id)
        .bind(next_epoch)
        .bind(request.reason.trim())
        .bind(json!(["work_generation", "editor", "qc"]))
        .bind(request.digest()?)
        .execute(&mut *tx)
        .await?;

        let mut rework_source_step_id = None;
        for step in old_steps {
            let (status, waiting_reason) = if step.plan_order < confirmation_order {
                ("succeeded", None)
            } else if step.step_key == "work_plan_confirmation" {
                ("external_wait", Some("quality_rework_confirmation"))
            } else {
                ("blocked", None)
            };
            let dependencies = if step.step_key == "work_plan_confirmation" {
                json!([])
            } else {
                step.dependencies
            };
            let output_digest = if step.step_key == "create_work_plan" {
                Some(reference.reference_digest.clone())
            } else if step.plan_order < confirmation_order {
                step.output_digest
            } else {
                None
            };
            let input_digest = if step.step_key == "work_plan_confirmation" {
                Some(reference.reference_digest.clone())
            } else if step.plan_order < confirmation_order {
                step.input_digest
            } else {
                None
            };
            let inserted_id = sqlx::query_scalar::<_, Uuid>(
                r#"
                INSERT INTO production_steps (
                    run_id,revision_epoch,plan_order,step_key,step_type,role_key,
                    dependencies,status,waiting_reason,attempt,input_package_id,
                    input_digest,output_digest,completed_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,
                          CASE WHEN $8='succeeded' THEN NOW() ELSE NULL END)
                RETURNING id
                "#,
            )
            .bind(request.production_run_id)
            .bind(next_epoch)
            .bind(step.plan_order)
            .bind(&step.step_key)
            .bind(&step.step_type)
            .bind(&step.role_key)
            .bind(dependencies)
            .bind(status)
            .bind(waiting_reason)
            .bind(if status == "succeeded" {
                step.attempt
            } else {
                0
            })
            .bind(if status == "succeeded" {
                step.input_package_id
            } else {
                None
            })
            .bind(input_digest)
            .bind(output_digest)
            .fetch_one(&mut *tx)
            .await?;
            if step.step_key == "create_work_plan" {
                rework_source_step_id = Some(inserted_id);
            }
        }
        let source_step_id =
            rework_source_step_id.ok_or_else(|| ProductionError::TransitionConflict {
                reason: "fixed plan has no create_work_plan step".into(),
            })?;
        for (link_type, work_version_id, work_plan_id, target_version) in [
            (
                "work_version",
                Some(reference.draft_work_version_id),
                None,
                reference.draft_version.to_string(),
            ),
            (
                "work_plan",
                None,
                Some(reference.work_plan_id),
                reference.work_plan_version.to_string(),
            ),
        ] {
            sqlx::query(
                r#"
                INSERT INTO production_domain_links (
                    run_id,source_step_id,revision_epoch,link_type,
                    work_version_id,work_plan_id,target_version,target_digest
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
                "#,
            )
            .bind(request.production_run_id)
            .bind(source_step_id)
            .bind(next_epoch)
            .bind(link_type)
            .bind(work_version_id)
            .bind(work_plan_id)
            .bind(target_version)
            .bind(&reference.reference_digest)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            r#"
            UPDATE production_runs
            SET current_revision_epoch=$2,status='external_wait',quality_status='not_started',
                error_code=NULL,error_details=NULL,updated_at=NOW()
            WHERE id=$1
            "#,
        )
        .bind(request.production_run_id)
        .bind(next_epoch)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE production_projects SET status='external_wait',updated_at=NOW() WHERE id=$1",
        )
        .bind(production_project_id)
        .execute(&mut *tx)
        .await?;
        Self::settle_integrated_resources(
            &mut tx,
            request.production_run_id,
            &request_digest,
            BTreeMap::from([("quality_reworks".into(), 1)]),
            false,
        )
        .await?;
        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            serde_json::to_value(reference)?,
        )
        .await?;
        tx.commit().await?;
        Ok(reference.clone())
    }

    /// 从当前正式作品运行、WorkPlan 和已批准 ProductionPackage 重建 compose 消费清单。
    pub async fn build_required_take_inventory(
        &self,
        run_id: Uuid,
    ) -> ProductionResult<RequiredTakeInventorySnapshot> {
        let mut tx = self.pool.begin().await?;
        let source = sqlx::query_as::<_, (i32, Uuid, i32, String, Option<String>)>(
            r#"
            SELECT run.current_revision_epoch,step.id,step.attempt,step.status,step.waiting_reason
            FROM production_runs run
            JOIN production_steps step
              ON step.run_id=run.id AND step.revision_epoch=run.current_revision_epoch
             AND step.step_key='wait_work_generation'
            WHERE run.id=$1
            FOR SHARE OF run,step
            "#,
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| media_evidence_blocker("required_take_source_step_missing"))?;
        let source_is_current = source.2 > 0
            && (source.3 == "succeeded"
                || (source.3 == "external_wait"
                    && source.4.as_deref() == Some("work_generation_evidence")));
        if !source_is_current {
            return Err(media_evidence_blocker(
                "required_take_source_step_not_ready",
            ));
        }

        let mut scopes = sqlx::query_as::<_, RequiredTakeBuildScopeRow>(
            r#"
            SELECT external.id AS work_generation_run_id,external.work_id,
                   external.work_version_id,external.status AS generation_status,work.script_id,
                   version.input_snapshot,plan.prompt_snapshot AS plan_prompt_snapshot,
                   external.prompt_snapshot AS generation_prompt_snapshot,
                   jsonb_build_object(
                       'id',version.id,'work_id',version.work_id,'version_no',version.version_no,
                       'source_manifest_version',version.source_manifest_version,
                       'input_snapshot',version.input_snapshot,'model_snapshot',version.model_snapshot,
                       'parameter_snapshot',version.parameter_snapshot,
                       'timeline_snapshot',version.timeline_snapshot,
                       'prompt_snapshot',version.prompt_snapshot
                   ) AS work_version_snapshot
            FROM production_domain_links link
            JOIN work_generation_runs external ON external.id=link.work_generation_run_id
            JOIN works work ON work.id=external.work_id
            JOIN work_versions version
              ON version.id=external.work_version_id AND version.work_id=external.work_id
            JOIN work_plans plan
              ON plan.id=external.work_plan_id AND plan.work_id=external.work_id
             AND plan.work_version_id=external.work_version_id
            WHERE link.run_id=$1 AND link.revision_epoch=$2
              AND link.link_type='work_generation_run'
            ORDER BY link.created_at,link.id
            FOR SHARE OF link,external,work,version,plan
            "#,
        )
        .bind(run_id)
        .bind(source.0)
        .fetch_all(&mut *tx)
        .await?;
        if scopes.len() != 1 {
            return Err(media_evidence_blocker(
                "required_take_generation_link_ambiguous",
            ));
        }
        let scope = scopes.remove(0);
        if scope.generation_status != "succeeded" {
            return Err(media_evidence_blocker(
                "required_take_generation_not_succeeded",
            ));
        }

        let production = scope
            .input_snapshot
            .get("production_crew")
            .ok_or_else(|| media_evidence_blocker("required_take_production_scope_missing"))?;
        if json_uuid(production.get("production_run_id")) != Some(run_id)
            || production.get("revision_epoch").and_then(Value::as_i64) != Some(source.0.into())
            || json_uuid(production.pointer("/script/script_id")) != Some(scope.script_id)
        {
            return Err(media_evidence_blocker(
                "required_take_production_scope_stale",
            ));
        }
        let package_ref = production
            .get("production_package")
            .ok_or_else(|| media_evidence_blocker("required_take_package_reference_missing"))?;
        let package_id = json_uuid(package_ref.get("id"))
            .ok_or_else(|| media_evidence_blocker("required_take_package_reference_invalid"))?;
        let package_version = package_ref
            .get("version")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| media_evidence_blocker("required_take_package_reference_invalid"))?;
        let package_source_step_id = json_uuid(package_ref.get("source_step_id"))
            .ok_or_else(|| media_evidence_blocker("required_take_package_reference_invalid"))?;
        let package_source_attempt = package_ref
            .get("source_attempt")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| media_evidence_blocker("required_take_package_reference_invalid"))?;
        let package_digest = package_ref
            .get("digest")
            .and_then(Value::as_str)
            .ok_or_else(|| media_evidence_blocker("required_take_package_reference_invalid"))?;
        let package = sqlx::query_as::<_, RequiredTakePackageRow>(
            r#"
            SELECT package.id,package.source_step_id,package.source_attempt,
                   package.package_version,package.package_digest,
                   package.revision_epoch,package.metadata,decision.decision
            FROM artifact_package_snapshots package
            JOIN production_gate_decisions decision
              ON decision.package_id=package.id AND decision.run_id=package.run_id
             AND decision.package_digest=package.package_digest
            WHERE package.id=$1 AND package.run_id=$2 AND package.package_type='production'
            FOR SHARE OF package,decision
            "#,
        )
        .bind(package_id)
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| media_evidence_blocker("required_take_approved_package_missing"))?;
        if package.package_version != package_version
            || package.package_digest != package_digest
            || package.source_step_id != package_source_step_id
            || package.source_attempt != package_source_attempt
            || package.revision_epoch != source.0
            || package.decision != "approved"
        {
            return Err(media_evidence_blocker(
                "required_take_approved_package_stale",
            ));
        }
        let package_metadata: ProductionPackageMetadata =
            serde_json::from_value(package.metadata.clone())
                .map_err(|_| media_evidence_blocker("required_take_package_metadata_invalid"))?;
        if package_metadata.script_id != scope.script_id {
            return Err(media_evidence_blocker("required_take_package_cross_script"));
        }

        let plan_segments_value = scope
            .plan_prompt_snapshot
            .get("segments")
            .cloned()
            .ok_or_else(|| media_evidence_blocker("required_take_plan_segments_missing"))?;
        if scope.generation_prompt_snapshot.get("segments") != Some(&plan_segments_value)
            || scope.input_snapshot.get("segments") != Some(&plan_segments_value)
        {
            return Err(media_evidence_blocker(
                "required_take_plan_snapshot_mismatch",
            ));
        }
        let plan_segments: Vec<RequiredTakePlanSegment> =
            serde_json::from_value(plan_segments_value)
                .map_err(|_| media_evidence_blocker("required_take_plan_segments_invalid"))?;
        if plan_segments.is_empty()
            || plan_segments.iter().enumerate().any(|(index, segment)| {
                segment.sequence != index + 1 || segment.scene_ids.is_empty()
            })
        {
            return Err(media_evidence_blocker(
                "required_take_plan_segments_invalid",
            ));
        }
        let flattened_scene_ids = plan_segments
            .iter()
            .flat_map(|segment| segment.scene_ids.iter().copied())
            .collect::<Vec<_>>();
        let package_scene_ids = package_metadata
            .scenes
            .iter()
            .map(|scene| scene.scene_id)
            .collect::<Vec<_>>();
        if flattened_scene_ids != package_scene_ids
            || flattened_scene_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != flattened_scene_ids.len()
        {
            return Err(media_evidence_blocker(
                "required_take_scene_order_or_coverage_invalid",
            ));
        }
        let formal_scene_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM scenes WHERE id=ANY($1) AND script_id=$2",
        )
        .bind(&package_scene_ids)
        .bind(scope.script_id)
        .fetch_one(&mut *tx)
        .await?;
        if formal_scene_count != package_scene_ids.len() as i64 {
            return Err(media_evidence_blocker("required_take_package_cross_script"));
        }

        let mut scene_shots = BTreeMap::<Uuid, Vec<Uuid>>::new();
        let mut shot_ids = BTreeSet::new();
        for shot in &package_metadata.shots {
            if !shot_ids.insert(shot.artifact_id) {
                return Err(media_evidence_blocker(
                    "required_take_shot_mapping_ambiguous",
                ));
            }
            scene_shots
                .entry(shot.scene_id)
                .or_default()
                .push(shot.artifact_id);
        }
        if package_scene_ids
            .iter()
            .any(|scene_id| scene_shots.get(scene_id).is_none_or(Vec::is_empty))
        {
            return Err(media_evidence_blocker("required_take_shot_mapping_missing"));
        }
        let package_shot_items = sqlx::query_as::<_, (Uuid, i32, String)>(
            r#"
            SELECT item.artifact_id,item.artifact_version,item.content_digest
            FROM artifact_package_items item
            WHERE item.package_id=$1 AND item.artifact_type='shot_contract'
            ORDER BY item.ordinal,item.id
            "#,
        )
        .bind(package.id)
        .fetch_all(&mut *tx)
        .await?;
        if package_shot_items.len() != shot_ids.len()
            || package_shot_items
                .iter()
                .map(|item| item.0)
                .collect::<BTreeSet<_>>()
                != shot_ids
        {
            return Err(media_evidence_blocker(
                "required_take_package_shot_items_mismatch",
            ));
        }
        for (shot_id, version, digest) in &package_shot_items {
            let expected_scene = package_metadata
                .shots
                .iter()
                .find(|shot| shot.artifact_id == *shot_id)
                .map(|shot| shot.scene_id)
                .ok_or_else(|| {
                    media_evidence_blocker("required_take_package_shot_items_mismatch")
                })?;
            let valid = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM shot_contracts
                    WHERE id=$1 AND run_id=$2 AND revision_epoch=$3
                      AND domain_scene_id=$4 AND version=$5 AND content_digest=$6
                      AND audit_status='complete'
                )
                "#,
            )
            .bind(shot_id)
            .bind(run_id)
            .bind(source.0)
            .bind(expected_scene)
            .bind(version)
            .bind(digest)
            .fetch_one(&mut *tx)
            .await?;
            if !valid {
                return Err(media_evidence_blocker(
                    "required_take_package_shot_provenance_stale",
                ));
            }
        }

        let mut compose_steps = sqlx::query_as::<_, RequiredTakeGenerationStepRow>(
            r#"
            SELECT id,step_type,status,depends_on,input_snapshot
            FROM work_generation_steps
            WHERE run_id=$1 AND step_type='compose' AND status='succeeded'
            ORDER BY step_no,id FOR SHARE
            "#,
        )
        .bind(scope.work_generation_run_id)
        .fetch_all(&mut *tx)
        .await?;
        if compose_steps.len() != 1 {
            return Err(media_evidence_blocker(
                "required_take_final_compose_ambiguous",
            ));
        }
        let compose = compose_steps.remove(0);
        let compose_attempts = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM work_generation_attempts WHERE step_id=$1 AND status='succeeded' ORDER BY attempt_no,id",
        )
        .bind(compose.id)
        .fetch_all(&mut *tx)
        .await?;
        if compose_attempts.len() != 1 {
            return Err(media_evidence_blocker(
                "required_take_final_compose_attempt_ambiguous",
            ));
        }
        let final_artifacts = sqlx::query_as::<_, (Uuid, String, String, Value)>(
            r#"
            SELECT id,sha256,mime_type,metadata FROM work_artifacts
            WHERE work_version_id=$1 AND generation_step_id=$2 AND role='final_video'
            ORDER BY created_at,id FOR SHARE
            "#,
        )
        .bind(scope.work_version_id)
        .bind(compose.id)
        .fetch_all(&mut *tx)
        .await?;
        if final_artifacts.len() != 1 {
            return Err(media_evidence_blocker(
                "required_take_final_media_ambiguous",
            ));
        }
        let final_artifact = &final_artifacts[0];
        if json_uuid(final_artifact.3.get("generation_attempt_id")) != Some(compose_attempts[0]) {
            return Err(media_evidence_blocker(
                "required_take_final_media_attempt_mismatch",
            ));
        }
        let duration_ms = media_duration_ms(&final_artifact.3)
            .ok_or_else(|| media_evidence_blocker("final_media_duration_missing"))?;

        let compose_dependencies = json_uuid_array(&compose.depends_on)
            .ok_or_else(|| media_evidence_blocker("required_take_compose_chain_invalid"))?;
        if compose_dependencies.len() != 1 {
            return Err(media_evidence_blocker(
                "required_take_compose_chain_invalid",
            ));
        }
        let mix = sqlx::query_as::<_, RequiredTakeGenerationStepRow>(
            r#"
            SELECT id,step_type,status,depends_on,input_snapshot
            FROM work_generation_steps WHERE id=$1 AND run_id=$2 FOR SHARE
            "#,
        )
        .bind(compose_dependencies[0])
        .bind(scope.work_generation_run_id)
        .fetch_optional(&mut *tx)
        .await?
        .filter(|step| step.step_type == "mix" && step.status == "succeeded")
        .ok_or_else(|| media_evidence_blocker("required_take_compose_chain_invalid"))?;
        let mix_dependencies = json_uuid_array(&mix.depends_on)
            .ok_or_else(|| media_evidence_blocker("required_take_mix_chain_invalid"))?;
        if mix_dependencies.is_empty()
            || mix_dependencies
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != mix_dependencies.len()
        {
            return Err(media_evidence_blocker("required_take_mix_chain_invalid"));
        }
        let dependency_steps = sqlx::query_as::<_, RequiredTakeGenerationStepRow>(
            r#"
            SELECT id,step_type,status,depends_on,input_snapshot
            FROM work_generation_steps WHERE id=ANY($1) AND run_id=$2
            ORDER BY step_no,id FOR SHARE
            "#,
        )
        .bind(&mix_dependencies)
        .bind(scope.work_generation_run_id)
        .fetch_all(&mut *tx)
        .await?;
        if dependency_steps.len() != mix_dependencies.len()
            || dependency_steps
                .iter()
                .any(|step| step.status != "succeeded")
        {
            return Err(media_evidence_blocker("required_take_mix_chain_invalid"));
        }
        let mut segment_steps = dependency_steps
            .into_iter()
            .filter(|step| step.step_type == "video_segment")
            .map(|step| {
                let segment: RequiredTakePlanSegment =
                    serde_json::from_value(step.input_snapshot.clone()).map_err(|_| {
                        media_evidence_blocker("required_take_segment_mapping_invalid")
                    })?;
                Ok((segment.sequence, segment, step))
            })
            .collect::<ProductionResult<Vec<_>>>()?;
        segment_steps.sort_by_key(|item| item.0);
        if segment_steps.len() != plan_segments.len()
            || segment_steps
                .iter()
                .zip(&plan_segments)
                .any(|((sequence, actual, _), expected)| {
                    *sequence != expected.sequence || actual.scene_ids != expected.scene_ids
                })
        {
            return Err(media_evidence_blocker(
                "required_take_segment_mapping_invalid",
            ));
        }

        let mut compose_inputs = Vec::with_capacity(segment_steps.len());
        for (_, segment, step) in segment_steps {
            let attempts = sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM work_generation_attempts WHERE step_id=$1 AND status='succeeded' ORDER BY attempt_no,id",
            )
            .bind(step.id)
            .fetch_all(&mut *tx)
            .await?;
            let artifacts = sqlx::query_as::<_, (Uuid, Value)>(
                r#"
                SELECT id,metadata FROM work_artifacts
                WHERE work_version_id=$1 AND generation_step_id=$2
                  AND role='reusable_intermediate'
                ORDER BY created_at,id
                "#,
            )
            .bind(scope.work_version_id)
            .bind(step.id)
            .fetch_all(&mut *tx)
            .await?;
            if attempts.len() != 1
                || artifacts.len() != 1
                || json_uuid(artifacts[0].1.get("generation_attempt_id")) != Some(attempts[0])
            {
                return Err(media_evidence_blocker(
                    "required_take_segment_provenance_ambiguous",
                ));
            }
            compose_inputs.push(ComposeInput {
                generation_step_id: step.id,
                generation_attempt_id: attempts[0],
                output_artifact_id: artifacts[0].0,
                segment_key: format!("segment-{}", segment.sequence),
                shot_contracts: segment
                    .scene_ids
                    .iter()
                    .map(|scene_id| {
                        scene_shots
                            .get(scene_id)
                            .cloned()
                            .map(|shots| (*scene_id, shots))
                            .ok_or_else(|| {
                                media_evidence_blocker("required_take_shot_mapping_missing")
                            })
                    })
                    .collect::<ProductionResult<Vec<_>>>()?,
                scene_ids: segment.scene_ids,
                consumed_by_final_compose: true,
                generation_succeeded: true,
            });
        }

        let work_version_hash = canonical_digest(&scope.work_version_snapshot)?;
        let inventory_id = Uuid::new_v5(
            &run_id,
            format!(
                "required-take-inventory:{}:{}:{}:{}",
                source.0, scope.work_generation_run_id, scope.work_version_id, work_version_hash
            )
            .as_bytes(),
        );
        let inventory = RequiredTakeInventorySnapshot::build(
            inventory_id,
            run_id,
            source.1,
            u32::try_from(source.2)
                .map_err(|_| media_evidence_blocker("required_take_source_attempt_invalid"))?,
            u32::try_from(source.0)
                .map_err(|_| media_evidence_blocker("required_take_revision_invalid"))?,
            scope.work_id,
            scope.work_version_id,
            scope.work_generation_run_id,
            FinalMediaAsset {
                artifact_id: final_artifact.0,
                sha256: final_artifact.1.clone(),
                mime_type: final_artifact.2.clone(),
                duration_ms,
            },
            work_version_hash,
            compose_inputs,
        )
        .map_err(|_| media_evidence_blocker("required_take_inventory_invalid"))?;
        tx.commit().await?;
        Ok(inventory)
    }

    /// 计算当前 WorkVersion 全快照 hash，供 inventory 绑定真实版本内容。
    pub async fn work_version_hash(&self, work_version_id: Uuid) -> ProductionResult<String> {
        let snapshot = sqlx::query_scalar::<_, Value>(
            r#"
            SELECT jsonb_build_object(
                'id',id,'work_id',work_id,'version_no',version_no,
                'source_manifest_version',source_manifest_version,
                'input_snapshot',input_snapshot,'model_snapshot',model_snapshot,
                'parameter_snapshot',parameter_snapshot,'timeline_snapshot',timeline_snapshot,
                'prompt_snapshot',prompt_snapshot
            )
            FROM work_versions WHERE id=$1
            "#,
        )
        .bind(work_version_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "WorkVersion not found".into(),
        })?;
        Ok(canonical_digest(&snapshot)?)
    }

    /// 原子保存 final compose 的确定性消费清单；所有正式外键先复验再写入。
    pub async fn save_required_take_inventory(
        &self,
        inventory: &RequiredTakeInventorySnapshot,
    ) -> ProductionResult<()> {
        inventory.validate()?;
        let mut tx = self.pool.begin().await?;
        let source = sqlx::query_as::<_, (Uuid, i32, i32, String, Option<String>)>(
            "SELECT run_id,revision_epoch,attempt,status,waiting_reason FROM production_steps WHERE id=$1 FOR UPDATE",
        )
        .bind(inventory.source_step_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "inventory source ProductionStep not found".into(),
        })?;
        if source.0 != inventory.run_id
            || source.1 != inventory.revision_epoch as i32
            || source.2 != inventory.source_attempt as i32
            || !(source.3 == "succeeded"
                || (source.3 == "external_wait"
                    && source.4.as_deref() == Some("work_generation_evidence")))
        {
            return Err(ProductionError::TransitionConflict {
                reason:
                    "inventory source step/attempt is not the current successful generation fact"
                        .into(),
            });
        }
        let generation = sqlx::query_as::<_, (Uuid, Uuid, String)>(
            "SELECT work_id,work_version_id,status FROM work_generation_runs WHERE id=$1",
        )
        .bind(inventory.work_generation_run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "inventory WorkGenerationRun not found".into(),
        })?;
        if generation.0 != inventory.work_id
            || generation.1 != inventory.work_version_id
            || generation.2 != "succeeded"
        {
            return Err(ProductionError::TransitionConflict {
                reason: "inventory does not target a succeeded WorkGenerationRun version".into(),
            });
        }
        let formally_linked = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM production_domain_links
                WHERE run_id=$1 AND link_type='work_generation_run'
                  AND work_generation_run_id=$2
            )
            "#,
        )
        .bind(inventory.run_id)
        .bind(inventory.work_generation_run_id)
        .fetch_one(&mut *tx)
        .await?;
        if !formally_linked {
            return Err(ProductionError::TransitionConflict {
                reason: "inventory WorkGenerationRun is not linked to this ProductionRun".into(),
            });
        }
        let final_artifact = sqlx::query_as::<_, (Uuid, String, String, Value)>(
            "SELECT work_version_id,role,sha256,metadata FROM work_artifacts WHERE id=$1",
        )
        .bind(inventory.final_asset.artifact_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "inventory final media artifact not found".into(),
        })?;
        let final_mime =
            sqlx::query_scalar::<_, String>("SELECT mime_type FROM work_artifacts WHERE id=$1")
                .bind(inventory.final_asset.artifact_id)
                .fetch_one(&mut *tx)
                .await?;
        if final_artifact.0 != inventory.work_version_id
            || final_artifact.1 != "final_video"
            || final_artifact.2 != inventory.final_asset.sha256
            || final_mime != inventory.final_asset.mime_type
            || media_duration_ms(&final_artifact.3) != Some(inventory.final_asset.duration_ms)
        {
            return Err(ProductionError::TransitionConflict {
                reason: "inventory final media identity differs from WorkArtifact".into(),
            });
        }
        let persisted_hash = self.work_version_hash(inventory.work_version_id).await?;
        if persisted_hash != inventory.work_version_hash {
            return Err(ProductionError::TransitionConflict {
                reason: "inventory WorkVersion hash is stale".into(),
            });
        }

        for take in &inventory.takes {
            let generation_step = sqlx::query_as::<_, (Uuid, String, String)>(
                "SELECT run_id,status,step_type FROM work_generation_steps WHERE id=$1",
            )
            .bind(take.generation_step_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ProductionError::TransitionConflict {
                reason: "required take generation step not found".into(),
            })?;
            let attempt = sqlx::query_as::<_, (Uuid, String)>(
                "SELECT step_id,status FROM work_generation_attempts WHERE id=$1",
            )
            .bind(take.generation_attempt_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ProductionError::TransitionConflict {
                reason: "required take generation attempt not found".into(),
            })?;
            let output = sqlx::query_as::<_, (Uuid, Option<Uuid>, String)>(
                "SELECT work_version_id,generation_step_id,role FROM work_artifacts WHERE id=$1",
            )
            .bind(take.output_artifact_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ProductionError::TransitionConflict {
                reason: "required take output WorkArtifact not found".into(),
            })?;
            if generation_step.0 != inventory.work_generation_run_id
                || generation_step.1 != "succeeded"
                || generation_step.2 != "video_segment"
                || attempt != (take.generation_step_id, "succeeded".into())
                || output.0 != inventory.work_version_id
                || output.1 != Some(take.generation_step_id)
                || output.2 != "reusable_intermediate"
            {
                return Err(ProductionError::TransitionConflict {
                    reason: "required take generation provenance is inconsistent".into(),
                });
            }
            for scene_id in &take.scene_ids {
                let scene_valid = sqlx::query_scalar::<_, bool>(
                    r#"
                    SELECT EXISTS(
                        SELECT 1 FROM scenes scene
                        JOIN works work ON work.script_id=scene.script_id
                        WHERE scene.id=$1 AND work.id=$2
                    )
                    "#,
                )
                .bind(scene_id)
                .bind(inventory.work_id)
                .fetch_one(&mut *tx)
                .await?;
                let shot_ids = take.scene_shot_map.get(scene_id).ok_or_else(|| {
                    ProductionError::TransitionConflict {
                        reason: "required take Scene has no ShotContract set".into(),
                    }
                })?;
                let shot_count = sqlx::query_scalar::<_, i64>(
                    r#"
                    SELECT COUNT(*) FROM shot_contracts
                    WHERE id=ANY($1) AND run_id=$2 AND domain_scene_id=$3
                    "#,
                )
                .bind(shot_ids)
                .bind(inventory.run_id)
                .bind(scene_id)
                .fetch_one(&mut *tx)
                .await?;
                if !scene_valid || shot_count != shot_ids.len() as i64 {
                    return Err(ProductionError::TransitionConflict {
                        reason: "required take contains cross-Script or ambiguous Shot mapping"
                            .into(),
                    });
                }
            }
        }

        sqlx::query(
            r#"
            INSERT INTO required_take_inventories (
                id,run_id,source_step_id,source_attempt,revision_epoch,work_id,
                work_version_id,work_generation_run_id,final_artifact_id,
                work_version_hash,inventory_digest
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            ON CONFLICT (run_id,work_version_id,inventory_digest) DO NOTHING
            "#,
        )
        .bind(inventory.inventory_id)
        .bind(inventory.run_id)
        .bind(inventory.source_step_id)
        .bind(inventory.source_attempt as i32)
        .bind(inventory.revision_epoch as i32)
        .bind(inventory.work_id)
        .bind(inventory.work_version_id)
        .bind(inventory.work_generation_run_id)
        .bind(inventory.final_asset.artifact_id)
        .bind(&inventory.work_version_hash)
        .bind(&inventory.inventory_digest)
        .execute(&mut *tx)
        .await?;
        let persisted_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM required_take_inventories WHERE run_id=$1 AND work_version_id=$2 AND inventory_digest=$3",
        )
        .bind(inventory.run_id)
        .bind(inventory.work_version_id)
        .bind(&inventory.inventory_digest)
        .fetch_one(&mut *tx)
        .await?;
        if persisted_id != inventory.inventory_id {
            return Err(ProductionError::TransitionConflict {
                reason: "inventory digest is already bound to another immutable identity".into(),
            });
        }
        for take in &inventory.takes {
            sqlx::query(
                r#"
                INSERT INTO required_takes (
                    id,inventory_id,ordinal,take_key,generation_step_id,generation_attempt_id,
                    output_artifact_id,segment_key,scene_ids,scene_shot_map
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                ON CONFLICT (inventory_id,take_key) DO NOTHING
                "#,
            )
            .bind(take.take_id)
            .bind(inventory.inventory_id)
            .bind(
                i32::try_from(take.ordinal).map_err(|_| ProductionError::TransitionConflict {
                    reason: "required take ordinal exceeds PostgreSQL range".into(),
                })?,
            )
            .bind(take.take_id.to_string())
            .bind(take.generation_step_id)
            .bind(take.generation_attempt_id)
            .bind(take.output_artifact_id)
            .bind(&take.segment_key)
            .bind(serde_json::to_value(&take.scene_ids)?)
            .bind(serde_json::to_value(&take.scene_shot_map)?)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// 保存 provider 返回的不可变脱敏媒体证据；临时访问参数不进入该接口。
    pub async fn save_media_evidence(
        &self,
        evidence: &MediaEvidenceSnapshot,
    ) -> ProductionResult<()> {
        evidence.validate()?;
        let mut tx = self.pool.begin().await?;
        let inventory = sqlx::query_as::<_, (Uuid, Uuid, i32, i32, Uuid, Uuid, String)>(
            r#"
            SELECT run_id,source_step_id,source_attempt,revision_epoch,
                   work_version_id,final_artifact_id,inventory_digest
            FROM required_take_inventories WHERE id=$1 FOR UPDATE
            "#,
        )
        .bind(evidence.inventory_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "media evidence inventory not found".into(),
        })?;
        if inventory
            != (
                evidence.run_id,
                evidence.source_step_id,
                evidence.source_attempt as i32,
                evidence.revision_epoch as i32,
                evidence.work_version_id,
                evidence.final_artifact_id,
                evidence.inventory_digest.clone(),
            )
        {
            return Err(ProductionError::TransitionConflict {
                reason: "media evidence differs from required take inventory".into(),
            });
        }
        let final_artifact = sqlx::query_as::<_, (String, String, Value)>(
            "SELECT sha256,mime_type,metadata FROM work_artifacts WHERE id=$1 AND work_version_id=$2 AND role='final_video'",
        )
        .bind(evidence.final_artifact_id)
        .bind(evidence.work_version_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "media evidence final WorkArtifact not found".into(),
        })?;
        if final_artifact.0 != evidence.asset_hash
            || final_artifact.1 != evidence.mime_type
            || media_duration_ms(&final_artifact.2) != Some(evidence.duration_ms)
        {
            return Err(ProductionError::TransitionConflict {
                reason: "media evidence final asset identity is stale".into(),
            });
        }
        sqlx::query(
            r#"
            INSERT INTO media_evidence_snapshots (
                id,run_id,source_step_id,source_attempt,revision_epoch,work_version_id,
                inventory_id,final_artifact_id,asset_hash,mime_type,duration_ms,
                vision_capability_version,audio_capability_version,redacted_analysis,evidence_digest
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
            ON CONFLICT (run_id,work_version_id,inventory_id,evidence_digest) DO NOTHING
            "#,
        )
        .bind(evidence.evidence_id)
        .bind(evidence.run_id)
        .bind(evidence.source_step_id)
        .bind(evidence.source_attempt as i32)
        .bind(evidence.revision_epoch as i32)
        .bind(evidence.work_version_id)
        .bind(evidence.inventory_id)
        .bind(evidence.final_artifact_id)
        .bind(&evidence.asset_hash)
        .bind(&evidence.mime_type)
        .bind(i64::try_from(evidence.duration_ms).map_err(|_| {
            ProductionError::TransitionConflict {
                reason: "media duration exceeds PostgreSQL range".into(),
            }
        })?)
        .bind(&evidence.vision_capability_version)
        .bind(&evidence.audio_capability_version)
        .bind(&evidence.redacted_analysis)
        .bind(&evidence.evidence_digest)
        .execute(&mut *tx)
        .await?;
        let persisted_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM media_evidence_snapshots WHERE run_id=$1 AND work_version_id=$2 AND inventory_id=$3 AND evidence_digest=$4",
        )
        .bind(evidence.run_id)
        .bind(evidence.work_version_id)
        .bind(evidence.inventory_id)
        .bind(&evidence.evidence_digest)
        .fetch_one(&mut *tx)
        .await?;
        if persisted_id != evidence.evidence_id {
            return Err(ProductionError::TransitionConflict {
                reason: "evidence digest is already bound to another immutable identity".into(),
            });
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn save_package(&self, package: &ArtifactPackageSnapshot) -> ProductionResult<()> {
        self.persist_package(package, None).await
    }

    /// 保存由 Runner 当前 lease 构建的 package，并在同一事务闭合 Gate attempt。
    pub async fn save_claimed_package(
        &self,
        package: &ArtifactPackageSnapshot,
        gate_step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
    ) -> ProductionResult<()> {
        if lease_owner.trim().is_empty() || attempt <= 0 {
            return Err(ProductionError::TransitionConflict {
                reason: "claimed package requires a lease owner and positive attempt".into(),
            });
        }
        self.persist_package(package, Some((gate_step_id, lease_owner, attempt)))
            .await
    }

    async fn persist_package(
        &self,
        package: &ArtifactPackageSnapshot,
        gate_claim: Option<(Uuid, &str, i32)>,
    ) -> ProductionResult<()> {
        let mut tx = self.pool.begin().await?;
        let source = sqlx::query_as::<_, (Uuid, i32, i32, String)>(
            r#"
            SELECT run_id, revision_epoch, attempt, status
            FROM production_steps WHERE id = $1 FOR UPDATE
            "#,
        )
        .bind(package.source_step_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "package source step not found".into(),
        })?;
        if source.0 != package.run_id
            || source.1 != package.revision_epoch as i32
            || source.2 != package.source_attempt as i32
            || source.3 != "succeeded"
            || package
                .items
                .iter()
                .any(|item| item.run_id != package.run_id)
        {
            return Err(ProductionError::TransitionConflict {
                reason: "package source run/epoch/step/attempt is inconsistent".into(),
            });
        }
        for item in &package.items {
            let item_source_valid = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM production_steps
                    WHERE id = $1 AND run_id = $2 AND revision_epoch = $3
                      AND attempt = $4 AND status = 'succeeded'
                )
                "#,
            )
            .bind(item.source_step_id)
            .bind(package.run_id)
            .bind(package.revision_epoch as i32)
            .bind(item.source_attempt as i32)
            .fetch_one(&mut *tx)
            .await?;
            if !item_source_valid {
                return Err(ProductionError::TransitionConflict {
                    reason: "package item source run/epoch/step/attempt is inconsistent".into(),
                });
            }
        }
        let current_epoch = sqlx::query_scalar::<_, i32>(
            "SELECT current_revision_epoch FROM production_runs WHERE id = $1 FOR UPDATE",
        )
        .bind(package.run_id)
        .fetch_one(&mut *tx)
        .await?;
        if current_epoch != package.revision_epoch as i32 {
            return Err(ProductionError::StalePackage);
        }
        if package.package_type == PackageType::Production {
            validate_production_suggestion_resolutions(&mut tx, package).await?;
        }
        let gate_key = gate_step_key(package.package_type);
        if let Some((gate_step_id, lease_owner, attempt)) = gate_claim {
            let owns_gate = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM production_steps
                    WHERE id=$1 AND run_id=$2 AND revision_epoch=$3
                      AND step_key=$4 AND step_type='gate' AND status='running'
                      AND attempt=$5 AND lease_owner=$6 AND lease_expires_at >= NOW()
                )
                "#,
            )
            .bind(gate_step_id)
            .bind(package.run_id)
            .bind(package.revision_epoch as i32)
            .bind(gate_key)
            .bind(attempt)
            .bind(lease_owner)
            .fetch_one(&mut *tx)
            .await?;
            if !owns_gate {
                return Err(ProductionError::TransitionConflict {
                    reason: "package Gate lease is no longer current".into(),
                });
            }
        } else {
            let (plan, workflow) = load_frozen_workflow(&mut tx, package.run_id).await?;
            let ready = workflow.with_dependency_unlocks(&plan)?;
            let enter_gate = WorkflowCommand::step(WorkflowCommandKind::EnterWait, gate_key);
            if !allowed_commands(&plan, &ready)?.contains(&enter_gate) {
                return Err(ProductionError::TransitionConflict {
                    reason: "package gate is outside the current runnable plan".into(),
                });
            }
        }
        let package_type = package_type_name(package.package_type);
        sqlx::query(
            r#"
            INSERT INTO artifact_package_snapshots (
                id, run_id, source_step_id, source_attempt, revision_epoch,
                package_type, package_version, package_digest, schema_version, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '1.0.0', $9)
            "#,
        )
        .bind(package.id)
        .bind(package.run_id)
        .bind(package.source_step_id)
        .bind(package.source_attempt as i32)
        .bind(package.revision_epoch as i32)
        .bind(package_type)
        .bind(package.package_version as i32)
        .bind(&package.package_digest)
        .bind(&package.metadata)
        .execute(&mut *tx)
        .await?;
        for (ordinal, item) in package.items.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO artifact_package_items (
                    package_id, ordinal, artifact_type, artifact_id, artifact_version,
                    content_digest, source_step_id, source_attempt
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(package.id)
            .bind(ordinal as i32)
            .bind(&item.artifact_type)
            .bind(item.artifact_id)
            .bind(item.version as i32)
            .bind(&item.content_digest)
            .bind(item.source_step_id)
            .bind(item.source_attempt as i32)
            .execute(&mut *tx)
            .await?;
        }
        let updated = if let Some((gate_step_id, lease_owner, attempt)) = gate_claim {
            let attempt_updated = sqlx::query(
                r#"
                UPDATE production_step_attempts
                SET status='succeeded', side_effect_state='confirmed',
                    result=$4, completed_at=NOW()
                WHERE step_id=$1 AND attempt_no=$2 AND lease_owner=$3
                  AND status IN ('running','prepared')
                "#,
            )
            .bind(gate_step_id)
            .bind(attempt)
            .bind(lease_owner)
            .bind(json!({
                "package_id": package.id,
                "package_digest": package.package_digest,
            }))
            .execute(&mut *tx)
            .await?;
            if attempt_updated.rows_affected() != 1 {
                return Err(ProductionError::TransitionConflict {
                    reason: "package Gate attempt is no longer current".into(),
                });
            }
            sqlx::query(
                r#"
                UPDATE production_steps
                SET status='waiting_approval', input_package_id=$2, input_digest=$3,
                    waiting_reason='package_approval', side_effect_state='confirmed',
                    lease_owner=NULL, lease_expires_at=NULL, updated_at=NOW()
                WHERE id=$1 AND status='running' AND attempt=$4 AND lease_owner=$5
                "#,
            )
            .bind(gate_step_id)
            .bind(package.id)
            .bind(&package.package_digest)
            .bind(attempt)
            .bind(lease_owner)
            .execute(&mut *tx)
            .await?
        } else {
            sqlx::query(
                r#"
                UPDATE production_steps
                SET status = 'waiting_approval', input_package_id = $4,
                    input_digest = $5, waiting_reason = 'package_approval', updated_at = NOW()
                WHERE run_id = $1 AND revision_epoch = $2 AND step_key = $3
                  AND status IN ('blocked', 'queued')
                "#,
            )
            .bind(package.run_id)
            .bind(package.revision_epoch as i32)
            .bind(gate_key)
            .bind(package.id)
            .bind(&package.package_digest)
            .execute(&mut *tx)
            .await?
        };
        if updated.rows_affected() != 1 {
            return Err(ProductionError::TransitionConflict {
                reason: "package gate is absent or not ready".into(),
            });
        }
        sqlx::query(
            "UPDATE production_runs SET status = 'waiting_approval', updated_at = NOW() WHERE id = $1",
        )
        .bind(package.run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn decide_package(
        &self,
        mut command: PackageDecisionCommand,
    ) -> ProductionResult<PersistedGateDecision> {
        command.actor.validate()?;
        command.affected_owners.sort();
        command.affected_owners.dedup();
        if command.decision == GateDecision::Reject
            && (command
                .reason
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
                || command.affected_owners.is_empty())
        {
            return Err(ProductionError::TransitionConflict {
                reason: "reject requires a non-empty reason and affected owner".into(),
            });
        }
        let request_digest = ProductionCommandStore::canonical_request_digest(&json!({
            "run_id": command.run_id,
            "package_digest": command.package_digest,
            "decision": command.decision,
            "reason": command.reason,
            "affected_owners": command.affected_owners,
        }))?;
        let command_scope = ProductionCommandScope::new(
            command.actor.clone(),
            match command.decision {
                GateDecision::Approve => ProductionCommandType::ApprovePackage,
                GateDecision::Reject => ProductionCommandType::RejectPackage,
            },
            ProductionAggregateType::ProductionRun,
            command.run_id,
            &command.idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        if let Some(result) =
            ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest).await?
        {
            let decision_id = uuid_from_result(&result, "decision_id")?;
            let record = load_gate_decision(&mut tx, decision_id).await?;
            tx.commit().await?;
            return Ok(record);
        }

        let package = sqlx::query_as::<_, PackageIdentity>(
            r#"
            SELECT package.id, package.package_type, package.revision_epoch, package.metadata
            FROM artifact_package_snapshots package
            JOIN production_runs run ON run.id = package.run_id
            WHERE package.run_id = $1 AND package.package_digest = $2
              AND package.revision_epoch = run.current_revision_epoch
              AND package.package_version = (
                  SELECT MAX(candidate.package_version)
                  FROM artifact_package_snapshots candidate
                  WHERE candidate.run_id = package.run_id
                    AND candidate.package_type = package.package_type
                    AND candidate.revision_epoch = package.revision_epoch
              )
            FOR UPDATE OF run
            "#,
        )
        .bind(command.run_id)
        .bind(&command.package_digest)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ProductionError::StalePackage)?;
        let existing = sqlx::query_as::<_, PersistedGateDecision>(
            r#"
            SELECT id, run_id, gate_step_id, package_id, package_digest, revision_epoch,
                   decision, reason, affected_owners, actor_type, actor_id, decided_at
            FROM production_gate_decisions WHERE package_id = $1
            "#,
        )
        .bind(package.id)
        .fetch_optional(&mut *tx)
        .await?;
        let database_decision = gate_decision_name(command.decision);
        if let Some(existing) = existing {
            if existing.decision != database_decision {
                return Err(ProductionError::TransitionConflict {
                    reason: "the package already has the opposite immutable decision".into(),
                });
            }
            ProductionCommandStore::record(
                &mut tx,
                &command_scope,
                &request_digest,
                json!({"decision_id": existing.id}),
            )
            .await?;
            tx.commit().await?;
            return Ok(existing);
        }
        let package_type = parse_package_type(&package.package_type)?;
        let gate_key = gate_step_key(package_type);
        validate_persisted_workflow_command(
            &mut tx,
            command.run_id,
            &command.idempotency_key,
            WorkflowCommand::step(
                match command.decision {
                    GateDecision::Approve => WorkflowCommandKind::ApprovePackage,
                    GateDecision::Reject => WorkflowCommandKind::RejectPackage,
                },
                gate_key,
            ),
        )
        .await?;
        let gate_step_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM production_steps
            WHERE run_id = $1 AND revision_epoch = $2 AND step_key = $3
              AND status = 'waiting_approval' AND input_package_id = $4
            FOR UPDATE
            "#,
        )
        .bind(command.run_id)
        .bind(package.revision_epoch)
        .bind(gate_key)
        .bind(package.id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "package gate is not waiting for this exact package".into(),
        })?;
        let decision_id = Uuid::new_v4();
        let record = sqlx::query_as::<_, PersistedGateDecision>(
            r#"
            INSERT INTO production_gate_decisions (
                id, run_id, gate_step_id, package_id, package_digest, revision_epoch,
                decision, reason, affected_owners, actor_type, actor_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, run_id, gate_step_id, package_id, package_digest, revision_epoch,
                      decision, reason, affected_owners, actor_type, actor_id, decided_at
            "#,
        )
        .bind(decision_id)
        .bind(command.run_id)
        .bind(gate_step_id)
        .bind(package.id)
        .bind(&command.package_digest)
        .bind(package.revision_epoch)
        .bind(database_decision)
        .bind(&command.reason)
        .bind(serde_json::to_value(&command.affected_owners)?)
        .bind(&command.actor.actor_type)
        .bind(&command.actor.actor_id)
        .fetch_one(&mut *tx)
        .await?;

        match command.decision {
            GateDecision::Approve => {
                if package_type == PackageType::Quality {
                    let gate_input: QualityGateInput = serde_json::from_value(
                        package
                            .metadata
                            .get("quality_gate_input")
                            .cloned()
                            .ok_or_else(|| quality_package_blocker("quality_gate_input_missing"))?,
                    )
                    .map_err(|_| quality_package_blocker("quality_gate_input_invalid"))?;
                    if QualityGate::evaluate(&gate_input)? != QualityGateOutcome::Approved {
                        return Err(ProductionError::TransitionConflict {
                            reason: "QualityPackage has not passed the QualityGate".into(),
                        });
                    }
                }
                sqlx::query(
                    r#"
                    UPDATE production_steps
                    SET status = 'succeeded', output_digest = $2, waiting_reason = NULL,
                        completed_at = NOW(), updated_at = NOW()
                    WHERE id = $1
                    "#,
                )
                .bind(gate_step_id)
                .bind(&command.package_digest)
                .execute(&mut *tx)
                .await?;
                unlock_ready_steps(&mut tx, command.run_id, package.revision_epoch).await?;
                if package_type == PackageType::Quality {
                    sqlx::query("UPDATE production_runs SET quality_status='approved' WHERE id=$1")
                        .bind(command.run_id)
                        .execute(&mut *tx)
                        .await?;
                    set_production_run_state(&mut tx, command.run_id, "completed", None, None)
                        .await?;
                } else {
                    sqlx::query(
                        "UPDATE production_runs SET status='queued',updated_at=NOW() WHERE id=$1",
                    )
                    .bind(command.run_id)
                    .execute(&mut *tx)
                    .await?;
                }
            }
            GateDecision::Reject => {
                create_rejection_epoch(
                    &mut tx,
                    command.run_id,
                    package_type,
                    package.revision_epoch,
                    package.id,
                    command.reason.as_deref().unwrap_or_default(),
                    &command.affected_owners,
                    &command.actor,
                )
                .await?;
            }
        }
        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            json!({"decision_id": record.id}),
        )
        .await?;
        tx.commit().await?;
        Ok(record)
    }

    pub async fn create_collaboration_suggestion(
        &self,
        command: CreateCollaborationSuggestionCommand,
    ) -> ProductionResult<PersistedCollaborationSuggestion> {
        validate_digest(&command.target_content_digest)?;
        validate_safe_object(&command.content)?;
        if command.source_attempt <= 0
            || command.target_artifact_version <= 0
            || command.from_role.trim().is_empty()
            || command.to_role.trim().is_empty()
            || command.target_artifact_type.trim().is_empty()
            || !matches!(
                command.suggestion_type.as_str(),
                "revision" | "addition" | "deletion"
            )
        {
            return Err(ProductionError::TransitionConflict {
                reason: "suggestion source, target, roles, and type must be complete".into(),
            });
        }
        let mut tx = self.pool.begin().await?;
        let source = sqlx::query_as::<_, (Uuid, i32, String, Option<String>, i32, Option<Uuid>)>(
            r#"
            SELECT run.production_project_id, step.revision_epoch, step.status,
                   step.role_key, step.attempt, step.model_call_id
            FROM production_steps step
            JOIN production_runs run ON run.id = step.run_id
            WHERE step.id = $1 AND step.run_id = $2
            FOR UPDATE OF step
            "#,
        )
        .bind(command.source_step_id)
        .bind(command.run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "suggestion source step is not part of the run".into(),
        })?;
        if !matches!(source.2.as_str(), "running" | "succeeded")
            || source.3.as_deref() != Some(command.from_role.as_str())
            || source.4 != command.source_attempt
            || source.5 != Some(command.source_model_call_id)
        {
            return Err(ProductionError::TransitionConflict {
                reason: "suggestion source role/attempt/ModelCall does not match the step".into(),
            });
        }
        let model_call_valid =
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM model_calls WHERE id = $1)")
                .bind(command.source_model_call_id)
                .fetch_one(&mut *tx)
                .await?;
        if !model_call_valid {
            return Err(ProductionError::TransitionConflict {
                reason: "suggestion source ModelCall does not exist".into(),
            });
        }
        let record = sqlx::query_as::<_, PersistedCollaborationSuggestion>(
            r#"
            INSERT INTO collaboration_suggestions (
                production_project_id, from_role, to_role, artifact_type, artifact_id,
                suggestion_type, content, run_id, source_step_id, source_attempt,
                revision_epoch, source_model_call_id, target_artifact_version,
                target_content_digest, blocking, audit_status
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, 'complete'
            )
            RETURNING id, production_project_id, run_id, source_step_id, source_attempt,
                      revision_epoch, source_model_call_id, from_role, to_role,
                      artifact_type, artifact_id, target_artifact_version,
                      target_content_digest, suggestion_type, content, blocking, status
            "#,
        )
        .bind(source.0)
        .bind(&command.from_role)
        .bind(&command.to_role)
        .bind(&command.target_artifact_type)
        .bind(command.target_artifact_id)
        .bind(&command.suggestion_type)
        .bind(&command.content)
        .bind(command.run_id)
        .bind(command.source_step_id)
        .bind(command.source_attempt)
        .bind(source.1)
        .bind(command.source_model_call_id)
        .bind(command.target_artifact_version)
        .bind(&command.target_content_digest)
        .bind(command.blocking)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(record)
    }

    pub async fn respond_to_collaboration_suggestion(
        &self,
        suggestion_id: Uuid,
        decision: SuggestionDecision,
        reason: Option<String>,
        actor: ProductionActor,
    ) -> ProductionResult<PersistedSuggestionResponse> {
        actor.validate()?;
        if decision == SuggestionDecision::Rejected
            && reason
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(ProductionError::TransitionConflict {
                reason: "rejected suggestion requires a non-empty reason".into(),
            });
        }
        let decision_name = match decision {
            SuggestionDecision::Accepted => "accepted",
            SuggestionDecision::Rejected => "rejected",
        };
        let mut tx = self.pool.begin().await?;
        let suggestion_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM collaboration_suggestions WHERE id = $1 AND audit_status = 'complete')",
        )
        .bind(suggestion_id)
        .fetch_one(&mut *tx)
        .await?;
        if !suggestion_exists {
            return Err(ProductionError::SuggestionNotFound { suggestion_id });
        }
        if let Some(existing) = sqlx::query_as::<_, PersistedSuggestionResponse>(
            r#"
            SELECT id, suggestion_id, decision, reason, actor_type, actor_id, created_at
            FROM collaboration_suggestion_responses
            WHERE suggestion_id = $1
            "#,
        )
        .bind(suggestion_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            if existing.decision == decision_name {
                tx.commit().await?;
                return Ok(existing);
            }
            return Err(ProductionError::TransitionConflict {
                reason: "suggestion already has the opposite immutable response".into(),
            });
        }
        let response = sqlx::query_as::<_, PersistedSuggestionResponse>(
            r#"
            INSERT INTO collaboration_suggestion_responses (
                suggestion_id, decision, reason, actor_type, actor_id
            ) VALUES ($1, $2, $3, $4, $5)
            RETURNING id, suggestion_id, decision, reason, actor_type, actor_id, created_at
            "#,
        )
        .bind(suggestion_id)
        .bind(decision_name)
        .bind(&reason)
        .bind(&actor.actor_type)
        .bind(&actor.actor_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE collaboration_suggestions
            SET status = $2, responded_at = NOW(), response_note = $3, updated_at = NOW()
            WHERE id = $1 AND status = 'pending'
            "#,
        )
        .bind(suggestion_id)
        .bind(decision_name)
        .bind(&reason)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(response)
    }

    async fn get_run_record(&self, id: Uuid) -> ProductionResult<ProductionRunRecord> {
        sqlx::query_as::<_, ProductionRunRecord>(
            r#"
            SELECT id, production_project_id, plan_snapshot_id, status, quality_status,
                   current_revision_epoch, resource_limits, binding_snapshot, source_snapshot,
                   cancellation_intent, error_code, error_details, actor_type, actor_id,
                   created_at, updated_at
            FROM production_runs WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: format!("production run {id} not found"),
        })
    }

    pub async fn claim_step(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        lease_ttl: Duration,
        request_digest: &str,
        idempotency_key: &str,
    ) -> ProductionResult<ProductionStepRecord> {
        if lease_owner.trim().is_empty()
            || lease_ttl.is_zero()
            || request_digest.len() != 64
            || idempotency_key.trim().is_empty()
        {
            return Err(ProductionError::TransitionConflict {
                reason: "claim identity, ttl, digest, and idempotency key are required".into(),
            });
        }
        let ttl = chrono::Duration::from_std(lease_ttl).map_err(|_| {
            ProductionError::TransitionConflict {
                reason: "lease ttl is outside supported range".into(),
            }
        })?;
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_as::<_, ProductionStepRecord>(
            r#"
            SELECT id, run_id, revision_epoch, plan_order, step_key, step_type, role_key,
                   dependencies, status, waiting_reason, error_code, error_details, retryable, attempt,
                   lease_owner, lease_expires_at, side_effect_state,
                   agent_run_id, model_call_id, context_snapshot_id
            FROM production_steps WHERE id = $1 FOR UPDATE
            "#,
        )
        .bind(step_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production step not found".into(),
        })?;
        let expired = current
            .lease_expires_at
            .is_some_and(|expires_at| expires_at < Utc::now());
        let run_claimable = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT current_revision_epoch = $2
               AND cancellation_intent IS NULL
               AND status IN ('queued', 'running')
            FROM production_runs WHERE id = $1 FOR UPDATE
            "#,
        )
        .bind(current.run_id)
        .bind(current.revision_epoch)
        .fetch_one(&mut *tx)
        .await?;
        let dependencies_satisfied = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT NOT EXISTS (
                SELECT 1
                FROM jsonb_array_elements_text($3::jsonb) dependency(step_key)
                WHERE NOT EXISTS (
                    SELECT 1 FROM production_steps completed
                    WHERE completed.run_id = $1 AND completed.revision_epoch = $2
                      AND completed.step_key = dependency.step_key
                      AND completed.status = 'succeeded'
                )
            )
            "#,
        )
        .bind(current.run_id)
        .bind(current.revision_epoch)
        .bind(&current.dependencies)
        .fetch_one(&mut *tx)
        .await?;
        if !run_claimable || !dependencies_satisfied {
            return Err(ProductionError::TransitionConflict {
                reason: "step is outside the current runnable plan or has unmet dependencies"
                    .into(),
            });
        }
        if current.status == "queued" {
            let command_kind = if matches!(current.step_type.as_str(), "gate" | "external_wait") {
                WorkflowCommandKind::EnterWait
            } else {
                WorkflowCommandKind::ExecuteStep
            };
            validate_persisted_workflow_command(
                &mut tx,
                current.run_id,
                idempotency_key,
                WorkflowCommand::step(command_kind, &current.step_key),
            )
            .await?;
        }
        if current.status == "running"
            && expired
            && matches!(current.side_effect_state.as_str(), "submitted" | "unknown")
        {
            sqlx::query(
                r#"
                UPDATE production_step_attempts
                SET status = 'attention_required', side_effect_state = $3,
                    error_details = jsonb_build_object('code', 'unknown_external_result'),
                    completed_at = NOW()
                WHERE step_id = $1 AND attempt_no = $2
                  AND status IN ('prepared', 'running')
                "#,
            )
            .bind(step_id)
            .bind(current.attempt)
            .bind(&current.side_effect_state)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE production_steps SET status = 'attention_required', lease_owner = NULL, lease_expires_at = NULL, error_code = 'unknown_external_result', updated_at = NOW() WHERE id = $1",
            )
            .bind(step_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Err(ProductionError::AttentionRequired);
        }
        let claimable = current.status == "queued"
            || (current.status == "running"
                && expired
                && matches!(current.side_effect_state.as_str(), "none" | "prepared"));
        if !claimable {
            return Err(ProductionError::TransitionConflict {
                reason: "step is not claimable or already has a live lease".into(),
            });
        }
        if current.status == "running" && expired {
            sqlx::query(
                r#"
                UPDATE production_step_attempts
                SET status = 'failed',
                    error_details = jsonb_build_object('code', 'lease_expired_before_side_effect'),
                    completed_at = NOW()
                WHERE step_id = $1 AND attempt_no = $2
                  AND status IN ('prepared', 'running')
                "#,
            )
            .bind(step_id)
            .bind(current.attempt)
            .execute(&mut *tx)
            .await?;
        }
        let next_attempt = current.attempt + 1;
        let lease_expires_at = Utc::now() + ttl;
        let claimed = sqlx::query_as::<_, ProductionStepRecord>(
            r#"
            UPDATE production_steps
            SET status = 'running', attempt = $2, lease_owner = $3,
                lease_expires_at = $4, started_at = COALESCE(started_at, NOW()), updated_at = NOW()
            WHERE id = $1
            RETURNING id, run_id, revision_epoch, plan_order, step_key, step_type, role_key,
                      dependencies, status, waiting_reason, error_code, error_details, retryable, attempt,
                      lease_owner, lease_expires_at, side_effect_state,
                      agent_run_id, model_call_id, context_snapshot_id
            "#,
        )
        .bind(step_id)
        .bind(next_attempt)
        .bind(lease_owner)
        .bind(lease_expires_at)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO production_step_attempts (
                step_id, attempt_no, status, request_digest, idempotency_key,
                lease_owner, side_effect_state
            ) VALUES ($1, $2, 'running', $3, $4, $5, $6)
            "#,
        )
        .bind(step_id)
        .bind(next_attempt)
        .bind(request_digest)
        .bind(idempotency_key)
        .bind(lease_owner)
        .bind(&current.side_effect_state)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(claimed)
    }

    pub async fn renew_step_lease(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
        lease_ttl: Duration,
    ) -> ProductionResult<ProductionStepRecord> {
        let ttl = validated_lease_ttl(lease_owner, attempt, lease_ttl)?;
        let lease_expires_at = Utc::now() + ttl;
        sqlx::query_as::<_, ProductionStepRecord>(
            r#"
            UPDATE production_steps
            SET lease_expires_at = $4, updated_at = NOW()
            WHERE id = $1 AND status = 'running' AND attempt = $2
              AND lease_owner = $3 AND lease_expires_at >= NOW()
            RETURNING id, run_id, revision_epoch, plan_order, step_key, step_type, role_key,
                      dependencies, status, waiting_reason, error_code, error_details, retryable, attempt,
                      lease_owner, lease_expires_at, side_effect_state,
                      agent_run_id, model_call_id, context_snapshot_id
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .bind(lease_owner)
        .bind(lease_expires_at)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "only the current owner may renew a live step lease".into(),
        })
    }

    pub async fn release_step_lease(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
    ) -> ProductionResult<()> {
        if lease_owner.trim().is_empty() || attempt <= 0 {
            return Err(ProductionError::TransitionConflict {
                reason: "lease owner and positive attempt are required".into(),
            });
        }
        let mut tx = self.pool.begin().await?;
        let side_effect_state = sqlx::query_scalar::<_, String>(
            r#"
            SELECT side_effect_state FROM production_steps
            WHERE id = $1 AND status = 'running' AND attempt = $2 AND lease_owner = $3
            FOR UPDATE
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .bind(lease_owner)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "only the current owner may release a step lease".into(),
        })?;
        if !matches!(side_effect_state.as_str(), "none" | "prepared") {
            return Err(ProductionError::AttentionRequired);
        }
        sqlx::query(
            r#"
            UPDATE production_step_attempts
            SET status = 'cancelled', completed_at = NOW()
            WHERE step_id = $1 AND attempt_no = $2
              AND lease_owner = $3 AND status IN ('prepared', 'running')
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .bind(lease_owner)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_resource_reservations
            SET status = 'released', settled_at = NOW()
            WHERE step_id = $1 AND attempt_no = $2 AND status = 'reserved'
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status = 'queued', lease_owner = NULL, lease_expires_at = NULL,
                side_effect_state = 'none', updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// 原子完成一个无外部副作用的 domain command step。
    ///
    /// Runner 只能通过该方法推进系统命令；步骤、attempt、后继解锁和 Run
    /// 状态在同一事务内提交，避免 worker 用裸 SQL 越过状态机。
    pub async fn complete_domain_step(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
        output_digest: &str,
    ) -> ProductionResult<()> {
        validate_digest(output_digest)?;
        let mut tx = self.pool.begin().await?;
        let step = lock_owned_step(&mut tx, step_id, lease_owner, attempt).await?;
        if step.step_type != "domain_command"
            || !matches!(step.side_effect_state.as_str(), "none" | "prepared")
        {
            return Err(ProductionError::TransitionConflict {
                reason: "only a side-effect-free domain command may be completed by Runner".into(),
            });
        }
        sqlx::query(
            r#"
            UPDATE production_step_attempts
            SET status='succeeded', side_effect_state='confirmed', result=$4, completed_at=NOW()
            WHERE step_id=$1 AND attempt_no=$2 AND lease_owner=$3
              AND status IN ('running','prepared')
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .bind(lease_owner)
        .bind(json!({"output_digest": output_digest}))
        .execute(&mut *tx)
        .await?;
        let updated = sqlx::query(
            r#"
            UPDATE production_steps
            SET status='succeeded', side_effect_state='confirmed', output_digest=$2,
                waiting_reason=NULL, error_code=NULL, error_details=NULL,
                lease_owner=NULL, lease_expires_at=NULL, completed_at=NOW(), updated_at=NOW()
            WHERE id=$1 AND status='running' AND attempt=$3 AND lease_owner=$4
            "#,
        )
        .bind(step_id)
        .bind(output_digest)
        .bind(attempt)
        .bind(lease_owner)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(ProductionError::TransitionConflict {
                reason: "domain command lease is no longer current".into(),
            });
        }
        unlock_ready_steps(&mut tx, step.run_id, step.revision_epoch).await?;
        sqlx::query(
            "UPDATE production_runs SET status='queued', error_code=NULL, error_details=NULL, updated_at=NOW() WHERE id=$1 AND status IN ('queued','running')",
        )
        .bind(step.run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// 将需要操作者或外部系统的步骤变成可恢复的等待状态，并释放当前 lease。
    pub async fn mark_step_external_wait(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
        waiting_reason: &str,
        error_code: &str,
    ) -> ProductionResult<()> {
        if waiting_reason.trim().is_empty() || error_code.trim().is_empty() {
            return Err(ProductionError::TransitionConflict {
                reason: "external wait reason and error code are required".into(),
            });
        }
        let mut tx = self.pool.begin().await?;
        let step = lock_owned_step(&mut tx, step_id, lease_owner, attempt).await?;
        if !matches!(step.side_effect_state.as_str(), "none" | "prepared") {
            return Err(ProductionError::AttentionRequired);
        }
        sqlx::query(
            r#"
            UPDATE production_step_attempts
            SET status='cancelled', completed_at=NOW()
            WHERE step_id=$1 AND attempt_no=$2 AND status IN ('running','prepared')
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status='external_wait', waiting_reason=$2, error_code=$3,
                lease_owner=NULL, lease_expires_at=NULL, updated_at=NOW()
            WHERE id=$1 AND status='running' AND attempt=$4
            "#,
        )
        .bind(step_id)
        .bind(waiting_reason)
        .bind(error_code)
        .bind(attempt)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE production_runs SET status='external_wait', error_code=NULL, error_details=NULL, updated_at=NOW() WHERE id=$1 AND status IN ('queued','running')",
        )
        .bind(step.run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn reserve_resources(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
        request: ResourceRequest,
        request_digest: &str,
    ) -> ProductionResult<Vec<PersistedResourceReservation>> {
        validate_digest(request_digest)?;
        if request.values.is_empty() {
            return Err(ProductionError::TransitionConflict {
                reason: "resource request must not be empty".into(),
            });
        }
        let mut tx = self.pool.begin().await?;
        let step = lock_owned_step(&mut tx, step_id, lease_owner, attempt).await?;
        if !matches!(step.side_effect_state.as_str(), "none" | "prepared") {
            return Err(ProductionError::AttentionRequired);
        }
        let limits_value = sqlx::query_scalar::<_, Value>(
            "SELECT resource_limits FROM production_runs WHERE id = $1 FOR UPDATE",
        )
        .bind(step.run_id)
        .fetch_one(&mut *tx)
        .await?;
        let limits: ResourceLimits = serde_json::from_value(limits_value)?;
        let all_existing = load_resource_reservations(&mut tx, step_id, attempt).await?;
        let existing = all_existing
            .iter()
            .filter(|item| item.request_digest == request_digest)
            .cloned()
            .collect::<Vec<_>>();
        if !existing.is_empty() {
            let existing_values: BTreeMap<_, _> = existing
                .iter()
                .map(|item| (item.resource_key.clone(), item.reserved_value as u64))
                .collect();
            if existing_values == request.values
                && existing
                    .iter()
                    .all(|item| item.request_digest == request_digest)
            {
                tx.commit().await?;
                return Ok(existing);
            }
            return Err(ProductionError::IdempotencyConflict);
        }
        if all_existing
            .iter()
            .any(|item| request.values.contains_key(&item.resource_key))
        {
            return Err(ProductionError::IdempotencyConflict);
        }

        for (resource_key, requested) in &request.values {
            let limit =
                limits
                    .value(resource_key)
                    .ok_or_else(|| ProductionError::TransitionConflict {
                        reason: format!("unknown resource key: {resource_key}"),
                    })?;
            let current = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COALESCE(SUM(
                    CASE
                        WHEN $2 = 'concurrency' AND status IN ('reserved', 'held_uncertain')
                            THEN reserved_value
                        WHEN $2 = 'concurrency' THEN 0
                        WHEN status = 'settled' THEN actual_value
                        WHEN status IN ('reserved', 'held_uncertain') THEN reserved_value
                        ELSE 0
                    END
                ), 0)::bigint
                FROM production_resource_reservations
                WHERE run_id = $1 AND resource_key = $2
                "#,
            )
            .bind(step.run_id)
            .bind(resource_key)
            .fetch_one(&mut *tx)
            .await? as u64;
            if current.saturating_add(*requested) > limit {
                return Err(ProductionError::ResourceLimit {
                    resource: resource_key.clone(),
                    current,
                    requested: *requested,
                    limit,
                });
            }
            i64::try_from(*requested).map_err(|_| ProductionError::ResourceLimit {
                resource: resource_key.clone(),
                current,
                requested: *requested,
                limit,
            })?;
        }

        let mut reservations = Vec::with_capacity(request.values.len());
        for (resource_key, requested) in request.values {
            let reservation = sqlx::query_as::<_, PersistedResourceReservation>(
                r#"
                INSERT INTO production_resource_reservations (
                    run_id, step_id, attempt_no, resource_key, reserved_value, request_digest
                ) VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id, run_id, step_id, attempt_no, resource_key, reserved_value,
                          actual_value, status, request_digest, created_at, settled_at
                "#,
            )
            .bind(step.run_id)
            .bind(step_id)
            .bind(attempt)
            .bind(resource_key)
            .bind(requested as i64)
            .bind(request_digest)
            .fetch_one(&mut *tx)
            .await?;
            reservations.push(reservation);
        }
        sqlx::query(
            "UPDATE production_steps SET side_effect_state = 'prepared', updated_at = NOW() WHERE id = $1",
        )
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_step_attempts SET status = 'prepared', side_effect_state = 'prepared'
            WHERE step_id = $1 AND attempt_no = $2 AND status = 'running'
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(reservations)
    }

    /// 供同一 PostgreSQL 事务内的正式领域边界使用，不依赖 worker lease。
    pub async fn reserve_integrated_resources(
        tx: &mut Transaction<'_, Postgres>,
        run_id: Uuid,
        step_id: Uuid,
        attempt: i32,
        request: ResourceRequest,
        request_digest: &str,
    ) -> ProductionResult<Vec<PersistedResourceReservation>> {
        validate_digest(request_digest)?;
        if attempt <= 0 || request.values.is_empty() {
            return Err(ProductionError::TransitionConflict {
                reason: "integrated resource request requires a positive attempt and values".into(),
            });
        }
        let limits_value = sqlx::query_scalar::<_, Value>(
            r#"
            SELECT run.resource_limits
            FROM production_runs run
            JOIN production_steps step ON step.run_id=run.id
            WHERE run.id=$1 AND step.id=$2
            FOR UPDATE OF run,step
            "#,
        )
        .bind(run_id)
        .bind(step_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "integrated resource step does not belong to the ProductionRun".into(),
        })?;
        let limits: ResourceLimits = serde_json::from_value(limits_value)?;
        let all_existing = load_resource_reservations(tx, step_id, attempt).await?;
        let existing = all_existing
            .iter()
            .filter(|item| item.request_digest == request_digest)
            .cloned()
            .collect::<Vec<_>>();
        if !existing.is_empty() {
            let existing_values = existing
                .iter()
                .map(|item| (item.resource_key.clone(), item.reserved_value as u64))
                .collect::<BTreeMap<_, _>>();
            if existing_values == request.values {
                return Ok(existing);
            }
            return Err(ProductionError::IdempotencyConflict);
        }
        if all_existing
            .iter()
            .any(|item| request.values.contains_key(&item.resource_key))
        {
            return Err(ProductionError::IdempotencyConflict);
        }

        for (resource_key, requested) in &request.values {
            let limit =
                limits
                    .value(resource_key)
                    .ok_or_else(|| ProductionError::TransitionConflict {
                        reason: format!("unknown resource key: {resource_key}"),
                    })?;
            let current = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COALESCE(SUM(
                    CASE
                        WHEN $2 = 'concurrency' AND status IN ('reserved','held_uncertain')
                            THEN reserved_value
                        WHEN $2 = 'concurrency' THEN 0
                        WHEN status='settled' THEN actual_value
                        WHEN status IN ('reserved','held_uncertain') THEN reserved_value
                        ELSE 0
                    END
                ),0)::bigint
                FROM production_resource_reservations
                WHERE run_id=$1 AND resource_key=$2
                "#,
            )
            .bind(run_id)
            .bind(resource_key)
            .fetch_one(&mut **tx)
            .await? as u64;
            if current.saturating_add(*requested) > limit {
                return Err(ProductionError::ResourceLimit {
                    resource: resource_key.clone(),
                    current,
                    requested: *requested,
                    limit,
                });
            }
            i64::try_from(*requested).map_err(|_| ProductionError::ResourceLimit {
                resource: resource_key.clone(),
                current,
                requested: *requested,
                limit,
            })?;
        }

        let mut reservations = Vec::with_capacity(request.values.len());
        for (resource_key, requested) in request.values {
            reservations.push(
                sqlx::query_as::<_, PersistedResourceReservation>(
                    r#"
                    INSERT INTO production_resource_reservations (
                        run_id,step_id,attempt_no,resource_key,reserved_value,request_digest
                    ) VALUES ($1,$2,$3,$4,$5,$6)
                    RETURNING id,run_id,step_id,attempt_no,resource_key,reserved_value,
                              actual_value,status,request_digest,created_at,settled_at
                    "#,
                )
                .bind(run_id)
                .bind(step_id)
                .bind(attempt)
                .bind(resource_key)
                .bind(requested as i64)
                .bind(request_digest)
                .fetch_one(&mut **tx)
                .await?,
            );
        }
        Ok(reservations)
    }

    /// 将一个跨领域资源请求按可信实际值结算；并发容量在记录用量后释放。
    pub async fn settle_integrated_resources(
        tx: &mut Transaction<'_, Postgres>,
        run_id: Uuid,
        request_digest: &str,
        actual_values: BTreeMap<String, u64>,
        result_uncertain: bool,
    ) -> ProductionResult<()> {
        validate_digest(request_digest)?;
        let reservations = sqlx::query_as::<_, PersistedResourceReservation>(
            r#"
            SELECT id,run_id,step_id,attempt_no,resource_key,reserved_value,
                   actual_value,status,request_digest,created_at,settled_at
            FROM production_resource_reservations
            WHERE run_id=$1 AND request_digest=$2
            ORDER BY resource_key FOR UPDATE
            "#,
        )
        .bind(run_id)
        .bind(request_digest)
        .fetch_all(&mut **tx)
        .await?;
        if reservations.is_empty()
            || reservations.len() != actual_values.len()
            || reservations
                .iter()
                .any(|item| !actual_values.contains_key(&item.resource_key))
        {
            return Err(ProductionError::TransitionConflict {
                reason: "integrated actual usage must cover its exact reservation".into(),
            });
        }
        if result_uncertain {
            if reservations
                .iter()
                .all(|item| matches!(item.status.as_str(), "settled" | "released"))
            {
                return Ok(());
            }
            if reservations
                .iter()
                .any(|item| item.status == "held_uncertain")
            {
                return Ok(());
            }
            sqlx::query(
                "UPDATE production_resource_reservations SET status='held_uncertain' WHERE run_id=$1 AND request_digest=$2 AND status='reserved'",
            )
            .bind(run_id)
            .bind(request_digest)
            .execute(&mut **tx)
            .await?;
            return Ok(());
        }
        if reservations
            .iter()
            .any(|item| item.status == "held_uncertain")
        {
            return Err(ProductionError::AttentionRequired);
        }

        for reservation in reservations {
            if matches!(reservation.status.as_str(), "settled" | "released") {
                continue;
            }
            let actual = actual_values[&reservation.resource_key];
            let actual_i64 =
                i64::try_from(actual).map_err(|_| ProductionError::TransitionConflict {
                    reason: "integrated actual usage exceeds PostgreSQL range".into(),
                })?;
            if actual > 0 {
                let usage_digest = canonical_digest(&json!({
                    "request_digest": request_digest,
                    "reservation_id": reservation.id,
                    "resource_key": reservation.resource_key,
                    "actual": actual,
                }))?;
                sqlx::query(
                    r#"
                    INSERT INTO production_resource_usage (
                        run_id,step_id,reservation_id,resource_key,used_value,usage_digest
                    ) VALUES ($1,$2,$3,$4,$5,$6)
                    ON CONFLICT (run_id,resource_key,usage_digest) DO NOTHING
                    "#,
                )
                .bind(run_id)
                .bind(reservation.step_id)
                .bind(reservation.id)
                .bind(&reservation.resource_key)
                .bind(actual_i64)
                .bind(usage_digest)
                .execute(&mut **tx)
                .await?;
            }
            let status = if actual == 0 || reservation.resource_key == "concurrency" {
                "released"
            } else {
                "settled"
            };
            sqlx::query(
                "UPDATE production_resource_reservations SET actual_value=$2,status=$3,settled_at=NOW() WHERE id=$1 AND status='reserved'",
            )
            .bind(reservation.id)
            .bind(actual_i64)
            .bind(status)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }

    pub async fn release_integrated_resources(
        tx: &mut Transaction<'_, Postgres>,
        run_id: Uuid,
        request_digest: &str,
    ) -> ProductionResult<()> {
        validate_digest(request_digest)?;
        sqlx::query(
            "UPDATE production_resource_reservations SET status='released',settled_at=NOW() WHERE run_id=$1 AND request_digest=$2 AND status='reserved'",
        )
        .bind(run_id)
        .bind(request_digest)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn settle_resources(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
        actual_values: BTreeMap<String, u64>,
        usage_digest: &str,
        result_uncertain: bool,
    ) -> ProductionResult<()> {
        validate_digest(usage_digest)?;
        let mut tx = self.pool.begin().await?;
        let step = lock_owned_step(&mut tx, step_id, lease_owner, attempt).await?;
        let reservations = load_resource_reservations(&mut tx, step_id, attempt).await?;
        if reservations.is_empty() {
            return Err(ProductionError::TransitionConflict {
                reason: "resource reservation not found".into(),
            });
        }
        let reserved_keys: Vec<_> = reservations
            .iter()
            .map(|item| item.resource_key.clone())
            .collect();
        if actual_values.len() != reserved_keys.len()
            || reserved_keys
                .iter()
                .any(|key| !actual_values.contains_key(key))
        {
            return Err(ProductionError::TransitionConflict {
                reason: "actual usage must cover the exact reserved resource set".into(),
            });
        }
        if result_uncertain {
            sqlx::query(
                r#"
                UPDATE production_resource_reservations
                SET status = 'held_uncertain'
                WHERE step_id = $1 AND attempt_no = $2 AND status = 'reserved'
                "#,
            )
            .bind(step_id)
            .bind(attempt)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                UPDATE production_step_attempts
                SET status = 'attention_required', side_effect_state = 'unknown',
                    result = $3, completed_at = NOW()
                WHERE step_id = $1 AND attempt_no = $2
                  AND status IN ('prepared', 'running')
                "#,
            )
            .bind(step_id)
            .bind(attempt)
            .bind(json!({"actual_values": actual_values, "usage_digest": usage_digest}))
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                UPDATE production_steps
                SET status = 'attention_required', side_effect_state = 'unknown',
                    error_code = 'unknown_external_result', lease_owner = NULL,
                    lease_expires_at = NULL, updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(step_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(());
        }

        for reservation in reservations {
            if reservation.status != "reserved" {
                return Err(ProductionError::TransitionConflict {
                    reason: "resource reservation is already terminal".into(),
                });
            }
            let actual = actual_values[&reservation.resource_key];
            let actual =
                i64::try_from(actual).map_err(|_| ProductionError::TransitionConflict {
                    reason: "actual resource usage exceeds database range".into(),
                })?;
            sqlx::query(
                r#"
                UPDATE production_resource_reservations
                SET actual_value = $2, status = 'settled', settled_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(reservation.id)
            .bind(actual)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO production_resource_usage (
                    run_id, step_id, reservation_id, resource_key, used_value, usage_digest
                ) VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(step.run_id)
            .bind(step_id)
            .bind(reservation.id)
            .bind(&reservation.resource_key)
            .bind(actual)
            .bind(usage_digest)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            "UPDATE production_steps SET side_effect_state = 'confirmed', updated_at = NOW() WHERE id = $1",
        )
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_step_attempts SET status = 'running', side_effect_state = 'confirmed'
            WHERE step_id = $1 AND attempt_no = $2 AND status = 'prepared'
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn release_resources(
        &self,
        step_id: Uuid,
        lease_owner: &str,
        attempt: i32,
    ) -> ProductionResult<()> {
        let mut tx = self.pool.begin().await?;
        let step = lock_owned_step(&mut tx, step_id, lease_owner, attempt).await?;
        if !matches!(step.side_effect_state.as_str(), "none" | "prepared") {
            return Err(ProductionError::AttentionRequired);
        }
        sqlx::query(
            r#"
            UPDATE production_resource_reservations
            SET status = 'released', settled_at = NOW()
            WHERE step_id = $1 AND attempt_no = $2 AND status = 'reserved'
            "#,
        )
        .bind(step_id)
        .bind(attempt)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE production_steps SET side_effect_state = 'none', updated_at = NOW() WHERE id = $1",
        )
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn recoverable_steps(
        &self,
        limit: i64,
    ) -> ProductionResult<Vec<ProductionStepRecord>> {
        sqlx::query_as::<_, ProductionStepRecord>(
            r#"
            SELECT id, run_id, revision_epoch, plan_order, step_key, step_type, role_key,
                   dependencies, status, waiting_reason, error_code, error_details, retryable, attempt,
                   lease_owner, lease_expires_at, side_effect_state,
                   agent_run_id, model_call_id, context_snapshot_id
            FROM production_steps
            WHERE status = 'queued'
               OR (status = 'running' AND lease_expires_at < NOW() AND side_effect_state IN ('none', 'prepared'))
            ORDER BY created_at LIMIT $1
            "#,
        )
        .bind(limit.clamp(1, 1000))
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// 返回需要外部系统或操作者继续观察的当前步骤；等待状态本身不自动 claim。
    pub async fn external_wait_steps(
        &self,
        limit: i64,
    ) -> ProductionResult<Vec<ProductionStepRecord>> {
        sqlx::query_as::<_, ProductionStepRecord>(
            r#"
            SELECT id, run_id, revision_epoch, plan_order, step_key, step_type, role_key,
                   dependencies, status, waiting_reason, error_code, error_details, retryable, attempt,
                   lease_owner, lease_expires_at, side_effect_state,
                   agent_run_id, model_call_id, context_snapshot_id
            FROM production_steps
            WHERE status='external_wait'
            ORDER BY updated_at
            LIMIT $1
            "#,
        )
        .bind(limit.clamp(1, 1000))
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// 将一个已解锁但只等待操作者的步骤持久化为 external_wait。
    pub async fn mark_operator_wait(
        &self,
        step_id: Uuid,
        waiting_reason: &str,
        error_code: &str,
    ) -> ProductionResult<()> {
        if waiting_reason.trim().is_empty() || error_code.trim().is_empty() {
            return Err(ProductionError::TransitionConflict {
                reason: "operator wait reason and error code are required".into(),
            });
        }
        let mut tx = self.pool.begin().await?;
        let step = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT run_id,status FROM production_steps WHERE id=$1 FOR UPDATE",
        )
        .bind(step_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production step not found".into(),
        })?;
        if !matches!(step.1.as_str(), "queued" | "external_wait") {
            return Err(ProductionError::TransitionConflict {
                reason: "step is not waiting for operator input".into(),
            });
        }
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status='external_wait', waiting_reason=$2, error_code=$3,
                retryable=FALSE, lease_owner=NULL, lease_expires_at=NULL, updated_at=NOW()
            WHERE id=$1
            "#,
        )
        .bind(step_id)
        .bind(waiting_reason)
        .bind(error_code)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE production_runs SET status='external_wait', updated_at=NOW() WHERE id=$1 AND status IN ('queued','running')",
        )
        .bind(step.0)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn cancel_run(
        &self,
        run_id: Uuid,
        actor: ProductionActor,
        idempotency_key: &str,
        reason: &str,
    ) -> ProductionResult<ProductionRunRecord> {
        actor.validate()?;
        if reason.trim().is_empty() {
            return Err(ProductionError::TransitionConflict {
                reason: "cancel reason must not be blank".into(),
            });
        }
        let request_digest = ProductionCommandStore::canonical_request_digest(
            &json!({"run_id": run_id, "reason": reason}),
        )?;
        let command_scope = ProductionCommandScope::new(
            actor.clone(),
            ProductionCommandType::CancelRun,
            ProductionAggregateType::ProductionRun,
            run_id,
            idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        if ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest)
            .await?
            .is_some()
        {
            tx.commit().await?;
            return self.get_run_record(run_id).await;
        }

        let intent_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT production_project_id FROM production_runs WHERE id = $1 FOR UPDATE",
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production run not found".into(),
        })?;
        validate_persisted_workflow_command(
            &mut tx,
            run_id,
            idempotency_key,
            WorkflowCommand::run(WorkflowCommandKind::CancelRun),
        )
        .await?;
        let external_run_ids = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT link.work_generation_run_id
            FROM production_domain_links link
            JOIN work_generation_runs external
              ON external.id = link.work_generation_run_id
            WHERE link.run_id = $1 AND link.link_type = 'work_generation_run'
              AND external.status IN ('queued', 'running', 'cancelling', 'waiting_manual')
            ORDER BY link.created_at, link.id
            "#,
        )
        .bind(run_id)
        .fetch_all(&mut *tx)
        .await?;
        let uncertain = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM production_steps
                WHERE run_id = $1 AND status IN ('running', 'cancelling', 'attention_required')
                  AND side_effect_state IN ('submitted', 'unknown')
            )
            "#,
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        let cancellation_intent = json!({
            "requested_by": {"actor_type": actor.actor_type, "actor_id": actor.actor_id},
            "reason": reason,
            "requested_at": Utc::now(),
            "request_digest": request_digest,
            "external_run_ids": external_run_ids,
            "external_results": {},
        });

        let terminal_status = if uncertain {
            "attention_required"
        } else if !external_run_ids.is_empty() {
            "cancelling"
        } else {
            "cancelled"
        };
        sqlx::query(
            r#"
            UPDATE production_resource_reservations reservation
            SET status = 'released', settled_at = NOW()
            FROM production_steps step
            WHERE reservation.run_id = $1 AND reservation.status = 'reserved'
              AND step.id = reservation.step_id
              AND step.side_effect_state IN ('none', 'prepared')
            "#,
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_resource_reservations reservation
            SET status = 'held_uncertain'
            FROM production_steps step
            WHERE reservation.run_id = $1 AND reservation.status = 'reserved'
              AND step.id = reservation.step_id
              AND step.side_effect_state IN ('submitted', 'unknown')
            "#,
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status = 'cancelled', lease_owner = NULL, lease_expires_at = NULL,
                waiting_reason = NULL, completed_at = COALESCE(completed_at, NOW()),
                updated_at = NOW()
            WHERE run_id = $1
              AND status NOT IN ('succeeded', 'failed', 'cancelled', 'superseded')
              AND side_effect_state IN ('none', 'prepared', 'confirmed')
            "#,
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status = 'attention_required', waiting_reason = 'manual_attention',
                error_code = 'unknown_external_result', lease_owner = NULL,
                lease_expires_at = NULL, updated_at = NOW()
            WHERE run_id = $1
              AND status NOT IN ('succeeded', 'failed', 'cancelled', 'superseded')
              AND side_effect_state IN ('submitted', 'unknown')
            "#,
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        let run = sqlx::query_as::<_, ProductionRunRecord>(
            r#"
            UPDATE production_runs
            SET cancellation_intent = $2, status = $3,
                error_code = CASE WHEN $3 = 'attention_required' THEN 'unknown_external_result' ELSE error_code END,
                completed_at = CASE WHEN $3 = 'cancelled' THEN NOW() ELSE completed_at END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, production_project_id, plan_snapshot_id, status, quality_status,
                      current_revision_epoch, resource_limits, binding_snapshot, source_snapshot,
                      cancellation_intent, error_code, error_details, actor_type, actor_id,
                      created_at, updated_at
            "#,
        )
        .bind(run_id)
        .bind(&cancellation_intent)
        .bind(terminal_status)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("UPDATE production_projects SET status = $2, updated_at = NOW() WHERE id = $1")
            .bind(intent_id)
            .bind(terminal_status)
            .execute(&mut *tx)
            .await?;
        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            json!({"run_id": run_id, "status": terminal_status}),
        )
        .await?;
        tx.commit().await?;
        Ok(run)
    }

    pub async fn cancellation_context(
        &self,
        run_id: Uuid,
    ) -> ProductionResult<CancellationContext> {
        let run = self.get_run_record(run_id).await?;
        let intent = run.cancellation_intent.as_ref().ok_or_else(|| {
            ProductionError::TransitionConflict {
                reason: "production Run has no cancellation intent".into(),
            }
        })?;
        let external_run_ids = cancellation_external_run_ids(intent)?;
        let external_results = intent
            .get("external_results")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !external_results.is_object() {
            return Err(ProductionError::TransitionConflict {
                reason: "cancellation external results must be an object".into(),
            });
        }
        Ok(CancellationContext {
            run,
            external_run_ids,
            external_results,
        })
    }

    pub async fn fail_run(
        &self,
        run_id: Uuid,
        actor: ProductionActor,
        idempotency_key: &str,
        error_code: &str,
    ) -> ProductionResult<ProductionRunRecord> {
        actor.validate()?;
        validate_stable_error_code(error_code)?;
        let request_digest = ProductionCommandStore::canonical_request_digest(&json!({
            "run_id": run_id,
            "error_code": error_code,
        }))?;
        let command_scope = ProductionCommandScope::new(
            actor.clone(),
            ProductionCommandType::FailRun,
            ProductionAggregateType::ProductionRun,
            run_id,
            idempotency_key,
        );
        let mut tx = self.pool.begin().await?;
        if ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest)
            .await?
            .is_some()
        {
            tx.commit().await?;
            return self.get_run_record(run_id).await;
        }
        let (plan, workflow) = load_frozen_workflow(&mut tx, run_id).await?;
        plan.validate_frozen()?;
        let mut run_state = workflow.run;
        run_state.transition(RunStatus::Failed)?;
        let intent_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT production_project_id FROM production_runs WHERE id = $1 FOR UPDATE",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        let uncertain = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM production_steps
                WHERE run_id = $1 AND side_effect_state IN ('submitted', 'unknown')
                  AND status NOT IN ('succeeded', 'failed', 'cancelled', 'superseded')
            ) OR EXISTS(
                SELECT 1 FROM production_domain_links link
                JOIN work_generation_runs external
                  ON external.id = link.work_generation_run_id
                WHERE link.run_id = $1 AND link.link_type = 'work_generation_run'
                  AND external.status IN ('queued', 'running', 'cancelling', 'waiting_manual')
            )
            "#,
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        let status = if uncertain {
            "attention_required"
        } else {
            "failed"
        };
        sqlx::query(
            r#"
            UPDATE production_resource_reservations reservation
            SET status = CASE
                    WHEN step.side_effect_state IN ('none', 'prepared') THEN 'released'
                    ELSE 'held_uncertain'
                END,
                settled_at = CASE
                    WHEN step.side_effect_state IN ('none', 'prepared') THEN NOW()
                    ELSE settled_at
                END
            FROM production_steps step
            WHERE reservation.run_id = $1 AND reservation.status = 'reserved'
              AND step.id = reservation.step_id
            "#,
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status = CASE
                    WHEN side_effect_state IN ('submitted', 'unknown')
                        THEN 'attention_required'
                    ELSE 'cancelled'
                END,
                waiting_reason = CASE
                    WHEN side_effect_state IN ('submitted', 'unknown')
                        THEN 'manual_attention'
                    ELSE NULL
                END,
                lease_owner = NULL, lease_expires_at = NULL,
                completed_at = CASE
                    WHEN side_effect_state IN ('submitted', 'unknown')
                        THEN completed_at
                    ELSE COALESCE(completed_at, NOW())
                END,
                updated_at = NOW()
            WHERE run_id = $1
              AND status NOT IN ('succeeded', 'failed', 'cancelled', 'superseded')
            "#,
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        let run = sqlx::query_as::<_, ProductionRunRecord>(
            r#"
            UPDATE production_runs
            SET status = $2,
                error_code = CASE
                    WHEN $2 = 'attention_required' THEN 'unknown_external_result'
                    ELSE $3
                END,
                completed_at = CASE WHEN $2 = 'failed' THEN NOW() ELSE completed_at END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, production_project_id, plan_snapshot_id, status, quality_status,
                      current_revision_epoch, resource_limits, binding_snapshot, source_snapshot,
                      cancellation_intent, error_code, error_details, actor_type, actor_id,
                      created_at, updated_at
            "#,
        )
        .bind(run_id)
        .bind(status)
        .bind(error_code)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("UPDATE production_projects SET status = $2, updated_at = NOW() WHERE id = $1")
            .bind(intent_id)
            .bind(status)
            .execute(&mut *tx)
            .await?;
        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            json!({"run_id": run_id, "status": status}),
        )
        .await?;
        tx.commit().await?;
        Ok(run)
    }

    pub async fn reconcile_external_cancellation(
        &self,
        run_id: Uuid,
        external_run_id: Uuid,
        state: ExternalCancellationState,
        error_code: Option<&str>,
    ) -> ProductionResult<ProductionRunRecord> {
        if let Some(error_code) = error_code {
            validate_stable_error_code(error_code)?;
        }
        let mut tx = self.pool.begin().await?;
        let (intent_id, cancellation_intent) = sqlx::query_as::<_, (Uuid, Option<Value>)>(
            r#"
                SELECT production_project_id, cancellation_intent
                FROM production_runs WHERE id = $1 FOR UPDATE
                "#,
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production run not found".into(),
        })?;
        let mut intent =
            cancellation_intent.ok_or_else(|| ProductionError::TransitionConflict {
                reason: "external cancellation result requires a persisted cancellation intent"
                    .into(),
            })?;
        let required_ids = cancellation_external_run_ids(&intent)?;
        if !required_ids.contains(&external_run_id) {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkGenerationRun is outside the persisted cancellation intent".into(),
            });
        }
        let results = intent
            .get_mut("external_results")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| ProductionError::TransitionConflict {
                reason: "cancellation external results must be an object".into(),
            })?;
        let state_name = external_cancellation_state_name(state);
        results.insert(
            external_run_id.to_string(),
            json!({
                "status": state_name,
                "error_code": error_code,
                "observed_at": Utc::now(),
            }),
        );
        let has_uncertain = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM production_steps
                WHERE run_id = $1 AND side_effect_state IN ('submitted', 'unknown')
                  AND status = 'attention_required'
            ) OR EXISTS(
                SELECT 1 FROM production_resource_reservations
                WHERE run_id = $1 AND status = 'held_uncertain'
            )
            "#,
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        let result_object = intent
            .get("external_results")
            .and_then(Value::as_object)
            .expect("external_results was validated above");
        let any_attention = required_ids.iter().any(|id| {
            result_object
                .get(&id.to_string())
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
                == Some("attention_required")
        });
        let all_cancelled = required_ids.iter().all(|id| {
            result_object
                .get(&id.to_string())
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
                == Some("cancelled")
        });
        let status = if any_attention || (all_cancelled && has_uncertain) {
            "attention_required"
        } else if all_cancelled {
            "cancelled"
        } else {
            "cancelling"
        };
        let final_error_code = match status {
            "attention_required" if has_uncertain => Some("unknown_external_result"),
            "attention_required" => error_code.or(Some("external_cancellation_attention_required")),
            _ => None,
        };
        let run = sqlx::query_as::<_, ProductionRunRecord>(
            r#"
            UPDATE production_runs
            SET cancellation_intent = $2, status = $3, error_code = $4,
                completed_at = CASE WHEN $3 = 'cancelled' THEN NOW() ELSE completed_at END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, production_project_id, plan_snapshot_id, status, quality_status,
                      current_revision_epoch, resource_limits, binding_snapshot, source_snapshot,
                      cancellation_intent, error_code, error_details, actor_type, actor_id,
                      created_at, updated_at
            "#,
        )
        .bind(run_id)
        .bind(&intent)
        .bind(status)
        .bind(final_error_code)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("UPDATE production_projects SET status = $2, updated_at = NOW() WHERE id = $1")
            .bind(intent_id)
            .bind(status)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(run)
    }

    pub async fn enqueue_wakeup(&self, run_id: Uuid, step_id: Uuid) -> ProductionResult<()> {
        self.ensure_wakeup(run_id, step_id).await.map(|_| ())
    }

    async fn ensure_wakeup(&self, run_id: Uuid, step_id: Uuid) -> ProductionResult<bool> {
        let mut tx = self.pool.begin().await?;
        let step_exists = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM production_steps
            WHERE id = $2 AND run_id = $1 AND status IN ('queued', 'running')
            FOR UPDATE
            "#,
        )
        .bind(run_id)
        .bind(step_id)
        .fetch_optional(&mut *tx)
        .await?;
        if step_exists.is_none() {
            return Err(ProductionError::TransitionConflict {
                reason: "only a queued or recoverable running step may be awakened".into(),
            });
        }
        let changed = ensure_wakeup_in_transaction(&mut tx, run_id, step_id).await?;
        tx.commit().await?;
        Ok(changed)
    }

    pub async fn enqueue_recoverable_wakeups(&self, limit: i64) -> ProductionResult<u64> {
        let recoverable = self.recoverable_steps(limit).await?;
        let mut changed = 0;
        for step in recoverable {
            changed += u64::from(self.ensure_wakeup(step.run_id, step.id).await?);
        }
        Ok(changed)
    }

    pub async fn pending_wakeups(
        &self,
        limit: i64,
    ) -> ProductionResult<Vec<ProductionWakeupRecord>> {
        sqlx::query_as::<_, ProductionWakeupRecord>(
            r#"
            SELECT id, run_id, step_id, status, delivery_attempts, last_error,
                   created_at, delivered_at
            FROM production_wakeups
            WHERE status = 'pending'
            ORDER BY created_at
            LIMIT $1
            "#,
        )
        .bind(limit.clamp(1, 1000))
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn record_wakeup_delivery(
        &self,
        wakeup_id: Uuid,
        delivered: bool,
        error: Option<&str>,
    ) -> ProductionResult<()> {
        if delivered && error.is_some() {
            return Err(ProductionError::TransitionConflict {
                reason: "a delivered wakeup cannot also contain an error".into(),
            });
        }
        let updated = sqlx::query(
            r#"
            UPDATE production_wakeups
            SET status = CASE WHEN $2 THEN 'delivered' ELSE 'pending' END,
                delivery_attempts = delivery_attempts + 1,
                last_error = CASE WHEN $2 THEN NULL ELSE $3 END,
                delivered_at = CASE WHEN $2 THEN NOW() ELSE NULL END
            WHERE id = $1 AND status = 'pending'
            "#,
        )
        .bind(wakeup_id)
        .bind(delivered)
        .bind(error)
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(ProductionError::TransitionConflict {
                reason: "wakeup is missing or already delivered".into(),
            });
        }
        Ok(())
    }
}

struct SanitizedModelCallFinish {
    status: &'static str,
    output_snapshot: Option<Value>,
    usage_snapshot: Option<Value>,
    error_snapshot: Option<Value>,
    structured_parse_status: Option<String>,
}

fn validate_role_finalize_command(command: &RoleFinalizeCommand) -> ProductionResult<()> {
    if command.attempt <= 0
        || command.revision_epoch < 0
        || command.role_key.trim().is_empty()
        || command.model_call_finish.id != command.model_call_id
    {
        return Err(ProductionError::TransitionConflict {
            reason: "role finalize identity is incomplete".into(),
        });
    }
    match &command.failure {
        Some(failure) => {
            validate_stable_error_code(&failure.code)?;
            if failure.message.trim().is_empty()
                || command.validated_output.is_some()
                || command.output_digest.is_some()
                || command.model_call_finish.status != AuditedTerminalStatus::Failed
            {
                return Err(ProductionError::TransitionConflict {
                    reason: "failed role finalize contains a success payload".into(),
                });
            }
        }
        None => {
            let digest = command.output_digest.as_deref().ok_or_else(|| {
                ProductionError::TransitionConflict {
                    reason: "successful role finalize requires an output digest".into(),
                }
            })?;
            validate_digest(digest)?;
            if command.output.is_none()
                || command.validated_output.is_none()
                || command.model_call_finish.status != AuditedTerminalStatus::Succeeded
            {
                return Err(ProductionError::TransitionConflict {
                    reason: "successful role finalize requires typed output and success audit"
                        .into(),
                });
            }
            let validated = command.validated_output.as_ref().ok_or_else(|| {
                ProductionError::TransitionConflict {
                    reason: "successful role finalize requires typed output".into(),
                }
            })?;
            if validated_role_key(validated) != command.role_key {
                return Err(ProductionError::TransitionConflict {
                    reason: "typed role output does not match the prepared role".into(),
                });
            }
        }
    }
    Ok(())
}

fn validated_role_key(output: &ValidatedRoleOutput) -> &'static str {
    match output {
        ValidatedRoleOutput::Producer(_) => "producer",
        ValidatedRoleOutput::Screenwriter(_) => "screenwriter",
        ValidatedRoleOutput::Director(_) => "director",
        ValidatedRoleOutput::Cinematographer(_) => "cinematographer",
        ValidatedRoleOutput::PerformanceDirector(_) => "performance_director",
        ValidatedRoleOutput::SoundDirector(_) => "sound_director",
        ValidatedRoleOutput::Editor(_) => "editor",
        ValidatedRoleOutput::Qc(_) => "qc",
        ValidatedRoleOutput::CharacterCritic(_) => "character_critic",
    }
}

fn sanitize_model_call_finish(
    input: &FinishAuditedCall,
) -> ProductionResult<SanitizedModelCallFinish> {
    let sanitize = |value: &Option<Value>| -> ProductionResult<Option<Value>> {
        value
            .as_ref()
            .map(|value| {
                let redacted = redact_audit_value(value, &input.known_secrets);
                validate_audit_payload(&redacted).map_err(|error| {
                    ProductionError::TransitionConflict {
                        reason: format!("unsafe ModelCall terminal audit: {error}"),
                    }
                })?;
                Ok(redacted)
            })
            .transpose()
    };
    Ok(SanitizedModelCallFinish {
        status: match input.status {
            AuditedTerminalStatus::Succeeded => "succeeded",
            AuditedTerminalStatus::Failed => "failed",
            AuditedTerminalStatus::Aborted => "aborted",
        },
        output_snapshot: sanitize(&input.output_snapshot)?,
        usage_snapshot: sanitize(&input.usage_snapshot)?,
        error_snapshot: sanitize(&input.error_snapshot)?,
        structured_parse_status: input.structured_parse_status.clone(),
    })
}

async fn finish_prepared_model_call(
    tx: &mut Transaction<'_, Postgres>,
    model_call_id: Uuid,
    finish: &SanitizedModelCallFinish,
) -> ProductionResult<()> {
    let updated = sqlx::query(
        r#"
        UPDATE model_calls
        SET status=$2, output_snapshot=$3, usage_snapshot=$4, error_snapshot=$5,
            structured_parse_status=$6, completed_at=NOW()
        WHERE id=$1 AND status='prepared'
        "#,
    )
    .bind(model_call_id)
    .bind(finish.status)
    .bind(&finish.output_snapshot)
    .bind(&finish.usage_snapshot)
    .bind(&finish.error_snapshot)
    .bind(&finish.structured_parse_status)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(ProductionError::TransitionConflict {
            reason: "prepared ModelCall was already finalized".into(),
        });
    }
    Ok(())
}

async fn settle_role_resources(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
) -> ProductionResult<()> {
    let reservations = load_resource_reservations(tx, command.step_id, command.attempt)
        .await?
        .into_iter()
        .filter(|item| {
            matches!(
                item.resource_key.as_str(),
                "role_calls" | "input_tokens" | "output_tokens"
            )
        })
        .collect::<Vec<_>>();
    if reservations.len() != 3 || reservations.iter().any(|item| item.status != "reserved") {
        return Err(ProductionError::TransitionConflict {
            reason: "role finalize requires the exact active resource reservations".into(),
        });
    }
    if command
        .failure
        .as_ref()
        .is_some_and(|failure| failure.result_uncertain)
    {
        sqlx::query(
            "UPDATE production_resource_reservations SET status='held_uncertain' WHERE step_id=$1 AND attempt_no=$2 AND status='reserved'",
        )
        .bind(command.step_id)
        .bind(command.attempt)
        .execute(&mut **tx)
        .await?;
        return Ok(());
    }
    for reservation in reservations {
        let actual = match reservation.resource_key.as_str() {
            "role_calls" => 1,
            "output_tokens" => reservation.reserved_value.min(command.output_tokens as i64),
            _ => reservation.reserved_value,
        };
        settle_one_role_reservation(tx, &reservation, actual).await?;
    }
    Ok(())
}

async fn settle_reserved_resources_after_database_failure(
    tx: &mut Transaction<'_, Postgres>,
    step_id: Uuid,
    attempt: i32,
) -> ProductionResult<()> {
    for reservation in load_resource_reservations(tx, step_id, attempt).await? {
        if reservation.status == "reserved" {
            settle_one_role_reservation(tx, &reservation, reservation.reserved_value).await?;
        }
    }
    Ok(())
}

async fn settle_one_role_reservation(
    tx: &mut Transaction<'_, Postgres>,
    reservation: &PersistedResourceReservation,
    actual: i64,
) -> ProductionResult<()> {
    let usage_digest = canonical_digest(&json!({
        "reservation_id": reservation.id,
        "resource_key": reservation.resource_key,
        "actual": actual,
    }))?;
    sqlx::query(
        r#"
        INSERT INTO production_resource_usage (
            run_id, step_id, reservation_id, resource_key, used_value, usage_digest
        ) VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (run_id, resource_key, usage_digest) DO NOTHING
        "#,
    )
    .bind(reservation.run_id)
    .bind(reservation.step_id)
    .bind(reservation.id)
    .bind(&reservation.resource_key)
    .bind(actual)
    .bind(usage_digest)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE production_resource_reservations SET actual_value=$2, status='settled', settled_at=NOW() WHERE id=$1 AND status='reserved'",
    )
    .bind(reservation.id)
    .bind(actual)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn persist_validated_role_output(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    output: &ValidatedRoleOutput,
) -> ProductionResult<Vec<PersistedRoleArtifact>> {
    use crate::state::artifacts::output_contract::ValidatedRoleOutput::*;

    let mut artifacts = Vec::new();
    match output {
        Producer(value) => artifacts.push(
            insert_simple_process_artifact(
                tx,
                command,
                "creative_briefs",
                "creative_brief",
                serde_json::to_value(&value.creative_brief)?,
            )
            .await?,
        ),
        Screenwriter(value) => {
            artifacts.push(
                insert_simple_process_artifact(
                    tx,
                    command,
                    "story_bibles",
                    "story_bible",
                    serde_json::to_value(&value.story_bible)?,
                )
                .await?,
            );
            for character in &value.character_bibles {
                artifacts.push(insert_character_bible(tx, command, character).await?);
            }
            artifacts.push(
                insert_simple_process_artifact(
                    tx,
                    command,
                    "script_drafts",
                    "script_draft",
                    serde_json::to_value(&value.script_draft)?,
                )
                .await?,
            );
        }
        Director(value) => {
            artifacts.push(
                insert_simple_process_artifact(
                    tx,
                    command,
                    "directorial_treatments",
                    "directorial_treatment",
                    serde_json::to_value(&value.directorial_treatment)?,
                )
                .await?,
            );
            for shot in &value.shot_contracts {
                artifacts.push(insert_shot_contract(tx, command, shot).await?);
            }
        }
        Cinematographer(value) | CharacterCritic(value) => {
            persist_collaboration_suggestions(tx, command, value).await?;
        }
        PerformanceDirector(value) => {
            for brief in &value.performance_briefs {
                artifacts.push(insert_performance_brief(tx, command, brief).await?);
            }
        }
        SoundDirector(value) => {
            artifacts.push(insert_sound_plan(tx, command, &value.sound_plan).await?);
        }
        Editor(value) => {
            validate_continuity_ledger_output_scope(tx, command, value).await?;
            for ledger in &value.continuity_ledgers {
                artifacts.push(insert_continuity_ledger(tx, command, ledger).await?);
            }
        }
        Qc(value) => {
            let ledger_versions = validate_take_review_output_scope(tx, command, value).await?;
            for review in &value.take_reviews {
                let mappings = ledger_versions
                    .get(&review.required_take_id)
                    .ok_or_else(|| quality_package_blocker("take_review_ledger_mapping_missing"))?;
                artifacts.push(insert_take_review(tx, command, review, mappings).await?);
            }
        }
    }
    Ok(artifacts)
}

async fn next_process_artifact_version(
    tx: &mut Transaction<'_, Postgres>,
    table: &'static str,
    production_project_id: Uuid,
    identity_column: Option<(&'static str, &str)>,
) -> ProductionResult<i32> {
    let sql = if let Some((column, _)) = identity_column {
        format!(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM {table} WHERE production_project_id=$1 AND {column}=$2"
        )
    } else {
        format!("SELECT COALESCE(MAX(version), 0) + 1 FROM {table} WHERE production_project_id=$1")
    };
    let mut query = sqlx::query_scalar::<_, i32>(&sql).bind(production_project_id);
    if let Some((_, identity)) = identity_column {
        query = query.bind(identity);
    }
    query.fetch_one(&mut **tx).await.map_err(Into::into)
}

async fn insert_simple_process_artifact(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    table: &'static str,
    artifact_type: &'static str,
    content: Value,
) -> ProductionResult<PersistedRoleArtifact> {
    let version =
        next_process_artifact_version(tx, table, command.production_project_id, None).await?;
    let id = Uuid::new_v4();
    let digest = canonical_digest(&content)?;
    let sql = format!(
        "INSERT INTO {table} (id, production_project_id, version, status, content, created_by, run_id, step_id, attempt, revision_epoch, content_digest, audit_status) VALUES ($1,$2,$3,'draft',$4,$5,$6,$7,$8,$9,$10,'complete')"
    );
    sqlx::query(&sql)
        .bind(id)
        .bind(command.production_project_id)
        .bind(version)
        .bind(content)
        .bind(&command.role_key)
        .bind(command.run_id)
        .bind(command.step_id)
        .bind(command.attempt)
        .bind(command.revision_epoch)
        .bind(digest)
        .execute(&mut **tx)
        .await?;
    Ok(PersistedRoleArtifact {
        artifact_type: artifact_type.into(),
        id,
        version,
        character_id: None,
        shot_id: None,
    })
}

async fn insert_character_bible(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    character: &crate::state::artifacts::output_contract::CharacterBibleOutput,
) -> ProductionResult<PersistedRoleArtifact> {
    let version = next_process_artifact_version(
        tx,
        "character_bibles",
        command.production_project_id,
        Some(("character_id", &character.character_id)),
    )
    .await?;
    let id = Uuid::new_v4();
    let content = serde_json::to_value(character)?;
    let digest = canonical_digest(&content)?;
    sqlx::query(
        r#"
        INSERT INTO character_bibles (
            id, production_project_id, character_id, version, status, content, created_by,
            run_id, step_id, attempt, revision_epoch, content_digest, audit_status
        ) VALUES ($1,$2,$3,$4,'draft',$5,$6,$7,$8,$9,$10,$11,'complete')
        "#,
    )
    .bind(id)
    .bind(command.production_project_id)
    .bind(&character.character_id)
    .bind(version)
    .bind(content)
    .bind(&command.role_key)
    .bind(command.run_id)
    .bind(command.step_id)
    .bind(command.attempt)
    .bind(command.revision_epoch)
    .bind(digest)
    .execute(&mut **tx)
    .await?;
    Ok(PersistedRoleArtifact {
        artifact_type: "character_bible".into(),
        id,
        version,
        character_id: Some(character.character_id.clone()),
        shot_id: None,
    })
}

async fn insert_shot_contract(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    shot: &crate::state::artifacts::output_contract::ShotContractOutput,
) -> ProductionResult<PersistedRoleArtifact> {
    let version = next_process_artifact_version(
        tx,
        "shot_contracts",
        command.production_project_id,
        Some(("shot_id", &shot.shot_id)),
    )
    .await?;
    let id = Uuid::new_v4();
    let content = serde_json::to_value(shot)?;
    let digest = canonical_digest(&content)?;
    sqlx::query(
        r#"
        INSERT INTO shot_contracts (
            id, production_project_id, shot_id, scene_id, domain_scene_id, version,
            status, content, created_by, run_id, step_id, attempt, revision_epoch,
            content_digest, audit_status
        ) VALUES ($1,$2,$3,$4,$5,$6,'draft',$7,$8,$9,$10,$11,$12,$13,'complete')
        "#,
    )
    .bind(id)
    .bind(command.production_project_id)
    .bind(&shot.shot_id)
    .bind(shot.scene_id.to_string())
    .bind(shot.scene_id)
    .bind(version)
    .bind(content)
    .bind(&command.role_key)
    .bind(command.run_id)
    .bind(command.step_id)
    .bind(command.attempt)
    .bind(command.revision_epoch)
    .bind(digest)
    .execute(&mut **tx)
    .await?;
    Ok(PersistedRoleArtifact {
        artifact_type: "shot_contract".into(),
        id,
        version,
        character_id: None,
        shot_id: Some(shot.shot_id.clone()),
    })
}

async fn insert_performance_brief(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    brief: &crate::state::artifacts::output_contract::PerformanceBriefOutput,
) -> ProductionResult<PersistedRoleArtifact> {
    let version = next_process_artifact_version(
        tx,
        "performance_briefs",
        command.production_project_id,
        Some(("character_id", &brief.character_id)),
    )
    .await?;
    let id = Uuid::new_v4();
    let content = serde_json::to_value(brief)?;
    let digest = canonical_digest(&content)?;
    sqlx::query(
        r#"
        INSERT INTO performance_briefs (
            id, production_project_id, character_id, character_bible_id, script_id,
            version, status, content, created_by, run_id, step_id, attempt,
            revision_epoch, content_digest, audit_status
        ) VALUES ($1,$2,$3,$4,$5,$6,'draft',$7,$8,$9,$10,$11,$12,$13,'complete')
        "#,
    )
    .bind(id)
    .bind(command.production_project_id)
    .bind(&brief.character_id)
    .bind(brief.character_bible_id)
    .bind(brief.script_id)
    .bind(version)
    .bind(content)
    .bind(&command.role_key)
    .bind(command.run_id)
    .bind(command.step_id)
    .bind(command.attempt)
    .bind(command.revision_epoch)
    .bind(digest)
    .execute(&mut **tx)
    .await?;
    Ok(PersistedRoleArtifact {
        artifact_type: "performance_brief".into(),
        id,
        version,
        character_id: Some(brief.character_id.clone()),
        shot_id: None,
    })
}

async fn insert_sound_plan(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    plan: &crate::state::artifacts::output_contract::SoundPlanOutput,
) -> ProductionResult<PersistedRoleArtifact> {
    let version =
        next_process_artifact_version(tx, "sound_plans", command.production_project_id, None)
            .await?;
    let id = Uuid::new_v4();
    let content = serde_json::to_value(plan)?;
    let digest = canonical_digest(&content)?;
    sqlx::query(
        r#"
        INSERT INTO sound_plans (
            id, production_project_id, script_id, version, status, content, created_by,
            run_id, step_id, attempt, revision_epoch, content_digest, audit_status
        ) VALUES ($1,$2,$3,$4,'draft',$5,$6,$7,$8,$9,$10,$11,'complete')
        "#,
    )
    .bind(id)
    .bind(command.production_project_id)
    .bind(plan.script_id)
    .bind(version)
    .bind(content)
    .bind(&command.role_key)
    .bind(command.run_id)
    .bind(command.step_id)
    .bind(command.attempt)
    .bind(command.revision_epoch)
    .bind(digest)
    .execute(&mut **tx)
    .await?;
    Ok(PersistedRoleArtifact {
        artifact_type: "sound_plan".into(),
        id,
        version,
        character_id: None,
        shot_id: None,
    })
}

async fn persist_collaboration_suggestions(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    output: &crate::state::artifacts::output_contract::CollaborationRoleOutput,
) -> ProductionResult<()> {
    for suggestion in &output.collaboration_suggestions {
        let target = command
            .input_packages
            .iter()
            .flat_map(|package| &package.items)
            .find(|item| {
                item.artifact_id == suggestion.target_artifact_id
                    && item.artifact_version == suggestion.target_artifact_version as i32
            })
            .ok_or_else(|| ProductionError::InvalidArtifactSchema {
                details: format!(
                    "suggestion target {} v{} is not in the exact input package",
                    suggestion.target_artifact_id, suggestion.target_artifact_version
                ),
            })?;
        let to_role = artifact_owner(&target.artifact_type)?;
        let content = json!({
            "content": suggestion.content,
            "priority": suggestion.priority,
            "rationale": suggestion.rationale,
        });
        sqlx::query(
            r#"
            INSERT INTO collaboration_suggestions (
                production_project_id, from_role, to_role, artifact_type, artifact_id,
                suggestion_type, content, run_id, source_step_id, source_attempt,
                revision_epoch, source_model_call_id, target_artifact_version,
                target_content_digest, blocking, audit_status
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,'complete')
            "#,
        )
        .bind(command.production_project_id)
        .bind(&command.role_key)
        .bind(to_role)
        .bind(&target.artifact_type)
        .bind(target.artifact_id)
        .bind(&suggestion.suggestion_type)
        .bind(content)
        .bind(command.run_id)
        .bind(command.step_id)
        .bind(command.attempt)
        .bind(command.revision_epoch)
        .bind(command.model_call_id)
        .bind(target.artifact_version)
        .bind(&target.content_digest)
        .bind(suggestion.blocking)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn artifact_owner(artifact_type: &str) -> ProductionResult<&'static str> {
    match artifact_type {
        "creative_brief" => Ok("producer"),
        "story_bible" | "character_bible" | "script_draft" => Ok("screenwriter"),
        "directorial_treatment" | "shot_contract" => Ok("director"),
        "performance_brief" => Ok("performance_director"),
        "sound_plan" => Ok("sound_director"),
        "continuity_ledger" => Ok("editor"),
        "take_review" => Ok("qc"),
        _ => Err(ProductionError::InvalidArtifactSchema {
            details: format!("suggestion target has unknown artifact type {artifact_type}"),
        }),
    }
}

async fn validate_continuity_ledger_output_scope(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    output: &crate::state::artifacts::output_contract::EditorOutput,
) -> ProductionResult<()> {
    let media = load_media_review_input(tx, command.run_id, command.revision_epoch).await?;
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM required_take_inventories WHERE id=$1 FOR UPDATE",
    )
    .bind(media.inventory.inventory_id)
    .fetch_one(&mut **tx)
    .await?;
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM continuity_ledgers WHERE run_id=$1 AND step_id=$2 AND attempt=$3 AND audit_status='complete'",
    )
    .bind(command.run_id)
    .bind(command.step_id)
    .bind(command.attempt)
    .fetch_one(&mut **tx)
    .await?;
    if existing != 0 {
        return Err(quality_package_blocker("continuity_current_ambiguous"));
    }
    let required_shots = media
        .inventory
        .takes
        .iter()
        .flat_map(|take| take.scene_shot_map.values())
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let actual_shots = output
        .continuity_ledgers
        .iter()
        .map(|ledger| ledger.shot_contract_id)
        .collect::<BTreeSet<_>>();
    if output.continuity_ledgers.len() != actual_shots.len()
        || actual_shots != required_shots
        || output.continuity_ledgers.iter().any(|ledger| {
            ledger.work_version_id != media.inventory.work_version_id
                || ledger.inventory_id != media.inventory.inventory_id
                || ledger.evidence_snapshot_id != media.evidence.evidence_id
        })
    {
        return Err(quality_package_blocker("continuity_output_scope_not_exact"));
    }
    Ok(())
}

async fn validate_take_review_output_scope(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    output: &crate::state::artifacts::output_contract::QcOutput,
) -> ProductionResult<BTreeMap<Uuid, Vec<ApplicableLedgerVersionRow>>> {
    let media = load_media_review_input(tx, command.run_id, command.revision_epoch).await?;
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM required_take_inventories WHERE id=$1 FOR UPDATE",
    )
    .bind(media.inventory.inventory_id)
    .fetch_one(&mut **tx)
    .await?;
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM take_reviews WHERE run_id=$1 AND step_id=$2 AND attempt=$3 AND audit_status='complete'",
    )
    .bind(command.run_id)
    .bind(command.step_id)
    .bind(command.attempt)
    .fetch_one(&mut **tx)
    .await?;
    if existing != 0 {
        return Err(quality_package_blocker("take_review_current_ambiguous"));
    }
    let editor =
        load_current_quality_role_step(tx, command.run_id, command.revision_epoch, "editor")
            .await?;
    let ledger_rows = sqlx::query_as::<_, ApplicableLedgerVersionRow>(
        r#"
        SELECT id AS continuity_ledger_id,shot_contract_id,version AS ledger_version,
               content_digest
        FROM continuity_ledgers
        WHERE run_id=$1 AND step_id=$2 AND attempt=$3 AND revision_epoch=$4
          AND work_version_id=$5 AND inventory_id=$6 AND evidence_snapshot_id=$7
          AND audit_status='complete'
        ORDER BY shot_contract_id,version,id
        FOR SHARE
        "#,
    )
    .bind(command.run_id)
    .bind(editor.0)
    .bind(editor.1)
    .bind(command.revision_epoch)
    .bind(media.inventory.work_version_id)
    .bind(media.inventory.inventory_id)
    .bind(media.evidence.evidence_id)
    .fetch_all(&mut **tx)
    .await?;
    let mut ledgers = BTreeMap::new();
    for ledger in ledger_rows {
        if ledgers.insert(ledger.shot_contract_id, ledger).is_some() {
            return Err(quality_package_blocker("continuity_current_ambiguous"));
        }
    }

    let required_takes = media
        .inventory
        .takes
        .iter()
        .map(|take| (take.take_id, take))
        .collect::<BTreeMap<_, _>>();
    let actual_takes = output
        .take_reviews
        .iter()
        .map(|review| review.required_take_id)
        .collect::<BTreeSet<_>>();
    if output.take_reviews.len() != actual_takes.len()
        || actual_takes != required_takes.keys().copied().collect()
        || output.take_reviews.iter().any(|review| {
            review.work_version_id != media.inventory.work_version_id
                || review.inventory_id != media.inventory.inventory_id
                || review.evidence_snapshot_id != media.evidence.evidence_id
        })
    {
        return Err(quality_package_blocker(
            "take_review_output_scope_not_exact",
        ));
    }

    let mut result = BTreeMap::new();
    for review in &output.take_reviews {
        let take = required_takes[&review.required_take_id];
        let mut ordered_shots = Vec::new();
        let mut seen = BTreeSet::new();
        for scene_id in &take.scene_ids {
            for shot_id in &take.scene_shot_map[scene_id] {
                if seen.insert(*shot_id) {
                    ordered_shots.push(*shot_id);
                }
            }
        }
        let actual = review
            .applicable_shot_contract_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if actual != ordered_shots.iter().copied().collect() {
            return Err(quality_package_blocker(
                "take_review_shot_mapping_not_exact",
            ));
        }
        let mappings = ordered_shots
            .iter()
            .map(|shot_id| ledgers.get(shot_id).cloned())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| quality_package_blocker("take_review_ledger_missing"))?;
        result.insert(review.required_take_id, mappings);
    }
    Ok(result)
}

async fn insert_continuity_ledger(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    ledger: &crate::state::artifacts::output_contract::ContinuityLedgerOutput,
) -> ProductionResult<PersistedRoleArtifact> {
    let version = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT COALESCE(MAX(version), 0) + 1 FROM continuity_ledgers
        WHERE run_id=$1 AND work_version_id=$2 AND inventory_id=$3 AND shot_contract_id=$4
        "#,
    )
    .bind(command.run_id)
    .bind(ledger.work_version_id)
    .bind(ledger.inventory_id)
    .bind(ledger.shot_contract_id)
    .fetch_one(&mut **tx)
    .await?;
    let id = Uuid::new_v4();
    let content = serde_json::to_value(ledger)?;
    let digest = canonical_digest(&content)?;
    sqlx::query(
        r#"
        INSERT INTO continuity_ledgers (
            id, production_project_id, shot_id, content, created_by, run_id, step_id,
            attempt, revision_epoch, work_version_id, inventory_id, evidence_snapshot_id,
            shot_contract_id, version, content_digest, audit_status
        ) VALUES ($1,$2,NULL,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,'complete')
        "#,
    )
    .bind(id)
    .bind(command.production_project_id)
    .bind(content)
    .bind(&command.role_key)
    .bind(command.run_id)
    .bind(command.step_id)
    .bind(command.attempt)
    .bind(command.revision_epoch)
    .bind(ledger.work_version_id)
    .bind(ledger.inventory_id)
    .bind(ledger.evidence_snapshot_id)
    .bind(ledger.shot_contract_id)
    .bind(version)
    .bind(digest)
    .execute(&mut **tx)
    .await?;
    Ok(PersistedRoleArtifact {
        artifact_type: "continuity_ledger".into(),
        id,
        version,
        character_id: None,
        shot_id: None,
    })
}

async fn insert_take_review(
    tx: &mut Transaction<'_, Postgres>,
    command: &RoleFinalizeCommand,
    review: &crate::state::artifacts::output_contract::TakeReviewOutput,
    applicable_ledgers: &[ApplicableLedgerVersionRow],
) -> ProductionResult<PersistedRoleArtifact> {
    let version = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT COALESCE(MAX(version), 0) + 1 FROM take_reviews
        WHERE run_id=$1 AND work_version_id=$2 AND inventory_id=$3 AND required_take_id=$4
        "#,
    )
    .bind(command.run_id)
    .bind(review.work_version_id)
    .bind(review.inventory_id)
    .bind(review.required_take_id)
    .fetch_one(&mut **tx)
    .await?;
    let id = Uuid::new_v4();
    let content = serde_json::to_value(review)?;
    let digest = canonical_digest(&content)?;
    sqlx::query(
        r#"
        INSERT INTO take_reviews (
            id, production_project_id, shot_id, take_number, status, content, created_by,
            run_id, step_id, attempt, revision_epoch, work_version_id, inventory_id,
            evidence_snapshot_id, required_take_id, version, content_digest, audit_status
        ) VALUES ($1,$2,NULL,NULL,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,'complete')
        "#,
    )
    .bind(id)
    .bind(command.production_project_id)
    .bind(&review.review_status)
    .bind(content)
    .bind(&command.role_key)
    .bind(command.run_id)
    .bind(command.step_id)
    .bind(command.attempt)
    .bind(command.revision_epoch)
    .bind(review.work_version_id)
    .bind(review.inventory_id)
    .bind(review.evidence_snapshot_id)
    .bind(review.required_take_id)
    .bind(version)
    .bind(digest)
    .execute(&mut **tx)
    .await?;
    for (ordinal, ledger) in applicable_ledgers.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO take_review_ledger_versions (
                take_review_id,ordinal,continuity_ledger_id,shot_contract_id,
                ledger_version,content_digest
            ) VALUES ($1,$2,$3,$4,$5,$6)
            "#,
        )
        .bind(id)
        .bind(
            i32::try_from(ordinal)
                .map_err(|_| quality_package_blocker("take_review_ledger_ordinal_invalid"))?,
        )
        .bind(ledger.continuity_ledger_id)
        .bind(ledger.shot_contract_id)
        .bind(ledger.ledger_version)
        .bind(&ledger.content_digest)
        .execute(&mut **tx)
        .await?;
    }
    Ok(PersistedRoleArtifact {
        artifact_type: "take_review".into(),
        id,
        version,
        character_id: None,
        shot_id: None,
    })
}

fn validated_lease_ttl(
    lease_owner: &str,
    attempt: i32,
    lease_ttl: Duration,
) -> ProductionResult<chrono::Duration> {
    if lease_owner.trim().is_empty() || attempt <= 0 || lease_ttl.is_zero() {
        return Err(ProductionError::TransitionConflict {
            reason: "lease owner, positive attempt, and ttl are required".into(),
        });
    }
    chrono::Duration::from_std(lease_ttl).map_err(|_| ProductionError::TransitionConflict {
        reason: "lease ttl is outside supported range".into(),
    })
}

fn validate_digest(digest: &str) -> ProductionResult<()> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|value| value.is_ascii_digit() || matches!(value, b'a'..=b'f'))
    {
        return Err(ProductionError::TransitionConflict {
            reason: "canonical lowercase SHA-256 digest is required".into(),
        });
    }
    Ok(())
}

fn production_package_error(details: impl Into<String>) -> ProductionError {
    ProductionError::InvalidArtifactSchema {
        details: details.into(),
    }
}

async fn lock_scene_visual_wait_step(
    tx: &mut Transaction<'_, Postgres>,
    input: &ProductionPackageInput,
    allow_succeeded: bool,
) -> ProductionResult<Uuid> {
    input.package_snapshot()?;
    let revision_epoch = i32::try_from(input.revision_epoch)
        .map_err(|_| production_package_error("revision epoch is invalid"))?;
    let row = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT step.id, step.status
        FROM production_steps step
        JOIN production_runs run ON run.id=step.run_id
        JOIN artifact_package_snapshots package
          ON package.id=$3 AND package.run_id=run.id
         AND package.package_digest=$4 AND package.package_type='production'
        JOIN production_gate_decisions decision
          ON decision.package_id=package.id AND decision.package_digest=package.package_digest
         AND decision.decision='approved'
        WHERE run.id=$1 AND run.current_revision_epoch=$2
          AND step.revision_epoch=$2 AND step.step_key='wait_scene_visual_manifest'
          AND package.revision_epoch=$2
        FOR UPDATE OF step, run, package
        "#,
    )
    .bind(input.run_id)
    .bind(revision_epoch)
    .bind(input.package_id)
    .bind(&input.package_digest)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ProductionError::StalePackage)?;
    let allowed = matches!(row.1.as_str(), "queued" | "running" | "external_wait")
        || (allow_succeeded && row.1 == "succeeded");
    if !allowed {
        return Err(ProductionError::TransitionConflict {
            reason: format!(
                "wait_scene_visual_manifest cannot be observed from status {}",
                row.1
            ),
        });
    }
    Ok(row.0)
}

fn positive_u32(value: i32, field: &str) -> ProductionResult<u32> {
    u32::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| production_package_error(format!("{field} must be positive")))
}

fn package_items<'a>(
    items: &'a [ApprovedProductionPackageItemRow],
    artifact_type: &str,
) -> Vec<&'a ApprovedProductionPackageItemRow> {
    items
        .iter()
        .filter(|item| item.artifact_type == artifact_type)
        .collect()
}

fn only_package_item<'a>(
    items: &'a [ApprovedProductionPackageItemRow],
    artifact_type: &str,
) -> ProductionResult<&'a ApprovedProductionPackageItemRow> {
    let matched = package_items(items, artifact_type);
    if matched.len() != 1 {
        return Err(production_package_error(format!(
            "ProductionPackage requires exactly one {artifact_type}"
        )));
    }
    Ok(matched[0])
}

async fn load_exact_process_artifact(
    tx: &mut Transaction<'_, Postgres>,
    table: &'static str,
    run_id: Uuid,
    item: &ApprovedProductionPackageItemRow,
) -> ProductionResult<ProductionPackageArtifactRow> {
    if !matches!(
        table,
        "directorial_treatments" | "shot_contracts" | "performance_briefs" | "sound_plans"
    ) {
        return Err(production_package_error(
            "unsupported typed production artifact table",
        ));
    }
    let sql = format!(
        "SELECT id, version, content, content_digest, step_id, attempt FROM {table} \
         WHERE id=$1 AND version=$2 AND content_digest=$3 AND run_id=$4 \
           AND step_id=$5 AND attempt=$6 AND audit_status='complete'"
    );
    sqlx::query_as::<_, ProductionPackageArtifactRow>(&sql)
        .bind(item.artifact_id)
        .bind(item.artifact_version)
        .bind(&item.content_digest)
        .bind(run_id)
        .bind(item.source_step_id)
        .bind(item.source_attempt)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| {
            production_package_error(format!(
                "typed {} provenance is incomplete",
                item.artifact_type
            ))
        })
}

fn versioned_process_artifact<T: DeserializeOwned>(
    item: &ApprovedProductionPackageItemRow,
    row: ProductionPackageArtifactRow,
) -> ProductionResult<VersionedProductionArtifact<T>> {
    if row.id != item.artifact_id
        || row.version != item.artifact_version
        || row.content_digest.trim() != item.content_digest
        || row.step_id != item.source_step_id
        || row.attempt != item.source_attempt
    {
        return Err(production_package_error(
            "typed process artifact differs from package provenance",
        ));
    }
    Ok(VersionedProductionArtifact {
        artifact_id: row.id,
        artifact_version: positive_u32(row.version, "artifact version")?,
        content_digest: row.content_digest.trim().into(),
        source_step_id: row.step_id,
        source_attempt: positive_u32(row.attempt, "artifact source attempt")?,
        content: serde_json::from_value(row.content)?,
    })
}

async fn load_typed_process_artifacts<T: DeserializeOwned>(
    tx: &mut Transaction<'_, Postgres>,
    table: &'static str,
    run_id: Uuid,
    items: Vec<&ApprovedProductionPackageItemRow>,
) -> ProductionResult<Vec<VersionedProductionArtifact<T>>> {
    if items.is_empty() {
        return Err(production_package_error(
            "typed process artifact collection is empty",
        ));
    }
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let row = load_exact_process_artifact(tx, table, run_id, item).await?;
        result.push(versioned_process_artifact(item, row)?);
    }
    Ok(result)
}

async fn load_exact_script_artifact(
    tx: &mut Transaction<'_, Postgres>,
    table: &'static str,
    run_id: Uuid,
    source: &ProductionPackageScriptArtifact,
) -> ProductionResult<ProductionPackageArtifactRow> {
    if !matches!(table, "script_drafts")
        || source.artifact_version <= 0
        || source.source_attempt <= 0
    {
        return Err(production_package_error(
            "Script source artifact identity is invalid",
        ));
    }
    let sql = format!(
        "SELECT id, version, content, content_digest, step_id, attempt FROM {table} \
         WHERE id=$1 AND version=$2 AND content_digest=$3 AND run_id=$4 \
           AND step_id=$5 AND attempt=$6 AND audit_status='complete'"
    );
    sqlx::query_as::<_, ProductionPackageArtifactRow>(&sql)
        .bind(source.artifact_id)
        .bind(source.artifact_version)
        .bind(&source.content_digest)
        .bind(run_id)
        .bind(source.source_step_id)
        .bind(source.source_attempt)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| {
            production_package_error("formal Script source artifact provenance is incomplete")
        })
}

async fn load_scoped_process_artifacts(
    tx: &mut Transaction<'_, Postgres>,
    table: &'static str,
    production_project_id: Uuid,
    run_id: Uuid,
    revision_epoch: i32,
    step: &ProductionPackageContributorStep,
) -> ProductionResult<Vec<ProductionPackageArtifactRow>> {
    if !matches!(
        table,
        "creative_briefs"
            | "story_bibles"
            | "character_bibles"
            | "script_drafts"
            | "directorial_treatments"
            | "shot_contracts"
            | "performance_briefs"
            | "sound_plans"
    ) {
        return Err(production_package_error(
            "unsupported package artifact table",
        ));
    }
    let sql = format!(
        "SELECT id, version, content, content_digest, step_id, attempt FROM {table} \
         WHERE production_project_id=$1 AND run_id=$2 AND revision_epoch=$3 \
           AND step_id=$4 AND attempt=$5 AND audit_status='complete' \
         ORDER BY version, id"
    );
    sqlx::query_as::<_, ProductionPackageArtifactRow>(&sql)
        .bind(production_project_id)
        .bind(run_id)
        .bind(revision_epoch)
        .bind(step.id)
        .bind(step.attempt)
        .fetch_all(&mut **tx)
        .await
        .map_err(Into::into)
}

fn process_artifact_ref(
    run_id: Uuid,
    artifact_type: &str,
    row: &ProductionPackageArtifactRow,
) -> ProductionResult<ArtifactRef> {
    let version = u32::try_from(row.version)
        .map_err(|_| production_package_error("process artifact version is invalid"))?;
    let source_attempt = u32::try_from(row.attempt)
        .map_err(|_| production_package_error("process artifact attempt is invalid"))?;
    if version == 0 || source_attempt == 0 {
        return Err(production_package_error(
            "process artifact version and attempt must be positive",
        ));
    }
    Ok(ArtifactRef {
        run_id,
        artifact_type: artifact_type.into(),
        artifact_id: row.id,
        version,
        content_digest: row.content_digest.trim().into(),
        source_step_id: row.step_id,
        source_attempt,
    })
}

async fn lock_owned_step(
    tx: &mut Transaction<'_, Postgres>,
    step_id: Uuid,
    lease_owner: &str,
    attempt: i32,
) -> ProductionResult<ProductionStepRecord> {
    if lease_owner.trim().is_empty() || attempt <= 0 {
        return Err(ProductionError::TransitionConflict {
            reason: "lease owner and positive attempt are required".into(),
        });
    }
    sqlx::query_as::<_, ProductionStepRecord>(
        r#"
        SELECT id, run_id, revision_epoch, plan_order, step_key, step_type, role_key,
               dependencies, status, waiting_reason, error_code, error_details, retryable, attempt,
               lease_owner, lease_expires_at, side_effect_state,
               agent_run_id, model_call_id, context_snapshot_id
        FROM production_steps
        WHERE id = $1 AND status = 'running' AND attempt = $2
          AND lease_owner = $3 AND lease_expires_at >= NOW()
        FOR UPDATE
        "#,
    )
    .bind(step_id)
    .bind(attempt)
    .bind(lease_owner)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::TransitionConflict {
        reason: "step does not have the requested live lease".into(),
    })
}

async fn load_resource_reservations(
    tx: &mut Transaction<'_, Postgres>,
    step_id: Uuid,
    attempt: i32,
) -> ProductionResult<Vec<PersistedResourceReservation>> {
    sqlx::query_as::<_, PersistedResourceReservation>(
        r#"
        SELECT id, run_id, step_id, attempt_no, resource_key, reserved_value,
               actual_value, status, request_digest, created_at, settled_at
        FROM production_resource_reservations
        WHERE step_id = $1 AND attempt_no = $2
        ORDER BY resource_key
        FOR UPDATE
        "#,
    )
    .bind(step_id)
    .bind(attempt)
    .fetch_all(&mut **tx)
    .await
    .map_err(Into::into)
}

fn json_string_array(value: &Value) -> ProductionResult<Vec<String>> {
    serde_json::from_value::<Vec<String>>(value.clone()).map_err(|_| {
        ProductionError::TransitionConflict {
            reason: "frozen step dependencies must be a string array".into(),
        }
    })
}

async fn validate_quality_rework_scope(
    tx: &mut Transaction<'_, Postgres>,
    request: &WorkVersionReworkRequest,
) -> ProductionResult<(PlanSnapshot, u64, Uuid)> {
    let run = sqlx::query_as::<_, (Uuid, i32, String, String, Value)>(
        r#"
        SELECT run.production_project_id,run.current_revision_epoch,run.status,
               run.quality_status,snapshot.plan
        FROM production_runs run
        JOIN production_plan_snapshots snapshot ON snapshot.id=run.plan_snapshot_id
        WHERE run.id=$1
        FOR UPDATE OF run
        "#,
    )
    .bind(request.production_run_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::TransitionConflict {
        reason: "quality rework ProductionRun not found".into(),
    })?;
    if run.1 != request.revision_epoch as i32
        || !matches!(run.2.as_str(), "queued" | "blocked" | "waiting_approval")
        || !matches!(run.3.as_str(), "rejected" | "needs_revision")
    {
        return Err(ProductionError::TransitionConflict {
            reason: "quality rework is outside the current rejected review state".into(),
        });
    }
    let plan: PlanSnapshot = serde_json::from_value(run.4)?;
    let media = load_media_review_input(tx, request.production_run_id, run.1).await?;
    if media.inventory.work_id != request.work_id
        || media.inventory.work_version_id != request.source_work_version_id
        || media.inventory.inventory_id != request.inventory_id
        || media.inventory.inventory_digest != request.inventory_digest
        || media.evidence.evidence_id != request.evidence_snapshot_id
        || media.evidence.evidence_digest != request.evidence_digest
    {
        return Err(media_evidence_blocker("quality_rework_media_scope_stale"));
    }
    let required_takes = media
        .inventory
        .takes
        .iter()
        .map(|take| take.take_id)
        .collect::<std::collections::BTreeSet<_>>();
    let required_shots = media
        .inventory
        .takes
        .iter()
        .flat_map(|take| take.scene_shot_map.values())
        .flatten()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let rejected_takes = request
        .rejected_take_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let affected_shots = request
        .affected_shot_contract_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let scope_valid = rejected_takes.is_subset(&required_takes)
        && affected_shots.is_subset(&required_shots)
        && (request.kind != WorkVersionReworkKind::FullRegeneration
            || (rejected_takes == required_takes && affected_shots == required_shots));
    if !scope_valid {
        return Err(media_evidence_blocker("quality_rework_coverage_invalid"));
    }
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM production_revision_epochs WHERE run_id=$1 AND reason_type='quality_rework'",
    )
    .bind(request.production_run_id)
    .fetch_one(&mut **tx)
    .await?;
    let count = u64::try_from(count).map_err(|_| ProductionError::TransitionConflict {
        reason: "quality rework count is invalid".into(),
    })?;
    Ok((plan, count, run.0))
}

async fn load_current_quality_role_step(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    revision_epoch: i32,
    role_key: &str,
) -> ProductionResult<(Uuid, i32)> {
    let rows = sqlx::query_as::<_, (Uuid, i32)>(
        r#"
        SELECT id,attempt FROM production_steps
        WHERE run_id=$1 AND revision_epoch=$2 AND step_key=$3 AND step_type='role'
          AND role_key=$3 AND status='succeeded' AND attempt > 0
        ORDER BY id
        FOR SHARE
        "#,
    )
    .bind(run_id)
    .bind(revision_epoch)
    .bind(role_key)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() != 1 {
        return Err(quality_package_blocker(match role_key {
            "editor" => "quality_editor_attempt_missing",
            "qc" => "quality_qc_attempt_missing",
            _ => "quality_role_attempt_invalid",
        }));
    }
    Ok(rows[0])
}

fn parse_quality_review_status(value: &str) -> ProductionResult<QualityReviewStatus> {
    match value {
        "approved" => Ok(QualityReviewStatus::Approved),
        "needs_revision" => Ok(QualityReviewStatus::NeedsRevision),
        "rejected" => Ok(QualityReviewStatus::Rejected),
        _ => Err(quality_package_blocker("take_review_status_invalid")),
    }
}

fn quality_package_blocker(reason: &str) -> ProductionError {
    ProductionError::EvidenceBlocker {
        reason: reason.into(),
        details: json!({"blocker": reason}),
    }
}

/// 从当前 revision 的唯一不可变快照重建 Editor/QC 输入；任何歧义都不得猜测选择。
async fn load_media_review_input(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    revision_epoch: i32,
) -> ProductionResult<MediaReviewInput> {
    let current_epoch = sqlx::query_scalar::<_, i32>(
        "SELECT current_revision_epoch FROM production_runs WHERE id=$1 FOR SHARE",
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::TransitionConflict {
        reason: "media review ProductionRun not found".into(),
    })?;
    if current_epoch != revision_epoch || revision_epoch < 0 {
        return Err(media_evidence_blocker("media_review_revision_stale"));
    }

    let mut inventories = sqlx::query_as::<_, MediaInventoryRow>(
        r#"
        SELECT id AS inventory_id,run_id,source_step_id,source_attempt,revision_epoch,
               work_id,work_version_id,work_generation_run_id,final_artifact_id,
               work_version_hash,inventory_digest
        FROM required_take_inventories
        WHERE run_id=$1 AND revision_epoch=$2
        ORDER BY created_at,id
        FOR SHARE
        "#,
    )
    .bind(run_id)
    .bind(revision_epoch)
    .fetch_all(&mut **tx)
    .await?;
    if inventories.is_empty() {
        return media_review_readiness(None, None);
    }
    if inventories.len() != 1 {
        return Err(media_evidence_blocker("required_take_inventory_ambiguous"));
    }
    let inventory_row = inventories.remove(0);

    let source = sqlx::query_as::<_, (Uuid, i32, i32, String, String)>(
        "SELECT run_id,revision_epoch,attempt,status,step_key FROM production_steps WHERE id=$1 FOR SHARE",
    )
    .bind(inventory_row.source_step_id)
    .fetch_optional(&mut **tx)
    .await?;
    if source
        != Some((
            inventory_row.run_id,
            inventory_row.revision_epoch,
            inventory_row.source_attempt,
            "succeeded".into(),
            "wait_work_generation".into(),
        ))
    {
        return Err(media_evidence_blocker(
            "required_take_inventory_source_stale",
        ));
    }
    let generation = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        "SELECT work_id,work_version_id,status FROM work_generation_runs WHERE id=$1",
    )
    .bind(inventory_row.work_generation_run_id)
    .fetch_optional(&mut **tx)
    .await?;
    if generation
        != Some((
            inventory_row.work_id,
            inventory_row.work_version_id,
            "succeeded".into(),
        ))
    {
        return Err(media_evidence_blocker(
            "required_take_inventory_generation_stale",
        ));
    }
    let formally_linked = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM production_domain_links
            WHERE run_id=$1 AND revision_epoch=$2 AND link_type='work_generation_run'
              AND work_generation_run_id=$3
        )
        "#,
    )
    .bind(run_id)
    .bind(revision_epoch)
    .bind(inventory_row.work_generation_run_id)
    .fetch_one(&mut **tx)
    .await?;
    if !formally_linked {
        return Err(media_evidence_blocker(
            "required_take_inventory_domain_link_missing",
        ));
    }

    let version_snapshot = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT jsonb_build_object(
            'id',id,'work_id',work_id,'version_no',version_no,
            'source_manifest_version',source_manifest_version,
            'input_snapshot',input_snapshot,'model_snapshot',model_snapshot,
            'parameter_snapshot',parameter_snapshot,'timeline_snapshot',timeline_snapshot,
            'prompt_snapshot',prompt_snapshot
        )
        FROM work_versions WHERE id=$1
        "#,
    )
    .bind(inventory_row.work_version_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| media_evidence_blocker("work_version_missing"))?;
    if canonical_digest(&version_snapshot)? != inventory_row.work_version_hash {
        return Err(media_evidence_blocker("work_version_hash_stale"));
    }

    let final_artifact = sqlx::query_as::<_, (Uuid, String, String, String, Value)>(
        "SELECT work_version_id,role,sha256,mime_type,metadata FROM work_artifacts WHERE id=$1",
    )
    .bind(inventory_row.final_artifact_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| media_evidence_blocker("final_media_missing"))?;
    let duration_ms = media_duration_ms(&final_artifact.4)
        .ok_or_else(|| media_evidence_blocker("final_media_duration_missing"))?;
    if final_artifact.0 != inventory_row.work_version_id || final_artifact.1 != "final_video" {
        return Err(media_evidence_blocker("final_media_identity_mismatch"));
    }

    let take_rows = sqlx::query_as::<_, RequiredTakeRow>(
        r#"
        SELECT take.id AS take_id,take.ordinal,take.generation_step_id,
               take.generation_attempt_id,take.output_artifact_id,take.segment_key,
               take.scene_ids,take.scene_shot_map,
               generation_step.run_id AS generation_run_id,
               generation_step.status AS generation_step_status,
               generation_step.step_type AS generation_step_type,
               attempt.step_id AS attempt_step_id,attempt.status AS attempt_status,
               output.work_version_id AS output_work_version_id,
               output.generation_step_id AS output_generation_step_id,
               output.role AS output_role
        FROM required_takes take
        JOIN work_generation_steps generation_step ON generation_step.id=take.generation_step_id
        JOIN work_generation_attempts attempt ON attempt.id=take.generation_attempt_id
        JOIN work_artifacts output ON output.id=take.output_artifact_id
        WHERE take.inventory_id=$1
        ORDER BY take.ordinal,take.id
        "#,
    )
    .bind(inventory_row.inventory_id)
    .fetch_all(&mut **tx)
    .await?;
    let mut takes = Vec::with_capacity(take_rows.len());
    for row in take_rows {
        if row.generation_run_id != inventory_row.work_generation_run_id
            || row.generation_step_status != "succeeded"
            || row.generation_step_type != "video_segment"
            || row.attempt_step_id != row.generation_step_id
            || row.attempt_status != "succeeded"
            || row.output_work_version_id != inventory_row.work_version_id
            || row.output_generation_step_id != Some(row.generation_step_id)
            || row.output_role != "reusable_intermediate"
        {
            return Err(media_evidence_blocker("required_take_provenance_stale"));
        }
        takes.push(RequiredTake {
            take_id: row.take_id,
            ordinal: usize::try_from(row.ordinal)
                .map_err(|_| media_evidence_blocker("required_take_ordinal_invalid"))?,
            generation_step_id: row.generation_step_id,
            generation_attempt_id: row.generation_attempt_id,
            output_artifact_id: row.output_artifact_id,
            segment_key: row.segment_key,
            scene_ids: serde_json::from_value(row.scene_ids)
                .map_err(|_| media_evidence_blocker("required_take_scene_mapping_invalid"))?,
            scene_shot_map: serde_json::from_value(row.scene_shot_map)
                .map_err(|_| media_evidence_blocker("required_take_shot_mapping_invalid"))?,
        });
    }
    let inventory = RequiredTakeInventorySnapshot {
        inventory_id: inventory_row.inventory_id,
        run_id: inventory_row.run_id,
        source_step_id: inventory_row.source_step_id,
        source_attempt: u32::try_from(inventory_row.source_attempt)
            .map_err(|_| media_evidence_blocker("required_take_source_attempt_invalid"))?,
        revision_epoch: u32::try_from(inventory_row.revision_epoch)
            .map_err(|_| media_evidence_blocker("required_take_revision_invalid"))?,
        work_id: inventory_row.work_id,
        work_version_id: inventory_row.work_version_id,
        work_generation_run_id: inventory_row.work_generation_run_id,
        final_asset: FinalMediaAsset {
            artifact_id: inventory_row.final_artifact_id,
            sha256: final_artifact.2,
            mime_type: final_artifact.3,
            duration_ms,
        },
        work_version_hash: inventory_row.work_version_hash,
        takes,
        inventory_digest: inventory_row.inventory_digest,
    };

    let mut evidence_rows = sqlx::query_as::<_, MediaEvidenceRow>(
        r#"
        SELECT evidence.id AS evidence_id,evidence.run_id,evidence.source_step_id,
               evidence.source_attempt,evidence.revision_epoch,evidence.work_version_id,
               evidence.inventory_id,inventory.inventory_digest,evidence.final_artifact_id,
               evidence.asset_hash,evidence.mime_type,evidence.duration_ms,
               evidence.vision_capability_version,evidence.audio_capability_version,
               evidence.redacted_analysis,evidence.evidence_digest
        FROM media_evidence_snapshots evidence
        JOIN required_take_inventories inventory ON inventory.id=evidence.inventory_id
        WHERE evidence.run_id=$1 AND evidence.revision_epoch=$2
          AND evidence.inventory_id=$3 AND evidence.work_version_id=$4
        ORDER BY evidence.created_at,evidence.id
        FOR SHARE OF evidence,inventory
        "#,
    )
    .bind(run_id)
    .bind(revision_epoch)
    .bind(inventory.inventory_id)
    .bind(inventory.work_version_id)
    .fetch_all(&mut **tx)
    .await?;
    if evidence_rows.is_empty() {
        return media_review_readiness(Some(inventory), None);
    }
    if evidence_rows.len() != 1 {
        return Err(media_evidence_blocker("media_evidence_ambiguous"));
    }
    let row = evidence_rows.remove(0);
    let evidence = MediaEvidenceSnapshot {
        evidence_id: row.evidence_id,
        run_id: row.run_id,
        source_step_id: row.source_step_id,
        source_attempt: u32::try_from(row.source_attempt)
            .map_err(|_| media_evidence_blocker("media_evidence_source_attempt_invalid"))?,
        revision_epoch: u32::try_from(row.revision_epoch)
            .map_err(|_| media_evidence_blocker("media_evidence_revision_invalid"))?,
        work_version_id: row.work_version_id,
        inventory_id: row.inventory_id,
        inventory_digest: row.inventory_digest,
        final_artifact_id: row.final_artifact_id,
        asset_hash: row.asset_hash,
        mime_type: row.mime_type,
        duration_ms: u64::try_from(row.duration_ms)
            .map_err(|_| media_evidence_blocker("media_evidence_duration_invalid"))?,
        vision_capability_version: row.vision_capability_version,
        audio_capability_version: row.audio_capability_version,
        redacted_analysis: row.redacted_analysis,
        evidence_digest: row.evidence_digest,
    };
    media_review_readiness(Some(inventory), Some(evidence))
}

fn media_evidence_blocker(reason: &str) -> ProductionError {
    ProductionError::EvidenceBlocker {
        reason: reason.into(),
        details: json!({"blocker": reason}),
    }
}

async fn load_role_input_package(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    revision_epoch: i32,
    package_id: Uuid,
    package_digest: &str,
    gate_step_id: Uuid,
) -> ProductionResult<RoleInputPackage> {
    validate_digest(package_digest)?;
    let package = sqlx::query_as::<_, RoleInputPackageRow>(
        r#"
        SELECT package.id, package.package_type, package.package_digest,
               package.revision_epoch
        FROM artifact_package_snapshots package
        JOIN production_gate_decisions decision
          ON decision.package_id = package.id
         AND decision.run_id = package.run_id
         AND decision.package_digest = package.package_digest
        WHERE package.id=$1 AND package.run_id=$2 AND package.revision_epoch=$3
          AND package.package_digest=$4 AND decision.gate_step_id=$5
          AND decision.decision='approved'
        "#,
    )
    .bind(package_id)
    .bind(run_id)
    .bind(revision_epoch)
    .bind(package_digest)
    .bind(gate_step_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::TransitionConflict {
        reason: "role input package is missing, stale, or not approved by the exact Gate".into(),
    })?;
    let items = sqlx::query_as::<_, RoleInputArtifactRef>(
        r#"
        SELECT artifact_type, artifact_id, artifact_version, content_digest,
               source_step_id, source_attempt
        FROM artifact_package_items
        WHERE package_id=$1
        ORDER BY ordinal
        "#,
    )
    .bind(package.id)
    .fetch_all(&mut **tx)
    .await?;
    if items.is_empty() {
        return Err(ProductionError::TransitionConflict {
            reason: "role input package has no immutable artifact references".into(),
        });
    }
    Ok(RoleInputPackage {
        id: package.id,
        package_type: package.package_type,
        digest: package.package_digest,
        revision_epoch: package.revision_epoch,
        items,
    })
}

fn validate_safe_object(value: &Value) -> ProductionResult<()> {
    if !value.is_object() {
        return Err(ProductionError::SourceInvalid {
            reason: "initial_input must be an object".into(),
        });
    }
    let encoded = serde_json::to_string(value)?.to_ascii_lowercase();
    if [
        "api_key",
        "authorization",
        "cookie",
        "credentials",
        "signed_url",
        "base64",
    ]
    .iter()
    .any(|forbidden| encoded.contains(forbidden))
    {
        return Err(ProductionError::SourceInvalid {
            reason: "initial_input contains credentials or media binary/temporary URL".into(),
        });
    }
    Ok(())
}

async fn lock_full_crew_intent(
    tx: &mut Transaction<'_, Postgres>,
    intent_id: Uuid,
) -> ProductionResult<()> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM production_projects
        WHERE id=$1 AND project_type='full_crew'
          AND status <> 'legacy_unbound' AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(intent_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ProductionError::ProjectNotFound {
        project_id: intent_id,
    })?;
    Ok(())
}

async fn load_intent_history(
    tx: &mut Transaction<'_, Postgres>,
    intent_id: Uuid,
) -> ProductionResult<IntentHistory> {
    let (has_run, has_artifacts, has_domain_links, run_terminal) =
        sqlx::query_as::<_, (bool, bool, bool, bool)>(
            r#"
            SELECT
                EXISTS(SELECT 1 FROM production_runs WHERE production_project_id=$1),
                EXISTS(
                    SELECT 1 FROM creative_briefs WHERE production_project_id=$1
                    UNION ALL SELECT 1 FROM story_bibles WHERE production_project_id=$1
                    UNION ALL SELECT 1 FROM character_bibles WHERE production_project_id=$1
                    UNION ALL SELECT 1 FROM script_drafts WHERE production_project_id=$1
                    UNION ALL SELECT 1 FROM directorial_treatments WHERE production_project_id=$1
                    UNION ALL SELECT 1 FROM shot_contracts WHERE production_project_id=$1
                    UNION ALL SELECT 1 FROM performance_briefs WHERE production_project_id=$1
                    UNION ALL SELECT 1 FROM sound_plans WHERE production_project_id=$1
                    UNION ALL SELECT 1 FROM continuity_ledgers WHERE production_project_id=$1
                    UNION ALL SELECT 1 FROM take_reviews WHERE production_project_id=$1
                    UNION ALL SELECT 1 FROM collaboration_suggestions WHERE production_project_id=$1
                ),
                EXISTS(
                    SELECT 1 FROM production_domain_links link
                    JOIN production_runs run ON run.id=link.run_id
                    WHERE run.production_project_id=$1
                ),
                COALESCE((
                    SELECT bool_and(status IN ('cancelled','failed','completed'))
                    FROM production_runs WHERE production_project_id=$1
                ),FALSE)
            "#,
        )
        .bind(intent_id)
        .fetch_one(&mut **tx)
        .await?;
    Ok(IntentHistory {
        has_run,
        has_artifacts,
        has_domain_links,
        run_terminal,
    })
}

fn validate_active_bindings(plan: &PlanSnapshot) -> ProductionResult<()> {
    let bindings =
        plan.role_bindings
            .as_object()
            .ok_or_else(|| ProductionError::TransitionConflict {
                reason: "role binding snapshot must be an object".into(),
            })?;
    for step in plan.steps.iter().filter(|step| step.role_key.is_some()) {
        if step.optional {
            continue;
        }
        let role = step.role_key.as_deref().unwrap_or_default();
        let binding = bindings
            .get(role)
            .and_then(Value::as_object)
            .ok_or_else(|| ProductionError::TransitionConflict {
                reason: format!("missing active binding for role {role}"),
            })?;
        if binding.get("lifecycle").and_then(Value::as_str) != Some("active") {
            return Err(ProductionError::TransitionConflict {
                reason: format!("role {role} is not bound to an active definition"),
            });
        }
    }
    Ok(())
}

fn validate_public_binding_snapshot(value: &Value) -> ProductionResult<()> {
    let encoded = serde_json::to_string(value)?.to_ascii_lowercase();
    if [
        "api_key",
        "api_secret",
        "authorization",
        "cookie",
        "credential",
        "signed_url",
        "base64",
        "price",
        "currency",
        "amount_limit",
        "budget_amount",
    ]
    .iter()
    .any(|forbidden| encoded.contains(forbidden))
    {
        return Err(ProductionError::TransitionConflict {
            reason: "role binding snapshot contains forbidden pricing or credential data".into(),
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn ensure_wakeup_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    step_id: Uuid,
) -> ProductionResult<bool> {
    let existing = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id,status FROM production_wakeups
        WHERE step_id=$1
        ORDER BY CASE WHEN status='pending' THEN 0 ELSE 1 END,created_at DESC
        LIMIT 1 FOR UPDATE
        "#,
    )
    .bind(step_id)
    .fetch_optional(&mut **tx)
    .await?;
    match existing {
        Some((_, status)) if status == "pending" => Ok(false),
        Some((id, _)) => {
            sqlx::query(
                r#"
                UPDATE production_wakeups
                SET status='pending',last_error=NULL,delivered_at=NULL,created_at=NOW()
                WHERE id=$1
                "#,
            )
            .bind(id)
            .execute(&mut **tx)
            .await?;
            Ok(true)
        }
        None => {
            sqlx::query("INSERT INTO production_wakeups (run_id,step_id) VALUES ($1,$2)")
                .bind(run_id)
                .bind(step_id)
                .execute(&mut **tx)
                .await?;
            Ok(true)
        }
    }
}

fn start_run_request_digest(intent_id: Uuid) -> ProductionResult<String> {
    ProductionCommandStore::canonical_request_digest(&json!({"intent_id": intent_id}))
}

fn uuid_from_result(result: &Value, key: &str) -> ProductionResult<Uuid> {
    result
        .get(key)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: format!("stored command result is missing {key}"),
        })
}

fn is_constraint(error: &sqlx::Error, constraint: &str) -> bool {
    error
        .as_database_error()
        .and_then(|database| database.constraint())
        == Some(constraint)
}

async fn json_rows(pool: &PgPool, query: &str, id: Uuid) -> ProductionResult<Vec<Value>> {
    sqlx::query_scalar::<_, Value>(query)
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}

async fn validate_persisted_workflow_command(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    idempotency_key: &str,
    command: WorkflowCommand,
) -> ProductionResult<()> {
    let (plan, workflow) = load_frozen_workflow(tx, run_id).await?;
    validate_workflow_command(
        &plan,
        &workflow,
        &CommandEnvelope {
            plan_key: plan.plan_key.clone(),
            plan_version: plan.plan_version.clone(),
            plan_digest: plan.digest.clone(),
            idempotency_key: idempotency_key.into(),
            command,
        },
    )
}

async fn load_frozen_workflow(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
) -> ProductionResult<(PlanSnapshot, WorkflowSnapshot)> {
    let (plan_value, run_status, current_epoch, cancellation_intent) =
        sqlx::query_as::<_, (Value, String, i32, Option<Value>)>(
            r#"
            SELECT snapshot.plan, run.status, run.current_revision_epoch,
                   run.cancellation_intent
            FROM production_runs run
            JOIN production_plan_snapshots snapshot ON snapshot.id = run.plan_snapshot_id
            WHERE run.id = $1
            FOR UPDATE OF run
            "#,
        )
        .bind(run_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "production run or frozen plan snapshot not found".into(),
        })?;
    let plan: PlanSnapshot = serde_json::from_value(plan_value)?;
    let current_revision_epoch =
        u32::try_from(current_epoch).map_err(|_| ProductionError::TransitionConflict {
            reason: "production Run has an invalid revision epoch".into(),
        })?;
    let mut run = RunState::new(parse_run_status(&run_status)?, current_revision_epoch);
    run.cancellation_requested = cancellation_intent.is_some();
    let records = sqlx::query_as::<_, ProductionStepRecord>(
        r#"
        SELECT id, run_id, revision_epoch, plan_order, step_key, step_type, role_key,
               dependencies, status, waiting_reason, error_code, error_details, retryable, attempt,
               lease_owner, lease_expires_at, side_effect_state,
               agent_run_id, model_call_id, context_snapshot_id
        FROM production_steps
        WHERE run_id = $1
        ORDER BY revision_epoch, plan_order
        "#,
    )
    .bind(run_id)
    .fetch_all(&mut **tx)
    .await?;
    let steps = records
        .into_iter()
        .map(step_state_from_record)
        .collect::<ProductionResult<Vec<_>>>()?;
    Ok((plan, WorkflowSnapshot::new(run, steps)))
}

fn step_state_from_record(record: ProductionStepRecord) -> ProductionResult<StepState> {
    Ok(StepState {
        id: record.id,
        step_key: record.step_key,
        kind: parse_step_kind(&record.step_type)?,
        revision_epoch: u32::try_from(record.revision_epoch).map_err(|_| {
            ProductionError::TransitionConflict {
                reason: "production step has an invalid revision epoch".into(),
            }
        })?,
        status: parse_step_status(&record.status)?,
        attempt: u32::try_from(record.attempt).map_err(|_| {
            ProductionError::TransitionConflict {
                reason: "production step has an invalid attempt".into(),
            }
        })?,
        lease_owner: record.lease_owner,
        lease_expires_at: record.lease_expires_at,
        side_effect_state: parse_side_effect_state(&record.side_effect_state)?,
        waiting_reason: record
            .waiting_reason
            .as_deref()
            .map(parse_waiting_reason)
            .transpose()?,
    })
}

fn parse_run_status(value: &str) -> ProductionResult<RunStatus> {
    match value {
        "created" => Ok(RunStatus::Created),
        "queued" => Ok(RunStatus::Queued),
        "running" => Ok(RunStatus::Running),
        "waiting_approval" => Ok(RunStatus::WaitingApproval),
        "external_wait" => Ok(RunStatus::ExternalWait),
        "blocked" => Ok(RunStatus::Blocked),
        "attention_required" => Ok(RunStatus::AttentionRequired),
        "cancelling" => Ok(RunStatus::Cancelling),
        "cancelled" => Ok(RunStatus::Cancelled),
        "failed" => Ok(RunStatus::Failed),
        "completed" => Ok(RunStatus::Completed),
        _ => Err(ProductionError::TransitionConflict {
            reason: format!("unknown production Run status: {value}"),
        }),
    }
}

fn parse_step_status(value: &str) -> ProductionResult<StepStatus> {
    match value {
        "blocked" => Ok(StepStatus::Blocked),
        "queued" => Ok(StepStatus::Queued),
        "running" => Ok(StepStatus::Running),
        "waiting_approval" => Ok(StepStatus::WaitingApproval),
        "external_wait" => Ok(StepStatus::ExternalWait),
        "succeeded" => Ok(StepStatus::Succeeded),
        "failed" => Ok(StepStatus::Failed),
        "attention_required" => Ok(StepStatus::AttentionRequired),
        "cancelling" => Ok(StepStatus::Cancelling),
        "cancelled" => Ok(StepStatus::Cancelled),
        "superseded" => Ok(StepStatus::Superseded),
        _ => Err(ProductionError::TransitionConflict {
            reason: format!("unknown production Step status: {value}"),
        }),
    }
}

fn parse_step_kind(value: &str) -> ProductionResult<StepKind> {
    match value {
        "role" => Ok(StepKind::Role),
        "gate" => Ok(StepKind::Gate),
        "domain_command" => Ok(StepKind::DomainCommand),
        "external_wait" => Ok(StepKind::ExternalWait),
        _ => Err(ProductionError::TransitionConflict {
            reason: format!("unknown production Step kind: {value}"),
        }),
    }
}

fn parse_side_effect_state(value: &str) -> ProductionResult<SideEffectState> {
    match value {
        "none" => Ok(SideEffectState::None),
        "prepared" => Ok(SideEffectState::Prepared),
        "submitted" => Ok(SideEffectState::Submitted),
        "confirmed" => Ok(SideEffectState::Confirmed),
        "unknown" => Ok(SideEffectState::Unknown),
        _ => Err(ProductionError::TransitionConflict {
            reason: format!("unknown production side-effect state: {value}"),
        }),
    }
}

fn parse_waiting_reason(value: &str) -> ProductionResult<super::state_machine::WaitingReason> {
    use super::state_machine::WaitingReason;
    match value {
        "dependencies" => Ok(WaitingReason::Dependencies),
        "package_approval" => Ok(WaitingReason::PackageApproval),
        "scene_visual_manifest" => Ok(WaitingReason::SceneVisualManifest),
        "work_plan_confirmation" => Ok(WaitingReason::WorkPlanConfirmation),
        "work_generation" => Ok(WaitingReason::WorkGeneration),
        "evidence_incomplete" => Ok(WaitingReason::EvidenceIncomplete),
        "external_failure" => Ok(WaitingReason::ExternalFailure),
        "external_cancel_conflict" => Ok(WaitingReason::ExternalCancelConflict),
        "manual_attention" => Ok(WaitingReason::ManualAttention),
        "cancellation_pending" => Ok(WaitingReason::CancellationPending),
        "revision_limit" => Ok(WaitingReason::RevisionLimit),
        _ => Err(ProductionError::TransitionConflict {
            reason: format!("unknown production waiting reason: {value}"),
        }),
    }
}

fn cancellation_external_run_ids(intent: &Value) -> ProductionResult<Vec<Uuid>> {
    intent
        .get("external_run_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "cancellation intent is missing external WorkGenerationRun identities".into(),
        })?
        .iter()
        .map(|value| {
            value
                .as_str()
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(|| ProductionError::TransitionConflict {
                    reason: "cancellation intent contains an invalid WorkGenerationRun identity"
                        .into(),
                })
        })
        .collect()
}

fn external_cancellation_state_name(state: ExternalCancellationState) -> &'static str {
    match state {
        ExternalCancellationState::Cancelling => "cancelling",
        ExternalCancellationState::Cancelled => "cancelled",
        ExternalCancellationState::AttentionRequired => "attention_required",
    }
}

fn validate_stable_error_code(error_code: &str) -> ProductionResult<()> {
    if error_code.trim().is_empty()
        || error_code.len() > 120
        || !error_code
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(ProductionError::TransitionConflict {
            reason: "production error code is invalid".into(),
        });
    }
    Ok(())
}

fn package_type_name(package_type: PackageType) -> &'static str {
    match package_type {
        PackageType::Brief => "brief",
        PackageType::Script => "script",
        PackageType::Production => "production",
        PackageType::Quality => "quality",
    }
}

fn parse_package_type(value: &str) -> ProductionResult<PackageType> {
    match value {
        "brief" => Ok(PackageType::Brief),
        "script" => Ok(PackageType::Script),
        "production" => Ok(PackageType::Production),
        "quality" => Ok(PackageType::Quality),
        _ => Err(ProductionError::TransitionConflict {
            reason: format!("unknown package type {value}"),
        }),
    }
}

fn gate_step_key(package_type: PackageType) -> &'static str {
    match package_type {
        PackageType::Brief => "brief_approval",
        PackageType::Script => "script_package_approval",
        PackageType::Production => "production_package_approval",
        PackageType::Quality => "quality_gate",
    }
}

fn gate_decision_name(decision: GateDecision) -> &'static str {
    match decision {
        GateDecision::Approve => "approved",
        GateDecision::Reject => "rejected",
    }
}

async fn validate_production_suggestion_resolutions(
    tx: &mut Transaction<'_, Postgres>,
    package: &ArtifactPackageSnapshot,
) -> ProductionResult<()> {
    let requirements = sqlx::query_as::<_, SuggestionResolutionRequirement>(
        r#"
        SELECT suggestion.id, suggestion.to_role, suggestion.artifact_type,
               suggestion.target_artifact_version, suggestion.target_content_digest,
               response.decision
        FROM collaboration_suggestions suggestion
        LEFT JOIN collaboration_suggestion_responses response
          ON response.suggestion_id = suggestion.id
        WHERE suggestion.run_id = $1 AND suggestion.revision_epoch = $2
          AND suggestion.audit_status = 'complete' AND suggestion.blocking = TRUE
        ORDER BY suggestion.created_at, suggestion.id
        "#,
    )
    .bind(package.run_id)
    .bind(package.revision_epoch as i32)
    .fetch_all(&mut **tx)
    .await?;
    let resolutions = package
        .metadata
        .get("suggestion_resolutions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for requirement in requirements {
        match requirement.decision.as_deref() {
            Some("rejected") => continue,
            None => {
                return Err(ProductionError::TransitionConflict {
                    reason: format!(
                        "blocking collaboration suggestion {} has no response",
                        requirement.id
                    ),
                })
            }
            Some("accepted") => {}
            Some(other) => {
                return Err(ProductionError::TransitionConflict {
                    reason: format!("unknown collaboration response {other}"),
                })
            }
        }
        let resolved = resolutions.iter().any(|resolution| {
            let suggestion_id = resolution
                .get("suggestion_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok());
            let artifact_id = resolution
                .get("artifact_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok());
            let version = resolution
                .get("artifact_version")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok());
            let digest = resolution.get("content_digest").and_then(Value::as_str);
            suggestion_id == Some(requirement.id)
                && resolution.get("owner_role").and_then(Value::as_str)
                    == Some(requirement.to_role.as_str())
                && version.is_some_and(|value| value > requirement.target_artifact_version as u32)
                && digest.is_some_and(|value| value != requirement.target_content_digest)
                && package.items.iter().any(|item| {
                    Some(item.artifact_id) == artifact_id
                        && Some(item.version) == version
                        && item.artifact_type == requirement.artifact_type
                        && Some(item.content_digest.as_str()) == digest
                })
        });
        if !resolved {
            return Err(ProductionError::TransitionConflict {
                reason: format!(
                    "accepted blocking suggestion {} requires a referenced newer owner artifact",
                    requirement.id
                ),
            });
        }
    }
    Ok(())
}

async fn load_gate_decision(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> ProductionResult<PersistedGateDecision> {
    sqlx::query_as::<_, PersistedGateDecision>(
        r#"
        SELECT id, run_id, gate_step_id, package_id, package_digest, revision_epoch,
               decision, reason, affected_owners, actor_type, actor_id, decided_at
        FROM production_gate_decisions WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::TransitionConflict {
        reason: "stored package decision not found".into(),
    })
}

async fn unlock_ready_steps(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    revision_epoch: i32,
) -> ProductionResult<()> {
    sqlx::query(
        r#"
        UPDATE production_steps candidate
        SET status = 'queued',
            input_package_id = COALESCE(
                candidate.input_package_id,
                (
                    SELECT (array_agg(DISTINCT completed.input_package_id))[1]
                    FROM jsonb_array_elements_text(candidate.dependencies) dependency(step_key)
                    JOIN production_steps completed
                      ON completed.run_id = candidate.run_id
                     AND completed.revision_epoch = candidate.revision_epoch
                     AND completed.step_key = dependency.step_key
                    WHERE completed.input_package_id IS NOT NULL
                    HAVING COUNT(DISTINCT completed.input_package_id) = 1
                )
            ),
            input_digest = COALESCE(
                candidate.input_digest,
                (
                    SELECT MIN(completed.input_digest)
                    FROM jsonb_array_elements_text(candidate.dependencies) dependency(step_key)
                    JOIN production_steps completed
                      ON completed.run_id = candidate.run_id
                     AND completed.revision_epoch = candidate.revision_epoch
                     AND completed.step_key = dependency.step_key
                    WHERE completed.input_digest IS NOT NULL
                    HAVING COUNT(DISTINCT completed.input_digest) = 1
                )
            ),
            updated_at = NOW()
        WHERE candidate.run_id = $1
          AND candidate.revision_epoch = $2
          AND candidate.status = 'blocked'
          AND NOT EXISTS (
              SELECT 1
              FROM jsonb_array_elements_text(candidate.dependencies) dependency(step_key)
              WHERE NOT EXISTS (
                  SELECT 1 FROM production_steps completed
                  WHERE completed.run_id = candidate.run_id
                    AND completed.revision_epoch = candidate.revision_epoch
                    AND completed.step_key = dependency.step_key
                    AND completed.status = 'succeeded'
              )
          )
        "#,
    )
    .bind(run_id)
    .bind(revision_epoch)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn set_production_run_state(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    status: &str,
    error_code: Option<&str>,
    error_details: Option<&Value>,
) -> ProductionResult<()> {
    let intent_status = match status {
        "queued" => "editing",
        "blocked" => "attention_required",
        "external_wait" => "external_wait",
        "attention_required" => "attention_required",
        "cancelling" => "cancelling",
        "cancelled" => "cancelled",
        "failed" => "failed",
        "completed" => "completed",
        other => {
            return Err(ProductionError::TransitionConflict {
                reason: format!("unsupported ProductionRun synchronization status: {other}"),
            })
        }
    };
    let production_project_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        UPDATE production_runs
        SET status=$2,error_code=$3,error_details=$4,
            completed_at=CASE
                WHEN $2 IN ('cancelled','failed','completed') THEN COALESCE(completed_at,NOW())
                ELSE NULL
            END,
            updated_at=NOW()
        WHERE id=$1
        RETURNING production_project_id
        "#,
    )
    .bind(run_id)
    .bind(status)
    .bind(error_code)
    .bind(error_details)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::TransitionConflict {
        reason: "production run not found".into(),
    })?;
    sqlx::query("UPDATE production_projects SET status=$2,updated_at=NOW() WHERE id=$1")
        .bind(production_project_id)
        .bind(intent_status)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn json_uuid(value: Option<&Value>) -> Option<Uuid> {
    value
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn json_uuid_array(value: &Value) -> Option<Vec<Uuid>> {
    value
        .as_array()?
        .iter()
        .map(|item| json_uuid(Some(item)))
        .collect()
}

fn media_duration_ms(metadata: &Value) -> Option<u64> {
    metadata
        .get("duration_ms")
        .and_then(Value::as_u64)
        .or_else(|| {
            metadata
                .get("duration_sec")
                .and_then(Value::as_u64)
                .and_then(|seconds| seconds.checked_mul(1_000))
        })
}

fn usage_u64(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn work_generation_resource_request(usage: &Value) -> ResourceRequest {
    let video_tasks = usage_u64(usage, "video_task_count");
    let video_duration_sec = usage_u64(usage, "video_seconds");
    let tts_characters = usage_u64(usage, "tts_characters");
    let asr_tasks = u64::from(usage_u64(usage, "asr_seconds") > 0);
    let concurrency = u64::from(video_tasks > 0 || tts_characters > 0 || asr_tasks > 0);
    ResourceRequest::work_generation(
        video_tasks,
        video_duration_sec,
        tts_characters,
        asr_tasks,
        concurrency,
    )
}

fn is_provider_step(step_type: &str) -> bool {
    matches!(step_type, "video_segment" | "tts" | "asr")
}

fn provider_retry_resource_request(step_type: &str, usage: &Value) -> Option<ResourceRequest> {
    let mut values = BTreeMap::new();
    match step_type {
        "video_segment" => {
            values.insert("video_tasks".into(), 1);
            values.insert(
                "video_duration_sec".into(),
                usage_u64(usage, "video_seconds"),
            );
        }
        "tts" => {
            values.insert("tts_characters".into(), usage_u64(usage, "tts_characters"));
        }
        "asr" => {
            values.insert("asr_tasks".into(), 1);
        }
        _ => return None,
    }
    Some(ResourceRequest::provider_retry(values))
}

fn provider_retry_actual(step_type: &str, usage: &Value, started: bool) -> BTreeMap<String, u64> {
    let mut actual = BTreeMap::from([(String::from("provider_retries"), u64::from(started))]);
    match step_type {
        "video_segment" => {
            actual.insert("video_tasks".into(), u64::from(started));
            actual.insert(
                "video_duration_sec".into(),
                if started {
                    usage_u64(usage, "video_seconds")
                } else {
                    0
                },
            );
        }
        "tts" => {
            actual.insert(
                "tts_characters".into(),
                if started {
                    usage_u64(usage, "tts_characters")
                } else {
                    0
                },
            );
        }
        "asr" => {
            actual.insert("asr_tasks".into(), u64::from(started));
        }
        _ => {}
    }
    actual
}

fn add_attempt_usage(
    actual: &mut BTreeMap<String, u64>,
    step_type: &str,
    usage: &Value,
    started: bool,
) {
    if !started {
        return;
    }
    match step_type {
        "video_segment" => {
            *actual.entry("video_tasks".into()).or_default() += 1;
            *actual.entry("video_duration_sec".into()).or_default() +=
                usage_u64(usage, "video_seconds");
        }
        "tts" => {
            *actual.entry("tts_characters".into()).or_default() +=
                usage_u64(usage, "tts_characters");
        }
        "asr" => {
            *actual.entry("asr_tasks".into()).or_default() += 1;
        }
        _ => {}
    }
}

fn attempt_started(status: Option<&str>, upstream_task_id: Option<&str>) -> bool {
    matches!(
        status,
        Some("running" | "succeeded" | "failed" | "waiting_manual")
    ) || (status == Some("cancelled") && upstream_task_id.is_some())
}

#[allow(clippy::too_many_arguments)]
async fn create_rejection_epoch(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    package_type: PackageType,
    current_epoch: i32,
    package_id: Uuid,
    reason: &str,
    requested_owners: &[String],
    actor: &ProductionActor,
) -> ProductionResult<()> {
    if package_type == PackageType::Quality {
        return Err(ProductionError::TransitionConflict {
            reason: "quality reject must use the typed WorkVersion rework command".into(),
        });
    }
    let plan_value = sqlx::query_scalar::<_, Value>(
        r#"
        SELECT snapshot.plan
        FROM production_runs run
        JOIN production_plan_snapshots snapshot ON snapshot.id = run.plan_snapshot_id
        WHERE run.id = $1
        "#,
    )
    .bind(run_id)
    .fetch_one(&mut **tx)
    .await?;
    let plan: PlanSnapshot = serde_json::from_value(plan_value)?;
    let package_name = package_type_name(package_type);
    let limit = plan
        .max_package_revisions
        .get(package_name)
        .copied()
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: format!("plan has no revision limit for {package_name}"),
        })? as i64;
    let rejection_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM production_gate_decisions decision
        JOIN artifact_package_snapshots package ON package.id = decision.package_id
        WHERE decision.run_id = $1 AND decision.decision = 'rejected'
          AND package.package_type = $2
        "#,
    )
    .bind(run_id)
    .bind(package_name)
    .fetch_one(&mut **tx)
    .await?;
    if rejection_count > limit {
        sqlx::query(
            r#"
            UPDATE production_runs SET status = 'attention_required',
                error_code = 'revision_limit_reached', updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(run_id)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_projects SET status = 'attention_required', updated_at = NOW()
            WHERE id = (SELECT production_project_id FROM production_runs WHERE id = $1)
            "#,
        )
        .bind(run_id)
        .execute(&mut **tx)
        .await?;
        return Ok(());
    }

    let owners = match package_type {
        PackageType::Brief => vec!["producer".to_string()],
        PackageType::Script => vec!["screenwriter".to_string()],
        PackageType::Production => requested_owners.to_vec(),
        PackageType::Quality => unreachable!(),
    };
    let mut normalized_requested = requested_owners.to_vec();
    normalized_requested.sort();
    normalized_requested.dedup();
    let mut normalized_owners = owners.clone();
    normalized_owners.sort();
    normalized_owners.dedup();
    if normalized_requested != normalized_owners {
        return Err(ProductionError::TransitionConflict {
            reason: format!(
                "{} reject must name exactly the fixed package owners",
                package_type_name(package_type)
            ),
        });
    }
    if owners.is_empty()
        || owners.iter().any(|owner| {
            !plan
                .steps
                .iter()
                .any(|step| step.role_key.as_deref() == Some(owner.as_str()))
        })
    {
        return Err(ProductionError::TransitionConflict {
            reason: "reject contains an owner outside the fixed plan".into(),
        });
    }
    let first_owner_order = plan
        .steps
        .iter()
        .position(|step| {
            step.role_key
                .as_ref()
                .is_some_and(|role| owners.contains(role))
        })
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "fixed impact graph cannot locate affected owner".into(),
        })?;
    let next_epoch = current_epoch + 1;
    let reason_type = match package_type {
        PackageType::Brief => "brief_reject",
        PackageType::Script => "script_reject",
        PackageType::Production => "production_reject",
        PackageType::Quality => unreachable!(),
    };
    sqlx::query(
        r#"
        INSERT INTO production_revision_epochs (
            run_id, epoch, reason_type, reason, affected_owners, source_package_id,
            actor_type, actor_id
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(run_id)
    .bind(next_epoch)
    .bind(reason_type)
    .bind(reason)
    .bind(serde_json::to_value(&owners)?)
    .bind(package_id)
    .bind(&actor.actor_type)
    .bind(&actor.actor_id)
    .execute(&mut **tx)
    .await?;

    for (plan_order, step) in plan.steps.iter().enumerate() {
        let affected_owner = step
            .role_key
            .as_ref()
            .is_some_and(|role| owners.contains(role));
        let status = if affected_owner {
            "queued"
        } else if plan_order < first_owner_order {
            "succeeded"
        } else {
            "blocked"
        };
        let step_type = serde_json::to_value(step.kind)?
            .as_str()
            .unwrap_or_default()
            .to_string();
        sqlx::query(
            r#"
            INSERT INTO production_steps (
                run_id, revision_epoch, plan_order, step_key, step_type,
                role_key, dependencies, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(run_id)
        .bind(next_epoch)
        .bind(plan_order as i32)
        .bind(&step.key)
        .bind(step_type)
        .bind(&step.role_key)
        .bind(serde_json::to_value(&step.dependencies)?)
        .bind(status)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query(
        r#"
        UPDATE production_runs
        SET current_revision_epoch = $2, status = 'queued', updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .bind(next_epoch)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
