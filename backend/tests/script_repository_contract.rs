use async_trait::async_trait;
use chrono::Utc;
use novex_api::agents::models::{Scene, Script, ScriptListFilter, ScriptStatus, ScriptSummary};
use novex_api::repositories::{ScriptRepository, ScriptRepositoryError};
use serde_json::json;
use uuid::Uuid;

fn sample_script(project_id: Uuid, status: ScriptStatus) -> Script {
    let now = Utc::now();
    Script::new(
        Uuid::new_v4(),
        project_id,
        None,
        "程序员必看：ChatGPT工作流".to_string(),
        "还在手写重复代码？".to_string(),
        json!({"topic": "ChatGPT如何改变程序员工作流"}),
        status,
        None,
        vec![Scene {
            id: Uuid::new_v4(),
            sequence: 1,
            narration: "传统程序员每天要写大量重复代码。".to_string(),
            visual_description: "程序员盯着屏幕，快速切换多个代码文件。".to_string(),
            emotion: "焦虑".to_string(),
            duration_sec: 8,
        }],
        now,
        now,
    )
}

struct MemoryScriptRepository {
    script: Script,
}

#[async_trait]
impl ScriptRepository for MemoryScriptRepository {
    async fn save_script(&self, script: Script) -> Result<Script, ScriptRepositoryError> {
        Ok(script)
    }

    async fn get_script(&self, script_id: Uuid) -> Result<Script, ScriptRepositoryError> {
        if self.script.id == script_id {
            Ok(self.script.clone())
        } else {
            Err(ScriptRepositoryError::NotFound(script_id))
        }
    }

    async fn list_scripts(
        &self,
        project_id: Uuid,
        filter: ScriptListFilter,
    ) -> Result<Vec<Script>, ScriptRepositoryError> {
        if self.script.project_id != project_id {
            return Ok(Vec::new());
        }

        if filter
            .status
            .as_ref()
            .is_some_and(|status| status != &self.script.status)
        {
            return Ok(Vec::new());
        }

        Ok(vec![self.script.clone()])
    }

    async fn list_script_summaries(
        &self,
        project_id: Uuid,
        filter: ScriptListFilter,
    ) -> Result<Vec<ScriptSummary>, ScriptRepositoryError> {
        Ok(self
            .list_scripts(project_id, filter)
            .await?
            .into_iter()
            .map(|script| ScriptSummary {
                script_id: script.id,
                topic_id: script.topic_id,
                source_topic_title: script
                    .content
                    .get("topic_snapshot")
                    .and_then(|snapshot| snapshot.get("title"))
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string),
                title: script.title,
                status: script.status,
                scene_count: script.scenes.len() as i64,
                parent_id: script.parent_id,
                created_at: script.created_at,
            })
            .collect())
    }

    async fn count_scripts(
        &self,
        project_id: Uuid,
        status: Option<ScriptStatus>,
    ) -> Result<i64, ScriptRepositoryError> {
        if self.script.project_id != project_id {
            return Ok(0);
        }

        if status
            .as_ref()
            .is_some_and(|status| status != &self.script.status)
        {
            return Ok(0);
        }

        Ok(1)
    }

    async fn update_script_status(
        &self,
        script_id: Uuid,
        status: ScriptStatus,
    ) -> Result<Script, ScriptRepositoryError> {
        let mut script = self.get_script(script_id).await?;
        script.status = status;
        Ok(script)
    }

    async fn update_scene(
        &self,
        script_id: Uuid,
        scene: Scene,
    ) -> Result<Script, ScriptRepositoryError> {
        let mut script = self.get_script(script_id).await?;
        let existing_scene = script
            .scenes
            .iter_mut()
            .find(|current_scene| current_scene.sequence == scene.sequence)
            .ok_or(ScriptRepositoryError::SceneNotFound {
                script_id,
                sequence: scene.sequence,
            })?;
        existing_scene.narration = scene.narration;
        existing_scene.visual_description = scene.visual_description;
        existing_scene.emotion = scene.emotion;
        existing_scene.duration_sec = scene.duration_sec;
        Ok(script)
    }
}

#[tokio::test]
async fn script_repository_trait_supports_script_lifecycle_operations() {
    let project_id = Uuid::new_v4();
    let script = sample_script(project_id, ScriptStatus::Draft);
    let repository = MemoryScriptRepository {
        script: script.clone(),
    };

    let saved = repository.save_script(script.clone()).await.unwrap();
    assert_eq!(saved.id, script.id);

    let fetched = repository.get_script(script.id).await.unwrap();
    assert_eq!(fetched.id, script.id);

    let draft_filter = ScriptListFilter {
        status: Some(ScriptStatus::Draft),
        limit: Some(20),
        offset: Some(0),
    };
    let listed = repository
        .list_scripts(project_id, draft_filter)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    let summaries = repository
        .list_script_summaries(
            project_id,
            ScriptListFilter {
                status: Some(ScriptStatus::Draft),
                limit: Some(20),
                offset: Some(0),
            },
        )
        .await
        .unwrap();
    assert_eq!(summaries[0].script_id, script.id);
    assert_eq!(summaries[0].scene_count, 1);
    assert_eq!(
        repository
            .count_scripts(project_id, Some(ScriptStatus::Draft))
            .await
            .unwrap(),
        1
    );

    let approved = repository
        .update_script_status(script.id, ScriptStatus::Approved)
        .await
        .unwrap();
    assert_eq!(approved.status, ScriptStatus::Approved);

    let updated_scene_script = repository
        .update_scene(
            script.id,
            Scene {
                id: script.scenes[0].id,
                sequence: 1,
                narration: "修改后的旁白。".to_string(),
                visual_description: "修改后的画面。".to_string(),
                emotion: "紧张".to_string(),
                duration_sec: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated_scene_script.scenes[0].emotion, "紧张");
}
