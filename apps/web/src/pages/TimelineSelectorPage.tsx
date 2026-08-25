import { useMutation, useQuery } from "@tanstack/react-query";
import { ArrowRight, Boxes } from "lucide-react";
import { useNavigate, useParams, useSearchParams } from "react-router";
import { assetCenterApi } from "../asset-center/api";
import { Button } from "../shared/ui";
import { ErrorNotice, PageIntro, QueryNotice, SurfaceHeading } from "../ui";
import { queryKeys, workbenchApi } from "../workbench/api";

export function TimelineSelectorPage() {
  const { projectId = "" } = useParams();
  const [searchParams] = useSearchParams();
  const assetVersionId = searchParams.get("assetVersionId");
  const assetVersionRevision = searchParams.get("assetVersionRevision");
  const assetVersionHash = searchParams.get("assetVersionHash");
  const episodes = useQuery({
    queryKey: queryKeys.episodes(projectId),
    queryFn: () => workbenchApi.listEpisodes(projectId),
    enabled: Boolean(projectId),
  });
  const episodeList = [...(episodes.data ?? [])].sort(
    (left, right) =>
      left.number - right.number || left.id.localeCompare(right.id),
  );
  const navigate = useNavigate();
  const handoff = useMutation({
    mutationFn: async (episodeId: string) => {
      if (!assetVersionId) return null;
      return assetCenterApi.timelineSelection(
        projectId,
        assetVersionId,
        episodeId,
      );
    },
    onSuccess: (selection, episodeId) => {
      const suffix = selection
        ? `?handoff=${encodeURIComponent(JSON.stringify(selection))}`
        : "";
      navigate(
        `/projects/${projectId}/episodes/${episodeId}/timeline${suffix}`,
      );
    },
  });

  return (
    <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8">
      <PageIntro
        eyebrow="时间线"
        title="选择一个剧集"
        detail="时间线只在已选择的剧集范围内工作，不会从全部剧集推断当前剪辑版本。"
      />
      <section className="grid gap-4 rounded-lg border border-border bg-card p-5 shadow-sm">
        <SurfaceHeading label="剧集" title="选择剧集" />
        {assetVersionId && (
          <div className="flex flex-wrap items-center gap-2 rounded-md bg-muted p-3 text-sm">
            <Boxes aria-hidden="true" className="size-4 text-primary" />
            <span>已选择素材版本</span>
            <small className="font-mono text-xs text-muted-foreground">
              {assetVersionId} / rev {assetVersionRevision ?? "?"} /{" "}
              {assetVersionHash?.slice(0, 12)}
            </small>
          </div>
        )}
        {episodes.isPending && <QueryNotice isPending error={null} empty="" />}
        {episodes.error && <ErrorNotice error={episodes.error} />}
        {!episodes.isPending &&
          !episodes.error &&
          episodes.data?.length === 0 && (
            <QueryNotice
              isPending={false}
              error={null}
              empty="当前项目尚无剧集；不会创建模板入口。"
            />
          )}
        <div className="grid gap-2">
          {episodeList.map((episode) => (
            <Button
              className="h-auto min-h-16 w-full justify-start text-left"
              disabled={handoff.isPending}
              key={episode.id}
              onClick={() =>
                assetVersionId
                  ? handoff.mutate(episode.id)
                  : navigate(
                      `/projects/${projectId}/episodes/${episode.id}/timeline`,
                    )
              }
              variant="outline"
            >
              <span className="grid size-9 shrink-0 place-items-center rounded bg-muted font-mono text-xs text-muted-foreground">
                {String(episode.number).padStart(2, "0")}
              </span>
              <span className="grid min-w-0 gap-1">
                <strong>{episode.title}</strong>
                <small className="font-mono text-xs text-muted-foreground">
                  {episode.id} / rev {episode.revision}
                </small>
              </span>
              <ArrowRight aria-hidden="true" className="ml-auto size-4" />
            </Button>
          ))}
        </div>
        {handoff.error && <ErrorNotice error={handoff.error} />}
      </section>
    </section>
  );
}
