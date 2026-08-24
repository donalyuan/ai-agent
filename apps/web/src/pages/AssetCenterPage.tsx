import {
  AlertTriangle,
  AudioLines,
  Check,
  ChevronLeft,
  ChevronRight,
  CircleHelp,
  FileArchive,
  Image as ImageIcon,
  LoaderCircle,
  Music2,
  Pause,
  Play,
  RefreshCw,
  ShieldCheck,
  Upload,
  Video,
  X,
} from "lucide-react";
import * as Dialog from "@radix-ui/react-dialog";
import { sha256 } from "@noble/hashes/sha2.js";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { Link, useParams, useSearchParams } from "react-router";
import {
  ASSET_SCHEMA_VERSION,
  assetCenterApi,
  assetCenterQueryKeys,
  assetRequest,
} from "../asset-center/api";
import type { CatalogItem, CatalogPage } from "../asset-center/contracts";
import { useAssetCenterStore } from "../asset-center/store";
import { traceHeaders } from "../workbench/trace-context";

const PAGE_SIZE = 30;
const DEFAULT_PART_SIZE = 8 * 1024 * 1024;

function hex(value: Uint8Array): string {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
  );
}

async function hashFileParts(
  file: File,
  partSize: number,
): Promise<{ checksum: string; partChecksums: string[] }> {
  const total = sha256.create();
  const partChecksums: string[] = [];
  for (let offset = 0; offset < file.size; offset += partSize) {
    const content = new Uint8Array(
      await file
        .slice(offset, Math.min(file.size, offset + partSize))
        .arrayBuffer(),
    );
    total.update(content);
    partChecksums.push(hex(sha256(content)));
  }
  return { checksum: hex(total.digest()), partChecksums };
}

function assetIcon(kind: CatalogItem["kind"]) {
  if (kind === "audio") return AudioLines;
  if (kind === "video") return Video;
  if (kind === "image") return ImageIcon;
  return FileArchive;
}

function bytes(size: number | undefined): string {
  if (size === undefined) return "--";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  if (size < 1024 * 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  return `${(size / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function statusLabel(status: CatalogItem["processingStatus"]): string {
  return {
    unknown: "未检查",
    pending: "处理中",
    ready: "可用",
    failed: "失败",
    stale: "已过期",
  }[status];
}

function AuthBadge({ status }: { status: string }) {
  const tone =
    status === "verified"
      ? "ready"
      : status === "expired" || status === "restricted"
        ? "bad"
        : "waiting";
  return (
    <span className={`status-tag ${tone}`}>
      {status === "verified" ? (
        <ShieldCheck size={13} />
      ) : (
        <CircleHelp size={13} />
      )}
      {status}
    </span>
  );
}

function AssetCenterPage() {
  const { projectId = "" } = useParams();
  const [searchParams, setSearchParams] = useSearchParams();
  const queryClient = useQueryClient();
  const [cursor, setCursor] = useState<string | null>(null);
  const selectedId = useAssetCenterStore((state) => state.selectedAssetId);
  const selectAsset = useAssetCenterStore((state) => state.selectAsset);
  const filters = useAssetCenterStore((state) => state.filters);
  const setFilter = useAssetCenterStore((state) => state.setFilter);
  const resetInteractionFilters = useAssetCenterStore(
    (state) => state.resetFilters,
  );
  const playing = useAssetCenterStore((state) => state.playing);
  const setPlaying = useAssetCenterStore((state) => state.setPlaying);
  const enterProject = useAssetCenterStore((state) => state.enterProject);
  const [uploadOpen, setUploadOpen] = useState(false);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [uploadStage, setUploadStage] = useState<
    | "idle"
    | "preflight"
    | "reservation"
    | "uploading"
    | "verifying"
    | "registered"
    | "failed"
  >("idle");
  const [uploadProgress, setUploadProgress] = useState(0);
  const [activeReservation, setActiveReservation] = useState<{
    id: string;
    revision: number;
    fingerprint: string;
    status: string;
    storageProfileId?: string;
    storageProfileRevision?: number;
  } | null>(null);
  const [selectedProfileId, setSelectedProfileId] = useState("");
  const [activeSessionId, setActiveSessionId] = useState<string | undefined>();
  const [metadataOpen, setMetadataOpen] = useState(false);
  const [metadataTags, setMetadataTags] = useState("");
  const [metadataAuthorization, setMetadataAuthorization] = useState("unknown");
  const [metadataError, setMetadataError] = useState<string | null>(null);
  const [audioPath, setAudioPath] = useState<string | null>(null);
  const [pageHistory, setPageHistory] = useState<Array<string | null>>([]);
  const [usageTab, setUsageTab] = useState<"versions" | "usage">("versions");
  useEffect(() => enterProject(projectId), [enterProject, projectId]);

  const reservationId = searchParams.get("reservationId") ?? "";
  const recovery = useQuery({
    queryKey: ["projects", projectId, "asset-reservations", reservationId],
    queryFn: () => assetCenterApi.reservation(projectId, reservationId),
    enabled: Boolean(projectId && reservationId),
  });
  useEffect(() => {
    if (!recovery.data) return;
    const reservation = recovery.data as {
      id: string;
      revision: number;
      fingerprint: string;
      status: string;
    };
    setActiveReservation(reservation);
    if (reservation.status === "reserved") {
      setUploadOpen(true);
      setUploadStage("idle");
    }
  }, [recovery.data]);

  const query = useQuery({
    queryKey: assetCenterQueryKeys.catalog(projectId, cursor, filters),
    queryFn: (): Promise<CatalogPage> =>
      assetCenterApi.catalog(projectId, cursor, filters, PAGE_SIZE),
    enabled: Boolean(projectId),
  });
  const uploadProfiles = useQuery({
    queryKey: ["projects", projectId, "asset-upload-profiles"],
    queryFn: () => assetCenterApi.uploadProfiles(projectId),
    enabled: Boolean(projectId && uploadOpen),
  });
  const selectedProfile =
    uploadProfiles.data?.find(
      (profile) =>
        profile.storageProfileId ===
        (activeReservation?.storageProfileId ?? selectedProfileId),
    ) ?? uploadProfiles.data?.find((profile) => profile.enabled);

  const selected = useMemo(
    () =>
      query.data?.items.find((item) => item.id === selectedId) ??
      query.data?.items[0] ??
      null,
    [query.data?.items, selectedId],
  );
  const versions = useQuery({
    queryKey: assetCenterQueryKeys.versions(selected?.id ?? ""),
    queryFn: () => assetCenterApi.versions(selected?.id ?? ""),
    enabled: Boolean(selected?.id),
  });
  const media = useQuery({
    queryKey: assetCenterQueryKeys.media(selected?.latestVersion?.id ?? ""),
    queryFn: () =>
      assetCenterApi.media(projectId, selected?.latestVersion?.id ?? ""),
    enabled: Boolean(selected?.latestVersion?.id),
  });
  const usage = useQuery({
    queryKey: assetCenterQueryKeys.usage(selected?.latestVersion?.id ?? ""),
    queryFn: () =>
      assetCenterApi.usage(projectId, selected?.latestVersion?.id ?? ""),
    enabled: Boolean(selected?.latestVersion?.id && usageTab === "usage"),
  });
  const playableDerivative = useMemo(() => {
    const projection = media.data as
      | {
          derivatives?: Array<{
            id: string;
            kind: string;
            status: string;
            grantAvailable: boolean;
          }>;
        }
      | undefined;
    return projection?.derivatives?.find(
      (item) =>
        item.status === "ready" &&
        item.grantAvailable &&
        ["proxy", "waveform"].includes(item.kind),
    );
  }, [media.data]);

  const resetFilters = () => {
    setCursor(null);
    setPageHistory([]);
    resetInteractionFilters();
  };

  async function upload(file: File) {
    setUploadError(null);
    setUploadStage("preflight");
    setUploadProgress(0);
    try {
      if (!file.type || file.size === 0)
        throw new Error("文件缺少 MIME 或为空");
      if (!selectedProfile) throw new Error("没有可用的 StorageProfile");
      const kind = file.type.startsWith("image/")
        ? "image"
        : file.type.startsWith("video/")
          ? "video"
          : file.type.startsWith("audio/")
            ? "audio"
            : "document";
      const partSize = Math.min(DEFAULT_PART_SIZE, Math.max(1, file.size));
      const admission = await assetCenterApi.admitUpload(projectId, {
        storageProfileId: selectedProfile.storageProfileId,
        storageProfileRevision:
          activeReservation?.storageProfileRevision ?? selectedProfile.revision,
        declaredMimeType: file.type,
        declaredSizeBytes: file.size,
        partSizeBytes: partSize,
      });
      const { checksum, partChecksums } = await hashFileParts(file, partSize);
      let reservation = activeReservation;
      if (reservation && reservation.fingerprint !== checksum) {
        throw new Error("所选文件与待恢复 reservation fingerprint 不一致");
      }
      if (!reservation) {
        const asset = (await assetRequest(`/v1/projects/${projectId}/assets`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-Project-Scope": projectId,
          },
          body: JSON.stringify({
            kind,
            name: file.name,
            sourceType: "user_upload",
            tags: [],
            authorizationStatus: "unknown",
            schemaVersion: ASSET_SCHEMA_VERSION,
          }),
        })) as { id: string; revision: number };
        setUploadStage("reservation");
        reservation = (await assetRequest(
          `/v1/projects/${projectId}/assets/${asset.id}/reservations`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-Project-Scope": projectId,
            },
            body: JSON.stringify({
              fingerprint: checksum,
              expectedAssetRevision: asset.revision,
              declaredKind: kind,
              declaredMimeType: file.type,
              declaredSizeBytes: file.size,
              declaredChecksum: checksum,
              storageProfileId: admission.storageProfileId,
              storageProfileRevision: admission.storageProfileRevision,
              storageProfileSnapshotHash: admission.storageProfileSnapshotHash,
              partSizeBytes: partSize,
              schemaVersion: ASSET_SCHEMA_VERSION,
            }),
          },
        )) as {
          id: string;
          revision: number;
          fingerprint: string;
          status: string;
        };
        setActiveReservation(reservation);
        setSearchParams({ reservationId: reservation.id });
      }
      const session = (await assetRequest(
        `/v1/projects/${projectId}/asset-reservations/${reservation.id}/uploads/resume`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-Project-Scope": projectId,
          },
          body: JSON.stringify({
            correlationId: "asset-center-ui",
            schemaVersion: ASSET_SCHEMA_VERSION,
          }),
        },
      )) as { sessionId: string };
      setActiveSessionId(session.sessionId);
      setUploadStage("uploading");
      const parts: Array<{
        partNumber: number;
        checksum: string;
        eTag: string;
        sizeBytes: number;
      }> = [];
      let partNumber = 1;
      for (let offset = 0; offset < file.size; offset += partSize) {
        const content = new Uint8Array(
          await file
            .slice(offset, Math.min(file.size, offset + partSize))
            .arrayBuffer(),
        );
        const partHash = partChecksums[partNumber - 1];
        if (!partHash) throw new Error(`分片 ${partNumber} checksum 缺失`);
        const response = await fetch(
          `/api/v1/projects/${projectId}/asset-reservations/${reservation.id}/uploads/${session.sessionId}/parts/${partNumber}`,
          {
            method: "PUT",
            body: content,
            headers: {
              ...traceHeaders(),
              "X-Project-Scope": projectId,
              "X-Part-Checksum": partHash,
              "X-Part-ETag": partHash,
              "X-Correlation-ID": "asset-center-ui",
            },
          },
        );
        if (!response.ok)
          throw new Error(`分片 ${partNumber} 上传失败（${response.status}）`);
        const receipt = (await response.json()) as {
          partNumber: number;
          checksum: string;
          eTag: string;
          sizeBytes: number;
        };
        parts.push({
          partNumber: receipt.partNumber,
          checksum: receipt.checksum,
          eTag: receipt.eTag,
          sizeBytes: receipt.sizeBytes,
        });
        partNumber += 1;
        setUploadProgress(
          Math.round(
            (Math.min(file.size, offset + content.byteLength) / file.size) *
              100,
          ),
        );
      }
      setUploadStage("verifying");
      const version = await assetRequest(
        `/v1/projects/${projectId}/asset-reservations/${reservation.id}/uploads/complete`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-Project-Scope": projectId,
          },
          body: JSON.stringify({
            sessionId: session.sessionId,
            parts,
            correlationId: "asset-center-ui",
            schemaVersion: ASSET_SCHEMA_VERSION,
          }),
        },
      );
      if (!version) throw new Error("owner 未返回 AssetVersion");
      setUploadStage("registered");
      setUploadProgress(100);
      setActiveReservation(null);
      setActiveSessionId(undefined);
      setSearchParams({});
      await queryClient.invalidateQueries({
        queryKey: ["projects", projectId, "asset-center"],
      });
    } catch (error) {
      setUploadStage("failed");
      setUploadError(
        error instanceof Error
          ? error.message
          : "上传 owner 状态未知，请先 reconcile",
      );
    }
  }

  async function mutateUpload(action: "cancel" | "reconcile") {
    if (!activeReservation) return;
    setUploadError(null);
    try {
      const value = (await assetCenterApi.mutateReservation(
        projectId,
        activeReservation.id,
        action,
        activeReservation.revision,
        activeSessionId,
      )) as {
        id: string;
        revision: number;
        fingerprint: string;
        status: string;
      };
      setActiveReservation(value);
      if (action === "cancel") {
        setUploadStage("failed");
        setUploadError("reservation 已取消；late object 不会登记 AssetVersion");
      }
    } catch (error) {
      setUploadError(error instanceof Error ? error.message : `${action} 失败`);
    }
  }

  async function saveMetadata() {
    if (!selected) return;
    setMetadataError(null);
    try {
      await assetCenterApi.patchMetadata(selected.id, selected.revision, {
        tags: metadataTags
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean),
        authorizationStatus: metadataAuthorization,
      });
      setMetadataOpen(false);
      await queryClient.invalidateQueries({
        queryKey: ["projects", projectId, "asset-center"],
      });
    } catch (error) {
      setMetadataError(
        error instanceof Error ? error.message : "metadata CAS 失败",
      );
    }
  }

  async function toggleAudio() {
    if (playing) {
      setPlaying(false);
      setAudioPath(null);
      return;
    }
    if (!selected?.latestVersion || !playableDerivative) return;
    const grant = (await assetCenterApi.mediaGrant(
      projectId,
      selected.latestVersion.id,
      playableDerivative.id,
    )) as { accessPath: string };
    setAudioPath(`/api${grant.accessPath}`);
    setPlaying(true);
  }

  const steps = [
    ["reservation", "Reservation"],
    ["uploading", "Multipart"],
    ["verifying", "校验"],
    ["registered", "AssetVersion"],
  ] as const;

  return (
    <section className="page-body asset-center-page">
      <div className="asset-hero">
        <div>
          <span className="micro-label accent">
            PROJECT ASSET CENTER / S08a
          </span>
          <h2>素材库</h2>
          <p>
            把每个文件的来源、授权、版本和使用位置放在同一份 owner projection
            里。读取、筛选和切换详情不会产生业务副作用。
          </p>
        </div>
        <div className="asset-hero-meta">
          <span className="status-tag ready">
            <ShieldCheck size={13} /> Local profile
          </span>
          <span className="mono">adapter: local_workspace</span>
          <button
            className="primary-button"
            onClick={() => {
              setUploadOpen(true);
              setUploadStage("idle");
            }}
          >
            <Upload size={15} /> 上传素材
          </button>
        </div>
      </div>
      <div className="asset-center-layout">
        <section className="asset-catalog-column">
          <div className="asset-filter-bar surface">
            <div className="asset-filter-heading">
              <span className="micro-label">
                CATALOG /{" "}
                {(query.data?.items.length ?? 0).toString().padStart(2, "0")}
              </span>
              <button
                className="icon-button"
                title="刷新目录"
                onClick={() => query.refetch()}
              >
                <RefreshCw size={15} />
              </button>
            </div>
            <div className="asset-filter-grid">
              <label>
                类型
                <select
                  value={filters.kind}
                  onChange={(event) => {
                    setCursor(null);
                    setFilter("kind", event.target.value);
                  }}
                >
                  <option value="">全部</option>
                  <option value="image">图片</option>
                  <option value="video">视频</option>
                  <option value="audio">音频</option>
                  <option value="document">文档</option>
                </select>
              </label>
              <label>
                角色
                <select
                  value={filters.role}
                  onChange={(event) => {
                    setCursor(null);
                    setFilter("role", event.target.value);
                  }}
                >
                  <option value="">全部</option>
                  <option value="character">角色</option>
                  <option value="location">场景</option>
                  <option value="dialogue">对白</option>
                  <option value="storyboard">分镜</option>
                  <option value="other">其他</option>
                </select>
              </label>
              <label>
                处理
                <select
                  value={filters.processing}
                  onChange={(event) => {
                    setCursor(null);
                    setFilter("processing", event.target.value);
                  }}
                >
                  <option value="">全部</option>
                  <option value="ready">可用</option>
                  <option value="pending">处理中</option>
                  <option value="failed">失败</option>
                  <option value="stale">已过期</option>
                </select>
              </label>
              <label className="asset-tag-filter">
                标签
                <input
                  value={filters.tag}
                  onChange={(event) => {
                    setCursor(null);
                    setFilter("tag", event.target.value);
                  }}
                  placeholder="例如 lead"
                />
              </label>
              <label>
                来源
                <select
                  value={filters.source}
                  onChange={(event) => {
                    setCursor(null);
                    setFilter("source", event.target.value);
                  }}
                >
                  <option value="">全部</option>
                  <option value="user_upload">用户上传</option>
                  <option value="provider_generated">Provider 生成</option>
                  <option value="source_material">原始材料</option>
                  <option value="imported">历史导入</option>
                </select>
              </label>
              <label>
                授权
                <select
                  value={filters.authorization}
                  onChange={(event) => {
                    setCursor(null);
                    setFilter("authorization", event.target.value);
                  }}
                >
                  <option value="">全部</option>
                  <option value="unknown">未知</option>
                  <option value="declared">已声明</option>
                  <option value="verified">已核验</option>
                  <option value="restricted">受限</option>
                  <option value="expired">已过期</option>
                </select>
              </label>
            </div>
            <div className="asset-filter-footer">
              <span className="read-only-label">
                只读 projection · stable cursor (updatedAt, id)
              </span>
              {Object.values(filters).some(Boolean) && (
                <button className="text-button" onClick={resetFilters}>
                  清除筛选
                </button>
              )}
            </div>
          </div>
          {query.isPending && (
            <div className="data-notice loading">
              <LoaderCircle className="spin" size={15} /> 正在读取资产目录...
            </div>
          )}
          {query.error && (
            <div className="data-notice unavailable">
              <AlertTriangle size={15} />{" "}
              {query.error instanceof Error
                ? query.error.message
                : "asset catalog unavailable"}
            </div>
          )}
          {!query.isPending &&
            !query.error &&
            query.data?.items.length === 0 && (
              <div className="asset-empty surface">
                <ImageIcon size={22} />
                <strong>这里还没有素材</strong>
                <span>
                  上传第一张图片或一段音频，上传完成后才会出现 AssetVersion。
                </span>
                <button
                  className="secondary-button"
                  onClick={() => setUploadOpen(true)}
                >
                  <Upload size={14} /> 开始上传
                </button>
              </div>
            )}
          <div className="asset-list" aria-label="资产目录">
            {query.data?.items.map((asset) => {
              const Icon = assetIcon(asset.kind);
              return (
                <button
                  key={asset.id}
                  className={`asset-list-row ${selected?.id === asset.id ? "selected" : ""}`}
                  onClick={() => {
                    selectAsset(asset.id);
                    setUsageTab("versions");
                  }}
                >
                  <span className={`asset-kind-icon kind-${asset.kind}`}>
                    <Icon size={17} />
                  </span>
                  <span className="asset-list-main">
                    <strong>{asset.name}</strong>
                    <span>
                      {asset.tags.length
                        ? asset.tags.map((tag) => `#${tag}`).join(" ")
                        : "无标签"}{" "}
                      · rev {asset.revision}
                    </span>
                  </span>
                  <span className="asset-list-status">
                    <span
                      className={`status-tag ${asset.processingStatus === "ready" ? "ready" : asset.processingStatus === "failed" ? "bad" : asset.processingStatus === "pending" ? "running" : "neutral"}`}
                    >
                      {statusLabel(asset.processingStatus)}
                    </span>
                    <small>{asset.versionCount} 个版本</small>
                  </span>
                  <ChevronRight size={16} />
                </button>
              );
            })}
          </div>
          <div className="asset-pagination">
            <button
              className="icon-button"
              title="上一页"
              disabled={!pageHistory.length}
              onClick={() => {
                const next = pageHistory.at(-1) ?? null;
                setPageHistory((old) => old.slice(0, -1));
                setCursor(next);
              }}
            >
              <ChevronLeft size={16} />
            </button>
            <span className="mono">{query.data?.items.length ?? 0} / page</span>
            <button
              className="icon-button"
              title="下一页"
              disabled={!query.data?.nextCursor}
              onClick={() => {
                if (query.data?.nextCursor) {
                  setPageHistory((old) => [...old, cursor]);
                  setCursor(query.data.nextCursor);
                }
              }}
            >
              <ChevronRight size={16} />
            </button>
          </div>
        </section>
        <aside className="asset-inspector surface">
          {!selected && (
            <div className="inspector-empty">
              <BoxesMark />
              <strong>选择一个素材</strong>
              <span>版本、授权和使用位置会在这里展开。</span>
            </div>
          )}
          {selected && (
            <>
              <div className="inspector-heading">
                <div>
                  <span className="micro-label">
                    ASSET / {selected.id.slice(0, 8)}
                  </span>
                  <h3>{selected.name}</h3>
                </div>
                <AuthBadge status={selected.authorizationStatus} />
              </div>
              <div className="inspector-meta">
                <span>
                  {selected.kind} · {selected.sourceType}
                </span>
                <span className="mono">revision {selected.revision}</span>
              </div>
              {!metadataOpen ? (
                <button
                  className="text-button metadata-edit-button"
                  onClick={() => {
                    setMetadataTags(selected.tags.join(", "));
                    setMetadataAuthorization(selected.authorizationStatus);
                    setMetadataOpen(true);
                    setMetadataError(null);
                  }}
                >
                  编辑标签与授权
                </button>
              ) : (
                <div className="metadata-editor">
                  <label>
                    标签（逗号分隔）
                    <input
                      value={metadataTags}
                      onChange={(event) => setMetadataTags(event.target.value)}
                    />
                  </label>
                  <label>
                    授权状态
                    <select
                      value={metadataAuthorization}
                      onChange={(event) =>
                        setMetadataAuthorization(event.target.value)
                      }
                    >
                      <option value="unknown">unknown</option>
                      <option value="declared">declared</option>
                      <option value="verified">verified</option>
                      <option value="restricted">restricted</option>
                      <option value="expired">expired</option>
                    </select>
                  </label>
                  <div className="metadata-actions">
                    <button
                      className="secondary-button"
                      onClick={() => setMetadataOpen(false)}
                    >
                      取消
                    </button>
                    <button className="primary-button" onClick={saveMetadata}>
                      保存变更
                    </button>
                  </div>
                  {metadataError && (
                    <div className="warning-line">
                      <AlertTriangle size={14} /> {metadataError}
                    </div>
                  )}
                </div>
              )}
              <div className="asset-preview-frame">
                <div className={`preview-mark kind-${selected.kind}`}>
                  {selected.kind === "audio" ? (
                    <Music2 size={30} />
                  ) : selected.kind === "video" ? (
                    <Video size={30} />
                  ) : (
                    <ImageIcon size={30} />
                  )}
                </div>
                <span>
                  {selected.latestVersion
                    ? `${selected.latestVersion.mimeType} · ${bytes(selected.latestVersion.sizeBytes)}`
                    : "尚未登记版本"}
                </span>
              </div>
              <div className="asset-tabbar">
                <button
                  className={usageTab === "versions" ? "active" : ""}
                  onClick={() => setUsageTab("versions")}
                >
                  版本与派生
                </button>
                <button
                  className={usageTab === "usage" ? "active" : ""}
                  onClick={() => setUsageTab("usage")}
                >
                  使用位置
                </button>
              </div>
              {usageTab === "versions" && (
                <div className="asset-detail-content">
                  <div className="detail-grid">
                    <span>
                      目录角色
                      <strong>{selected.catalogRole ?? "未指定"}</strong>
                    </span>
                    <span>
                      许可证<strong>{selected.licenseLabel ?? "未声明"}</strong>
                    </span>
                    <span>
                      更新时间
                      <strong>
                        {new Date(selected.updatedAt).toLocaleDateString(
                          "zh-CN",
                        )}
                      </strong>
                    </span>
                    <span>
                      版本数量<strong>{selected.versionCount}</strong>
                    </span>
                  </div>
                  <div className="derivative-strip">
                    <span className="micro-label">MEDIA READINESS</span>
                    {media.isPending && (
                      <span className="muted">读取中...</span>
                    )}
                    {media.error && (
                      <span className="warning-text">不可用</span>
                    )}
                    {!media.isPending && !media.error && (
                      <div className="derivative-list">
                        {(
                          (
                            media.data as
                              | {
                                  derivatives?: Array<{
                                    kind: string;
                                    status: string;
                                    grantAvailable: boolean;
                                  }>;
                                }
                              | undefined
                          )?.derivatives ?? []
                        ).map((item) => (
                          <span
                            key={item.kind}
                            className={
                              item.status === "ready" && item.grantAvailable
                                ? "derivative ready"
                                : "derivative"
                            }
                          >
                            {item.kind.replace("_", " ")} · {item.status}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                  {selected.kind === "audio" && (
                    <div className="audio-bar">
                      <button
                        className="icon-button"
                        title={playing ? "暂停试听" : "试听"}
                        onClick={() => void toggleAudio()}
                        disabled={!playableDerivative}
                      >
                        <span>
                          {playing ? <Pause size={16} /> : <Play size={16} />}
                        </span>
                      </button>
                      <div className="audio-wave">
                        <span />
                        <span />
                        <span />
                        <span />
                        <span />
                        <span />
                        <span />
                        <span />
                        <span />
                        <span />
                      </div>
                      <span className="mono">
                        {selected.latestVersion?.durationMs
                          ? `${Math.round(selected.latestVersion.durationMs / 1000)}s`
                          : "--"}
                      </span>
                      {audioPath && (
                        <audio
                          className="asset-audio"
                          src={audioPath}
                          controls
                          autoPlay
                          onEnded={() => {
                            setPlaying(false);
                            setAudioPath(null);
                          }}
                        />
                      )}
                    </div>
                  )}
                  <div className="version-list">
                    {versions.isPending && (
                      <span className="muted">读取版本历史...</span>
                    )}
                    {Array.isArray(versions.data) &&
                      (
                        versions.data as Array<{
                          id: string;
                          versionNumber?: number;
                          revision: number;
                          contentHash: string;
                          mimeType: string;
                          sizeBytes: number;
                        }>
                      ).map((version) => (
                        <div key={version.id} className="version-line">
                          <span className="version-badge">
                            v{version.versionNumber ?? "?"}
                          </span>
                          <span className="mono">
                            {version.contentHash.slice(0, 12)}...
                          </span>
                          <span>
                            {version.mimeType} · {bytes(version.sizeBytes)}
                          </span>
                          <span className="muted">rev {version.revision}</span>
                        </div>
                      ))}
                  </div>
                </div>
              )}
              {usageTab === "usage" && (
                <div className="asset-detail-content">
                  {usage.isPending && (
                    <span className="muted">读取使用位置...</span>
                  )}
                  {usage.error && (
                    <div className="data-notice unavailable">
                      <AlertTriangle size={14} /> usage projection unavailable
                    </div>
                  )}
                  {usage.data !== undefined && (
                    <>
                      <div
                        className={`usage-state ${(usage.data as { status?: string }).status}`}
                      >
                        {(usage.data as { status?: string }).status} ·{" "}
                        {
                          (
                            (usage.data as { references?: unknown[] })
                              .references ?? []
                          ).length
                        }{" "}
                        个引用
                      </div>
                      <div className="usage-list">
                        {(
                          (
                            usage.data as {
                              references?: Array<{
                                ownerType: string;
                                ownerId: string;
                                ownerRevision: number;
                                state: string;
                                deepLink: string;
                              }>;
                            }
                          ).references ?? []
                        ).map((reference) => (
                          <Link
                            key={`${reference.ownerType}-${reference.ownerId}`}
                            to={reference.deepLink}
                            className="usage-line"
                          >
                            <span>{reference.ownerType}</span>
                            <strong>{reference.state}</strong>
                            <small>rev {reference.ownerRevision}</small>
                            <ChevronRight size={14} />
                          </Link>
                        ))}
                      </div>
                      {(
                        (usage.data as { unavailableOwners?: string[] })
                          .unavailableOwners ?? []
                      ).length > 0 && (
                        <div className="warning-line">
                          <AlertTriangle size={14} /> owner unavailable:{" "}
                          {(
                            usage.data as { unavailableOwners: string[] }
                          ).unavailableOwners.join(", ")}
                        </div>
                      )}
                    </>
                  )}
                </div>
              )}
              <div className="inspector-footer">
                <Link
                  className="secondary-button"
                  to={
                    selected.latestVersion
                      ? `/projects/${projectId}/episodes/select/timeline?assetVersionId=${encodeURIComponent(selected.latestVersion.id)}&assetVersionRevision=${selected.latestVersion.revision}&assetVersionHash=${encodeURIComponent(selected.latestVersion.contentHash)}`
                      : `/projects/${projectId}/episodes/select/timeline`
                  }
                >
                  <Video size={14} /> 交给 Timeline
                </Link>
                <Link
                  className="text-button"
                  to={`/projects/${projectId}/review`}
                >
                  候选审核
                </Link>
              </div>
            </>
          )}
        </aside>
      </div>
      <Dialog.Root open={uploadOpen} onOpenChange={setUploadOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="modal-backdrop">
            <Dialog.Content
              className="upload-modal surface"
              aria-labelledby="upload-title"
            >
              <div className="modal-heading">
                <div>
                  <span className="micro-label accent">
                    LOCAL UPLOAD / OWNER HANDOFF
                  </span>
                  <Dialog.Title asChild>
                    <h3 id="upload-title">上传素材</h3>
                  </Dialog.Title>
                </div>
                <Dialog.Close asChild>
                  <button className="icon-button" title="关闭">
                    <X size={17} />
                  </button>
                </Dialog.Close>
              </div>
              {activeReservation && (
                <div className="reservation-recovery">
                  <div>
                    <span className="micro-label">RECOVERED RESERVATION</span>
                    <strong className="mono">{activeReservation.id}</strong>
                    <small>
                      {activeReservation.status} / rev{" "}
                      {activeReservation.revision}
                    </small>
                  </div>
                  <div>
                    <button
                      className="secondary-button"
                      onClick={() => void mutateUpload("reconcile")}
                    >
                      <RefreshCw size={14} /> Reconcile
                    </button>
                    <button
                      className="danger-button"
                      onClick={() => void mutateUpload("cancel")}
                    >
                      <X size={14} /> 取消上传
                    </button>
                  </div>
                </div>
              )}
              {uploadStage === "idle" && (
                <>
                  <label className="setting-line">
                    <span>StorageProfile</span>
                    <select
                      aria-label="StorageProfile"
                      value={selectedProfile?.storageProfileId ?? ""}
                      disabled={
                        Boolean(activeReservation) || uploadProfiles.isPending
                      }
                      onChange={(event) =>
                        setSelectedProfileId(event.target.value)
                      }
                    >
                      {(uploadProfiles.data ?? []).map((profile) => (
                        <option
                          key={profile.storageProfileId}
                          value={profile.storageProfileId}
                          disabled={!profile.enabled}
                        >
                          {profile.name} / {profile.adapterKey} / rev{" "}
                          {profile.revision}
                        </option>
                      ))}
                    </select>
                  </label>
                  {uploadProfiles.error && (
                    <div className="data-notice unavailable">
                      <AlertTriangle size={14} />
                      {uploadProfiles.error instanceof Error
                        ? uploadProfiles.error.message
                        : "StorageProfile projection unavailable"}
                    </div>
                  )}
                  <label className="drop-zone">
                    <Upload size={25} />
                    <strong>选择图片、视频、音频或文档</strong>
                    <span>
                      {activeReservation
                        ? "重新选择 fingerprint 匹配的原文件，将恢复同一 UploadSession。"
                        : "文件 metadata 先经过 owner admission；通过后才分片读取和哈希。"}
                    </span>
                    <input
                      type="file"
                      disabled={!selectedProfile}
                      onChange={(event) => {
                        const file = event.target.files?.[0];
                        if (file) void upload(file);
                      }}
                    />
                  </label>
                </>
              )}
              {uploadStage !== "idle" && (
                <>
                  <div className="upload-steps">
                    {steps.map(([key, label]) => (
                      <div
                        key={key}
                        className={
                          uploadStage === key ||
                          (uploadStage === "failed" && key === "uploading")
                            ? "active"
                            : steps.findIndex(([name]) => name === key) <
                                steps.findIndex(
                                  ([name]) => name === uploadStage,
                                )
                              ? "done"
                              : ""
                        }
                      >
                        <span>
                          {steps.findIndex(([name]) => name === key) <
                          steps.findIndex(([name]) => name === uploadStage) ? (
                            <Check size={13} />
                          ) : (
                            steps.findIndex(([name]) => name === uploadStage) +
                            1
                          )}
                        </span>
                        {label}
                      </div>
                    ))}
                  </div>
                  <div className="upload-progress">
                    <div style={{ width: `${uploadProgress}%` }} />
                  </div>
                  <div className="upload-status">
                    <strong>
                      {uploadStage === "registered"
                        ? "已登记 AssetVersion"
                        : uploadStage === "failed"
                          ? "上传状态需要诊断"
                          : uploadStage === "preflight"
                            ? "校验文件和 profile capability"
                            : uploadStage === "reservation"
                              ? "创建 reservation"
                              : uploadStage === "uploading"
                                ? `上传 multipart · ${uploadProgress}%`
                                : "校验 StoredObject"}
                    </strong>
                    <span>
                      {uploadStage === "registered"
                        ? "同一 operation 可安全重试，不会创建第二版本。"
                        : (uploadError ?? "owner 正在处理，不要重复点击。")}
                    </span>
                  </div>
                  {uploadStage === "failed" && (
                    <div className="data-notice unavailable">
                      <AlertTriangle size={14} />{" "}
                      {uploadError ?? "submission_unknown，请先 reconcile"}
                    </div>
                  )}
                </>
              )}
              {uploadStage === "registered" && (
                <button
                  className="primary-button full"
                  onClick={() => {
                    setUploadOpen(false);
                    void queryClient.invalidateQueries({
                      queryKey: ["projects", projectId, "asset-center"],
                    });
                  }}
                >
                  完成
                </button>
              )}
            </Dialog.Content>
          </Dialog.Overlay>
        </Dialog.Portal>
      </Dialog.Root>
    </section>
  );
}

function BoxesMark() {
  return (
    <span className="inspector-empty-mark">
      <Upload size={21} />
    </span>
  );
}

export { AssetCenterPage };
