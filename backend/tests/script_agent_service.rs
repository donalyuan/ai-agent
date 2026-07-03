use async_trait::async_trait;
use chrono::{DateTime, Utc};
use novex_api::agents::models::{
    GenerateScriptRequest, Script, ScriptListFilter, ScriptStatus, ScriptStyle, ScriptSummary,
};
use novex_api::agents::{LLMClient, LLMError, ScriptAgentError, ScriptAgentService};
use novex_api::repositories::{
    CreateProjectInput, Project, ProjectRepository, ProjectRepositoryError, ScriptRepository,
    ScriptRepositoryError,
};
use novex_model::LLMPrompt;
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

struct MemoryProjectRepository {
    project_ids: HashSet<Uuid>,
}

#[async_trait]
impl ProjectRepository for MemoryProjectRepository {
    async fn project_exists(&self, project_id: Uuid) -> Result<bool, ProjectRepositoryError> {
        Ok(self.project_ids.contains(&project_id))
    }

    async fn create_project(
        &self,
        input: CreateProjectInput,
    ) -> Result<Project, ProjectRepositoryError> {
        let now = Utc::now();
        Ok(Project {
            id: Uuid::new_v4(),
            name: input.name,
            positioning: input.positioning,
            description: input.description,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    async fn list_projects(&self) -> Result<Vec<Project>, ProjectRepositoryError> {
        let now = Utc::now();
        Ok(self
            .project_ids
            .iter()
            .map(|project_id| Project {
                id: *project_id,
                name: "测试项目".to_string(),
                positioning: "测试定位".to_string(),
                description: "脚本服务测试项目".to_string(),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            })
            .collect())
    }
}

#[derive(Default)]
struct MemoryScriptRepository {
    scripts: Mutex<HashMap<Uuid, Script>>,
}

#[async_trait]
impl ScriptRepository for MemoryScriptRepository {
    async fn save_script(&self, script: Script) -> Result<Script, ScriptRepositoryError> {
        self.scripts
            .lock()
            .unwrap()
            .insert(script.id, script.clone());
        Ok(script)
    }

    async fn get_script(&self, script_id: Uuid) -> Result<Script, ScriptRepositoryError> {
        self.scripts
            .lock()
            .unwrap()
            .get(&script_id)
            .cloned()
            .ok_or(ScriptRepositoryError::NotFound(script_id))
    }

    async fn list_scripts(
        &self,
        project_id: Uuid,
        filter: ScriptListFilter,
    ) -> Result<Vec<Script>, ScriptRepositoryError> {
        let mut scripts: Vec<Script> = self
            .scripts
            .lock()
            .unwrap()
            .values()
            .filter(|script| script.project_id == project_id)
            .filter(|script| {
                filter
                    .status
                    .as_ref()
                    .is_none_or(|status| status == &script.status)
            })
            .cloned()
            .collect();
        scripts.sort_by_key(|script| script.created_at);
        scripts.reverse();
        Ok(scripts)
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
        Ok(self
            .scripts
            .lock()
            .unwrap()
            .values()
            .filter(|script| script.project_id == project_id)
            .filter(|script| status.as_ref().is_none_or(|value| value == &script.status))
            .count() as i64)
    }

    async fn update_script_status(
        &self,
        script_id: Uuid,
        status: ScriptStatus,
    ) -> Result<Script, ScriptRepositoryError> {
        let mut scripts = self.scripts.lock().unwrap();
        let script = scripts
            .get_mut(&script_id)
            .ok_or(ScriptRepositoryError::NotFound(script_id))?;
        script.status = status;
        script.updated_at = Utc::now();
        Ok(script.clone())
    }
}

struct ScriptedLLMClient {
    responses: Mutex<VecDeque<Result<String, LLMError>>>,
}

#[async_trait]
impl LLMClient for ScriptedLLMClient {
    async fn generate_script(&self, _prompt: LLMPrompt) -> Result<String, LLMError> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("test LLM response should exist")
    }
}

fn service(
    project_ids: HashSet<Uuid>,
    responses: Vec<Result<String, LLMError>>,
) -> (
    ScriptAgentService,
    Arc<MemoryScriptRepository>,
    Arc<ScriptedLLMClient>,
) {
    let script_repository = Arc::new(MemoryScriptRepository::default());
    let llm_client = Arc::new(ScriptedLLMClient {
        responses: Mutex::new(VecDeque::from(responses)),
    });
    let service = ScriptAgentService::new(
        llm_client.clone(),
        script_repository.clone(),
        Arc::new(MemoryProjectRepository { project_ids }),
    );

    (service, script_repository, llm_client)
}

fn valid_llm_json() -> String {
    json!({
        "title": "程序员必看：ChatGPT工作流",
        "hook": "还在手写重复代码？",
        "scenes": [
            {
                "sequence": 1,
                "narration": "传统程序员每天要写大量重复代码，复制粘贴改参数，枯燥又容易出错，团队还要花很多时间检查这些重复劳动带来的隐藏问题。",
                "visual_description": "程序员盯着屏幕，快速切换多个代码文件。",
                "emotion": "焦虑",
                "duration_sec": 8
            },
            {
                "sequence": 2,
                "narration": "现在只要描述需求，AI 就能快速生成初稿，让开发者把时间放回设计和验证，从重复劳动转向架构判断、边界测试和真实业务理解。",
                "visual_description": "屏幕上弹出代码建议，程序员露出惊喜表情。",
                "emotion": "惊喜",
                "duration_sec": 9
            },
            {
                "sequence": 3,
                "narration": "更重要的是，AI 可以帮你解释陌生代码，让新人快速理解项目结构、关键流程和历史取舍，减少只靠猜测修改代码的风险。",
                "visual_description": "代码结构图展开，重点模块被高亮标注。",
                "emotion": "好奇",
                "duration_sec": 9
            },
            {
                "sequence": 4,
                "narration": "遇到报错时，把日志和上下文交给 AI，它能给出排查方向，但最终仍要由程序员验证证据、复现实验并确认根因。",
                "visual_description": "终端错误日志旁边出现排查清单。",
                "emotion": "紧张",
                "duration_sec": 10
            },
            {
                "sequence": 5,
                "narration": "未来的竞争不是谁会复制答案，而是谁能把 AI 产出的初稿打磨成可靠系统，并用工程纪律保证结果长期可维护。",
                "visual_description": "程序员提交通过测试的代码，仪表盘显示绿色通过。",
                "emotion": "平静",
                "duration_sec": 10
            }
        ]
    })
    .to_string()
}

fn request(project_id: Uuid) -> GenerateScriptRequest {
    GenerateScriptRequest {
        project_id,
        topic: "ChatGPT如何改变程序员工作流".to_string(),
        style: Some(ScriptStyle::Knowledge),
        scene_count: Some(5),
        parent_id: None,
    }
}

#[tokio::test]
async fn generate_script_retries_invalid_llm_json_then_persists_script() {
    let project_id = Uuid::new_v4();
    let (service, repository, _llm) = service(
        HashSet::from([project_id]),
        vec![Ok("不是 JSON".to_string()), Ok(valid_llm_json())],
    );

    let script = service.generate_script(request(project_id)).await.unwrap();

    assert_eq!(script.project_id, project_id);
    assert_eq!(script.scenes.len(), 5);
    assert_eq!(script.content["metadata"]["retry_count"], 1);
    assert_eq!(repository.count_scripts(project_id, None).await.unwrap(), 1);
}

#[tokio::test]
async fn generate_script_returns_project_not_found_before_calling_llm() {
    let missing_project_id = Uuid::new_v4();
    let (service, _repository, _llm) = service(HashSet::new(), vec![Ok(valid_llm_json())]);

    let error = service
        .generate_script(request(missing_project_id))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ScriptAgentError::ProjectNotFound(project_id) if project_id == missing_project_id
    ));
}

#[tokio::test]
async fn service_reads_lists_and_updates_script_status() {
    let project_id = Uuid::new_v4();
    let (service, _repository, _llm) =
        service(HashSet::from([project_id]), vec![Ok(valid_llm_json())]);
    let script = service.generate_script(request(project_id)).await.unwrap();

    let fetched = service.get_script(script.id).await.unwrap();
    assert_eq!(fetched.id, script.id);

    let listed = service
        .list_scripts(
            project_id,
            ScriptListFilter {
                status: Some(ScriptStatus::Draft),
                limit: Some(20),
                offset: Some(0),
            },
        )
        .await
        .unwrap();
    assert_eq!(listed.total, 1);
    assert_eq!(listed.scripts[0].script_id, script.id);
    assert_eq!(listed.scripts[0].scene_count, 5);

    let updated = service
        .update_status(script.id, ScriptStatus::Approved)
        .await
        .unwrap();
    assert_eq!(updated.status, ScriptStatus::Approved);
    assert!(updated.updated_at >= DateTime::<Utc>::MIN_UTC);
}
