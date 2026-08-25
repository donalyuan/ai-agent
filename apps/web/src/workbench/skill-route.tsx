import { Check, RefreshCw, Route as RouteIcon } from "lucide-react";
import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Button, CardContent } from "../shared/ui";
import { queryKeys, workbenchApi } from "./api";
import { queryClient } from "../app/query-client";
import { formatRevision } from "./display";
import { WorkbenchNotice, WorkbenchPanel, WorkbenchPanelHeader } from "./ui";

export function SkillRoutePanel({ projectId }: { projectId: string }) {
  const [route, setRoute] = useState<Awaited<
    ReturnType<typeof workbenchApi.resolveSkillRoute>
  > | null>(null);
  const resolve = useMutation({
    mutationFn: () => workbenchApi.resolveSkillRoute(projectId),
    onSuccess: setRoute,
  });
  const select = useMutation({
    mutationFn: (candidate: { name: string; version: string }) => {
      if (!route) return Promise.reject(new Error("请先读取可用方案。"));
      return workbenchApi.selectSkillRoute(
        projectId,
        route.id,
        candidate.name,
        candidate.version,
        route.revision,
      );
    },
    onSuccess: () =>
      void queryClient.invalidateQueries({
        queryKey: queryKeys.skillRoutes(projectId),
      }),
  });

  return (
    <WorkbenchPanel>
      <WorkbenchPanelHeader
        icon={<RouteIcon aria-hidden="true" className="size-4" />}
        label="文本方案"
        title="选择一次并固定版本"
        detail="只有明确请求候选并完成选择，文本生成任务才会携带固定方案。"
        trailing={
          <span className="text-xs text-muted-foreground">
            {route ? formatRevision(route.revision) : "未读取"}
          </span>
        }
      />
      <CardContent className="grid gap-4">
        <Button
          className="w-fit"
          variant="outline"
          disabled={resolve.isPending}
          onClick={() => resolve.mutate()}
        >
          {resolve.isPending ? "正在读取…" : "读取可用方案"}{" "}
          <RefreshCw aria-hidden="true" />
        </Button>
        {resolve.error && (
          <WorkbenchNotice tone="danger">
            {resolve.error instanceof Error
              ? resolve.error.message
              : "读取方案失败，请重试。"}
          </WorkbenchNotice>
        )}
        {route && (
          <div className="grid gap-3 rounded-md border border-border bg-muted/40 p-4">
            <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
              <span>候选集合 · {formatRevision(route.revision)}</span>
              <span>
                {route.needsManualSelection ? "需要手动选择" : "可以固定"}
              </span>
            </div>
            {route.fallbackReason && (
              <WorkbenchNotice tone="warning">
                {route.fallbackReason}
              </WorkbenchNotice>
            )}
            {route.candidates.map((candidate, index) => (
              <div
                className="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-3 first:border-t-0 first:pt-0"
                key={`${candidate.name}@${candidate.version}`}
              >
                <div className="grid gap-1">
                  <strong>候选方案 {index + 1}</strong>
                  <span className="text-xs text-muted-foreground">
                    版本 {candidate.version} · 匹配度 {candidate.score}
                  </span>
                </div>
                <Button
                  variant="outline"
                  disabled={select.isPending}
                  onClick={() => select.mutate(candidate)}
                >
                  固定这个方案 <Check aria-hidden="true" />
                </Button>
              </div>
            ))}
            {select.data !== undefined && (
              <WorkbenchNotice tone="success">
                方案选择已固定，后续任务会使用服务返回的版本。
              </WorkbenchNotice>
            )}
            {select.error && (
              <WorkbenchNotice tone="danger">
                {select.error instanceof Error
                  ? select.error.message
                  : "固定方案失败，请重试。"}
              </WorkbenchNotice>
            )}
          </div>
        )}
      </CardContent>
    </WorkbenchPanel>
  );
}
