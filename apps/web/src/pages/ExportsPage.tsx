import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CircleAlert,
  Download,
  Plus,
  RefreshCw,
  RotateCcw,
  Trash2,
  Upload,
} from "lucide-react";
import { useMemo, useState } from "react";
import { useParams } from "react-router";
import {
  artifactGrantSchema,
  downloadableArtifact,
  exportBatchSchema,
  latestJobsByEpisode,
  type ExportArtifact,
  type ExportJob,
} from "../exports/contracts";
import { ErrorNotice, PageIntro } from "../ui";
import { OwnerApiError } from "../workbench/api";

type ExportSelection = {
  episodeId: string;
  timelineVersionId: string;
  timelineVersionRevision: number;
  outputBaseName: string;
};

async function request(projectId: string, path: string, init?: RequestInit) {
  const response = await fetch(`/api/v1/projects/${projectId}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      "X-Project-Scope": projectId,
      ...(init?.headers ?? {}),
    },
  });
  const body = await response.json().catch(() => null);
  if (!response.ok)
    throw new OwnerApiError(
      response.status,
      body?.detail?.type ?? "export_unavailable",
      body?.detail?.message ?? `Export owner 请求失败（${response.status}）`,
    );
  return body;
}

const operationId = (prefix: string) =>
  `${prefix}:${globalThis.crypto.randomUUID()}`;

export function ExportsPage() {
  const { projectId = "" } = useParams();
  const client = useQueryClient();
  const [batchId, setBatchId] = useState("");
  const [episodeId, setEpisodeId] = useState("");
  const [timelineVersionId, setTimelineVersionId] = useState("");
  const [timelineVersionRevision, setTimelineVersionRevision] = useState("1");
  const [outputBaseName, setOutputBaseName] = useState("");
  const [storageProfileId, setStorageProfileId] = useState("");
  const [storageProfileRevision, setStorageProfileRevision] = useState("1");
  const [selections, setSelections] = useState<ExportSelection[]>([]);
  const [retryEpisodeIds, setRetryEpisodeIds] = useState<string[]>([]);
  const [diagnostic, setDiagnostic] = useState("");

  const batch = useQuery({
    queryKey: ["projects", projectId, "export-batch", batchId],
    queryFn: async () =>
      exportBatchSchema.parse(
        await request(projectId, `/export-batches/${batchId}`),
      ),
    enabled: false,
  });
  const latestJobs = useMemo(
    () => latestJobsByEpisode(batch.data?.jobs ?? []),
    [batch.data?.jobs],
  );
  const failedEpisodeIds = useMemo(
    () =>
      [...latestJobs.values()]
        .filter((job) => job.status === "failed")
        .map((job) => job.episodeId),
    [latestJobs],
  );
  const selectedFailedEpisodeIds = retryEpisodeIds.filter((episode) =>
    failedEpisodeIds.includes(episode),
  );

  const retry = useMutation({
    mutationFn: () => {
      if (!batchId || selectedFailedEpisodeIds.length === 0)
        throw new Error("请至少选择一个失败 Episode");
      return request(projectId, `/export-batches/${batchId}/retries`, {
        method: "POST",
        body: JSON.stringify({
          episodeIds: selectedFailedEpisodeIds,
          logicalOperation: operationId(`export-retry:${batchId}`),
          schemaVersion: "1.0.0",
        }),
      });
    },
    onSuccess: async () => {
      setRetryEpisodeIds([]);
      await client.invalidateQueries({
        queryKey: ["projects", projectId, "export-batch", batchId],
      });
      await batch.refetch();
    },
    onError: (error) =>
      setDiagnostic(error instanceof Error ? error.message : "重试失败"),
  });

  const create = useMutation({
    mutationFn: async () => {
      if (selections.length === 0) throw new Error("请先添加至少一个 Episode");
      return exportBatchSchema.parse(
        await request(projectId, "/export-batches", {
          method: "POST",
          body: JSON.stringify({
            selections,
            exportProfile: "light",
            idempotencyKey: operationId(`export:${projectId}`),
            storageProfileId,
            storageProfileRevision: Number(storageProfileRevision),
            expectedRevision: 1,
            settings: {
              aspectRatio: "9:16",
              width: 1080,
              height: 1920,
              fps: 30,
              container: "mp4",
              videoCodec: "h264",
              pixelFormat: "yuv420p",
              audioCodec: "aac",
              sampleRate: 48000,
              subtitleEncoding: "UTF-8",
            },
            schemaVersion: "1.0.0",
          }),
        }),
      );
    },
    onSuccess: (value) => {
      setBatchId(value.id);
      setDiagnostic(`已提交 ${value.members.length} 集导出`);
      client.setQueryData(
        ["projects", projectId, "export-batch", value.id],
        value,
      );
    },
    onError: (error) =>
      setDiagnostic(error instanceof Error ? error.message : "创建 batch 失败"),
  });

  const grant = useMutation({
    mutationFn: async ({
      job,
      artifact,
    }: {
      job: ExportJob;
      artifact: ExportArtifact;
    }) => {
      if (!downloadableArtifact(job, artifact))
        throw new Error("artifact 尚未通过下载授权前置校验");
      return artifactGrantSchema.parse(
        await request(
          projectId,
          `/episodes/${job.episodeId}/timeline/versions/${job.timelineVersionId}` +
            `/export-jobs/${job.id}/artifacts/${artifact.id}/download-grants`,
          {
            method: "POST",
            body: JSON.stringify({ ttlSeconds: 300, schemaVersion: "1.0.0" }),
          },
        ),
      );
    },
    onSuccess: (value) => {
      window.open(`/api${value.accessPath}`, "_blank", "noopener,noreferrer");
    },
    onError: (error) =>
      setDiagnostic(error instanceof Error ? error.message : "下载授权失败"),
  });

  const addSelection = () => {
    const revision = Number(timelineVersionRevision);
    if (!episodeId || !timelineVersionId || !outputBaseName || revision !== 1) {
      setDiagnostic("Episode、published Version、revision 1 和输出名均为必填");
      return;
    }
    if (
      selections.some(
        (selection) =>
          selection.episodeId === episodeId ||
          selection.outputBaseName === outputBaseName,
      )
    ) {
      setDiagnostic("Episode 和输出名必须在 batch 内唯一");
      return;
    }
    setSelections((current) => [
      ...current,
      {
        episodeId,
        timelineVersionId,
        timelineVersionRevision: revision,
        outputBaseName,
      },
    ]);
    setEpisodeId("");
    setTimelineVersionId("");
    setOutputBaseName("");
    setDiagnostic("");
  };

  return (
    <section className="page-body">
      <PageIntro
        eyebrow="PROJECT EXPORTS / S10"
        title="逐集导出"
        detail="显式组装 Episode 与 published TimelineVersion；一个 batch 完成全集合 preflight，每集独立输出。"
      />
      <section className="surface export-panel">
        <div className="export-rule">
          <CircleAlert size={17} />
          <span>
            Renderer、Storage 或 artifact 未配置时保留 owner 原始诊断。
          </span>
        </div>

        <div className="export-create-grid">
          <div className="export-fields">
            <label className="setting-line">
              <span>Episode ID</span>
              <input
                value={episodeId}
                onChange={(event) => setEpisodeId(event.target.value.trim())}
                placeholder="显式 Episode owner ID"
              />
            </label>
            <label className="setting-line">
              <span>Published Version ID</span>
              <input
                value={timelineVersionId}
                onChange={(event) =>
                  setTimelineVersionId(event.target.value.trim())
                }
                placeholder="显式 TimelineVersion"
              />
            </label>
            <label className="setting-line compact-number">
              <span>Version revision</span>
              <input
                type="number"
                min="1"
                max="1"
                value={timelineVersionRevision}
                onChange={(event) =>
                  setTimelineVersionRevision(event.target.value)
                }
              />
            </label>
            <label className="setting-line">
              <span>Output base name</span>
              <input
                value={outputBaseName}
                onChange={(event) =>
                  setOutputBaseName(event.target.value.trim())
                }
                placeholder="episode-01"
              />
            </label>
            <button className="secondary-button" onClick={addSelection}>
              <Plus size={15} /> 添加成员
            </button>
          </div>

          <div className="export-selection-list" aria-label="待提交导出成员">
            <div className="export-list-heading">
              <strong>批次成员</strong>
              <span>{selections.length} 集</span>
            </div>
            {selections.length === 0 ? (
              <div className="export-empty">尚未添加 Episode</div>
            ) : (
              selections.map((selection, index) => (
                <div className="export-selection-row" key={selection.episodeId}>
                  <span className="export-index">
                    {String(index + 1).padStart(2, "0")}
                  </span>
                  <span>
                    <strong>{selection.outputBaseName}</strong>
                    <small>{selection.episodeId}</small>
                    <small>{selection.timelineVersionId}</small>
                  </span>
                  <button
                    className="icon-button"
                    title="移除成员"
                    aria-label={`移除 Episode ${selection.episodeId}`}
                    onClick={() =>
                      setSelections((current) =>
                        current.filter(
                          (item) => item.episodeId !== selection.episodeId,
                        ),
                      )
                    }
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              ))
            )}
          </div>

          <div className="export-submit-row">
            <label className="setting-line">
              <span>StorageProfile ID</span>
              <input
                value={storageProfileId}
                onChange={(event) =>
                  setStorageProfileId(event.target.value.trim())
                }
                placeholder="显式 profile ID"
              />
            </label>
            <label className="setting-line compact-number">
              <span>Profile revision</span>
              <input
                type="number"
                min="1"
                value={storageProfileRevision}
                onChange={(event) =>
                  setStorageProfileRevision(event.target.value)
                }
              />
            </label>
            <button
              className="primary-button"
              disabled={
                selections.length === 0 ||
                !storageProfileId ||
                Number(storageProfileRevision) < 1 ||
                create.isPending
              }
              onClick={() => create.mutate()}
            >
              <Upload size={15} /> 提交 {selections.length} 集导出
            </button>
          </div>
        </div>

        <div className="export-batch-loader">
          <label className="setting-line">
            <span>ExportBatch ID</span>
            <input
              value={batchId}
              onChange={(event) => setBatchId(event.target.value.trim())}
              placeholder="粘贴已创建的 batch ID"
            />
          </label>
          <button
            className="secondary-button"
            disabled={!batchId || batch.isFetching}
            onClick={() => batch.refetch()}
          >
            <RefreshCw size={15} /> 读取 batch
          </button>
        </div>

        {batch.error && <ErrorNotice error={batch.error} />}
        {diagnostic && (
          <div className="warning-line">
            <CircleAlert size={14} /> {diagnostic}
          </div>
        )}

        {batch.data && (
          <div className="export-batch-detail">
            <div className="export-list-heading">
              <span>
                <strong>{batch.data.id}</strong>
                <small>{batch.data.status}</small>
              </span>
              <button
                className="secondary-button"
                disabled={
                  selectedFailedEpisodeIds.length === 0 || retry.isPending
                }
                onClick={() => retry.mutate()}
              >
                <RotateCcw size={14} /> 重试所选失败集
              </button>
            </div>
            <div className="export-job-list">
              {[...latestJobs.values()].map((job) => (
                <div className="export-job-row" key={job.id}>
                  <div className="export-job-title">
                    {job.status === "failed" ? (
                      <input
                        type="checkbox"
                        aria-label={`重试 Episode ${job.episodeId}`}
                        checked={retryEpisodeIds.includes(job.episodeId)}
                        onChange={(event) =>
                          setRetryEpisodeIds((current) =>
                            event.target.checked
                              ? [...current, job.episodeId]
                              : current.filter((id) => id !== job.episodeId),
                          )
                        }
                      />
                    ) : (
                      <span
                        className="export-status-dot"
                        data-status={job.status}
                      />
                    )}
                    <span>
                      <strong>{job.episodeId}</strong>
                      <small>
                        {job.status}
                        {job.packagingPhase ? ` / ${job.packagingPhase}` : ""}
                      </small>
                    </span>
                  </div>
                  <div className="export-artifact-actions">
                    {job.artifacts.map((artifact) => (
                      <button
                        className="secondary-button"
                        key={artifact.id}
                        disabled={
                          !downloadableArtifact(job, artifact) ||
                          grant.isPending
                        }
                        onClick={() => grant.mutate({ job, artifact })}
                      >
                        <Download size={13} /> 下载{" "}
                        {artifactLabel(artifact.artifactType)}
                      </button>
                    ))}
                  </div>
                  {(job.rendererDiagnostic || job.diagnostics.length > 0) && (
                    <small className="export-job-diagnostic">
                      {job.rendererDiagnostic ??
                        `${job.diagnostics.length} 项 owner diagnostic`}
                    </small>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}
      </section>
    </section>
  );
}

function artifactLabel(value: ExportArtifact["artifactType"]) {
  return value === "light_manifest" ? "LIGHT" : value.toUpperCase();
}
