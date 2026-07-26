use async_trait::async_trait;
use novex_agent::{
    text_context_candidate, AuditedCallOwner, AuditedModelError, AuditedModelExecutor,
    AuditedModelRequest, AuditedTerminalStatus, BoundModelResolver, ContextAuditStore,
    FinishAuditedCall, FixedModelBinding, ModelCallAuditStore, PersistContextCompileAttempt,
    PersistContextSnapshot, PrepareAuditedCall, PrepareAuditedCallWithContext, ResolvedBoundModel,
    TextContextCandidateInput,
};
use novex_ai_core::{
    definition_digest, ContextPriority, DefinitionRegistry, ModelCapabilities, TrustLevel,
};
use novex_model::{LLMClient, LLMError, LLMPrompt};
use serde_json::json;
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
struct FakeClient {
    calls: Arc<AtomicUsize>,
    result: Result<String, LLMError>,
}

#[derive(Clone)]
struct OrderedClient {
    events: Arc<Mutex<Vec<&'static str>>>,
    prompts: Arc<Mutex<Vec<LLMPrompt>>>,
}

#[async_trait]
impl LLMClient for OrderedClient {
    async fn generate_script(&self, prompt: LLMPrompt) -> Result<String, LLMError> {
        self.events.lock().unwrap().push("provider");
        self.prompts.lock().unwrap().push(prompt);
        Ok("governed output".into())
    }
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
    contexts: Mutex<Vec<PersistContextSnapshot>>,
    attempts: Mutex<Vec<PersistContextCompileAttempt>>,
    failure_links: Mutex<Vec<(AuditedCallOwner, Uuid, Option<Uuid>)>>,
    binding_blocked: AtomicBool,
    finished: Mutex<Vec<FinishAuditedCall>>,
    events: Option<Arc<Mutex<Vec<&'static str>>>>,
}

#[async_trait]
impl ModelCallAuditStore for FakeAudit {
    async fn prepare_with_context(
        &self,
        input: PrepareAuditedCallWithContext,
    ) -> Result<Uuid, novex_agent::BoxError> {
        if self.fail_prepare {
            return Err("injected audit failure".into());
        }
        self.prepared.lock().unwrap().push(input.model_call);
        self.contexts.lock().unwrap().push(input.context);
        if let Some(events) = &self.events {
            events.lock().unwrap().push("snapshot_and_model_call");
        }
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

#[async_trait]
impl ContextAuditStore for FakeAudit {
    async fn binding_is_executable(
        &self,
        _owner: AuditedCallOwner,
    ) -> Result<bool, novex_agent::BoxError> {
        Ok(!self.binding_blocked.load(Ordering::SeqCst))
    }

    async fn block_tokenizer_profile_binding(
        &self,
        _owner: AuditedCallOwner,
    ) -> Result<(), novex_agent::BoxError> {
        self.binding_blocked.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn persist_snapshot(
        &self,
        _input: PersistContextSnapshot,
    ) -> Result<Uuid, novex_agent::BoxError> {
        Ok(Uuid::new_v4())
    }

    async fn persist_attempt(
        &self,
        input: PersistContextCompileAttempt,
    ) -> Result<Uuid, novex_agent::BoxError> {
        self.attempts.lock().unwrap().push(input);
        Ok(Uuid::new_v4())
    }

    async fn link_failure(
        &self,
        owner: AuditedCallOwner,
        attempt_id: Uuid,
        step_id: Option<Uuid>,
    ) -> Result<(), novex_agent::BoxError> {
        self.failure_links
            .lock()
            .unwrap()
            .push((owner, attempt_id, step_id));
        Ok(())
    }
}

#[tokio::test]
async fn context_budget_failure_persists_only_a_compile_attempt() {
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
                    result: Ok("must not run".into()),
                }),
                model_id,
                &fingerprint,
            ),
        }),
        audit.clone(),
        audit.clone(),
    );
    let mut governed_request = request(model_id, &fingerprint);
    governed_request.context_candidates[0] = text_context_candidate(TextContextCandidateInput {
        candidate_id: "input-1".into(),
        source_kind: "user_instruction".into(),
        source_id: "test".into(),
        source_version: "1".into(),
        trust: TrustLevel::UserInstruction,
        priority: ContextPriority::P0,
        required: true,
        render_order: 0,
        observed_at: "2026-01-01T00:00:00Z".into(),
        text: "x".repeat(200_000),
    });

    assert!(matches!(
        executor.execute(governed_request).await,
        Err(AuditedModelError::ContextCompile(code)) if code == "context_budget_exceeded"
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(audit.prepared.lock().unwrap().is_empty());
    assert!(audit.contexts.lock().unwrap().is_empty());
    let attempts = audit.attempts.lock().unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].attempt.code, "context_budget_exceeded");
    assert_eq!(audit.failure_links.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn schema_conflict_and_tokenizer_failures_persist_attempts_without_model_calls() {
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
                    result: Ok("must not run".into()),
                }),
                model_id,
                &fingerprint,
            ),
        }),
        audit.clone(),
        audit.clone(),
    );

    let mut schema_failure = request(model_id, &fingerprint);
    schema_failure.context_candidates[0].content_hash = "a".repeat(64);
    assert!(matches!(
        executor.execute(schema_failure).await,
        Err(AuditedModelError::ContextCompile(code)) if code == "context_content_hash_mismatch"
    ));

    let mut conflict = request(model_id, &fingerprint);
    let mut left = text_context_candidate(TextContextCandidateInput {
        candidate_id: "fact-left".into(),
        source_kind: "account_strategy".into(),
        source_id: "strategy-left".into(),
        source_version: "1".into(),
        trust: TrustLevel::ConfirmedFact,
        priority: ContextPriority::P1,
        required: false,
        render_order: 0,
        observed_at: "2026-01-01T00:00:00Z".into(),
        text: "left".into(),
    });
    left.fact_key = Some("strategy.positioning".into());
    let mut right = text_context_candidate(TextContextCandidateInput {
        candidate_id: "fact-right".into(),
        source_kind: "account_strategy".into(),
        source_id: "strategy-right".into(),
        source_version: "1".into(),
        trust: TrustLevel::ConfirmedFact,
        priority: ContextPriority::P1,
        required: false,
        render_order: 1,
        observed_at: "2026-01-01T00:00:00Z".into(),
        text: "right".into(),
    });
    right.fact_key = Some("strategy.positioning".into());
    conflict.context_candidates.extend([left, right]);
    assert!(matches!(
        executor.execute(conflict).await,
        Err(AuditedModelError::ContextCompile(code)) if code == "context_conflict"
    ));

    let tokenizer_audit = Arc::new(FakeAudit::default());
    let mut unresolved = resolved(
        Arc::new(FakeClient {
            calls: calls.clone(),
            result: Ok("must not run".into()),
        }),
        model_id,
        &fingerprint,
    );
    unresolved.tokenizer_profile_key = "missing-profile".into();
    let tokenizer_executor = AuditedModelExecutor::new(
        registry(),
        Arc::new(FakeResolver {
            resolved: unresolved,
        }),
        tokenizer_audit.clone(),
        tokenizer_audit.clone(),
    );
    let mut tokenizer_failure = request(model_id, &fingerprint);
    tokenizer_failure.binding.tokenizer_profile.key = "missing-profile".into();
    tokenizer_failure.binding.tokenizer_profile.digest = "f".repeat(64);
    assert!(matches!(
        tokenizer_executor.execute(tokenizer_failure).await,
        Err(AuditedModelError::TokenizerProfileUnavailable)
    ));

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(audit.prepared.lock().unwrap().is_empty());
    assert_eq!(
        audit
            .attempts
            .lock()
            .unwrap()
            .iter()
            .map(|item| item.attempt.code.as_str())
            .collect::<Vec<_>>(),
        ["context_content_hash_mismatch", "context_conflict"]
    );
    assert_eq!(
        tokenizer_audit.attempts.lock().unwrap()[0].attempt.code,
        "tokenizer_profile_unavailable"
    );
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
        agent_version: "2.0.0".into(),
        node_key: "script.complete".into(),
        variables: BTreeMap::from([("scene_count".into(), json!(3))]),
        context_candidates: vec![text_context_candidate(TextContextCandidateInput {
            candidate_id: "input-1".into(),
            source_kind: "user_instruction".into(),
            source_id: "test".into(),
            source_version: "1".into(),
            trust: TrustLevel::UserInstruction,
            priority: ContextPriority::P0,
            required: true,
            render_order: 0,
            observed_at: "2026-01-01T00:00:00Z".into(),
            text: "write a script".into(),
        })],
        context_atomic_groups: Vec::new(),
        compiled_at: "2026-01-01T00:00:00Z".into(),
        tool_profile: "chat".into(),
        tool_schema: None,
        binding: fixed_binding(model_id, fingerprint),
        context_sources: json!([{"id":"input-1","source":"test"}]),
        memory_sources: json!([]),
        parameters: json!({"temperature":0.8}),
        asset_references: json!([]),
    }
}

fn fixed_binding(model_id: Uuid, fingerprint: &str) -> FixedModelBinding {
    let registry = registry();
    let agent = registry.agent("video.script", "2.0.0").unwrap();
    let context_policy_bindings = agent
        .nodes
        .iter()
        .map(|(node_key, reference)| {
            let policy = reference.context_policy.as_ref().unwrap();
            let definition = registry
                .context_policy(&policy.key, &policy.version)
                .unwrap();
            (
                node_key.clone(),
                novex_agent::FixedDefinitionBinding {
                    key: policy.key.clone(),
                    version: policy.version.clone(),
                    digest: definition_digest(definition).unwrap(),
                },
            )
        })
        .collect();
    let profile = registry
        .tokenizer_profile("byte-upper-bound", "1.0.0")
        .unwrap();
    FixedModelBinding {
        model_id,
        behavior_fingerprint: fingerprint.into(),
        context_policy_bindings,
        tokenizer_profile: novex_agent::FixedDefinitionBinding {
            key: profile.profile_key.clone(),
            version: profile.version.clone(),
            digest: definition_digest(profile).unwrap(),
        },
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
        tokenizer_profile_key: "byte-upper-bound".into(),
        tokenizer_profile_version: "1.0.0".into(),
        max_output_tokens: 3_000,
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
async fn provider_context_overflow_finishes_once_and_blocks_the_fixed_binding() {
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
                    result: Err(LLMError::Provider(
                        "400 context_length_exceeded: maximum context length reached".into(),
                    )),
                }),
                model_id,
                &fingerprint,
            ),
        }),
        audit.clone(),
        audit.clone(),
    );

    assert!(matches!(
        executor.execute(request(model_id, &fingerprint)).await,
        Err(AuditedModelError::TokenizerProfileIncompatible { .. })
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(audit.finished.lock().unwrap().len(), 1);
    assert_eq!(
        audit.finished.lock().unwrap()[0]
            .error_snapshot
            .as_ref()
            .unwrap()["kind"],
        "tokenizer_profile_incompatible"
    );
    assert!(audit.binding_blocked.load(Ordering::SeqCst));

    assert!(matches!(
        executor.execute(request(model_id, &fingerprint)).await,
        Err(AuditedModelError::ContextBindingRebindRequired)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(audit.prepared.lock().unwrap().len(), 1);
    assert_eq!(audit.finished.lock().unwrap().len(), 1);
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
        Arc::new(FakeAudit::default()),
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
        Arc::new(FakeAudit::default()),
    );
    assert!(matches!(
        incompatible.execute(request(model_id, &fingerprint)).await,
        Err(AuditedModelError::ModelCapabilityMismatch)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn policy_or_profile_digest_drift_blocks_before_context_or_provider_execution() {
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
                    result: Ok("must not run".into()),
                }),
                model_id,
                &fingerprint,
            ),
        }),
        audit.clone(),
        audit.clone(),
    );

    let mut policy_drift = request(model_id, &fingerprint);
    policy_drift
        .binding
        .context_policy_bindings
        .get_mut("script.complete")
        .unwrap()
        .digest = "d".repeat(64);
    assert!(matches!(
        executor.execute(policy_drift).await,
        Err(AuditedModelError::ContextBindingRebindRequired)
    ));

    let mut profile_drift = request(model_id, &fingerprint);
    profile_drift.binding.tokenizer_profile.digest = "e".repeat(64);
    assert!(matches!(
        executor.execute(profile_drift).await,
        Err(AuditedModelError::ContextBindingRebindRequired)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(audit.prepared.lock().unwrap().is_empty());
    assert!(audit.contexts.lock().unwrap().is_empty());
    assert!(audit.attempts.lock().unwrap().is_empty());
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
    let audit = Arc::new(FakeAudit::default());
    let executor =
        AuditedModelExecutor::new(registry(), resolver.clone(), audit.clone(), audit.clone());

    assert_eq!(
        executor
            .execute(request(model_id, &fingerprint))
            .await
            .unwrap()
            .output,
        "first credential"
    );
    let mut retry = request(model_id, &fingerprint);
    retry.attempt = 2;
    assert_eq!(
        executor.execute(retry).await.unwrap().output,
        "rotated credential"
    );
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 2);
    assert_eq!(provider_calls.load(Ordering::SeqCst), 2);
    let prepared = audit.prepared.lock().unwrap();
    assert_eq!(prepared.len(), 2);
    assert_eq!([prepared[0].attempt, prepared[1].attempt], [1, 2]);
    assert_ne!(
        prepared[0].snapshot.context_snapshot_id,
        prepared[1].snapshot.context_snapshot_id
    );
    assert_eq!(audit.contexts.lock().unwrap().len(), 2);
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

#[tokio::test]
async fn requires_governed_context_snapshot_before_the_provider_is_called() {
    let model_id = Uuid::new_v4();
    let fingerprint = "b".repeat(64);
    let events = Arc::new(Mutex::new(Vec::new()));
    let provider_prompts = Arc::new(Mutex::new(Vec::new()));
    let audit = Arc::new(FakeAudit {
        events: Some(events.clone()),
        ..FakeAudit::default()
    });
    let executor = AuditedModelExecutor::new(
        registry(),
        Arc::new(FakeResolver {
            resolved: resolved(
                Arc::new(OrderedClient {
                    events: events.clone(),
                    prompts: provider_prompts.clone(),
                }),
                model_id,
                &fingerprint,
            ),
        }),
        audit.clone(),
        audit.clone(),
    );

    executor
        .execute(request(model_id, &fingerprint))
        .await
        .unwrap();
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["snapshot_and_model_call", "provider"]
    );
    let prepared = audit.prepared.lock().unwrap();
    assert_eq!(prepared.len(), 1);
    let snapshot = &prepared[0].snapshot;
    assert_eq!(snapshot.schema_version, "2");
    assert!(snapshot.fragments.is_empty());
    assert!(snapshot.context_snapshot_id.is_some());
    assert!(snapshot.context_digest.is_some());
    assert!(snapshot.logical_input.is_some());
    let contexts = audit.contexts.lock().unwrap();
    assert_eq!(contexts.len(), 1);
    assert_eq!(
        snapshot.context_digest.as_deref(),
        Some(contexts[0].snapshot.digest.as_str())
    );
    let provider_prompts = provider_prompts.lock().unwrap();
    assert_eq!(provider_prompts.len(), 1);
    assert_eq!(provider_prompts[0].system, snapshot.system);
    assert_eq!(provider_prompts[0].user, snapshot.user);
    assert_eq!(
        provider_prompts[0].max_output_tokens,
        snapshot.max_output_tokens
    );
    let logical_input = snapshot.logical_input.as_ref().unwrap();
    assert_eq!(logical_input.system, provider_prompts[0].system);
    assert_eq!(
        logical_input.messages[0].content,
        json!(provider_prompts[0].user)
    );
}
