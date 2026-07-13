use novex_api::api::asset_generation::dto::{
    AssetGenerationPlanRequest, AssetGenerationTaskRequest,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn asset_generation_requests_require_image_model_id_instead_of_provider() {
    let model_id = Uuid::new_v4();
    let plan: AssetGenerationPlanRequest = serde_json::from_value(json!({
        "model_id": model_id,
        "image_candidates_per_scene": 3,
        "use_reference_materials": true
    }))
    .unwrap();
    let task: AssetGenerationTaskRequest = serde_json::from_value(json!({
        "model_id": model_id,
        "image_candidates_per_scene": 3,
        "use_reference_materials": true
    }))
    .unwrap();

    assert_eq!(plan.model_id, model_id);
    assert_eq!(task.model_id, model_id);
    assert!(plan.validate_for_api().is_ok());
    assert!(task.validate_for_api().is_ok());
    assert!(serde_json::from_value::<AssetGenerationTaskRequest>(json!({
        "provider": "gpt-image-2",
        "image_candidates_per_scene": 3
    }))
    .is_err());
}
