import { expect, test, type Page, type Route } from "@playwright/test";

const projectId = "11111111-1111-4111-8111-111111111111";
const planId = "22222222-2222-4222-8222-222222222222";
const workId = "33333333-3333-4333-8333-333333333333";
const versionId = "44444444-4444-4444-8444-444444444444";

type Target = ReturnType<typeof createTarget>;

test("人工发布从双平台准备到人工确认形成可审计闭环", async ({ page, context }, testInfo) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript(() => {
    (window as typeof window & { __openedUrls: string[] }).__openedUrls = [];
    window.open = ((url?: string | URL) => {
      (window as typeof window & { __openedUrls: string[] }).__openedUrls.push(String(url ?? ""));
      return null;
    }) as typeof window.open;
  });
  const state = await mockPublicationApi(page);

  await page.setViewportSize({ width: 1920, height: 980 });
  await page.goto(`/publishing/workbench?plan=${planId}`);
  await expect(page.getByRole("heading", { name: "人工发布运营" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "夏日防晒指南" })).toBeVisible();
  await expect(page.getByRole("button", { name: /发布运营/ })).toHaveClass(/active/);
  await expect(page.getByRole("button", { name: "发布工作台" })).toHaveClass(/active/);
  await assertNoHorizontalOverflow(page);
  await page.screenshot({ path: testInfo.outputPath("publication-wide-1920x980.png"), fullPage: true });

  await page.setViewportSize({ width: 1440, height: 900 });
  await assertNoHorizontalOverflow(page);
  await page.screenshot({ path: testInfo.outputPath("publication-narrow-1440x900.png"), fullPage: true });

  let xhs = page.getByRole("region", { name: "小红书发布目标" });
  await xhs.getByLabel("平台标题").fill("小红书独立标题");
  await xhs.getByLabel("发布正文").fill("小红书独立正文");
  await xhs.getByRole("button", { name: "保存草稿" }).click();
  xhs = page.getByRole("region", { name: "小红书发布目标" });
  await expect(xhs.getByLabel("平台标题")).toHaveValue("小红书独立标题");
  await expect(page.getByRole("region", { name: "抖音发布目标" }).getByLabel("平台标题")).toHaveValue("抖音独立标题");
  await xhs.getByRole("button", { name: "生成发布包" }).click();
  await expect(page.getByRole("region", { name: "小红书发布目标" }).getByText("准备完成")).toBeVisible();

  let douyin = page.getByRole("region", { name: "抖音发布目标" });
  await douyin.getByRole("button", { name: "复制文案" }).click();
  await expect(douyin.getByText("文案已复制并记录审计")).toBeVisible();
  await douyin.getByRole("button", { name: "下载完整发布包" }).click();
  await expect.poll(() => state.downloadAudits).toBe(1);

  douyin = page.getByRole("region", { name: "抖音发布目标" });
  await douyin.getByRole("button", { name: "去平台发布" }).click();
  douyin = page.getByRole("region", { name: "抖音发布目标" });
  await expect(douyin.getByText("等待人工发布").first()).toBeVisible();
  await expect.poll(() => openedUrls(page)).toContain("https://creator.douyin.com/");

  await douyin.getByRole("button", { name: "标记需处理" }).click();
  douyin = page.getByRole("region", { name: "抖音发布目标" });
  await expect(douyin.getByText("需处理")).toBeVisible();
  await douyin.getByLabel("发布正文").fill("修正后的抖音正文");
  await douyin.getByRole("button", { name: "保存草稿" }).click();
  douyin = page.getByRole("region", { name: "抖音发布目标" });
  await douyin.getByRole("button", { name: "生成发布包" }).click();
  douyin = page.getByRole("region", { name: "抖音发布目标" });
  await douyin.getByRole("button", { name: "去平台发布" }).click();

  douyin = page.getByRole("region", { name: "抖音发布目标" });
  await douyin.getByLabel("官方作品链接").fill("https://www.douyin.com/video/123");
  await douyin.getByLabel("实际发布时间").fill("2026-07-22T06:00");
  await douyin.getByRole("button", { name: "人工确认已发布" }).click();
  await expect(page.getByText("计划状态 部分完成")).toBeVisible();
  await expect(page.getByRole("region", { name: "抖音发布目标" }).getByText("人工确认已发布").first()).toBeVisible();

  xhs = page.getByRole("region", { name: "小红书发布目标" });
  await xhs.getByRole("button", { name: "去平台发布" }).click();
  xhs = page.getByRole("region", { name: "小红书发布目标" });
  await xhs.getByLabel("官方作品链接").fill("https://www.xiaohongshu.com/explore/123");
  await xhs.getByLabel("实际发布时间").fill("2026-07-22T06:10");
  await xhs.getByRole("button", { name: "人工确认已发布" }).click();
  await expect(page.getByText("计划状态 已发布")).toBeVisible();

  await page.getByRole("button", { name: "发布记录" }).click();
  await expect(page.getByText("夏日防晒指南").first()).toBeVisible();
  expect(state.copyAudits).toBe(1);
  expect(await openedUrls(page)).toEqual(expect.arrayContaining([
    "https://creator.douyin.com/",
    "https://creator.xiaohongshu.com/",
  ]));
  expect((await openedUrls(page)).filter((url) => url.startsWith("https://creator."))).toEqual([
    "https://creator.douyin.com/",
    "https://creator.douyin.com/",
    "https://creator.xiaohongshu.com/",
  ]);
});

async function mockPublicationApi(page: Page) {
  const douyin = createTarget("douyin", "ready", 1);
  const xiaohongshu = createTarget("xiaohongshu", "draft", 1);
  const targets = new Map<string, Target>([[douyin.platform, douyin], [xiaohongshu.platform, xiaohongshu]]);
  const state = { copyAudits: 0, downloadAudits: 0 };

  await page.route("**/health", (route) => json(route, {}));
  await page.route("**/api/model-options?*", (route) => json(route, { models: [] }));
  await page.route("**/api/projects", (route) => json(route, { projects: [{
    project_id: projectId, name: "科技账号", positioning: "知识内容", description: "", strategy_profile: { target_audience: "", content_pillars: [], tone_style: "", forbidden_topics: [], reference_accounts: [], topic_preferences: "" }, status: "active", created_at: "2026-07-20T00:00:00Z", updated_at: "2026-07-23T00:00:00Z",
  }] }));
  await page.route("**/api/video-workspace/menus", (route) => json(route, { menus: workspaceMenus() }));
  await page.route(`**/api/works/${workId}`, (route) => json(route, {
    id: workId, project_id: projectId, script_id: "script-1", title: "夏日防晒指南", status: "succeeded", archived: false, current_version_id: versionId,
    versions: [{ id: versionId, work_id: workId, version_no: 2, status: "completed", source_version_id: null, derivation_kind: "initial", source_manifest_version: "manifest-v1", input_snapshot: {}, model_snapshot: {}, parameter_snapshot: {}, prompt_snapshot: {}, timeline_snapshot: {}, created_at: "2026-07-22T00:00:00Z", updated_at: "2026-07-22T00:00:00Z", completed_at: "2026-07-22T00:00:00Z" }],
    artifacts: [], timelines: [], generation_audit: [], created_at: "2026-07-20T00:00:00Z", updated_at: "2026-07-23T00:00:00Z",
  }));
  await page.route("**/api/publications", (route) => json(route, { items: [{ ...plan(targets), work_title: "夏日防晒指南" }] }));
  await page.route(`**/api/publications/${planId}`, (route) => json(route, plan(targets)));
  await page.route("**/api/publications/*/targets/*", async (route) => {
    expect(route.request().headers()["idempotency-key"]).toBeTruthy();
    const platform = route.request().url().split("/").at(-1) as "douyin" | "xiaohongshu";
    const current = targets.get(platform)!;
    const body = route.request().postDataJSON();
    expect(body.expected_revision).toBe(current.draft_revision);
    Object.assign(current, {
      title: body.title, body: body.body, tags: body.tags, cover_artifact_id: body.cover_artifact_id, planned_at: body.planned_at,
      draft_revision: current.draft_revision + 1, status: "draft", handed_off_at: null, updated_at: new Date().toISOString(),
    });
    await json(route, current);
  });
  await page.route("**/api/publication-targets/*/package", async (route) => {
    const target = targetById(targets, route);
    expect(route.request().headers()["idempotency-key"]).toBeTruthy();
    expect(route.request().postDataJSON().draft_revision).toBe(target.draft_revision);
    target.status = "ready";
    await json(route, { id: `package-${target.id}`, publication_target_id: target.id, draft_revision: target.draft_revision, platform_rule_version: "manual-web-v1", manifest: {}, manifest_sha256: "a".repeat(64), created_at: new Date().toISOString(), created: true });
  });
  await page.route("**/api/publication-targets/*/downloads", (route) => {
    const target = targetById(targets, route);
    return json(route, { target_id: target.id, draft_revision: target.draft_revision, video: { artifact_id: "video-1", download_url: "/api/work-artifacts/video-1/download" }, cover: null, package: { id: `package-${target.id}`, manifest_sha256: "a".repeat(64), download_url: `/api/publication-packages/package-${target.id}/download` } });
  });
  await page.route("**/api/publication-targets/*/copy-audits", async (route) => { state.copyAudits += 1; await route.fulfill({ status: 204 }); });
  await page.route("**/api/publication-targets/*/download-audits", async (route) => { state.downloadAudits += 1; await route.fulfill({ status: 204 }); });
  await page.route("**/api/publication-targets/*/handoff", async (route) => {
    const target = targetById(targets, route);
    target.status = "handed_off";
    target.handed_off_at = new Date().toISOString();
    const entrance = target.platform === "douyin" ? "https://creator.douyin.com/" : "https://creator.xiaohongshu.com/";
    await json(route, { target, official_entrance: entrance, publication_confirmation: "manual_required" });
  });
  await page.route("**/api/publication-targets/*/needs-attention", async (route) => { const target = targetById(targets, route); target.status = "needs_attention"; await json(route, target); });
  await page.route("**/api/publication-targets/*/published", async (route) => {
    const target = targetById(targets, route);
    const body = route.request().postDataJSON();
    Object.assign(target, { status: "published", published_url: body.published_url, published_at: body.published_at, result_snapshot: { confirmation: "manual" } });
    await json(route, target);
  });
  return state;
}

function createTarget(platform: "douyin" | "xiaohongshu", status: "draft" | "ready", draftRevision: number) {
  return { id: `target-${platform}`, publication_plan_id: planId, platform, status: status as string, title: platform === "douyin" ? "抖音独立标题" : "", body: platform === "douyin" ? "抖音独立正文" : "", tags: [], cover_artifact_id: null, planned_at: "2026-07-22T02:00:00Z", draft_revision: draftRevision, handed_off_at: null as string | null, published_at: null as string | null, published_url: null as string | null, result_snapshot: {}, overdue: true, created_at: "2026-07-22T00:00:00Z", updated_at: "2026-07-23T00:00:00Z" };
}

function plan(targets: Map<string, Target>) {
  const values = Array.from(targets.values());
  const published = values.filter((target) => target.status === "published").length;
  const status = published === values.length ? "published" : published > 0 ? "partially_published" : values.some((target) => target.status === "needs_attention") ? "needs_attention" : values.some((target) => target.status === "draft") ? "draft" : values.some((target) => target.status === "handed_off") ? "handed_off" : "ready";
  return { id: planId, handoff_id: "handoff-1", work_id: workId, work_version_id: versionId, final_video_artifact_id: "video-1", subtitle_artifact_id: null, status, targets: values, created_at: "2026-07-22T00:00:00Z", updated_at: new Date().toISOString() };
}

function workspaceMenus() {
  const sections = [["content-strategy", "内容策略", 10], ["script-creation", "脚本创作", 20], ["material-management", "素材管理", 30], ["production", "作品生产", 40], ["publishing", "发布运营", 50], ["analytics", "数据分析", 60], ["workflow-tasks", "工作流任务", 70]] as const;
  return sections.map(([key, label, order]) => ({ menu_id: `menu-${key}`, menu_key: key, label, description: label, route_path: key === "publishing" ? "/publishing/workbench" : `/${key}`, icon: "circle", menu_type: "section", module_key: key, agent_key: null, sort_order: order, is_enabled: key === "publishing" || key === "content-strategy", status: key === "publishing" || key === "content-strategy" ? "active" : "planned", metadata: {}, children: key === "publishing" ? [{ menu_id: "menu-publish-workbench", menu_key: "publish-scheduler", label: "发布工作台", description: "人工发布运营", route_path: "/publishing/workbench", icon: "send", menu_type: "page", module_key: "publishing.workbench", agent_key: null, sort_order: 10, is_enabled: true, status: "active", metadata: {}, children: [] }] : [] }));
}

function targetById(targets: Map<string, Target>, route: Route) { const id = route.request().url().split("/publication-targets/")[1].split("/")[0]; const target = Array.from(targets.values()).find((item) => item.id === id); if (!target) throw new Error(`未知目标 ${id}`); return target; }
async function json(route: Route, body: unknown) { await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) }); }
async function openedUrls(page: Page) { return page.evaluate(() => (window as typeof window & { __openedUrls: string[] }).__openedUrls); }
async function assertNoHorizontalOverflow(page: Page) { const metrics = await page.evaluate(() => ({ viewport: window.innerWidth, document: document.documentElement.scrollWidth, body: document.body.scrollWidth })); expect(metrics.document).toBeLessThanOrEqual(metrics.viewport); expect(metrics.body).toBeLessThanOrEqual(metrics.viewport); }
