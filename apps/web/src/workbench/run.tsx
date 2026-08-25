import {
  ArrowRight,
  CircleAlert,
  History,
  RefreshCw,
  Timer,
} from "lucide-react";
import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Button, CardContent } from "../shared/ui";
import { OwnerApiError, workbenchApi } from "./api";
import type { RunDetail } from "./contracts";
import { operationLabel, statusLabel } from "./display";
import {
  WorkbenchNotice,
  WorkbenchPanel,
  WorkbenchPanelHeader,
  WorkbenchStatus,
} from "./ui";

export function RunPanel({
  projectId,
  currentBrief,
  run,
  onRunChange,
}: {
  projectId: string;
  currentBrief: boolean;
  run: RunDetail | null;
  onRunChange: (run: RunDetail) => void;
}) {
  const [message, setMessage] = useState<string | null>(null);
  const start = useMutation({
    mutationFn: async () => {
      const routes = await workbenchApi.listSkillRoutes(projectId);
      if (routes.length !== 1)
        throw new OwnerApiError(
          409,
          "skill_route_required",
          "请先完成一次文本方案选择。",
        );
      const source = await workbenchApi.ensureWorkflow(projectId);
      return workbenchApi.startRun(
        projectId,
        source.id,
        source.bindingRevision,
        `text:${projectId}:${Date.now()}`,
        routes[0].id,
      );
    },
    onSuccess: (value) => {
      setMessage("文本生成任务已创建。");
      onRunChange(value);
    },
    onError: (error) =>
      setMessage(
        error instanceof Error ? error.message : "创建任务失败，请重试。",
      ),
  });

  return (
    <WorkbenchPanel>
      <WorkbenchPanelHeader
        icon={<Timer aria-hidden="true" className="size-4" />}
        label="文本生成任务"
        title={run ? `任务 ${run.id.slice(0, 8)}` : "尚未创建生成任务"}
        detail="创建任务前会重新确认唯一方案和已发布工作流。"
        trailing={<WorkbenchStatus value={run?.status ?? "idle"} />}
      />
      <CardContent className="grid gap-4">
        <div
          className="h-1.5 overflow-hidden rounded-full bg-muted"
          aria-label="任务进度"
        >
          <div
            className={`h-full rounded-full bg-primary transition-all ${run ? "w-2/3" : "w-0"}`}
          />
        </div>
        <dl className="grid gap-3 rounded-md border border-border bg-muted/40 p-4 text-sm sm:grid-cols-2">
          <div className="grid gap-1">
            <dt className="text-muted-foreground">工作流版本</dt>
            <dd className="font-medium">默认戏剧流程 · 已发布</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">执行配置</dt>
            <dd className="font-medium">本地离线配置</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">存储位置</dt>
            <dd className="font-medium">本地工作区</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">费用门槛</dt>
            <dd className="font-medium text-warning-foreground">需要确认</dd>
          </div>
        </dl>
        <Button
          className="w-full"
          disabled={!currentBrief || start.isPending}
          onClick={() => start.mutate()}
        >
          {start.isPending
            ? "正在确认并创建…"
            : run
              ? "再次读取任务"
              : "明确创建文本任务"}{" "}
          <ArrowRight aria-hidden="true" />
        </Button>
        {message && (
          <WorkbenchNotice tone={start.error ? "danger" : "success"}>
            {message}
          </WorkbenchNotice>
        )}
        {run && (
          <RunSummary projectId={projectId} run={run} onUpdate={onRunChange} />
        )}
      </CardContent>
    </WorkbenchPanel>
  );
}

function RunSummary({
  projectId,
  run,
  onUpdate,
}: {
  projectId: string;
  run: RunDetail;
  onUpdate: (run: RunDetail) => void;
}) {
  const [error, setError] = useState<unknown>(null);
  const [snapshotId, setSnapshotId] = useState<string | null>(null);
  const canCancel =
    run.status === "queued" ||
    run.status === "running" ||
    run.status === "waiting_review";
  const events = useQuery({
    queryKey: ["runs", run.id, "events", run.latestEventSequence],
    queryFn: () =>
      workbenchApi.getRunEvents(projectId, run.id, run.latestEventSequence),
    enabled: false,
  });
  const snapshots = useQuery({
    queryKey: ["projects", projectId, "run-input-snapshots"],
    queryFn: () => workbenchApi.listRunInputSnapshots(projectId),
    enabled: false,
  });
  const cancel = useMutation({
    mutationFn: () => workbenchApi.cancelRun(projectId, run.id, run.revision),
    onSuccess: onUpdate,
    onError: setError,
  });
  const successor = useMutation({
    mutationFn: () =>
      workbenchApi.createSuccessorRun(projectId, run.id, run.revision),
    onSuccess: onUpdate,
    onError: setError,
  });
  const rerun = useMutation({
    mutationFn: () => {
      if (!snapshotId)
        return Promise.reject(new Error("请先选择历史输入快照。"));
      const item = (
        snapshots.data as Array<{ id: string; revision: number }> | undefined
      )?.find((value) => value.id === snapshotId);
      if (!item) return Promise.reject(new Error("所选快照不属于当前项目。"));
      return workbenchApi.rerunHistorical(projectId, item.id, item.revision);
    },
    onSuccess: onUpdate,
    onError: setError,
  });

  return (
    <div className="grid gap-4 rounded-md border border-border bg-background p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="grid gap-1">
          <span className="text-xs font-semibold tracking-wide text-muted-foreground">
            任务详情
          </span>
          <span className="font-mono text-xs text-muted-foreground">
            {run.id}
          </span>
        </div>
        <WorkbenchStatus value={run.status} />
      </div>
      <div className="grid gap-2 text-sm sm:grid-cols-3">
        <span className="text-muted-foreground">
          最新事件 <b className="text-foreground">{run.latestEventSequence}</b>
        </span>
        <span className="text-muted-foreground">
          工作流版本{" "}
          <b className="text-foreground">{run.workflowVersionNumber}</b>
        </span>
        <span className="text-muted-foreground">
          节点数量 <b className="text-foreground">{run.nodes.length}</b>
        </span>
      </div>
      {run.failure && (
        <WorkbenchNotice tone="warning">
          <span>
            任务失败：{String(run.failure.message ?? "请查看任务详情")}
          </span>
        </WorkbenchNotice>
      )}
      <div className="flex flex-wrap gap-2">
        {canCancel && (
          <Button
            variant="destructive"
            disabled={cancel.isPending}
            onClick={() => cancel.mutate()}
          >
            <CircleAlert aria-hidden="true" />
            {cancel.isPending ? "正在取消…" : "取消任务"}
          </Button>
        )}
        <Button variant="outline" onClick={() => void events.refetch()}>
          <RefreshCw aria-hidden="true" />
          读取新事件
        </Button>
        {run.allowedActions.createSuccessor && (
          <Button
            variant="outline"
            disabled={successor.isPending}
            onClick={() => successor.mutate()}
          >
            <ArrowRight aria-hidden="true" />
            从失败节点继续
          </Button>
        )}
        <Button variant="outline" onClick={() => void snapshots.refetch()}>
          <History aria-hidden="true" />
          历史输入快照
        </Button>
      </div>
      {events.error && (
        <WorkbenchNotice tone="danger">
          {events.error instanceof Error
            ? events.error.message
            : "读取事件失败，请重试。"}
        </WorkbenchNotice>
      )}
      {events.data && (
        <div className="flex flex-wrap gap-2" aria-label="任务事件">
          {events.data.map((event) => (
            <span
              className="rounded border border-border px-2 py-1 font-mono text-xs text-muted-foreground"
              key={event.id}
            >
              第 {event.sequence} 条 · {operationLabel(event.eventType)}
            </span>
          ))}
        </div>
      )}
      {snapshots.data !== undefined && snapshots.data !== null && (
        <div className="grid gap-3 rounded-md border border-border bg-muted/40 p-3">
          <label className="grid gap-1.5 text-sm font-medium">
            选择历史输入快照
            <select
              className="h-10 rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              value={snapshotId ?? ""}
              onChange={(event) => setSnapshotId(event.target.value || null)}
            >
              <option value="">请选择</option>
              {(
                snapshots.data as Array<{
                  id: string;
                  revision: number;
                  runnable?: boolean;
                }>
              ).map((item) => (
                <option
                  key={item.id}
                  value={item.id}
                  disabled={item.runnable === false}
                >
                  {item.id.slice(0, 8)} · 第 {item.revision} 版
                  {item.runnable === false ? " · 不可运行" : ""}
                </option>
              ))}
            </select>
          </label>
          <Button
            className="w-fit"
            variant="outline"
            disabled={!snapshotId || rerun.isPending}
            onClick={() => rerun.mutate()}
          >
            {rerun.isPending ? "正在创建…" : "按准确快照重新运行"}
          </Button>
        </div>
      )}
      <div className="grid gap-2">
        {run.nodes.map((node) => (
          <div
            className="grid gap-2 rounded border border-border px-3 py-2 text-sm sm:grid-cols-[minmax(0,1fr)_auto_auto] sm:items-center"
            key={node.id}
          >
            <span className="font-mono text-xs text-muted-foreground">
              {operationLabel(node.nodeKey)}
            </span>
            <WorkbenchStatus value={node.status} />
            <span className="text-xs text-muted-foreground">
              {operationLabel(node.logicalOperation)}
            </span>
          </div>
        ))}
      </div>
      {error != null && (
        <WorkbenchNotice tone="danger">
          {error instanceof Error ? error.message : "操作失败，请重试。"}
        </WorkbenchNotice>
      )}
    </div>
  );
}

export { statusLabel };
