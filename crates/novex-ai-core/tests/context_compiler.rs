use novex_ai_core::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

fn profile(mode: TokenizerMode) -> TokenizerProfile {
    TokenizerProfile {
        profile_key: "fixture.profile".into(),
        version: "1.0.0".into(),
        status: DefinitionStatus::Active,
        implementation_version: "1".into(),
        mode,
        applicable_protocols: vec!["openai_responses".into()],
        applicable_model_families: vec!["fixture".into()],
        framing: FramingRules {
            per_message_tokens: 3,
            per_tool_tokens: 4,
            request_tokens: 3,
            reply_priming_tokens: 3,
        },
        safety_reserve_tokens: 16,
    }
}

fn candidate(
    id: &str,
    text: &str,
    trust: TrustLevel,
    priority: ContextPriority,
    required: bool,
) -> ContextCandidate {
    let payload = ContextPayload::Text { text: text.into() };
    ContextCandidate {
        candidate_id: id.into(),
        source_kind: "fixture".into(),
        source_id: id.into(),
        source_version: "1".into(),
        fact_key: None,
        trust,
        priority,
        required,
        render_order: match priority {
            ContextPriority::P0 => 0,
            ContextPriority::P1 => 1,
            ContextPriority::P2 => 2,
            ContextPriority::P3 => 3,
            ContextPriority::P4 => 4,
        },
        observed_at: "2026-07-25T00:00:00Z".into(),
        valid_until: None,
        supersedes: vec![],
        content_hash: sha256_hex(
            canonical_json(&serde_json::to_value(&payload).unwrap()).as_bytes(),
        ),
        atomic_group_id: None,
        payload,
    }
}

#[test]
fn render_order_is_independent_from_budget_priority() {
    let mut instruction = candidate(
        "instruction",
        "current user instruction",
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
    );
    instruction.render_order = 1;
    let mut fact = candidate(
        "fact",
        "confirmed project fact",
        TrustLevel::ConfirmedFact,
        ContextPriority::P1,
        true,
    );
    fact.render_order = 0;

    let compiled = ContextCompiler::compile(request(vec![instruction, fact])).unwrap();

    assert_eq!(compiled.selected_order, ["fact", "instruction"]);
    assert_eq!(
        compiled.logical_input.messages[0].content,
        "confirmed project fact"
    );
    assert_eq!(
        compiled.logical_input.messages[1].content,
        "current user instruction"
    );
}

fn request(candidates: Vec<ContextCandidate>) -> ContextCompileRequest {
    ContextCompileRequest {
        schema_version: "2".into(),
        owner: ExecutorOwner::Rust,
        owner_id: "owner-1".into(),
        node_key: "fixture.node".into(),
        compiled_at: "2026-07-25T00:00:00Z".into(),
        model_context_window: 256,
        policy: ContextPolicyDefinition {
            policy_key: "fixture.policy".into(),
            version: "1.0.0".into(),
            status: DefinitionStatus::Active,
            executor_owners: vec![ExecutorOwner::Rust],
            allowed_sources: vec!["fixture".into()],
            required_sources: vec![],
            stable_sort: vec![
                "priority".into(),
                "source_kind".into(),
                "source_id".into(),
                "source_version".into(),
                "candidate_id".into(),
            ],
        },
        tokenizer_profile: profile(TokenizerMode::Conservative {
            algorithm: "utf8-byte-upper-bound@1".into(),
        }),
        prepared_prompt: PreparedPromptEnvelope {
            system: "fixed system".into(),
            user_template_fixed: String::new(),
            tool_schema: Some(json!([])),
            output_schema: None,
            protocol_envelope_tokens: 3,
            max_output_tokens: 32,
        },
        candidates,
        atomic_groups: vec![],
    }
}

#[test]
fn compiler_is_deterministic_and_keeps_trust_independent_from_priority() {
    let p0 = candidate(
        "instruction",
        "current user instruction",
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
    );
    let p1 = candidate(
        "fact",
        "confirmed project fact",
        TrustLevel::ConfirmedFact,
        ContextPriority::P1,
        false,
    );
    let p4 = candidate(
        "candidate",
        "untrusted draft",
        TrustLevel::Candidate,
        ContextPriority::P4,
        false,
    );
    let p2 = candidate(
        "p2-candidate",
        "candidate at p2",
        TrustLevel::Candidate,
        ContextPriority::P2,
        false,
    );
    let p3 = candidate(
        "p3-platform",
        "platform at p3",
        TrustLevel::Platform,
        ContextPriority::P3,
        false,
    );
    let first = ContextCompiler::compile(request(vec![
        p4.clone(),
        p3.clone(),
        p1.clone(),
        p2.clone(),
        p0.clone(),
    ]))
    .unwrap();
    let second = ContextCompiler::compile(request(vec![p0, p2, p4, p1, p3])).unwrap();
    assert_eq!(first.digest, second.digest);
    assert_eq!(
        first.selected_order,
        [
            "instruction",
            "fact",
            "p2-candidate",
            "p3-platform",
            "candidate"
        ]
    );
    assert_eq!(first.tokenizer_mode, "conservative");
    assert!(
        first.budget.final_input_tokens
            + first.budget.max_output_tokens
            + first.budget.safety_reserve_tokens
            <= first.budget.model_context_window
    );
}

#[test]
fn budget_ledger_accounts_for_every_fixed_component_and_profile_framing() {
    let compiled = ContextCompiler::compile(request(vec![])).unwrap();
    assert_eq!(compiled.budget.system_prompt_tokens, 12);
    assert_eq!(compiled.budget.user_template_fixed_tokens, 0);
    assert_eq!(compiled.budget.tool_schema_tokens, 2);
    assert_eq!(compiled.budget.output_schema_tokens, 0);
    assert_eq!(compiled.budget.protocol_envelope_tokens, 15);
    assert_eq!(compiled.budget.max_output_tokens, 32);
    assert_eq!(compiled.budget.safety_reserve_tokens, 16);
    assert_eq!(compiled.budget.dynamic_context_budget, 179);
    assert_eq!(compiled.budget.final_input_tokens, 0);

    let mut with_tools = request(vec![]);
    with_tools.prepared_prompt.tool_schema = Some(json!([
        {"name": "read"},
        {"name": "write"}
    ]));
    let with_tools = ContextCompiler::compile(with_tools).unwrap();
    assert_eq!(
        with_tools.budget.protocol_envelope_tokens,
        compiled.budget.protocol_envelope_tokens + 8
    );
}

#[test]
fn final_logical_input_recheck_rejects_a_real_bpe_boundary_overflow_without_reselection() {
    let mut input = request(vec![candidate(
        "required",
        "A",
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
    )]);
    input.model_context_window = 15;
    input.prepared_prompt.system.clear();
    input.prepared_prompt.user_template_fixed = "aa".into();
    input.prepared_prompt.tool_schema = None;
    input.prepared_prompt.protocol_envelope_tokens = 0;
    input.prepared_prompt.max_output_tokens = 1;
    input.tokenizer_profile = profile(TokenizerMode::Exact {
        encoding: "cl100k_base".into(),
        asset_digest: ENCODING_CONTRACT_V1_DIGEST.into(),
    });
    input.tokenizer_profile.safety_reserve_tokens = 0;

    // `aa` and `A` are one token each, while the finalized `aAa` is three tokens.
    let compiled = ContextCompiler::compile(input.clone()).unwrap();
    assert_eq!(compiled.selected_order, ["required"]);
    let error = ContextCompiler::finalize(
        &compiled,
        &input.tokenizer_profile,
        LogicalModelInput {
            system: String::new(),
            messages: vec![LogicalMessage {
                role: "user".into(),
                content: Value::String("aAa".into()),
                thinking: None,
                tool_call_id: None,
            }],
            tool_schema: None,
            output_schema: None,
        },
    )
    .unwrap_err();
    assert_eq!(error.stage, CompileFailureStage::Finalize);
    assert_eq!(error.code, "context_budget_exceeded");
    assert_eq!(compiled.selected_order, ["required"]);
}

#[test]
fn compiler_fails_closed_for_conflicts_required_budget_and_invalid_hashes() {
    let mut first = candidate(
        "fact-a",
        "A",
        TrustLevel::ConfirmedFact,
        ContextPriority::P1,
        true,
    );
    first.fact_key = Some("project.target".into());
    let mut second = candidate(
        "fact-b",
        "B",
        TrustLevel::ConfirmedFact,
        ContextPriority::P1,
        true,
    );
    second.fact_key = Some("project.target".into());
    assert_eq!(
        ContextCompiler::compile(request(vec![first, second]))
            .unwrap_err()
            .code,
        "context_conflict"
    );

    let mut oversized = candidate(
        "required",
        &"中".repeat(200),
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
    );
    let oversized_request = request(vec![oversized.clone()]);
    let error = ContextCompiler::compile(oversized_request.clone()).unwrap_err();
    assert_eq!(error.code, "context_budget_exceeded");
    let attempt = error.attempt(&oversized_request);
    assert!(attempt.budget.is_some());
    assert!(attempt.decisions.is_empty());
    assert!(!serde_json::to_string(&attempt).unwrap().contains('中'));
    oversized.content_hash = "0".repeat(64);
    assert_eq!(
        ContextCompiler::compile(request(vec![oversized]))
            .unwrap_err()
            .code,
        "context_content_hash_mismatch"
    );
}

#[test]
fn tool_groups_are_atomic_and_compile_attempts_do_not_contain_payloads() {
    let mut tool_request = candidate(
        "tool-request",
        "{\"call\":\"1\"}",
        TrustLevel::Reference,
        ContextPriority::P0,
        true,
    );
    let mut tool_result = candidate(
        "tool-result",
        "{\"result\":\"ok\"}",
        TrustLevel::Reference,
        ContextPriority::P0,
        true,
    );
    tool_request.atomic_group_id = Some("tool-1".into());
    tool_result.atomic_group_id = Some("tool-1".into());
    let mut input = request(vec![tool_request, tool_result]);
    input.atomic_groups = vec![ContextAtomicGroup {
        group_id: "tool-1".into(),
        member_ids: vec!["tool-request".into(), "tool-result".into()],
    }];
    let snapshot = ContextCompiler::compile(input.clone()).unwrap();
    assert_eq!(snapshot.selected_order, ["tool-request", "tool-result"]);

    input.atomic_groups[0].member_ids.pop();
    let error = ContextCompiler::compile(input.clone()).unwrap_err();
    let attempt = error.attempt(&input);
    let serialized = serde_json::to_value(attempt).unwrap();
    assert_eq!(serialized["code"], "context_atomic_group_invalid");
    assert_eq!(serialized["decisions"], Value::Array(vec![]));
    assert!(!serialized.to_string().contains("result"));
}

#[test]
fn incomplete_atomic_groups_are_excluded_together_or_fail_when_required() {
    let mut request_part = candidate(
        "tool-request",
        "request",
        TrustLevel::Reference,
        ContextPriority::P2,
        false,
    );
    let mut result_part = candidate(
        "tool-result",
        "result",
        TrustLevel::Reference,
        ContextPriority::P2,
        false,
    );
    request_part.atomic_group_id = Some("tool-1".into());
    request_part.valid_until = Some("2026-07-24T00:00:00Z".into());
    result_part.atomic_group_id = Some("tool-1".into());
    let group = ContextAtomicGroup {
        group_id: "tool-1".into(),
        member_ids: vec!["tool-request".into(), "tool-result".into()],
    };
    let mut input = request(vec![request_part.clone(), result_part.clone()]);
    input.atomic_groups = vec![group.clone()];
    let snapshot = ContextCompiler::compile(input).unwrap();
    assert!(snapshot.selected_order.is_empty());
    assert!(snapshot.decisions.iter().any(|decision| {
        decision.candidate_id == "tool-result"
            && decision.decision == ContextDecisionCode::AtomicGroupExcluded
    }));

    result_part.required = true;
    let mut required = request(vec![request_part.clone(), result_part.clone()]);
    required.atomic_groups = vec![group.clone()];
    assert_eq!(
        ContextCompiler::compile(required).unwrap_err().code,
        "required_context_unavailable"
    );

    result_part.required = false;
    result_part.atomic_group_id = None;
    let mut malformed = request(vec![request_part, result_part]);
    malformed.atomic_groups = vec![group];
    assert_eq!(
        ContextCompiler::compile(malformed).unwrap_err().code,
        "context_atomic_group_invalid"
    );
}

#[test]
fn candidate_schema_validates_assets_timestamps_supersedes_and_json_order() {
    let payload = ContextPayload::Asset {
        asset: AssetReference {
            asset_id: "asset-1".into(),
            version: "1".into(),
            sha256: "a".repeat(64),
            mime: "image/png".into(),
            metadata: BTreeMap::new(),
        },
    };
    let mut asset = candidate(
        "asset",
        "placeholder",
        TrustLevel::Reference,
        ContextPriority::P2,
        false,
    );
    asset.payload = payload;
    asset.content_hash =
        sha256_hex(canonical_json(&serde_json::to_value(&asset.payload).unwrap()).as_bytes());
    ContextCompiler::compile(request(vec![asset.clone()])).unwrap();

    let mut invalid_asset = asset.clone();
    if let ContextPayload::Asset { asset } = &mut invalid_asset.payload {
        asset.mime.clear();
    }
    invalid_asset.content_hash = sha256_hex(
        canonical_json(&serde_json::to_value(&invalid_asset.payload).unwrap()).as_bytes(),
    );
    assert_eq!(
        ContextCompiler::compile(request(vec![invalid_asset]))
            .unwrap_err()
            .code,
        "context_schema_invalid"
    );

    let mut invalid_time = asset.clone();
    invalid_time.observed_at = "not-a-time".into();
    assert_eq!(
        ContextCompiler::compile(request(vec![invalid_time]))
            .unwrap_err()
            .code,
        "context_schema_invalid"
    );
    let mut invalid_supersedes = asset.clone();
    invalid_supersedes.supersedes = vec!["missing".into()];
    assert_eq!(
        ContextCompiler::compile(request(vec![invalid_supersedes]))
            .unwrap_err()
            .code,
        "context_schema_invalid"
    );

    let mut wrong_owner = request(vec![asset.clone()]);
    wrong_owner.owner = ExecutorOwner::Pi;
    assert_eq!(
        ContextCompiler::compile(wrong_owner).unwrap_err().code,
        "context_schema_invalid"
    );
    let mut denied_source = request(vec![asset.clone()]);
    denied_source.candidates[0].source_kind = "denied".into();
    assert_eq!(
        ContextCompiler::compile(denied_source).unwrap_err().code,
        "context_schema_invalid"
    );
    let mut required_source = request(vec![asset.clone()]);
    required_source.policy.required_sources = vec!["fixture".into()];
    assert_eq!(
        ContextCompiler::compile(required_source).unwrap_err().code,
        "context_schema_invalid"
    );

    let first_payload = ContextPayload::Message {
        message: LogicalMessage {
            role: "user".into(),
            content: json!({"a": 1, "b": 2}),
            thinking: None,
            tool_call_id: None,
        },
    };
    let second_payload = ContextPayload::Message {
        message: LogicalMessage {
            role: "user".into(),
            content: serde_json::from_str("{\"b\":2,\"a\":1}").unwrap(),
            thinking: None,
            tool_call_id: None,
        },
    };
    let mut first = candidate(
        "message",
        "placeholder",
        TrustLevel::Reference,
        ContextPriority::P2,
        false,
    );
    first.payload = first_payload;
    first.content_hash =
        sha256_hex(canonical_json(&serde_json::to_value(&first.payload).unwrap()).as_bytes());
    let mut second = first.clone();
    second.payload = second_payload;
    second.content_hash =
        sha256_hex(canonical_json(&serde_json::to_value(&second.payload).unwrap()).as_bytes());
    assert_eq!(
        ContextCompiler::compile(request(vec![first]))
            .unwrap()
            .digest,
        ContextCompiler::compile(request(vec![second]))
            .unwrap()
            .digest
    );
}

#[test]
fn exact_and_conservative_tokenizers_cover_unicode_and_reject_unknown_profiles() {
    let exact = ProfileTokenizer::from_profile(profile(TokenizerMode::Exact {
        encoding: "cl100k_base".into(),
        asset_digest: ENCODING_CONTRACT_V1_DIGEST.into(),
    }))
    .unwrap();
    assert!(exact.count_text("中文 🔒 JSON") > 0);
    let conservative = ProfileTokenizer::from_profile(profile(TokenizerMode::Conservative {
        algorithm: "utf8-byte-upper-bound@1".into(),
    }))
    .unwrap();
    assert_eq!(conservative.count_text("中"), 3);
    assert!(
        ProfileTokenizer::from_profile(profile(TokenizerMode::Conservative {
            algorithm: "chars/4".into()
        }))
        .is_err()
    );
}

#[test]
fn exact_token_counts_match_the_shared_cross_language_asset() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../agent-definitions/tokenizers/encoding-contract-v1.json"
    ))
    .unwrap();
    for encoding in ["cl100k_base", "o200k_base"] {
        let tokenizer = ProfileTokenizer::from_profile(profile(TokenizerMode::Exact {
            encoding: encoding.into(),
            asset_digest: ENCODING_CONTRACT_V1_DIGEST.into(),
        }))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            assert_eq!(
                tokenizer.count_text(case["text"].as_str().unwrap()),
                case[encoding].as_u64().unwrap(),
                "token count mismatch for {encoding}/{}",
                case["id"]
            );
        }
    }
}

#[test]
fn exact_tokenizer_cache_is_versioned_and_bounded() {
    for version in 0..12 {
        let mut current = profile(TokenizerMode::Exact {
            encoding: "cl100k_base".into(),
            asset_digest: ENCODING_CONTRACT_V1_DIGEST.into(),
        });
        current.version = format!("1.0.{version}");
        ProfileTokenizer::from_profile(current).unwrap();
    }
    assert!(ProfileTokenizer::cache_size() <= 8);
    let invalid = profile(TokenizerMode::Exact {
        encoding: "cl100k_base".into(),
        asset_digest: "0".repeat(64),
    });
    assert_eq!(
        ProfileTokenizer::from_profile(invalid).unwrap_err().code,
        "tokenizer_profile_unavailable"
    );
}

#[test]
fn prompt_prepare_context_finalize_preserves_the_under_budget_legacy_compile_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("agent-definitions");
    let registry = DefinitionRegistry::load(root).unwrap();
    let compiler = PromptCompiler::new(&registry);
    let legacy = compiler
        .compile(
            "video.project-strategy",
            "2.0.0",
            "project.strategy_draft",
            PromptCompileInput {
                schema_version: "1".into(),
                variables: BTreeMap::new(),
                fragments: vec![DynamicFragment {
                    id: "legacy".into(),
                    trust: TrustLevel::Reference,
                    source: "migration_baseline_fixture".into(),
                    content: Some("golden-user-input".into()),
                    asset: None,
                }],
            },
            "chat",
            None,
        )
        .unwrap();
    let prepared = compiler
        .prepare(
            "video.project-strategy",
            "2.0.0",
            "project.strategy_draft",
            PromptPrepareInput {
                variables: BTreeMap::new(),
                tool_profile: "chat".into(),
                tool_schema: None,
                model_max_output_tokens: 2_000,
            },
        )
        .unwrap();
    let payload = ContextPayload::Text {
        text: "golden-user-input".into(),
    };
    let candidate = ContextCandidate {
        candidate_id: "golden".into(),
        source_kind: "user_instruction".into(),
        source_id: "golden".into(),
        source_version: "1".into(),
        fact_key: None,
        trust: TrustLevel::UserInstruction,
        priority: ContextPriority::P0,
        required: true,
        render_order: 0,
        observed_at: "2026-07-25T00:00:00Z".into(),
        valid_until: None,
        supersedes: vec![],
        content_hash: sha256_hex(
            canonical_json(&serde_json::to_value(&payload).unwrap()).as_bytes(),
        ),
        atomic_group_id: None,
        payload,
    };
    let context = ContextCompiler::compile(ContextCompileRequest {
        schema_version: "2".into(),
        owner: ExecutorOwner::Rust,
        owner_id: "owner".into(),
        node_key: "project.strategy_draft".into(),
        compiled_at: "2026-07-25T00:00:00Z".into(),
        model_context_window: 8192,
        policy: registry
            .context_policy("project.strategy_draft.baseline", "1.0.0")
            .unwrap()
            .clone(),
        tokenizer_profile: registry
            .tokenizer_profile("byte-upper-bound", "1.0.0")
            .unwrap()
            .clone(),
        prepared_prompt: prepared.envelope.clone(),
        candidates: vec![candidate],
        atomic_groups: vec![],
    })
    .unwrap();
    let finalized = compiler
        .finalize(
            prepared,
            "context-snapshot-1",
            &context,
            registry
                .tokenizer_profile("byte-upper-bound", "1.0.0")
                .unwrap(),
        )
        .unwrap();
    assert_eq!(finalized.prompt_snapshot.schema_version, "2");
    assert_eq!(finalized.prompt_snapshot.system, legacy.system);
    assert_eq!(finalized.prompt_snapshot.user, legacy.user);
    assert_eq!(
        finalized.prompt_snapshot.output_schema,
        legacy.output_schema
    );
    assert!(finalized.prompt_snapshot.fragments.is_empty());
    assert_eq!(
        finalized.prompt_snapshot.context_digest.as_deref(),
        Some(finalized.context_snapshot.digest.as_str())
    );
    assert_eq!(
        finalized
            .prompt_snapshot
            .logical_input
            .as_ref()
            .unwrap()
            .messages[0]
            .content,
        Value::String(legacy.user)
    );
    assert_eq!(
        finalized.prompt_snapshot.logical_input.as_ref(),
        Some(&finalized.context_snapshot.logical_input)
    );
}

#[test]
fn prompt_prepare_preserves_workspace_tools_fixed_templates_and_output_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("agent-definitions");
    let registry = DefinitionRegistry::load(root).unwrap();
    let tools = json!([
        {"name": "read"},
        {"name": "write"},
        {"name": "edit"},
        {"name": "bash"}
    ]);
    let prepared = PromptCompiler::new(&registry)
        .prepare(
            "personal.general",
            "2.0.0",
            "personal.turn",
            PromptPrepareInput {
                variables: BTreeMap::new(),
                tool_profile: "workspace".into(),
                tool_schema: Some(tools.clone()),
                model_max_output_tokens: 2_048,
            },
        )
        .unwrap();
    assert!(!prepared.envelope.system.is_empty());
    assert_eq!(prepared.envelope.user_template_fixed, "");
    assert_eq!(prepared.envelope.tool_schema, Some(tools));
    assert_eq!(prepared.envelope.output_schema, None);
    assert_eq!(prepared.envelope.max_output_tokens, 2_048);
}

#[test]
fn every_active_node_preserves_its_under_budget_legacy_compile_output_and_reads_v1_v2_snapshots() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("agent-definitions");
    let registry = DefinitionRegistry::load(root).unwrap();
    let compiler = PromptCompiler::new(&registry);
    let tokenizer_profile = registry
        .tokenizer_profile("byte-upper-bound", "1.0.0")
        .unwrap();
    let mut checked = 0;

    for agent in registry
        .agents()
        .iter()
        .filter(|agent| agent.status == DefinitionStatus::Active)
    {
        for (node_key, reference) in &agent.nodes {
            let prompt = registry
                .prompts()
                .iter()
                .find(|prompt| {
                    prompt.prompt_key == reference.key && prompt.version == reference.version
                })
                .unwrap();
            let variables = if node_key == "script.complete" {
                BTreeMap::from([("scene_count".into(), json!(3))])
            } else {
                BTreeMap::new()
            };
            let tool_profile = prompt.tool_profile.as_deref().unwrap_or("chat");
            let prepared = compiler
                .prepare(
                    &agent.agent_key,
                    &agent.version,
                    node_key,
                    PromptPrepareInput {
                        variables: variables.clone(),
                        tool_profile: tool_profile.into(),
                        tool_schema: None,
                        model_max_output_tokens: 4_096,
                    },
                )
                .unwrap();
            let trust = prompt.fragment_trust().unwrap();
            let legacy = compiler
                .compile(
                    &agent.agent_key,
                    &agent.version,
                    node_key,
                    PromptCompileInput {
                        schema_version: "1".into(),
                        variables,
                        fragments: vec![DynamicFragment {
                            id: "legacy".into(),
                            trust,
                            source: "migration_baseline_fixture".into(),
                            content: Some("golden-user-input".into()),
                            asset: None,
                        }],
                    },
                    tool_profile,
                    None,
                )
                .unwrap();
            let policy_reference = reference.context_policy.as_ref().unwrap();
            let policy = registry
                .context_policy(&policy_reference.key, &policy_reference.version)
                .unwrap();
            let payload = ContextPayload::Text {
                text: "golden-user-input".into(),
            };
            let compiled = ContextCompiler::compile(ContextCompileRequest {
                schema_version: "2".into(),
                owner: agent.executor_owner,
                owner_id: format!("golden-{node_key}"),
                node_key: node_key.clone(),
                compiled_at: "2026-07-25T00:00:00Z".into(),
                model_context_window: 1_000_000,
                policy: policy.clone(),
                tokenizer_profile: tokenizer_profile.clone(),
                prepared_prompt: prepared.envelope.clone(),
                candidates: vec![ContextCandidate {
                    candidate_id: "golden".into(),
                    source_kind: policy
                        .required_sources
                        .first()
                        .unwrap_or(&policy.allowed_sources[0])
                        .clone(),
                    source_id: "golden".into(),
                    source_version: "1".into(),
                    fact_key: None,
                    trust,
                    priority: ContextPriority::P0,
                    required: true,
                    render_order: 0,
                    observed_at: "2026-07-25T00:00:00Z".into(),
                    valid_until: None,
                    supersedes: vec![],
                    content_hash: sha256_hex(
                        canonical_json(&serde_json::to_value(&payload).unwrap()).as_bytes(),
                    ),
                    atomic_group_id: None,
                    payload,
                }],
                atomic_groups: vec![],
            })
            .unwrap();
            let finalized = compiler
                .finalize(
                    prepared,
                    format!("snapshot-{node_key}"),
                    &compiled,
                    tokenizer_profile,
                )
                .unwrap();
            assert_eq!(
                finalized.prompt_snapshot.system, legacy.system,
                "{node_key}"
            );
            assert_eq!(finalized.prompt_snapshot.user, legacy.user, "{node_key}");
            assert_eq!(
                finalized.prompt_snapshot.output_schema, legacy.output_schema,
                "{node_key}"
            );
            assert_eq!(
                finalized.prompt_snapshot.max_output_tokens, legacy.max_output_tokens,
                "{node_key}"
            );
            assert_eq!(
                read_prompt_snapshot(serde_json::to_value(&legacy).unwrap()).unwrap(),
                legacy
            );
            assert_eq!(
                read_prompt_snapshot(serde_json::to_value(&finalized.prompt_snapshot).unwrap())
                    .unwrap(),
                finalized.prompt_snapshot
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 27);
}

#[test]
fn context_snapshot_digest_matches_the_shared_cross_language_contract() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../agent-definitions/fixtures/context-compile-contract-v1.json"
    ))
    .unwrap();
    let request: ContextCompileRequest =
        serde_json::from_value(fixture["request"].clone()).unwrap();
    let snapshot = ContextCompiler::compile(request.clone()).unwrap();
    assert_eq!(snapshot.digest, fixture["expected_digest"]);
    let final_logical_input: LogicalModelInput =
        serde_json::from_value(fixture["final_logical_input"].clone()).unwrap();
    let final_snapshot =
        ContextCompiler::finalize(&snapshot, &request.tokenizer_profile, final_logical_input)
            .unwrap();
    assert_eq!(final_snapshot.digest, fixture["expected_snapshot_digest"]);
    let mut invalid = request;
    invalid.candidates[0].content_hash = "0".repeat(64);
    let attempt = ContextCompiler::compile(invalid.clone())
        .unwrap_err()
        .attempt(&invalid);
    assert_eq!(attempt.digest, fixture["expected_schema_attempt_digest"]);
}

#[test]
fn expiry_supersedes_duplicates_and_fixed_budget_have_stable_outcomes() {
    let mut expired = candidate(
        "expired",
        "old",
        TrustLevel::Reference,
        ContextPriority::P3,
        false,
    );
    expired.valid_until = Some("2026-07-24T00:00:00Z".into());
    let mut old = candidate(
        "old",
        "older",
        TrustLevel::Reference,
        ContextPriority::P3,
        false,
    );
    let mut current = candidate(
        "current",
        "newer",
        TrustLevel::Reference,
        ContextPriority::P3,
        false,
    );
    current.supersedes = vec!["old".into()];
    let duplicate = ContextCandidate {
        candidate_id: "duplicate".into(),
        source_id: "duplicate".into(),
        ..current.clone()
    };
    let snapshot =
        ContextCompiler::compile(request(vec![duplicate, old.clone(), current, expired])).unwrap();
    assert!(snapshot.decisions.iter().any(
        |item| item.candidate_id == "expired" && item.decision == ContextDecisionCode::Expired
    ));
    assert!(
        snapshot
            .decisions
            .iter()
            .any(|item| item.candidate_id == "old"
                && item.decision == ContextDecisionCode::Superseded)
    );
    assert!(snapshot
        .decisions
        .iter()
        .any(|item| item.decision == ContextDecisionCode::DuplicateContent));

    old.required = true;
    let mut fixed_overflow = request(vec![old]);
    fixed_overflow.model_context_window = 32;
    assert_eq!(
        ContextCompiler::compile(fixed_overflow).unwrap_err().code,
        "context_budget_exceeded"
    );
}

#[test]
fn context_compiler_module_has_no_model_tool_or_business_repository_dependency() {
    let source = include_str!("../src/context.rs");
    for forbidden in [
        "LLMClient",
        "ModelCallRepository",
        "TopicRepository",
        "ProjectRepository",
        "execute_tool",
        "generate_script(",
    ] {
        assert!(
            !source.contains(forbidden),
            "ContextCompiler must not depend on {forbidden}"
        );
    }
}
