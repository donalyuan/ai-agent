import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { createApiClient } from "../lib/api";
import { ModelManagementPage } from "./ModelManagementPage";

const textModel = {
  model_id: "11111111-1111-4111-8111-111111111111",
  display_name: "GPT Text",
  model_type: "text",
  provider_name: "OpenAI",
  api_protocol: "openai_responses",
  protocol_version: "v1",
  auth_scheme: "bearer",
  request_base_url: "https://api.example/v1",
  upstream_model: "gpt-test",
  api_key_masked: "secr****-key",
  api_secret_masked: null,
  api_key_configured: true,
  api_secret_configured: false,
  timeout_seconds: 120,
  reasoning_effort: "high",
  max_output_tokens: 3000,
  settings: {},
  sort_order: 0,
  remark: "",
  status: "enabled",
  is_default: true,
  last_call_status: "never",
  last_call_at: null,
  last_error_summary: null,
  source: "admin",
  version: 2,
  deleted_at: null,
  created_at: "2026-07-11T00:00:00Z",
  updated_at: "2026-07-11T00:00:00Z",
} as const;

function jsonResponse(body: unknown) {
  return Promise.resolve(new Response(JSON.stringify(body), { status: 200 }));
}

describe("AI 模型管理页面", () => {
  it("展示筛选表格并打开类型化添加抽屉", async () => {
    const fetcher = vi.fn(() => jsonResponse({ models: [textModel] }));
    const { container } = render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );

    expect(await screen.findByText("GPT Text")).toBeInTheDocument();
    const layout = container.querySelector(".modelManagementLayout");
    expect(layout).toBeInTheDocument();
    expect(layout).toContainElement(container.querySelector(".modelHeader"));
    expect(layout).toContainElement(container.querySelector(".modelTabs"));
    expect(layout).toContainElement(container.querySelector(".modelToolbar"));
    expect(layout).toContainElement(container.querySelector(".modelTableWrap"));
    expect(screen.getByRole("link", { name: "模型与路由" })).toHaveClass("active");
    expect(screen.getByRole("link", { name: "用户与权限" })).toHaveAttribute("href", "/#用户与权限");
    for (const tab of ["文本模型", "图片模型", "视频模型"]) {
      expect(screen.getByRole("button", { name: tab })).toBeInTheDocument();
    }
    for (const heading of ["模型名称", "类型", "供应商 / API 协议", "请求地址", "默认", "状态", "最近调用", "更新时间", "操作"]) {
      expect(screen.getByRole("columnheader", { name: heading })).toBeInTheDocument();
    }

    fireEvent.click(screen.getByRole("button", { name: "添加模型" }));
    expect(screen.getByRole("dialog", { name: "添加 AI 模型" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("模型类型"), { target: { value: "image" } });
    expect(screen.getByLabelText("默认图片尺寸")).toBeInTheDocument();
    expect(screen.queryByLabelText("推理等级")).not.toBeInTheDocument();
  });

  it("编辑凭据留空并为默认停用展示替代确认", async () => {
    const replacement = { ...textModel, model_id: "22222222-2222-4222-8222-222222222222", display_name: "GPT Backup", is_default: false };
    const fetcher = vi.fn(() => jsonResponse({ models: [textModel, replacement] }));
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    await screen.findByText("GPT Text");

    fireEvent.click(screen.getAllByRole("button", { name: "编辑" })[0]);
    const apiKey = screen.getByLabelText("API Key") as HTMLInputElement;
    expect(apiKey.value).toBe("");
    expect(screen.getByText("已配置：secr****-key")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "关闭" }));

    fireEvent.click(screen.getAllByRole("button", { name: "停用" })[0]);
    expect(screen.getByRole("dialog", { name: "停用默认模型" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "GPT Backup" })).toBeInTheDocument();
  });

  it("无模型时展示空状态", async () => {
    const fetcher = vi.fn(() => jsonResponse({ models: [] }));
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );

    expect(await screen.findByText("当前类型暂无模型")).toBeInTheDocument();
  });
});
