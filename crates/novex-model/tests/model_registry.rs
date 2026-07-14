use novex_model::{ApiProtocol, AuthScheme, ModelSettings, ModelType};
use serde_json::json;
use std::str::FromStr;

#[test]
fn protocol_must_match_model_type_and_auth_scheme() {
    assert!(ApiProtocol::OpenAiResponses.supports(ModelType::Text));
    assert!(ApiProtocol::OpenAiChatCompletions.supports(ModelType::Text));
    assert!(!ApiProtocol::OpenAiResponses.supports(ModelType::Image));
    assert!(ApiProtocol::OpenAiImages.supports(ModelType::Image));
    assert!(ApiProtocol::VolcengineArkImages.supports(ModelType::Image));
    assert!(ApiProtocol::RunwayApi.supports(ModelType::Video));
    assert!(ApiProtocol::KlingApi.supports(ModelType::Video));

    assert!(!ApiProtocol::VolcengineArkImages.supports(ModelType::Text));
    assert!(!ApiProtocol::OpenAiChatCompletions.supports(ModelType::Image));
    assert!(!ApiProtocol::OpenAiResponses.supports(ModelType::Video));
    assert_eq!(ApiProtocol::RunwayApi.required_auth(), AuthScheme::Bearer);
    assert_eq!(
        ApiProtocol::VolcengineArkImages.required_auth(),
        AuthScheme::Bearer
    );
    assert_eq!(
        ApiProtocol::from_str("volcengine_ark_images").unwrap(),
        ApiProtocol::VolcengineArkImages
    );
    assert!(ApiProtocol::from_str("jimeng_visual").is_err());
}

#[test]
fn settings_are_deserialized_into_model_specific_types() {
    let image = ModelSettings::parse(
        ModelType::Image,
        json!({
            "supported_sizes": ["1024x1024", "1536x1024"],
            "default_size": "1024x1024",
            "max_images_per_request": 4
        }),
    )
    .expect("valid image settings should parse");
    assert_eq!(image.default_image_size(), Some("1024x1024"));

    let video = ModelSettings::parse(
        ModelType::Video,
        json!({
            "resolutions": ["1080p"],
            "aspect_ratios": ["9:16"],
            "min_duration_seconds": 5,
            "max_duration_seconds": 10
        }),
    )
    .expect("valid video settings should parse");
    assert_eq!(video.video_duration_range(), Some((5, 10)));

    assert!(ModelSettings::parse(
        ModelType::Image,
        json!({"default_size": "1024x1024", "unknown": true})
    )
    .is_err());
    assert!(ModelSettings::parse(
        ModelType::Image,
        json!({"request_key": "legacy-jimeng-key"})
    )
    .is_err());
}

#[test]
fn protocol_and_model_values_have_stable_storage_names() {
    assert_eq!(ModelType::Text.as_str(), "text");
    assert_eq!(ApiProtocol::OpenAiResponses.as_str(), "openai_responses");
    assert_eq!(
        ApiProtocol::VolcengineArkImages.as_str(),
        "volcengine_ark_images"
    );
    assert_eq!(AuthScheme::AccessKeySecret.as_str(), "access_key_secret");
}
