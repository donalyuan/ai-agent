use super::{canonical_digest, domain_error};
use crate::ProductionResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageType {
    Brief,
    Script,
    Production,
    Quality,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub run_id: Uuid,
    pub artifact_type: String,
    pub artifact_id: Uuid,
    pub version: u32,
    pub content_digest: String,
    pub source_step_id: Uuid,
    pub source_attempt: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactPackageSnapshot {
    pub id: Uuid,
    pub package_type: PackageType,
    pub run_id: Uuid,
    pub source_step_id: Uuid,
    pub source_attempt: u32,
    pub revision_epoch: u32,
    pub package_version: u32,
    pub items: Vec<ArtifactRef>,
    pub metadata: Value,
    pub package_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionSceneRef {
    pub scene_id: Uuid,
    pub scene_version: String,
    pub scene_digest: String,
    pub sequence: u32,
    pub duration_sec: u32,
    pub character_bible_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionCharacterRef {
    pub character_bible_id: Uuid,
    pub character_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionShotRef {
    pub artifact_id: Uuid,
    pub shot_id: String,
    pub sequence: u32,
    pub scene_id: Uuid,
    pub duration_sec: u32,
    pub character_bible_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionPerformanceRef {
    pub artifact_id: Uuid,
    pub script_id: Uuid,
    pub character_bible_id: Uuid,
    pub character_id: String,
    pub scene_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionSoundRef {
    pub artifact_id: Uuid,
    pub script_id: Uuid,
    pub scene_ids: Vec<Uuid>,
}

/// ProductionPackage 的领域闭合证据；所有 ID 均来自正式 Script 或精确过程产物。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionPackageMetadata {
    pub script_id: Uuid,
    pub script_version: String,
    pub script_digest: String,
    pub scenes: Vec<ProductionSceneRef>,
    pub characters: Vec<ProductionCharacterRef>,
    pub shots: Vec<ProductionShotRef>,
    pub performance_briefs: Vec<ProductionPerformanceRef>,
    pub sound_plan: ProductionSoundRef,
    pub suggestion_resolutions: Vec<Value>,
}

#[derive(Serialize)]
struct PackageDigestInput<'a> {
    package_type: PackageType,
    run_id: Uuid,
    source_step_id: Uuid,
    source_attempt: u32,
    revision_epoch: u32,
    package_version: u32,
    items: &'a [ArtifactRef],
    metadata: &'a Value,
}

impl ArtifactPackageSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        package_type: PackageType,
        run_id: Uuid,
        source_step_id: Uuid,
        source_attempt: u32,
        revision_epoch: u32,
        package_version: u32,
        mut items: Vec<ArtifactRef>,
        metadata: Value,
    ) -> ProductionResult<Self> {
        if items.is_empty() || source_attempt == 0 || package_version == 0 || !metadata.is_object()
        {
            return Err(domain_error(
                "package identity, items, and metadata must be complete",
            ));
        }
        if items.iter().any(|item| {
            item.run_id != run_id
                || item.version == 0
                || item.source_attempt == 0
                || item.content_digest.len() != 64
        }) {
            return Err(domain_error(
                "package contains cross-run or invalid artifact references",
            ));
        }
        items.sort_by(|left, right| {
            (&left.artifact_type, left.artifact_id, left.version).cmp(&(
                &right.artifact_type,
                right.artifact_id,
                right.version,
            ))
        });
        if items.windows(2).any(|pair| {
            pair[0].artifact_type == pair[1].artifact_type
                && pair[0].artifact_id == pair[1].artifact_id
                && pair[0].version == pair[1].version
        }) {
            return Err(domain_error("package contains duplicate artifact identity"));
        }
        validate_package_schema(package_type, &items, &metadata)?;
        let package_digest = canonical_digest(&PackageDigestInput {
            package_type,
            run_id,
            source_step_id,
            source_attempt,
            revision_epoch,
            package_version,
            items: &items,
            metadata: &metadata,
        })?;
        Ok(Self {
            id: Uuid::new_v4(),
            package_type,
            run_id,
            source_step_id,
            source_attempt,
            revision_epoch,
            package_version,
            items,
            metadata,
            package_digest,
        })
    }
}

fn validate_package_schema(
    package_type: PackageType,
    items: &[ArtifactRef],
    metadata: &Value,
) -> ProductionResult<()> {
    let counts = items.iter().fold(BTreeMap::new(), |mut counts, item| {
        *counts.entry(item.artifact_type.as_str()).or_insert(0usize) += 1;
        counts
    });
    let exact = |artifact_type: &str, expected: usize| {
        counts.get(artifact_type).copied().unwrap_or(0) == expected
    };
    let at_least_one = |artifact_type: &str| counts.get(artifact_type).copied().unwrap_or(0) >= 1;
    let allowed: &[&str] = match package_type {
        PackageType::Brief => &["creative_brief"],
        PackageType::Script => &["story_bible", "character_bible", "script_draft"],
        PackageType::Production => &[
            "directorial_treatment",
            "shot_contract",
            "performance_brief",
            "sound_plan",
        ],
        PackageType::Quality => &[
            "required_take_inventory",
            "media_evidence",
            "continuity_ledger",
            "take_review",
        ],
    };
    if counts
        .keys()
        .any(|artifact_type| !allowed.contains(artifact_type))
    {
        return Err(domain_error(
            "package contains an artifact type outside its schema",
        ));
    }
    let cardinality_valid = match package_type {
        PackageType::Brief => exact("creative_brief", 1),
        PackageType::Script => {
            exact("story_bible", 1) && at_least_one("character_bible") && exact("script_draft", 1)
        }
        PackageType::Production => {
            exact("directorial_treatment", 1)
                && at_least_one("shot_contract")
                && at_least_one("performance_brief")
                && exact("sound_plan", 1)
        }
        PackageType::Quality => {
            exact("required_take_inventory", 1)
                && exact("media_evidence", 1)
                && at_least_one("continuity_ledger")
                && at_least_one("take_review")
        }
    };
    if !cardinality_valid {
        return Err(domain_error("package artifact cardinality is incomplete"));
    }
    if package_type == PackageType::Quality {
        let work_version_id = metadata
            .get("work_version_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        let inventory_digest = metadata.get("inventory_digest").and_then(Value::as_str);
        if work_version_id.is_none()
            || inventory_digest.is_none_or(|value| {
                value.len() != 64
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            })
        {
            return Err(domain_error(
                "quality package requires work_version_id and inventory_digest",
            ));
        }
    }
    if package_type == PackageType::Production {
        let metadata: ProductionPackageMetadata = serde_json::from_value(metadata.clone())
            .map_err(|error| {
                domain_error(format!(
                    "production package requires typed metadata: {error}"
                ))
            })?;
        validate_production_package(items, &metadata)?;
    }
    Ok(())
}

fn validate_production_package(
    items: &[ArtifactRef],
    metadata: &ProductionPackageMetadata,
) -> ProductionResult<()> {
    if metadata.script_version.trim().is_empty() || !valid_digest(&metadata.script_digest) {
        return Err(domain_error(
            "production package Script version and digest are invalid",
        ));
    }
    if metadata.scenes.is_empty() {
        return Err(domain_error("production package has no formal Scene"));
    }

    let mut scene_ids = BTreeSet::new();
    let mut scene_characters = BTreeMap::<Uuid, BTreeSet<Uuid>>::new();
    let mut referenced_characters = BTreeSet::new();
    for (index, scene) in metadata.scenes.iter().enumerate() {
        if scene.sequence as usize != index + 1 {
            return Err(domain_error(
                "production package Scene sequence must be continuous from 1",
            ));
        }
        if !(1..=30).contains(&scene.duration_sec)
            || scene.scene_version.trim().is_empty()
            || !valid_digest(&scene.scene_digest)
        {
            return Err(domain_error(
                "production package Scene duration/version/digest is invalid",
            ));
        }
        if !scene_ids.insert(scene.scene_id) {
            return Err(domain_error(
                "production package contains duplicate Scene identity",
            ));
        }
        let characters = unique_uuid_set(
            &scene.character_bible_ids,
            "production package Scene contains duplicate Character reference",
        )?;
        referenced_characters.extend(characters.iter().copied());
        scene_characters.insert(scene.scene_id, characters);
    }

    let mut character_ids = BTreeSet::new();
    let mut character_keys = BTreeMap::new();
    for character in &metadata.characters {
        if character.character_id.trim().is_empty()
            || !character_ids.insert(character.character_bible_id)
            || character_keys
                .insert(
                    character.character_id.as_str(),
                    character.character_bible_id,
                )
                .is_some()
        {
            return Err(domain_error(
                "production package contains duplicate or blank Character identity",
            ));
        }
    }
    if !referenced_characters.is_subset(&character_ids) {
        return Err(domain_error(
            "production package Scene references an unknown Character",
        ));
    }

    let mut shot_artifacts = BTreeSet::new();
    let mut shot_keys = BTreeSet::new();
    let mut shot_count_by_scene = BTreeMap::<Uuid, usize>::new();
    let mut shot_duration_by_scene = BTreeMap::<Uuid, u32>::new();
    for (index, shot) in metadata.shots.iter().enumerate() {
        if shot.sequence as usize != index + 1 {
            return Err(domain_error(
                "production package Shot sequence must be continuous from 1",
            ));
        }
        if shot.shot_id.trim().is_empty()
            || !shot_artifacts.insert(shot.artifact_id)
            || !shot_keys.insert(shot.shot_id.as_str())
        {
            return Err(domain_error(
                "production package contains duplicate Shot identity",
            ));
        }
        if !(1..=30).contains(&shot.duration_sec) {
            return Err(domain_error(
                "production package Shot duration is outside the supported range",
            ));
        }
        let allowed_characters = scene_characters.get(&shot.scene_id).ok_or_else(|| {
            domain_error("production package Shot contains a cross-Script Scene reference")
        })?;
        let shot_characters = unique_uuid_set(
            &shot.character_bible_ids,
            "production package Shot contains duplicate Character reference",
        )?;
        if !shot_characters.is_subset(allowed_characters) {
            return Err(domain_error(
                "production package Shot references a Character outside its Scene",
            ));
        }
        *shot_count_by_scene.entry(shot.scene_id).or_default() += 1;
        *shot_duration_by_scene.entry(shot.scene_id).or_default() += shot.duration_sec;
    }
    for scene in &metadata.scenes {
        if shot_count_by_scene
            .get(&scene.scene_id)
            .copied()
            .unwrap_or(0)
            == 0
        {
            return Err(domain_error("production package Scene has no ShotContract"));
        }
        if shot_duration_by_scene
            .get(&scene.scene_id)
            .copied()
            .unwrap_or(0)
            != scene.duration_sec
        {
            return Err(domain_error(
                "production package Shot duration does not close its Scene duration",
            ));
        }
    }

    let mut performance_artifacts = BTreeSet::new();
    let mut performance_characters = BTreeSet::new();
    for brief in &metadata.performance_briefs {
        if brief.script_id != metadata.script_id {
            return Err(domain_error(
                "production package PerformanceBrief contains a cross-Script reference",
            ));
        }
        if !performance_artifacts.insert(brief.artifact_id)
            || !performance_characters.insert(brief.character_bible_id)
        {
            return Err(domain_error(
                "production package contains duplicate PerformanceBrief identity",
            ));
        }
        let expected_character_id = metadata
            .characters
            .iter()
            .find(|character| character.character_bible_id == brief.character_bible_id)
            .map(|character| character.character_id.as_str())
            .ok_or_else(|| {
                domain_error("production package PerformanceBrief references unknown Character")
            })?;
        if brief.character_id != expected_character_id {
            return Err(domain_error(
                "production package PerformanceBrief Character identity is inconsistent",
            ));
        }
        let expected_scenes = metadata
            .scenes
            .iter()
            .filter(|scene| {
                scene
                    .character_bible_ids
                    .contains(&brief.character_bible_id)
            })
            .map(|scene| scene.scene_id)
            .collect::<Vec<_>>();
        if brief.scene_ids != expected_scenes {
            return Err(domain_error(
                "production package PerformanceBrief Scene set is not closed",
            ));
        }
    }
    if performance_characters != referenced_characters {
        return Err(domain_error(
            "production package referenced Character has no PerformanceBrief",
        ));
    }

    let expected_scene_order = metadata
        .scenes
        .iter()
        .map(|scene| scene.scene_id)
        .collect::<Vec<_>>();
    if metadata.sound_plan.script_id != metadata.script_id {
        return Err(domain_error(
            "production package SoundPlan contains a cross-Script reference",
        ));
    }
    if metadata.sound_plan.scene_ids != expected_scene_order {
        return Err(domain_error(
            "production package SoundPlan Scene set is not closed",
        ));
    }

    validate_metadata_artifact_set(items, "shot_contract", &shot_artifacts)?;
    validate_metadata_artifact_set(items, "performance_brief", &performance_artifacts)?;
    validate_metadata_artifact_set(
        items,
        "sound_plan",
        &BTreeSet::from([metadata.sound_plan.artifact_id]),
    )?;
    Ok(())
}

fn validate_metadata_artifact_set(
    items: &[ArtifactRef],
    artifact_type: &str,
    expected: &BTreeSet<Uuid>,
) -> ProductionResult<()> {
    let actual = items
        .iter()
        .filter(|item| item.artifact_type == artifact_type)
        .map(|item| item.artifact_id)
        .collect::<BTreeSet<_>>();
    if &actual != expected {
        return Err(domain_error(format!(
            "production package {artifact_type} metadata does not match package items"
        )));
    }
    Ok(())
}

fn unique_uuid_set(values: &[Uuid], duplicate_error: &str) -> ProductionResult<BTreeSet<Uuid>> {
    let result = values.iter().copied().collect::<BTreeSet<_>>();
    if result.len() != values.len() {
        return Err(domain_error(duplicate_error));
    }
    Ok(result)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    Approve,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecisionRecord {
    pub id: Uuid,
    pub package_id: Uuid,
    pub package_digest: String,
    pub decision: GateDecision,
    pub actor_id: String,
    pub reason: Option<String>,
    pub affected_owners: Vec<String>,
}

#[derive(Default)]
pub struct GateDecisionBook {
    records: BTreeMap<Uuid, GateDecisionRecord>,
}

impl GateDecisionBook {
    pub fn decide(
        &mut self,
        package: &ArtifactPackageSnapshot,
        submitted_digest: &str,
        decision: GateDecision,
        actor_id: &str,
        reason: Option<String>,
        mut affected_owners: Vec<String>,
    ) -> ProductionResult<GateDecisionRecord> {
        if submitted_digest != package.package_digest {
            return Err(domain_error("stale_package"));
        }
        if let Some(existing) = self.records.get(&package.id) {
            if existing.decision == decision {
                return Ok(existing.clone());
            }
            return Err(domain_error("gate decision conflict"));
        }
        if actor_id.trim().is_empty() {
            return Err(domain_error("stable actor is required"));
        }
        if decision == GateDecision::Reject
            && (reason
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
                || affected_owners.is_empty())
        {
            return Err(domain_error("reject requires reason and affected owners"));
        }
        affected_owners.sort();
        affected_owners.dedup();
        let record = GateDecisionRecord {
            id: Uuid::new_v4(),
            package_id: package.id,
            package_digest: package.package_digest.clone(),
            decision,
            actor_id: actor_id.into(),
            reason,
            affected_owners,
        };
        self.records.insert(package.id, record.clone());
        Ok(record)
    }
}
