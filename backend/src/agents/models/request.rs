use super::{
    ContentTopic, ContentTopicSource, ContentTopicStatus, Scene, Script, ScriptStatus,
    ScriptSummary, TopicGenerationBatchStatus, TopicGenerationBatchSummary,
};
use crate::agents::conversation::{
    AgentConversation, AgentConversationStatus, AgentMessage, AgentMessageRole, AgentRunRecord,
};
use crate::repositories::Project;
use crate::repositories::WorkspaceMenuTreeNode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct CreateProjectRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[serde(default)]
    #[validate(length(max = 500))]
    pub positioning: String,
    #[serde(default)]
    #[validate(length(max = 2000))]
    pub description: String,
}

impl CreateProjectRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("项目名称不能为空".to_string());
        }

        self.validate()
            .map_err(|error| format!("项目参数无效: {error}"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectResponse {
    pub project_id: Uuid,
    pub name: String,
    pub positioning: String,
    pub description: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Project> for ProjectResponse {
    fn from(project: Project) -> Self {
        Self {
            project_id: project.id,
            name: project.name,
            positioning: project.positioning,
            description: project.description,
            status: project.status,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResponse>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CreateContentTopicRequest {
    pub title: String,
    #[serde(default)]
    pub angle: String,
    #[serde(default)]
    pub target_audience: String,
    #[serde(default)]
    pub hook_points: Vec<String>,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub score_reason: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl CreateContentTopicRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        validate_topic_payload(TopicPayloadValidation {
            title: &self.title,
            angle: &self.angle,
            target_audience: &self.target_audience,
            hook_points: &self.hook_points,
            content_type: &self.content_type,
            score: self.score,
            score_reason: &self.score_reason,
            tags: &self.tags,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateContentTopicRequest {
    pub title: String,
    #[serde(default)]
    pub angle: String,
    #[serde(default)]
    pub target_audience: String,
    #[serde(default)]
    pub hook_points: Vec<String>,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub score_reason: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl UpdateContentTopicRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        validate_topic_payload(TopicPayloadValidation {
            title: &self.title,
            angle: &self.angle,
            target_audience: &self.target_audience,
            hook_points: &self.hook_points,
            content_type: &self.content_type,
            score: self.score,
            score_reason: &self.score_reason,
            tags: &self.tags,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateContentTopicStatusRequest {
    pub status: ContentTopicStatus,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct PrepareScriptFromTopicRequest {
    #[serde(default)]
    pub style: Option<ScriptStyle>,
    #[serde(default)]
    #[validate(range(min = 3, max = 12))]
    pub scene_count: Option<u8>,
}

impl PrepareScriptFromTopicRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        self.validate()
            .map_err(|error| format!("脚本确认参数无效: {error}"))
    }

    pub fn style_or_default(&self) -> ScriptStyle {
        self.style.clone().unwrap_or_default()
    }

    pub fn scene_count_or_default(&self) -> u8 {
        self.scene_count.unwrap_or(6)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ContentTopicResponse {
    pub topic_id: Uuid,
    pub project_id: Uuid,
    pub batch_id: Option<Uuid>,
    pub title: String,
    pub angle: String,
    pub target_audience: String,
    pub hook_points: Vec<String>,
    pub content_type: String,
    pub score: Option<f64>,
    pub score_reason: String,
    pub tags: Vec<String>,
    pub source: ContentTopicSource,
    pub status: ContentTopicStatus,
    pub metadata: Value,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ContentTopic> for ContentTopicResponse {
    fn from(topic: ContentTopic) -> Self {
        Self {
            topic_id: topic.id,
            project_id: topic.project_id,
            batch_id: topic.batch_id,
            title: topic.title,
            angle: topic.angle,
            target_audience: topic.target_audience,
            hook_points: topic.hook_points,
            content_type: topic.content_type,
            score: topic.score,
            score_reason: topic.score_reason,
            tags: topic.tags,
            source: topic.source,
            status: topic.status,
            metadata: topic.metadata,
            deleted_at: topic.deleted_at,
            created_at: topic.created_at,
            updated_at: topic.updated_at,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ContentTopicStatsResponse {
    pub total: i64,
    pub idea: i64,
    pub approved: i64,
    pub scripted: i64,
    pub archived: i64,
}

impl ContentTopicStatsResponse {
    pub fn from_counts(counts: Vec<(ContentTopicStatus, i64)>) -> Self {
        let mut stats = Self::default();
        for (status, count) in counts {
            stats.total += count;
            match status {
                ContentTopicStatus::Idea => stats.idea = count,
                ContentTopicStatus::Approved => stats.approved = count,
                ContentTopicStatus::Scripted => stats.scripted = count,
                ContentTopicStatus::Archived => stats.archived = count,
            }
        }
        stats
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ContentTopicListResponse {
    pub topics: Vec<ContentTopicResponse>,
    pub stats: ContentTopicStatsResponse,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicGenerationBatchSummaryResponse {
    pub batch_id: Uuid,
    pub project_id: Uuid,
    pub supplement_of_batch_id: Option<Uuid>,
    pub prompt: String,
    pub requested_count: i32,
    pub topic_count: i64,
    pub status: TopicGenerationBatchStatus,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TopicGenerationBatchSummary> for TopicGenerationBatchSummaryResponse {
    fn from(summary: TopicGenerationBatchSummary) -> Self {
        Self {
            batch_id: summary.batch.id,
            project_id: summary.batch.project_id,
            supplement_of_batch_id: summary.batch.supplement_of_batch_id,
            prompt: summary.batch.prompt,
            requested_count: summary.batch.requested_count,
            topic_count: summary.topic_count,
            status: summary.batch.status,
            error_message: summary.batch.error_message,
            created_at: summary.batch.created_at,
            updated_at: summary.batch.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicGenerationBatchListResponse {
    pub batches: Vec<TopicGenerationBatchSummaryResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicScriptRequestPreview {
    pub project_id: Uuid,
    pub topic_id: Uuid,
    pub topic: String,
    pub style: ScriptStyle,
    pub scene_count: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrepareScriptFromTopicResponse {
    pub topic: ContentTopicResponse,
    pub topic_snapshot: Value,
    pub script_request: TopicScriptRequestPreview,
}

struct TopicPayloadValidation<'a> {
    title: &'a str,
    angle: &'a str,
    target_audience: &'a str,
    hook_points: &'a [String],
    content_type: &'a str,
    score: Option<f64>,
    score_reason: &'a str,
    tags: &'a [String],
}

fn validate_topic_payload(payload: TopicPayloadValidation<'_>) -> Result<(), String> {
    if payload.title.trim().is_empty() {
        return Err("选题标题不能为空".to_string());
    }
    if payload.title.chars().count() > 160 {
        return Err("选题标题不能超过160个字符".to_string());
    }
    if payload.angle.chars().count() > 1000 {
        return Err("选题角度不能超过1000个字符".to_string());
    }
    if payload.target_audience.chars().count() > 500 {
        return Err("目标受众不能超过500个字符".to_string());
    }
    if payload.content_type.chars().count() > 80 {
        return Err("内容类型不能超过80个字符".to_string());
    }
    if let Some(score) = payload.score {
        if !(0.0..=100.0).contains(&score) {
            return Err("选题评分必须在0到100之间".to_string());
        }
    }
    if payload.score_reason.chars().count() > 1000 {
        return Err("评分理由不能超过1000个字符".to_string());
    }
    if payload.hook_points.len() > 10 {
        return Err("选题看点不能超过10个".to_string());
    }
    if payload.tags.len() > 20 {
        return Err("选题标签不能超过20个".to_string());
    }
    if payload
        .hook_points
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err("选题看点不能为空".to_string());
    }
    if payload.tags.iter().any(|value| value.trim().is_empty()) {
        return Err("选题标签不能为空".to_string());
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkspaceMenuListResponse {
    pub menus: Vec<WorkspaceMenuNodeResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkspaceMenuNodeResponse {
    pub menu_id: Uuid,
    pub menu_key: String,
    pub label: String,
    pub description: String,
    pub route_path: Option<String>,
    pub icon: String,
    pub menu_type: String,
    pub module_key: Option<String>,
    pub agent_key: Option<String>,
    pub sort_order: i32,
    pub is_enabled: bool,
    pub status: String,
    pub metadata: Value,
    pub children: Vec<WorkspaceMenuNodeResponse>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct CreateAgentConversationRequest {
    pub agent_type: String,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub subject_type: Option<String>,
    #[serde(default)]
    pub subject_id: Option<Uuid>,
    #[validate(length(min = 1, max = 160))]
    pub title: String,
    #[serde(default)]
    pub metadata: Value,
}

impl CreateAgentConversationRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("会话标题不能为空".to_string());
        }
        let agent_type = self.agent_type.trim();
        if !matches!(agent_type, "script" | "topic") {
            return Err("暂不支持该 Agent 类型".to_string());
        }
        if self.project_id.is_none() {
            return Err("Agent 会话必须绑定项目".to_string());
        }
        let subject_type = self.subject_type.as_deref().map(str::trim);
        if agent_type == "topic" && (self.subject_id.is_some() || subject_type.is_some()) {
            return Err("选题会话暂不绑定 subject".to_string());
        }
        if agent_type == "script" && self.subject_id.is_some() && subject_type != Some("script") {
            return Err("脚本会话 subject_type 必须为 script".to_string());
        }
        if agent_type == "script"
            && self.subject_id.is_none()
            && subject_type.is_some_and(|value| !value.is_empty())
        {
            return Err("未绑定脚本会话不能传 subject_type".to_string());
        }
        self.validate()
            .map_err(|error| format!("会话参数无效: {error}"))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct SendAgentMessageRequest {
    #[validate(length(min = 1, max = 4000))]
    pub content: String,
    #[serde(default)]
    pub supplement_of_batch_id: Option<Uuid>,
}

impl SendAgentMessageRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.content.trim().is_empty() {
            return Err("消息不能为空".to_string());
        }
        self.validate()
            .map_err(|error| format!("消息参数无效: {error}"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentConversationResponse {
    pub conversation_id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub status: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AgentConversation> for AgentConversationResponse {
    fn from(conversation: AgentConversation) -> Self {
        Self {
            conversation_id: conversation.id,
            project_id: conversation.project_id,
            agent_type: conversation.agent_type,
            subject_type: conversation.subject_type,
            subject_id: conversation.subject_id,
            title: conversation.title,
            status: conversation_status_value(&conversation.status).to_string(),
            metadata: conversation.metadata,
            created_at: conversation.created_at,
            updated_at: conversation.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentMessageResponse {
    pub message_id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

impl From<AgentMessage> for AgentMessageResponse {
    fn from(message: AgentMessage) -> Self {
        Self {
            message_id: message.id,
            conversation_id: message.conversation_id,
            role: message_role_value(&message.role).to_string(),
            content: message.content,
            metadata: message.metadata,
            created_at: message.created_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentRunResponse {
    pub run_id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub status: String,
    pub input: Value,
    pub output: Option<Value>,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

impl From<AgentRunRecord> for AgentRunResponse {
    fn from(run: AgentRunRecord) -> Self {
        Self {
            run_id: run.id,
            project_id: run.project_id,
            agent_type: run.agent_type,
            status: run.status,
            input: run.input,
            output: run.output,
            error_message: run.error_message,
            started_at: run.started_at,
            ended_at: run.ended_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentMessageListResponse {
    pub messages: Vec<AgentMessageResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentTurnResponseBody {
    pub user_message: AgentMessageResponse,
    pub assistant_message: AgentMessageResponse,
    pub run: AgentRunResponse,
}

fn conversation_status_value(status: &AgentConversationStatus) -> &'static str {
    match status {
        AgentConversationStatus::Active => "active",
        AgentConversationStatus::Archived => "archived",
    }
}

fn message_role_value(role: &AgentMessageRole) -> &'static str {
    match role {
        AgentMessageRole::System => "system",
        AgentMessageRole::User => "user",
        AgentMessageRole::Assistant => "assistant",
        AgentMessageRole::Tool => "tool",
    }
}

impl From<WorkspaceMenuTreeNode> for WorkspaceMenuNodeResponse {
    fn from(node: WorkspaceMenuTreeNode) -> Self {
        Self {
            menu_id: node.menu.id,
            menu_key: node.menu.menu_key,
            label: node.menu.label,
            description: node.menu.description,
            route_path: node.menu.route_path,
            icon: node.menu.icon,
            menu_type: node.menu.menu_type,
            module_key: node.menu.module_key,
            agent_key: node.menu.agent_key,
            sort_order: node.menu.sort_order,
            is_enabled: node.menu.is_enabled,
            status: node.menu.status,
            metadata: node.menu.metadata,
            children: node
                .children
                .into_iter()
                .map(WorkspaceMenuNodeResponse::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptStyle {
    #[default]
    Knowledge,
    Story,
    Tutorial,
}

impl ScriptStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Knowledge => "knowledge",
            Self::Story => "story",
            Self::Tutorial => "tutorial",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Knowledge => "知识科普类",
            Self::Story => "故事叙述类",
            Self::Tutorial => "教程讲解类",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct GenerateScriptRequest {
    pub project_id: Uuid,
    #[serde(default)]
    #[validate(length(max = 200))]
    pub topic: String,
    #[serde(default)]
    pub topic_id: Option<Uuid>,
    #[serde(default)]
    pub style: Option<ScriptStyle>,
    #[serde(default)]
    #[validate(range(min = 3, max = 12))]
    pub scene_count: Option<u8>,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
}

impl GenerateScriptRequest {
    pub fn style_or_default(&self) -> ScriptStyle {
        self.style.clone().unwrap_or_default()
    }

    pub fn scene_count_or_default(&self) -> u8 {
        self.scene_count.unwrap_or(6)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct ScriptListFilter {
    #[serde(default)]
    pub status: Option<ScriptStatus>,
    #[serde(default)]
    #[validate(range(min = 1, max = 100))]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

impl ScriptListFilter {
    pub fn limit_or_default(&self) -> u32 {
        self.limit.unwrap_or(20)
    }

    pub fn offset_or_default(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScriptResponse {
    pub script_id: Uuid,
    pub project_id: Uuid,
    pub topic_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_snapshot: Option<Value>,
    pub title: String,
    pub hook: String,
    pub scenes: Vec<SceneResponse>,
    pub status: ScriptStatus,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Script> for ScriptResponse {
    fn from(script: Script) -> Self {
        let topic_snapshot = script.content.get("topic_snapshot").cloned();
        Self {
            script_id: script.id,
            project_id: script.project_id,
            topic_id: script.topic_id,
            topic_snapshot,
            title: script.title,
            hook: script.hook,
            scenes: script.scenes.into_iter().map(SceneResponse::from).collect(),
            status: script.status,
            parent_id: script.parent_id,
            created_at: script.created_at,
            updated_at: script.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScriptSummaryResponse {
    pub script_id: Uuid,
    pub topic_id: Option<Uuid>,
    pub source_topic_title: Option<String>,
    pub title: String,
    pub status: ScriptStatus,
    pub scene_count: usize,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl From<Script> for ScriptSummaryResponse {
    fn from(script: Script) -> Self {
        let source_topic_title = script_source_topic_title(&script.content);
        Self {
            script_id: script.id,
            topic_id: script.topic_id,
            source_topic_title,
            title: script.title,
            status: script.status,
            scene_count: script.scenes.len(),
            parent_id: script.parent_id,
            created_at: script.created_at,
        }
    }
}

impl From<ScriptSummary> for ScriptSummaryResponse {
    fn from(summary: ScriptSummary) -> Self {
        Self {
            script_id: summary.script_id,
            topic_id: summary.topic_id,
            source_topic_title: summary.source_topic_title,
            title: summary.title,
            status: summary.status,
            scene_count: usize::try_from(summary.scene_count).unwrap_or(usize::MAX),
            parent_id: summary.parent_id,
            created_at: summary.created_at,
        }
    }
}

fn script_source_topic_title(content: &Value) -> Option<String> {
    content
        .get("topic_snapshot")
        .and_then(|snapshot| snapshot.get("title"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToString::to_string)
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScriptListResponse {
    pub scripts: Vec<ScriptSummaryResponse>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateScriptStatusRequest {
    pub status: ScriptStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UpdateScriptStatusResponse {
    pub script_id: Uuid,
    pub status: ScriptStatus,
    pub updated_at: DateTime<Utc>,
}

impl From<Script> for UpdateScriptStatusResponse {
    fn from(script: Script) -> Self {
        Self {
            script_id: script.id,
            status: script.status,
            updated_at: script.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SceneResponse {
    pub scene_id: Uuid,
    pub sequence: i32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: i32,
}

impl From<Scene> for SceneResponse {
    fn from(scene: Scene) -> Self {
        Self {
            scene_id: scene.id,
            sequence: scene.sequence,
            narration: scene.narration,
            visual_description: scene.visual_description,
            emotion: scene.emotion,
            duration_sec: scene.duration_sec,
        }
    }
}
