use novex_api::application::agents::runtime::{AgentRuntime, AgentTurnRequest};
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

    assert_type::<AgentRuntime>();
    assert_type::<AiModelService>();
    assert_type::<ProjectService>();
    assert_type::<MaterialService>();
    assert_type::<ScriptService>();
    assert_type::<ConversationService>();
    assert_type::<HealthService>();
    assert_type::<TopicService>();
    assert_type::<AssetGenerationService>();
    assert_type::<AgentTurnRequest>();
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
            "application/agents/runtime/mod.rs",
            include_str!("../src/application/agents/runtime/mod.rs"),
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
