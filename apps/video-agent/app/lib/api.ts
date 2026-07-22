export type ScriptStatus = "draft" | "approved" | "archived";
export type ScriptStyle = "knowledge" | "story" | "tutorial";
export type ContentTopicStatus = "idea" | "approved" | "scripted" | "archived";
export type ContentTopicSource = "manual" | "agent";

export type AccountStrategyProfile = {
  target_audience: string;
  content_pillars: string[];
  tone_style: string;
  forbidden_topics: string[];
  reference_accounts: string[];
  topic_preferences: string;
};

export type Project = {
  project_id: string;
  name: string;
  positioning: string;
  description: string;
  strategy_profile: AccountStrategyProfile;
  status: string;
  created_at: string;
  updated_at: string;
};

export type ProjectListResponse = {
  projects: Project[];
};

export type WorkspaceMenuStatus = "active" | "planned" | "disabled";
export type WorkspaceMenuType = "section" | "page" | "group";

export type WorkspaceMenuNode = {
  menu_id: string;
  menu_key: string;
  label: string;
  description: string;
  route_path: string | null;
  icon: string;
  menu_type: WorkspaceMenuType;
  module_key: string | null;
  agent_key: string | null;
  sort_order: number;
  is_enabled: boolean;
  status: WorkspaceMenuStatus;
  metadata: Record<string, unknown>;
  children: WorkspaceMenuNode[];
};

export type WorkspaceMenuListResponse = {
  menus: WorkspaceMenuNode[];
};

export type CreateProjectPayload = {
  name: string;
  positioning: string;
  description: string;
  strategy_profile?: AccountStrategyProfile;
};

export type UpdateProjectStrategyProfilePayload = {
  name: string;
  positioning: string;
  description: string;
  strategy_profile: AccountStrategyProfile;
};

export type StrategyProfileDraftPayload = {
  direction_notes: string;
  model_id: string;
};

export type StrategyProfileDraftResponse = {
  draft: AccountStrategyProfile;
  draft_summary: string;
};

export type ScriptSummary = {
  script_id: string;
  topic_id: string | null;
  source_topic_title: string | null;
  title: string;
  status: ScriptStatus;
  scene_count: number;
  parent_id: string | null;
  created_at: string;
  updated_at: string;
};

export type Scene = {
  scene_id: string;
  sequence: number;
  narration: string;
  visual_description: string;
  emotion: string;
  duration_sec: number;
};

export type ScriptDetail = {
  script_id: string;
  project_id: string;
  topic_id: string | null;
  topic_snapshot?: ContentTopicSnapshot | null;
  title: string;
  hook: string;
  scenes: Scene[];
  status: ScriptStatus;
  parent_id: string | null;
  created_at: string;
  updated_at: string;
};

export type ContentTopic = {
  topic_id: string;
  project_id: string;
  batch_id: string | null;
  title: string;
  angle: string;
  target_audience: string;
  hook_points: string[];
  content_type: string;
  score: number | null;
  score_reason: string;
  tags: string[];
  source: ContentTopicSource;
  status: ContentTopicStatus;
  metadata: Record<string, unknown>;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
};

export type MaterialType = "video" | "image" | "audio" | "subtitle";
export type MaterialStatus = "active" | "archived";
export type MaterialStatusFilter = MaterialStatus | "all";
export type AudioUsage = "tts" | "bgm" | "ambient" | "action_sfx" | "mixed" | "other";
export type MaterialSource = "user_upload" | "ai_generated" | "work_generation";

export type MaterialGenerationSnapshot = {
  work_id?: string;
  work_version_id?: string;
  generation_run_id?: string;
  generation_step_id?: string;
  artifact_role?: string;
  audio_usage?: AudioUsage;
  model_snapshot?: Record<string, unknown>;
  voice_snapshot?: Record<string, unknown>;
  prompt_snapshot?: Record<string, unknown>;
  timeline_snapshot?: Record<string, unknown>;
  resource_usage?: Record<string, unknown>;
  request_trace_id?: string;
  alignment_source?: string;
  source_audio_material_id?: string;
  duration_sec?: number;
  subtitle_format?: string;
};

export type Material = {
  material_id: string;
  project_id: string;
  material_type: MaterialType;
  file_url: string;
  thumbnail_url: string | null;
  file_name: string;
  tags: string[];
  metadata: Record<string, unknown>;
  source: MaterialSource | null;
  audio_usage: AudioUsage | null;
  work_id: string | null;
  work_version_id: string | null;
  generation: MaterialGenerationSnapshot | null;
  usage_count: number;
  status: MaterialStatus;
  created_at: string;
  updated_at: string;
};

export type MaterialPayload = {
  material_type: MaterialType;
  file_url: string;
  thumbnail_url?: string | null;
  file_name: string;
  tags: string[];
  metadata: Record<string, unknown>;
};

export type MaterialUploadPayload = {
  file: File;
  file_name: string;
  tags: string[];
  audio_usage?: AudioUsage;
};

export type MaterialListResponse = {
  materials: Material[];
};

export type MaterialFilters = {
  material_type?: MaterialType | "all";
  status?: MaterialStatusFilter;
  q?: string;
  tag?: string;
  audio_usage?: AudioUsage | "all";
  source?: MaterialSource | "all";
  work_id?: string;
  work_version_id?: string;
};

export type AssetGenerationProvider = "gpt-image-2" | "jimeng";
export type AssetGenerationTaskType = "image_candidates" | "video_draft" | "video_generation";
export type AssetGenerationTaskStatus =
  | "draft"
  | "pending"
  | "processing"
  | "completed"
  | "failed";
export type SceneAssetCandidateType = "image" | "video";
export type SceneAssetCandidateSource = "existing_material" | "ai_generated" | "video_task";
export type SceneAssetCandidateStatus = "candidate" | "selected" | "rejected" | "failed";

export type AssetGenerationRequestPayload = {
  model_id: string;
  image_candidates_per_scene: number;
  use_reference_materials: boolean;
};

export type AssetGenerationPlanResponse = {
  script_id: string;
  scene_count: number;
  image_candidate_count: number;
  max_image_candidate_count: number;
  model_id: string;
  provider: AssetGenerationProvider;
  reference_material_count: number;
  can_create: boolean;
  warnings: string[];
};

export type AssetGenerationTask = {
  task_id: string;
  project_id: string;
  script_id: string | null;
  scene_id: string | null;
  model_id: string | null;
  model_snapshot: Record<string, unknown> | null;
  provider: AssetGenerationProvider;
  task_type: AssetGenerationTaskType;
  status: AssetGenerationTaskStatus;
  candidate_count: number;
  reference_material_ids: string[];
  params: Record<string, unknown>;
  result: Record<string, unknown>;
  error_message: string | null;
  retry_count: number;
  dismissed_at: string | null;
  read_only: boolean;
  created_at: string;
  updated_at: string;
};

export type AssetGenerationTaskListResponse = {
  script_id: string;
  tasks: AssetGenerationTask[];
};

export type SceneAssetCandidate = {
  candidate_id: string;
  project_id: string;
  script_id: string;
  scene_id: string;
  material_id: string | null;
  candidate_type: SceneAssetCandidateType;
  source: SceneAssetCandidateSource;
  status: SceneAssetCandidateStatus;
  rank: number;
  generation_task_id: string | null;
  metadata: Record<string, unknown>;
  file_url: string | null;
  thumbnail_url: string | null;
  file_name: string | null;
  created_at: string;
  updated_at: string;
};

export type SceneAssetCandidateListResponse = {
  candidates: SceneAssetCandidate[];
};

export type SceneVisualManifestBlockerReason =
  | "image_generation_failed"
  | "selected_image_missing"
  | "selected_material_missing"
  | "selected_material_not_image"
  | "material_archived"
  | "material_url_missing";

export type SceneVisualManifestBlocker = {
  scene_id: string;
  sequence: number;
  reason: SceneVisualManifestBlockerReason;
};

export type SceneVisualManifestItem = {
  scene_id: string;
  sequence: number;
  narration: string;
  visual_description: string;
  emotion: string;
  duration_sec: number;
  candidate_id: string;
  material_id: string;
  file_url: string;
  thumbnail_url: string | null;
  source_snapshot: Record<string, unknown>;
};

export type SceneVisualManifest = {
  script_id: string;
  project_id?: string;
  script_title: string;
  script_updated_at: string;
  input_version: string;
  scenes: SceneVisualManifestItem[];
};

export type WorkPlanResponse = {
  work_id: string;
  work_title: string;
  plan_id: string;
  work_version_id: string;
  plan_version: number;
  status: string;
  input_fingerprint: string;
  model_snapshot: Record<string, unknown>;
  capability_snapshot: Record<string, unknown>;
  output_snapshot: Record<string, unknown>;
  prompt_snapshot: Record<string, unknown>;
  timeline_snapshot: Record<string, unknown>;
  resource_usage: Record<string, unknown>;
  warnings: unknown;
  segments: Array<Record<string, unknown>>;
  can_confirm: boolean;
  blockers: string[];
  created_at: string;
};

export type WorkRunResponse = {
  run_id: string;
  work_id: string;
  work_version_id: string;
  work_plan_id: string;
  status: string;
  created: boolean;
  resource_usage: Record<string, unknown>;
};

export type WorkGenerationTask = {
  id: string;
  work_id: string;
  work_version_id: string;
  work_plan_id: string;
  title: string;
  version_no: number;
  status: string;
  current_stage: string;
  progress_percent: number;
  successful_steps: number;
  running_steps: number;
  queued_steps: number;
  failed_steps: number;
  can_cancel?: boolean;
  cancel_mode?: "local" | "provider" | "none";
  cancel_block_reason?: string | null;
  resource_usage: Record<string, unknown>;
  error_category: string | null;
  error_summary: string | null;
  created_at: string;
  updated_at: string;
  dismissed_at: string | null;
};

export type WorkGenerationAttempt = {
  id: string;
  attempt_no: number;
  status: string;
  model_snapshot: Record<string, unknown>;
  resource_usage: Record<string, unknown>;
  error_category: string | null;
  error_code: string | null;
  error_summary: string | null;
  request_trace_id: string | null;
  upstream_task_id: string | null;
  provider_cancel_supported?: boolean;
  cancel_requested_at?: string | null;
  cancel_response?: string | null;
  created_at: string;
  updated_at: string;
};

export type WorkGenerationStep = {
  id: string;
  step_no: number;
  step_type: string;
  status: string;
  is_required: boolean;
  depends_on: unknown;
  model_snapshot: Record<string, unknown>;
  resource_usage: Record<string, unknown>;
  result_material_ids: unknown;
  external_task_id: string | null;
  error_category: string | null;
  error_code: string | null;
  error_summary: string | null;
  attempts: WorkGenerationAttempt[];
};

export type WorkGenerationTaskDetails = {
  task: WorkGenerationTask;
  steps: WorkGenerationStep[];
};

export type WorkGenerationTaskCounts = {
  pending: number;
  running: number;
  completed: number;
  attention: number;
  cancelled: number;
  total: number;
};

export type WorkGenerationTaskListResponse = {
  tasks: WorkGenerationTask[];
  counts: WorkGenerationTaskCounts;
};

export type WorkSummary = {
  id: string;
  project_id: string;
  script_id: string;
  title: string;
  status: string;
  archived: boolean;
  current_version_id: string | null;
  current_completed_version_id: string | null;
  current_completed_version_no: number | null;
  aspect_ratio: string | null;
  duration_seconds: number | null;
  cover_artifact_id: string | null;
  cover_storage_path: string | null;
  created_at: string;
  updated_at: string;
};

export type WorkVersion = {
  id: string;
  work_id: string;
  version_no: number;
  status: "draft" | "confirmed" | "running" | "completed" | "failed";
  source_version_id: string | null;
  derivation_kind: "initial" | "edit" | "full_regeneration";
  source_manifest_version: string;
  input_snapshot: Record<string, unknown>;
  model_snapshot: Record<string, unknown>;
  parameter_snapshot: Record<string, unknown>;
  prompt_snapshot: Record<string, unknown>;
  timeline_snapshot: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
};

export type WorkArtifact = {
  id: string;
  work_version_id: string;
  version_status: string;
  role: "final_video" | "subtitle" | "mix" | "audio_track" | "production_package" | "reusable_intermediate";
  material_id: string | null;
  file_name: string;
  storage_path: string;
  mime_type: string;
  size_bytes: number;
  sha256: string;
  metadata: Record<string, unknown>;
};

export type WorkTimeline = {
  work_version_id: string;
  video: unknown[];
  audio: unknown[];
  subtitles: unknown[];
};

export type WorkGenerationAudit = {
  id: string;
  work_version_id: string;
  status: string;
  current_stage: string;
  progress_percent: number;
  error_category: string | null;
  error_summary: string | null;
  attempt_count: number;
  created_at: string;
  updated_at: string;
};

export type WorkDetails = {
  id: string;
  project_id: string;
  script_id: string;
  title: string;
  status: string;
  archived: boolean;
  current_version_id: string | null;
  versions: WorkVersion[];
  artifacts: WorkArtifact[];
  timelines: WorkTimeline[];
  generation_audit: WorkGenerationAudit[];
  model_catalog?: Record<string, { display_name: string; model_type: string }>;
  created_at: string;
  updated_at: string;
};

export type WorkListResponse = { items: WorkSummary[]; archived: boolean };

export type DeriveWorkVersionPayload = {
  input_snapshot_patch?: Record<string, unknown>;
  model_snapshot_patch?: Record<string, unknown>;
  parameter_snapshot_patch?: Record<string, unknown>;
  prompt_snapshot_patch?: Record<string, unknown>;
  timeline_snapshot_patch?: Record<string, unknown>;
};

export type WorkVersionChange = { path: string; old_value: unknown; new_value: unknown };

export type WorkVersionDiff = {
  id: string;
  work_id: string;
  source_version_id: string;
  draft_version_id: string;
  plan_version: number;
  source_fingerprint: string;
  draft_fingerprint: string;
  changes: WorkVersionChange[];
  affected_nodes: string[];
  reused_artifact_ids: string[];
  resource_usage: Record<string, number>;
  status: string;
  created_at: string;
};

export type WorkDiffConfirmation = { run_id: string; diff_plan_id: string; created: boolean };
export type WorkArtifactDownload = { artifact: WorkArtifact; integrity_status: "available" | "missing" | "corrupt" };
export type WorkDownloadManifest = { work_version_id: string; artifacts: WorkArtifactDownload[] };
export type WorkPublicationHandoff = {
  id: string;
  work_id: string;
  work_version_id: string;
  final_video_artifact_id: string;
  subtitle_artifact_id: string | null;
  status: "draft";
  payload: Record<string, unknown>;
  created_at: string;
  created: boolean;
};

export type WorkPlanPayload = {
  llm_model_id: string;
  video_model_id: string;
  tts_model_id?: string | null;
  tts_voice_type?: string | null;
  narration_override?: string | null;
  duration_strategy: "preset15" | "preset30" | "preset45" | "preset60" | "custom" | "follow_narration";
  duration_seconds?: number | null;
  aspect_ratio: string;
  resolution: string;
  audio_mode: "independent_tts" | "seedance_original" | "seedance_original_and_tts" | "silent";
  full_prompt: string;
  scene_prompts?: string[];
  segment_prompts?: string[];
  narration_seconds?: number;
  audio_material_ids?: string[];
  burn_subtitles?: boolean;
};

export type ContentTopicSnapshot = {
  topic_id: string;
  title: string;
  angle: string;
  target_audience: string;
  hook_points: string[];
  content_type: string;
  score: number | null;
  score_reason: string;
  tags: string[];
  source: ContentTopicSource;
  status: ContentTopicStatus;
  created_at: string;
};

export type ContentTopicStats = {
  total: number;
  idea: number;
  approved: number;
  scripted: number;
  archived: number;
};

export type TopicGenerationBatchStatus = "running" | "succeeded" | "failed";
export type TopicReviewSnapshotStatus = "succeeded" | "failed";
export type TopicQualityEvaluationStatus = "succeeded" | "failed";
export type TopicReviewPriority = "priority" | "backup" | "reject";
export type TopicQualityDecision = "pass" | "reject";
export type TopicGroupSort = "script_priority" | "created_at";
export type TopicGroupReviewFreshness = "fresh" | "missing" | "stale";
export type TopicGroupScriptPriorityStatus =
  | "ready_for_script"
  | "needs_review"
  | "needs_supplement"
  | "defer";
export type TopicReviewRiskFlag =
  | "too_generic"
  | "duplicate"
  | "hard_to_script"
  | "off_positioning"
  | "compliance_risk";
export type TopicQualityFlag =
  | "too_generic"
  | "duplicate"
  | "off_positioning"
  | "hard_to_script"
  | "compliance_risk"
  | "score_untrusted";

export type TopicGenerationBatchSummary = {
  batch_id: string;
  project_id: string;
  supplement_of_batch_id: string | null;
  prompt: string;
  requested_count: number;
  topic_count: number;
  status: TopicGenerationBatchStatus;
  error_message: string | null;
  created_at: string;
  updated_at: string;
};

export type ContentTopicListResponse = {
  topics: ContentTopic[];
  stats: ContentTopicStats;
};

export type TopicGenerationBatchListResponse = {
  batches: TopicGenerationBatchSummary[];
};

export type TopicGroupScriptPriorityMetrics = {
  priority_count: number;
  backup_count: number;
  reject_count: number;
  duplicate_count: number;
  hard_to_script_count: number;
  off_positioning_count: number;
  compliance_risk_count: number;
  ready_candidate_count: number;
  high_score_topic_count: number;
};

export type TopicGroupScriptPriority = {
  status: TopicGroupScriptPriorityStatus;
  score: number | null;
  reason: string;
  metrics: TopicGroupScriptPriorityMetrics;
  recommended_topic_ids: string[];
};

export type TopicGroupSummary = {
  root_batch_id: string;
  project_id: string;
  prompt: string;
  created_at: string;
  topic_count: number;
  supplement_batch_count: number;
  latest_review_snapshot_id: string | null;
  review_freshness: TopicGroupReviewFreshness;
  script_priority: TopicGroupScriptPriority;
};

export type TopicGroupListResponse = {
  topic_groups: TopicGroupSummary[];
};

export type TopicReviewItem = {
  topic_id: string;
  priority: TopicReviewPriority;
  reason: string;
  risk_flags: TopicReviewRiskFlag[];
  similar_topic_ids: string[];
};

export type TopicReviewResult = {
  topic_reviews: TopicReviewItem[];
};

export type TopicReviewSnapshot = {
  snapshot_id: string;
  project_id: string;
  root_batch_id: string;
  source_run_id: string | null;
  status: TopicReviewSnapshotStatus;
  review_summary: string;
  result: TopicReviewResult;
  error_message: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

export type TopicQualityGateItem = {
  candidate_key: string;
  title: string;
  decision: TopicQualityDecision;
  quality_score: number;
  flags: TopicQualityFlag[];
  reason: string;
};

export type TopicQualityGateResult = {
  summary: string;
  items: TopicQualityGateItem[];
};

export type TopicQualityEvaluation = {
  evaluation_id: string;
  project_id: string;
  batch_id: string;
  source_run_id: string | null;
  status: TopicQualityEvaluationStatus;
  pass_count: number;
  reject_count: number;
  rewrite_triggered: boolean;
  result: TopicQualityGateResult;
  error_message: string | null;
  created_at: string;
  updated_at: string;
};

export type DeletedContentTopicResponse = {
  topic_id: string;
  deleted_at: string;
};

export type ContentTopicFilters = {
  status?: ContentTopicStatus | "all";
  source?: ContentTopicSource | "all";
  batch_id?: string | null;
};

export type ContentTopicPayload = {
  title: string;
  angle: string;
  target_audience: string;
  hook_points: string[];
  content_type: string;
  score?: number | null;
  score_reason: string;
  tags: string[];
};

export type PrepareScriptFromTopicPayload = {
  style?: ScriptStyle;
  scene_count?: number;
};

export type TopicScriptRequestPreview = {
  project_id: string;
  topic_id: string;
  topic: string;
  style: ScriptStyle;
  scene_count: number;
};

export type PrepareScriptFromTopicResponse = {
  topic: ContentTopic;
  topic_snapshot: ContentTopicSnapshot;
  script_request: TopicScriptRequestPreview;
};

export type ScriptListResponse = {
  scripts: ScriptSummary[];
  total: number;
  limit: number;
  offset: number;
};

export type GenerateScriptPayload = {
  project_id: string;
  model_id: string;
  topic?: string;
  topic_id?: string | null;
  style?: ScriptStyle;
  scene_count?: number;
  parent_id?: string | null;
};

export type UpdateScriptStatusResponse = {
  script_id: string;
  status: ScriptStatus;
  updated_at: string;
};

export type AgentType = "script" | "topic" | "sound" | "work" | "material" | "video" | "publish" | "optimization";
export type AgentMessageRole = "user" | "assistant" | "system";
export type AgentRunStatus = "running" | "succeeded" | "completed" | "failed";
export type ScriptAgentIntent = "generate_script" | "edit_script";

export type AgentConversation = {
  conversation_id: string;
  project_id: string | null;
  agent_type: AgentType;
  subject_type: string | null;
  subject_id: string | null;
  title: string;
  status: "active" | "archived";
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

export type AgentMessage = {
  message_id: string;
  conversation_id: string;
  role: AgentMessageRole;
  content: string;
  metadata: Record<string, unknown>;
  created_at: string;
};

export type ScriptAgentTurnMetadata = {
  intent: ScriptAgentIntent;
  script_id: string | null;
  script_created: boolean;
  needs_input: boolean;
  missing_fields: string[];
};

export type AgentRun = {
  run_id: string;
  conversation_id?: string;
  project_id: string | null;
  agent_type: AgentType;
  status: AgentRunStatus;
  input: Record<string, unknown>;
  output: Record<string, unknown> | null;
  error?: string | null;
  error_message?: string | null;
  started_at: string;
  ended_at?: string | null;
  finished_at?: string | null;
};

export type CreateAgentConversationPayload = {
  project_id: string;
  agent_type: AgentType;
  subject_type?: string | null;
  subject_id?: string | null;
  title?: string;
  metadata?: Record<string, unknown>;
};

export type AgentMessageListResponse = {
  messages: AgentMessage[];
};

export type SoundAgentContextPayload = {
  speech_model_id: string;
  tts_text: string;
  voice_type: string;
  language: string;
  parameters: Record<string, unknown>;
  subtitle_segments: string[];
};

export type SendAgentMessagePayload = {
  content: string;
  model_id: string;
  supplement_of_batch_id?: string | null;
  sound_context?: SoundAgentContextPayload;
};

export type AgentTurnResponse = {
  user_message: AgentMessage;
  assistant_message: AgentMessage;
  run: AgentRun;
};

export type ApiClient = {
  baseUrl: string;
  fetcher: typeof fetch;
};

export type ModelType = "text" | "image" | "video" | "speech";
export type ModelOption = {
  model_id: string;
  display_name: string;
  model_type: ModelType;
  provider_name: string;
  api_protocol: string;
  upstream_model: string;
  is_default: boolean;
  capabilities?: Record<string, unknown>;
};

export type ModelOptionListResponse = { models: ModelOption[] };

export type VoiceCatalogSync = {
  sync_id: string;
  model_id: string;
  trigger_source: "admin" | "workspace" | "scheduled";
  status: "queued" | "running" | "succeeded" | "failed";
  page_limit: number;
  page_count: number;
  speaker_count: number;
  error_summary: string | null;
  requested_at: string;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
};

export type VoiceCatalogEntry = {
  voice_id: string;
  voice_type: string;
  resource_id: string;
  name: string;
  avatar_url: string | null;
  gender: string | null;
  age: string | null;
  categories: unknown;
  normal_labels: string[];
  special_labels: string[];
  trial_url: string | null;
  short_trial_url: string | null;
  languages: unknown;
  emotions: unknown;
  description: string;
  is_available: boolean;
  catalog_version: number;
  created_at: string;
  updated_at: string;
};

export type VoiceCatalog = {
  model_id: string;
  source_model_id: string;
  model_settings: Record<string, unknown>;
  last_sync: VoiceCatalogSync | null;
  voices: VoiceCatalogEntry[];
};

export type SoundTaskType = "tts_preview" | "tts" | "asr";
export type SoundTaskStatus = "queued" | "running" | "succeeded" | "failed" | "cancelled";

export type SoundTaskIntent = {
  task_type: SoundTaskType;
  model_id: string;
  text_content?: string;
  voice_type?: string | null;
  language?: string | null;
  emotion?: string | null;
  parameters?: Record<string, unknown>;
  generate_subtitle?: boolean;
  subtitle_segments?: string[];
  source_audio_material_id?: string | null;
  audio_inspection_id?: string | null;
  source_script_id?: string | null;
  source_script_updated_at?: string | null;
  source_script_scene_ids?: string[];
};

export type SoundScriptSourceSnapshot = {
  script_id: string;
  title: string;
  updated_at: string;
  scenes: Array<{ scene_id: string; sequence: number; narration: string }>;
};

export type ConfirmedSoundTaskIntent = SoundTaskIntent & { confirmation_token: string };

export type SoundTaskPreflight = {
  task_type: SoundTaskType;
  model_id: string;
  model_display_name: string;
  voice_snapshot: Record<string, unknown> | null;
  resource_usage: Record<string, unknown>;
  normalized_parameters: Record<string, unknown>;
  source_script_snapshot: SoundScriptSourceSnapshot | null;
  confirmation_token: string;
};

export type AudioInspection = {
  inspection_id: string;
  project_id: string;
  material_id: string;
  status: "queued" | "running" | "succeeded" | "failed";
  source_sha256: string | null;
  file_size_bytes: number | null;
  duration_ms: number | null;
  container_format: string | null;
  audio_codec: string | null;
  sample_rate_hz: number | null;
  channel_count: number | null;
  error_code: string | null;
  error_summary: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
};

export type SoundTask = {
  task_id: string;
  project_id: string;
  parent_task_id: string | null;
  task_type: SoundTaskType;
  status: SoundTaskStatus;
  model_id: string;
  audio_inspection_id: string | null;
  source_audio_material_id: string | null;
  source_script_id: string | null;
  source_script_snapshot: SoundScriptSourceSnapshot | null;
  output_audio_material_id: string | null;
  output_subtitle_material_id: string | null;
  text_content: string;
  voice_type: string | null;
  language: string | null;
  emotion: string | null;
  parameters: Record<string, unknown>;
  generate_subtitle: boolean;
  subtitle_segments: string[];
  model_snapshot: Record<string, unknown> | null;
  voice_snapshot: Record<string, unknown> | null;
  resource_usage: Record<string, unknown>;
  timeline: unknown;
  result: Record<string, unknown> | null;
  request_id: string;
  upstream_log_id: string | null;
  attempt_count: number;
  max_attempts: number;
  error_code: string | null;
  error_summary: string | null;
  error_details: {
    http_status?: number;
    provider_error_code?: string;
    provider_error_message?: string;
  };
  staging_status: string;
  cleanup_attempt_count: number;
  cleanup_error_summary: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
};

export type SoundTaskListResponse = { tasks: SoundTask[] };

export class ApiError extends Error {
  status: number;
  details: unknown;

  constructor(status: number, message: string, details: unknown) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.details = details;
  }
}

const defaultApiPort = "18180";
const localApiBaseUrl = `http://localhost:${defaultApiPort}`;

function getDefaultApiBaseUrl() {
  const currentLocation = globalThis.location;
  if (currentLocation?.protocol && currentLocation.hostname) {
    return `${currentLocation.protocol}//${currentLocation.hostname}:${defaultApiPort}`;
  }

  return localApiBaseUrl;
}

export function getApiBaseUrl() {
  return (process.env.NEXT_PUBLIC_API_BASE_URL || getDefaultApiBaseUrl()).replace(/\/+$/, "");
}

export function createApiClient(options: Partial<ApiClient> = {}): ApiClient {
  return {
    baseUrl: (options.baseUrl || getApiBaseUrl()).replace(/\/+$/, ""),
    fetcher: options.fetcher || ((...args) => globalThis.fetch(...args)),
  };
}

export async function checkHealth(client: ApiClient): Promise<boolean> {
  try {
    const response = await client.fetcher(`${client.baseUrl}/health`, {
      headers: { accept: "application/json" },
    });
    return response.ok;
  } catch {
    return false;
  }
}

export function listProjects(client: ApiClient) {
  return request<ProjectListResponse>(client, "/api/projects");
}

export function listWorkspaceMenus(client: ApiClient) {
  return request<WorkspaceMenuListResponse>(client, "/api/video-workspace/menus");
}

export function createProject(client: ApiClient, payload: CreateProjectPayload) {
  return request<Project>(client, "/api/projects", {
    method: "POST",
    body: payload,
  });
}

export function listModelOptions(client: ApiClient, modelType: ModelType) {
  return request<ModelOptionListResponse>(
    client,
    `/api/model-options?type=${encodeURIComponent(modelType)}`,
  );
}

export function getVoiceCatalog(
  client: ApiClient,
  modelId: string,
  includeUnavailable = false,
) {
  const query = includeUnavailable ? "?include_unavailable=true" : "";
  return request<VoiceCatalog>(
    client,
    `/api/speech/models/${modelId}/voice-catalog${query}`,
  );
}

export function requestWorkspaceVoiceCatalogCheck(client: ApiClient, modelId: string) {
  return request<VoiceCatalogSync>(
    client,
    `/api/speech/models/${modelId}/voice-catalog/check`,
    { method: "POST" },
  );
}

export function requestAudioInspection(
  client: ApiClient,
  projectId: string,
  materialId: string,
  idempotencyKey: string,
) {
  return request<AudioInspection>(
    client,
    `/api/projects/${projectId}/audio-materials/${materialId}/inspection`,
    { method: "POST", headers: { "Idempotency-Key": idempotencyKey } },
  );
}

export function getAudioInspection(client: ApiClient, projectId: string, materialId: string) {
  return request<AudioInspection>(
    client,
    `/api/projects/${projectId}/audio-materials/${materialId}/inspection`,
  );
}

export function preflightSoundTask(
  client: ApiClient,
  projectId: string,
  payload: SoundTaskIntent,
) {
  return request<SoundTaskPreflight>(
    client,
    `/api/projects/${projectId}/sound-subtitle/tasks/preflight`,
    { method: "POST", body: payload },
  );
}

export function createSoundTask(
  client: ApiClient,
  projectId: string,
  payload: ConfirmedSoundTaskIntent,
  idempotencyKey: string,
) {
  return request<SoundTask>(client, `/api/projects/${projectId}/sound-subtitle/tasks`, {
    method: "POST",
    body: payload,
    headers: { "Idempotency-Key": idempotencyKey },
  }).then((task) => normalizeSoundTaskUrls(client, task));
}

export function listSoundTasks(client: ApiClient, projectId: string) {
  return request<SoundTaskListResponse>(
    client,
    `/api/projects/${projectId}/sound-subtitle/tasks`,
  ).then((response) => ({
    tasks: response.tasks.map((task) => normalizeSoundTaskUrls(client, task)),
  }));
}

export function getSoundTask(client: ApiClient, projectId: string, taskId: string) {
  return request<SoundTask>(
    client,
    `/api/projects/${projectId}/sound-subtitle/tasks/${taskId}`,
  ).then((task) => normalizeSoundTaskUrls(client, task));
}

export function retrySoundTask(
  client: ApiClient,
  projectId: string,
  taskId: string,
  payload: ConfirmedSoundTaskIntent,
  idempotencyKey: string,
) {
  return request<SoundTask>(
    client,
    `/api/projects/${projectId}/sound-subtitle/tasks/${taskId}/retry`,
    {
      method: "POST",
      body: payload,
      headers: { "Idempotency-Key": idempotencyKey },
    },
  ).then((task) => normalizeSoundTaskUrls(client, task));
}

export function cancelSoundTask(client: ApiClient, projectId: string, taskId: string) {
  return request<SoundTask>(
    client,
    `/api/projects/${projectId}/sound-subtitle/tasks/${taskId}/cancel`,
    { method: "POST" },
  ).then((task) => normalizeSoundTaskUrls(client, task));
}

export function updateProjectStrategyProfile(
  client: ApiClient,
  projectId: string,
  payload: UpdateProjectStrategyProfilePayload,
) {
  return request<Project>(client, `/api/projects/${projectId}/strategy-profile`, {
    method: "PUT",
    body: payload,
  });
}

export function generateStrategyProfileDraft(
  client: ApiClient,
  projectId: string,
  payload: StrategyProfileDraftPayload,
) {
  return request<StrategyProfileDraftResponse>(
    client,
    `/api/projects/${projectId}/strategy-profile/draft`,
    {
      method: "POST",
      body: payload,
    },
  );
}

export function listScripts(
  client: ApiClient,
  projectId: string,
  filters: { status?: ScriptStatus | "all"; limit?: number; offset?: number } = {},
) {
  const searchParams = new URLSearchParams();
  if (filters.status && filters.status !== "all") {
    searchParams.set("status", filters.status);
  }
  if (filters.limit !== undefined) searchParams.set("limit", String(filters.limit));
  if (filters.offset !== undefined) searchParams.set("offset", String(filters.offset));
  const query = searchParams.toString();
  return request<ScriptListResponse>(
    client,
    `/api/projects/${projectId}/scripts${query ? `?${query}` : ""}`,
  );
}

export function listContentTopics(
  client: ApiClient,
  projectId: string,
  filters: ContentTopicFilters = {},
) {
  const searchParams = new URLSearchParams();
  if (filters.status && filters.status !== "all") {
    searchParams.set("status", filters.status);
  }
  if (filters.source && filters.source !== "all") {
    searchParams.set("source", filters.source);
  }
  if (filters.batch_id) {
    searchParams.set("batch_id", filters.batch_id);
  }
  const query = searchParams.toString();
  return request<ContentTopicListResponse>(
    client,
    `/api/projects/${projectId}/topics${query ? `?${query}` : ""}`,
  );
}

export function listMaterials(
  client: ApiClient,
  projectId: string,
  filters: MaterialFilters = {},
) {
  const searchParams = new URLSearchParams();
  if (filters.material_type && filters.material_type !== "all") {
    searchParams.set("type", filters.material_type);
  }
  if (filters.status && filters.status !== "all") {
    searchParams.set("status", filters.status);
  }
  if (filters.q?.trim()) {
    searchParams.set("q", filters.q.trim());
  }
  if (filters.tag?.trim()) {
    searchParams.set("tag", filters.tag.trim());
  }
  if (filters.audio_usage && filters.audio_usage !== "all") {
    searchParams.set("audio_usage", filters.audio_usage);
  }
  if (filters.source && filters.source !== "all") {
    searchParams.set("source", filters.source);
  }
  if (filters.work_id?.trim()) {
    searchParams.set("work_id", filters.work_id.trim());
  }
  if (filters.work_version_id?.trim()) {
    searchParams.set("work_version_id", filters.work_version_id.trim());
  }
  const query = searchParams.toString();
  return request<MaterialListResponse>(
    client,
    `/api/projects/${projectId}/materials${query ? `?${query}` : ""}`,
  ).then((response) => ({
    ...response,
    materials: response.materials.map((material) => normalizeMaterialUrls(client, material)),
  }));
}

export function createMaterial(
  client: ApiClient,
  projectId: string,
  payload: MaterialPayload,
) {
  return request<Material>(client, `/api/projects/${projectId}/materials`, {
    method: "POST",
    body: payload,
  }).then((material) => normalizeMaterialUrls(client, material));
}

export async function uploadMaterial(
  client: ApiClient,
  projectId: string,
  payload: MaterialUploadPayload,
) {
  const body = new FormData();
  body.append("file", payload.file);
  body.append("file_name", payload.file_name.trim());
  body.append("tags", JSON.stringify(payload.tags));
  if (payload.audio_usage) {
    body.append("audio_usage", payload.audio_usage);
  }
  const response = await client.fetcher(
    `${client.baseUrl}/api/projects/${projectId}/materials/upload`,
    {
      method: "POST",
      headers: { accept: "application/json" },
      body,
    },
  );
  const responseBody = await parseJson(response);
  if (!response.ok) {
    throw new ApiError(response.status, errorMessage(responseBody), responseBody);
  }
  return normalizeMaterialUrls(client, responseBody as Material);
}

export function getMaterial(client: ApiClient, materialId: string) {
  return request<Material>(client, `/api/materials/${materialId}`).then((material) =>
    normalizeMaterialUrls(client, material),
  );
}

export function updateMaterial(
  client: ApiClient,
  materialId: string,
  payload: MaterialPayload,
) {
  return request<Material>(client, `/api/materials/${materialId}`, {
    method: "PUT",
    body: {
      ...payload,
      file_url: stableApiAssetUrl(client, payload.file_url),
      thumbnail_url: stableApiAssetUrl(client, payload.thumbnail_url || null),
    },
  }).then((material) => normalizeMaterialUrls(client, material));
}

export function updateMaterialStatus(
  client: ApiClient,
  materialId: string,
  status: MaterialStatus,
) {
  return request<Material>(client, `/api/materials/${materialId}/status`, {
    method: "PUT",
    body: { status },
  }).then((material) => normalizeMaterialUrls(client, material));
}

export function getAssetGenerationPlan(
  client: ApiClient,
  scriptId: string,
  payload: AssetGenerationRequestPayload,
) {
  return request<AssetGenerationPlanResponse>(
    client,
    `/api/scripts/${scriptId}/asset-generation-plan`,
    {
      method: "POST",
      body: payload,
    },
  );
}

export function createAssetGenerationTasks(
  client: ApiClient,
  scriptId: string,
  payload: AssetGenerationRequestPayload,
) {
  return request<AssetGenerationTaskListResponse>(
    client,
    `/api/scripts/${scriptId}/asset-generation-tasks`,
    {
      method: "POST",
      body: payload,
    },
  );
}

export function listAssetGenerationTasks(client: ApiClient, scriptId: string) {
  return request<AssetGenerationTaskListResponse>(
    client,
    `/api/scripts/${scriptId}/asset-generation-tasks`,
  );
}

export function listAssetCandidates(client: ApiClient, scriptId: string) {
  return request<SceneAssetCandidateListResponse>(
    client,
    `/api/scripts/${scriptId}/asset-candidates`,
  ).then((response) => ({
    ...response,
    candidates: response.candidates.map((candidate) => normalizeCandidateUrls(client, candidate)),
  }));
}

export function getSceneVisualManifest(client: ApiClient, scriptId: string) {
  return request<SceneVisualManifest>(
    client,
    `/api/scripts/${scriptId}/scene-visual-manifest`,
  ).then((manifest) => normalizeSceneVisualManifestUrls(client, manifest));
}

export function validateSceneVisualManifest(
  client: ApiClient,
  scriptId: string,
  expectedInputVersion: string,
) {
  return request<SceneVisualManifest>(
    client,
    `/api/scripts/${scriptId}/scene-visual-manifest/validate`,
    {
      method: "POST",
      body: { expected_input_version: expectedInputVersion },
    },
  ).then((manifest) => normalizeSceneVisualManifestUrls(client, manifest));
}

export function createWorkPlan(client: ApiClient, scriptId: string, payload: WorkPlanPayload) {
  return request<WorkPlanResponse>(client, `/api/scripts/${scriptId}/work-generation/plans`, {
    method: "POST",
    body: payload,
  });
}

export function confirmWorkPlan(client: ApiClient, planId: string, idempotencyKey: string) {
  return request<WorkRunResponse>(client, `/api/work-generation/plans/${planId}/confirm`, {
    method: "POST",
    headers: { "idempotency-key": idempotencyKey },
  });
}

export function listWorkGenerationTasks(
  client: ApiClient,
  projectId: string,
  filters: { view?: string; stage?: string; query?: string; include_hidden?: boolean } = {},
) {
  const params = new URLSearchParams();
  if (filters.view) params.set("view", filters.view);
  if (filters.stage) params.set("stage", filters.stage);
  if (filters.query) params.set("query", filters.query);
  if (filters.include_hidden) params.set("include_hidden", "true");
  const suffix = params.toString() ? `?${params.toString()}` : "";
  return request<WorkGenerationTaskListResponse>(
    client,
    `/api/projects/${projectId}/work-generation/tasks${suffix}`,
  );
}

export function getWorkGenerationTask(client: ApiClient, runId: string) {
  return request<WorkGenerationTaskDetails>(client, `/api/work-generation/runs/${runId}`);
}

export function cancelWorkGenerationRun(client: ApiClient, runId: string) {
  return request<WorkGenerationTaskDetails>(client, `/api/work-generation/runs/${runId}/cancel`, { method: "POST" });
}

export function dismissWorkGenerationRun(client: ApiClient, runId: string) {
  return request<WorkGenerationTaskDetails>(client, `/api/work-generation/runs/${runId}/dismiss`, { method: "POST" });
}

export function retryWorkGenerationStep(client: ApiClient, stepId: string, idempotencyKey: string) {
  return request<WorkGenerationAttempt>(client, `/api/work-generation/steps/${stepId}/retry`, {
    method: "POST",
    headers: { "idempotency-key": idempotencyKey },
  });
}

export function listWorks(
  client: ApiClient,
  projectId: string,
  filters: { archived?: boolean; query?: string } = {},
) {
  const params = new URLSearchParams();
  params.set("archived", filters.archived ? "true" : "false");
  if (filters.query?.trim()) params.set("query", filters.query.trim());
  return request<WorkListResponse>(client, `/api/projects/${projectId}/works?${params.toString()}`);
}

export function getWork(client: ApiClient, workId: string) {
  return request<WorkDetails>(client, `/api/works/${workId}`);
}

export function deriveWorkVersion(client: ApiClient, versionId: string, payload: DeriveWorkVersionPayload = {}) {
  return request<WorkVersion>(client, `/api/work-versions/${versionId}/derive`, { method: "POST", body: payload });
}

export function regenerateWorkVersion(client: ApiClient, versionId: string) {
  return request<WorkVersion>(client, `/api/work-versions/${versionId}/regenerate`, { method: "POST" });
}

export function analyzeWorkVersionDiff(client: ApiClient, versionId: string) {
  return request<WorkVersionDiff>(client, `/api/work-versions/${versionId}/diff`, { method: "POST" });
}

export function confirmWorkVersionDiff(client: ApiClient, diffId: string, idempotencyKey: string) {
  return request<WorkDiffConfirmation>(client, `/api/work-version-diffs/${diffId}/confirm`, {
    method: "POST",
    headers: { "idempotency-key": idempotencyKey },
  });
}

export function deleteWork(client: ApiClient, workId: string) {
  return request<null>(client, `/api/works/${workId}`, { method: "DELETE" });
}

export function archiveWork(client: ApiClient, workId: string) {
  return request<Pick<WorkSummary, "id" | "title" | "status" | "archived">>(client, `/api/works/${workId}/archive`, { method: "POST" });
}

export function restoreWork(client: ApiClient, workId: string) {
  return request<Pick<WorkSummary, "id" | "title" | "status" | "archived">>(client, `/api/works/${workId}/restore`, { method: "POST" });
}

export function getWorkVersionDownloads(client: ApiClient, versionId: string) {
  return request<WorkDownloadManifest>(client, `/api/work-versions/${versionId}/downloads`);
}

export function getWorkArtifactDownloadUrl(client: ApiClient, artifactId: string) {
  return `${client.baseUrl}/api/work-artifacts/${encodeURIComponent(artifactId)}/download`;
}

export function getProductionPackageDownloadUrl(client: ApiClient, versionId: string) {
  return `${client.baseUrl}/api/work-versions/${encodeURIComponent(versionId)}/production-package`;
}

export function createPublicationHandoff(client: ApiClient, versionId: string, idempotencyKey: string) {
  return request<WorkPublicationHandoff>(client, `/api/work-versions/${versionId}/publication-handoffs`, {
    method: "POST",
    headers: { "idempotency-key": idempotencyKey },
  });
}

export function selectAssetCandidate(client: ApiClient, sceneId: string, candidateId: string) {
  return request<SceneAssetCandidate>(
    client,
    `/api/scenes/${sceneId}/asset-candidates/${candidateId}/select`,
    {
      method: "PUT",
    },
  ).then((candidate) => normalizeCandidateUrls(client, candidate));
}

export function rejectAssetCandidate(client: ApiClient, sceneId: string, candidateId: string) {
  return request<SceneAssetCandidate>(
    client,
    `/api/scenes/${sceneId}/asset-candidates/${candidateId}/reject`,
    {
      method: "PUT",
    },
  ).then((candidate) => normalizeCandidateUrls(client, candidate));
}

export function createSceneAssetGenerationTask(
  client: ApiClient,
  sceneId: string,
  payload: AssetGenerationRequestPayload,
  idempotencyKey: string,
) {
  return request<AssetGenerationTask>(client, `/api/scenes/${sceneId}/asset-generation-tasks`, {
    method: "POST",
    body: payload,
    headers: { "idempotency-key": idempotencyKey },
  });
}

export function dismissAssetGenerationTask(client: ApiClient, taskId: string) {
  return request<AssetGenerationTask>(client, `/api/asset-generation-tasks/${taskId}/dismiss`, {
    method: "POST",
  });
}

export function listTopicGenerationBatches(client: ApiClient, projectId: string) {
  return request<TopicGenerationBatchListResponse>(
    client,
    `/api/projects/${projectId}/topic-generation-batches`,
  );
}

export function listTopicGroups(
  client: ApiClient,
  projectId: string,
  options: { sort?: TopicGroupSort } = {},
) {
  const searchParams = new URLSearchParams();
  if (options.sort) {
    searchParams.set("sort", options.sort);
  }
  const query = searchParams.toString();
  return request<TopicGroupListResponse>(
    client,
    `/api/projects/${projectId}/topic-groups${query ? `?${query}` : ""}`,
  );
}

export function createTopicGroupReview(
  client: ApiClient,
  rootBatchId: string,
  payload: { model_id: string },
) {
  return request<TopicReviewSnapshot>(client, `/api/topic-groups/${rootBatchId}/reviews`, {
    method: "POST",
    body: payload,
  });
}

export function getLatestTopicGroupReview(client: ApiClient, rootBatchId: string) {
  return request<TopicReviewSnapshot | null>(
    client,
    `/api/topic-groups/${rootBatchId}/reviews/latest`,
  );
}

export function getLatestTopicQualityEvaluation(
  client: ApiClient,
  batchId: string,
  projectId?: string,
) {
  const searchParams = new URLSearchParams();
  if (projectId) {
    searchParams.set("project_id", projectId);
  }
  const query = searchParams.toString();
  return request<TopicQualityEvaluation | null>(
    client,
    `/api/topic-generation-batches/${batchId}/quality-evaluation${query ? `?${query}` : ""}`,
  );
}

export function createContentTopic(
  client: ApiClient,
  projectId: string,
  payload: ContentTopicPayload,
) {
  return request<ContentTopic>(client, `/api/projects/${projectId}/topics`, {
    method: "POST",
    body: payload,
  });
}

export function updateContentTopic(
  client: ApiClient,
  topicId: string,
  payload: ContentTopicPayload,
) {
  return request<ContentTopic>(client, `/api/topics/${topicId}`, {
    method: "PUT",
    body: payload,
  });
}

export function updateContentTopicStatus(
  client: ApiClient,
  topicId: string,
  status: ContentTopicStatus,
) {
  return request<ContentTopic>(client, `/api/topics/${topicId}/status`, {
    method: "PUT",
    body: { status },
  });
}

export function deleteContentTopic(client: ApiClient, topicId: string) {
  return request<DeletedContentTopicResponse>(client, `/api/topics/${topicId}`, {
    method: "DELETE",
  });
}

export function prepareScriptFromTopic(
  client: ApiClient,
  topicId: string,
  payload: PrepareScriptFromTopicPayload = {},
) {
  return request<PrepareScriptFromTopicResponse>(client, `/api/topics/${topicId}/prepare-script`, {
    method: "POST",
    body: payload,
  });
}

export function getScript(client: ApiClient, scriptId: string) {
  return request<ScriptDetail>(client, `/api/scripts/${scriptId}`);
}

export function generateScript(client: ApiClient, payload: GenerateScriptPayload) {
  return request<ScriptDetail>(client, "/api/scripts/generate", {
    method: "POST",
    body: payload,
  });
}

export function updateScriptStatus(
  client: ApiClient,
  scriptId: string,
  status: ScriptStatus,
) {
  return request<UpdateScriptStatusResponse>(client, `/api/scripts/${scriptId}/status`, {
    method: "PUT",
    body: { status },
  });
}

export function createAgentConversation(
  client: ApiClient,
  payload: CreateAgentConversationPayload,
) {
  return request<AgentConversation>(client, "/api/agent/conversations", {
    method: "POST",
    body: payload,
  });
}

export function listAgentMessages(client: ApiClient, conversationId: string) {
  return request<AgentMessageListResponse>(
    client,
    `/api/agent/conversations/${conversationId}/messages`,
  );
}

export function sendAgentMessage(
  client: ApiClient,
  conversationId: string,
  payload: SendAgentMessagePayload,
) {
  return request<AgentTurnResponse>(
    client,
    `/api/agent/conversations/${conversationId}/messages`,
    {
      method: "POST",
      body: payload,
    },
  );
}

export function getScriptAgentTurnMetadata(message: AgentMessage): ScriptAgentTurnMetadata {
  const metadata = message.metadata;
  return {
    intent: getScriptAgentIntent(metadata.intent),
    script_id: typeof metadata.script_id === "string" ? metadata.script_id : null,
    script_created: metadata.script_created === true,
    needs_input: metadata.needs_input === true,
    missing_fields: Array.isArray(metadata.missing_fields)
      ? metadata.missing_fields.filter((field): field is string => typeof field === "string")
      : [],
  };
}

function getScriptAgentIntent(value: unknown): ScriptAgentIntent {
  return value === "edit_script" ? "edit_script" : "generate_script";
}

async function request<T>(
  client: ApiClient,
  path: string,
  options: {
    method?: "GET" | "POST" | "PUT" | "DELETE";
    body?: unknown;
    headers?: Record<string, string>;
  } = {},
): Promise<T> {
  const headers: Record<string, string> = {
    accept: "application/json",
    ...options.headers,
  };
  const init: RequestInit = { headers };

  if (options.method) {
    init.method = options.method;
  }

  if (options.body !== undefined) {
    headers["content-type"] = "application/json";
    init.body = JSON.stringify(options.body);
  }

  const response = await client.fetcher(`${client.baseUrl}${path}`, init);
  const body = await parseJson(response);

  if (!response.ok) {
    throw new ApiError(response.status, errorMessage(body), body);
  }

  return body as T;
}

function normalizeMaterialUrls(client: ApiClient, material: Material): Material {
  return {
    ...material,
    file_url: resolveApiAssetUrl(client, material.file_url) || material.file_url,
    thumbnail_url: resolveApiAssetUrl(client, material.thumbnail_url),
  };
}

function normalizeSoundTaskUrls(client: ApiClient, task: SoundTask): SoundTask {
  if (!task.result) {
    return task;
  }
  const result = { ...task.result };
  for (const key of ["audio_file_url", "subtitle_file_url"]) {
    if (typeof result[key] === "string") {
      result[key] = resolveApiAssetUrl(client, result[key] as string);
    }
  }
  return { ...task, result };
}

function stableApiAssetUrl(client: ApiClient, value: string | null): string | null {
  const assetPrefix = `${client.baseUrl}/assets/`;
  return value?.startsWith(assetPrefix) ? value.slice(client.baseUrl.length) : value;
}

function normalizeCandidateUrls(
  client: ApiClient,
  candidate: SceneAssetCandidate,
): SceneAssetCandidate {
  return {
    ...candidate,
    file_url: resolveApiAssetUrl(client, candidate.file_url),
    thumbnail_url: resolveApiAssetUrl(client, candidate.thumbnail_url),
  };
}

function normalizeSceneVisualManifestUrls(
  client: ApiClient,
  manifest: SceneVisualManifest,
): SceneVisualManifest {
  return {
    ...manifest,
    scenes: manifest.scenes.map((scene) => ({
      ...scene,
      file_url: resolveApiAssetUrl(client, scene.file_url) || scene.file_url,
      thumbnail_url: resolveApiAssetUrl(client, scene.thumbnail_url),
    })),
  };
}

function resolveApiAssetUrl(client: ApiClient, value: string | null): string | null {
  if (!value?.startsWith("/assets/")) {
    return value;
  }
  return `${client.baseUrl}${value}`;
}

async function parseJson(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) {
    return null;
  }

  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function errorMessage(body: unknown) {
  if (body && typeof body === "object" && "error" in body) {
    const error = (body as { error: unknown }).error;
    if (typeof error === "string" && error.trim()) {
      return error;
    }
    if (error && typeof error === "object" && "message" in error) {
      const message = (error as { message?: unknown }).message;
      if (typeof message === "string" && message.trim()) {
        return message;
      }
    }
  }

  return "请求失败";
}
