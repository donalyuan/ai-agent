import {
  Activity,
  ArrowLeft,
  ArrowRight,
  Check,
  ChevronDown,
  Clapperboard,
  CircleAlert,
  Layers3,
} from "lucide-react";
import { useMutation } from "@tanstack/react-query";
import { Link } from "react-router";
import { Button } from "../shared/ui";
import { queryKeys, workbenchApi } from "./api";
import { queryClient } from "../app/query-client";
import type { SceneProjection } from "./contracts";
import {
  episodeSliceKey,
  EMPTY_EPISODE_SLICE,
  usePresentationStore,
} from "./presentation-store";
import { WorkbenchNotice, WorkbenchQueryNotice, WorkbenchStatus } from "./ui";

function moveItem<T>(items: T[], from: number, to: number): T[] {
  const next = items.slice();
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

export function StoryboardView({
  episodeId,
  state,
  projectId,
}: {
  episodeId: string | null;
  state: { data?: SceneProjection[]; isPending: boolean; error: unknown };
  projectId: string;
}) {
  const slice = usePresentationStore((store) =>
    episodeId
      ? (store.slices[episodeSliceKey(projectId, episodeId)] ??
        EMPTY_EPISODE_SLICE)
      : null,
  );
  const patchSlice = usePresentationStore((store) => store.patchSlice);
  const storyboardKey = episodeId
    ? queryKeys.storyboard(projectId, episodeId)
    : (["disabled"] as const);
  const reorderScenes = useMutation({
    mutationFn: (sceneIds: string[]) => {
      if (!episodeId) return Promise.reject(new Error("请先选择剧集。"));
      const current =
        queryClient.getQueryData<SceneProjection[]>(storyboardKey);
      const expectedRevision = current?.[0]?.sceneOrderRevision ?? 1;
      return workbenchApi.reorderScenes(
        projectId,
        episodeId,
        sceneIds,
        expectedRevision,
      );
    },
    onMutate: async (sceneIds) => {
      if (!episodeId) return undefined;
      await queryClient.cancelQueries({ queryKey: storyboardKey });
      const previous =
        queryClient.getQueryData<SceneProjection[]>(storyboardKey);
      if (previous) {
        const byId = new Map(
          previous.map((scene: SceneProjection) => [scene.id, scene]),
        );
        queryClient.setQueryData(
          storyboardKey,
          sceneIds.map((id, index) => ({
            ...byId.get(id)!,
            number: index + 1,
          })),
        );
      }
      return { previous };
    },
    onError: (_error, _sceneIds, context) => {
      if (context?.previous)
        queryClient.setQueryData(storyboardKey, context.previous);
      void queryClient.invalidateQueries({ queryKey: storyboardKey });
    },
    onSuccess: (data) => queryClient.setQueryData(storyboardKey, data),
  });
  const reorderShots = useMutation({
    mutationFn: ({
      scene,
      shotIds,
    }: {
      scene: SceneProjection;
      shotIds: string[];
    }) => {
      if (!episodeId) return Promise.reject(new Error("请先选择剧集。"));
      return workbenchApi.reorderShots(
        projectId,
        episodeId,
        scene.id,
        shotIds,
        scene.revision,
      );
    },
    onMutate: async ({ scene, shotIds }) => {
      if (!episodeId) return undefined;
      await queryClient.cancelQueries({ queryKey: storyboardKey });
      const previous =
        queryClient.getQueryData<SceneProjection[]>(storyboardKey);
      if (previous) {
        queryClient.setQueryData(
          storyboardKey,
          previous.map((item: SceneProjection) =>
            item.id !== scene.id
              ? item
              : {
                  ...item,
                  shots: shotIds.map((id, index) => ({
                    ...new Map(
                      item.shots.map(
                        (shot: SceneProjection["shots"][number]) => [
                          shot.id,
                          shot,
                        ],
                      ),
                    ).get(id)!,
                    number: index + 1,
                  })),
                },
          ),
        );
      }
      return { previous };
    },
    onError: (_error, _variables, context) => {
      if (context?.previous)
        queryClient.setQueryData(storyboardKey, context.previous);
      void queryClient.invalidateQueries({ queryKey: storyboardKey });
    },
    onSuccess: (data) => queryClient.setQueryData(storyboardKey, data),
  });

  if (!episodeId)
    return (
      <div className="grid place-items-center gap-2 rounded-md border border-dashed border-border px-6 py-12 text-center">
        <Activity aria-hidden="true" className="size-6 text-muted-foreground" />
        <strong>请先选择剧集</strong>
        <span className="text-sm text-muted-foreground">
          分镜、镜头和折叠状态都必须绑定明确的项目与剧集。
        </span>
      </div>
    );
  if (state.isPending) return <WorkbenchQueryNotice isPending error={null} />;
  if (state.error)
    return <WorkbenchQueryNotice isPending={false} error={state.error} />;
  const scenes = state.data ?? [];
  const filter = slice?.filters.status ?? "all";
  const toggleScene = (sceneId: string) => {
    const collapsed = new Set(slice?.collapsedSceneIds ?? []);
    if (collapsed.has(sceneId)) collapsed.delete(sceneId);
    else collapsed.add(sceneId);
    patchSlice(projectId, episodeId, { collapsedSceneIds: [...collapsed] });
  };
  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border pb-3">
        <span className="text-xs font-semibold tracking-wide text-muted-foreground">
          {scenes.length} 个场次 ·{" "}
          {scenes.reduce((sum, scene) => sum + scene.shots.length, 0)} 个镜头
        </span>
        <label className="flex items-center gap-2 text-sm font-medium">
          筛选状态
          <select
            className="h-9 rounded-md border border-input bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            value={filter}
            onChange={(event) =>
              patchSlice(projectId, episodeId, {
                filters: {
                  ...(slice?.filters ?? { model: "all", review: "all" }),
                  status: event.target.value,
                },
              })
            }
            aria-label="筛选镜头状态"
          >
            <option value="all">全部</option>
            <option value="ready">就绪</option>
            <option value="pending">等待处理</option>
            <option value="stale">需要刷新</option>
          </select>
        </label>
      </div>
      {scenes.length === 0 && (
        <div className="grid place-items-center gap-2 rounded-md border border-dashed border-border px-6 py-12 text-center">
          <Layers3
            aria-hidden="true"
            className="size-6 text-muted-foreground"
          />
          <strong>当前剧集还没有分镜</strong>
          <span className="text-sm text-muted-foreground">
            不会自动选择其他剧集，也不会创建模板或任务。
          </span>
        </div>
      )}
      {scenes.map((scene, sceneIndex) => {
        const collapsed = slice?.collapsedSceneIds.includes(scene.id) ?? false;
        const shots = scene.shots.filter(
          (shot) => filter === "all" || shot.status === filter,
        );
        return (
          <div
            className="overflow-hidden rounded-md border border-border"
            key={scene.id}
          >
            <div className="flex flex-wrap items-center gap-2 border-b border-border bg-muted/40 p-2">
              <Button
                className="h-auto min-h-12 min-w-0 flex-1 justify-start text-left"
                variant="ghost"
                onClick={() => toggleScene(scene.id)}
                aria-expanded={!collapsed}
              >
                <span className="rounded bg-primary/10 px-2 py-1 text-xs font-semibold text-primary">
                  场 {String(scene.number).padStart(2, "0")}
                </span>
                <span className="grid min-w-0 gap-1">
                  <strong className="truncate">
                    {scene.title || "未命名场次"}
                  </strong>
                  <small className="truncate font-mono text-xs text-muted-foreground">
                    {scene.id} · 第 {scene.revision} 版
                  </small>
                </span>
                <span className="ml-auto text-xs text-muted-foreground">
                  {shots.length} 个镜头
                </span>
                <ChevronDown
                  aria-hidden="true"
                  className={`size-4 transition-transform ${collapsed ? "-rotate-90" : ""}`}
                />
              </Button>
              <div className="flex gap-1" aria-label="场次排序">
                <Button
                  size="icon-sm"
                  variant="outline"
                  title="场次上移"
                  aria-label="场次上移"
                  disabled={sceneIndex === 0 || reorderScenes.isPending}
                  onClick={() =>
                    reorderScenes.mutate(
                      moveItem(
                        scenes.map((item) => item.id),
                        sceneIndex,
                        sceneIndex - 1,
                      ),
                    )
                  }
                >
                  <ArrowLeft aria-hidden="true" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="outline"
                  title="场次下移"
                  aria-label="场次下移"
                  disabled={
                    sceneIndex === scenes.length - 1 || reorderScenes.isPending
                  }
                  onClick={() =>
                    reorderScenes.mutate(
                      moveItem(
                        scenes.map((item) => item.id),
                        sceneIndex,
                        sceneIndex + 1,
                      ),
                    )
                  }
                >
                  <ArrowRight aria-hidden="true" />
                </Button>
              </div>
            </div>
            {!collapsed && (
              <div className="grid gap-3 p-3 sm:grid-cols-2 2xl:grid-cols-3">
                {shots.map((shot, shotIndex) => (
                  <ShotCard
                    key={shot.id}
                    projectId={projectId}
                    episodeId={episodeId}
                    scene={scene}
                    shot={shot}
                    canMoveUp={shotIndex > 0}
                    canMoveDown={shotIndex < shots.length - 1}
                    onMove={(direction) =>
                      reorderShots.mutate({
                        scene,
                        shotIds: moveItem(
                          scene.shots.map((item) => item.id),
                          shotIndex,
                          direction === "up" ? shotIndex - 1 : shotIndex + 1,
                        ),
                      })
                    }
                    onSelect={() =>
                      patchSlice(projectId, episodeId, {
                        selectedShotId: shot.id,
                      })
                    }
                  />
                ))}
              </div>
            )}
          </div>
        );
      })}
      {reorderScenes.error && (
        <WorkbenchNotice tone="danger">
          {reorderScenes.error instanceof Error
            ? reorderScenes.error.message
            : "场次排序失败，请重试。"}
        </WorkbenchNotice>
      )}
      {reorderShots.error && (
        <WorkbenchNotice tone="danger">
          {reorderShots.error instanceof Error
            ? reorderShots.error.message
            : "镜头排序失败，请重试。"}
        </WorkbenchNotice>
      )}
    </div>
  );
}

function ShotCard({
  projectId,
  episodeId,
  scene,
  shot,
  canMoveUp,
  canMoveDown,
  onMove,
  onSelect,
}: {
  projectId: string;
  episodeId: string;
  scene: SceneProjection;
  shot: SceneProjection["shots"][number];
  canMoveUp: boolean;
  canMoveDown: boolean;
  onMove: (direction: "up" | "down") => void;
  onSelect: () => void;
}) {
  const timelineReady = Boolean(
    shot.currentVideo?.timelineReady || shot.currentImage?.timelineReady,
  );
  const continuityBlocked = Boolean(shot.continuityTasks.some(Boolean));
  const reviewMedia = shot.currentVideo ?? shot.currentImage;
  const reviewParams = new URLSearchParams({ episodeId, shotId: shot.id });
  if (reviewMedia && shot.continuitySnapshot) {
    reviewParams.set("assetVersionId", reviewMedia.assetVersionId);
    reviewParams.set(
      "assetVersionRevision",
      String(reviewMedia.assetVersionRevision),
    );
    reviewParams.set("assetVersionHash", reviewMedia.assetVersionHash);
    reviewParams.set("continuitySnapshotId", shot.continuitySnapshot.ownerId);
    reviewParams.set(
      "continuitySnapshotRevision",
      String(shot.continuitySnapshot.revision),
    );
    reviewParams.set(
      "continuitySnapshotHash",
      shot.continuitySnapshot.contentHash,
    );
  }
  return (
    <article className="grid overflow-hidden rounded-md border border-border bg-background">
      <div className="grid aspect-[16/9] place-items-center gap-2 bg-muted p-3 text-center">
        <div className="flex w-full items-center justify-between text-xs font-semibold text-muted-foreground">
          <span>场 {String(scene.number).padStart(2, "0")}</span>
          <span>镜头 {String(shot.number).padStart(2, "0")}</span>
        </div>
        <Clapperboard
          aria-hidden="true"
          className="size-6 text-muted-foreground"
        />
        <span className="text-xs text-muted-foreground">
          {shot.currentVideo || shot.currentImage
            ? "已有媒体引用"
            : "暂未绑定媒体"}
        </span>
      </div>
      <div className="grid gap-3 p-4">
        <div className="flex items-center justify-between gap-2">
          <strong>镜头 {String(shot.number).padStart(2, "0")}</strong>
          <WorkbenchStatus value={shot.status} />
        </div>
        <span className="font-mono text-xs text-muted-foreground">
          {shot.id} · 第 {shot.revision} 版
        </span>
        <dl className="grid gap-1 text-xs text-muted-foreground">
          <div className="flex justify-between gap-3">
            <dt>镜头设定</dt>
            <dd className="max-w-[60%] truncate text-right">
              {shot.specRef?.ownerId ?? "未绑定"}
            </dd>
          </div>
          <div className="flex justify-between gap-3">
            <dt>连续性快照</dt>
            <dd className="max-w-[60%] truncate text-right">
              {shot.continuitySnapshot?.ownerId ?? "未绑定"}
            </dd>
          </div>
        </dl>
        {(continuityBlocked || !timelineReady) && (
          <div className="grid gap-1">
            {continuityBlocked && (
              <span className="inline-flex items-center gap-1 text-xs text-warning-foreground">
                <CircleAlert aria-hidden="true" className="size-3.5" />
                连续性任务尚未完成
              </span>
            )}
            {!timelineReady && (
              <span className="inline-flex items-center gap-1 text-xs text-warning-foreground">
                <CircleAlert aria-hidden="true" className="size-3.5" />
                媒体衍生内容尚未就绪
              </span>
            )}
          </div>
        )}
        <div className="flex flex-wrap items-center gap-2">
          <Button
            size="icon-sm"
            variant="outline"
            title="选择镜头"
            aria-label="选择镜头"
            onClick={onSelect}
          >
            <Check aria-hidden="true" />
          </Button>
          <Button
            size="icon-sm"
            variant="outline"
            title="镜头上移"
            aria-label="镜头上移"
            disabled={!canMoveUp}
            onClick={() => onMove("up")}
          >
            <ArrowLeft aria-hidden="true" />
          </Button>
          <Button
            size="icon-sm"
            variant="outline"
            title="镜头下移"
            aria-label="镜头下移"
            disabled={!canMoveDown}
            onClick={() => onMove("down")}
          >
            <ArrowRight aria-hidden="true" />
          </Button>
          <Link
            className="ml-auto inline-flex items-center gap-1 text-sm font-medium text-primary underline-offset-4 hover:underline"
            to={`/projects/${projectId}/review?${reviewParams.toString()}`}
          >
            审核 <ArrowRight aria-hidden="true" />
          </Link>
          {timelineReady && !continuityBlocked ? (
            <Link
              className="inline-flex items-center gap-1 text-sm font-medium text-primary underline-offset-4 hover:underline"
              to={`/projects/${projectId}/episodes/${episodeId}/timeline?shotId=${shot.id}`}
            >
              时间线 <ArrowRight aria-hidden="true" />
            </Link>
          ) : (
            <span className="text-xs text-muted-foreground">
              时间线暂不可用
            </span>
          )}
        </div>
      </div>
    </article>
  );
}
