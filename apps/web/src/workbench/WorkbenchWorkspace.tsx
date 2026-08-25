import { CheckCircle2, Clapperboard, Layers3 } from "lucide-react";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useSearchParams } from "react-router";
import { Badge, Tabs, TabsContent, TabsList, TabsTrigger } from "../shared/ui";
import { queryKeys, workbenchApi } from "./api";
import type { RunDetail } from "./contracts";
import { queryClient } from "../app/query-client";
import { CreativeBriefPanel } from "./creative-brief";
import { SkillRoutePanel } from "./skill-route";
import { RunPanel } from "./run";
import { StoryboardView } from "./storyboard";
import { AssetBibleView, WorkflowView } from "./workflow-panel";

type WorkbenchView = "storyboard" | "workflow" | "asset-bible";

function isWorkbenchView(value: string | null): value is WorkbenchView {
  return (
    value === "storyboard" || value === "workflow" || value === "asset-bible"
  );
}

export function WorkbenchWorkspace({ projectId }: { projectId: string }) {
  const [searchParams, setSearchParams] = useSearchParams();
  const [run, setRun] = useState<RunDetail | null>(null);
  const selectedEpisodeId = searchParams.get("episodeId");
  const view = isWorkbenchView(searchParams.get("view"))
    ? searchParams.get("view")!
    : "storyboard";
  const creative = useQuery({
    queryKey: queryKeys.creative(projectId),
    queryFn: () => workbenchApi.getCreative(projectId),
    enabled: Boolean(projectId),
  });
  const episodes = useQuery({
    queryKey: queryKeys.episodes(projectId),
    queryFn: () => workbenchApi.listEpisodes(projectId),
    enabled: Boolean(projectId),
  });
  const storyboard = useQuery({
    queryKey: selectedEpisodeId
      ? queryKeys.storyboard(projectId, selectedEpisodeId)
      : ["disabled"],
    queryFn: () =>
      workbenchApi.getStoryboard(projectId, selectedEpisodeId as string),
    enabled: Boolean(selectedEpisodeId),
  });
  const workflow = useQuery({
    queryKey: queryKeys.workflow(projectId),
    queryFn: () => workbenchApi.getWorkflow(projectId),
    enabled: view === "workflow",
  });
  const episodeList = [...(episodes.data ?? [])].sort(
    (left, right) =>
      left.number - right.number || left.id.localeCompare(right.id),
  );
  const selectedEpisode = episodeList.find(
    (episode) => episode.id === selectedEpisodeId,
  );
  const setEpisode = (episodeId: string) => {
    const params = new URLSearchParams(searchParams);
    if (episodeId) params.set("episodeId", episodeId);
    else params.delete("episodeId");
    setSearchParams(params);
  };
  const setView = (next: string) => {
    if (!isWorkbenchView(next)) return;
    const params = new URLSearchParams(searchParams);
    params.set("view", next);
    setSearchParams(params);
  };

  return (
    <div className="flex h-full min-h-0 flex-col bg-muted/20">
      <header className="shrink-0 border-b border-border bg-background">
        <div className="flex flex-wrap items-start justify-between gap-4 px-4 py-5 sm:px-6 lg:px-8">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-xs font-semibold tracking-wide text-primary">
              <Clapperboard aria-hidden="true" className="size-4" />
              项目工作区
            </div>
            <h2 className="mt-2 text-2xl font-semibold tracking-tight">
              把创作意图变成可审核的镜头
            </h2>
            <p className="mt-1 max-w-3xl text-sm text-muted-foreground">
              在同一个项目范围内完成创作设定、方案选择和分镜查看；所有写入都由明确操作触发。
            </p>
          </div>
          <div
            className="flex flex-1 flex-wrap items-end justify-end gap-x-4 gap-y-2"
            data-testid="workbench-context"
          >
            <label className="grid min-w-52 gap-1 text-sm font-medium">
              当前剧集
              <select
                className="h-10 rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
                value={selectedEpisodeId ?? ""}
                onChange={(event) => setEpisode(event.target.value)}
              >
                <option value="">选择剧集</option>
                {episodeList.map((episode) => (
                  <option key={episode.id} value={episode.id}>
                    {String(episode.number).padStart(2, "0")} / {episode.title}
                  </option>
                ))}
              </select>
            </label>
            {selectedEpisode && (
              <span className="pb-2 text-xs text-muted-foreground">
                第 {selectedEpisode.number} 集 · 版本 {selectedEpisode.revision}
              </span>
            )}
            <span className="inline-flex items-center gap-1.5 pb-2 text-xs text-muted-foreground">
              <CheckCircle2
                aria-hidden="true"
                className="size-4 text-success"
              />
              当前页面不会自动写入数据
            </span>
            <Badge variant="secondary">
              {creative.data?.creationMode === "adaptation" ? "改编" : "原创"}
            </Badge>
          </div>
        </div>
      </header>
      <div
        className="min-h-0 flex-1 overflow-y-auto"
        data-testid="workbench-scroll-region"
      >
        <div
          className="grid w-full gap-6 p-4 sm:p-6 lg:p-8"
          data-testid="workbench-canvas"
        >
          <div className="flex flex-wrap items-end justify-between gap-3">
            <div>
              <h3 className="text-lg font-semibold">制作工作区</h3>
              <p className="mt-1 text-sm text-muted-foreground">
                项目数据、生成任务和视觉投影分区展示，菜单始终保持在左侧。
              </p>
            </div>
            {creative.data?.creativeBrief && (
              <span className="text-xs text-muted-foreground">
                简报 {creative.data.creativeBrief.revision} · 项目版本{" "}
                {creative.data.projectRevision}
              </span>
            )}
          </div>
          <div className="grid gap-4 xl:grid-cols-[minmax(0,1.15fr)_minmax(20rem,0.85fr)]">
            <CreativeBriefPanel projectId={projectId} creative={creative} />
            <RunPanel
              projectId={projectId}
              currentBrief={Boolean(creative.data?.creativeBrief)}
              run={run}
              onRunChange={setRun}
            />
          </div>
          <SkillRoutePanel projectId={projectId} />
          <section
            className="grid gap-4 border-t border-border pt-6"
            aria-labelledby="projection-heading"
          >
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <div className="flex items-center gap-2 text-xs font-semibold tracking-wide text-muted-foreground">
                  <Layers3 aria-hidden="true" className="size-4" />
                  项目投影
                </div>
                <h3
                  id="projection-heading"
                  className="mt-1 text-lg font-semibold"
                >
                  剧集的统一事实视图
                </h3>
              </div>
              {selectedEpisode && (
                <span className="text-xs text-muted-foreground">
                  当前剧集：{selectedEpisode.title}
                </span>
              )}
            </div>
            <Tabs value={view} onValueChange={setView} className="grid gap-4">
              <TabsList aria-label="项目投影视图">
                <TabsTrigger value="storyboard">分镜</TabsTrigger>
                <TabsTrigger value="workflow">工作流</TabsTrigger>
                <TabsTrigger value="asset-bible">资产设定</TabsTrigger>
              </TabsList>
              <TabsContent value="storyboard">
                <StoryboardView
                  episodeId={selectedEpisodeId}
                  state={storyboard}
                  projectId={projectId}
                />
              </TabsContent>
              <TabsContent value="workflow">
                <WorkflowView state={workflow} />
              </TabsContent>
              <TabsContent value="asset-bible">
                <AssetBibleView projectId={projectId} />
              </TabsContent>
            </Tabs>
          </section>
        </div>
      </div>
    </div>
  );
}

export { queryClient };
