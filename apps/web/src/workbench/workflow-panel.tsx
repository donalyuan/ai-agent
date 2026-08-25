import { Boxes, CircleAlert, Workflow } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { VirtualList } from "../shared/ui";
import { queryKeys } from "./api";
import { WorkflowProjection } from "./workflow-projection";
import { WorkbenchNotice, WorkbenchQueryNotice, WorkbenchStatus } from "./ui";

export function WorkflowView({
  state,
}: {
  state: {
    data?: {
      definition: { nodes: unknown[] };
      contentHash: string;
      revision: number;
      versionNumber: number;
      templateKey: string;
    };
    isPending: boolean;
    error: unknown;
  };
}) {
  if (state.isPending) return <WorkbenchQueryNotice isPending error={null} />;
  if (state.error)
    return <WorkbenchQueryNotice isPending={false} error={state.error} />;
  if (!state.data)
    return (
      <div className="grid place-items-center gap-2 rounded-md border border-dashed border-border px-6 py-12 text-center">
        <Workflow aria-hidden="true" className="size-6 text-muted-foreground" />
        <strong>尚未读取工作流</strong>
        <span className="text-sm text-muted-foreground">
          只有用户明确创建任务时才会生成或升级工作流。
        </span>
      </div>
    );
  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center gap-3 rounded-md border border-border bg-muted/40 p-3 text-sm">
        <WorkbenchStatus value="published" />
        <strong>
          {state.data.templateKey === "drama-mvp-a-default"
            ? "默认戏剧流程"
            : "已发布流程"}
        </strong>
        <span className="text-xs text-muted-foreground">
          版本 {state.data.versionNumber} · 第 {state.data.revision} 版 ·
          内容校验已固定
        </span>
      </div>
      <WorkflowProjection nodes={state.data.definition.nodes} />
      <VirtualList
        ariaLabel="已发布工作流节点"
        className="max-h-72 rounded-md border border-border"
        getKey={(node, index) =>
          typeof node === "object" && node && "key" in node
            ? String((node as { key: unknown }).key)
            : `node-${index + 1}`
        }
        items={state.data.definition.nodes}
        renderItem={(_node, index) => (
          <div className="flex items-center justify-between gap-3 border-b border-border px-3 py-2 text-sm last:border-b-0">
            <span>节点 {String(index + 1).padStart(2, "0")}</span>
            <span className="text-xs text-muted-foreground">仅供查看</span>
          </div>
        )}
      />
      <WorkbenchNotice>
        <span className="inline-flex items-center gap-2">
          <CircleAlert aria-hidden="true" className="size-4" />
          流程编辑、连线和发布属于后续阶段，本页没有对应写入操作。
        </span>
      </WorkbenchNotice>
    </div>
  );
}

export function AssetBibleView({ projectId }: { projectId: string }) {
  const entries = useQuery({
    queryKey: queryKeys.bible(projectId),
    queryFn: async () => {
      const response = await fetch(
        `/api/v1/projects/${projectId}/asset-bible/entries`,
        { headers: { "X-Project-Scope": projectId } },
      );
      if (!response.ok)
        throw new Error(`资产设定服务暂时不可用（${response.status}）。`);
      return response.json() as Promise<unknown[]>;
    },
    enabled: Boolean(projectId),
  });
  if (entries.isPending) return <WorkbenchQueryNotice isPending error={null} />;
  if (entries.error)
    return <WorkbenchQueryNotice isPending={false} error={entries.error} />;
  if (entries.data?.length === 0)
    return (
      <div className="grid place-items-center gap-2 rounded-md border border-dashed border-border px-6 py-12 text-center">
        <Boxes aria-hidden="true" className="size-6 text-muted-foreground" />
        <strong>还没有资产设定</strong>
        <span className="text-sm text-muted-foreground">
          设定条目和版本由资产设定服务管理，工作台不会自动创建。
        </span>
      </div>
    );
  return (
    <div className="grid gap-2">
      {entries.data?.map((_entry, index) => (
        <div
          className="flex items-center gap-3 rounded-md border border-border px-3 py-3"
          key={index}
        >
          <span className="grid size-8 place-items-center rounded bg-primary/10 text-primary">
            <Boxes aria-hidden="true" className="size-4" />
          </span>
          <span className="grid gap-1">
            <strong>资产设定 {index + 1}</strong>
            <small className="text-xs text-muted-foreground">
              服务投影 · 不可变版本
            </small>
          </span>
          <WorkbenchStatus value="ready" />
        </div>
      ))}
    </div>
  );
}
