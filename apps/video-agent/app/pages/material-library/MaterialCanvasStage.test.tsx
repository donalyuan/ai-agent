import { render, screen } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import type { Material } from "../../lib/api";

vi.mock("react-konva/es/ReactKonvaCore", async () => {
  const React = await import("react");
  const MockNode = ({ children, className, text }: { children?: ReactNode; className?: string; text?: string }) =>
    React.createElement("div", className ? { className } : null, children ?? text ?? null);
  const Group = ({ children, x, y }: { children?: ReactNode; x?: number; y?: number }) =>
    React.createElement("div", { "data-konva-group": "", "data-x": x, "data-y": y }, children);
  const Text = ({ ellipsis, height, text, wrap }: { ellipsis?: boolean; height?: number; text?: string; wrap?: string }) =>
    React.createElement(
      "span",
      {
        "data-ellipsis": ellipsis ? "true" : "false",
        "data-height": height,
        "data-wrap": wrap,
      },
      text,
    );
  return { Stage: MockNode, Layer: MockNode, Group, Rect: MockNode, Text, Image: MockNode };
});

import { MaterialCanvasStage, materialCanvasNodes } from "./MaterialCanvasStage";

const longFileName = "别硬扛，用Debug解决烦心事-镜头01-第01张.jpg";

function material(index: number): Material {
  return {
    material_id: `abababab-abab-4aba-8aba-abababababa${index}`,
    project_id: "11111111-1111-4111-8111-111111111111",
    material_type: "image",
    file_url: `https://cdn.example.com/${index}.jpg`,
    thumbnail_url: null,
    file_name: index === 0 ? longFileName : `素材-${index}.jpg`,
  tags: [],
  metadata: {},
  source: null,
  audio_usage: null,
  work_id: null,
  work_version_id: null,
  generation: null,
  usage_count: 0,
    status: "active",
    created_at: "2026-07-14T00:00:00Z",
    updated_at: "2026-07-14T00:00:00Z",
  };
}

describe("MaterialCanvasStage", () => {
  it("详情关闭后使用释放的宽度增加同一行节点数", () => {
    const materials = [material(0), material(1), material(2), material(3)];

    const openNodes = materialCanvasNodes(materials, 1200, true);
    const closedNodes = materialCanvasNodes(materials, 1200, false);

    expect(openNodes[2]?.y).toBeGreaterThan(openNodes[0]?.y || 0);
    expect(closedNodes[2]?.y).toBe(closedNodes[0]?.y);
  });

  it("详情打开时所有节点都位于右侧抽屉安全区之外", () => {
    render(
      createElement(MaterialCanvasStage, {
        detailOpen: true,
        height: 720,
        materials: [material(0), material(1), material(2), material(3)],
        selectedMaterialId: material(0).material_id,
        width: 1000,
        onSelectMaterial: vi.fn(),
      }),
    );

    const nodeGroups = document.querySelectorAll("[data-konva-group]");
    expect(nodeGroups).toHaveLength(4);
    for (const nodeGroup of nodeGroups) {
      const x = Number(nodeGroup.getAttribute("data-x"));
      expect(x + 206).toBeLessThanOrEqual(624);
    }
  });

  it("长文件名限制在固定两行标题区并启用省略", () => {
    render(
      createElement(MaterialCanvasStage, {
        detailOpen: false,
        height: 720,
        materials: [material(0)],
        selectedMaterialId: null,
        width: 1200,
        onSelectMaterial: vi.fn(),
      }),
    );

    const title = screen.getByText(longFileName);
    expect(title).toHaveAttribute("data-height", "32");
    expect(title).toHaveAttribute("data-wrap", "char");
    expect(title).toHaveAttribute("data-ellipsis", "true");
  });
});
