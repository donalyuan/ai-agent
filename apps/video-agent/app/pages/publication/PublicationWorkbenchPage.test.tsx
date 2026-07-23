import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError, createApiClient, type PublicationPlanDetails } from "../../lib/api";
import { PublicationWorkbenchPage } from "./PublicationWorkbenchPage";

const apiMocks = vi.hoisted(() => ({
  list: vi.fn(),
  details: vi.fn(),
  work: vi.fn(),
  save: vi.fn(),
  package: vi.fn(),
  downloads: vi.fn(),
  copyAudit: vi.fn(),
  downloadAudit: vi.fn(),
  handoff: vi.fn(),
  needsAttention: vi.fn(),
  cancel: vi.fn(),
  published: vi.fn(),
  correct: vi.fn(),
}));

vi.mock("../../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/api")>()),
  listPublications: apiMocks.list,
  getPublication: apiMocks.details,
  getWork: apiMocks.work,
  savePublicationTarget: apiMocks.save,
  generatePublicationPackage: apiMocks.package,
  getPublicationDownloads: apiMocks.downloads,
  auditPublicationCopy: apiMocks.copyAudit,
  auditPublicationDownload: apiMocks.downloadAudit,
  handoffPublicationTarget: apiMocks.handoff,
  markPublicationNeedsAttention: apiMocks.needsAttention,
  cancelPublicationTarget: apiMocks.cancel,
  confirmPublicationPublished: apiMocks.published,
  correctPublicationResult: apiMocks.correct,
}));

const douyinTarget = {
  id: "target-douyin",
  publication_plan_id: "plan-1",
  platform: "douyin" as const,
  status: "ready" as const,
  title: "抖音标题",
  body: "抖音正文",
  tags: ["#效率"],
  cover_artifact_id: null,
  planned_at: "2026-07-22T02:00:00.000Z",
  draft_revision: 2,
  handed_off_at: null,
  published_at: null,
  published_url: null,
  result_snapshot: {},
  overdue: true,
  created_at: "2026-07-21T00:00:00.000Z",
  updated_at: "2026-07-22T00:00:00.000Z",
};

const xiaohongshuTarget = {
  ...douyinTarget,
  id: "target-xhs",
  platform: "xiaohongshu" as const,
  status: "draft" as const,
  title: "小红书标题",
  body: "小红书正文",
  tags: ["#创作"],
  planned_at: null,
  draft_revision: 4,
  overdue: false,
};

const plan: PublicationPlanDetails = {
  id: "plan-1",
  handoff_id: "handoff-1",
  work_id: "work-1",
  work_version_id: "version-2",
  final_video_artifact_id: "video-1",
  subtitle_artifact_id: null,
  status: "draft",
  targets: [douyinTarget, xiaohongshuTarget],
  created_at: "2026-07-21T00:00:00.000Z",
  updated_at: "2026-07-22T00:00:00.000Z",
};

describe("PublicationWorkbenchPage", () => {
  beforeEach(() => {
    Object.values(apiMocks).forEach((mock) => mock.mockReset());
    apiMocks.list.mockResolvedValue({ items: [{ ...plan, work_title: "夏日防晒指南" }] });
    apiMocks.details.mockResolvedValue(plan);
    apiMocks.work.mockResolvedValue({ id: "work-1", title: "夏日防晒指南", versions: [], artifacts: [] });
  });

  it("按 URL 中的明确 plan 加载，不静默选择其他计划", async () => {
    render(<PublicationWorkbenchPage client={createApiClient()} planId="plan-1" writesDisabled={false} />);

    expect(await screen.findByRole("heading", { name: "夏日防晒指南" })).toBeInTheDocument();
    expect(apiMocks.details).toHaveBeenCalledWith(expect.anything(), "plan-1");
    expect(screen.getByText("V2")).toBeInTheDocument();
    expect(screen.getByText("计划时间已逾期，状态不会自动改变")).toBeInTheDocument();
  });

  it("双平台草稿独立编辑，保存一个平台不会改写另一个平台", async () => {
    apiMocks.save.mockResolvedValue({ ...douyinTarget, title: "新抖音标题", draft_revision: 3, status: "draft" });
    apiMocks.details.mockResolvedValueOnce(plan).mockResolvedValue({
      ...plan,
      targets: [{ ...douyinTarget, title: "新抖音标题", draft_revision: 3, status: "draft" }, xiaohongshuTarget],
    });
    render(<PublicationWorkbenchPage client={createApiClient()} planId="plan-1" writesDisabled={false} />);
    const douyinPanel = await screen.findByRole("region", { name: "抖音发布目标" });
    const xhsPanel = screen.getByRole("region", { name: "小红书发布目标" });

    fireEvent.change(within(douyinPanel).getByLabelText("平台标题"), { target: { value: "新抖音标题" } });
    fireEvent.click(within(douyinPanel).getByRole("button", { name: "保存草稿" }));

    await waitFor(() => expect(apiMocks.save).toHaveBeenCalledWith(expect.anything(), "plan-1", "douyin", expect.objectContaining({
      expected_revision: 2,
      title: "新抖音标题",
    }), expect.any(String)));
    expect(within(xhsPanel).getByLabelText("平台标题")).toHaveValue("小红书标题");
  });

  it("打开官方入口后仍显示等待人工发布，失败写入不乐观更新", async () => {
    apiMocks.handoff.mockResolvedValue({
      target: { ...douyinTarget, status: "handed_off" },
      official_entrance: "https://creator.douyin.com/",
      publication_confirmation: "manual_required",
    });
    apiMocks.details.mockResolvedValueOnce(plan).mockResolvedValue({
      ...plan,
      targets: [{ ...douyinTarget, status: "handed_off" }, xiaohongshuTarget],
    });
    const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);
    render(<PublicationWorkbenchPage client={createApiClient()} planId="plan-1" writesDisabled={false} />);
    const douyinPanel = await screen.findByRole("region", { name: "抖音发布目标" });

    fireEvent.click(within(douyinPanel).getByRole("button", { name: "去平台发布" }));

    await waitFor(() => expect(openSpy).toHaveBeenCalledWith("https://creator.douyin.com/", "_blank", "noopener,noreferrer"));
    await waitFor(() => expect(screen.getAllByText("等待人工发布").length).toBeGreaterThan(0));
    openSpy.mockRestore();
  });

  it("revision 冲突和 artifact 损坏均保留服务端真实状态并允许安全重试", async () => {
    apiMocks.save.mockRejectedValue(new ApiError(409, "草稿 revision 已过期", { code: "publication_conflict" }));
    apiMocks.package.mockRejectedValue(new ApiError(409, "成片哈希不匹配", { code: "publication_artifact_integrity" }));
    render(<PublicationWorkbenchPage client={createApiClient()} planId="plan-1" writesDisabled={false} />);
    const panel = await screen.findByRole("region", { name: "抖音发布目标" });

    fireEvent.change(within(panel).getByLabelText("平台标题"), { target: { value: "冲突中的本地标题" } });
    fireEvent.click(within(panel).getByRole("button", { name: "保存草稿" }));
    expect(await within(panel).findByText("草稿已被其他操作更新，请重新读取后再试")).toBeInTheDocument();
    expect(within(panel).getByText("准备完成")).toBeInTheDocument();

    fireEvent.click(within(panel).getByRole("button", { name: "生成发布包" }));
    expect(await within(panel).findByText(/artifact 完整性校验失败/)).toBeInTheDocument();
    expect(within(panel).getByText("准备完成")).toBeInTheDocument();
  });

  it("发布记录只展示 published 或 cancelled，并支持平台筛选", async () => {
    const published = { ...douyinTarget, id: "published-douyin", status: "published" as const, published_url: "https://www.douyin.com/video/123", published_at: "2026-07-22T03:00:00Z" };
    apiMocks.list.mockResolvedValue({ items: [
      { ...plan, id: "plan-pending", work_title: "待发布作品" },
      { ...plan, id: "plan-published", work_title: "已发布作品", status: "published", targets: [published] },
    ] });
    render(<PublicationWorkbenchPage client={createApiClient()} planId={null} writesDisabled={false} />);
    await screen.findByText("待发布作品");

    fireEvent.click(screen.getByRole("button", { name: "发布记录" }));
    expect(await screen.findByText("已发布作品")).toBeInTheDocument();
    expect(screen.queryByText("待发布作品")).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("平台筛选"), { target: { value: "xiaohongshu" } });
    expect(screen.getByText("没有匹配记录")).toBeInTheDocument();
  });

  it("人工结果必须是目标平台 HTTPS 官方链接和已发生时间", async () => {
    const handedOff = { ...douyinTarget, status: "handed_off" as const };
    apiMocks.details.mockResolvedValue({ ...plan, targets: [handedOff, xiaohongshuTarget] });
    render(<PublicationWorkbenchPage client={createApiClient()} planId="plan-1" writesDisabled={false} />);
    const panel = await screen.findByRole("region", { name: "抖音发布目标" });
    fireEvent.change(within(panel).getByLabelText("官方作品链接"), { target: { value: "https://example.com/video/1" } });
    fireEvent.change(within(panel).getByLabelText("实际发布时间"), { target: { value: "2026-07-22T03:00" } });

    fireEvent.click(within(panel).getByRole("button", { name: "人工确认已发布" }));

    expect(await within(panel).findByText(/请输入\s*抖音 HTTPS 官方作品链接/)).toBeInTheDocument();
    expect(apiMocks.published).not.toHaveBeenCalled();
    expect(within(panel).getAllByText("等待人工发布").length).toBeGreaterThan(0);
  });
});
