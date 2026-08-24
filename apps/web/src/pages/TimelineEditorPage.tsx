import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CircleAlert,
  GitCompareArrows,
  Plus,
  RefreshCw,
  Scissors,
  Trash2,
  Upload,
  Volume2,
} from "lucide-react";
import { useMemo, useState } from "react";
import { useParams, useSearchParams } from "react-router";
import { timelineApi, timelineQueryKeys } from "../timeline/api";
import { OwnerApiError } from "../workbench/api";
import { ErrorNotice, PageIntro, QueryNotice, SurfaceHeading } from "../ui";

const frameTime = (frame: number) =>
  `${Math.floor(frame / 30 / 60)
    .toString()
    .padStart(2, "0")}:${Math.floor((frame / 30) % 60)
    .toString()
    .padStart(2, "0")}.${String(frame % 30).padStart(2, "0")}`;

export function TimelineEditorPage() {
  const { projectId = "", episodeId = "" } = useParams();
  const [params, setParams] = useSearchParams();
  const [caption, setCaption] = useState("");
  const [captionStart, setCaptionStart] = useState("0");
  const [captionEnd, setCaptionEnd] = useState("30");
  const [publishName, setPublishName] = useState("");
  const [commandError, setCommandError] = useState<unknown>(null);
  const client = useQueryClient();
  const current = useQuery({
    queryKey: timelineQueryKeys.current(projectId, episodeId),
    queryFn: () => timelineApi.current(projectId, episodeId),
    enabled: Boolean(projectId && episodeId),
  });
  const versions = useQuery({
    queryKey: timelineQueryKeys.versions(projectId, episodeId),
    queryFn: () => timelineApi.versions(projectId, episodeId),
    enabled: Boolean(projectId && episodeId),
  });
  const selectedVersionId = params.get("versionId") ?? "";
  const selectedVersion = useQuery({
    queryKey: timelineQueryKeys.version(
      projectId,
      episodeId,
      selectedVersionId,
    ),
    queryFn: () => timelineApi.version(projectId, episodeId, selectedVersionId),
    enabled: Boolean(selectedVersionId),
  });
  const mutate = useMutation({
    mutationFn: ({
      command,
      payload,
    }: {
      command: string;
      payload: Record<string, unknown>;
    }) =>
      timelineApi.command(
        projectId,
        episodeId,
        current.data?.revision ?? 0,
        command,
        payload,
      ),
    onSuccess: async () => {
      setCommandError(null);
      await client.invalidateQueries({
        queryKey: timelineQueryKeys.current(projectId, episodeId),
      });
    },
    onError: async (error) => {
      setCommandError(error);
      if (error instanceof OwnerApiError && error.status === 409)
        await client.invalidateQueries({
          queryKey: timelineQueryKeys.current(projectId, episodeId),
        });
    },
  });
  const publish = useMutation({
    mutationFn: () =>
      timelineApi.publish(
        projectId,
        episodeId,
        current.data?.revision ?? 0,
        publishName.trim(),
      ),
    onSuccess: async (version) => {
      setPublishName("");
      await client.invalidateQueries({
        queryKey: timelineQueryKeys.versions(projectId, episodeId),
      });
      setParams((previous) => {
        previous.set("versionId", version.id);
        return previous;
      });
    },
    onError: setCommandError,
  });
  const probe = useMutation({
    mutationFn: () => timelineApi.probeRenderer(projectId),
  });
  const timeline = current.data;
  const tracks = useMemo(
    () => [
      { key: "video", label: "VIDEO", items: timeline?.clips ?? [] },
      {
        key: "dialogue",
        label: "DIALOGUE",
        items:
          timeline?.soundCues.filter((item) => item.track === "dialogue") ?? [],
      },
      {
        key: "music",
        label: "MUSIC",
        items:
          timeline?.soundCues.filter((item) => item.track === "music") ?? [],
      },
      {
        key: "ambience",
        label: "AMBIENCE",
        items:
          timeline?.soundCues.filter((item) => item.track === "ambience") ?? [],
      },
      {
        key: "effects",
        label: "EFFECTS",
        items:
          timeline?.soundCues.filter((item) => item.track === "effects") ?? [],
      },
      { key: "caption", label: "CAPTION", items: timeline?.captions ?? [] },
    ],
    [timeline],
  );

  if (!episodeId)
    return (
      <section className="page-body">
        <PageIntro
          eyebrow="EPISODE TIMELINE / S09"
          title="需要显式选择 Episode"
          detail="Timeline 不从项目全部集视图推断 current Cut。"
        />
      </section>
    );
  return (
    <section className="page-body timeline-editor-page">
      <PageIntro
        eyebrow="EPISODE TIMELINE / S09"
        title={`Episode ${episodeId.slice(0, 8)}`}
        detail="30fps current Cut 立即持久化；版本发布和导出都需要显式确认。"
      />
      <div className="timeline-toolbar surface">
        <div>
          <span className="micro-label">CURRENT CUT</span>
          <strong className="mono">
            {timeline?.id ?? "--"} / rev {timeline?.revision ?? "--"}
          </strong>
        </div>
        <div className="toolbar-actions">
          <button
            className="secondary-button"
            onClick={() => current.refetch()}
          >
            <RefreshCw size={15} /> 刷新
          </button>
          <button className="secondary-button" onClick={() => probe.mutate()}>
            <Volume2 size={15} /> Renderer probe
          </button>
          <label className="compact-field">
            <span>版本名</span>
            <input
              value={publishName}
              onChange={(event) => setPublishName(event.target.value)}
              placeholder="例如 cut-v1"
            />
          </label>
          <button
            className="primary-button"
            disabled={!publishName.trim() || !timeline || publish.isPending}
            onClick={() => publish.mutate()}
          >
            <Upload size={15} /> 发布版本
          </button>
        </div>
      </div>
      {(current.isPending || versions.isPending) && (
        <QueryNotice isPending error={null} empty="" />
      )}
      {current.error && <ErrorNotice error={current.error} />}
      {commandError && (
        <div className="warning-line" role="alert">
          <CircleAlert size={14} />{" "}
          {commandError instanceof Error
            ? commandError.message
            : "owner command failed"}
        </div>
      )}
      {probe.data && (
        <div className="warning-line">
          <CircleAlert size={14} /> renderer:{" "}
          {String((probe.data as { status?: string }).status ?? "unconfigured")}{" "}
          {(probe.data as { diagnostic?: string }).diagnostic ?? ""}
        </div>
      )}
      {timeline && (
        <div className="timeline-workspace">
          <section className="surface timeline-canvas">
            <div className="timeline-ruler">
              {[0, 150, 300, 450, 600].map((frame) => (
                <span key={frame}>{frameTime(frame)}</span>
              ))}
            </div>
            {tracks.map((track) => (
              <div className="timeline-track" key={track.key}>
                <span className="track-label">{track.label}</span>
                <div className="timeline-lane">
                  {track.items.length === 0 && (
                    <span className="timeline-empty">暂无 owner item</span>
                  )}
                  {track.items.map((item, index) => {
                    const start =
                      "timelineStart" in item
                        ? item.timelineStart
                        : "startFrame" in item
                          ? item.startFrame
                          : 0;
                    const duration =
                      "durationFrames" in item
                        ? item.durationFrames
                        : "endFrame" in item
                          ? item.endFrame - item.startFrame
                          : 30;
                    return (
                      <div
                        className={`clip clip-${index % 4}`}
                        style={{
                          left: `${Math.min(96, Number(start) / 6)}%`,
                          width: `${Math.max(8, Math.min(55, Number(duration) / 6))}%`,
                        }}
                        key={String(item.id)}
                      >
                        <strong>
                          {"assetVersionId" in item
                            ? String(item.assetVersionId).slice(0, 10)
                            : "text" in item
                              ? String(item.text).slice(0, 18)
                              : `item ${index + 1}`}
                        </strong>
                        <span>
                          {frameTime(Number(start))} / {Number(duration)}f
                        </span>
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}
          </section>
          <aside className="surface timeline-inspector">
            <SurfaceHeading label="OWNER INSPECTOR" title="精确 command" />
            <div className="inspector-block">
              <span>Frame rate</span>
              <strong>30 fps</strong>
            </div>
            <div className="inspector-block">
              <span>Transition</span>
              <strong>cut / crossfade only</strong>
            </div>
            <div className="inspector-block">
              <span>Current fingerprint</span>
              <strong className="mono">
                {timeline.timelineFingerprint.slice(0, 16)}...
              </strong>
            </div>
            <div className="timeline-actions">
              <span className="micro-label">CLIP COMMANDS</span>
              {timeline.clips.map((clip) => (
                <div className="command-row" key={clip.id}>
                  <span className="mono">{clip.id.slice(0, 8)}</span>
                  <button
                    className="icon-button"
                    title="拆分 Clip"
                    onClick={() =>
                      mutate.mutate({
                        command: "SplitClip",
                        payload: {
                          clipId: clip.id,
                          splitFrame:
                            clip.inFrame +
                            Math.max(1, Math.floor(clip.durationFrames / 2)),
                        },
                      })
                    }
                  >
                    <Scissors size={15} />
                  </button>
                  <button
                    className="icon-button"
                    title="删除 Clip 引用"
                    onClick={() =>
                      mutate.mutate({
                        command: "DeleteClip",
                        payload: { clipId: clip.id },
                      })
                    }
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              ))}
              <span className="micro-label">CAPTION</span>
              <input
                value={caption}
                onChange={(event) => setCaption(event.target.value)}
                placeholder="手工字幕文本"
              />
              <div className="inline-fields">
                <input
                  value={captionStart}
                  onChange={(event) => setCaptionStart(event.target.value)}
                  aria-label="字幕开始帧"
                />
                <input
                  value={captionEnd}
                  onChange={(event) => setCaptionEnd(event.target.value)}
                  aria-label="字幕结束帧"
                />
              </div>
              <button
                className="secondary-button full"
                disabled={!caption.trim()}
                onClick={() => {
                  mutate.mutate({
                    command: "UpsertManualCaption",
                    payload: {
                      caption: {
                        id: crypto.randomUUID(),
                        text: caption.trim(),
                        startFrame: Number(captionStart),
                        endFrame: Number(captionEnd),
                      },
                    },
                  });
                  setCaption("");
                }}
              >
                <Plus size={15} /> 添加手工字幕
              </button>
              <div className="support-note">
                <GitCompareArrows size={14} /> SoundCue 使用
                start/duration/trigger/priority/continuityRefs；automation/keyframe
                会被 owner 拒绝。
              </div>
              {timeline.ducking && (
                <div className="support-note">
                  Ducking {timeline.ducking.enabled ? "enabled" : "disabled"} ·
                  -{timeline.ducking.attenuationDb}dB ·{" "}
                  {timeline.ducking.attackFrames}/
                  {timeline.ducking.releaseFrames}f
                </div>
              )}
            </div>
          </aside>
        </div>
      )}
      <section className="surface timeline-versions">
        <SurfaceHeading label="IMMUTABLE TIMELINE VERSIONS" title="只读比较" />
        {versions.data?.length ? (
          versions.data.map((version) => (
            <button
              className={`version-row ${selectedVersionId === version.id ? "selected" : ""}`}
              key={version.id}
              onClick={() =>
                setParams((previous) => {
                  previous.set("versionId", version.id);
                  return previous;
                })
              }
            >
              <span>
                <strong>{version.name}</strong>
                <small className="mono">
                  {version.id} / source cut rev {version.sourceCutRevision}
                </small>
              </span>
              <GitCompareArrows size={15} />
            </button>
          ))
        ) : (
          <div className="timeline-empty">尚无 published TimelineVersion</div>
        )}
        {selectedVersion.data && (
          <pre className="safe-pre">
            {JSON.stringify(selectedVersion.data.snapshot, null, 2)}
          </pre>
        )}
      </section>
    </section>
  );
}
