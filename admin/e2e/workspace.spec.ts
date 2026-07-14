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
