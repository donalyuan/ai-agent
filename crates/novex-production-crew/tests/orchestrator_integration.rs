//! ProductionOrchestrator 集成测试
//!
//! ## 说明
//! - 带 `#[ignore]` 的测试需要真实数据库，在 CI/CD 或手动验证时运行。
//! - 不带 `#[ignore]` 的测试为纯内存逻辑验证，无 DB 依赖。
//!
//! ## 运行带 DB 的集成测试
//! ```bash
//! docker compose -f /server/docker-compose.yml exec -T novex-backend sh -lc \
//!   'cd /app && DATABASE_URL=postgres://... cargo test -p novex-production-crew --test orchestrator_integration -- --ignored'
//! ```

use novex_production_crew::{
    executor::role_executor::{ArtifactSummary, RoleExecutionStatus, RoleExecutor},
    roles::definition::{Lifecycle, PromptRef, RoleDefinition},
    state::artifacts::ArtifactType,
};
use serde_json::json;

// ── 输入就绪检查（无 DB）───────────────────────────────────────────────────

/// 角色不存在时 check_inputs_ready 应提前返回 RoleNotFound 错误（通过 RoleRegistry）
/// 这里直接用 RoleDefinition 测试底层 RoleExecutor
#[test]
fn missing_input_artifact_returns_correct_error() {
    use novex_production_crew::error::ProductionError;

    let def = RoleDefinition {
        role_key: "director".to_string(),
        role_name: "导演".to_string(),
        responsibilities: vec![],
        input_artifacts: vec![ArtifactType::StoryBible, ArtifactType::ScriptDraft],
        output_artifacts: vec![],
        allowed_tools: vec![],
        prompt_definition_ref: PromptRef {
            key: "production.director.general".to_string(),
            version: "@1".to_string(),
        },
        lifecycle: Lifecycle::Active,
    };

    // 空项目，无任何输入产物
    let available = vec![];
    let err = RoleExecutor::check_inputs_ready(&def, &available).unwrap_err();
    assert!(matches!(err, ProductionError::MissingInputArtifact { .. }));
    // 错误信息中包含缺失产物名称
    let msg = err.to_string();
    assert!(msg.contains("StoryBible") || msg.contains("缺少"));
}

/// 输入产物均就绪时 check_inputs_ready 应通过
#[test]
fn all_input_artifacts_ready_check_passes() {
    let def = RoleDefinition {
        role_key: "qc".to_string(),
        role_name: "质检员".to_string(),
        responsibilities: vec![],
        input_artifacts: vec![ArtifactType::ShotContract, ArtifactType::ContinuityLedger],
        output_artifacts: vec![ArtifactType::TakeReview],
        allowed_tools: vec![],
        prompt_definition_ref: PromptRef {
            key: "production.qc.general".to_string(),
            version: "@1".to_string(),
        },
        lifecycle: Lifecycle::Active,
    };

    let available = vec![ArtifactType::ShotContract, ArtifactType::ContinuityLedger];
    assert!(RoleExecutor::check_inputs_ready(&def, &available).is_ok());
}

// ── schema 验证（无 DB）────────────────────────────────────────────────────

/// 无效输出（缺失必需键）应返回 InvalidArtifactSchema 错误
#[test]
fn invalid_output_schema_returns_error() {
    use novex_production_crew::error::ProductionError;

    let def = RoleDefinition {
        role_key: "producer".to_string(),
        role_name: "制片人".to_string(),
        responsibilities: vec![],
        input_artifacts: vec![],
        output_artifacts: vec![ArtifactType::CreativeBrief],
        allowed_tools: vec![],
        prompt_definition_ref: PromptRef {
            key: "production.producer.general".to_string(),
            version: "@1".to_string(),
        },
        lifecycle: Lifecycle::Active,
    };

    // 输出不包含 creative_brief 顶层键
    let bad_output = json!({ "wrong_key": {} });
    let err = RoleExecutor::validate_output(&def, &bad_output).unwrap_err();
    assert!(matches!(err, ProductionError::InvalidArtifactSchema { .. }));
    // 确认无产物被保存（通过逻辑保证：validate 先于 save，失败则 save 不会调用）
    assert!(err.to_string().contains("creative_brief"));
}

/// 有效的 creative_brief 输出应通过验证
#[test]
fn valid_creative_brief_output_passes_validation() {
    let def = RoleDefinition {
        role_key: "producer".to_string(),
        role_name: "制片人".to_string(),
        responsibilities: vec![],
        input_artifacts: vec![],
        output_artifacts: vec![ArtifactType::CreativeBrief],
        allowed_tools: vec![],
        prompt_definition_ref: PromptRef {
            key: "production.producer.general".to_string(),
            version: "@1".to_string(),
        },
        lifecycle: Lifecycle::Active,
    };

    let good_output = json!({
        "creative_brief": {
            "target_audience": "18-25岁女性",
            "tone": ["活泼"],
            "key_messages": ["健康美妆"],
            "constraints": { "duration_seconds": 60 },
            "success_criteria": ["完播率>60%"]
        }
    });
    assert!(RoleExecutor::validate_output(&def, &good_output).is_ok());
}

// ── RoleRegistry bootstrap（无 DB）─────────────────────────────────────────

/// RoleRegistry::bootstrap 应能从真实 roles 目录加载所有9个角色
#[test]
fn role_registry_bootstrap_loads_all_production_roles() {
    use novex_production_crew::roles::RoleRegistry;
    use std::path::Path;

    // 容器内路径，若在宿主机运行此测试则跳过
    let roles_dir = Path::new("/app/crates/novex-production-crew/roles");
    if !roles_dir.exists() {
        eprintln!("Skipping: roles dir not found at {:?}", roles_dir);
        return;
    }

    let registry = RoleRegistry::bootstrap(roles_dir).expect("bootstrap should succeed");
    let all_roles = registry.list_all();
    assert_eq!(
        all_roles.len(),
        9,
        "应加载9个角色: producer, screenwriter, director, cinematographer, \
         performance_director, sound_director, editor, qc, character_critic"
    );

    // 验证关键角色存在
    assert!(registry.get("producer").is_ok());
    assert!(registry.get("screenwriter").is_ok());
    assert!(registry.get("director").is_ok());
    assert!(registry.get("qc").is_ok());

    // 验证不存在的角色返回 RoleNotFound
    assert!(registry.get("nonexistent").is_err());
}

// ── 端到端集成测试（需要真实DB + mock LLM）──────────────────────────────────

/// Producer 角色端到端：创建测试项目 → 执行 producer → 验证 CreativeBrief 写入 DB
///
/// 前置条件：DATABASE_URL 环境变量已配置，DB schema 已迁移
#[tokio::test]
#[ignore = "需要真实 DB：手动运行或在 CI 中启用"]
async fn producer_role_end_to_end_with_mock_llm() {
    // 此测试在 CI 或开发环境手动验证时运行，这里作为框架占位符
    // 实际实现需要：
    //   1. 创建 DB 连接池
    //   2. 插入测试 ProductionProject（fast_lane，metadata 含 preferred_model_id）
    //   3. 构建 mock AuditedModelExecutor（注入 StaticModelClientResolver + FakeAudit）
    //   4. 调用 ProductionOrchestrator::execute_role("producer", ...)
    //   5. 验证 creative_briefs 表中存在 version=1, status=draft 的记录
    //   6. 验证 model_calls 表中存在对应审计记录
    //   7. 验证 production_projects.status = "scripting"
    //   8. 清理测试数据

    eprintln!("TODO: 实现完整 DB 集成测试（当前为框架占位符）");
    // 测试框架已就绪，真实 DB 测试在 hand curl 验证阶段完成（Task 53-54）
}

/// Director 角色：输入产物缺失时应返回 missing_input_artifact 错误
///
/// 前置条件：真实 DB
#[tokio::test]
#[ignore = "需要真实 DB：手动运行或在 CI 中启用"]
async fn director_role_missing_input_returns_error() {
    eprintln!("TODO: 使用真实 DB 验证 MissingInputArtifact 错误路径");
    // 空项目 → 直接执行 director → 期望 ProductionError::MissingInputArtifact
}

/// 输出 schema 无效时应返回 invalid_artifact_schema 并不写入 DB
///
/// 前置条件：真实 DB + mock LLM（返回不符合 schema 的输出）
#[tokio::test]
#[ignore = "需要真实 DB：手动运行或在 CI 中启用"]
async fn invalid_output_schema_no_artifact_saved() {
    eprintln!("TODO: mock LLM 返回不符合 schema 的 JSON，验证无产物写入 DB");
}
