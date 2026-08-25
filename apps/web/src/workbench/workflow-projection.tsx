import { Background, Controls, ReactFlow } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { LockKeyhole } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { projectWorkflowNodes } from "./workflow-projection-model";

function WorkflowProjection({
  nodes: sourceNodes,
}: {
  nodes: readonly unknown[];
}) {
  const projection = useMemo(
    () => projectWorkflowNodes(sourceNodes),
    [sourceNodes],
  );
  const [onlyRenderVisibleElements, setOnlyRenderVisibleElements] =
    useState(false);

  useEffect(() => {
    // React Flow needs one full layout pass to populate node handle bounds. Once
    // measured, re-enabling culling removes offscreen nodes from the real DOM.
    const firstFrame = requestAnimationFrame(() => {
      requestAnimationFrame(() => setOnlyRenderVisibleElements(true));
    });
    return () => cancelAnimationFrame(firstFrame);
  }, [projection.nodes]);

  if (projection.nodes.length === 0) {
    return (
      <p className="py-8 text-center text-sm text-muted-foreground">
        没有已发布的工作流节点
      </p>
    );
  }

  return (
    <div className="grid gap-2">
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <LockKeyhole className="size-4 text-success" />{" "}
        已发布版本的只读视图，图形编辑属于后续版本。
      </div>
      <div
        className="h-[360px] overflow-hidden rounded-md border border-border"
        data-node-count={projection.nodes.length}
        data-testid="workflow-projection"
      >
        <ReactFlow
          edges={projection.edges}
          edgesReconnectable={false}
          deleteKeyCode={null}
          defaultViewport={{ x: 0, y: 0, zoom: 1 }}
          maxZoom={1.15}
          minZoom={0.35}
          nodes={projection.nodes}
          nodesConnectable={false}
          nodesDraggable={false}
          nodesFocusable
          onlyRenderVisibleElements={onlyRenderVisibleElements}
          elementsSelectable
          onBeforeDelete={async () => false}
          onConnect={() => undefined}
          onEdgesChange={() => undefined}
          onNodesChange={() => undefined}
          panOnDrag
          proOptions={{ hideAttribution: true }}
        >
          <Background gap={18} size={1} />
          <Controls showInteractive={false} />
        </ReactFlow>
      </div>
    </div>
  );
}

export { WorkflowProjection };
