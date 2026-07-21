import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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

const ttsModel = {
  ...textModel,
  model_id: "33333333-3333-4333-8333-333333333333",
  display_name: "豆包 TTS 2.0",
  model_type: "speech",
  provider_name: "火山引擎",
  api_protocol: "volcengine_tts_v3",
  protocol_version: "v3",
  auth_scheme: "api_key",
  request_base_url: "https://openspeech.bytedance.com/api/v3",
  upstream_model: "doubao-seed-tts-2.0",
  catalog_access_key_masked: "cata****1234",
  catalog_secret_key_masked: "cata****5678",
  catalog_access_key_configured: true,
  catalog_secret_key_configured: true,
  voice_catalog_mode: "official_sync",
  voice_catalog_source_model_id: null,
  voice_catalog_source_display_name: null,
  reasoning_effort: null,
  max_output_tokens: null,
  settings: {
    resource_id: "seed-tts-2.0",
    supported_audio_formats: ["mp3", "wav"],
    default_audio_format: "mp3",
    supported_sample_rates: [24000],
    default_sample_rate: 24000,
    max_input_characters: 3000,
    max_audio_duration_seconds: null,
    supports_word_timestamps: true,
    word_timestamp_languages: ["zh-cn", "en-us"],
    catalog_sync_interval_minutes: 1440,
    parameters: {},
  },
  is_default: true,
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
    for (const tab of ["文本模型", "图片模型", "视频模型", "语音模型"]) {
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

  it("按模型类型限制 API 协议和凭据", async () => {
    const fetcher = vi.fn(() => jsonResponse({ models: [] }));
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    await screen.findByText("当前类型暂无模型");

    fireEvent.click(screen.getByRole("button", { name: "添加模型" }));
    const dialog = screen.getByRole("dialog", { name: "添加 AI 模型" });
    const modelType = within(dialog).getByLabelText("模型类型");
    const protocolNames = () => within(
      within(dialog).getByLabelText("API 调用协议"),
    ).getAllByRole("option").map((option) => option.textContent);

    expect(protocolNames()).toEqual([
      "OpenAI Responses",
      "OpenAI Chat Completions",
    ]);

    fireEvent.change(modelType, { target: { value: "image" } });
    expect(protocolNames()).toEqual(["OpenAI Images", "火山方舟图片生成"]);
    const protocol = within(dialog).getByLabelText("API 调用协议");
    fireEvent.change(protocol, { target: { value: "volcengine_ark_images" } });
    expect(within(dialog).queryByLabelText("API Secret")).not.toBeInTheDocument();
    expect(within(dialog).getByLabelText("单次最大图片数")).toHaveValue(1);
    expect(within(dialog).getByLabelText("单次最大图片数")).toBeDisabled();

    fireEvent.change(modelType, { target: { value: "video" } });
    expect(protocolNames()).toEqual(["火山方舟 Seedance", "Runway API", "可灵 API"]);

    fireEvent.change(modelType, { target: { value: "speech" } });
    expect(protocolNames()).toEqual([
      "豆包 TTS V3",
      "OpenAI Audio Speech（中转）",
      "豆包 ASR V3",
    ]);
    expect(within(dialog).getByLabelText("TTS X-Api-Key")).toBeInTheDocument();
    expect(within(dialog).queryByLabelText("API Secret")).not.toBeInTheDocument();
  });

  it("提交 TTS 目录凭据和版本化声音能力", async () => {
    const fetcher = vi.fn(
      (_input: RequestInfo | URL, _init?: RequestInit) => jsonResponse({ models: [] }),
    );
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    await screen.findByText("当前类型暂无模型");

    fireEvent.click(screen.getByRole("button", { name: "添加模型" }));
    const dialog = screen.getByRole("dialog", { name: "添加 AI 模型" });
    fireEvent.change(within(dialog).getByLabelText("模型类型"), {
      target: { value: "speech" },
    });
    for (const [label, value] of [
      ["显示名称", "豆包 TTS 2.0"],
      ["供应商", "火山引擎"],
      ["上游模型标识", "doubao-seed-tts-2.0"],
      ["请求地址", "https://openspeech.bytedance.com/api/v3"],
      ["TTS X-Api-Key", "speech-key"],
      ["OpenAPI Access Key（AK）", "catalog-access"],
      ["OpenAPI Secret Key（SK）", "catalog-secret"],
    ] as const) {
      fireEvent.change(within(dialog).getByLabelText(label), { target: { value } });
    }
    expect(within(dialog).getByText(
      /OpenAPI AK\/SK 仅用于 ListSpeakers HMAC 签名，不会进入请求体/,
    )).toBeInTheDocument();
    const timestampLanguageTrigger = within(dialog).getByRole("button", { name: "时间戳语言" });
    expect(timestampLanguageTrigger).toHaveAttribute("aria-expanded", "false");
    expect(timestampLanguageTrigger).toHaveTextContent("简体中文、美式英语");
    expect(within(dialog).queryByRole("checkbox", { name: "简体中文" })).not.toBeInTheDocument();

    fireEvent.click(timestampLanguageTrigger);
    expect(timestampLanguageTrigger).toHaveAttribute("aria-expanded", "true");
    const languageSearch = within(dialog).getByRole("searchbox", { name: "搜索时间戳语言" });
    fireEvent.change(languageSearch, { target: { value: "美式" } });
    expect(within(dialog).queryByRole("checkbox", { name: "简体中文" })).not.toBeInTheDocument();
    expect(within(dialog).getByRole("checkbox", { name: "美式英语" })).toBeInTheDocument();
    fireEvent.change(languageSearch, { target: { value: "" } });

    const simplifiedChinese = within(dialog).getByRole("checkbox", { name: "简体中文" });
    const americanEnglish = within(dialog).getByRole("checkbox", { name: "美式英语" });
    expect(simplifiedChinese).toBeChecked();
    expect(americanEnglish).toBeChecked();
    fireEvent.click(americanEnglish);
    expect(americanEnglish).not.toBeChecked();
    expect(simplifiedChinese).toBeDisabled();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(timestampLanguageTrigger).toHaveAttribute("aria-expanded", "false");
    expect(timestampLanguageTrigger).toHaveTextContent("简体中文");
    expect(within(dialog).queryByRole("searchbox", { name: "搜索时间戳语言" })).not.toBeInTheDocument();

    fireEvent.click(timestampLanguageTrigger);
    fireEvent.pointerDown(document.body);
    expect(timestampLanguageTrigger).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(within(dialog).getByRole("button", { name: "保存模型" }));

    await waitFor(() => {
      const createCall = fetcher.mock.calls.find(([, init]) => init?.method === "POST");
      expect(createCall).toBeDefined();
      const payload = JSON.parse(String(createCall?.[1]?.body));
      expect(payload).toMatchObject({
        model_type: "speech",
        api_protocol: "volcengine_tts_v3",
        auth_scheme: "api_key",
        catalog_access_key: "catalog-access",
        catalog_secret_key: "catalog-secret",
        voice_catalog_mode: "official_sync",
        voice_catalog_source_model_id: null,
      });
      expect(payload).not.toHaveProperty("staging_config");
      expect(payload.settings).toMatchObject({
        resource_id: "seed-tts-2.0",
        max_input_characters: 3000,
        max_audio_duration_seconds: null,
        supports_word_timestamps: true,
        word_timestamp_languages: ["zh-cn"],
        catalog_sync_interval_minutes: 1440,
      });
    });
  });

  it("切换 ASR 时清除 TTS 配置并只读引用系统 TOS", async () => {
    const fetcher = vi.fn((input: RequestInfo | URL, _init?: RequestInit) => {
      if (String(input).endsWith("/api/tools/tos-staging")) {
        return jsonResponse({
          configured: true,
          version: 4,
          enabled: true,
          pending_cleanup_count: 0,
          last_check_status: "succeeded",
        });
      }
      return jsonResponse({ models: [] });
    });
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    await screen.findByText("当前类型暂无模型");

    fireEvent.click(screen.getByRole("button", { name: "添加模型" }));
    const dialog = screen.getByRole("dialog", { name: "添加 AI 模型" });
    fireEvent.change(within(dialog).getByLabelText("模型类型"), { target: { value: "speech" } });
    fireEvent.change(within(dialog).getByLabelText("API 调用协议"), {
      target: { value: "volcengine_asr_v3" },
    });
    expect(within(dialog).queryByLabelText("OpenAPI Access Key（AK）")).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("checkbox", { name: "简体中文" })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("checkbox", { name: "美式英语" })).not.toBeInTheDocument();
    expect(within(dialog).getByLabelText("时间戳语言")).toHaveTextContent("自动识别（全部语言）");
    expect(await within(dialog).findByLabelText("系统私有 TOS 状态")).toHaveTextContent("已配置并启用");
    expect(within(dialog).getByRole("link", { name: "前往工具与 MCP 配置" })).toHaveAttribute("href", "/tools");
    expect(within(dialog).queryByLabelText("TOS Endpoint")).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText("TOS Access Key")).not.toBeInTheDocument();
    for (const [label, value] of [
      ["显示名称", "豆包 ASR 2.0"],
      ["供应商", "火山引擎"],
      ["上游模型标识", "doubao-seed-asr-2.0"],
      ["请求地址", "https://openspeech.bytedance.com/api/v3"],
      ["ASR X-Api-Key", "speech-key"],
    ] as const) {
      fireEvent.change(within(dialog).getByLabelText(label), { target: { value } });
    }
    fireEvent.click(within(dialog).getByRole("button", { name: "保存模型" }));

    await waitFor(() => {
      const createCall = fetcher.mock.calls.find(([, init]) => init?.method === "POST");
      const payload = JSON.parse(String(createCall?.[1]?.body));
      expect(payload.catalog_access_key).toBeNull();
      expect(payload.catalog_secret_key).toBeNull();
      expect(payload.voice_catalog_mode).toBe("official_sync");
      expect(payload.voice_catalog_source_model_id).toBeNull();
      expect(payload.settings).toMatchObject({
        resource_id: "volc.seedasr.auc",
        max_input_characters: null,
        max_audio_duration_seconds: 7200,
        catalog_sync_interval_minutes: null,
      });
      expect(payload).not.toHaveProperty("staging_config");
    });
  });

  it("中转 TTS 复用同上游官方目录且不提交 OpenAPI AK/SK", async () => {
    const fetcher = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.endsWith(`/api/speech/models/${ttsModel.model_id}/voice-catalog`)) {
        return jsonResponse({
          model_id: ttsModel.model_id,
          source_model_id: ttsModel.model_id,
          model_settings: ttsModel.settings,
          last_sync: null,
          voices: [],
        });
      }
      if (url.endsWith("/api/admin/models") && init?.method === "POST") {
        return jsonResponse({ ...ttsModel, display_name: "Seed TTS 中转" });
      }
      return jsonResponse({ models: url.includes("type=speech") ? [ttsModel] : [] });
    });
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "语音模型" }));
    expect(await screen.findByText("豆包 TTS 2.0")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "添加模型" }));
    const dialog = screen.getByRole("dialog", { name: "添加 AI 模型" });
    for (const [label, value] of [
      ["显示名称", "Seed TTS 中转"],
      ["供应商", "中转服务"],
      ["请求地址", "https://speech-gateway.example.com/api/v3"],
      ["TTS X-Api-Key", "gateway-key"],
    ] as const) {
      fireEvent.change(within(dialog).getByLabelText(label), { target: { value } });
    }

    fireEvent.click(within(dialog).getByRole("radio", { name: "复用已有目录" }));
    expect(within(dialog).queryByLabelText("OpenAPI Access Key（AK）")).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText("OpenAPI Secret Key（SK）")).not.toBeInTheDocument();
    await waitFor(() => {
      expect(within(dialog).getByLabelText("目录来源模型")).toHaveValue(ttsModel.model_id);
    });
    expect(within(dialog).getByRole("option", { name: "豆包 TTS 2.0" })).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "保存模型" }));

    await waitFor(() => {
      const createCall = fetcher.mock.calls.find(([input, init]) =>
        String(input).endsWith("/api/admin/models") && init?.method === "POST",
      );
      const payload = JSON.parse(String(createCall?.[1]?.body));
      expect(payload).toMatchObject({
        display_name: "Seed TTS 中转",
        upstream_model: "doubao-seed-tts-2.0",
        voice_catalog_mode: "shared",
        voice_catalog_source_model_id: ttsModel.model_id,
        catalog_access_key: null,
        catalog_secret_key: null,
      });
    });
  });

  it("OpenAI Audio Speech 中转强制复用官方目录并提交 Bearer 配置", async () => {
    const fetcher = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.endsWith(`/api/speech/models/${ttsModel.model_id}/voice-catalog`)) {
        return jsonResponse({
          model_id: ttsModel.model_id,
          source_model_id: ttsModel.model_id,
          model_settings: ttsModel.settings,
          last_sync: null,
          voices: [],
        });
      }
      if (url.endsWith("/api/admin/models") && init?.method === "POST") {
        return jsonResponse({
          ...ttsModel,
          display_name: "ZeekAI Seed TTS",
          api_protocol: "openai_audio_speech",
          auth_scheme: "bearer",
        });
      }
      return jsonResponse({ models: url.includes("type=speech") ? [ttsModel] : [] });
    });
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    expect(await screen.findByText("当前类型暂无模型")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "添加模型" }));
    const dialog = screen.getByRole("dialog", { name: "添加 AI 模型" });
    fireEvent.change(within(dialog).getByLabelText("模型类型"), {
      target: { value: "speech" },
    });
    fireEvent.change(within(dialog).getByLabelText("API 调用协议"), {
      target: { value: "openai_audio_speech" },
    });
    for (const [label, value] of [
      ["显示名称", "ZeekAI Seed TTS"],
      ["供应商", "ZeekAI"],
      ["请求地址", "https://api.zeekai-cn.cc/v1"],
      ["Bearer API Key", "gateway-key"],
    ] as const) {
      fireEvent.change(within(dialog).getByLabelText(label), { target: { value } });
    }

    expect(within(dialog).queryByRole("radio", { name: "官方同步" })).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText("OpenAPI Access Key（AK）")).not.toBeInTheDocument();
    await waitFor(() => {
      expect(within(dialog).getByLabelText("目录来源模型")).toHaveValue(ttsModel.model_id);
    });
    expect(fetcher.mock.calls.some(([input]) => (
      String(input) === "http://api.test/api/admin/models?type=speech&status=enabled"
    ))).toBe(true);
    expect(within(dialog).getByLabelText("时间戳语言")).toHaveTextContent("不支持（仅生成配音）");
    expect(within(dialog).getByRole("checkbox", { name: "支持字词时间戳" })).not.toBeChecked();
    expect(dialog.querySelector("form")).toBeValid();
    fireEvent.click(within(dialog).getByRole("button", { name: "保存模型" }));

    await waitFor(() => {
      const createCall = fetcher.mock.calls.find(([input, init]) =>
        String(input).endsWith("/api/admin/models") && init?.method === "POST",
      );
      const payload = JSON.parse(String(createCall?.[1]?.body));
      expect(payload).toMatchObject({
        api_protocol: "openai_audio_speech",
        auth_scheme: "bearer",
        request_base_url: "https://api.zeekai-cn.cc/v1",
        voice_catalog_mode: "shared",
        voice_catalog_source_model_id: ttsModel.model_id,
        catalog_access_key: null,
        catalog_secret_key: null,
        settings: {
          supports_word_timestamps: false,
          word_timestamp_languages: [],
          catalog_sync_interval_minutes: null,
        },
      });
    });
  });

  it("编辑中转 Bearer Key 时使用独立滚动字段区并提交更新", async () => {
    const gatewayModel = {
      ...ttsModel,
      model_id: "77777777-7777-4777-8777-777777777777",
      display_name: "ZeekAI Seed TTS",
      provider_name: "ZeekAI",
      api_protocol: "openai_audio_speech",
      protocol_version: "v1",
      auth_scheme: "bearer",
      request_base_url: "https://api.zeekai-cn.cc/v1",
      api_key_masked: "gate****-key",
      voice_catalog_mode: "shared",
      voice_catalog_source_model_id: ttsModel.model_id,
      voice_catalog_source_display_name: ttsModel.display_name,
      catalog_access_key_masked: null,
      catalog_secret_key_masked: null,
      catalog_access_key_configured: false,
      catalog_secret_key_configured: false,
      settings: {
        ...ttsModel.settings,
        supports_word_timestamps: false,
        word_timestamp_languages: [],
        catalog_sync_interval_minutes: null,
      },
      version: 4,
    } as const;
    let updatePayload: Record<string, unknown> | null = null;
    const fetcher = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.endsWith(`/api/admin/models/${gatewayModel.model_id}`) && init?.method === "PUT") {
        updatePayload = JSON.parse(String(init.body));
        return jsonResponse({ ...gatewayModel, ...updatePayload, version: 5 });
      }
      if (url.includes("/api/speech/models/") && url.endsWith("/voice-catalog")) {
        return jsonResponse({
          model_id: gatewayModel.model_id,
          source_model_id: ttsModel.model_id,
          model_settings: gatewayModel.settings,
          last_sync: null,
          voices: [],
        });
      }
      if (url.includes("/api/admin/models?type=speech")) {
        return jsonResponse({ models: [ttsModel, gatewayModel] });
      }
      return jsonResponse({ models: [textModel] });
    });
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    await screen.findByText("GPT Text");
    fireEvent.click(screen.getByRole("button", { name: "语音模型" }));
    const modelName = await screen.findByText("ZeekAI Seed TTS", { exact: true });
    const modelRow = modelName.closest("tr");
    expect(modelRow).not.toBeNull();
    fireEvent.click(within(modelRow as HTMLTableRowElement).getByRole("button", { name: "编辑" }));

    const dialog = screen.getByRole("dialog", { name: "编辑 AI 模型" });
    await waitFor(() => {
      expect(within(dialog).getByLabelText("目录来源模型")).toHaveValue(ttsModel.model_id);
    });
    const form = dialog.querySelector("form");
    const scrollRegion = dialog.querySelector(".modelDrawerFormScroll");
    const saveButton = within(dialog).getByRole("button", { name: "保存模型" });
    expect(form).toHaveClass("modelDrawerForm");
    expect(scrollRegion).toContainElement(within(dialog).getByLabelText("Bearer API Key"));
    expect(scrollRegion).not.toContainElement(saveButton);

    fireEvent.change(within(dialog).getByLabelText("Bearer API Key"), {
      target: { value: "replacement-gateway-key" },
    });
    fireEvent.click(saveButton);

    await waitFor(() => {
      expect(updatePayload).toMatchObject({
        version: 4,
        api_protocol: "openai_audio_speech",
        api_key: "replacement-gateway-key",
        voice_catalog_source_model_id: ttsModel.model_id,
      });
    });
  });

  it("目录来源独立加载时区分加载失败、无匹配并支持重试", async () => {
    let sourceAttempts = 0;
    let finishFirstSourceRequest: ((response: Response) => void) | undefined;
    const firstSourceRequest = new Promise<Response>((resolve) => {
      finishFirstSourceRequest = resolve;
    });
    const fetcher = vi.fn((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/api/admin/models?type=speech&status=enabled")) {
        sourceAttempts += 1;
        return sourceAttempts === 1
          ? firstSourceRequest
          : jsonResponse({ models: [ttsModel] });
      }
      return jsonResponse({ models: [] });
    });
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    expect(await screen.findByText("当前类型暂无模型")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "添加模型" }));
    const dialog = screen.getByRole("dialog", { name: "添加 AI 模型" });
    fireEvent.change(within(dialog).getByLabelText("模型类型"), {
      target: { value: "speech" },
    });
    fireEvent.change(within(dialog).getByLabelText("API 调用协议"), {
      target: { value: "openai_audio_speech" },
    });

    expect(await within(dialog).findByText("正在加载目录来源模型...")).toBeInTheDocument();
    finishFirstSourceRequest?.(new Response(
      JSON.stringify({ code: "internal_error", message: "测试错误" }),
      { status: 500, headers: { "Content-Type": "application/json" } },
    ));
    expect(await within(dialog).findByText("目录来源模型加载失败")).toBeInTheDocument();
    expect(within(dialog).queryByText("没有匹配当前上游模型和资源 ID 的官方目录模型。")).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button", { name: "重试加载目录来源" }));
    await waitFor(() => {
      expect(within(dialog).getByLabelText("目录来源模型")).toHaveValue(ttsModel.model_id);
    });
    expect(sourceAttempts).toBe(2);
  });

  it("主动同步 TTS 音色目录并展示同步状态", async () => {
    const fetcher = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input).endsWith("/voice-catalog/sync") && init?.method === "POST") {
        return jsonResponse({
          sync_id: "44444444-4444-4444-8444-444444444444",
          model_id: ttsModel.model_id,
          trigger_source: "admin",
          status: "queued",
          page_limit: 100,
          page_count: 0,
          speaker_count: 0,
          error_summary: null,
          requested_at: "2026-07-15T00:00:00Z",
          started_at: null,
          completed_at: null,
          created_at: "2026-07-15T00:00:00Z",
          updated_at: "2026-07-15T00:00:00Z",
        });
      }
      return jsonResponse({ models: [ttsModel] });
    });
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "语音模型" }));
    expect(await screen.findByText("豆包 TTS 2.0")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "同步音色" }));

    expect(await screen.findByText("音色目录已进入同步队列")).toBeInTheDocument();
    expect(fetcher).toHaveBeenCalledWith(
      `http://api.test/api/admin/models/${ttsModel.model_id}/voice-catalog/sync`,
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("提交 Ark Bearer 配置并保持空图片尺寸", async () => {
    const fetcher = vi.fn(
      (_input: RequestInfo | URL, _init?: RequestInit) => jsonResponse({ models: [] }),
    );
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    await screen.findByText("当前类型暂无模型");

    fireEvent.click(screen.getByRole("button", { name: "添加模型" }));
    const dialog = screen.getByRole("dialog", { name: "添加 AI 模型" });
    fireEvent.change(within(dialog).getByLabelText("模型类型"), {
      target: { value: "image" },
    });
    fireEvent.change(within(dialog).getByLabelText("API 调用协议"), {
      target: { value: "volcengine_ark_images" },
    });
    fireEvent.change(within(dialog).getByLabelText("显示名称"), {
      target: { value: "Seedream Ark" },
    });
    fireEvent.change(within(dialog).getByLabelText("供应商"), {
      target: { value: "火山引擎" },
    });
    fireEvent.change(within(dialog).getByLabelText("上游模型标识"), {
      target: { value: "doubao-seedream-5-0-260128" },
    });
    fireEvent.change(within(dialog).getByLabelText("请求地址"), {
      target: { value: "https://ark.cn-beijing.volces.com/api/v3" },
    });
    fireEvent.change(within(dialog).getByLabelText("API Key"), {
      target: { value: "test-key" },
    });
    fireEvent.change(within(dialog).getByLabelText("默认图片尺寸"), {
      target: { value: "" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "保存模型" }));

    await waitFor(() => {
      const createCall = fetcher.mock.calls.find(([, init]) => init?.method === "POST");
      expect(createCall).toBeDefined();
      const payload = JSON.parse(String(createCall?.[1]?.body));
      expect(payload.api_protocol).toBe("volcengine_ark_images");
      expect(payload.auth_scheme).toBe("bearer");
      expect(payload.api_secret).toBeNull();
      expect(payload.settings).toEqual({
        supported_sizes: [],
        default_size: null,
        max_images_per_request: 1,
      });
    });
  });

  it("设为默认使用 POST 并在成功后刷新列表", async () => {
    const replacement = {
      ...textModel,
      model_id: "22222222-2222-4222-8222-222222222222",
      display_name: "GPT Backup",
      is_default: false,
      version: 3,
    };
    const fetcher = vi.fn((_input: RequestInfo | URL, _init?: RequestInit) =>
      jsonResponse({ models: [textModel, replacement] }),
    );
    render(
      <ModelManagementPage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
      />,
    );
    await screen.findByText("GPT Backup");

    fireEvent.click(screen.getByRole("button", { name: "设为默认" }));

    await waitFor(() => {
      expect(fetcher).toHaveBeenCalledWith(
        `http://api.test/api/admin/models/${replacement.model_id}/default`,
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify({ version: 3 }),
        }),
      );
    });
    await waitFor(() => {
      const modelCalls = fetcher.mock.calls.filter(([input]) =>
        String(input).includes("/api/admin/models"),
      );
      expect(modelCalls).toHaveLength(3);
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
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
