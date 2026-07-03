import { expect, test } from "@playwright/test";

test("admin 首屏是平台管理后台，不展示视频生产流程", async ({ page }) => {
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
});
