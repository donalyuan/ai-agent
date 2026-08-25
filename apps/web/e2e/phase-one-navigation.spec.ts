import path from "node:path";

import { test, expect } from "@playwright/test";

const projectId =
  process.env.E2E_PROJECT_ID ?? "1bd307e2-28dd-4a41-a4f4-b2cd10fcfdab";

function workflowFixture() {
  const nodes = Array.from({ length: 300 }, (_, index) => ({
    key: `fixture.node.${String(index + 1).padStart(3, "0")}`,
    scope: {
      projectId,
      episodeId: `episode-${String(Math.floor(index / 150) + 1).padStart(2, "0")}`,
      sceneId: `scene-${String(Math.floor(index / 25) + 1).padStart(2, "0")}`,
      shotId: `shot-${String(index + 1).padStart(3, "0")}`,
    },
    ports: { input: "fixture.input.v1", output: "fixture.output.v1" },
  }));
  return {
    id: "fixture-workflow-300",
    projectId,
    templateKey: "drama-mvp-a-default",
    scopeType: "project",
    scopeIds: [projectId],
    definition: {
      nodes,
      schemaVersion: "1.0.0",
      skills: ["novel-writing", "drama-skills"],
    },
    revision: 1,
    contentHash: "f".repeat(64),
    status: "published",
    versionNumber: 1,
    schemaVersion: "1.0.0",
    bindingId: "fixture-binding",
    bindingRevision: 1,
  };
}

test("shared project navigation stays read-only and renders the fixed workflow projection", async ({
  page,
  baseURL,
}, testInfo) => {
  const writes: string[] = [];
  page.on("request", (request) => {
    if (request.method() !== "GET")
      writes.push(`${request.method()} ${request.url()}`);
  });
  await page.route("**/api/v1/projects/*/workflow/default", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(workflowFixture()),
    });
  });

  await page.goto(`${baseURL}/projects/${projectId}/workbench`);
  await expect(page.getByText("Mock + Local offline")).toBeVisible();
  await expect(page.getByText("adapter: local_workspace")).toBeVisible();
  await page.getByRole("tab", { name: "Workflow source" }).click();
  await expect(page.getByTestId("workflow-projection")).toHaveAttribute(
    "data-node-count",
    "300",
  );
  await expect
    .poll(() => page.locator(".react-flow__node").count())
    .toBeGreaterThan(0);
  await expect
    .poll(() => page.locator(".react-flow__node").count())
    .toBeLessThan(300);

  const routes = [
    ["候选审核", "候选审片台"],
    ["项目资产", "素材库"],
    ["集时间线", "先选择一集"],
    ["项目导出", "逐集导出"],
    ["模型设置", "模型与能力"],
  ] as const;
  for (const [label, heading] of routes) {
    await page.getByRole("link", { name: label }).click();
    await expect(page.getByRole("heading", { name: heading })).toBeVisible();
    await expect(page).toHaveURL(new RegExp(`/projects/${projectId}/`));
  }
  expect(writes, `navigation generated writes: ${writes.join(", ")}`).toEqual(
    [],
  );

  await page.screenshot({
    path: path.resolve(
      process.cwd(),
      "../../output/playwright",
      `phase-one-${testInfo.project.name}-navigation.png`,
    ),
    fullPage: true,
  });
});
