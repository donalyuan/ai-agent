use axum::{http::header, response::IntoResponse, routing::post, Json, Router};
use novex_model::{LLMClient, LLMError, LLMJsonSchema, LLMPrompt, OpenAIClient, OpenAIConfig};
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

fn topic_batch_schema() -> LLMJsonSchema {
    LLMJsonSchema {
        name: "topic_batch".to_string(),
        strict: true,
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["topics"],
            "properties": {
                "topics": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": [
                            "title",
                            "angle",
                            "target_audience",
                            "hook_points",
                            "content_type",
                            "score",
                            "score_reason",
                            "tags"
                        ],
                        "properties": {
                            "title": { "type": "string", "minLength": 1 },
                            "angle": { "type": "string", "minLength": 1 },
                            "target_audience": { "type": "string", "minLength": 1 },
                            "hook_points": {
                                "type": "array",
                                "minItems": 1,
                                "items": { "type": "string", "minLength": 1 }
                            },
                            "content_type": { "type": "string", "minLength": 1 },
                            "score": { "type": "number", "minimum": 0, "maximum": 100 },
                            "score_reason": { "type": "string", "minLength": 1 },
                            "tags": {
                                "type": "array",
                                "minItems": 1,
                                "items": { "type": "string", "minLength": 1 }
                            }
                        }
                    }
                }
            }
        }),
    }
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
            output_schema: None,
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
            output_schema: None,
        })
        .await
        .unwrap();

    assert!(response.contains("响应测试标题"));
}

#[tokio::test]
async fn openai_client_sends_responses_json_schema_when_prompt_has_schema() {
    async fn handler(Json(payload): Json<Value>) -> impl IntoResponse {
        assert_eq!(payload["text"]["format"]["type"], "json_schema");
        assert_eq!(payload["text"]["format"]["name"], "topic_batch");
        assert_eq!(payload["text"]["format"]["strict"], true);
        assert_eq!(
            payload["text"]["format"]["schema"]["required"],
            json!(["topics"])
        );
        assert!(payload["text"]["format"]["schema"]["additionalProperties"] == false);

        responses_stream("{\"topics\":[]}")
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
            output_schema: Some(topic_batch_schema()),
        })
        .await
        .unwrap();

    assert_eq!(response, "{\"topics\":[]}");
}

#[tokio::test]
async fn openai_client_sends_chat_completion_json_schema_when_prompt_has_schema() {
    async fn handler(Json(payload): Json<Value>) -> Json<Value> {
        assert_eq!(payload["response_format"]["type"], "json_schema");
        assert_eq!(
            payload["response_format"]["json_schema"]["name"],
            "topic_batch"
        );
        assert_eq!(payload["response_format"]["json_schema"]["strict"], true);
        assert_eq!(
            payload["response_format"]["json_schema"]["schema"]["required"],
            json!(["topics"])
        );
        assert!(payload["response_format"].get("schema").is_none());
        assert!(payload["response_format"].get("name").is_none());

        Json(json!({
            "choices": [
                {
                    "message": {
                        "content": "{\"topics\":[]}"
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
            output_schema: Some(topic_batch_schema()),
        })
        .await
        .unwrap();

    assert_eq!(response, "{\"topics\":[]}");
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
            output_schema: None,
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
            output_schema: None,
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
            output_schema: None,
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
            output_schema: None,
        })
        .await
        .unwrap_err();

    assert_eq!(
        error,
        LLMError::Provider("502 Bad Gateway: upstream failed".to_string())
    );
}
