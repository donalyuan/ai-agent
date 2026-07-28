//! Full Crew 真实媒体验收的只读授权清单；本模块不调用任何 provider。

use novex_ai_core::{canonical_json, sha256_hex};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionMediaValidationLimits {
    pub video_max_calls: u64,
    pub video_max_duration_seconds: u64,
    pub tts_max_calls: u64,
    pub tts_max_characters: u64,
    pub asr_max_calls: u64,
    pub asr_max_duration_seconds: u64,
    pub media_analysis_max_calls: u64,
    pub media_analysis_max_assets: u64,
    pub max_retries_per_capability: u64,
    pub video_max_cost_micros: u64,
    pub tts_max_cost_micros: u64,
    pub asr_max_cost_micros: u64,
    pub media_analysis_max_cost_micros: u64,
}

impl ProductionMediaValidationLimits {
    pub fn conservative_v3() -> Self {
        Self {
            video_max_calls: 1,
            video_max_duration_seconds: 5,
            tts_max_calls: 1,
            tts_max_characters: 100,
            asr_max_calls: 1,
            asr_max_duration_seconds: 10,
            media_analysis_max_calls: 1,
            media_analysis_max_assets: 2,
            max_retries_per_capability: 0,
            video_max_cost_micros: 500_000,
            tts_max_cost_micros: 100_000,
            asr_max_cost_micros: 100_000,
            media_analysis_max_cost_micros: 200_000,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaAnalysisCapability {
    pub provider_key: String,
    pub configuration_fingerprint: String,
    pub vision_capability_version: String,
    pub audio_capability_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MediaValidationModelBinding {
    pub model_id: Uuid,
    pub display_name: String,
    pub api_protocol: String,
    pub configuration_fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProductionMediaValidationItem {
    pub capability: String,
    pub approved_real_calls: bool,
    pub model_binding: Option<MediaValidationModelBinding>,
    pub analysis_capability: Option<MediaAnalysisCapability>,
    pub limits: Value,
    pub max_cost_micros: u64,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProductionMediaValidationTotals {
    pub max_real_calls: u64,
    pub max_retries: u64,
    pub max_cost_micros: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProductionMediaValidationPlan {
    pub schema_version: String,
    pub authorization_state: String,
    pub authorization_ready: bool,
    pub authorization_digest: String,
    pub items: Vec<ProductionMediaValidationItem>,
    pub totals: ProductionMediaValidationTotals,
    pub blockers: Vec<String>,
    pub external_effects: Value,
}

impl ProductionMediaValidationPlan {
    pub fn approved_items(
        &self,
        explicitly_confirmed: bool,
        confirmed_digest: &str,
    ) -> Result<Vec<ProductionMediaValidationItem>, ProductionMediaValidationError> {
        if !explicitly_confirmed || confirmed_digest != self.authorization_digest {
            return Err(ProductionMediaValidationError::ApprovalRequired);
        }
        if !self.authorization_ready {
            return Err(ProductionMediaValidationError::CapabilityBlocked(
                self.blockers.join("; "),
            ));
        }
        Ok(self
            .items
            .iter()
            .cloned()
            .map(|mut item| {
                item.approved_real_calls = true;
                item
            })
            .collect())
    }
}

pub async fn build_production_media_validation_plan(
    pool: &PgPool,
    analysis_capability: Option<MediaAnalysisCapability>,
    limits: ProductionMediaValidationLimits,
) -> Result<ProductionMediaValidationPlan, ProductionMediaValidationError> {
    validate_limits(&limits)?;
    let video = selected_model(
        pool,
        "video",
        &["volcengine_ark_video", "runway_api", "kling_api"],
    )
    .await?;
    let tts = selected_model(
        pool,
        "speech",
        &["volcengine_tts_v3", "openai_audio_speech"],
    )
    .await?;
    let asr = selected_model(pool, "speech", &["volcengine_asr_v3"]).await?;

    let mut items = vec![
        model_item(
            "video_generation",
            video,
            json!({
                "max_calls": limits.video_max_calls,
                "max_duration_seconds": limits.video_max_duration_seconds,
                "max_retries": limits.max_retries_per_capability,
            }),
            limits.video_max_cost_micros,
        ),
        model_item(
            "tts",
            tts,
            json!({
                "max_calls": limits.tts_max_calls,
                "max_characters": limits.tts_max_characters,
                "max_retries": limits.max_retries_per_capability,
            }),
            limits.tts_max_cost_micros,
        ),
        model_item(
            "asr",
            asr,
            json!({
                "max_calls": limits.asr_max_calls,
                "max_duration_seconds": limits.asr_max_duration_seconds,
                "max_retries": limits.max_retries_per_capability,
            }),
            limits.asr_max_cost_micros,
        ),
    ];
    let analysis_blockers = match &analysis_capability {
        Some(capability)
            if !capability.provider_key.trim().is_empty()
                && capability.configuration_fingerprint.len() == 64
                && !capability.vision_capability_version.trim().is_empty()
                && !capability.audio_capability_version.trim().is_empty() =>
        {
            Vec::new()
        }
        _ => vec![
            "MediaEvidenceProvider 缺少精确 configuration fingerprint 或 vision/audio 能力版本"
                .into(),
        ],
    };
    items.push(ProductionMediaValidationItem {
        capability: "media_analysis".into(),
        approved_real_calls: false,
        model_binding: None,
        analysis_capability,
        limits: json!({
            "max_calls": limits.media_analysis_max_calls,
            "max_assets": limits.media_analysis_max_assets,
            "requires_vision": true,
            "requires_audio_or_asr": true,
            "max_retries": limits.max_retries_per_capability,
        }),
        max_cost_micros: limits.media_analysis_max_cost_micros,
        blockers: analysis_blockers,
    });

    let blockers = items
        .iter()
        .flat_map(|item| {
            item.blockers
                .iter()
                .map(|blocker| format!("{}: {blocker}", item.capability))
        })
        .collect::<Vec<_>>();
    let max_real_calls = limits
        .video_max_calls
        .checked_add(limits.tts_max_calls)
        .and_then(|value| value.checked_add(limits.asr_max_calls))
        .and_then(|value| value.checked_add(limits.media_analysis_max_calls))
        .ok_or_else(|| ProductionMediaValidationError::InvalidPlan("调用上限溢出".into()))?;
    let totals = ProductionMediaValidationTotals {
        max_real_calls,
        max_retries: limits
            .max_retries_per_capability
            .checked_mul(items.len() as u64)
            .ok_or_else(|| ProductionMediaValidationError::InvalidPlan("重试上限溢出".into()))?,
        max_cost_micros: items.iter().try_fold(0_u64, |total, item| {
            total
                .checked_add(item.max_cost_micros)
                .ok_or_else(|| ProductionMediaValidationError::InvalidPlan("成本上限溢出".into()))
        })?,
    };
    let digest_payload = json!({
        "schema_version": "1",
        "items": items,
        "totals": totals,
        "blockers": blockers,
    });
    let authorization_digest = sha256_hex(canonical_json(&digest_payload).as_bytes());
    Ok(ProductionMediaValidationPlan {
        schema_version: "1".into(),
        authorization_state: "awaiting_explicit_user_confirmation".into(),
        authorization_ready: blockers.is_empty(),
        authorization_digest,
        items,
        totals,
        blockers,
        external_effects: json!({
            "video_generation_calls": 0,
            "tts_calls": 0,
            "asr_calls": 0,
            "media_analysis_calls": 0,
            "provider_tasks_created": 0,
        }),
    })
}

fn model_item(
    capability: &str,
    model_binding: Option<MediaValidationModelBinding>,
    limits: Value,
    max_cost_micros: u64,
) -> ProductionMediaValidationItem {
    let blockers = model_binding
        .is_none()
        .then(|| vec!["没有 enabled provider model".into()])
        .unwrap_or_default();
    ProductionMediaValidationItem {
        capability: capability.into(),
        approved_real_calls: false,
        model_binding,
        analysis_capability: None,
        limits,
        max_cost_micros,
        blockers,
    }
}

async fn selected_model(
    pool: &PgPool,
    model_type: &str,
    protocols: &[&str],
) -> Result<Option<MediaValidationModelBinding>, ProductionMediaValidationError> {
    let protocols = protocols
        .iter()
        .map(|protocol| protocol.to_string())
        .collect::<Vec<_>>();
    let row = sqlx::query(
        r#"
        SELECT id, display_name, model_type, provider_name, api_protocol,
               protocol_version, request_base_url, upstream_model, timeout_seconds,
               reasoning_effort, max_output_tokens, context_window,
               tokenizer_profile_key, tokenizer_profile_version, settings, version
        FROM ai_models
        WHERE model_type = $1 AND api_protocol = ANY($2)
          AND status = 'enabled' AND deleted_at IS NULL
        ORDER BY is_default DESC, sort_order, created_at, id
        LIMIT 1
        "#,
    )
    .bind(model_type)
    .bind(protocols)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let snapshot = json!({
        "id": row.try_get::<Uuid, _>("id")?,
        "display_name": row.try_get::<String, _>("display_name")?,
        "model_type": row.try_get::<String, _>("model_type")?,
        "provider_name": row.try_get::<String, _>("provider_name")?,
        "api_protocol": row.try_get::<String, _>("api_protocol")?,
        "protocol_version": row.try_get::<String, _>("protocol_version")?,
        "request_base_url": row.try_get::<String, _>("request_base_url")?,
        "upstream_model": row.try_get::<String, _>("upstream_model")?,
        "timeout_seconds": row.try_get::<i32, _>("timeout_seconds")?,
        "reasoning_effort": row.try_get::<Option<String>, _>("reasoning_effort")?,
        "max_output_tokens": row.try_get::<Option<i32>, _>("max_output_tokens")?,
        "context_window": row.try_get::<Option<i64>, _>("context_window")?,
        "tokenizer_profile_key": row.try_get::<Option<String>, _>("tokenizer_profile_key")?,
        "tokenizer_profile_version": row.try_get::<Option<String>, _>("tokenizer_profile_version")?,
        "settings": row.try_get::<Value, _>("settings")?,
        "version": row.try_get::<i64, _>("version")?,
    });
    Ok(Some(MediaValidationModelBinding {
        model_id: row.try_get("id")?,
        display_name: row.try_get("display_name")?,
        api_protocol: row.try_get("api_protocol")?,
        configuration_fingerprint: sha256_hex(canonical_json(&snapshot).as_bytes()),
    }))
}

fn validate_limits(
    limits: &ProductionMediaValidationLimits,
) -> Result<(), ProductionMediaValidationError> {
    if limits.video_max_calls == 0
        || limits.video_max_duration_seconds == 0
        || limits.tts_max_calls == 0
        || limits.tts_max_characters == 0
        || limits.asr_max_calls == 0
        || limits.asr_max_duration_seconds == 0
        || limits.media_analysis_max_calls == 0
        || limits.media_analysis_max_assets == 0
        || limits.video_max_cost_micros == 0
        || limits.tts_max_cost_micros == 0
        || limits.asr_max_cost_micros == 0
        || limits.media_analysis_max_cost_micros == 0
    {
        return Err(ProductionMediaValidationError::InvalidPlan(
            "调用、资源和成本上限必须为正值".into(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub enum ProductionMediaValidationError {
    Storage(sqlx::Error),
    InvalidPlan(String),
    ApprovalRequired,
    CapabilityBlocked(String),
}

impl fmt::Display for ProductionMediaValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "媒体验收计划查询失败: {error}"),
            Self::InvalidPlan(reason) => write!(formatter, "媒体验收计划无效: {reason}"),
            Self::ApprovalRequired => formatter.write_str("真实媒体验收需要确认精确授权 digest"),
            Self::CapabilityBlocked(reason) => write!(formatter, "真实媒体验收能力阻断: {reason}"),
        }
    }
}

impl std::error::Error for ProductionMediaValidationError {}

impl From<sqlx::Error> for ProductionMediaValidationError {
    fn from(value: sqlx::Error) -> Self {
        Self::Storage(value)
    }
}
