import { expect, test } from "@playwright/test";

const baseModel = {
  model_id: "11111111-1111-4111-8111-111111111111",
  display_name: "默认文本模型",
  model_type: "text",
  provider_name: "OpenAI",
  api_protocol: "openai_responses",
  protocol_version: "v1",
  auth_scheme: "bearer",
  request_base_url: "https://api.example.test/v1",
  upstream_model: "gpt-default",
  api_key_masked: "test****-key",
  api_secret_masked: null,
  api_key_configured: true,
  api_secret_configured: false,
  timeout_seconds: 120,
  reasoning_effort: "low",
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
  version: 1,
  deleted_at: null,
  created_at: "2026-07-11T00:00:00Z",
  updated_at: "2026-07-11T00:00:00Z",
};

test("admin 首屏是平台管理后台，不展示视频生产流程", async ({ page }) => {
  await page.route(/\/api\/admin\/models(?:\?.*)?$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { models: [] } });
  });
  await page.route(/\/api\/tools\/tos-staging$/, async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        configured: false,
        version: null,
        enabled: false,
        pending_cleanup_count: 0,
        last_check_status: "never",
      },
    });
  });
  await page.goto("/");

  await expect(page.getByText("NOVEX ADMIN").first()).toBeVisible();
  await expect(page.getByRole("heading", { name: "平台管理后台" })).toBeVisible();

  for (const label of ["用户与权限", "模型与路由", "工具与 MCP", "任务与日志", "成本与限额", "环境健康"]) {
    await expect(page.getByRole("heading", { name: label })).toBeVisible();
  }

  await expect(page.getByText("视频工作台")).toHaveCount(0);
  await expect(page.getByText("脚本智能体")).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "生成脚本" })).toHaveCount(0);
  await expect(page.getByText("时间轴对照视图")).toHaveCount(0);
  await expect(page.getByLabel("分镜数")).toHaveCount(0);

  await page.getByRole("link", { name: "模型与路由" }).click();
  await expect(page).toHaveURL(/\/models$/);
  await expect(page.getByRole("heading", { name: "AI 模型管理" })).toBeVisible();
  await page.getByRole("link", { name: "用户与权限" }).click();
  await expect(page).toHaveURL(/\/#%E7%94%A8%E6%88%B7%E4%B8%8E%E6%9D%83%E9%99%90$/);
  await expect(page.getByRole("heading", { name: "平台管理后台" })).toBeVisible();
  await page.getByRole("link", { name: "工具与 MCP" }).click();
  await expect(page).toHaveURL(/\/tools$/);
  await expect(page.getByRole("heading", { name: "私有 TOS" })).toBeVisible();
});

test("火山方舟图片协议表单固定 Bearer 与单候选", async ({ page }) => {
  await page.route(/\/api\/admin\/models(?:\?.*)?$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { models: [] } });
  });
  await page.goto("/models");

  await page.getByRole("button", { name: "添加模型" }).click();
  const drawer = page.getByRole("dialog", { name: "添加 AI 模型" });
  await drawer.getByLabel("模型类型").selectOption("image");
  await expect(drawer.getByRole("option", { name: "火山方舟图片生成" })).toHaveCount(1);
  await expect(drawer.getByRole("option", { name: "即梦 Visual" })).toHaveCount(0);
  await drawer.getByLabel("API 调用协议").selectOption("volcengine_ark_images");

  await expect(drawer.getByLabel("API Secret")).toHaveCount(0);
  await expect(drawer.getByLabel("默认图片尺寸")).toHaveValue("");
  await expect(drawer.getByLabel("单次最大图片数")).toHaveValue("1");
  await expect(drawer.getByLabel("单次最大图片数")).toBeDisabled();
});

test("目录来源模型独立于当前列表标签和筛选加载", async ({ page }) => {
  const officialTtsModel = {
    ...baseModel,
    model_id: "44444444-4444-4444-8444-444444444444",
    display_name: "豆包 TTS 2.0",
    model_type: "speech",
    provider_name: "火山引擎",
    api_protocol: "volcengine_tts_v3",
    protocol_version: "v3",
    auth_scheme: "api_key",
    upstream_model: "doubao-seed-tts-2.0",
    voice_catalog_mode: "official_sync",
    voice_catalog_source_model_id: null,
    voice_catalog_source_display_name: null,
    settings: { resource_id: "seed-tts-2.0" },
  };
  const modelRequests: string[] = [];
  await page.route(/\/api\/admin\/models(?:\?.*)?$/, async (route) => {
    const url = new URL(route.request().url());
    modelRequests.push(`${url.pathname}${url.search}`);
    const independentSourceQuery = url.searchParams.get("type") === "speech"
      && url.searchParams.get("status") === "enabled"
      && [...url.searchParams.keys()].length === 2;
    await route.fulfill({
      contentType: "application/json",
      json: { models: independentSourceQuery ? [officialTtsModel] : [] },
    });
  });
  await page.route(/\/api\/speech\/models\/.*\/voice-catalog$/, async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        model_id: officialTtsModel.model_id,
        source_model_id: officialTtsModel.model_id,
        model_settings: officialTtsModel.settings,
        last_sync: null,
        voices: [],
      },
    });
  });

  await page.goto("/models");
  await page.getByLabel("搜索模型").fill("只筛选当前文本列表");
  await page.getByRole("button", { name: "添加模型" }).click();
  const drawer = page.getByRole("dialog", { name: "添加 AI 模型" });
  await drawer.getByLabel("模型类型").selectOption("speech");
  await drawer.getByLabel("API 调用协议").selectOption("openai_audio_speech");

  await expect(drawer.getByLabel("目录来源模型")).toHaveValue(officialTtsModel.model_id);
  await expect(drawer.getByRole("option", { name: "豆包 TTS 2.0" })).toHaveCount(1);
  expect(modelRequests).toContain("/api/admin/models?type=speech&status=enabled");
  expect(modelRequests.some((request) => request.includes("type=text") && request.includes("q="))).toBe(true);
});

test("私有 TOS 在工具与 MCP 中独立保存并检查连接", async ({ page }) => {
  let current: Record<string, unknown> = {
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
  };
  let savedPayload: Record<string, unknown> | null = null;
  await page.route(/\/api\/tools\/tos-staging(?:\/check)?$/, async (route) => {
    const request = route.request();
    if (request.url().endsWith("/check")) {
      expect(request.postDataJSON()).toEqual({ version: 1 });
      current = {
        ...current,
        last_check_status: "queued",
        last_check_requested_at: "2026-07-16T08:00:00Z",
      };
      await route.fulfill({ status: 202, contentType: "application/json", json: current });
      return;
    }
    if (request.method() === "PUT") {
      savedPayload = request.postDataJSON();
      current = {
        ...current,
        ...savedPayload,
        configured: true,
        config_id: "66666666-6666-4666-8666-666666666666",
        version: 1,
        access_key_masked: "tos-****1234",
        secret_key_masked: "tos-****5678",
        access_key_configured: true,
        secret_key_configured: true,
      };
    }
    await route.fulfill({ contentType: "application/json", json: current });
  });

  await page.goto("/tools");
  await expect(page.getByLabel("Bucket")).toBeEnabled();
  await page.getByLabel("Bucket").fill("novex-private-staging");
  await page.getByLabel("Access Key").fill("tos-access-1234");
  await page.getByLabel("Secret Key").fill("tos-secret-5678");
  await expect(page.getByLabel("启用系统 TOS")).toBeDisabled();
  await page.getByRole("button", { name: "保存配置" }).click();

  await expect(page.getByText("系统 TOS 已保存为版本 1")).toBeVisible();
  expect(savedPayload).toMatchObject({
    version: null,
    enabled: false,
    endpoint: "https://tos-cn-beijing.volces.com",
    region: "cn-beijing",
    bucket: "novex-private-staging",
    object_prefix: "novex/asr",
  });
  await page.getByRole("button", { name: "检查连接" }).click();
  await expect(page.getByText("TOS Bucket 连接检查已进入队列")).toBeVisible();
  await expect(page.getByText("待检查")).toBeVisible();
});

test("语音模型支持 TTS 目录凭据并只读引用系统 TOS", async ({ page }) => {
  let speechModels: Array<Record<string, unknown>> = [];
  let createdPayload: Record<string, unknown> | null = null;
  let updatedPayload: Record<string, unknown> | null = null;
  await page.route(/\/api\/admin\/models(?:\/.*)?(?:\?.*)?$/, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.pathname === "/api/admin/models" && request.method() === "GET") {
      await route.fulfill({ contentType: "application/json", json: { models: url.searchParams.get("type") === "speech" ? speechModels : [] } });
      return;
    }
    if (url.pathname === "/api/admin/models" && request.method() === "POST") {
      createdPayload = request.postDataJSON();
      const isSharedCatalog = createdPayload?.voice_catalog_mode === "shared";
      const isOpenAiGateway = createdPayload?.api_protocol === "openai_audio_speech";
      const created = {
        ...baseModel,
        ...createdPayload,
        model_id: isOpenAiGateway
          ? "77777777-7777-4777-8777-777777777777"
          : isSharedCatalog
            ? "66666666-6666-4666-8666-666666666666"
            : "44444444-4444-4444-8444-444444444444",
        catalog_access_key_masked: isSharedCatalog ? null : "cata****cess",
        catalog_secret_key_masked: isSharedCatalog ? null : "cata****cret",
        catalog_access_key_configured: !isSharedCatalog,
        catalog_secret_key_configured: !isSharedCatalog,
        voice_catalog_mode: isSharedCatalog ? "shared" : "official_sync",
        voice_catalog_source_model_id: isSharedCatalog
          ? "44444444-4444-4444-8444-444444444444"
          : null,
        voice_catalog_source_display_name: isSharedCatalog ? "豆包 TTS 2.0" : null,
        api_key_masked: "spee****-key",
        api_secret_masked: null,
        api_key_configured: true,
        api_secret_configured: false,
        reasoning_effort: null,
        max_output_tokens: null,
        is_default: true,
      };
      speechModels = [...speechModels, created];
      await route.fulfill({ status: 201, contentType: "application/json", json: created });
      return;
    }
    const modelMatch = url.pathname.match(/^\/api\/admin\/models\/([^/]+)$/);
    if (modelMatch && request.method() === "PUT") {
      updatedPayload = request.postDataJSON();
      const current = speechModels.find((model) => model.model_id === modelMatch[1]);
      const updated = {
        ...current,
        ...updatedPayload,
        version: Number(current?.version ?? 1) + 1,
      };
      speechModels = speechModels.map((model) => model.model_id === modelMatch[1] ? updated : model);
      await route.fulfill({ contentType: "application/json", json: updated });
      return;
    }
    if (url.pathname.endsWith("/voice-catalog/sync") && request.method() === "POST") {
      await route.fulfill({
        status: 201,
        contentType: "application/json",
        json: {
          sync_id: "55555555-5555-4555-8555-555555555555",
          model_id: "44444444-4444-4444-8444-444444444444",
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
        },
      });
      return;
    }
    await route.abort();
  });
  await page.route(/\/api\/speech\/models\/.*\/voice-catalog(?:\?.*)?$/, async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        model_id: route.request().url().split("/models/")[1].split("/")[0],
        source_model_id: "44444444-4444-4444-8444-444444444444",
        model_settings: {},
        last_sync: null,
        voices: [],
      },
    });
  });
  await page.route(/\/api\/tools\/tos-staging$/, async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        configured: true,
        version: 4,
        enabled: true,
        pending_cleanup_count: 0,
        last_check_status: "succeeded",
      },
    });
  });
  await page.goto("/models");
  await page.getByRole("button", { name: "语音模型" }).click();
  await page.getByRole("button", { name: "添加模型" }).click();
  const drawer = page.getByRole("dialog", { name: "添加 AI 模型" });
  await drawer.getByLabel("模型类型").selectOption("speech");
  await expect(drawer.getByLabel("API 调用协议")).toHaveValue("volcengine_tts_v3");
  const timestampLanguageTrigger = drawer.getByRole("button", { name: "时间戳语言" });
  await expect(timestampLanguageTrigger).toHaveAttribute("aria-expanded", "false");
  await expect(timestampLanguageTrigger).toContainText("简体中文、美式英语");
  await timestampLanguageTrigger.click();
  const languageSearch = drawer.getByRole("searchbox", { name: "搜索时间戳语言" });
  await languageSearch.fill("美式");
  await expect(drawer.getByRole("checkbox", { name: "简体中文" })).toHaveCount(0);
  await expect(drawer.getByRole("checkbox", { name: "美式英语" })).toBeVisible();
  await languageSearch.fill("");
  await expect(drawer.getByRole("checkbox", { name: "简体中文" })).toBeChecked();
  await expect(drawer.getByRole("checkbox", { name: "美式英语" })).toBeChecked();
  await drawer.getByRole("checkbox", { name: "美式英语" }).uncheck();
  await expect(drawer.getByRole("checkbox", { name: "简体中文" })).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(timestampLanguageTrigger).toHaveAttribute("aria-expanded", "false");
  await expect(timestampLanguageTrigger).toContainText("简体中文");
  await timestampLanguageTrigger.click();
  await drawer.getByText("声音能力", { exact: true }).click();
  await expect(timestampLanguageTrigger).toHaveAttribute("aria-expanded", "false");
  await drawer.getByLabel("显示名称").fill("豆包 TTS 2.0");
  await drawer.getByLabel("TTS X-Api-Key").fill("speech-key");
  await expect(drawer.getByText(/OpenAPI AK\/SK 仅用于 ListSpeakers HMAC 签名，不会进入请求体/)).toBeVisible();
  await drawer.getByLabel("OpenAPI Access Key（AK）").fill("catalog-access");
  await drawer.getByLabel("OpenAPI Secret Key（SK）").fill("catalog-secret");
  await drawer.getByRole("button", { name: "保存模型" }).click();

  await expect(page.getByText("豆包 TTS 2.0", { exact: true })).toBeVisible();
  expect(createdPayload).toMatchObject({
    model_type: "speech",
    api_protocol: "volcengine_tts_v3",
    auth_scheme: "api_key",
    catalog_access_key: "catalog-access",
    catalog_secret_key: "catalog-secret",
    voice_catalog_mode: "official_sync",
    voice_catalog_source_model_id: null,
    settings: {
      word_timestamp_languages: ["zh-cn"],
    },
  });
  await page.getByRole("button", { name: "同步音色" }).click();
  await expect(page.getByText("音色目录已进入同步队列")).toBeVisible();

  await page.getByRole("button", { name: "添加模型" }).click();
  const sharedDrawer = page.getByRole("dialog", { name: "添加 AI 模型" });
  await sharedDrawer.getByLabel("模型类型").selectOption("speech");
  await sharedDrawer.getByLabel("显示名称").fill("Seed TTS 中转");
  await sharedDrawer.getByLabel("供应商").fill("中转服务");
  await sharedDrawer.getByLabel("请求地址").fill("https://speech-gateway.example.com/api/v3");
  await sharedDrawer.getByLabel("TTS X-Api-Key").fill("gateway-key");
  await sharedDrawer.getByRole("radio", { name: "复用已有目录" }).check();
  await expect(sharedDrawer.getByLabel("OpenAPI Access Key（AK）")).toHaveCount(0);
  await expect(sharedDrawer.getByLabel("OpenAPI Secret Key（SK）")).toHaveCount(0);
  await expect(sharedDrawer.getByLabel("目录来源模型")).toHaveValue(
    "44444444-4444-4444-8444-444444444444",
  );
  await sharedDrawer.getByRole("button", { name: "保存模型" }).click();
  await expect(page.getByText("Seed TTS 中转", { exact: true })).toBeVisible();
  expect(createdPayload).toMatchObject({
    voice_catalog_mode: "shared",
    voice_catalog_source_model_id: "44444444-4444-4444-8444-444444444444",
    catalog_access_key: null,
    catalog_secret_key: null,
  });
  await expect(page.getByText("复用：豆包 TTS 2.0", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "添加模型" }).click();
  const gatewayDrawer = page.getByRole("dialog", { name: "添加 AI 模型" });
  await gatewayDrawer.getByLabel("模型类型").selectOption("speech");
  await gatewayDrawer.getByLabel("API 调用协议").selectOption("openai_audio_speech");
  await gatewayDrawer.getByLabel("显示名称").fill("ZeekAI Seed TTS");
  await gatewayDrawer.getByLabel("供应商").fill("ZeekAI");
  await gatewayDrawer.getByLabel("请求地址").fill("https://api.zeekai.example.com/v1");
  await gatewayDrawer.getByLabel("Bearer API Key").fill("gateway-bearer-key");
  await expect(gatewayDrawer.getByRole("radio", { name: "官方同步" })).toHaveCount(0);
  await expect(gatewayDrawer.getByLabel("OpenAPI Access Key（AK）")).toHaveCount(0);
  await expect(gatewayDrawer.getByLabel("OpenAPI Secret Key（SK）")).toHaveCount(0);
  await expect(gatewayDrawer.getByLabel("目录来源模型")).toHaveValue(
    "44444444-4444-4444-8444-444444444444",
  );
  await expect(gatewayDrawer.getByLabel("时间戳语言")).toHaveText("不支持（仅生成配音）");
  const gatewayDrawerLayout = await gatewayDrawer.evaluate((drawerElement) => {
    const header = drawerElement.querySelector(":scope > header")?.getBoundingClientRect();
    const scrollRegion = drawerElement.querySelector(".modelDrawerFormScroll") as HTMLElement | null;
    const footer = drawerElement.querySelector("form > footer")?.getBoundingClientRect();
    const drawer = drawerElement.getBoundingClientRect();
    return header && scrollRegion && footer
      ? {
          headerBottom: Math.round(header.bottom),
          footerTop: Math.round(footer.top),
          footerBottom: Math.round(footer.bottom),
          drawerBottom: Math.round(drawer.bottom),
          scrollHeight: scrollRegion.scrollHeight,
          clientHeight: scrollRegion.clientHeight,
        }
      : null;
  });
  expect(gatewayDrawerLayout).not.toBeNull();
  expect(gatewayDrawerLayout!.footerTop).toBeGreaterThanOrEqual(gatewayDrawerLayout!.headerBottom);
  expect(gatewayDrawerLayout!.footerBottom).toBeLessThanOrEqual(gatewayDrawerLayout!.drawerBottom);
  expect(gatewayDrawerLayout!.scrollHeight).toBeGreaterThan(gatewayDrawerLayout!.clientHeight);
  await gatewayDrawer.getByRole("button", { name: "保存模型" }).click();
  await expect(page.getByText("ZeekAI Seed TTS", { exact: true })).toBeVisible();
  expect(createdPayload).toMatchObject({
    api_protocol: "openai_audio_speech",
    auth_scheme: "bearer",
    request_base_url: "https://api.zeekai.example.com/v1",
    voice_catalog_mode: "shared",
    voice_catalog_source_model_id: "44444444-4444-4444-8444-444444444444",
    catalog_access_key: null,
    catalog_secret_key: null,
    settings: {
      supports_word_timestamps: false,
      word_timestamp_languages: [],
      catalog_sync_interval_minutes: null,
    },
  });

  const gatewayRow = page.getByRole("row").filter({ hasText: "ZeekAI Seed TTS" });
  await gatewayRow.getByRole("button", { name: "编辑" }).click();
  const gatewayEditDrawer = page.getByRole("dialog", { name: "编辑 AI 模型" });
  await expect(gatewayEditDrawer.getByLabel("Bearer API Key")).toHaveValue("");
  await gatewayEditDrawer.getByLabel("Bearer API Key").fill("replacement-gateway-key");
  await gatewayEditDrawer.getByRole("button", { name: "保存模型" }).click();
  expect(updatedPayload).toMatchObject({
    api_key: "replacement-gateway-key",
    api_protocol: "openai_audio_speech",
    voice_catalog_source_model_id: "44444444-4444-4444-8444-444444444444",
  });

  await page.getByRole("button", { name: "添加模型" }).click();
  const asrDrawer = page.getByRole("dialog", { name: "添加 AI 模型" });
  await asrDrawer.getByLabel("模型类型").selectOption("speech");
  await asrDrawer.getByLabel("API 调用协议").selectOption("volcengine_asr_v3");
  await expect(asrDrawer.getByLabel("OpenAPI Access Key（AK）")).toHaveCount(0);
  await expect(asrDrawer.getByLabel("ASR X-Api-Key")).toBeVisible();
  await expect(asrDrawer.getByRole("checkbox", { name: "简体中文" })).toHaveCount(0);
  await expect(asrDrawer.getByRole("checkbox", { name: "美式英语" })).toHaveCount(0);
  await expect(asrDrawer.getByLabel("时间戳语言")).toHaveText("自动识别（全部语言）");
  await expect(asrDrawer.getByLabel("系统私有 TOS 状态")).toContainText("已配置并启用");
  await expect(asrDrawer.getByRole("link", { name: "前往工具与 MCP 配置" })).toHaveAttribute("href", "/tools");
  await expect(asrDrawer.getByLabel("TOS Endpoint")).toHaveCount(0);
  await expect(asrDrawer.getByLabel("TOS Access Key")).toHaveCount(0);
});

test("AI 模型管理使用 mocked API 完成创建、编辑、默认替换、停用和删除", async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 948 });
  const replacementModel = {
    ...baseModel,
    model_id: "22222222-2222-4222-8222-222222222222",
    display_name: "备用文本模型",
    upstream_model: "gpt-secondary",
    is_default: false,
  };
  let models = [baseModel, replacementModel];

  await page.route(/\/api\/admin\/models(?:\/.*)?(?:\?.*)?$/, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const path = url.pathname;
    const method = request.method();

    if (path === "/api/admin/models" && method === "GET") {
      await route.fulfill({ contentType: "application/json", json: { models } });
      return;
    }
    if (path === "/api/admin/models" && method === "POST") {
      const payload = request.postDataJSON();
      expect(payload.api_key).toBe("test-create-key");
      const created = {
        ...baseModel,
        ...payload,
        model_id: "33333333-3333-4333-8333-333333333333",
        api_key_masked: "test****-key",
        is_default: false,
        version: 1,
      };
      models = [...models, created];
      await route.fulfill({ contentType: "application/json", status: 201, json: created });
      return;
    }

    const modelId = path.split("/")[4];
    const model = models.find((item) => item.model_id === modelId);
    if (!model) {
      await route.fulfill({ contentType: "application/json", status: 404, json: { error: { code: "model_not_found" } } });
      return;
    }
    if (method === "PUT" && path === `/api/admin/models/${modelId}`) {
      const payload = request.postDataJSON();
      expect(payload.api_key).toBe("");
      const updated = { ...model, ...payload, version: model.version + 1 };
      models = models.map((item) => item.model_id === modelId ? updated : item);
      await route.fulfill({ contentType: "application/json", json: updated });
      return;
    }
    if (method === "POST" && path.endsWith("/default")) {
      models = models.map((item) => ({
        ...item,
        is_default: item.model_id === modelId,
        version: item.version + (item.model_id === modelId ? 1 : 0),
      }));
      await route.fulfill({ contentType: "application/json", json: models.find((item) => item.model_id === modelId) });
      return;
    }
    if (method === "PUT" && path.endsWith("/status")) {
      const payload = request.postDataJSON();
      models = models.map((item) => ({
        ...item,
        status: item.model_id === modelId ? payload.status : item.status,
        is_default: payload.replacement_model_id
          ? item.model_id === payload.replacement_model_id
          : item.model_id === modelId ? false : item.is_default,
        version: item.model_id === modelId ? item.version + 1 : item.version,
      }));
      await route.fulfill({ contentType: "application/json", json: models.find((item) => item.model_id === modelId) });
      return;
    }
    if (method === "DELETE") {
      models = models.filter((item) => item.model_id !== modelId);
      await route.fulfill({ contentType: "application/json", json: { model_id: modelId, deletion_mode: "physical" } });
      return;
    }
    await route.abort();
  });

  await page.goto("/models");
  await expect(page.getByRole("heading", { name: "AI 模型管理" })).toBeVisible();
  const modelLayout = await page.locator(".adminWorkbench").evaluate((workbench) => {
    const header = workbench.querySelector(".modelHeader")?.getBoundingClientRect();
    const tabs = workbench.querySelector(".modelTabs")?.getBoundingClientRect();
    const toolbar = workbench.querySelector(".modelToolbar")?.getBoundingClientRect();
    const table = workbench.querySelector(".modelTableWrap")?.getBoundingClientRect();
    return header && tabs && toolbar && table
      ? {
          headerToTabs: Math.round(tabs.top - header.bottom),
          tabsHeight: Math.round(tabs.height),
          tabsToToolbar: Math.round(toolbar.top - tabs.bottom),
          toolbarHeight: Math.round(toolbar.height),
          toolbarToTable: Math.round(table.top - toolbar.bottom),
        }
      : null;
  });
  expect(modelLayout).not.toBeNull();
  expect(modelLayout!.headerToTabs).toBeLessThanOrEqual(2);
  expect(modelLayout!.tabsHeight).toBeLessThanOrEqual(64);
  expect(modelLayout!.tabsToToolbar).toBeLessThanOrEqual(2);
  expect(modelLayout!.toolbarHeight).toBeLessThanOrEqual(72);
  expect(modelLayout!.toolbarToTable).toBeLessThanOrEqual(2);
  await page.getByRole("button", { name: "添加模型" }).click();
  const createDrawer = page.getByRole("dialog", { name: "添加 AI 模型" });
  await createDrawer.getByLabel("显示名称").fill("新文本模型");
  await createDrawer.getByLabel("供应商").fill("OpenAI");
  await createDrawer.getByLabel("上游模型标识").fill("gpt-new");
  await createDrawer.getByLabel("请求地址").fill("https://api.example.test/v1");
  await createDrawer.getByLabel("API Key").fill("test-create-key");
  await createDrawer.getByRole("button", { name: "保存模型" }).click();
  await expect(page.getByText("新文本模型", { exact: true })).toBeVisible();

  let modelRow = page.getByRole("row").filter({ hasText: "新文本模型" });
  await modelRow.getByRole("button", { name: "编辑" }).click();
  const editDrawer = page.getByRole("dialog", { name: "编辑 AI 模型" });
  await expect(editDrawer.getByLabel("API Key")).toHaveValue("");
  await editDrawer.getByLabel("显示名称").fill("已编辑文本模型");
  await editDrawer.getByRole("button", { name: "保存模型" }).click();
  await expect(page.getByText("已编辑文本模型", { exact: true })).toBeVisible();

  modelRow = page.getByRole("row").filter({ hasText: "已编辑文本模型" });
  await modelRow.getByRole("button", { name: "设为默认" }).click();
  await expect(modelRow.getByText("默认", { exact: true })).toBeVisible();
  await modelRow.getByRole("button", { name: "停用" }).click();
  const disableDialog = page.getByRole("dialog", { name: "停用默认模型" });
  await disableDialog.getByLabel("替代默认模型").selectOption(baseModel.model_id);
  await disableDialog.getByRole("button", { name: "确认停用" }).click();
  await expect(modelRow.getByText("已停用", { exact: true })).toBeVisible();

  await modelRow.getByRole("button", { name: "删除" }).click();
  const deleteDialog = page.getByRole("dialog", { name: "删除模型" });
  await deleteDialog.getByRole("button", { name: "确认删除" }).click();
  await expect(page.getByText("已编辑文本模型", { exact: true })).toHaveCount(0);
});
