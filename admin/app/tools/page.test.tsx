import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { createApiClient } from "../lib/api";
import { TosStagingToolPage } from "./TosStagingToolPage";

const unconfigured = {
  configured: false,
  config_id: null,
  version: null,
  enabled: false,
  storage_provider: null,
  endpoint: null,
  region: null,
  bucket: null,
  object_prefix: null,
  access_key_masked: null,
  secret_key_masked: null,
  access_key_configured: false,
  secret_key_configured: false,
  signed_url_ttl_seconds: null,
  max_file_bytes: null,
  max_audio_duration_seconds: null,
  pending_cleanup_count: 0,
  last_check_status: "never",
  last_check_requested_at: null,
  last_checked_at: null,
  last_check_error_summary: null,
  created_at: null,
  updated_at: null,
} as const;

function jsonResponse(body: unknown, status = 200) {
  return Promise.resolve(new Response(JSON.stringify(body), { status }));
}

describe("私有 TOS 工具页面", () => {
  it("首次保存独立系统配置并发起真实连接检查队列", async () => {
    let current: Record<string, unknown> = unconfigured;
    let savedPayload: Record<string, unknown> | null = null;
    const fetcher = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const path = new URL(String(input)).pathname;
      if (path.endsWith("/check") && init?.method === "POST") {
        current = {
          ...current,
          last_check_status: "queued",
          last_check_requested_at: "2026-07-16T08:00:00Z",
        };
        return jsonResponse(current, 202);
      }
      if (init?.method === "PUT") {
        savedPayload = JSON.parse(String(init.body));
        current = {
          ...unconfigured,
          ...savedPayload,
          configured: true,
          config_id: "66666666-6666-4666-8666-666666666666",
          version: 1,
          access_key_masked: "tos-****1234",
          secret_key_masked: "tos-****5678",
          access_key_configured: true,
          secret_key_configured: true,
          access_key: undefined,
          secret_key: undefined,
        };
        return jsonResponse(current);
      }
      return jsonResponse(current);
    });

    render(
      <TosStagingToolPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );

    expect(await screen.findByRole("heading", { name: "私有 TOS" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "工具与 MCP" })).toHaveClass("active");
    expect(screen.getByRole("button", { name: "检查连接" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Bucket"), {
      target: { value: "novex-private-staging" },
    });
    fireEvent.change(screen.getByLabelText("Access Key"), {
      target: { value: "tos-access-1234" },
    });
    fireEvent.change(screen.getByLabelText("Secret Key"), {
      target: { value: "tos-secret-5678" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存配置" }));

    expect(await screen.findByText("系统 TOS 已保存为版本 1")).toBeInTheDocument();
    expect(savedPayload).toMatchObject({
      version: null,
      enabled: false,
      storage_provider: "volcengine_tos",
      endpoint: "https://tos-cn-beijing.volces.com",
      region: "cn-beijing",
      bucket: "novex-private-staging",
      object_prefix: "novex/asr",
      access_key: "tos-access-1234",
      secret_key: "tos-secret-5678",
    });

    fireEvent.click(screen.getByRole("button", { name: "检查连接" }));
    expect(await screen.findByText("TOS Bucket 连接检查已进入队列")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText("待检查")).toBeInTheDocument());
    const checkCall = fetcher.mock.calls.find(([input]) => String(input).endsWith("/check"));
    expect(checkCall?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({ version: 1 }),
    });
  });

  it("待清理对象存在时禁用配置保存但保留状态展示", async () => {
    const fetcher = vi.fn(() => jsonResponse({
      ...unconfigured,
      configured: true,
      config_id: "66666666-6666-4666-8666-666666666666",
      version: 3,
      enabled: true,
      bucket: "private-bucket",
      object_prefix: "novex/asr",
      endpoint: "https://tos-cn-beijing.volces.com",
      region: "cn-beijing",
      access_key_configured: true,
      secret_key_configured: true,
      access_key_masked: "tos-****1234",
      secret_key_masked: "tos-****5678",
      signed_url_ttl_seconds: 600,
      max_file_bytes: 104857600,
      max_audio_duration_seconds: 7200,
      pending_cleanup_count: 2,
    }));

    render(
      <TosStagingToolPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );

    expect(await screen.findByText("当前有 2 个临时对象待清理，配置修改与停用已锁定。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存配置" })).toBeDisabled();
  });
});
