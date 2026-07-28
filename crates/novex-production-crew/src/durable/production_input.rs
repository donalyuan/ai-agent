//! 已批准 ProductionPackage 到画面/作品 Application Port 的强类型输入。

use super::{
    canonical_digest, domain_error,
    package::{
        ArtifactPackageSnapshot, ArtifactRef, PackageType, ProductionCharacterRef,
        ProductionPackageMetadata, ProductionPerformanceRef, ProductionSceneRef, ProductionShotRef,
        ProductionSoundRef,
    },
};
use crate::{
    state::artifacts::output_contract::{
        DirectorialTreatmentOutput, PerformanceBriefOutput, ShotContractOutput, SoundPlanOutput,
    },
    ProductionResult,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormalScriptInput {
    pub script_id: Uuid,
    pub script_version: String,
    pub script_digest: String,
    pub title: String,
    pub hook: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormalSceneInput {
    pub scene_id: Uuid,
    pub scene_version: String,
    pub scene_digest: String,
    pub sequence: u32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: u32,
    pub character_bible_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionedProductionArtifact<T> {
    pub artifact_id: Uuid,
    pub artifact_version: u32,
    pub content_digest: String,
    pub source_step_id: Uuid,
    pub source_attempt: u32,
    pub content: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedSuggestionResolution {
    pub suggestion_id: Uuid,
    pub owner_role: String,
    pub artifact_id: Uuid,
    pub artifact_version: u32,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionPackageContent {
    pub script: FormalScriptInput,
    pub scenes: Vec<FormalSceneInput>,
    pub directorial_treatment: VersionedProductionArtifact<DirectorialTreatmentOutput>,
    pub shot_contracts: Vec<VersionedProductionArtifact<ShotContractOutput>>,
    pub performance_briefs: Vec<VersionedProductionArtifact<PerformanceBriefOutput>>,
    pub sound_plan: VersionedProductionArtifact<SoundPlanOutput>,
    pub applied_suggestions: Vec<AppliedSuggestionResolution>,
}

/// 端口只能接收由不可变 package 和精确过程产物共同构造的输入。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionPackageInput {
    pub run_id: Uuid,
    pub revision_epoch: u32,
    pub package_id: Uuid,
    pub package_version: u32,
    pub package_digest: String,
    pub package_source_step_id: Uuid,
    pub package_source_attempt: u32,
    pub script: FormalScriptInput,
    pub scenes: Vec<FormalSceneInput>,
    pub directorial_treatment: VersionedProductionArtifact<DirectorialTreatmentOutput>,
    pub shot_contracts: Vec<VersionedProductionArtifact<ShotContractOutput>>,
    pub performance_briefs: Vec<VersionedProductionArtifact<PerformanceBriefOutput>>,
    pub sound_plan: VersionedProductionArtifact<SoundPlanOutput>,
    pub applied_suggestions: Vec<AppliedSuggestionResolution>,
    pub input_digest: String,
}

impl ProductionPackageInput {
    pub fn from_approved_package(
        package: &ArtifactPackageSnapshot,
        mut content: ProductionPackageContent,
    ) -> ProductionResult<Self> {
        if package.package_type != PackageType::Production {
            return Err(domain_error(
                "typed production input requires ProductionPackage",
            ));
        }
        let rebuilt = ArtifactPackageSnapshot::build(
            package.package_type,
            package.run_id,
            package.source_step_id,
            package.source_attempt,
            package.revision_epoch,
            package.package_version,
            package.items.clone(),
            package.metadata.clone(),
        )?;
        if rebuilt.package_digest != package.package_digest {
            return Err(domain_error("ProductionPackage digest is not canonical"));
        }
        let metadata: ProductionPackageMetadata = serde_json::from_value(package.metadata.clone())
            .map_err(|error| {
                domain_error(format!("ProductionPackage metadata is invalid: {error}"))
            })?;
        validate_script_and_scenes(&metadata, &content.script, &content.scenes)?;
        validate_versioned_artifact(
            package,
            "directorial_treatment",
            &content.directorial_treatment,
        )?;
        for shot in &content.shot_contracts {
            validate_versioned_artifact(package, "shot_contract", shot)?;
        }
        for brief in &content.performance_briefs {
            validate_versioned_artifact(package, "performance_brief", brief)?;
        }
        validate_versioned_artifact(package, "sound_plan", &content.sound_plan)?;
        validate_artifact_cardinality(package, &content)?;
        validate_process_content(&metadata, &content)?;

        let expected_suggestions = metadata
            .suggestion_resolutions
            .iter()
            .cloned()
            .map(serde_json::from_value::<AppliedSuggestionResolution>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                domain_error(format!(
                    "ProductionPackage suggestion resolution is invalid: {error}"
                ))
            })?;
        content
            .applied_suggestions
            .sort_by_key(|item| (item.suggestion_id, item.artifact_id));
        let mut expected_suggestions = expected_suggestions;
        expected_suggestions.sort_by_key(|item| (item.suggestion_id, item.artifact_id));
        if content.applied_suggestions != expected_suggestions {
            return Err(domain_error(
                "typed production input suggestion resolutions differ from its package",
            ));
        }
        validate_suggestions(package, &content.applied_suggestions)?;

        let input_digest = input_digest(
            package.id,
            &package.package_digest,
            &content.script,
            &content.scenes,
            &content.directorial_treatment,
            &content.shot_contracts,
            &content.performance_briefs,
            &content.sound_plan,
            &content.applied_suggestions,
        )?;
        Ok(Self {
            run_id: package.run_id,
            revision_epoch: package.revision_epoch,
            package_id: package.id,
            package_version: package.package_version,
            package_digest: package.package_digest.clone(),
            package_source_step_id: package.source_step_id,
            package_source_attempt: package.source_attempt,
            script: content.script,
            scenes: content.scenes,
            directorial_treatment: content.directorial_treatment,
            shot_contracts: content.shot_contracts,
            performance_briefs: content.performance_briefs,
            sound_plan: content.sound_plan,
            applied_suggestions: content.applied_suggestions,
            input_digest,
        })
    }

    pub fn into_content(self) -> ProductionPackageContent {
        ProductionPackageContent {
            script: self.script,
            scenes: self.scenes,
            directorial_treatment: self.directorial_treatment,
            shot_contracts: self.shot_contracts,
            performance_briefs: self.performance_briefs,
            sound_plan: self.sound_plan,
            applied_suggestions: self.applied_suggestions,
        }
    }

    pub fn package_snapshot(&self) -> ProductionResult<ArtifactPackageSnapshot> {
        let mut items = Vec::new();
        items.push(artifact_ref(
            self.run_id,
            "directorial_treatment",
            &self.directorial_treatment,
        ));
        items.extend(
            self.shot_contracts
                .iter()
                .map(|item| artifact_ref(self.run_id, "shot_contract", item)),
        );
        items.extend(
            self.performance_briefs
                .iter()
                .map(|item| artifact_ref(self.run_id, "performance_brief", item)),
        );
        items.push(artifact_ref(self.run_id, "sound_plan", &self.sound_plan));
        let metadata = metadata_from_input(self)?;
        let mut package = ArtifactPackageSnapshot::build(
            PackageType::Production,
            self.run_id,
            self.package_source_step_id,
            self.package_source_attempt,
            self.revision_epoch,
            self.package_version,
            items,
            serde_json::to_value(metadata)?,
        )?;
        package.id = self.package_id;
        if package.package_digest != self.package_digest {
            return Err(domain_error(
                "typed production input cannot reconstruct its package digest",
            ));
        }
        Ok(package)
    }
}

fn validate_script_and_scenes(
    metadata: &ProductionPackageMetadata,
    script: &FormalScriptInput,
    scenes: &[FormalSceneInput],
) -> ProductionResult<()> {
    if script.script_id != metadata.script_id
        || script.script_version != metadata.script_version
        || script.script_digest != metadata.script_digest
        || script.title.trim().is_empty()
        || script.hook.trim().is_empty()
        || scenes.len() != metadata.scenes.len()
    {
        return Err(domain_error(
            "typed Script/Scene input differs from ProductionPackage metadata",
        ));
    }
    for (scene, reference) in scenes.iter().zip(&metadata.scenes) {
        if scene.scene_id != reference.scene_id
            || scene.scene_version != reference.scene_version
            || scene.scene_digest != reference.scene_digest
            || scene.sequence != reference.sequence
            || scene.duration_sec != reference.duration_sec
            || scene.character_bible_ids != reference.character_bible_ids
            || scene.narration.trim().is_empty()
            || scene.visual_description.trim().is_empty()
            || scene.emotion.trim().is_empty()
        {
            return Err(domain_error(
                "typed formal Scene content differs from ProductionPackage metadata",
            ));
        }
    }
    Ok(())
}

fn validate_versioned_artifact<T>(
    package: &ArtifactPackageSnapshot,
    artifact_type: &str,
    artifact: &VersionedProductionArtifact<T>,
) -> ProductionResult<()> {
    let exact = package.items.iter().any(|item| {
        item.artifact_type == artifact_type
            && item.artifact_id == artifact.artifact_id
            && item.version == artifact.artifact_version
            && item.content_digest == artifact.content_digest
            && item.source_step_id == artifact.source_step_id
            && item.source_attempt == artifact.source_attempt
    });
    if !exact {
        return Err(domain_error(format!(
            "typed {artifact_type} content has no exact package provenance"
        )));
    }
    Ok(())
}

fn validate_artifact_cardinality(
    package: &ArtifactPackageSnapshot,
    content: &ProductionPackageContent,
) -> ProductionResult<()> {
    let exact = |artifact_type: &str, actual: usize| {
        package
            .items
            .iter()
            .filter(|item| item.artifact_type == artifact_type)
            .count()
            == actual
    };
    if !exact("directorial_treatment", 1)
        || !exact("shot_contract", content.shot_contracts.len())
        || !exact("performance_brief", content.performance_briefs.len())
        || !exact("sound_plan", 1)
    {
        return Err(domain_error(
            "typed production content does not cover every package artifact",
        ));
    }
    Ok(())
}

fn validate_process_content(
    metadata: &ProductionPackageMetadata,
    content: &ProductionPackageContent,
) -> ProductionResult<()> {
    let character_map = metadata
        .characters
        .iter()
        .map(|item| (item.character_id.as_str(), item.character_bible_id))
        .collect::<BTreeMap<_, _>>();
    let shots = content
        .shot_contracts
        .iter()
        .map(|item| {
            Ok(ProductionShotRef {
                artifact_id: item.artifact_id,
                shot_id: item.content.shot_id.clone(),
                sequence: item.content.sequence,
                scene_id: item.content.scene_id,
                duration_sec: item.content.duration_sec,
                character_bible_ids: item
                    .content
                    .character_ids
                    .iter()
                    .map(|id| {
                        character_map.get(id.as_str()).copied().ok_or_else(|| {
                            domain_error("typed ShotContract references an unknown Character")
                        })
                    })
                    .collect::<ProductionResult<Vec<_>>>()?,
            })
        })
        .collect::<ProductionResult<Vec<_>>>()?;
    if shots != metadata.shots {
        return Err(domain_error(
            "typed ShotContract content differs from ProductionPackage metadata",
        ));
    }
    let performances = content
        .performance_briefs
        .iter()
        .map(|item| ProductionPerformanceRef {
            artifact_id: item.artifact_id,
            script_id: item.content.script_id,
            character_bible_id: item.content.character_bible_id,
            character_id: item.content.character_id.clone(),
            scene_ids: item
                .content
                .emotional_arc
                .iter()
                .map(|scene| scene.scene_id)
                .collect(),
        })
        .collect::<Vec<_>>();
    if performances != metadata.performance_briefs {
        return Err(domain_error(
            "typed PerformanceBrief content differs from ProductionPackage metadata",
        ));
    }
    let sound = ProductionSoundRef {
        artifact_id: content.sound_plan.artifact_id,
        script_id: content.sound_plan.content.script_id,
        scene_ids: content
            .sound_plan
            .content
            .scene_sound_notes
            .iter()
            .map(|scene| scene.scene_id)
            .collect(),
    };
    if sound != metadata.sound_plan {
        return Err(domain_error(
            "typed SoundPlan content differs from ProductionPackage metadata",
        ));
    }
    Ok(())
}

fn validate_suggestions(
    package: &ArtifactPackageSnapshot,
    suggestions: &[AppliedSuggestionResolution],
) -> ProductionResult<()> {
    let mut ids = BTreeSet::new();
    for suggestion in suggestions {
        if !ids.insert(suggestion.suggestion_id)
            || suggestion.owner_role.trim().is_empty()
            || suggestion.artifact_version == 0
            || !valid_digest(&suggestion.content_digest)
            || !package.items.iter().any(|item| {
                item.artifact_id == suggestion.artifact_id
                    && item.version == suggestion.artifact_version
                    && item.content_digest == suggestion.content_digest
            })
        {
            return Err(domain_error(
                "applied suggestion does not reference a unique exact package artifact",
            ));
        }
    }
    Ok(())
}

fn input_digest(
    package_id: Uuid,
    package_digest: &str,
    script: &FormalScriptInput,
    scenes: &[FormalSceneInput],
    directorial_treatment: &VersionedProductionArtifact<DirectorialTreatmentOutput>,
    shot_contracts: &[VersionedProductionArtifact<ShotContractOutput>],
    performance_briefs: &[VersionedProductionArtifact<PerformanceBriefOutput>],
    sound_plan: &VersionedProductionArtifact<SoundPlanOutput>,
    applied_suggestions: &[AppliedSuggestionResolution],
) -> ProductionResult<String> {
    Ok(canonical_digest(&json!({
        "package_id": package_id,
        "package_digest": package_digest,
        "script": script,
        "scenes": scenes,
        "directorial_treatment": directorial_treatment,
        "shot_contracts": shot_contracts,
        "performance_briefs": performance_briefs,
        "sound_plan": sound_plan,
        "applied_suggestions": applied_suggestions,
    }))?)
}

fn artifact_ref<T>(
    run_id: Uuid,
    artifact_type: &str,
    artifact: &VersionedProductionArtifact<T>,
) -> ArtifactRef {
    ArtifactRef {
        run_id,
        artifact_type: artifact_type.into(),
        artifact_id: artifact.artifact_id,
        version: artifact.artifact_version,
        content_digest: artifact.content_digest.clone(),
        source_step_id: artifact.source_step_id,
        source_attempt: artifact.source_attempt,
    }
}

fn metadata_from_input(
    input: &ProductionPackageInput,
) -> ProductionResult<ProductionPackageMetadata> {
    let mut characters = BTreeMap::new();
    for brief in &input.performance_briefs {
        if characters
            .insert(
                brief.content.character_id.clone(),
                brief.content.character_bible_id,
            )
            .is_some()
        {
            return Err(domain_error(
                "duplicate Character in typed production input",
            ));
        }
    }
    let character_refs = characters
        .iter()
        .map(
            |(character_id, character_bible_id)| ProductionCharacterRef {
                character_bible_id: *character_bible_id,
                character_id: character_id.clone(),
            },
        )
        .collect::<Vec<_>>();
    let scenes = input
        .scenes
        .iter()
        .map(|scene| ProductionSceneRef {
            scene_id: scene.scene_id,
            scene_version: scene.scene_version.clone(),
            scene_digest: scene.scene_digest.clone(),
            sequence: scene.sequence,
            duration_sec: scene.duration_sec,
            character_bible_ids: scene.character_bible_ids.clone(),
        })
        .collect();
    let shots = input
        .shot_contracts
        .iter()
        .map(|shot| {
            Ok(ProductionShotRef {
                artifact_id: shot.artifact_id,
                shot_id: shot.content.shot_id.clone(),
                sequence: shot.content.sequence,
                scene_id: shot.content.scene_id,
                duration_sec: shot.content.duration_sec,
                character_bible_ids: shot
                    .content
                    .character_ids
                    .iter()
                    .map(|id| {
                        characters.get(id).copied().ok_or_else(|| {
                            domain_error("ShotContract references an unknown Character")
                        })
                    })
                    .collect::<ProductionResult<Vec<_>>>()?,
            })
        })
        .collect::<ProductionResult<Vec<_>>>()?;
    let performance_briefs = input
        .performance_briefs
        .iter()
        .map(|brief| ProductionPerformanceRef {
            artifact_id: brief.artifact_id,
            script_id: brief.content.script_id,
            character_bible_id: brief.content.character_bible_id,
            character_id: brief.content.character_id.clone(),
            scene_ids: brief
                .content
                .emotional_arc
                .iter()
                .map(|scene| scene.scene_id)
                .collect(),
        })
        .collect();
    Ok(ProductionPackageMetadata {
        script_id: input.script.script_id,
        script_version: input.script.script_version.clone(),
        script_digest: input.script.script_digest.clone(),
        scenes,
        characters: character_refs,
        shots,
        performance_briefs,
        sound_plan: ProductionSoundRef {
            artifact_id: input.sound_plan.artifact_id,
            script_id: input.sound_plan.content.script_id,
            scene_ids: input
                .sound_plan
                .content
                .scene_sound_notes
                .iter()
                .map(|scene| scene.scene_id)
                .collect(),
        },
        suggestion_resolutions: input
            .applied_suggestions
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
