import type { Edge, Node } from "@xyflow/react";

type WorkflowNode = Record<string, unknown>;

type WorkflowNodeData = WorkflowNode & {
  label: string;
  source: WorkflowNode;
  frozenScope: unknown;
  frozenSelection: unknown;
};

type WorkflowProjectionData = {
  nodes: Node<WorkflowNodeData>[];
  edges: Edge[];
};

const workflowLabels: Record<string, string> = {
  "text.generate": "文本生成",
  "text.review": "文本审核",
  "image.generate": "画面生成",
  "video.submit": "视频提交",
};

function asWorkflowNode(value: unknown): WorkflowNode {
  return value && typeof value === "object"
    ? (value as WorkflowNode)
    : { value };
}

function projectWorkflowNodes(
  sourceNodes: readonly unknown[],
): WorkflowProjectionData {
  const nodes: Node<WorkflowNodeData>[] = sourceNodes.map((value, index) => {
    const source = asWorkflowNode(value);
    const fallbackId = `node-${index + 1}`;
    const sourceKey = String(source.key ?? source.id ?? "");
    const frozenScope =
      source.frozenScope ?? source.scope ?? source.scopeRefs ?? null;
    const frozenSelection =
      source.frozenSelection ??
      source.selection ??
      source.selectionSnapshot ??
      null;

    return {
      id: String(source.id ?? source.key ?? fallbackId),
      data: {
        ...source,
        label: workflowLabels[sourceKey] ?? `流程节点 ${index + 1}`,
        source,
        frozenScope,
        frozenSelection,
      },
      position: { x: (index % 4) * 190, y: Math.floor(index / 4) * 76 },
      draggable: false,
      connectable: false,
      selectable: true,
      deletable: false,
      focusable: true,
      // React Flow cannot cull nodes whose bounds are unknown during initial layout.
      width: 172,
      height: 54,
      style: { borderRadius: 6, borderColor: "var(--ui-border)", fontSize: 12 },
    };
  });
  const edges: Edge[] = nodes.slice(1).map((node, index) => ({
    id: `edge-${index}`,
    source: nodes[index].id,
    target: node.id,
    selectable: false,
    focusable: false,
    deletable: false,
    reconnectable: false,
  }));
  return { nodes, edges };
}

export { projectWorkflowNodes };
export type { WorkflowNode, WorkflowNodeData };
