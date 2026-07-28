use crate::{
    durable::plan::{PlanSnapshot, StepKind},
    ProductionError, ProductionResult,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Created,
    Queued,
    Running,
    WaitingApproval,
    ExternalWait,
    Blocked,
    AttentionRequired,
    Cancelling,
    Cancelled,
    Failed,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Blocked,
    Queued,
    Running,
    WaitingApproval,
    ExternalWait,
    Succeeded,
    Failed,
    AttentionRequired,
    Cancelling,
    Cancelled,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectState {
    None,
    Prepared,
    Submitted,
    Confirmed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitingReason {
    Dependencies,
    PackageApproval,
    SceneVisualManifest,
    WorkPlanConfirmation,
    WorkGeneration,
    EvidenceIncomplete,
    ExternalFailure,
    ExternalCancelConflict,
    ManualAttention,
    CancellationPending,
    RevisionLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepState {
    pub id: Uuid,
    pub step_key: String,
    pub kind: StepKind,
    pub revision_epoch: u32,
    pub status: StepStatus,
    pub attempt: u32,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub side_effect_state: SideEffectState,
    pub waiting_reason: Option<WaitingReason>,
}

impl StepState {
    pub fn queued(id: Uuid, key: &str, kind: StepKind, revision_epoch: u32) -> Self {
        Self {
            id,
            step_key: key.into(),
            kind,
            revision_epoch,
            status: StepStatus::Queued,
            attempt: 0,
            lease_owner: None,
            lease_expires_at: None,
            side_effect_state: SideEffectState::None,
            waiting_reason: None,
        }
    }

    pub fn waiting_approval(id: Uuid, key: &str, kind: StepKind, revision_epoch: u32) -> Self {
        Self {
            status: StepStatus::WaitingApproval,
            waiting_reason: Some(WaitingReason::PackageApproval),
            ..Self::queued(id, key, kind, revision_epoch)
        }
    }

    pub fn transitioned(mut self, target: StepStatus) -> ProductionResult<Self> {
        self.transition(target)?;
        Ok(self)
    }

    pub fn transition(&mut self, target: StepStatus) -> ProductionResult<()> {
        let safe_to_cancel = !matches!(
            self.side_effect_state,
            SideEffectState::Submitted | SideEffectState::Unknown
        );
        let common_terminal = match target {
            StepStatus::Cancelled => {
                !matches!(
                    self.status,
                    StepStatus::Succeeded | StepStatus::Cancelled | StepStatus::Superseded
                ) && safe_to_cancel
            }
            StepStatus::Superseded => !matches!(
                self.status,
                StepStatus::Succeeded
                    | StepStatus::Cancelled
                    | StepStatus::Superseded
                    | StepStatus::Cancelling
            ),
            _ => false,
        };
        let allowed = common_terminal
            || match self.kind {
                StepKind::Role | StepKind::DomainCommand => match self.status {
                    StepStatus::Blocked => target == StepStatus::Queued,
                    StepStatus::Queued => target == StepStatus::Running,
                    StepStatus::Running => matches!(
                        target,
                        StepStatus::Succeeded
                            | StepStatus::Failed
                            | StepStatus::AttentionRequired
                            | StepStatus::Cancelling
                    ),
                    StepStatus::Failed => target == StepStatus::Queued,
                    StepStatus::AttentionRequired => {
                        target == StepStatus::Cancelling
                            || (target == StepStatus::Queued && safe_to_cancel)
                    }
                    StepStatus::Cancelling => matches!(
                        target,
                        StepStatus::Cancelled | StepStatus::AttentionRequired
                    ),
                    StepStatus::WaitingApproval
                    | StepStatus::ExternalWait
                    | StepStatus::Succeeded
                    | StepStatus::Cancelled
                    | StepStatus::Superseded => false,
                },
                StepKind::Gate => match self.status {
                    StepStatus::Blocked => target == StepStatus::Queued,
                    StepStatus::Queued => target == StepStatus::WaitingApproval,
                    StepStatus::WaitingApproval => {
                        matches!(target, StepStatus::Succeeded | StepStatus::Queued)
                    }
                    StepStatus::AttentionRequired => target == StepStatus::Cancelling,
                    StepStatus::Cancelling => matches!(
                        target,
                        StepStatus::Cancelled | StepStatus::AttentionRequired
                    ),
                    StepStatus::Running
                    | StepStatus::ExternalWait
                    | StepStatus::Succeeded
                    | StepStatus::Failed
                    | StepStatus::Cancelled
                    | StepStatus::Superseded => false,
                },
                StepKind::ExternalWait => match self.status {
                    StepStatus::Blocked => target == StepStatus::Queued,
                    StepStatus::Queued => target == StepStatus::ExternalWait,
                    StepStatus::ExternalWait => matches!(
                        target,
                        StepStatus::Succeeded
                            | StepStatus::Failed
                            | StepStatus::AttentionRequired
                            | StepStatus::Queued
                            | StepStatus::Cancelling
                    ),
                    StepStatus::Failed => target == StepStatus::Queued,
                    StepStatus::AttentionRequired => {
                        target == StepStatus::Cancelling
                            || (target == StepStatus::Queued && safe_to_cancel)
                    }
                    StepStatus::Cancelling => matches!(
                        target,
                        StepStatus::Cancelled | StepStatus::AttentionRequired
                    ),
                    StepStatus::Running
                    | StepStatus::WaitingApproval
                    | StepStatus::Succeeded
                    | StepStatus::Cancelled
                    | StepStatus::Superseded => false,
                },
            };
        if !allowed {
            return Err(transition_error(format!(
                "invalid {:?} step transition: {:?} -> {:?}",
                self.kind, self.status, target
            )));
        }
        self.status = target;
        if target != StepStatus::Running {
            self.lease_owner = None;
            self.lease_expires_at = None;
        }
        self.waiting_reason = match target {
            StepStatus::Blocked => Some(WaitingReason::Dependencies),
            StepStatus::WaitingApproval => Some(WaitingReason::PackageApproval),
            StepStatus::ExternalWait => Some(external_waiting_reason(&self.step_key)),
            StepStatus::AttentionRequired => Some(WaitingReason::ManualAttention),
            StepStatus::Cancelling => Some(WaitingReason::CancellationPending),
            _ => None,
        };
        Ok(())
    }

    pub fn derived_waiting_reason(&self) -> Option<WaitingReason> {
        self.waiting_reason.or_else(|| match self.status {
            StepStatus::Blocked => Some(WaitingReason::Dependencies),
            StepStatus::WaitingApproval => Some(WaitingReason::PackageApproval),
            StepStatus::ExternalWait => Some(external_waiting_reason(&self.step_key)),
            StepStatus::AttentionRequired => Some(WaitingReason::ManualAttention),
            StepStatus::Cancelling => Some(WaitingReason::CancellationPending),
            _ => None,
        })
    }

    pub fn claim(
        &mut self,
        owner: &str,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> ProductionResult<()> {
        if self.status != StepStatus::Queued
            || self.lease_owner.is_some()
            || ttl <= Duration::zero()
            || !matches!(self.kind, StepKind::Role | StepKind::DomainCommand)
        {
            return Err(transition_error("step is not claimable"));
        }
        self.transition(StepStatus::Running)?;
        self.attempt += 1;
        self.lease_owner = Some(owner.into());
        self.lease_expires_at = Some(now + ttl);
        Ok(())
    }

    pub fn reclaim_expired(
        &mut self,
        owner: &str,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> ProductionResult<()> {
        if self.status != StepStatus::Running
            || self.lease_expires_at.is_none_or(|expiry| expiry >= now)
        {
            return Err(transition_error("step lease is not expired"));
        }
        if matches!(
            self.side_effect_state,
            SideEffectState::Submitted | SideEffectState::Unknown
        ) {
            self.status = StepStatus::AttentionRequired;
            self.lease_owner = None;
            self.lease_expires_at = None;
            self.waiting_reason = Some(WaitingReason::ManualAttention);
            return Err(transition_error("external side effect result is uncertain"));
        }
        self.attempt += 1;
        self.lease_owner = Some(owner.into());
        self.lease_expires_at = Some(now + ttl);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunState {
    pub status: RunStatus,
    pub current_revision_epoch: u32,
    pub cancellation_requested: bool,
}

impl RunState {
    pub fn new(status: RunStatus, current_revision_epoch: u32) -> Self {
        Self {
            status,
            current_revision_epoch,
            cancellation_requested: matches!(status, RunStatus::Cancelling | RunStatus::Cancelled),
        }
    }

    pub fn cancelling(current_revision_epoch: u32) -> Self {
        Self::new(RunStatus::Cancelling, current_revision_epoch)
    }

    pub fn transition(&mut self, target: RunStatus) -> ProductionResult<()> {
        let allowed = match self.status {
            RunStatus::Created => matches!(
                target,
                RunStatus::Queued | RunStatus::Cancelling | RunStatus::Cancelled
            ),
            RunStatus::Queued | RunStatus::Running => matches!(
                target,
                RunStatus::Queued
                    | RunStatus::Running
                    | RunStatus::WaitingApproval
                    | RunStatus::ExternalWait
                    | RunStatus::Blocked
                    | RunStatus::AttentionRequired
                    | RunStatus::Cancelling
                    | RunStatus::Cancelled
                    | RunStatus::Failed
                    | RunStatus::Completed
            ),
            RunStatus::WaitingApproval => matches!(
                target,
                RunStatus::Queued
                    | RunStatus::AttentionRequired
                    | RunStatus::Cancelling
                    | RunStatus::Cancelled
                    | RunStatus::Failed
            ),
            RunStatus::ExternalWait => matches!(
                target,
                RunStatus::Queued
                    | RunStatus::Running
                    | RunStatus::Blocked
                    | RunStatus::AttentionRequired
                    | RunStatus::Cancelling
                    | RunStatus::Cancelled
                    | RunStatus::Failed
            ),
            RunStatus::Blocked => matches!(
                target,
                RunStatus::Queued
                    | RunStatus::AttentionRequired
                    | RunStatus::Cancelling
                    | RunStatus::Cancelled
                    | RunStatus::Failed
            ),
            RunStatus::AttentionRequired => matches!(
                target,
                RunStatus::Queued
                    | RunStatus::Cancelling
                    | RunStatus::Cancelled
                    | RunStatus::Failed
            ),
            RunStatus::Cancelling => {
                matches!(target, RunStatus::Cancelled | RunStatus::AttentionRequired)
            }
            RunStatus::Cancelled | RunStatus::Failed | RunStatus::Completed => false,
        };
        if !allowed {
            return Err(transition_error(format!(
                "invalid run transition: {:?} -> {:?}",
                self.status, target
            )));
        }
        self.status = target;
        self.cancellation_requested = self.cancellation_requested
            || matches!(target, RunStatus::Cancelling | RunStatus::Cancelled);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSnapshot {
    pub run: RunState,
    pub steps: Vec<StepState>,
}

impl WorkflowSnapshot {
    pub fn new(run: RunState, steps: Vec<StepState>) -> Self {
        Self { run, steps }
    }

    pub fn with_dependency_unlocks(&self, plan: &PlanSnapshot) -> ProductionResult<Self> {
        let unlocks: BTreeSet<_> = unlockable_steps(plan, self)?.into_iter().collect();
        let mut next = self.clone();
        for step in &mut next.steps {
            if step.revision_epoch == self.run.current_revision_epoch
                && step.status == StepStatus::Blocked
                && unlocks.contains(step.step_key.as_str())
            {
                step.transition(StepStatus::Queued)?;
            }
        }
        Ok(next)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCommandKind {
    ExecuteStep,
    EnterWait,
    ApprovePackage,
    RejectPackage,
    Resume,
    RetryStep,
    CancelRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCommand {
    pub kind: WorkflowCommandKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_key: Option<String>,
}

impl WorkflowCommand {
    pub fn step(kind: WorkflowCommandKind, step_key: impl Into<String>) -> Self {
        Self {
            kind,
            step_key: Some(step_key.into()),
        }
    }

    pub fn run(kind: WorkflowCommandKind) -> Self {
        Self {
            kind,
            step_key: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub plan_key: String,
    pub plan_version: String,
    pub plan_digest: String,
    pub idempotency_key: String,
    pub command: WorkflowCommand,
}

pub fn unlockable_steps(
    plan: &PlanSnapshot,
    workflow: &WorkflowSnapshot,
) -> ProductionResult<Vec<String>> {
    plan.validate_frozen()?;
    validate_step_snapshot(plan, workflow)?;
    if workflow.run.cancellation_requested
        || !matches!(workflow.run.status, RunStatus::Queued | RunStatus::Running)
    {
        return Ok(Vec::new());
    }
    Ok(workflow
        .steps
        .iter()
        .filter(|step| {
            step.revision_epoch == workflow.run.current_revision_epoch
                && step.status == StepStatus::Blocked
                && plan.step(&step.step_key).is_some_and(|definition| {
                    definition.dependencies.iter().all(|dependency| {
                        current_step(workflow, dependency)
                            .is_some_and(|value| value.status == StepStatus::Succeeded)
                    })
                })
        })
        .map(|step| step.step_key.clone())
        .collect())
}

pub fn allowed_commands(
    plan: &PlanSnapshot,
    workflow: &WorkflowSnapshot,
) -> ProductionResult<Vec<WorkflowCommand>> {
    plan.validate_frozen()?;
    validate_step_snapshot(plan, workflow)?;
    if matches!(
        workflow.run.status,
        RunStatus::Cancelling | RunStatus::Cancelled | RunStatus::Failed | RunStatus::Completed
    ) {
        return Ok(Vec::new());
    }

    let mut commands = Vec::new();
    if !workflow.run.cancellation_requested {
        for step in workflow
            .steps
            .iter()
            .filter(|step| step.revision_epoch == workflow.run.current_revision_epoch)
        {
            match (step.kind, step.status) {
                (StepKind::Role | StepKind::DomainCommand, StepStatus::Queued)
                    if dependencies_satisfied(plan, workflow, step)? =>
                {
                    commands.push(WorkflowCommand::step(
                        WorkflowCommandKind::ExecuteStep,
                        &step.step_key,
                    ));
                }
                (StepKind::Gate | StepKind::ExternalWait, StepStatus::Queued)
                    if dependencies_satisfied(plan, workflow, step)? =>
                {
                    commands.push(WorkflowCommand::step(
                        WorkflowCommandKind::EnterWait,
                        &step.step_key,
                    ));
                }
                (StepKind::Gate, StepStatus::WaitingApproval) => {
                    commands.push(WorkflowCommand::step(
                        WorkflowCommandKind::ApprovePackage,
                        &step.step_key,
                    ));
                    commands.push(WorkflowCommand::step(
                        WorkflowCommandKind::RejectPackage,
                        &step.step_key,
                    ));
                }
                (StepKind::ExternalWait, StepStatus::ExternalWait) => {
                    commands.push(WorkflowCommand::step(
                        WorkflowCommandKind::Resume,
                        &step.step_key,
                    ));
                }
                (_, StepStatus::Failed) => commands.push(WorkflowCommand::step(
                    WorkflowCommandKind::RetryStep,
                    &step.step_key,
                )),
                (_, StepStatus::AttentionRequired)
                    if matches!(
                        step.side_effect_state,
                        SideEffectState::None | SideEffectState::Prepared
                    ) =>
                {
                    commands.push(WorkflowCommand::step(
                        WorkflowCommandKind::RetryStep,
                        &step.step_key,
                    ));
                }
                _ => {}
            }
        }
    }
    commands.push(WorkflowCommand::run(WorkflowCommandKind::CancelRun));
    Ok(commands)
}

pub fn validate_workflow_command(
    plan: &PlanSnapshot,
    workflow: &WorkflowSnapshot,
    envelope: &CommandEnvelope,
) -> ProductionResult<()> {
    if envelope.idempotency_key.trim().is_empty() {
        return Err(ProductionError::IdempotencyConflict);
    }
    plan.validate_frozen()?;
    if envelope.plan_key != plan.plan_key
        || envelope.plan_version != plan.plan_version
        || envelope.plan_digest != plan.digest
    {
        return Err(transition_error(
            "command plan identity does not match the frozen Run plan",
        ));
    }
    if workflow.run.cancellation_requested
        && envelope.command.kind != WorkflowCommandKind::CancelRun
    {
        return Err(transition_error(
            "cancellation intent prevents new workflow commands",
        ));
    }
    if !allowed_commands(plan, workflow)?.contains(&envelope.command) {
        return Err(transition_error(
            "command is not allowed by the current Run/Step state",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionCause {
    BriefRejected,
    ScriptRejected,
    ProductionRejected,
    ScriptSemanticChange,
    ProductionExpressionChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionDirective {
    pub next_epoch: u32,
    pub reopen_owners: Vec<String>,
    pub invalidates_formal_script: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionOutcome {
    Open(RevisionDirective),
    LimitReached,
}

pub fn derive_revision(
    plan: &PlanSnapshot,
    current_epoch: u32,
    cause: RevisionCause,
    requested_owners: &[&str],
    completed_revisions: u32,
    accepted_semantic_suggestion: bool,
) -> ProductionResult<RevisionOutcome> {
    plan.validate_frozen()?;
    if cause == RevisionCause::ScriptSemanticChange && !accepted_semantic_suggestion {
        return Err(transition_error(
            "script semantic revision requires an accepted Director suggestion",
        ));
    }
    let (limit_key, fixed_owners, invalidates_formal_script) = match cause {
        RevisionCause::BriefRejected => ("brief", vec!["producer"], false),
        RevisionCause::ScriptRejected => ("script", vec!["screenwriter"], false),
        RevisionCause::ScriptSemanticChange => ("script", vec!["screenwriter"], true),
        RevisionCause::ProductionRejected => {
            if requested_owners.is_empty()
                || requested_owners.iter().any(|owner| {
                    !matches!(
                        *owner,
                        "director" | "performance_director" | "sound_director"
                    )
                })
            {
                return Err(transition_error(
                    "production reject contains an owner outside the fixed impact graph",
                ));
            }
            ("production", requested_owners.to_vec(), false)
        }
        RevisionCause::ProductionExpressionChange => {
            if requested_owners.len() != 1
                || !matches!(
                    requested_owners[0],
                    "director" | "cinematographer" | "performance_director" | "sound_director"
                )
            {
                return Err(transition_error(
                    "production expression revision requires exactly one process owner",
                ));
            }
            ("production", requested_owners.to_vec(), false)
        }
    };
    let mut normalized_requested = requested_owners.to_vec();
    normalized_requested.sort_unstable();
    normalized_requested.dedup();
    let mut normalized_fixed = fixed_owners;
    normalized_fixed.sort_unstable();
    normalized_fixed.dedup();
    if normalized_requested != normalized_fixed {
        return Err(transition_error(
            "revision owners do not match the fixed impact graph",
        ));
    }
    if normalized_fixed.iter().any(|owner| {
        !plan
            .steps
            .iter()
            .any(|step| step.role_key.as_deref() == Some(*owner))
    }) {
        return Err(transition_error(
            "revision owner is absent from the frozen plan",
        ));
    }
    let limit = plan
        .max_package_revisions
        .get(limit_key)
        .copied()
        .ok_or_else(|| transition_error("frozen plan has no revision limit"))?;
    if completed_revisions >= limit {
        return Ok(RevisionOutcome::LimitReached);
    }
    Ok(RevisionOutcome::Open(RevisionDirective {
        next_epoch: current_epoch
            .checked_add(1)
            .ok_or_else(|| transition_error("revision epoch overflow"))?,
        reopen_owners: normalized_fixed.into_iter().map(str::to_string).collect(),
        invalidates_formal_script,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentCommand {
    Delete,
    Archive,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentHistory {
    pub has_run: bool,
    pub has_artifacts: bool,
    pub has_domain_links: bool,
    pub run_terminal: bool,
}

pub fn validate_intent_command(
    command: IntentCommand,
    history: &IntentHistory,
) -> ProductionResult<()> {
    let has_history = history.has_run || history.has_artifacts || history.has_domain_links;
    match command {
        IntentCommand::Delete if !has_history => Ok(()),
        IntentCommand::Delete => Err(transition_error(
            "a ProductionProject with history cannot be deleted",
        )),
        IntentCommand::Archive if has_history && history.run_terminal => Ok(()),
        IntentCommand::Archive if !has_history => Err(transition_error(
            "an empty ProductionProject should be deleted instead of archived",
        )),
        IntentCommand::Archive => Err(transition_error(
            "an active ProductionProject must terminate before archival",
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkGenerationStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    WaitingManual,
    UnknownSubmission,
    Cancelling,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalWorkDecision {
    pub run_status: RunStatus,
    pub waiting_reason: Option<WaitingReason>,
    pub unlocks_editor: bool,
}

pub fn external_work_decision(
    status: WorkGenerationStatus,
    final_media_present: bool,
    inventory_complete: bool,
    cancellation_requested: bool,
) -> ExternalWorkDecision {
    match status {
        WorkGenerationStatus::Queued | WorkGenerationStatus::Running => ExternalWorkDecision {
            run_status: RunStatus::ExternalWait,
            waiting_reason: Some(WaitingReason::WorkGeneration),
            unlocks_editor: false,
        },
        WorkGenerationStatus::Succeeded if final_media_present && inventory_complete => {
            ExternalWorkDecision {
                run_status: RunStatus::Running,
                waiting_reason: None,
                unlocks_editor: true,
            }
        }
        WorkGenerationStatus::Succeeded => ExternalWorkDecision {
            run_status: RunStatus::Blocked,
            waiting_reason: Some(WaitingReason::EvidenceIncomplete),
            unlocks_editor: false,
        },
        WorkGenerationStatus::Failed => ExternalWorkDecision {
            run_status: RunStatus::Blocked,
            waiting_reason: Some(WaitingReason::ExternalFailure),
            unlocks_editor: false,
        },
        WorkGenerationStatus::WaitingManual | WorkGenerationStatus::UnknownSubmission => {
            ExternalWorkDecision {
                run_status: RunStatus::AttentionRequired,
                waiting_reason: Some(WaitingReason::ManualAttention),
                unlocks_editor: false,
            }
        }
        WorkGenerationStatus::Cancelling => ExternalWorkDecision {
            run_status: RunStatus::Cancelling,
            waiting_reason: Some(WaitingReason::CancellationPending),
            unlocks_editor: false,
        },
        WorkGenerationStatus::Cancelled if cancellation_requested => ExternalWorkDecision {
            run_status: RunStatus::Cancelled,
            waiting_reason: None,
            unlocks_editor: false,
        },
        WorkGenerationStatus::Cancelled => ExternalWorkDecision {
            run_status: RunStatus::Blocked,
            waiting_reason: Some(WaitingReason::ExternalCancelConflict),
            unlocks_editor: false,
        },
    }
}

/// 显式映射外部技术状态。HTTP 202 从不作为本函数输入或成功证据。
pub fn external_work_state(
    status: WorkGenerationStatus,
    final_media_present: bool,
    inventory_complete: bool,
    cancellation_requested: bool,
) -> RunStatus {
    external_work_decision(
        status,
        final_media_present,
        inventory_complete,
        cancellation_requested,
    )
    .run_status
}

fn validate_step_snapshot(
    plan: &PlanSnapshot,
    workflow: &WorkflowSnapshot,
) -> ProductionResult<()> {
    let mut identities = BTreeSet::new();
    for step in &workflow.steps {
        if !identities.insert((step.revision_epoch, step.step_key.as_str())) {
            return Err(transition_error(
                "workflow snapshot contains a duplicate step identity",
            ));
        }
        let definition = plan
            .step(&step.step_key)
            .ok_or_else(|| transition_error("workflow step is absent from the frozen plan"))?;
        if definition.kind != step.kind {
            return Err(transition_error(
                "workflow step kind differs from the frozen plan",
            ));
        }
    }
    Ok(())
}

fn current_step<'a>(workflow: &'a WorkflowSnapshot, step_key: &str) -> Option<&'a StepState> {
    workflow.steps.iter().find(|step| {
        step.revision_epoch == workflow.run.current_revision_epoch && step.step_key == step_key
    })
}

fn dependencies_satisfied(
    plan: &PlanSnapshot,
    workflow: &WorkflowSnapshot,
    step: &StepState,
) -> ProductionResult<bool> {
    if step.revision_epoch != workflow.run.current_revision_epoch {
        return Ok(false);
    }
    let definition = plan
        .step(&step.step_key)
        .ok_or_else(|| transition_error("workflow step is absent from the frozen plan"))?;
    Ok(definition.dependencies.iter().all(|dependency| {
        current_step(workflow, dependency)
            .is_some_and(|value| value.status == StepStatus::Succeeded)
    }))
}

fn external_waiting_reason(step_key: &str) -> WaitingReason {
    match step_key {
        "wait_scene_visual_manifest" => WaitingReason::SceneVisualManifest,
        "work_plan_confirmation" => WaitingReason::WorkPlanConfirmation,
        "wait_work_generation" => WaitingReason::WorkGeneration,
        _ => WaitingReason::ManualAttention,
    }
}

fn transition_error(reason: impl Into<String>) -> ProductionError {
    ProductionError::TransitionConflict {
        reason: reason.into(),
    }
}
