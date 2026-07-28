use super::{canonical_digest, domain_error};
use crate::{ProductionError, ProductionResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeInput {
    pub generation_step_id: Uuid,
    pub generation_attempt_id: Uuid,
    pub output_artifact_id: Uuid,
    pub segment_key: String,
    pub scene_ids: Vec<Uuid>,
    pub shot_contracts: Vec<(Uuid, Vec<Uuid>)>,
    pub consumed_by_final_compose: bool,
    pub generation_succeeded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredTake {
    pub take_id: Uuid,
    pub ordinal: usize,
    pub generation_step_id: Uuid,
    pub generation_attempt_id: Uuid,
    pub output_artifact_id: Uuid,
    pub segment_key: String,
    pub scene_ids: Vec<Uuid>,
    pub scene_shot_map: BTreeMap<Uuid, Vec<Uuid>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredTakeInventory {
    pub work_version_id: Uuid,
    pub takes: Vec<RequiredTake>,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalMediaAsset {
    pub artifact_id: Uuid,
    pub sha256: String,
    pub mime_type: String,
    pub duration_ms: u64,
}

impl FinalMediaAsset {
    pub fn validate(&self) -> ProductionResult<()> {
        if !valid_digest(&self.sha256)
            || !self.mime_type.starts_with("video/")
            || self.duration_ms == 0
        {
            return Err(domain_error("final media identity is incomplete"));
        }
        Ok(())
    }
}

/// 当前 WorkGenerationRun 的 final compose 实际消费清单；take 与 Shot 是集合映射。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredTakeInventorySnapshot {
    pub inventory_id: Uuid,
    pub run_id: Uuid,
    pub source_step_id: Uuid,
    pub source_attempt: u32,
    pub revision_epoch: u32,
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub work_generation_run_id: Uuid,
    pub final_asset: FinalMediaAsset,
    pub work_version_hash: String,
    pub takes: Vec<RequiredTake>,
    pub inventory_digest: String,
}

impl RequiredTakeInventorySnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        inventory_id: Uuid,
        run_id: Uuid,
        source_step_id: Uuid,
        source_attempt: u32,
        revision_epoch: u32,
        work_id: Uuid,
        work_version_id: Uuid,
        work_generation_run_id: Uuid,
        final_asset: FinalMediaAsset,
        work_version_hash: String,
        inputs: Vec<ComposeInput>,
    ) -> ProductionResult<Self> {
        final_asset.validate()?;
        if source_attempt == 0 || !valid_digest(&work_version_hash) {
            return Err(domain_error(
                "required take inventory lacks source attempt or WorkVersion hash",
            ));
        }
        let inventory = build_required_take_inventory(work_version_id, inputs)?;
        let inventory_digest = canonical_digest(&(
            run_id,
            source_step_id,
            source_attempt,
            revision_epoch,
            work_id,
            work_version_id,
            work_generation_run_id,
            &final_asset,
            work_version_hash.as_str(),
            &inventory.takes,
        ))?;
        Ok(Self {
            inventory_id,
            run_id,
            source_step_id,
            source_attempt,
            revision_epoch,
            work_id,
            work_version_id,
            work_generation_run_id,
            final_asset,
            work_version_hash,
            takes: inventory.takes,
            inventory_digest,
        })
    }

    pub fn validate(&self) -> ProductionResult<()> {
        self.final_asset.validate()?;
        if self.source_attempt == 0
            || self.takes.is_empty()
            || !valid_digest(&self.work_version_hash)
            || !valid_digest(&self.inventory_digest)
        {
            return Err(domain_error(
                "required take inventory snapshot is incomplete",
            ));
        }
        let mut identities = BTreeSet::new();
        for (ordinal, take) in self.takes.iter().enumerate() {
            if take.ordinal != ordinal
                || take.segment_key.trim().is_empty()
                || take.scene_ids.is_empty()
                || !identities.insert((take.generation_attempt_id, take.output_artifact_id))
                || take
                    .scene_ids
                    .iter()
                    .any(|scene_id| take.scene_shot_map.get(scene_id).is_none_or(Vec::is_empty))
                || take.scene_shot_map.len() != take.scene_ids.len()
            {
                return Err(domain_error(
                    "required take inventory contains ambiguous provenance",
                ));
            }
        }
        let expected = canonical_digest(&(
            self.run_id,
            self.source_step_id,
            self.source_attempt,
            self.revision_epoch,
            self.work_id,
            self.work_version_id,
            self.work_generation_run_id,
            &self.final_asset,
            self.work_version_hash.as_str(),
            &self.takes,
        ))?;
        if expected != self.inventory_digest {
            return Err(domain_error(
                "required take inventory digest is not canonical",
            ));
        }
        Ok(())
    }
}

impl RequiredTakeInventory {
    pub fn required_shot_ids(&self) -> Vec<Uuid> {
        let mut shots = BTreeSet::new();
        for take in &self.takes {
            for values in take.scene_shot_map.values() {
                shots.extend(values.iter().copied());
            }
        }
        shots.into_iter().collect()
    }
}

pub fn build_required_take_inventory(
    work_version_id: Uuid,
    inputs: Vec<ComposeInput>,
) -> ProductionResult<RequiredTakeInventory> {
    let consumed: Vec<_> = inputs
        .into_iter()
        .filter(|input| input.consumed_by_final_compose && input.generation_succeeded)
        .collect();
    if consumed.is_empty() {
        return Err(domain_error(
            "final compose has no successful consumed output",
        ));
    }
    let mut attempt_assets = BTreeSet::new();
    let mut takes = Vec::with_capacity(consumed.len());
    for (ordinal, input) in consumed.into_iter().enumerate() {
        if input.segment_key.trim().is_empty() || input.scene_ids.is_empty() {
            return Err(domain_error(
                "required take segment and scenes must be non-empty",
            ));
        }
        if !attempt_assets.insert((input.generation_attempt_id, input.output_artifact_id)) {
            return Err(domain_error("duplicate generation attempt/output mapping"));
        }
        let scene_set: BTreeSet<_> = input.scene_ids.iter().copied().collect();
        if scene_set.len() != input.scene_ids.len() {
            return Err(domain_error("required take contains duplicate scene"));
        }
        let mut scene_shot_map = BTreeMap::new();
        for (scene_id, shot_ids) in input.shot_contracts {
            let unique: BTreeSet<_> = shot_ids.iter().copied().collect();
            if !scene_set.contains(&scene_id)
                || shot_ids.is_empty()
                || unique.len() != shot_ids.len()
            {
                return Err(domain_error("ambiguous scene/shot mapping"));
            }
            scene_shot_map.insert(scene_id, shot_ids);
        }
        if scene_set
            .iter()
            .any(|scene| !scene_shot_map.contains_key(scene))
        {
            return Err(domain_error(
                "every scene requires deterministic shot contracts",
            ));
        }
        let take_id = Uuid::new_v5(
            &work_version_id,
            format!(
                "{}:{}:{}",
                input.generation_step_id, input.generation_attempt_id, input.output_artifact_id
            )
            .as_bytes(),
        );
        takes.push(RequiredTake {
            take_id,
            ordinal,
            generation_step_id: input.generation_step_id,
            generation_attempt_id: input.generation_attempt_id,
            output_artifact_id: input.output_artifact_id,
            segment_key: input.segment_key,
            scene_ids: input.scene_ids,
            scene_shot_map,
        });
    }
    let digest = canonical_digest(&(work_version_id, &takes))?;
    Ok(RequiredTakeInventory {
        work_version_id,
        takes,
        digest,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaCapability {
    pub available: bool,
    pub version: Option<String>,
}

impl MediaCapability {
    pub fn available(version: &str) -> Self {
        Self {
            available: true,
            version: Some(version.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaEvidence {
    pub work_version_id: Uuid,
    pub inventory_digest: String,
    pub final_asset_id: Uuid,
    pub asset_hash: String,
    pub mime_type: String,
    pub duration_ms: u64,
    pub vision: MediaCapability,
    pub audio: MediaCapability,
    pub redacted_analysis: Value,
}

/// 只保存自管资产身份、能力版本和脱敏结果；临时访问参数不属于该类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaEvidenceSnapshot {
    pub evidence_id: Uuid,
    pub run_id: Uuid,
    pub source_step_id: Uuid,
    pub source_attempt: u32,
    pub revision_epoch: u32,
    pub work_version_id: Uuid,
    pub inventory_id: Uuid,
    pub inventory_digest: String,
    pub final_artifact_id: Uuid,
    pub asset_hash: String,
    pub mime_type: String,
    pub duration_ms: u64,
    pub vision_capability_version: String,
    pub audio_capability_version: String,
    pub redacted_analysis: Value,
    pub evidence_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaReviewInput {
    pub inventory: RequiredTakeInventorySnapshot,
    pub evidence: MediaEvidenceSnapshot,
}

/// Editor/QC 只能从完整真实媒体快照进入，不接受文本产物降级输入。
pub fn media_review_readiness(
    inventory: Option<RequiredTakeInventorySnapshot>,
    evidence: Option<MediaEvidenceSnapshot>,
) -> ProductionResult<MediaReviewInput> {
    let inventory = inventory.ok_or_else(|| evidence_blocker("required_take_inventory_missing"))?;
    inventory
        .validate()
        .map_err(|_| evidence_blocker("required_take_inventory_invalid"))?;
    let evidence = evidence.ok_or_else(|| evidence_blocker("media_evidence_missing"))?;
    if evidence.vision_capability_version.trim().is_empty() {
        return Err(ProductionError::CapabilityMismatch {
            reason: "vision capability version is missing".into(),
        });
    }
    if evidence.audio_capability_version.trim().is_empty() {
        return Err(ProductionError::CapabilityMismatch {
            reason: "audio/ASR capability version is missing".into(),
        });
    }
    evidence
        .validate()
        .map_err(|_| evidence_blocker("media_evidence_invalid"))?;
    if evidence.run_id != inventory.run_id
        || evidence.source_step_id != inventory.source_step_id
        || evidence.source_attempt != inventory.source_attempt
        || evidence.revision_epoch != inventory.revision_epoch
        || evidence.work_version_id != inventory.work_version_id
        || evidence.inventory_id != inventory.inventory_id
        || evidence.inventory_digest != inventory.inventory_digest
        || evidence.final_artifact_id != inventory.final_asset.artifact_id
        || evidence.asset_hash != inventory.final_asset.sha256
        || evidence.mime_type != inventory.final_asset.mime_type
        || evidence.duration_ms != inventory.final_asset.duration_ms
    {
        return Err(evidence_blocker("media_evidence_identity_mismatch"));
    }
    if !evidence
        .redacted_analysis
        .get("final_media")
        .is_some_and(Value::is_object)
    {
        return Err(evidence_blocker("final_media_analysis_missing"));
    }
    let analyzed_takes = evidence
        .redacted_analysis
        .get("takes")
        .and_then(Value::as_array)
        .ok_or_else(|| evidence_blocker("take_analysis_missing"))?;
    let take_ids = analyzed_takes
        .iter()
        .map(|item| {
            item.get("take_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(|| evidence_blocker("take_analysis_identity_invalid"))
        })
        .collect::<ProductionResult<BTreeSet<_>>>()?;
    let required: BTreeSet<_> = inventory.takes.iter().map(|take| take.take_id).collect();
    if take_ids.len() != analyzed_takes.len() || take_ids != required {
        return Err(evidence_blocker("take_analysis_coverage_incomplete"));
    }
    Ok(MediaReviewInput {
        inventory,
        evidence,
    })
}

impl MediaEvidenceSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        evidence_id: Uuid,
        run_id: Uuid,
        source_step_id: Uuid,
        source_attempt: u32,
        revision_epoch: u32,
        work_version_id: Uuid,
        inventory_id: Uuid,
        inventory_digest: String,
        final_asset: FinalMediaAsset,
        vision_capability_version: String,
        audio_capability_version: String,
        redacted_analysis: Value,
    ) -> ProductionResult<Self> {
        final_asset.validate()?;
        if source_attempt == 0
            || !valid_digest(&inventory_digest)
            || vision_capability_version.trim().is_empty()
            || audio_capability_version.trim().is_empty()
            || !redacted_analysis.is_object()
            || contains_forbidden_media_data(&redacted_analysis)?
        {
            return Err(domain_error(
                "media evidence snapshot or capability versions are incomplete",
            ));
        }
        let evidence_digest = canonical_digest(&(
            run_id,
            source_step_id,
            source_attempt,
            revision_epoch,
            work_version_id,
            inventory_id,
            inventory_digest.as_str(),
            &final_asset,
            vision_capability_version.as_str(),
            audio_capability_version.as_str(),
            &redacted_analysis,
        ))?;
        Ok(Self {
            evidence_id,
            run_id,
            source_step_id,
            source_attempt,
            revision_epoch,
            work_version_id,
            inventory_id,
            inventory_digest,
            final_artifact_id: final_asset.artifact_id,
            asset_hash: final_asset.sha256,
            mime_type: final_asset.mime_type,
            duration_ms: final_asset.duration_ms,
            vision_capability_version,
            audio_capability_version,
            redacted_analysis,
            evidence_digest,
        })
    }

    pub fn validate(&self) -> ProductionResult<()> {
        let rebuilt = Self::build(
            self.evidence_id,
            self.run_id,
            self.source_step_id,
            self.source_attempt,
            self.revision_epoch,
            self.work_version_id,
            self.inventory_id,
            self.inventory_digest.clone(),
            FinalMediaAsset {
                artifact_id: self.final_artifact_id,
                sha256: self.asset_hash.clone(),
                mime_type: self.mime_type.clone(),
                duration_ms: self.duration_ms,
            },
            self.vision_capability_version.clone(),
            self.audio_capability_version.clone(),
            self.redacted_analysis.clone(),
        )?;
        if rebuilt.evidence_digest != self.evidence_digest {
            return Err(domain_error("media evidence digest is not canonical"));
        }
        Ok(())
    }
}

impl MediaEvidence {
    pub fn validate(&self) -> ProductionResult<()> {
        if self.inventory_digest.len() != 64
            || self.asset_hash.len() != 64
            || self.duration_ms == 0
            || !self.mime_type.starts_with("video/")
            || !self.vision.available
            || self.vision.version.as_deref().is_none_or(str::is_empty)
            || !self.audio.available
            || self.audio.version.as_deref().is_none_or(str::is_empty)
            || !self.redacted_analysis.is_object()
        {
            return Err(domain_error(
                "media evidence or required capabilities are incomplete",
            ));
        }
        if contains_forbidden_media_data(&self.redacted_analysis)? {
            return Err(domain_error(
                "media evidence contains forbidden secret or binary data",
            ));
        }
        Ok(())
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn contains_forbidden_media_data(value: &Value) -> Result<bool, serde_json::Error> {
    let encoded = serde_json::to_string(value)?.to_ascii_lowercase();
    Ok(["api_key", "authorization", "signed_url", "base64"]
        .iter()
        .any(|forbidden| encoded.contains(forbidden)))
}

fn evidence_blocker(reason: &str) -> ProductionError {
    ProductionError::EvidenceBlocker {
        reason: reason.into(),
        details: serde_json::json!({"blocker": reason}),
    }
}

#[derive(Debug, Clone)]
pub struct ContinuityEvidence {
    pub work_version_id: Uuid,
    pub inventory_digest: String,
    pub shot_contract_id: Uuid,
}

impl ContinuityEvidence {
    pub fn new(work_version_id: Uuid, inventory_digest: String, shot_contract_id: Uuid) -> Self {
        Self {
            work_version_id,
            inventory_digest,
            shot_contract_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeEvidence {
    Approved,
    Rejected,
    NeedsRevision,
}

#[derive(Debug, Clone)]
pub struct TakeReviewEvidence {
    pub work_version_id: Uuid,
    pub inventory_digest: String,
    pub take_id: Uuid,
    pub status: TakeEvidence,
}

impl TakeReviewEvidence {
    pub fn new(
        work_version_id: Uuid,
        inventory_digest: String,
        take_id: Uuid,
        status: TakeEvidence,
    ) -> Self {
        Self {
            work_version_id,
            inventory_digest,
            take_id,
            status,
        }
    }
}

pub fn quality_coverage(
    inventory: &RequiredTakeInventory,
    evidence: &MediaEvidence,
    ledgers: &[ContinuityEvidence],
    reviews: &[TakeReviewEvidence],
) -> ProductionResult<()> {
    evidence.validate()?;
    if evidence.work_version_id != inventory.work_version_id
        || evidence.inventory_digest != inventory.digest
    {
        return Err(domain_error("stale media evidence"));
    }
    let required_shots: BTreeSet<_> = inventory.required_shot_ids().into_iter().collect();
    let current_ledgers: Vec<_> = ledgers
        .iter()
        .filter(|item| {
            item.work_version_id == inventory.work_version_id
                && item.inventory_digest == inventory.digest
        })
        .collect();
    let covered_shots: BTreeSet<_> = current_ledgers
        .iter()
        .map(|item| item.shot_contract_id)
        .collect();
    if current_ledgers.len() != covered_shots.len() || covered_shots != required_shots {
        return Err(domain_error(
            "continuity ledger does not exactly cover required shots",
        ));
    }

    let required_takes: BTreeSet<_> = inventory.takes.iter().map(|take| take.take_id).collect();
    let current_reviews: Vec<_> = reviews
        .iter()
        .filter(|item| {
            item.work_version_id == inventory.work_version_id
                && item.inventory_digest == inventory.digest
        })
        .collect();
    let covered_takes: BTreeSet<_> = current_reviews.iter().map(|item| item.take_id).collect();
    if current_reviews.len() != covered_takes.len()
        || covered_takes != required_takes
        || current_reviews
            .iter()
            .any(|review| review.status != TakeEvidence::Approved)
    {
        return Err(domain_error(
            "take reviews do not exactly approve required takes",
        ));
    }
    Ok(())
}
