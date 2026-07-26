use async_trait::async_trait;
use chrono::Utc;
use novex_agent::*;
use novex_ai_core::AgentKey;
use novex_model::{ApiProtocol, ModelExecutionSnapshot, ModelType};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Default)]
struct FakeState {
    session: Option<AgentSession>,
    messages: Vec<NewMessage>,
    runs: Vec<StartRun>,
    finishes: Vec<FinishRun>,
    steps: Vec<AgentStep>,
    fail_assistant_save: bool,
    fail_finish: bool,
}

#[derive(Clone, Default)]
struct FakePorts(Arc<Mutex<FakeState>>);

#[derive(Debug)]
struct FakeError(&'static str);

impl std::fmt::Display for FakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for FakeError {}

#[async_trait]
impl SessionStore for FakePorts {
    async fn load_session(&self, id: Uuid) -> Result<AgentSession, BoxError> {
        self.0
            .lock()
            .unwrap()
            .session
            .clone()
            .filter(|session| session.id == id)
            .ok_or_else(|| Box::new(FakeError("session missing")) as BoxError)
    }

    async fn save_message(&self, input: NewMessage) -> Result<StoredMessage, BoxError> {
        let mut state = self.0.lock().unwrap();
        if input.role == MessageRole::Assistant && state.fail_assistant_save {
            return Err(Box::new(FakeError("assistant save failed")));
        }
        state.messages.push(input.clone());
        Ok(StoredMessage {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            role: input.role,
            content: input.content,
            metadata: input.metadata,
            created_at: Utc::now(),
        })
    }
}

#[async_trait]
impl RunRecorder for FakePorts {
    async fn start_run(&self, input: StartRun) -> Result<RunRecord, BoxError> {
        self.0.lock().unwrap().runs.push(input.clone());
        Ok(RunRecord {
            id: Uuid::new_v4(),
            project_id: input.project_id,
            agent_key: input.agent_key,
            status: "running".into(),
            input: input.input,
            output: None,
            error_message: None,
            context_compile_attempt_id: None,
            model_id: input.model_id,
            model_snapshot: input.model_snapshot,
            started_at: Utc::now(),
            ended_at: None,
        })
    }

    async fn finish_run(&self, input: FinishRun) -> Result<RunRecord, BoxError> {
        let mut state = self.0.lock().unwrap();
        state.finishes.push(input.clone());
        if state.fail_finish {
            return Err(Box::new(FakeError("run finish failed")));
        }
        Ok(RunRecord {
            id: input.run_id,
            project_id: None,
            agent_key: AgentKey::new("test").unwrap(),
            status: input.status.as_str().into(),
            input: json!({}),
            output: input.output,
            error_message: input.error_message,
            context_compile_attempt_id: input.context_compile_attempt_id,
            model_id: None,
            model_snapshot: None,
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
        })
    }
}

#[async_trait]
impl StepRecorder for FakePorts {
    async fn record_step(&self, step: AgentStep) -> Result<Uuid, BoxError> {
        self.0.lock().unwrap().steps.push(step);
        Ok(Uuid::new_v4())
    }
}

struct TestAdapter {
    key: AgentKey,
    outcome: Result<AgentOutcome, &'static str>,
    calls: Arc<Mutex<Vec<Value>>>,
}

#[async_trait]
impl AgentAdapter for TestAdapter {
    fn key(&self) -> &AgentKey {
        &self.key
    }

    async fn execute(
        &self,
        invocation: &AgentInvocation,
        context: &AgentExecutionContext,
    ) -> Result<AgentOutcome, BoxError> {
        self.calls.lock().unwrap().push(invocation.payload.clone());
        context
            .steps
            .record_step(AgentStep {
                run_id: context.run_id,
                order: 1,
                step_type: "test_step".into(),
                status: "succeeded".into(),
                input: json!({}),
                output: None,
                error_message: None,
            })
            .await?;
        self.outcome
            .clone()
            .map_err(|message| Box::new(FakeError(message)) as BoxError)
    }
}

fn model() -> ModelExecutionRef {
    ModelExecutionRef {
        audited: None,
        snapshot: Some(ModelExecutionSnapshot {
            model_id: Uuid::new_v4(),
            display_name: "fixture".into(),
            model_type: ModelType::Text,
            provider_name: "fixture".into(),
            api_protocol: ApiProtocol::OpenAiResponses,
            protocol_version: "v1".into(),
            request_base_url: "http://fixture".into(),
            upstream_model: "fixture".into(),
            reasoning_effort: None,
            timeout_seconds: 30,
            max_output_tokens: None,
            context_window: None,
            tokenizer_profile_key: None,
            tokenizer_profile_version: None,
            settings: json!({}),
        }),
    }
}

fn invocation(session_id: Uuid) -> AgentInvocation {
    AgentInvocation {
        session_id,
        user_message: " hello ".into(),
        user_metadata: json!({}),
        run_input: json!({}),
        payload: json!({"strict_value": 7}),
    }
}

fn standalone_run() -> StartRun {
    StartRun {
        session_id: Uuid::new_v4(),
        project_id: Some(Uuid::new_v4()),
        agent_key: AgentKey::new("test").unwrap(),
        input: json!({"intent": "standalone"}),
        model_id: None,
        model_snapshot: None,
    }
}

fn fixture(
    outcome: Result<AgentOutcome, &'static str>,
) -> (AgentRunCoordinator, FakePorts, Arc<Mutex<Vec<Value>>>, Uuid) {
    let session_id = Uuid::new_v4();
    let ports = FakePorts::default();
    ports.0.lock().unwrap().session = Some(AgentSession {
        id: session_id,
        project_id: Some(Uuid::new_v4()),
        agent_key: AgentKey::new("test").unwrap(),
        subject_type: None,
        subject_id: None,
        metadata: json!({}),
    });
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut registry = AgentRegistry::new();
    registry
        .register(Arc::new(TestAdapter {
            key: AgentKey::new("test").unwrap(),
            outcome,
            calls: calls.clone(),
        }))
        .unwrap();
    let coordinator = AgentRunCoordinator::new(
        Arc::new(registry),
        Arc::new(ports.clone()),
        Arc::new(ports.clone()),
        Arc::new(ports.clone()),
    );
    (coordinator, ports, calls, session_id)
}

#[test]
fn registry_rejects_duplicates_mismatches_and_resolves_new_adapters() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let adapter = || {
        Arc::new(TestAdapter {
            key: AgentKey::new("test").unwrap(),
            outcome: Ok(AgentOutcome::new("ok", json!({}))),
            calls: calls.clone(),
        })
    };
    let mut registry = AgentRegistry::new();
    registry.register(adapter()).unwrap();
    assert!(matches!(
        registry.register(adapter()),
        Err(RegistryError::DuplicateKey(_))
    ));
    assert!(matches!(
        registry.register_as(AgentKey::new("other").unwrap(), adapter()),
        Err(RegistryError::KeyMismatch { .. })
    ));
    assert!(registry.resolve(&AgentKey::new("test").unwrap()).is_some());
    assert!(registry
        .resolve(&AgentKey::new("unknown").unwrap())
        .is_none());
}

#[tokio::test]
async fn coordinator_success_preserves_payload_and_finishes_once() {
    let (coordinator, ports, calls, session_id) = fixture(Ok(AgentOutcome::new(
        "assistant",
        json!({"kind": "fixture"}),
    )));
    let result = coordinator
        .execute(invocation(session_id), model())
        .await
        .unwrap();
    assert_eq!(result.user_message.content, "hello");
    assert_eq!(result.assistant_message.content, "assistant");
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[json!({"strict_value": 7})]
    );
    let state = ports.0.lock().unwrap();
    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.finishes.len(), 1);
    assert_eq!(state.finishes[0].status, RunStatus::Succeeded);
    assert_eq!(state.steps.len(), 1);
}

#[tokio::test]
async fn coordinator_adapter_failure_has_no_fake_reply_and_finishes_once() {
    let (coordinator, ports, _, session_id) = fixture(Err("adapter failed"));
    assert!(matches!(
        coordinator.execute(invocation(session_id), model()).await,
        Err(KernelError::Adapter(_))
    ));
    let state = ports.0.lock().unwrap();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.finishes.len(), 1);
    assert_eq!(state.finishes[0].status, RunStatus::Failed);
    assert_eq!(
        state.finishes[0].error_message.as_deref(),
        Some("adapter failed")
    );
}

#[tokio::test]
async fn coordinator_assistant_save_failure_finishes_failed_once() {
    let (coordinator, ports, _, session_id) =
        fixture(Ok(AgentOutcome::new("assistant", json!({}))));
    ports.0.lock().unwrap().fail_assistant_save = true;
    assert!(matches!(
        coordinator.execute(invocation(session_id), model()).await,
        Err(KernelError::Store(_))
    ));
    let state = ports.0.lock().unwrap();
    assert_eq!(state.finishes.len(), 1);
    assert_eq!(state.finishes[0].status, RunStatus::Failed);
}

#[tokio::test]
async fn unknown_agent_is_rejected_before_messages_or_run() {
    let (coordinator, ports, _, session_id) =
        fixture(Ok(AgentOutcome::new("assistant", json!({}))));
    ports.0.lock().unwrap().session.as_mut().unwrap().agent_key = AgentKey::new("unknown").unwrap();
    assert!(matches!(
        coordinator.execute(invocation(session_id), model()).await,
        Err(KernelError::UnsupportedAgent(key)) if key.as_str() == "unknown"
    ));
    let state = ports.0.lock().unwrap();
    assert!(state.messages.is_empty());
    assert!(state.runs.is_empty());
}

#[tokio::test]
async fn run_lifecycle_success_finishes_once_with_mapped_output() {
    let ports = FakePorts::default();
    let coordinator = RunLifecycleCoordinator::new(Arc::new(ports.clone()));

    let result = coordinator
        .execute(
            standalone_run(),
            |run_id| async move { Ok::<_, FakeError>((run_id, "done")) },
            |(_, value)| Some(json!({"result": value})),
            |error| error.to_string(),
        )
        .await
        .unwrap();

    assert_eq!(result.1, "done");
    let state = ports.0.lock().unwrap();
    assert_eq!(state.runs.len(), 1);
    assert_eq!(state.finishes.len(), 1);
    assert_eq!(state.finishes[0].status, RunStatus::Succeeded);
    assert_eq!(state.finishes[0].output, Some(json!({"result": "done"})));
}

#[tokio::test]
async fn run_lifecycle_operation_failure_preserves_error_and_finishes_once() {
    let ports = FakePorts::default();
    let coordinator = RunLifecycleCoordinator::new(Arc::new(ports.clone()));

    let result = coordinator
        .execute(
            standalone_run(),
            |_| async { Err::<(), _>(FakeError("operation failed")) },
            |_| None,
            |_| "audit failure".to_string(),
        )
        .await;

    assert!(matches!(
        result,
        Err(RunLifecycleError::Operation(FakeError("operation failed")))
    ));
    let state = ports.0.lock().unwrap();
    assert_eq!(state.finishes.len(), 1);
    assert_eq!(state.finishes[0].status, RunStatus::Failed);
    assert_eq!(
        state.finishes[0].error_message.as_deref(),
        Some("audit failure")
    );
}

#[tokio::test]
async fn run_lifecycle_success_finish_failure_returns_store_error_once() {
    let ports = FakePorts::default();
    ports.0.lock().unwrap().fail_finish = true;
    let coordinator = RunLifecycleCoordinator::new(Arc::new(ports.clone()));

    let result = coordinator
        .execute(
            standalone_run(),
            |_| async { Ok::<_, FakeError>(()) },
            |_| Some(json!({"done": true})),
            |error| error.to_string(),
        )
        .await;

    assert!(matches!(result, Err(RunLifecycleError::Store(_))));
    let state = ports.0.lock().unwrap();
    assert_eq!(state.finishes.len(), 1);
    assert_eq!(state.finishes[0].status, RunStatus::Succeeded);
}

#[test]
fn invocation_contract_has_no_business_specific_fields() {
    let public_fields = HashMap::from([
        ("session_id", "common"),
        ("user_message", "common"),
        ("user_metadata", "common"),
        ("run_input", "common"),
        ("payload", "adapter-owned"),
    ]);
    assert!(!public_fields.contains_key("supplement_of_batch_id"));
    assert!(!public_fields.contains_key("sound_context"));
}
