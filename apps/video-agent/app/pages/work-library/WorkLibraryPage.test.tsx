import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createApiClient, type Project, type WorkDetails, type WorkSummary } from "../../lib/api";
import { WorkLibraryPage } from "./WorkLibraryPage";

const apiMocks = vi.hoisted(() => ({
  list: vi.fn(),
  details: vi.fn(),
  derive: vi.fn(),
  regenerate: vi.fn(),
  diff: vi.fn(),
  confirmDiff: vi.fn(),
  downloads: vi.fn(),
  archive: vi.fn(),
  restore: vi.fn(),
  remove: vi.fn(),
  handoff: vi.fn(),
  publicationPlan: vi.fn(),
  createConversation: vi.fn(),
  sendMessage: vi.fn(),
}));

vi.mock("../../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/api")>()),
  listWorks: apiMocks.list,
  getWork: apiMocks.details,
  deriveWorkVersion: apiMocks.derive,
  regenerateWorkVersion: apiMocks.regenerate,
  analyzeWorkVersionDiff: apiMocks.diff,
  confirmWorkVersionDiff: apiMocks.confirmDiff,
  getWorkVersionDownloads: apiMocks.downloads,
  archiveWork: apiMocks.archive,
  restoreWork: apiMocks.restore,
  deleteWork: apiMocks.remove,
  createPublicationHandoff: apiMocks.handoff,
  createPublicationPlan: apiMocks.publicationPlan,
  createAgentConversation: apiMocks.createConversation,
  sendAgentMessage: apiMocks.sendMessage,
}));

const textModels = [{
  model_id: "model-text",
  display_name: "默认文本模型",
  model_type: "text" as const,
  provider_name: "test",
  api_protocol: "openai_responses",
  upstream_model: "test-model",
  is_default: true,
}];

const project: Project = {
  project_id: "project-1",
  name: "科技账号",
  positioning: "知识内容",
  description: "",
  strategy_profile: {
    target_audience: "",
    content_pillars: [],
    tone_style: "",
    forbidden_topics: [],
    reference_accounts: [],
    topic_preferences: "",
  },
  status: "active",
  created_at: "2026-07-22T00:00:00Z",
  updated_at: "2026-07-22T00:00:00Z",
};

const works: WorkSummary[] = [
  {
    id: "work-1",
    project_id: project.project_id,
    script_id: "script-1",
    title: "夏日防晒指南",
    status: "succeeded",
    archived: false,
    current_version_id: "version-2",
    current_completed_version_id: "version-2",
    current_completed_version_no: 2,
    aspect_ratio: "9:16",
    duration_seconds: 30,
    cover_artifact_id: "video-2",
    cover_storage_path: "works/final.mp4",
    created_at: "2026-07-21T00:00:00Z",
    updated_at: "2026-07-22T00:00:00Z",
  },
  {
    id: "work-2",
    project_id: project.project_id,
    script_id: "script-2",
    title: "AI 工作流",
    status: "failed",
    archived: false,
    current_version_id: "version-3",
    current_completed_version_id: null,
    current_completed_version_no: null,
    aspect_ratio: "16:9",
    duration_seconds: 15,
    cover_artifact_id: null,
    cover_storage_path: null,
    created_at: "2026-07-20T00:00:00Z",
    updated_at: "2026-07-20T00:00:00Z",
  },
];

const details: WorkDetails = {
  id: works[0].id,
  project_id: project.project_id,
  script_id: works[0].script_id,
  title: works[0].title,
  status: works[0].status,
  archived: false,
  current_version_id: "version-2",
  versions: [
    {
      id: "version-2",
      work_id: works[0].id,
      version_no: 2,
      status: "completed",
      source_version_id: "version-1",
      derivation_kind: "edit",
      source_manifest_version: "manifest-2",
      input_snapshot: { scenes: [{ id: "scene-1", narration: "注意补涂防晒" }] },
      model_snapshot: { video: { display_name: "Seedance 2.0" } },
      parameter_snapshot: { aspect_ratio: "9:16", resolution: "1080p" },
      prompt_snapshot: { full_prompt: "海边防晒知识短片" },
      timeline_snapshot: { audio_mode: "independent_tts" },
      created_at: "2026-07-22T00:00:00Z",
      updated_at: "2026-07-22T00:01:00Z",
      completed_at: "2026-07-22T00:05:00Z",
    },
    {
      id: "version-1",
      work_id: works[0].id,
      version_no: 1,
      status: "failed",
      source_version_id: null,
      derivation_kind: "initial",
      source_manifest_version: "manifest-1",
      input_snapshot: {},
      model_snapshot: {},
      parameter_snapshot: {},
      prompt_snapshot: {},
      timeline_snapshot: {},
      created_at: "2026-07-21T00:00:00Z",
      updated_at: "2026-07-21T00:01:00Z",
      completed_at: "2026-07-21T00:05:00Z",
    },
  ],
  artifacts: [
    {
      id: "video-2",
      work_version_id: "version-2",
      version_status: "completed",
      role: "final_video",
      material_id: "material-video",
      file_name: "final-v2.mp4",
      storage_path: "works/final-v2.mp4",
      mime_type: "video/mp4",
      size_bytes: 1024,
      sha256: "a".repeat(64),
      metadata: {},
    },
    {
      id: "subtitle-2",
      work_version_id: "version-2",
      version_status: "completed",
      role: "subtitle",
      material_id: "material-subtitle",
      file_name: "final-v2.srt",
      storage_path: "works/final-v2.srt",
      mime_type: "application/x-subrip",
      size_bytes: 256,
      sha256: "b".repeat(64),
      metadata: {},
    },
  ],
  timelines: [{
    work_version_id: "version-2",
    video: [{ label: "镜头 1", start_seconds: 0, duration_seconds: 15 }],
    audio: [{ label: "TTS 配音", start_seconds: 0, duration_seconds: 30 }],
    subtitles: [{ label: "中文字幕", start_seconds: 0, duration_seconds: 30 }],
  }],
  generation_audit: [
    {
      id: "run-failed",
      work_version_id: "version-1",
      status: "failed",
      current_stage: "video_segment",
      progress_percent: 40,
      error_category: "provider",
      error_summary: "上游视频生成失败",
      attempt_count: 2,
      created_at: "2026-07-21T00:00:00Z",
      updated_at: "2026-07-21T00:01:00Z",
    },
  ],
  created_at: "2026-07-21T00:00:00Z",
  updated_at: "2026-07-22T00:00:00Z",
};

describe("WorkLibraryPage", () => {
  beforeEach(() => {
    Object.values(apiMocks).forEach((mock) => mock.mockReset());
    apiMocks.list.mockResolvedValue({ items: works, archived: false });
    apiMocks.details.mockResolvedValue(details);
    apiMocks.downloads.mockResolvedValue({ work_version_id: "version-2", artifacts: [] });
    apiMocks.publicationPlan.mockResolvedValue({ id: "plan-1" });
  });

  it("默认网格可切换高密度列表，并在筛选刷新后保持视图与作品选择", async () => {
    render(<WorkLibraryPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} />);

    expect(await screen.findByRole("region", { name: "作品网格" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "列表视图" }));
    fireEvent.click(screen.getByRole("button", { name: /夏日防晒指南.*查看详情/ }));
    expect(await screen.findByRole("heading", { name: "夏日防晒指南" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "返回作品库" }));
    expect(screen.getByRole("region", { name: "作品列表" })).toBeInTheDocument();

    fireEvent.change(screen.getByRole("searchbox", { name: "搜索作品" }), { target: { value: "防晒" } });
    await waitFor(() => expect(apiMocks.list).toHaveBeenLastCalledWith(expect.anything(), project.project_id, { archived: false, query: "防晒" }));
    expect(screen.getByRole("region", { name: "作品列表" })).toBeInTheDocument();
  });

  it("详情严格按所选版本展示成片、三轨时间轴、快照和失败审计", async () => {
    render(<WorkLibraryPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} onOpenGenerationTask={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: /夏日防晒指南.*查看详情/ }));

    expect(await screen.findByLabelText("V2 成片预览")).toHaveAttribute("src", "http://api.test/api/work-artifacts/video-2/download");
    expect(screen.getByText("视频轨")).toBeInTheDocument();
    expect(screen.getByText("TTS 配音")).toBeInTheDocument();
    expect(screen.getByText("中文字幕")).toBeInTheDocument();
    expect(screen.getByText("Seedance 2.0")).toBeInTheDocument();
    expect(screen.queryByText(/input_snapshot/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "展开技术快照" }));
    expect(screen.getByText(/input_snapshot/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /失败与早期记录.*失败 1/ }));
    fireEvent.click(screen.getByRole("button", { name: /V1.*失败/ }));
    expect(screen.getByText("上游视频生成失败")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "查看生成任务" })).toBeInTheDocument();
    expect(screen.queryByText(/费用|金额|币种|价格/)).not.toBeInTheDocument();
  });

  it("继续修改后通过作品 Agent 更新同一草稿并确认差异", async () => {
    const draft = { ...details.versions[0], id: "draft-3", version_no: 3, status: "draft", source_version_id: "version-2" };
    apiMocks.derive.mockResolvedValue(draft);
    apiMocks.details
      .mockResolvedValueOnce(details)
      .mockResolvedValue({ ...details, current_version_id: draft.id, versions: [draft, ...details.versions] });
    const analyzedDiff = {
      id: "diff-1",
      work_id: details.id,
      source_version_id: "version-2",
      draft_version_id: draft.id,
      plan_version: 1,
      source_fingerprint: "a".repeat(64),
      draft_fingerprint: "b".repeat(64),
      changes: [{ path: "prompt_snapshot.full_prompt", old_value: "旧提示词", new_value: "新提示词" }],
      affected_nodes: ["video_segment:scene-1", "compose"],
      reused_artifact_ids: ["subtitle-2"],
      resource_usage: { video_task_count: 1, video_seconds: 15, tts_characters: 0, asr_seconds: 0 },
      status: "analyzed",
      created_at: "2026-07-22T01:00:00Z",
    };
    apiMocks.createConversation.mockResolvedValue({ conversation_id: "conversation-1" });
    apiMocks.sendMessage.mockResolvedValue({
      user_message: { message_id: "message-user", conversation_id: "conversation-1", role: "user", content: "保留配音，让画面节奏更紧凑", metadata: {}, created_at: "2026-07-22T01:00:00Z" },
      assistant_message: { message_id: "message-assistant", conversation_id: "conversation-1", role: "assistant", content: "已保留配音并收紧画面节奏。", metadata: { draft_version_id: draft.id, version_no: 3, requires_confirmation: true, diff: analyzedDiff }, created_at: "2026-07-22T01:00:01Z" },
      run: { run_id: "agent-run", project_id: project.project_id, agent_type: "work", status: "succeeded", input: {}, output: {}, started_at: "2026-07-22T01:00:00Z" },
    });
    apiMocks.confirmDiff.mockResolvedValue({ run_id: "run-3", diff_plan_id: "diff-1", created: true });
    const onRunCreated = vi.fn();
    render(<WorkLibraryPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} textModels={textModels} writesDisabled={false} onRunCreated={onRunCreated} />);
    fireEvent.click(await screen.findByRole("button", { name: /夏日防晒指南.*查看详情/ }));
    fireEvent.click(await screen.findByRole("button", { name: "继续修改" }));
    await waitFor(() => expect(apiMocks.derive).toHaveBeenCalledWith(expect.anything(), "version-2", {}));

    expect(screen.queryByLabelText("全局提示词")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "保存草稿修改" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "分析版本差异" })).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("作品修改要求"), { target: { value: "保留配音，让画面节奏更紧凑" } });
    fireEvent.click(screen.getByRole("button", { name: "发送修改要求" }));
    await waitFor(() => expect(apiMocks.createConversation).toHaveBeenCalledWith(expect.anything(), expect.objectContaining({
      agent_type: "work", project_id: project.project_id, subject_type: "work", subject_id: details.id,
    })));
    expect(apiMocks.sendMessage).toHaveBeenCalledWith(expect.anything(), "conversation-1", {
      model_id: "model-text", content: "保留配音，让画面节奏更紧凑",
    });
    expect(await screen.findByText("已保留配音并收紧画面节奏。")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "查看影响并确认" }));

    expect(await screen.findByRole("heading", { name: "版本差异确认" })).toBeInTheDocument();
    const dialog = screen.getByRole("dialog", { name: "版本差异确认" });
    expect(within(dialog).getByText("video_segment:scene-1")).toBeInTheDocument();
    expect(within(dialog).getByText(/视频任务 1/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "确认并创建运行" }));
    await waitFor(() => expect(apiMocks.confirmDiff).toHaveBeenCalledWith(expect.anything(), "diff-1", expect.any(String)));
    expect(onRunCreated).toHaveBeenCalledWith("run-3");
  });

  it("Agent 发送失败时保留用户消息和输入内容", async () => {
    const draft = { ...details.versions[0], id: "draft-3", version_no: 3, status: "draft", source_version_id: "version-2" };
    apiMocks.derive.mockResolvedValue(draft);
    apiMocks.details.mockResolvedValueOnce(details).mockResolvedValue({ ...details, current_version_id: draft.id, versions: [draft, ...details.versions] });
    apiMocks.createConversation.mockResolvedValue({ conversation_id: "conversation-1" });
    apiMocks.sendMessage.mockRejectedValue(new Error("模型输出无效"));
    render(<WorkLibraryPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} textModels={textModels} writesDisabled={false} />);
    fireEvent.click(await screen.findByRole("button", { name: /夏日防晒指南.*查看详情/ }));
    fireEvent.click(await screen.findByRole("button", { name: "继续修改" }));
    const input = await screen.findByLabelText("作品修改要求");
    fireEvent.change(input, { target: { value: "保留这条消息" } });
    fireEvent.click(screen.getByRole("button", { name: "发送修改要求" }));
    expect(await screen.findByText("模型输出无效")).toBeInTheDocument();
    expect(screen.getAllByText("保留这条消息").length).toBeGreaterThanOrEqual(2);
    expect(input).toHaveValue("保留这条消息");
  });

  it("完成版本支持显式下载、创建发布计划和作品归档恢复", async () => {
    apiMocks.downloads.mockResolvedValue({
      work_version_id: "version-2",
      artifacts: details.artifacts.map((artifact) => ({ artifact, integrity_status: "available" as const })),
    });
    apiMocks.handoff.mockResolvedValue({ id: "handoff-1", work_id: details.id, work_version_id: "version-2", final_video_artifact_id: "video-2", subtitle_artifact_id: "subtitle-2", status: "draft", payload: {}, created_at: "2026-07-22T01:00:00Z", created: true });
    apiMocks.archive.mockResolvedValue({ id: details.id, title: details.title, status: "archived", archived: true });
    apiMocks.restore.mockResolvedValue({ id: details.id, title: details.title, status: "succeeded", archived: false });
    const onOpenPublicationPlan = vi.fn();
    render(<WorkLibraryPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} onOpenPublicationPlan={onOpenPublicationPlan} />);
    fireEvent.click(await screen.findByRole("button", { name: /夏日防晒指南.*查看详情/ }));

    fireEvent.click(await screen.findByRole("button", { name: "下载" }));
    expect(await screen.findByRole("link", { name: "下载 final-v2.mp4" })).toHaveAttribute("href", "http://api.test/api/work-artifacts/video-2/download");
    expect(screen.getByRole("link", { name: "下载制作包" })).toHaveAttribute("href", "http://api.test/api/work-versions/version-2/production-package");
    fireEvent.click(screen.getByRole("button", { name: "进入发布" }));
    await waitFor(() => expect(apiMocks.handoff).toHaveBeenCalledWith(expect.anything(), "version-2", expect.any(String)));
    await waitFor(() => expect(apiMocks.publicationPlan).toHaveBeenCalledWith(expect.anything(), "handoff-1", expect.any(String)));
    expect(onOpenPublicationPlan).toHaveBeenCalledWith("plan-1");

    fireEvent.click(screen.getByRole("button", { name: "归档作品" }));
    await waitFor(() => expect(apiMocks.archive).toHaveBeenCalledWith(expect.anything(), details.id));
  });

  it("发布计划创建失败时停留在作品详情并显示明确错误", async () => {
    apiMocks.handoff.mockResolvedValue({ id: "handoff-1", work_id: details.id, work_version_id: "version-2", final_video_artifact_id: "video-2", subtitle_artifact_id: "subtitle-2", status: "draft", payload: {}, created_at: "2026-07-22T01:00:00Z", created: true });
    apiMocks.publicationPlan.mockRejectedValue(new Error("发布计划创建失败"));
    const onOpenPublicationPlan = vi.fn();
    render(<WorkLibraryPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} onOpenPublicationPlan={onOpenPublicationPlan} />);
    fireEvent.click(await screen.findByRole("button", { name: /夏日防晒指南.*查看详情/ }));

    fireEvent.click(await screen.findByRole("button", { name: "进入发布" }));

    expect(await screen.findByText("发布计划创建失败")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "夏日防晒指南" })).toBeInTheDocument();
    expect(onOpenPublicationPlan).not.toHaveBeenCalled();
  });
});
