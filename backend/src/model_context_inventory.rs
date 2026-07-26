//! Read-only governed Context readiness report for enabled text models.

use novex_ai_core::{behavior_fingerprint, DefinitionRegistry, DefinitionStatus, ModelBehavior};
use novex_model::ApiProtocol;
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextReadiness {
    Ready,
    ConfigurationMissing,
    ProfileUnavailable,
    ProfileIncompatible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingBehaviorState {
    Unbound,
    Stable,
    BehaviorDrift,
    NotComparable,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModelContextInventoryItem {
    pub model_id: Uuid,
    pub display_name: String,
    pub provider_name: String,
    pub api_protocol: String,
    pub upstream_model: String,
    pub upstream_model_evidence: &'static str,
    pub context_window: Option<i64>,
    pub tokenizer_profile_key: Option<String>,
    pub tokenizer_profile_version: Option<String>,
    pub profile_status: Option<String>,
    pub protocol_applicable: Option<bool>,
    pub declared_model_families: Vec<String>,
    pub profile_selection_is_operator_evidence: bool,
    pub readiness: ContextReadiness,
    pub behavior_fingerprint: Option<String>,
    pub binding_behavior_state: BindingBehaviorState,
    pub credential_rotation_changes_fingerprint: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModelContextInventoryReport {
    pub schema_version: &'static str,
    pub read_only: bool,
    pub credential_columns_selected: bool,
    pub models: Vec<ModelContextInventoryItem>,
}

pub async fn build_model_context_inventory(
    pool: &PgPool,
    definitions: &DefinitionRegistry,
) -> Result<ModelContextInventoryReport, sqlx::Error> {
    // Credential columns are intentionally absent from this projection.
    let rows = sqlx::query(
        r#"
        SELECT id, display_name, provider_name, api_protocol, request_base_url,
               upstream_model, reasoning_effort, max_output_tokens, context_window,
               tokenizer_profile_key, tokenizer_profile_version, settings
        FROM ai_models
        WHERE model_type = 'text' AND status = 'enabled' AND deleted_at IS NULL
        ORDER BY display_name, id
        "#,
    )
    .fetch_all(pool)
    .await?;
    let binding_rows = sqlx::query(
        r#"
        SELECT model_id, behavior_fingerprint
        FROM agent_conversation_bindings
        WHERE model_id IS NOT NULL AND behavior_fingerprint IS NOT NULL
        UNION ALL
        SELECT model_id, behavior_fingerprint
        FROM agent_run_bindings
        WHERE model_id IS NOT NULL
        "#,
    )
    .fetch_all(pool)
    .await?;
    let mut bound_fingerprints = HashMap::<Uuid, Vec<String>>::new();
    for row in binding_rows {
        bound_fingerprints
            .entry(row.get("model_id"))
            .or_default()
            .push(row.get("behavior_fingerprint"));
    }

    let mut models = Vec::with_capacity(rows.len());
    for row in rows {
        let model_id: Uuid = row.get("id");
        let protocol_value: String = row.get("api_protocol");
        let protocol = ApiProtocol::from_str(&protocol_value).ok();
        let context_window: Option<i64> = row.get("context_window");
        let profile_key: Option<String> = row.get("tokenizer_profile_key");
        let profile_version: Option<String> = row.get("tokenizer_profile_version");
        let profile = profile_key
            .as_deref()
            .zip(profile_version.as_deref())
            .and_then(|(key, version)| definitions.tokenizer_profile(key, version).ok());
        let protocol_applicable = profile.map(|item| {
            item.applicable_protocols
                .iter()
                .any(|candidate| candidate == &protocol_value)
        });
        let readiness = if context_window.is_none()
            || profile_key.is_none()
            || profile_version.is_none()
            || row.get::<Option<i32>, _>("max_output_tokens").is_none()
        {
            ContextReadiness::ConfigurationMissing
        } else if profile.is_none()
            || profile.is_some_and(|item| {
                matches!(
                    item.status,
                    DefinitionStatus::Candidate | DefinitionStatus::Revoked
                )
            })
        {
            ContextReadiness::ProfileUnavailable
        } else if protocol.is_none() || protocol_applicable != Some(true) {
            ContextReadiness::ProfileIncompatible
        } else {
            ContextReadiness::Ready
        };
        let behavior_fingerprint = if readiness == ContextReadiness::Ready {
            let behavior = ModelBehavior {
                protocol: protocol_value.clone(),
                request_base_url: row.get("request_base_url"),
                upstream_model: row.get("upstream_model"),
                reasoning_effort: row.get("reasoning_effort"),
                max_output_tokens: row.get::<i32, _>("max_output_tokens") as u32,
                context_window: context_window.expect("ready context window") as u64,
                tokenizer_profile_key: profile_key.clone().expect("ready profile key"),
                tokenizer_profile_version: profile_version.clone().expect("ready profile version"),
                settings: row.get::<Value, _>("settings"),
            };
            behavior_fingerprint(&behavior).ok().map(|value| value.0)
        } else {
            None
        };
        let bindings = bound_fingerprints.get(&model_id);
        let binding_behavior_state = match (bindings, behavior_fingerprint.as_deref()) {
            (None, Some(_)) => BindingBehaviorState::Unbound,
            (Some(values), Some(current))
                if values.iter().all(|fingerprint| fingerprint == current) =>
            {
                BindingBehaviorState::Stable
            }
            (Some(_), Some(_)) => BindingBehaviorState::BehaviorDrift,
            _ => BindingBehaviorState::NotComparable,
        };
        models.push(ModelContextInventoryItem {
            model_id,
            display_name: row.get("display_name"),
            provider_name: row.get("provider_name"),
            api_protocol: protocol_value,
            upstream_model: row.get("upstream_model"),
            upstream_model_evidence: "opaque_not_inferred",
            context_window,
            tokenizer_profile_key: profile_key,
            tokenizer_profile_version: profile_version,
            profile_status: profile.map(|item| definition_status(item.status).to_string()),
            protocol_applicable,
            declared_model_families: profile
                .map(|item| item.applicable_model_families.clone())
                .unwrap_or_default(),
            profile_selection_is_operator_evidence: profile.is_some(),
            readiness,
            behavior_fingerprint,
            binding_behavior_state,
            credential_rotation_changes_fingerprint: false,
        });
    }
    Ok(ModelContextInventoryReport {
        schema_version: "1",
        read_only: true,
        credential_columns_selected: false,
        models,
    })
}

fn definition_status(status: DefinitionStatus) -> &'static str {
    match status {
        DefinitionStatus::Candidate => "candidate",
        DefinitionStatus::Active => "active",
        DefinitionStatus::Supported => "supported",
        DefinitionStatus::Revoked => "revoked",
    }
}
