use axum::{http::header, response::IntoResponse, routing::post, Json, Router};
use novex_model::{LLMClient, LLMError, LLMPrompt, OpenAIClient, OpenAIConfig};
use serde_json::{json, Value};
use tokio::net::TcpListener;

fn responses_stream(text: &str) -> impl IntoResponse {
    let body = text
        .chars()
        .map(|character| {
            format!(
                "event: response.output_text.delta\ndata: {}\n\n",
                json!({
                    "type": "response.output_text.delta",
                    "delta": character.to_string()
                })
            )
        })
        .collect::<String>()
        + "event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n";

    ([(header::CONTENT_TYPE, "text/event-stream")], body)
}

#[tokio::test]
async fn openai_client_sends_chat_completion_request_and_reads_content() {
    async fn handler(Json(payload): Json<Value>) -> Json<Value> {
        assert_eq!(payload["model"], "test-model");
        assert_eq!(payload["messages"][0]["role"], "system");
        assert_eq!(payload["messages"][0]["content"], "system prompt");
        assert_eq!(payload["messages"][1]["role"], "user");
        assert_eq!(payload["messages"][1]["content"], "user prompt");
        assert_eq!(payload["response_format"]["type"], "json_object");

        Json(json!({
            "choices": [
                {
                    "message": {
                        "content": "{\"title\":\"测试标题\",\"hook\":\"测试hook\",\"scenes\":[]}"
                    }
                }
            ]
        }))
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/v1/chat/completions", post(handler));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = OpenAIClient::new(OpenAIConfig {
        api_key: "test-key".to_string(),
        base_url: format!("http://{address}/v1"),
        model: "test-model".to_string(),
        timeout_seconds: 5,
        responses_reasoning_effort: Some("low".to_string()),
        responses_max_output_tokens: 3000,
    })
    .unwrap();

    let response = client
        .generate_script(LLMPrompt {
            system: "system prompt".to_string(),
            user: "user prompt".to_string(),
            max_output_tokens: None,
        })
        .await
        .unwrap();

    assert!(response.contains("测试标题"));
}

#[tokio::test]
async fn openai_client_sends_responses_request_and_reads_output_text() {
    async fn handler(
        headers: axum::http::HeaderMap,
        Json(payload): Json<Value>,
    ) -> impl IntoResponse {
        assert_eq!(
            headers
                .get(axum::http::header::USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some("codex-cli/0.142.5")
        );
        assert_eq!(payload["model"], "test-model");
        assert_eq!(payload["input"][0]["role"], "system");
        assert_eq!(payload["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(payload["input"][0]["content"][0]["text"], "system prompt");
        assert_eq!(payload["input"][1]["role"], "user");
        assert_eq!(payload["input"][1]["content"][0]["type"], "input_text");
        assert_eq!(payload["input"][1]["content"][0]["text"], "user prompt");
        assert_eq!(payload["text"]["format"]["type"], "json_object");
        assert_eq!(payload["reasoning"]["effort"], "low");
        assert_eq!(payload["max_output_tokens"], 3000);
        assert_eq!(payload["stream"], true);

        responses_stream("{\"title\":\"响应测试标题\",\"hook\":\"测试hook\",\"scenes\":[]}")
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/responses", post(handler));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = OpenAIClient::new(OpenAIConfig {
        api_key: "test-key".to_string(),
        base_url: format!("http://{address}/responses"),
        model: "test-model".to_string(),
        timeout_seconds: 5,
        responses_reasoning_effort: Some("low".to_string()),
        responses_max_output_tokens: 3000,
    })
    .unwrap();

    let response = client
        .generate_script(LLMPrompt {
            system: "system prompt".to_string(),
            user: "user prompt".to_string(),
            max_output_tokens: None,
        })
        .await
        .unwrap();

    assert!(response.contains("响应测试标题"));
}

#[tokio::test]
async fn openai_client_uses_configured_responses_reasoning_and_token_limit() {
    async fn handler(Json(payload): Json<Value>) -> impl IntoResponse {
        assert_eq!(payload["reasoning"]["effort"], "high");
        assert_eq!(payload["max_output_tokens"], 4096);
        assert_eq!(payload["stream"], true);

        responses_stream("{\"title\":\"配置测试标题\",\"hook\":\"测试hook\",\"scenes\":[]}")
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/responses", post(handler));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = OpenAIClient::new(OpenAIConfig {
        api_key: "test-key".to_string(),
        base_url: format!("http://{address}/responses"),
        model: "test-model".to_string(),
        timeout_seconds: 5,
        responses_reasoning_effort: Some("high".to_string()),
        responses_max_output_tokens: 4096,
    })
    .unwrap();

    let response = client
        .generate_script(LLMPrompt {
            system: "system prompt".to_string(),
            user: "user prompt".to_string(),
            max_output_tokens: None,
        })
        .await
        .unwrap();

    assert!(response.contains("配置测试标题"));
}

#[tokio::test]
async fn openai_client_uses_prompt_output_token_limit_when_present() {
    async fn handler(Json(payload): Json<Value>) -> impl IntoResponse {
        assert_eq!(payload["max_output_tokens"], 900);

        responses_stream("{\"title\":\"单次预算测试\",\"hook\":\"测试hook\",\"scenes\":[]}")
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/responses", post(handler));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = OpenAIClient::new(OpenAIConfig {
        api_key: "test-key".to_string(),
        base_url: format!("http://{address}/responses"),
        model: "test-model".to_string(),
        timeout_seconds: 5,
        responses_reasoning_effort: Some("xhigh".to_string()),
        responses_max_output_tokens: 3000,
    })
    .unwrap();

    let response = client
        .generate_script(LLMPrompt {
            system: "system prompt".to_string(),
            user: "user prompt".to_string(),
            max_output_tokens: Some(900),
        })
        .await
        .unwrap();

    assert!(response.contains("单次预算测试"));
}

#[tokio::test]
async fn openai_client_omits_responses_reasoning_when_disabled() {
    async fn handler(Json(payload): Json<Value>) -> impl IntoResponse {
        assert!(payload.get("reasoning").is_none());
        assert_eq!(payload["max_output_tokens"], 3000);
        assert_eq!(payload["stream"], true);

        responses_stream("{\"title\":\"关闭推理测试\",\"hook\":\"测试hook\",\"scenes\":[]}")
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/responses", post(handler));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = OpenAIClient::new(OpenAIConfig {
        api_key: "test-key".to_string(),
        base_url: format!("http://{address}/responses"),
        model: "test-model".to_string(),
        timeout_seconds: 5,
        responses_reasoning_effort: None,
        responses_max_output_tokens: 3000,
    })
    .unwrap();

    let response = client
        .generate_script(LLMPrompt {
            system: "system prompt".to_string(),
            user: "user prompt".to_string(),
            max_output_tokens: None,
        })
        .await
        .unwrap();

    assert!(response.contains("关闭推理测试"));
}

#[tokio::test]
async fn openai_client_formats_provider_errors_with_status_and_body() {
    async fn handler() -> (axum::http::StatusCode, &'static str) {
        (axum::http::StatusCode::BAD_GATEWAY, "upstream failed")
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/responses", post(handler));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = OpenAIClient::new(OpenAIConfig {
        api_key: "test-key".to_string(),
        base_url: format!("http://{address}/responses"),
        model: "test-model".to_string(),
        timeout_seconds: 5,
        responses_reasoning_effort: None,
        responses_max_output_tokens: 3000,
    })
    .unwrap();

    let error = client
        .generate_script(LLMPrompt {
            system: "system prompt".to_string(),
            user: "user prompt".to_string(),
            max_output_tokens: None,
        })
        .await
        .unwrap_err();

    assert_eq!(
        error,
        LLMError::Provider("502 Bad Gateway: upstream failed".to_string())
    );
}
