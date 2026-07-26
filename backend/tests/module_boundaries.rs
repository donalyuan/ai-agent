use novex_agent::{AgentInvocation, AgentRunCoordinator};
use novex_api::application::agents::adapters::{
    ScriptAgentAdapter, SoundAgentAdapter, TopicAgentAdapter, WorkAgentAdapter,
};
use novex_api::application::ai_models::AiModelService;
use novex_api::application::asset_generation::AssetGenerationService;
use novex_api::application::conversations::ConversationService;
use novex_api::application::health::HealthService;
use novex_api::application::materials::MaterialService;
use novex_api::application::projects::ProjectService;
use novex_api::application::scripts::ScriptService;
use novex_api::application::topics::TopicService;
use novex_api::bootstrap::{AppConfig, AppState};
use novex_api::domain::conversation::{AgentConversation, AgentMessage};
use novex_api::domain::script::{Scene, Script, ScriptStatus};
use novex_api::domain::topic::{ContentTopic, ContentTopicStatus};

#[test]
fn layered_public_modules_are_available() {
    fn assert_type<T>() {}

    assert_type::<AgentRunCoordinator>();
    assert_type::<AiModelService>();
    assert_type::<ProjectService>();
    assert_type::<MaterialService>();
    assert_type::<ScriptService>();
    assert_type::<ConversationService>();
    assert_type::<HealthService>();
    assert_type::<TopicService>();
    assert_type::<AssetGenerationService>();
    assert_type::<AgentInvocation>();
    assert_type::<ScriptAgentAdapter>();
    assert_type::<TopicAgentAdapter>();
    assert_type::<SoundAgentAdapter>();
    assert_type::<WorkAgentAdapter>();
    assert_type::<AppConfig>();
    assert_type::<AppState>();
    assert_type::<AgentConversation>();
    assert_type::<AgentMessage>();
    assert_type::<Scene>();
    assert_type::<Script>();
    assert_type::<ScriptStatus>();
    assert_type::<ContentTopic>();
    assert_type::<ContentTopicStatus>();
}

#[test]
fn api_layer_does_not_execute_sql_or_build_model_prompts() {
    let api_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api");
    let mut rust_sources = Vec::new();
    collect_rust_sources(&api_dir, &mut rust_sources);

    for (path, source) in rust_sources {
        for forbidden in [
            "sqlx::",
            "query!",
            "query_as!",
            "LLMPrompt",
            "LLMJsonSchema",
        ] {
            assert!(
                !source.contains(forbidden),
                "API layer {} must not contain {forbidden}",
                path.display()
            );
        }
    }
}

fn collect_rust_sources(
    directory: &std::path::Path,
    sources: &mut Vec<(std::path::PathBuf, String)>,
) {
    for entry in std::fs::read_dir(directory).expect("API source directory should be readable") {
        let path = entry.expect("API source entry should be readable").path();
        if path.is_dir() {
            collect_rust_sources(&path, sources);
        } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("rs") {
            let source = std::fs::read_to_string(&path).expect("Rust source should be readable");
            sources.push((path, source));
        }
    }
}

#[test]
fn lower_layers_do_not_depend_on_http_api_modules() {
    let lower_layer_sources = [
        ("agents/llm.rs", include_str!("../src/agents/llm.rs")),
        (
            "agents/script_agent.rs",
            include_str!("../src/agents/script_agent.rs"),
        ),
        (
            "application/agents/adapters/mod.rs",
            include_str!("../src/application/agents/adapters/mod.rs"),
        ),
        (
            "bootstrap/state.rs",
            include_str!("../src/bootstrap/state.rs"),
        ),
    ];

    for (path, source) in lower_layer_sources {
        assert!(
            !source.contains("crate::api"),
            "lower layer {path} must not import the HTTP API layer"
        );
    }

    let workspace_application = include_str!("../src/application/workspace.rs");
    assert!(
        !workspace_application.contains("crate::bootstrap"),
        "application/workspace.rs must receive repositories instead of AppState"
    );
}

#[test]
fn foundation_agent_crates_do_not_depend_on_backend_or_transport_layers() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend must live under the workspace root");

    for crate_name in ["novex-ai-core", "novex-agent"] {
        let crate_dir = workspace_root.join("crates").join(crate_name);
        let manifest = std::fs::read_to_string(crate_dir.join("Cargo.toml"))
            .expect("foundation crate manifest should be readable");
        assert!(
            !manifest.contains("novex-api") && !manifest.contains("backend"),
            "{crate_name} must not depend on backend"
        );
        assert!(
            !manifest.contains("axum") && !manifest.contains("sqlx"),
            "{crate_name} must not depend on Axum or SQLx"
        );

        let mut sources = Vec::new();
        collect_rust_sources(&crate_dir.join("src"), &mut sources);
        for (path, source) in sources {
            for forbidden in ["novex_api", "crate::api", "axum::", "sqlx::"] {
                assert!(
                    !source.contains(forbidden),
                    "foundation source {} must not contain {forbidden}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn agent_kernel_has_no_business_dispatch_or_legacy_runtime_path() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend must live under the workspace root");
    let kernel_source =
        std::fs::read_to_string(workspace_root.join("crates/novex-agent/src/lib.rs"))
            .expect("novex-agent source should be readable");
    for forbidden in [
        "\"script\"",
        "\"topic\"",
        "\"sound\"",
        "\"work\"",
        "match conversation.agent_type",
    ] {
        assert!(
            !kernel_source.contains(forbidden),
            "novex-agent kernel must not contain business dispatch token {forbidden}"
        );
    }

    let legacy_path = workspace_root.join("backend/src/application/agents/runtime");
    assert!(
        !legacy_path.exists(),
        "legacy Agent Runtime path must be removed"
    );

    let adapter_source = std::fs::read_to_string(
        workspace_root.join("backend/src/application/agents/adapters/mod.rs"),
    )
    .expect("adapter module should be readable");
    assert!(!adapter_source.contains("Option<Arc<dyn TopicRepository"));
    assert!(!adapter_source.contains("Option<Arc<PostgresVoiceCatalogRepository"));
    assert!(!adapter_source.contains("Option<Arc<PostgresWorkLibraryRepository"));
}

#[test]
fn production_model_adapters_cannot_bypass_the_audited_executor() {
    let backend = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    collect_rust_sources(&backend.join("src/application"), &mut sources);
    collect_rust_sources(&backend.join("src/agents"), &mut sources);

    for (path, source) in sources {
        for forbidden in [
            "PromptCompileInput",
            "DynamicFragment",
            "PromptCompiler",
            "ContextCompiler",
            "PostgresModelCallRepository",
            "PostgresContextAuditRepository",
        ] {
            assert!(
                !source.contains(forbidden),
                "production model path {} must not obtain bypass primitive {forbidden}",
                path.display()
            );
        }
    }

    let bootstrap = std::fs::read_to_string(backend.join("src/bootstrap/state.rs"))
        .expect("bootstrap state should be readable");
    assert_eq!(
        bootstrap.matches("AuditedModelExecutor::new").count(),
        1,
        "AuditedModelExecutor must have one production bootstrap assembly point"
    );
}
