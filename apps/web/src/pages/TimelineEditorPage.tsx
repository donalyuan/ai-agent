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
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  VirtualList,
} from "../shared/ui";
import { timelineApi, timelineQueryKeys } from "../timeline/api";
import { TimelineEditorWorkspace } from "../timeline/editor-workspace";
import { MediaPreview } from "../timeline/media-preview";
import { SortableClipLane } from "../timeline/sortable-clip-lane";
import { OwnerApiError } from "../workbench/api";
import { ErrorNotice, PageIntro, QueryNotice, SurfaceHeading } from "../ui";

const frameTime = (frame: number) =>
  `${Math.floor(frame / 30 / 60)
    .toString()
    .padStart(2, "0")}:${Math.floor((frame / 30) % 60)
    .toString()
    .padStart(2, "0")}.${String(frame % 30).padStart(2, "0")}`;

type CommandInput = {
  expectedRevision: number;
  command: string;
  payload: Record<string, unknown>;
};

type TimelineHandoff = {
  projectId: string;
  episodeId: string;
  assetVersionId: string;
  assetVersionRevision: number;
  assetVersionHash: string;
  derivativeFingerprint: string;
  acceptedCurrent: boolean;
  derivativeStatus: string;
  availableFrames: number;
  kind?: string;
  shotId?: string;
  authorizationStatus?: string;
  licenseStatus?: string;
  storageVerified?: boolean;
};

const readHandoff = (value: string, projectId: string, episodeId: string) => {
  try {
    const handoff = JSON.parse(value) as TimelineHandoff;
    return handoff.projectId === projectId && handoff.episodeId === episodeId
      ? handoff
      : null;
  } catch {
    return null;
  }
};

const isValidAvailableFrames = (value: unknown): value is number =>
  typeof value === "number" && Number.isInteger(value) && value > 0;

export function TimelineEditorPage() {
  const { projectId = "", episodeId = "" } = useParams();
  const [params, setParams] = useSearchParams();
  const [caption, setCaption] = useState("");
  const [captionStart, setCaptionStart] = useState("0");
  const [captionEnd, setCaptionEnd] = useState("30");
  const [publishName, setPublishName] = useState("");
  const [publishPreflight, setPublishPreflight] = useState<{
    expectedRevision: number;
    name: string;
    timelineFingerprint: string;
  } | null>(null);
  const [handoffShotId, setHandoffShotId] = useState("");
  const [replaceClipId, setReplaceClipId] = useState("");
  const [audioTrack, setAudioTrack] = useState<
    "dialogue" | "music" | "ambience" | "effects"
  >("music");
  const [commandError, setCommandError] = useState<unknown>(null);
  const [commandLog, setCommandLog] = useState<string[]>([]);
  const client = useQueryClient();
  const selectedVersionId = params.get("versionId") ?? "";
  const handoff = readHandoff(
    params.get("handoff") ?? "",
    projectId,
    episodeId,
  );
  const readOnly = Boolean(selectedVersionId);
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
    mutationFn: ({ expectedRevision, command, payload }: CommandInput) =>
      timelineApi.command(
        projectId,
        episodeId,
        expectedRevision,
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
      setCommandLog((items) =>
        [
          `failed: ${error instanceof Error ? error.message : "owner command failed"}`,
          ...items,
        ].slice(0, 80),
      );
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
        const next = new URLSearchParams(previous);
        next.set("versionId", version.id);
        return next;
      });
    },
    onError: async (error) => {
      setCommandError(error);
      if (error instanceof OwnerApiError && error.status === 409) {
        await client.invalidateQueries({
          queryKey: timelineQueryKeys.current(projectId, episodeId),
        });
      }
    },
  });
  const preflightPublish = useMutation({
    mutationFn: () =>
      timelineApi.preflightPublish(
        projectId,
        episodeId,
        current.data?.revision ?? 0,
        publishName.trim(),
      ),
    onSuccess: (result) =>
      setPublishPreflight({
        expectedRevision: result.expectedRevision,
        name: result.name,
        timelineFingerprint: result.timelineFingerprint,
      }),
    onError: async (error) => {
      setCommandError(error);
      if (error instanceof OwnerApiError && error.status === 409) {
        await client.invalidateQueries({
          queryKey: timelineQueryKeys.current(projectId, episodeId),
        });
      }
    },
  });
  const probe = useMutation({
    mutationFn: () => timelineApi.probeRenderer(projectId),
  });
  const timeline = current.data;
  const handoffAvailableFrames = handoff?.availableFrames;
  const hasValidHandoffFrames = isValidAvailableFrames(handoffAvailableFrames);
  const replacementClip = timeline?.clips.find(
    (clip) => clip.id === replaceClipId,
  );
  const replacementRequiredFrames = replacementClip
    ? replacementClip.inFrame + replacementClip.durationFrames
    : null;
  const handoffFrameDiagnostic =
    handoff && !hasValidHandoffFrames
      ? "Handoff availableFrames 必须是大于 0 的整数，已阻断 AddClip。"
      : null;
  const replacementFrameDiagnostic =
    handoff && replacementClip && replacementRequiredFrames !== null
      ? !hasValidHandoffFrames
        ? "Handoff availableFrames 无法证明新源的真实帧边界，已阻断 ReplaceClipSource。"
        : handoffAvailableFrames < replacementRequiredFrames
          ? `新源帧数不足：availableFrames=${handoffAvailableFrames}，需要覆盖到 ${replacementRequiredFrames} 帧（inFrame ${replacementClip.inFrame} + durationFrames ${replacementClip.durationFrames}），已阻断 ReplaceClipSource。`
          : null
      : null;
  const runCommand = (command: string, payload: Record<string, unknown>) => {
    const expectedRevision = current.data?.revision;
    if (!expectedRevision || readOnly) return;
    setCommandError(null);
    setCommandLog((items) =>
      [`${command} @ rev ${expectedRevision}`, ...items].slice(0, 80),
    );
    mutate.mutate({ expectedRevision, command, payload });
  };
  const tracks = useMemo(
    () => [
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
      <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8">
        <PageIntro
          eyebrow="EPISODE TIMELINE / S09"
          title="需要显式选择 Episode"
          detail="Timeline 不从项目全部集视图推断 current Cut。"
        />
      </section>
    );

  return (
    <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8 gap-5">
      <PageIntro
        eyebrow="EPISODE TIMELINE / S09"
        title={`Episode ${episodeId.slice(0, 8)}`}
        detail="30fps current Cut 立即持久化；版本发布和导出都需要显式确认。"
      />
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-border bg-card p-4 shadow-sm rounded-lg border border-border bg-card p-5 shadow-sm">
        <div>
          <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            CURRENT CUT
          </span>
          <strong className="font-mono text-xs text-muted-foreground">
            {timeline?.id ?? "--"} / rev {timeline?.revision ?? "--"}
          </strong>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => current.refetch()}
          >
            <RefreshCw /> 刷新
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => probe.mutate()}
          >
            <Volume2 /> Renderer probe
          </Button>
          <label className="grid gap-1 text-sm">
            <span>版本名</span>
            <Input
              value={publishName}
              onChange={(event) => setPublishName(event.target.value)}
              placeholder="例如 cut-v1"
              disabled={readOnly}
            />
          </label>
          <Button
            type="button"
            disabled={
              !publishName.trim() || !timeline || publish.isPending || readOnly
            }
            onClick={() => preflightPublish.mutate()}
          >
            <Upload /> 检查并发布
          </Button>
        </div>
      </div>
      {(current.isPending || versions.isPending) && (
        <QueryNotice isPending error={null} empty="" />
      )}
      {current.error && <ErrorNotice error={current.error} />}
      {commandError && (
        <div
          className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"
          role="alert"
        >
          <CircleAlert size={14} />{" "}
          {commandError instanceof Error
            ? commandError.message
            : "owner command failed"}
        </div>
      )}
      {probe.data && (
        <div
          className="flex items-start gap-2 rounded-md border border-warning/30 bg-warning/10 p-3 text-sm text-warning-foreground"
          role="status"
        >
          <CircleAlert size={14} /> renderer:{" "}
          {String((probe.data as { status?: string }).status ?? "unconfigured")}{" "}
          {String((probe.data as { diagnostic?: string }).diagnostic ?? "")}
        </div>
      )}
      {timeline && (
        <TimelineEditorWorkspace
          canvas={
            <section className="grid gap-4" data-testid="timeline-canvas">
              <div className="flex items-center justify-between gap-4">
                <div>
                  <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    PIXI / PROXY PREVIEW
                  </span>
                  <h3 className="mt-1 text-base font-semibold">
                    Canvas monitor
                  </h3>
                </div>
                <span className="font-mono text-xs text-muted-foreground">
                  30fps · integer frames
                </span>
              </div>
              <MediaPreview
                kind="pixi"
                url={String(
                  (timeline.clips[0] as Record<string, unknown> | undefined)
                    ?.previewUrl ?? "",
                )}
                title="Timeline Pixi preview"
              />
              <div className="grid gap-3 rounded-md border border-border p-3">
                <div className="flex items-center justify-between gap-3">
                  <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    CLIP LANES
                  </span>
                  <span className="text-xs text-muted-foreground">
                    同父排序 · CAS command
                  </span>
                </div>
                <SortableClipLane
                  clips={timeline.clips}
                  disabled={readOnly || mutate.isPending}
                  onReorder={(clipIds) =>
                    runCommand("ReorderClips", { clipIds })
                  }
                />
              </div>
              <div className="grid gap-2">
                <div className="grid gap-1 border-b border-border pb-2 font-mono text-xs text-muted-foreground">
                  {[0, 150, 300, 450, 600].map((frame) => (
                    <span key={frame}>{frameTime(frame)}</span>
                  ))}
                </div>
                {tracks.map((track) => (
                  <div
                    className="grid gap-2 border-b border-border py-3"
                    key={track.key}
                  >
                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      {track.label}
                    </span>
                    <div className="min-h-16 rounded-md bg-muted p-2">
                      {track.items.length === 0 && (
                        <span className="rounded-md border border-dashed border-border p-4 text-center text-sm text-muted-foreground">
                          暂无 owner item
                        </span>
                      )}
                      {track.items.map((item, index) => {
                        const start =
                          "startFrame" in item ? item.startFrame : 0;
                        const duration =
                          "durationFrames" in item
                            ? item.durationFrames
                            : "endFrame" in item
                              ? item.endFrame - item.startFrame
                              : 30;
                        return (
                          <div
                            className={`absolute inset-y-2 rounded-sm border border-primary/30 bg-primary/15 ${index % 2 ? "bg-accent" : ""}`}
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
              </div>
            </section>
          }
          inspector={
            <aside
              className="grid content-start gap-4"
              data-testid="timeline-inspector"
            >
              <SurfaceHeading label="OWNER INSPECTOR" title="精确 command" />
              <div className="grid gap-2 rounded-md border border-border p-3 text-sm">
                <div className="flex justify-between gap-4">
                  <span className="text-muted-foreground">Frame rate</span>
                  <strong>30 fps</strong>
                </div>
                <div className="flex justify-between gap-4">
                  <span className="text-muted-foreground">Transition</span>
                  <strong>cut / crossfade</strong>
                </div>
                <div className="grid gap-1">
                  <span className="text-muted-foreground">
                    Current fingerprint
                  </span>
                  <strong className="truncate font-mono text-xs text-muted-foreground">
                    {timeline.timelineFingerprint}
                  </strong>
                </div>
              </div>
              {handoff && (
                <div className="grid gap-2 rounded-md border border-border p-3 text-sm">
                  <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    ASSET CENTER / REVIEW HANDOFF
                  </span>
                  <strong className="truncate font-mono text-xs text-muted-foreground">
                    {handoff.assetVersionId} · rev{" "}
                    {handoff.assetVersionRevision}
                  </strong>
                  <span className="truncate font-mono text-xs text-muted-foreground">
                    {handoff.assetVersionHash} · {handoff.derivativeFingerprint}
                  </span>
                  {handoffFrameDiagnostic && (
                    <div
                      className="flex items-start gap-2 rounded-md border border-warning/30 bg-warning/10 p-3 text-sm text-warning-foreground"
                      data-testid="timeline-handoff-frame-diagnostic"
                      role="alert"
                    >
                      <CircleAlert size={14} /> {handoffFrameDiagnostic}
                    </div>
                  )}
                  <Input
                    aria-label="Handoff Shot ID"
                    value={handoffShotId || handoff.shotId || ""}
                    onChange={(event) => setHandoffShotId(event.target.value)}
                    placeholder="显式 Shot ID"
                    disabled={readOnly || mutate.isPending}
                  />
                  {handoff.kind !== "audio" && (
                    <Button
                      type="button"
                      variant="outline"
                      disabled={
                        readOnly ||
                        mutate.isPending ||
                        !(handoffShotId || handoff.shotId) ||
                        !hasValidHandoffFrames ||
                        handoff.acceptedCurrent !== true ||
                        handoff.derivativeStatus !== "ready"
                      }
                      onClick={() =>
                        runCommand("AddClip", {
                          clip: {
                            id: crypto.randomUUID(),
                            projectId,
                            episodeId,
                            shotId: handoffShotId || handoff.shotId,
                            assetVersionId: handoff.assetVersionId,
                            assetVersionRevision: handoff.assetVersionRevision,
                            assetVersionHash: handoff.assetVersionHash,
                            derivativeFingerprint:
                              handoff.derivativeFingerprint,
                            acceptedCurrent: handoff.acceptedCurrent,
                            derivativeStatus: handoff.derivativeStatus,
                            inFrame: 0,
                            outFrame: handoff.availableFrames,
                            timelineStart: timeline.clips.reduce(
                              (end, clip) =>
                                Math.max(
                                  end,
                                  clip.timelineStart + clip.durationFrames,
                                ),
                              0,
                            ),
                          },
                        })
                      }
                    >
                      <Plus /> 添加 Video / Image Clip
                    </Button>
                  )}
                  {handoff.kind === "audio" && (
                    <>
                      <Select
                        value={audioTrack}
                        onValueChange={(value) =>
                          setAudioTrack(
                            value as
                              | "dialogue"
                              | "music"
                              | "ambience"
                              | "effects",
                          )
                        }
                      >
                        <SelectTrigger
                          aria-label="SoundCue track"
                          disabled={readOnly || mutate.isPending}
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="dialogue">dialogue</SelectItem>
                          <SelectItem value="music">music</SelectItem>
                          <SelectItem value="ambience">ambience</SelectItem>
                          <SelectItem value="effects">effects</SelectItem>
                        </SelectContent>
                      </Select>
                      <Button
                        type="button"
                        variant="outline"
                        disabled={
                          readOnly ||
                          mutate.isPending ||
                          timeline.clips.length === 0
                        }
                        onClick={() =>
                          runCommand("AddSoundCue", {
                            cue: {
                              id: crypto.randomUUID(),
                              projectId,
                              episodeId,
                              assetVersionId: handoff.assetVersionId,
                              assetVersionRevision:
                                handoff.assetVersionRevision,
                              assetVersionHash: handoff.assetVersionHash,
                              explicitSelection: true,
                              storageVerified: handoff.storageVerified === true,
                              authorizationStatus: "authorized",
                              licenseStatus: "approved",
                              track: audioTrack,
                              startFrame: 0,
                              durationFrames: Math.max(
                                1,
                                timeline.clips.reduce(
                                  (end, clip) =>
                                    Math.max(
                                      end,
                                      clip.timelineStart + clip.durationFrames,
                                    ),
                                  0,
                                ),
                              ),
                              trigger: "manual",
                              triggerRef: null,
                              priority: 0,
                              continuityRefs: [],
                              gainDb: 0,
                              mute: false,
                              solo: false,
                              fadeInFrames: 0,
                              fadeOutFrames: 0,
                            },
                          })
                        }
                      >
                        <Plus /> 添加 {audioTrack} SoundCue
                      </Button>
                    </>
                  )}
                  {handoff.kind !== "audio" && timeline.clips.length > 0 && (
                    <>
                      <Input
                        aria-label="Replace Clip ID"
                        value={replaceClipId}
                        onChange={(event) =>
                          setReplaceClipId(event.target.value)
                        }
                        placeholder="选择要替换的 Clip ID"
                        disabled={readOnly || mutate.isPending}
                      />
                      <Button
                        type="button"
                        variant="outline"
                        disabled={
                          readOnly ||
                          mutate.isPending ||
                          !replaceClipId ||
                          !(handoffShotId || handoff.shotId) ||
                          !hasValidHandoffFrames ||
                          replacementFrameDiagnostic !== null ||
                          handoff.acceptedCurrent !== true ||
                          handoff.derivativeStatus !== "ready"
                        }
                        onClick={() => {
                          const old = timeline.clips.find(
                            (clip) => clip.id === replaceClipId,
                          );
                          if (!old) {
                            setCommandError(
                              new Error("Clip 不存在于 current Cut"),
                            );
                            return;
                          }
                          const requiredFrames =
                            old.inFrame + old.durationFrames;
                          if (!hasValidHandoffFrames) {
                            setCommandError(
                              new Error(
                                "Handoff availableFrames 无法证明新源的真实帧边界，已阻断 ReplaceClipSource。",
                              ),
                            );
                            return;
                          }
                          if (handoff.availableFrames < requiredFrames) {
                            setCommandError(
                              new Error(
                                `新源帧数不足：availableFrames=${handoff.availableFrames}，需要覆盖到 ${requiredFrames} 帧，已阻断 ReplaceClipSource。`,
                              ),
                            );
                            return;
                          }
                          runCommand("ReplaceClipSource", {
                            clipId: old.id,
                            oldSource: {
                              assetVersionId: old.assetVersionId,
                              assetVersionRevision: old.assetVersionRevision,
                              assetVersionHash: old.assetVersionHash,
                              derivativeFingerprint: old.derivativeFingerprint,
                            },
                            newSource: {
                              projectId,
                              episodeId,
                              shotId: handoffShotId || handoff.shotId,
                              assetVersionId: handoff.assetVersionId,
                              assetVersionRevision:
                                handoff.assetVersionRevision,
                              assetVersionHash: handoff.assetVersionHash,
                              derivativeFingerprint:
                                handoff.derivativeFingerprint,
                              acceptedCurrent: handoff.acceptedCurrent,
                              derivativeStatus: handoff.derivativeStatus,
                              authorizationStatus: "authorized",
                              licenseStatus: "approved",
                              availableFrames: handoff.availableFrames,
                            },
                          });
                        }}
                      >
                        <RefreshCw /> 确认替换 Clip source
                      </Button>
                      {replacementFrameDiagnostic && (
                        <div
                          className="flex items-start gap-2 rounded-md border border-warning/30 bg-warning/10 p-3 text-sm text-warning-foreground"
                          data-testid="timeline-replace-frame-diagnostic"
                          role="alert"
                        >
                          <CircleAlert size={14} /> {replacementFrameDiagnostic}
                        </div>
                      )}
                    </>
                  )}
                </div>
              )}
              <div className="grid gap-2">
                <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  CLIP COMMANDS / VIRTUAL
                </span>
                <VirtualList
                  items={timeline.clips}
                  estimateSize={40}
                  height={220}
                  getKey={(clip) => clip.id}
                  ariaLabel="Clip command list"
                  renderItem={(clip) => (
                    <div className="flex min-h-10 items-center justify-between gap-2 border-b border-border px-2">
                      <span className="truncate font-mono text-xs text-muted-foreground">
                        {clip.id}
                      </span>
                      <span className="flex shrink-0 gap-1">
                        <Button
                          type="button"
                          size="icon-sm"
                          variant="ghost"
                          aria-label={`拆分 Clip ${clip.id}`}
                          disabled={readOnly || mutate.isPending}
                          onClick={() =>
                            runCommand("SplitClip", {
                              clipId: clip.id,
                              splitFrame:
                                clip.inFrame +
                                Math.max(
                                  1,
                                  Math.floor(clip.durationFrames / 2),
                                ),
                            })
                          }
                        >
                          <Scissors />
                        </Button>
                        <Button
                          type="button"
                          size="icon-sm"
                          variant="ghost"
                          aria-label={`删除 Clip ${clip.id}`}
                          data-testid={`delete-clip-${clip.id}`}
                          disabled={readOnly || mutate.isPending}
                          onClick={() =>
                            runCommand("DeleteClip", { clipId: clip.id })
                          }
                        >
                          <Trash2 />
                        </Button>
                      </span>
                    </div>
                  )}
                />
              </div>
              <div className="grid gap-2">
                <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  WAVEFORM PROXY
                </span>
                <MediaPreview
                  kind="waveform"
                  url={String(
                    (
                      timeline.soundCues[0] as
                        | Record<string, unknown>
                        | undefined
                    )?.previewUrl ?? "",
                  )}
                  title="Timeline waveform preview"
                />
              </div>
              <div className="grid gap-2">
                <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  CAPTION / OWNER COMMAND
                </span>
                <Input
                  value={caption}
                  onChange={(event) => setCaption(event.target.value)}
                  placeholder="手工字幕文本"
                  aria-label="手工字幕文本"
                  disabled={readOnly}
                />
                <div className="grid grid-cols-2 gap-2">
                  <Input
                    value={captionStart}
                    onChange={(event) => setCaptionStart(event.target.value)}
                    aria-label="字幕开始帧"
                    disabled={readOnly}
                  />
                  <Input
                    value={captionEnd}
                    onChange={(event) => setCaptionEnd(event.target.value)}
                    aria-label="字幕结束帧"
                    disabled={readOnly}
                  />
                </div>
                <Button
                  type="button"
                  variant="outline"
                  disabled={!caption.trim() || readOnly || mutate.isPending}
                  onClick={() => {
                    runCommand("UpsertManualCaption", {
                      caption: {
                        id: crypto.randomUUID(),
                        text: caption.trim(),
                        startFrame: Number(captionStart),
                        endFrame: Number(captionEnd),
                      },
                    });
                    setCaption("");
                  }}
                >
                  <Plus /> 添加手工字幕
                </Button>
              </div>
              {timeline.ducking && (
                <div className="rounded-md border border-border p-3 text-xs text-muted-foreground">
                  Ducking {timeline.ducking.enabled ? "enabled" : "disabled"} ·
                  -{timeline.ducking.attenuationDb}dB ·{" "}
                  {timeline.ducking.attackFrames}/
                  {timeline.ducking.releaseFrames}f
                </div>
              )}
            </aside>
          }
        />
      )}
      <section
        className="grid gap-3 rounded-md border border-border p-4"
        data-testid="timeline-versions"
      >
        <SurfaceHeading
          label="IMMUTABLE TIMELINE VERSIONS"
          title="只读比较"
          trailing={
            <GitCompareArrows className="size-4 text-muted-foreground" />
          }
        />
        {versions.data?.length ? (
          <VirtualList
            items={versions.data}
            estimateSize={48}
            height={240}
            getKey={(version) => version.id}
            ariaLabel="Timeline version list"
            renderItem={(version) => (
              <button
                type="button"
                className={`flex min-h-12 w-full items-center justify-between gap-3 border-b border-border px-2 text-left hover:bg-muted ${selectedVersionId === version.id ? "bg-muted" : ""}`}
                aria-pressed={selectedVersionId === version.id}
                onClick={() =>
                  setParams((previous) => {
                    const next = new URLSearchParams(previous);
                    next.set("versionId", version.id);
                    return next;
                  })
                }
              >
                <span className="grid min-w-0 gap-1">
                  <strong className="truncate">{version.name}</strong>
                  <small className="truncate font-mono text-xs text-muted-foreground">
                    {version.id} / source cut rev {version.sourceCutRevision}
                  </small>
                </span>
                <GitCompareArrows className="size-4 shrink-0" />
              </button>
            )}
          />
        ) : (
          <div className="rounded-md border border-dashed border-border p-4 text-center text-sm text-muted-foreground">
            尚无 published TimelineVersion
          </div>
        )}
        {selectedVersion.data && (
          <pre
            className="max-h-72 overflow-auto rounded-md bg-muted p-3 text-xs"
            data-testid="version-snapshot"
          >
            {JSON.stringify(selectedVersion.data.snapshot, null, 2)}
          </pre>
        )}
      </section>
      <section className="grid gap-2 rounded-md border border-border p-4">
        <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          COMMAND LOG / VIRTUAL
        </span>
        <VirtualList
          items={commandLog}
          estimateSize={28}
          height={132}
          getKey={(entry, index) => `${index}-${entry}`}
          ariaLabel="Timeline command log"
          renderItem={(entry) => (
            <div className="border-b border-border px-2 py-1 font-mono text-xs text-muted-foreground">
              {entry}
            </div>
          )}
        />
      </section>
      <Dialog
        open={Boolean(publishPreflight)}
        onOpenChange={(open) => !open && setPublishPreflight(null)}
      >
        <DialogContent aria-describedby="timeline-publish-confirmation">
          <DialogTitle>确认发布 TimelineVersion</DialogTitle>
          <DialogDescription id="timeline-publish-confirmation">
            当前 Cut rev {publishPreflight?.expectedRevision} 已通过 owner
            preflight； 发布会创建不可变 TimelineVersion，不会重写已有版本。
          </DialogDescription>
          <p className="truncate font-mono text-xs text-muted-foreground">
            {publishPreflight?.timelineFingerprint}
          </p>
          <div className="flex flex-wrap justify-end gap-2">
            <DialogClose asChild>
              <Button type="button" variant="outline">
                取消
              </Button>
            </DialogClose>
            <Button
              type="button"
              disabled={publish.isPending}
              onClick={() => {
                if (!publishPreflight) return;
                publish.mutate();
                setPublishPreflight(null);
              }}
            >
              <Upload /> 确认发布 {publishPreflight?.name}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </section>
  );
}
