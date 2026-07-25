use async_trait::async_trait;
use novex_agent::{
    AuditedCallOwner, AuditedModelError, AuditedModelExecutor, AuditedModelRequest,
    AuditedTerminalStatus, BoundModelResolver, FinishAuditedCall, FixedModelBinding,
    ModelCallAuditStore, PrepareAuditedCall, ResolvedBoundModel,
};
use novex_ai_core::{
    DefinitionRegistry, DynamicFragment, ModelCapabilities, PromptCompileInput, TrustLevel,
};
use novex_model::{LLMClient, LLMError, LLMPrompt};
use serde_json::json;
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
struct FakeClient {
    calls: Arc<AtomicUsize>,
    result: Result<String, LLMError>,
}

#[async_trait]
impl LLMClient for FakeClient {
    async fn generate_script(&self, _prompt: LLMPrompt) -> Result<String, LLMError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}

struct FakeResolver {
    resolved: ResolvedBoundModel,
}

#[async_trait]
impl BoundModelResolver for FakeResolver {
    async fn resolve(&self, _model_id: Uuid) -> Result<ResolvedBoundModel, novex_agent::BoxError> {
        Ok(self.resolved.clone())
    }
}

struct SequenceResolver {
    resolved: Mutex<VecDeque<ResolvedBoundModel>>,
    calls: AtomicUsize,
}

#[async_trait]
impl BoundModelResolver for SequenceResolver {
    async fn resolve(&self, _model_id: Uuid) -> Result<ResolvedBoundModel, novex_agent::BoxError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.resolved
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| "missing resolved model fixture".into())
    }
}

#[derive(Default)]
struct FakeAudit {
    fail_prepare: bool,
    prepared: Mutex<Vec<PrepareAuditedCall>>,
    finished: Mutex<Vec<FinishAuditedCall>>,
}

#[async_trait]
impl ModelCallAuditStore for FakeAudit {
    async fn prepare(&self, input: PrepareAuditedCall) -> Result<Uuid, novex_agent::BoxError> {
        if self.fail_prepare {
            return Err("injected audit failure".into());
        }
        self.prepared.lock().unwrap().push(input);
        Ok(Uuid::new_v4())
    }

    async fn associate_step(
        &self,
        _model_call_id: Uuid,
        _step_id: Uuid,
    ) -> Result<(), novex_agent::BoxError> {
        Ok(())
    }

    async fn finish(&self, input: FinishAuditedCall) -> Result<(), novex_agent::BoxError> {
        self.finished.lock().unwrap().push(input);
        Ok(())
    }
}

fn registry() -> Arc<DefinitionRegistry> {
    Arc::new(
        DefinitionRegistry::load(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("agent-definitions"),
        )
        .unwrap(),
    )
}

fn request(model_id: Uuid, fingerprint: &str) -> AuditedModelRequest {
    AuditedModelRequest {
        owner: AuditedCallOwner::AgentRun(Uuid::new_v4()),
        step_id: None,
        root_call_id: None,
        parent_call_id: None,
        attempt: 1,
        agent_key: "video.script".into(),
        agent_version: "1.0.0".into(),
        node_key: "script.complete".into(),
        compile_input: PromptCompileInput {
            schema_version: "1".into(),
            variables: BTreeMap::from([("scene_count".into(), json!(3))]),
            fragments: vec![DynamicFragment {
                id: "input-1".into(),
                trust: TrustLevel::UserInstruction,
                source: "test".into(),
                content: Some("write a script".into()),
                asset: None,
            }],
        },
        tool_profile: "chat".into(),
        tool_schema: None,
        binding: FixedModelBinding {
            model_id,
            behavior_fingerprint: fingerprint.into(),
        },
        context_sources: json!([{"id":"input-1","source":"test"}]),
        memory_sources: json!([]),
        parameters: json!({"temperature":0.8}),
        asset_references: json!([]),
    }
}

fn resolved(client: Arc<dyn LLMClient>, model_id: Uuid, fingerprint: &str) -> ResolvedBoundModel {
    ResolvedBoundModel {
        client,
        model_id,
        behavior_fingerprint: fingerprint.into(),
        capabilities: ModelCapabilities {
            text: true,
            tool_calling: false,
            structured_output: true,
            vision: false,
            reasoning: false,
            context_window: 128_000,
        },
        model_snapshot: json!({"provider":"fake"}),
        known_secrets: vec!["fake-secret".into()],
    }
}

#[tokio::test]
async fn persists_prepared_before_provider_and_finishes_success_or_failure_once() {
    let model_id = Uuid::new_v4();
    let fingerprint = "b".repeat(64);
    let calls = Arc::new(AtomicUsize::new(0));
    let audit = Arc::new(FakeAudit::default());
    let executor = AuditedModelExecutor::new(
        registry(),
        Arc::new(FakeResolver {
            resolved: resolved(
                Arc::new(FakeClient {
                    calls: calls.clone(),
                    result: Ok("output".into()),
                }),
                model_id,
                &fingerprint,
            ),
        }),
        audit.clone(),
    );

    let response = executor
        .execute(request(model_id, &fingerprint))
        .await
        .unwrap();
    assert_eq!(response.output, "output");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(audit.prepared.lock().unwrap().len(), 1);
    assert_eq!(
        audit.finished.lock().unwrap()[0].status,
        AuditedTerminalStatus::Succeeded
    );

    let failed_audit = Arc::new(FakeAudit::default());
    let failed = AuditedModelExecutor::new(
        registry(),
        Arc::new(FakeResolver {
            resolved: resolved(
                Arc::new(FakeClient {
                    calls: calls.clone(),
                    result: Err(LLMError::Provider("provider failed".into())),
                }),
                model_id,
                &fingerprint,
            ),
        }),
        failed_audit.clone(),
    );
    assert!(matches!(
        failed.execute(request(model_id, &fingerprint)).await,
        Err(AuditedModelError::Provider { .. })
    ));
    assert_eq!(
        failed_audit.finished.lock().unwrap()[0].status,
        AuditedTerminalStatus::Failed
    );
}

#[tokio::test]
async fn compile_binding_or_prepared_failure_never_calls_provider() {
    let model_id = Uuid::new_v4();
    let fingerprint = "b".repeat(64);
    let calls = Arc::new(AtomicUsize::new(0));
    let client = Arc::new(FakeClient {
        calls: calls.clone(),
        result: Ok("must not run".into()),
    });
    let audit = Arc::new(FakeAudit {
        fail_prepare: true,
        ..FakeAudit::default()
    });
    let executor = AuditedModelExecutor::new(
        registry(),
        Arc::new(FakeResolver {
            resolved: resolved(client.clone(), model_id, &fingerprint),
        }),
        audit,
    );
    assert!(matches!(
        executor.execute(request(model_id, &fingerprint)).await,
        Err(AuditedModelError::PrepareAudit(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let drifted = AuditedModelExecutor::new(
        registry(),
        Arc::new(FakeResolver {
            resolved: resolved(client, model_id, &"c".repeat(64)),
        }),
        Arc::new(FakeAudit::default()),
    );
    assert!(matches!(
        drifted.execute(request(model_id, &fingerprint)).await,
        Err(AuditedModelError::ModelRebindRequired)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let mut incompatible = resolved(
        Arc::new(FakeClient {
            calls: calls.clone(),
            result: Ok("must not run".into()),
        }),
        model_id,
        &fingerprint,
    );
    incompatible.capabilities.structured_output = false;
    let incompatible = AuditedModelExecutor::new(
        registry(),
        Arc::new(FakeResolver {
            resolved: incompatible,
        }),
        Arc::new(FakeAudit::default()),
    );
    assert!(matches!(
        incompatible.execute(request(model_id, &fingerprint)).await,
        Err(AuditedModelError::ModelCapabilityMismatch)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn resolves_the_bound_model_again_before_every_provider_call() {
    let model_id = Uuid::new_v4();
    let fingerprint = "b".repeat(64);
    let provider_calls = Arc::new(AtomicUsize::new(0));
    let first = resolved(
        Arc::new(FakeClient {
            calls: provider_calls.clone(),
            result: Ok("first credential".into()),
        }),
        model_id,
        &fingerprint,
    );
    let mut rotated = resolved(
        Arc::new(FakeClient {
            calls: provider_calls.clone(),
            result: Ok("rotated credential".into()),
        }),
        model_id,
        &fingerprint,
    );
    rotated.known_secrets = vec!["rotated-secret".into()];
    let resolver = Arc::new(SequenceResolver {
        resolved: Mutex::new(VecDeque::from([first, rotated])),
        calls: AtomicUsize::new(0),
    });
    let executor =
        AuditedModelExecutor::new(registry(), resolver.clone(), Arc::new(FakeAudit::default()));

    assert_eq!(
        executor
            .execute(request(model_id, &fingerprint))
            .await
            .unwrap()
            .output,
        "first credential"
    );
    assert_eq!(
        executor
            .execute(request(model_id, &fingerprint))
            .await
            .unwrap()
            .output,
        "rotated credential"
    );
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 2);
    assert_eq!(provider_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn structured_parse_failure_finishes_the_same_call_as_failed_with_output_evidence() {
    let model_id = Uuid::new_v4();
    let fingerprint = "b".repeat(64);
    let audit = Arc::new(FakeAudit::default());
    let executor = AuditedModelExecutor::new(
        registry(),
        Arc::new(FakeResolver {
            resolved: resolved(
                Arc::new(FakeClient {
                    calls: Arc::new(AtomicUsize::new(0)),
                    result: Ok("not-json".into()),
                }),
                model_id,
                &fingerprint,
            ),
        }),
        audit.clone(),
    );

    let error = executor
        .execute_parsed(request(model_id, &fingerprint), |raw| {
            serde_json::from_str::<serde_json::Value>(raw).map_err(|error| error.to_string())
        })
        .await
        .unwrap_err();

    assert!(matches!(error, AuditedModelError::StructuredParse { .. }));
    let finished = audit.finished.lock().unwrap();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].status, AuditedTerminalStatus::Failed);
    assert_eq!(
        finished[0].structured_parse_status.as_deref(),
        Some("failed")
    );
    assert_eq!(
        finished[0].output_snapshot,
        Some(json!({"text":"not-json"}))
    );
    assert_eq!(
        finished[0].error_snapshot.as_ref().unwrap()["kind"],
        "structured_parse"
    );
}
