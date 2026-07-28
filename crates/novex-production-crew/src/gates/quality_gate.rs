//! QualityGate：只批准当前 WorkVersion 的完整媒体质量证据集合。

use crate::durable::media::{media_review_readiness, MediaReviewInput};
use crate::error::{ProductionError, ProductionResult};
use crate::gates::gate_trait::{Gate, GateContext, GateDecision};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityReviewStatus {
    Approved,
    NeedsRevision,
    Rejected,
}

/// QualityPackage 选中的当前 ContinuityLedger 版本。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityContinuityLedger {
    pub id: Uuid,
    pub run_id: Uuid,
    pub revision_epoch: u32,
    pub work_version_id: Uuid,
    pub inventory_id: Uuid,
    pub inventory_digest: String,
    pub evidence_snapshot_id: Uuid,
    pub shot_contract_id: Uuid,
    pub version: u32,
}

/// QualityPackage 选中的当前 TakeReview 版本。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityTakeReview {
    pub id: Uuid,
    pub run_id: Uuid,
    pub revision_epoch: u32,
    pub work_version_id: Uuid,
    pub inventory_id: Uuid,
    pub inventory_digest: String,
    pub evidence_snapshot_id: Uuid,
    pub required_take_id: Uuid,
    pub applicable_shot_contract_ids: Vec<Uuid>,
    pub status: QualityReviewStatus,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityGateInput {
    pub media_review: MediaReviewInput,
    pub continuity_ledgers: Vec<QualityContinuityLedger>,
    pub take_reviews: Vec<QualityTakeReview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityGateOutcome {
    Approved,
    NeedsRevision { review_ids: Vec<Uuid> },
    Rejected { review_ids: Vec<Uuid> },
}

impl QualityGateOutcome {
    pub fn production_quality_status(&self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::NeedsRevision { .. } => "needs_revision",
            Self::Rejected { .. } => "rejected",
        }
    }
}

pub struct QualityGate;

impl QualityGate {
    /// 校验 QualityPackage 的媒体 scope 和 shot/take 精确覆盖，不筛选或猜测“最新”记录。
    pub fn evaluate(input: &QualityGateInput) -> ProductionResult<QualityGateOutcome> {
        let media = media_review_readiness(
            Some(input.media_review.inventory.clone()),
            Some(input.media_review.evidence.clone()),
        )?;
        let inventory = &media.inventory;
        let evidence_id = media.evidence.evidence_id;

        let required_shots: BTreeSet<_> = inventory
            .takes
            .iter()
            .flat_map(|take| take.scene_shot_map.values())
            .flatten()
            .copied()
            .collect();
        let required_takes: BTreeMap<_, _> = inventory
            .takes
            .iter()
            .map(|take| {
                let shots = take
                    .scene_shot_map
                    .values()
                    .flatten()
                    .copied()
                    .collect::<BTreeSet<_>>();
                (take.take_id, shots)
            })
            .collect();
        if required_shots.is_empty() || required_takes.is_empty() {
            return Err(quality_blocker("quality_inventory_empty"));
        }

        let ledger_ids: BTreeSet<_> = input
            .continuity_ledgers
            .iter()
            .map(|ledger| ledger.id)
            .collect();
        let covered_shots: BTreeSet<_> = input
            .continuity_ledgers
            .iter()
            .map(|ledger| ledger.shot_contract_id)
            .collect();
        if input.continuity_ledgers.len() != ledger_ids.len()
            || input.continuity_ledgers.len() != covered_shots.len()
            || covered_shots != required_shots
        {
            return Err(quality_blocker("continuity_coverage_not_exact"));
        }
        if input.continuity_ledgers.iter().any(|ledger| {
            ledger.run_id != inventory.run_id
                || ledger.revision_epoch != inventory.revision_epoch
                || ledger.work_version_id != inventory.work_version_id
                || ledger.inventory_id != inventory.inventory_id
                || ledger.inventory_digest != inventory.inventory_digest
                || ledger.evidence_snapshot_id != evidence_id
                || ledger.version == 0
        }) {
            return Err(quality_blocker("continuity_scope_stale"));
        }

        let review_ids: BTreeSet<_> = input.take_reviews.iter().map(|review| review.id).collect();
        let covered_takes: BTreeSet<_> = input
            .take_reviews
            .iter()
            .map(|review| review.required_take_id)
            .collect();
        if input.take_reviews.len() != review_ids.len()
            || input.take_reviews.len() != covered_takes.len()
            || covered_takes != required_takes.keys().copied().collect()
        {
            return Err(quality_blocker("take_review_coverage_not_exact"));
        }
        for review in &input.take_reviews {
            if review.run_id != inventory.run_id
                || review.revision_epoch != inventory.revision_epoch
                || review.work_version_id != inventory.work_version_id
                || review.inventory_id != inventory.inventory_id
                || review.inventory_digest != inventory.inventory_digest
                || review.evidence_snapshot_id != evidence_id
                || review.version == 0
            {
                return Err(quality_blocker("take_review_scope_stale"));
            }
            let actual: BTreeSet<_> = review
                .applicable_shot_contract_ids
                .iter()
                .copied()
                .collect();
            if actual.len() != review.applicable_shot_contract_ids.len()
                || required_takes.get(&review.required_take_id) != Some(&actual)
            {
                return Err(quality_blocker("take_review_shot_mapping_not_exact"));
            }
        }

        let rejected = input
            .take_reviews
            .iter()
            .filter(|review| review.status == QualityReviewStatus::Rejected)
            .map(|review| review.id)
            .collect::<Vec<_>>();
        if !rejected.is_empty() {
            return Ok(QualityGateOutcome::Rejected {
                review_ids: rejected,
            });
        }
        let needs_revision = input
            .take_reviews
            .iter()
            .filter(|review| review.status == QualityReviewStatus::NeedsRevision)
            .map(|review| review.id)
            .collect::<Vec<_>>();
        if !needs_revision.is_empty() {
            return Ok(QualityGateOutcome::NeedsRevision {
                review_ids: needs_revision,
            });
        }
        Ok(QualityGateOutcome::Approved)
    }
}

#[async_trait]
impl Gate for QualityGate {
    fn name(&self) -> &str {
        "quality_gate"
    }

    async fn check(&self, context: &GateContext) -> ProductionResult<GateDecision> {
        let value = context
            .project_metadata
            .get("quality_gate_input")
            .cloned()
            .ok_or_else(|| quality_blocker("quality_gate_input_missing"))?;
        let input: QualityGateInput = serde_json::from_value(value)
            .map_err(|_| quality_blocker("quality_gate_input_invalid"))?;
        match Self::evaluate(&input)? {
            QualityGateOutcome::Approved => Ok(GateDecision::Pass),
            QualityGateOutcome::NeedsRevision { review_ids } => Ok(GateDecision::WaitApproval {
                artifact_id: review_ids[0],
            }),
            QualityGateOutcome::Rejected { review_ids } => Ok(GateDecision::Reject {
                reason: format!(
                    "当前 WorkVersion 的 TakeReview 不通过：{}",
                    review_ids
                        .iter()
                        .map(Uuid::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            }),
        }
    }
}

fn quality_blocker(reason: &str) -> ProductionError {
    ProductionError::EvidenceBlocker {
        reason: reason.into(),
        details: json!({"blocker": reason}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable::media::{
        media_review_readiness, ComposeInput, FinalMediaAsset, MediaEvidenceSnapshot,
        RequiredTakeInventorySnapshot,
    };
    use std::collections::HashMap;

    fn digest(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn input() -> QualityGateInput {
        let run_id = Uuid::new_v4();
        let source_step_id = Uuid::new_v4();
        let work_id = Uuid::new_v4();
        let work_version_id = Uuid::new_v4();
        let scene_a = Uuid::new_v4();
        let scene_b = Uuid::new_v4();
        let shot_a = Uuid::new_v4();
        let shot_b = Uuid::new_v4();
        let final_asset = FinalMediaAsset {
            artifact_id: Uuid::new_v4(),
            sha256: digest('a'),
            mime_type: "video/mp4".into(),
            duration_ms: 12_000,
        };
        let inventory = RequiredTakeInventorySnapshot::build(
            Uuid::new_v4(),
            run_id,
            source_step_id,
            1,
            0,
            work_id,
            work_version_id,
            Uuid::new_v4(),
            final_asset.clone(),
            digest('b'),
            vec![
                ComposeInput {
                    generation_step_id: Uuid::new_v4(),
                    generation_attempt_id: Uuid::new_v4(),
                    output_artifact_id: Uuid::new_v4(),
                    segment_key: "segment-a".into(),
                    scene_ids: vec![scene_a],
                    shot_contracts: vec![(scene_a, vec![shot_a])],
                    consumed_by_final_compose: true,
                    generation_succeeded: true,
                },
                ComposeInput {
                    generation_step_id: Uuid::new_v4(),
                    generation_attempt_id: Uuid::new_v4(),
                    output_artifact_id: Uuid::new_v4(),
                    segment_key: "segment-b".into(),
                    scene_ids: vec![scene_b],
                    shot_contracts: vec![(scene_b, vec![shot_b])],
                    consumed_by_final_compose: true,
                    generation_succeeded: true,
                },
            ],
        )
        .unwrap();
        let evidence = MediaEvidenceSnapshot::build(
            Uuid::new_v4(),
            run_id,
            source_step_id,
            1,
            0,
            work_version_id,
            inventory.inventory_id,
            inventory.inventory_digest.clone(),
            final_asset,
            "vision@1".into(),
            "asr@1".into(),
            json!({
                "final_media": {"result": "analyzed"},
                "takes": inventory.takes.iter().map(|take| json!({"take_id": take.take_id})).collect::<Vec<_>>()
            }),
        )
        .unwrap();
        let media_review =
            media_review_readiness(Some(inventory.clone()), Some(evidence.clone())).unwrap();
        let continuity_ledgers = [shot_a, shot_b]
            .into_iter()
            .map(|shot_contract_id| QualityContinuityLedger {
                id: Uuid::new_v4(),
                run_id,
                revision_epoch: 0,
                work_version_id,
                inventory_id: inventory.inventory_id,
                inventory_digest: inventory.inventory_digest.clone(),
                evidence_snapshot_id: evidence.evidence_id,
                shot_contract_id,
                version: 1,
            })
            .collect();
        let take_reviews = inventory
            .takes
            .iter()
            .map(|take| QualityTakeReview {
                id: Uuid::new_v4(),
                run_id,
                revision_epoch: 0,
                work_version_id,
                inventory_id: inventory.inventory_id,
                inventory_digest: inventory.inventory_digest.clone(),
                evidence_snapshot_id: evidence.evidence_id,
                required_take_id: take.take_id,
                applicable_shot_contract_ids: take
                    .scene_shot_map
                    .values()
                    .flatten()
                    .copied()
                    .collect(),
                status: QualityReviewStatus::Approved,
                version: 1,
            })
            .collect();
        QualityGateInput {
            media_review,
            continuity_ledgers,
            take_reviews,
        }
    }

    #[test]
    fn exact_current_work_version_coverage_passes() {
        let outcome = QualityGate::evaluate(&input()).unwrap();
        assert_eq!(outcome, QualityGateOutcome::Approved);
        assert_eq!(outcome.production_quality_status(), "approved");
    }

    #[test]
    fn empty_partial_duplicate_and_stale_quality_sets_fail_closed() {
        let base = input();

        let mut empty_inventory = base.clone();
        empty_inventory.media_review.inventory.takes.clear();
        assert_eq!(
            QualityGate::evaluate(&empty_inventory).unwrap_err().code(),
            "evidence_blocker"
        );

        let mut partial_ledgers = base.clone();
        partial_ledgers.continuity_ledgers.pop();
        assert_eq!(
            QualityGate::evaluate(&partial_ledgers).unwrap_err().code(),
            "evidence_blocker"
        );

        let mut duplicate_ledgers = base.clone();
        duplicate_ledgers
            .continuity_ledgers
            .push(duplicate_ledgers.continuity_ledgers[0].clone());
        assert_eq!(
            QualityGate::evaluate(&duplicate_ledgers)
                .unwrap_err()
                .code(),
            "evidence_blocker"
        );

        let mut stale_ledger = base.clone();
        stale_ledger.continuity_ledgers[0].work_version_id = Uuid::new_v4();
        assert_eq!(
            QualityGate::evaluate(&stale_ledger).unwrap_err().code(),
            "evidence_blocker"
        );

        let mut empty_reviews = base.clone();
        empty_reviews.take_reviews.clear();
        assert_eq!(
            QualityGate::evaluate(&empty_reviews).unwrap_err().code(),
            "evidence_blocker"
        );

        let mut partial_reviews = base.clone();
        partial_reviews.take_reviews.pop();
        assert_eq!(
            QualityGate::evaluate(&partial_reviews).unwrap_err().code(),
            "evidence_blocker"
        );

        let mut duplicate_reviews = base.clone();
        duplicate_reviews
            .take_reviews
            .push(duplicate_reviews.take_reviews[0].clone());
        assert_eq!(
            QualityGate::evaluate(&duplicate_reviews)
                .unwrap_err()
                .code(),
            "evidence_blocker"
        );

        let mut stale_review = base;
        stale_review.take_reviews[0].inventory_digest = digest('f');
        assert_eq!(
            QualityGate::evaluate(&stale_review).unwrap_err().code(),
            "evidence_blocker"
        );
    }

    #[tokio::test]
    async fn gate_maps_only_complete_current_reviews_and_never_invents_an_artifact_id() {
        let gate = QualityGate;
        let mut current = input();
        let needs_revision_id = current.take_reviews[0].id;
        current.take_reviews[0].status = QualityReviewStatus::NeedsRevision;
        let ctx = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts: HashMap::new(),
            project_metadata: json!({"quality_gate_input": current}),
        };
        assert!(matches!(
            gate.check(&ctx).await.unwrap(),
            GateDecision::WaitApproval { artifact_id } if artifact_id == needs_revision_id
        ));
        assert_eq!(
            QualityGate::evaluate(
                &serde_json::from_value(ctx.project_metadata["quality_gate_input"].clone())
                    .unwrap()
            )
            .unwrap()
            .production_quality_status(),
            "needs_revision"
        );

        let missing = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts: HashMap::new(),
            project_metadata: json!({}),
        };
        assert_eq!(
            gate.check(&missing).await.unwrap_err().code(),
            "evidence_blocker"
        );
    }
}
