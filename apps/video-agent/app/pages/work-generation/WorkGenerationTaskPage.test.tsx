import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Material, Project, WorkGenerationTask, WorkGenerationTaskDetails } from "../../lib/api";
import { createApiClient } from "../../lib/api";
import { WorkGenerationTaskPage } from "./WorkGenerationTaskPage";

const apiMocks = vi.hoisted(() => ({
  list: vi.fn(),
  details: vi.fn(),
  material: vi.fn(),
  cancel: vi.fn(),
  dismiss: vi.fn(),
  retry: vi.fn(),
}));

const emptyCounts = { pending: 0, running: 0, completed: 0, attention: 0, cancelled: 0, total: 0 };

vi.mock("../../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/api")>()),
  listWorkGenerationTasks: apiMocks.list,
  getWorkGenerationTask: apiMocks.details,
  getMaterial: apiMocks.material,
  cancelWorkGenerationRun: apiMocks.cancel,
  dismissWorkGenerationRun: apiMocks.dismiss,
  retryWorkGenerationStep: apiMocks.retry,
}));

const project: Project = {
  project_id: "11111111-1111-4111-8111-111111111111",
  name: "测试项目",
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
  created_at: "2026-07-20T00:00:00Z",
  updated_at: "2026-07-20T00:00:00Z",
};

function task(overrides: Partial<WorkGenerationTask> = {}): WorkGenerationTask {
  return {
    id: "22222222-2222-4222-8222-222222222222",
    work_id: "33333333-3333-4333-8333-333333333333",
    work_version_id: "44444444-4444-4444-8444-444444444444",
    work_plan_id: "55555555-5555-4555-8555-555555555555",
    title: "夏日防晒指南",
    version_no: 2,
    status: "running",
    current_stage: "video_segment",
    progress_percent: 40,
    successful_steps: 2,
    running_steps: 1,
    queued_steps: 2,
    failed_steps: 0,
    can_cancel: false,
    cancel_mode: "provider",
    cancel_block_reason: "当前 provider 不支持运行中取消，任务仍需等待上游终态",
    resource_usage: { video_seconds: 30 },
    error_category: null,
    error_summary: null,
    created_at: "2026-07-20T01:00:00Z",
    updated_at: "2026-07-20T01:01:00Z",
    dismissed_at: null,
    ...overrides,
  };
}

function details(value: WorkGenerationTask): WorkGenerationTaskDetails {
  return {
    task: value,
    steps: [{
      id: "66666666-6666-4666-8666-666666666666",
      step_no: 1,
      step_type: "video_segment",
      status: "running",
      is_required: true,
      depends_on: [],
      model_snapshot: { display_name: "Fake Video" },
      resource_usage: { video_seconds: 15 },
      result_material_ids: [],
      external_task_id: "fake-upstream",
      error_category: null,
      error_code: null,
      error_summary: null,
      attempts: [],
    }],
  };
}

describe("WorkGenerationTaskPage", () => {
  beforeEach(() => {
    apiMocks.list.mockReset();
    apiMocks.details.mockReset();
    apiMocks.material.mockReset();
    apiMocks.cancel.mockReset();
    apiMocks.dismiss.mockReset();
    apiMocks.retry.mockReset();
    window.history.replaceState({}, "", "/production/tasks");
  });

  afterEach(() => vi.restoreAllMocks());

  it("普通入口点击查看后才加载并打开对应任务详情", async () => {
    const current = task();
    let resolveDetails: ((value: WorkGenerationTaskDetails) => void) | undefined;
    apiMocks.list.mockResolvedValue({ tasks: [current], counts: { ...emptyCounts, running: 1, total: 1 } });
    apiMocks.details.mockImplementation(() => new Promise((resolve) => { resolveDetails = resolve; }));

    render(<WorkGenerationTaskPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} />);

    const row = await screen.findByRole("button", { name: new RegExp(`${current.title}.*查看`) });
    expect(apiMocks.details).not.toHaveBeenCalled();
    expect(screen.getByText("选择一个运行")).toBeInTheDocument();

    fireEvent.click(row);

    expect(screen.getByText("正在读取任务详情")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: new RegExp(`${current.title}.*查看中`) })).toHaveAttribute("aria-pressed", "true");
    expect(window.location.search).toBe(`?run_id=${current.id}`);

    await act(async () => resolveDetails?.(details(current)));

    expect(await screen.findByRole("heading", { name: `${current.title} · V${current.version_no}` })).toBeInTheDocument();
    expect(screen.getByText("执行步骤与调用审计")).toBeInTheDocument();
  });

  it("连续点击任务时忽略较早返回的过期详情", async () => {
    const first = task({ id: "first-run", title: "第一条作品" });
    const second = task({ id: "second-run", title: "第二条作品", version_no: 3 });
    const resolvers = new Map<string, (value: WorkGenerationTaskDetails) => void>();
    apiMocks.list.mockResolvedValue({ tasks: [first, second], counts: { ...emptyCounts, running: 2, total: 2 } });
    apiMocks.details.mockImplementation((_client, runId: string) => new Promise((resolve) => { resolvers.set(runId, resolve); }));

    render(<WorkGenerationTaskPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} />);

    fireEvent.click(await screen.findByRole("button", { name: /第一条作品.*查看/ }));
    fireEvent.click(screen.getByRole("button", { name: /第二条作品.*查看/ }));
    await act(async () => resolvers.get(second.id)?.(details(second)));
    expect(await screen.findByRole("heading", { name: "第二条作品 · V3" })).toBeInTheDocument();

    await act(async () => resolvers.get(first.id)?.(details(first)));
    expect(screen.queryByRole("heading", { name: "第一条作品 · V2" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "第二条作品 · V3" })).toBeInTheDocument();
  });

  it("运行中 provider 不支持取消时展示明确原因且不伪造取消入口", async () => {
    const current = task();
    apiMocks.list.mockResolvedValue({ tasks: [current], counts: { ...emptyCounts, running: 1, total: 1 } });
    apiMocks.details.mockResolvedValue(details(current));

    render(<WorkGenerationTaskPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} />);
    fireEvent.click(await screen.findByRole("button", { name: new RegExp(`${current.title}.*查看`) }));

    expect(await screen.findByText(current.cancel_block_reason!)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "取消运行" })).not.toBeInTheDocument();
  });

  it("运行中 provider 支持取消时确认后调用原运行取消 API", async () => {
    const current = task({ can_cancel: true, cancel_block_reason: null });
    const cancelling = task({ status: "cancelling", can_cancel: false, cancel_block_reason: null });
    apiMocks.list.mockResolvedValue({ tasks: [current], counts: { ...emptyCounts, running: 1, total: 1 } });
    apiMocks.details.mockResolvedValue(details(current));
    apiMocks.cancel.mockResolvedValue(details(cancelling));
    vi.spyOn(window, "confirm").mockReturnValue(true);

    render(<WorkGenerationTaskPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} />);
    fireEvent.click(await screen.findByRole("button", { name: new RegExp(`${current.title}.*查看`) }));
    fireEvent.click(await screen.findByRole("button", { name: "取消运行" }));

    await waitFor(() => expect(apiMocks.cancel).toHaveBeenCalledWith(expect.anything(), current.id));
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining("provider"));
  });

  it("失败节点重试确认展示用量、必要下游和复用成功素材", async () => {
    const failedTask = task({ status: "failed", failed_steps: 1, running_steps: 0 });
    const failedDetails = details(failedTask);
    failedDetails.steps = [
      {
        ...failedDetails.steps[0],
        id: "success-step",
        status: "succeeded",
        result_material_ids: ["material-1"],
      },
      {
        ...failedDetails.steps[0],
        id: "failed-step",
        step_no: 2,
        status: "failed",
        resource_usage: { video_seconds: 15 },
      },
      {
        ...failedDetails.steps[0],
        id: "compose-step",
        step_no: 3,
        step_type: "compose",
        status: "blocked",
        depends_on: ["failed-step"],
      },
    ];
    apiMocks.list.mockResolvedValue({ tasks: [failedTask], counts: { ...emptyCounts, attention: 1, total: 1 } });
    apiMocks.details.mockResolvedValue(failedDetails);
    apiMocks.retry.mockResolvedValue({ id: "retry-attempt", attempt_no: 2, status: "queued" });
    vi.spyOn(window, "confirm").mockReturnValue(true);

    render(<WorkGenerationTaskPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} />);
    fireEvent.click(await screen.findByRole("button", { name: new RegExp(`${failedTask.title}.*查看`) }));
    await screen.findByText("失败", { selector: ".workGenerationStepStatus" });
    fireEvent.click(screen.getAllByRole("button", { name: /Seedance 分段/ }).at(-1)!);
    fireEvent.click(await screen.findByRole("button", { name: "重试失败节点" }));

    await waitFor(() => expect(apiMocks.retry).toHaveBeenCalledWith(expect.anything(), "failed-step", expect.any(String)));
    const confirmation = vi.mocked(window.confirm).mock.calls[0][0] as string;
    expect(confirmation).toContain("视频 15");
    expect(confirmation).toContain("必要下游：1 个步骤");
    expect(confirmation).toContain("继续复用：1 个成功素材");
    expect(confirmation).not.toMatch(/费用|价格|金额/);
  });

  it("当前视图为空时使用服务端全局计数并可进入已取消视图", async () => {
    apiMocks.list.mockResolvedValue({
      tasks: [],
      counts: { ...emptyCounts, completed: 1, cancelled: 1, total: 2 },
    });

    render(<WorkGenerationTaskPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} />);

    expect(await screen.findByText("共 2 个任务")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "已完成 1" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /更多筛选/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "特殊状态" }), { target: { value: "cancelled" } });

    await waitFor(() => expect(apiMocks.list).toHaveBeenLastCalledWith(
      expect.anything(),
      project.project_id,
      expect.objectContaining({ view: "cancelled" }),
    ));
  });

  it("已完成任务展示已登记成品并提供素材库入口", async () => {
    const completed = task({ status: "succeeded", current_stage: "completed", successful_steps: 7, running_steps: 0, progress_percent: 100 });
    const completedDetails = details(completed);
    completedDetails.steps = [{
      ...completedDetails.steps[0],
      step_type: "compose",
      status: "succeeded",
      result_material_ids: ["material-final"],
    }];
    const material = {
      material_id: "material-final",
      project_id: project.project_id,
      material_type: "video",
      file_url: "http://api.test/assets/generated/final.mp4",
      thumbnail_url: null,
      file_name: "夏日防晒指南 成片.mp4",
      tags: ["作品生成", "成片"],
      metadata: {},
      source: "work_generation",
      audio_usage: null,
      work_id: completed.work_id,
      work_version_id: completed.work_version_id,
      generation: null,
      usage_count: 0,
      status: "active",
      created_at: completed.created_at,
      updated_at: completed.updated_at,
    } as Material;
    apiMocks.list.mockResolvedValue({ tasks: [completed], counts: { ...emptyCounts, completed: 1, total: 1 } });
    apiMocks.details.mockResolvedValue(completedDetails);
    apiMocks.material.mockResolvedValue(material);
    const onOpenMaterialLibrary = vi.fn();

    render(<WorkGenerationTaskPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} onOpenMaterialLibrary={onOpenMaterialLibrary} />);
    fireEvent.click(await screen.findByRole("button", { name: new RegExp(`${completed.title}.*查看`) }));

    expect(await screen.findByText("生成成品")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "在素材库查看" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "在素材库查看" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "在素材库查看" }));
    expect(onOpenMaterialLibrary).toHaveBeenCalledTimes(1);
  });

  it("run_id 只用于首次定位，后续轮询严格回到任务所属状态视图", async () => {
    const completed = task({ id: "completed-run", title: "已完成作品", status: "succeeded", current_stage: "completed", successful_steps: 6, running_steps: 0 });
    const cancelled = task({ id: "cancelled-run", title: "已取消作品", status: "cancelled", current_stage: "cancelled", running_steps: 0 });
    const counts = { ...emptyCounts, completed: 1, cancelled: 1, total: 2 };
    window.history.replaceState({}, "", `/production/tasks?run_id=${completed.id}`);
    apiMocks.list.mockImplementation((_client, _projectId, filters) => Promise.resolve({
      tasks: filters.view === "completed" ? [completed] : filters.view === "cancelled" ? [cancelled] : [],
      counts,
    }));
    apiMocks.details.mockImplementation((_client, runId) => Promise.resolve(details(runId === completed.id ? completed : cancelled)));

    render(<WorkGenerationTaskPage client={createApiClient({ baseUrl: "http://api.test" })} project={project} writesDisabled={false} />);

    await waitFor(() => expect(apiMocks.list).toHaveBeenCalledWith(
      expect.anything(),
      project.project_id,
      expect.objectContaining({ view: "completed" }),
    ));
    expect(screen.getByText("已完成作品")).toBeInTheDocument();
    expect(screen.queryByText("已取消作品")).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "已完成 1" })).toHaveAttribute("aria-selected", "true");
    expect(apiMocks.list).not.toHaveBeenCalledWith(expect.anything(), project.project_id, expect.objectContaining({ view: "all" }));
  });
});
